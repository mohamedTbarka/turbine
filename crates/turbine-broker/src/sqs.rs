//! AWS SQS broker implementation
//!
//! This implementation supports:
//! - Standard queues (at-least-once delivery)
//! - FIFO queues (exactly-once delivery)
//! - Long polling for efficient message retrieval
//! - Message deduplication
//! - Dead letter queues via redrive policy

use crate::traits::{Broker, BrokerError, BrokerResult, BrokerStats, Consumer, ConsumerConfig};
use async_trait::async_trait;
use aws_config::BehaviorVersion;
use aws_sdk_sqs::{
    config::Region,
    types::{MessageAttributeValue, QueueAttributeName},
    Client,
};
use base64::Engine;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use turbine_core::{Delivery, Message, Serializer, TaskId};
use uuid::Uuid;

/// SQS broker implementation
#[derive(Clone)]
pub struct SqsBroker {
    client: Client,
    config: Arc<SqsConfig>,
    /// Queue URL cache
    queue_urls: Arc<RwLock<HashMap<String, String>>>,
}

/// Configuration for SQS broker
#[derive(Debug, Clone)]
pub struct SqsConfig {
    /// AWS region
    pub region: String,

    /// Queue name prefix
    pub queue_prefix: String,

    /// Default serializer
    pub serializer: Serializer,

    /// Use FIFO queues
    pub use_fifo: bool,

    /// Long polling wait time (0-20 seconds)
    pub wait_time_seconds: i32,

    /// Visibility timeout in seconds
    pub visibility_timeout: i32,

    /// Message retention period in seconds (60 - 1209600)
    pub message_retention: i32,

    /// Enable dead letter queue
    pub enable_dlq: bool,

    /// Max receive count before sending to DLQ
    pub max_receive_count: i32,

    /// Custom endpoint URL (for LocalStack, etc.)
    pub endpoint_url: Option<String>,
}

impl Default for SqsConfig {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            queue_prefix: "turbine".to_string(),
            serializer: Serializer::MessagePack,
            use_fifo: false,
            wait_time_seconds: 20, // Maximum long polling
            visibility_timeout: 1800, // 30 minutes
            message_retention: 345600, // 4 days
            enable_dlq: true,
            max_receive_count: 3,
            endpoint_url: None,
        }
    }
}

impl SqsBroker {
    /// Create a new SQS broker with custom config
    pub async fn with_config(config: SqsConfig) -> BrokerResult<Self> {
        let aws_config = if let Some(endpoint) = &config.endpoint_url {
            aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(config.region.clone()))
                .endpoint_url(endpoint)
                .load()
                .await
        } else {
            aws_config::defaults(BehaviorVersion::latest())
                .region(Region::new(config.region.clone()))
                .load()
                .await
        };

        let client = Client::new(&aws_config);

        // Test connection by listing queues
        client
            .list_queues()
            .max_results(1)
            .send()
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        info!("Connected to AWS SQS in region {}", config.region);

        Ok(Self {
            client,
            config: Arc::new(config),
            queue_urls: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get queue name with prefix
    fn queue_name(&self, queue: &str) -> String {
        if self.config.use_fifo {
            format!("{}-{}.fifo", self.config.queue_prefix, queue)
        } else {
            format!("{}-{}", self.config.queue_prefix, queue)
        }
    }

    /// Get DLQ name
    fn dlq_queue_name(&self, queue: &str) -> String {
        if self.config.use_fifo {
            format!("{}-{}-dlq.fifo", self.config.queue_prefix, queue)
        } else {
            format!("{}-{}-dlq", self.config.queue_prefix, queue)
        }
    }

    /// Get queue URL (cached)
    async fn get_queue_url(&self, queue: &str) -> BrokerResult<String> {
        let name = self.queue_name(queue);

        // Check cache
        {
            let cache = self.queue_urls.read().await;
            if let Some(url) = cache.get(&name) {
                return Ok(url.clone());
            }
        }

        // Fetch from SQS
        let result = self
            .client
            .get_queue_url()
            .queue_name(&name)
            .send()
            .await
            .map_err(|e| BrokerError::QueueNotFound(e.to_string()))?;

        let url = result
            .queue_url()
            .ok_or_else(|| BrokerError::QueueNotFound(name.clone()))?
            .to_string();

        // Cache it
        {
            let mut cache = self.queue_urls.write().await;
            cache.insert(name, url.clone());
        }

        Ok(url)
    }

    /// Serialize a message
    fn serialize_message(&self, message: &Message) -> BrokerResult<Vec<u8>> {
        self.config
            .serializer
            .serialize(message)
            .map(|b| b.to_vec())
            .map_err(|e| BrokerError::Serialization(e.to_string()))
    }

    /// Deserialize a message
    fn deserialize_message(&self, data: &[u8]) -> BrokerResult<Message> {
        self.config
            .serializer
            .deserialize(data)
            .map_err(|e| BrokerError::Serialization(e.to_string()))
    }

    /// Create DLQ and return its ARN
    async fn create_dlq(&self, queue: &str) -> BrokerResult<String> {
        let dlq_name = self.dlq_queue_name(queue);

        let mut attributes = HashMap::new();
        attributes.insert(
            QueueAttributeName::MessageRetentionPeriod,
            "1209600".to_string(), // 14 days for DLQ
        );

        if self.config.use_fifo {
            attributes.insert(QueueAttributeName::FifoQueue, "true".to_string());
        }

        let result = self
            .client
            .create_queue()
            .queue_name(&dlq_name)
            .set_attributes(Some(attributes))
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let dlq_url = result
            .queue_url()
            .ok_or_else(|| BrokerError::Internal("No DLQ URL returned".to_string()))?;

        // Get DLQ ARN
        let attrs = self
            .client
            .get_queue_attributes()
            .queue_url(dlq_url)
            .attribute_names(QueueAttributeName::QueueArn)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let arn = attrs
            .attributes()
            .and_then(|a| a.get(&QueueAttributeName::QueueArn))
            .ok_or_else(|| BrokerError::Internal("No DLQ ARN returned".to_string()))?
            .clone();

        Ok(arn)
    }
}

#[async_trait]
impl Broker for SqsBroker {
    type Consumer = SqsConsumer;

    async fn connect(url: &str) -> BrokerResult<Self> {
        // URL format: sqs://region or sqs://region?endpoint=http://localhost:4566
        let config = if url.contains("endpoint=") {
            let parts: Vec<&str> = url.split('?').collect();
            let region = parts[0].trim_start_matches("sqs://");
            let endpoint = parts
                .get(1)
                .and_then(|p| p.strip_prefix("endpoint="))
                .map(|s| s.to_string());

            SqsConfig {
                region: region.to_string(),
                endpoint_url: endpoint,
                ..Default::default()
            }
        } else {
            let region = url.trim_start_matches("sqs://");
            SqsConfig {
                region: region.to_string(),
                ..Default::default()
            }
        };

        Self::with_config(config).await
    }

    async fn is_connected(&self) -> bool {
        self.client.list_queues().max_results(1).send().await.is_ok()
    }

    async fn close(&self) -> BrokerResult<()> {
        Ok(())
    }

    async fn declare_queue(&self, queue: &str) -> BrokerResult<()> {
        let name = self.queue_name(queue);

        let mut attributes = HashMap::new();
        attributes.insert(
            QueueAttributeName::VisibilityTimeout,
            self.config.visibility_timeout.to_string(),
        );
        attributes.insert(
            QueueAttributeName::MessageRetentionPeriod,
            self.config.message_retention.to_string(),
        );
        attributes.insert(
            QueueAttributeName::ReceiveMessageWaitTimeSeconds,
            self.config.wait_time_seconds.to_string(),
        );

        if self.config.use_fifo {
            attributes.insert(QueueAttributeName::FifoQueue, "true".to_string());
            attributes.insert(
                QueueAttributeName::ContentBasedDeduplication,
                "true".to_string(),
            );
        }

        // Create DLQ first if enabled
        if self.config.enable_dlq {
            let dlq_arn = self.create_dlq(queue).await?;

            let redrive_policy = serde_json::json!({
                "deadLetterTargetArn": dlq_arn,
                "maxReceiveCount": self.config.max_receive_count
            });
            attributes.insert(
                QueueAttributeName::RedrivePolicy,
                redrive_policy.to_string(),
            );
        }

        let result = self
            .client
            .create_queue()
            .queue_name(&name)
            .set_attributes(Some(attributes))
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        if let Some(url) = result.queue_url() {
            let mut cache = self.queue_urls.write().await;
            cache.insert(name.clone(), url.to_string());
        }

        debug!("Declared SQS queue: {}", name);
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> BrokerResult<()> {
        let url = self.get_queue_url(queue).await?;

        self.client
            .delete_queue()
            .queue_url(&url)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Also delete DLQ
        if self.config.enable_dlq {
            let dlq_name = self.dlq_queue_name(queue);
            if let Ok(dlq_url) = self.get_queue_url(&dlq_name).await {
                let _ = self.client.delete_queue().queue_url(&dlq_url).send().await;
            }
        }

        // Clear cache
        {
            let mut cache = self.queue_urls.write().await;
            cache.remove(&self.queue_name(queue));
        }

        Ok(())
    }

    async fn queue_length(&self, queue: &str) -> BrokerResult<usize> {
        let url = self.get_queue_url(queue).await?;

        let result = self
            .client
            .get_queue_attributes()
            .queue_url(&url)
            .attribute_names(QueueAttributeName::ApproximateNumberOfMessages)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let count = result
            .attributes()
            .and_then(|a| a.get(&QueueAttributeName::ApproximateNumberOfMessages))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);

        Ok(count)
    }

    async fn publish(&self, queue: &str, message: &Message) -> BrokerResult<()> {
        // Ensure queue exists
        self.declare_queue(queue).await?;
        let url = self.get_queue_url(queue).await?;

        let data = self.serialize_message(message)?;
        let body = base64::engine::general_purpose::STANDARD.encode(&data);

        // Build message attributes
        let mut attributes = HashMap::new();
        attributes.insert(
            "TaskId".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(message.task_id().to_string())
                .build()
                .unwrap(),
        );
        attributes.insert(
            "TaskName".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(&message.headers.task_name)
                .build()
                .unwrap(),
        );

        let mut request = self
            .client
            .send_message()
            .queue_url(&url)
            .message_body(&body)
            .set_message_attributes(Some(attributes));

        // FIFO requires message group ID and deduplication ID
        if self.config.use_fifo {
            let group_id = message.headers.queue.clone();
            let dedup_id = message
                .headers
                .correlation_id
                .clone()
                .unwrap_or_else(|| message.task_id().to_string());

            request = request
                .message_group_id(&group_id)
                .message_deduplication_id(&dedup_id);
        }

        request
            .send()
            .await
            .map_err(|e| BrokerError::Publish(e.to_string()))?;

        debug!("Published message {} to SQS queue {}", message.task_id(), queue);
        Ok(())
    }

    async fn publish_delayed(
        &self,
        queue: &str,
        message: &Message,
        delay: Duration,
    ) -> BrokerResult<()> {
        self.declare_queue(queue).await?;
        let url = self.get_queue_url(queue).await?;

        let data = self.serialize_message(message)?;
        let body = base64::engine::general_purpose::STANDARD.encode(&data);

        // SQS supports up to 15 minutes delay
        let delay_seconds = delay.as_secs().min(900) as i32;

        let mut request = self
            .client
            .send_message()
            .queue_url(&url)
            .message_body(&body)
            .delay_seconds(delay_seconds);

        if self.config.use_fifo {
            request = request
                .message_group_id(&message.headers.queue)
                .message_deduplication_id(message.task_id().to_string());
        }

        request
            .send()
            .await
            .map_err(|e| BrokerError::Publish(e.to_string()))?;

        debug!(
            "Scheduled message {} for delivery in {:?}",
            message.task_id(),
            delay
        );
        Ok(())
    }

    async fn consume(&self, config: ConsumerConfig) -> BrokerResult<Self::Consumer> {
        Ok(SqsConsumer::new(self.clone(), config))
    }

    async fn get_message(&self, _queue: &str, _task_id: &TaskId) -> BrokerResult<Option<Message>> {
        warn!("get_message not supported in SQS");
        Ok(None)
    }

    async fn revoke(&self, _queue: &str, _task_id: &TaskId) -> BrokerResult<bool> {
        warn!("revoke not supported in SQS");
        Ok(false)
    }

    async fn purge(&self, queue: &str) -> BrokerResult<usize> {
        let url = self.get_queue_url(queue).await?;
        let length = self.queue_length(queue).await?;

        self.client
            .purge_queue()
            .queue_url(&url)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        Ok(length)
    }

    async fn stats(&self) -> BrokerResult<BrokerStats> {
        Ok(BrokerStats::default())
    }

    async fn publish_to_dlq(
        &self,
        queue: &str,
        message: &Message,
        reason: &str,
    ) -> BrokerResult<()> {
        let dlq_name = self.dlq_queue_name(queue);

        // Ensure DLQ exists and get URL
        let _ = self.create_dlq(queue).await?;

        let dlq_url = self
            .client
            .get_queue_url()
            .queue_name(&dlq_name)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?
            .queue_url()
            .unwrap_or_default()
            .to_string();

        let data = self.serialize_message(message)?;
        let body = base64::engine::general_purpose::STANDARD.encode(&data);

        // Build message attributes
        let mut attributes = HashMap::new();
        attributes.insert(
            "DeathReason".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(reason)
                .build()
                .unwrap(),
        );
        attributes.insert(
            "OriginalQueue".to_string(),
            MessageAttributeValue::builder()
                .data_type("String")
                .string_value(queue)
                .build()
                .unwrap(),
        );

        let mut request = self
            .client
            .send_message()
            .queue_url(&dlq_url)
            .message_body(&body)
            .set_message_attributes(Some(attributes));

        if self.config.use_fifo {
            request = request
                .message_group_id(&message.headers.queue)
                .message_deduplication_id(format!("{}-dlq", message.task_id()));
        }

        request
            .send()
            .await
            .map_err(|e| BrokerError::Publish(e.to_string()))?;

        warn!(
            "Message {} moved to DLQ for queue {}: {}",
            message.task_id(),
            queue,
            reason
        );
        Ok(())
    }

    async fn dlq_length(&self, queue: &str) -> BrokerResult<usize> {
        let dlq_name = self.dlq_queue_name(queue);

        let dlq_url = self
            .client
            .get_queue_url()
            .queue_name(&dlq_name)
            .send()
            .await
            .map_err(|e| BrokerError::QueueNotFound(e.to_string()))?
            .queue_url()
            .unwrap_or_default()
            .to_string();

        let result = self
            .client
            .get_queue_attributes()
            .queue_url(&dlq_url)
            .attribute_names(QueueAttributeName::ApproximateNumberOfMessages)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let count = result
            .attributes()
            .and_then(|a| a.get(&QueueAttributeName::ApproximateNumberOfMessages))
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(0);

        Ok(count)
    }

    async fn reprocess_from_dlq(&self, _queue: &str, _task_id: &TaskId) -> BrokerResult<bool> {
        warn!("reprocess_from_dlq requires manual implementation in SQS");
        Ok(false)
    }

    async fn purge_dlq(&self, queue: &str) -> BrokerResult<usize> {
        let dlq_name = self.dlq_queue_name(queue);

        let dlq_url = self
            .client
            .get_queue_url()
            .queue_name(&dlq_name)
            .send()
            .await
            .map_err(|e| BrokerError::QueueNotFound(e.to_string()))?
            .queue_url()
            .unwrap_or_default()
            .to_string();

        let length = self.dlq_length(queue).await?;

        self.client
            .purge_queue()
            .queue_url(&dlq_url)
            .send()
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        Ok(length)
    }
}

/// SQS consumer implementation
pub struct SqsConsumer {
    broker: SqsBroker,
    config: ConsumerConfig,
    /// Pending messages for ack/nack
    pending: Arc<RwLock<HashMap<String, (String, String, Vec<u8>)>>>, // tag -> (queue_url, receipt_handle, data)
    current_queue_index: usize,
}

impl SqsConsumer {
    fn new(broker: SqsBroker, config: ConsumerConfig) -> Self {
        Self {
            broker,
            config,
            pending: Arc::new(RwLock::new(HashMap::new())),
            current_queue_index: 0,
        }
    }

    fn next_queue(&mut self) -> &str {
        let queue = &self.config.queues[self.current_queue_index];
        self.current_queue_index = (self.current_queue_index + 1) % self.config.queues.len();
        queue
    }
}

#[async_trait]
impl Consumer for SqsConsumer {
    async fn next(&mut self) -> Option<BrokerResult<Delivery>> {
        let queue = self.next_queue().to_string();

        let url = match self.broker.get_queue_url(&queue).await {
            Ok(u) => u,
            Err(e) => return Some(Err(e)),
        };

        let result = self
            .broker
            .client
            .receive_message()
            .queue_url(&url)
            .max_number_of_messages(1)
            .wait_time_seconds(self.config.block_timeout.as_secs() as i32)
            .visibility_timeout(self.config.visibility_timeout.as_secs() as i32)
            .message_attribute_names("All")
            .send()
            .await;

        match result {
            Ok(response) => {
                let messages = response.messages();
                if let Some(sqs_msg) = messages.first() {
                    let body = sqs_msg.body().unwrap_or_default();
                    let receipt_handle = sqs_msg.receipt_handle().unwrap_or_default().to_string();

                    // Decode base64 body
                    let data = match base64::engine::general_purpose::STANDARD.decode(body) {
                        Ok(d) => d,
                        Err(e) => {
                            return Some(Err(BrokerError::Serialization(e.to_string())))
                        }
                    };

                    match self.broker.deserialize_message(&data) {
                        Ok(message) => {
                            let tag = Uuid::new_v4().to_string();

                            // Store for ack/nack
                            {
                                let mut pending = self.pending.write().await;
                                pending.insert(
                                    tag.clone(),
                                    (url.clone(), receipt_handle, data),
                                );
                            }

                            return Some(Ok(Delivery::new(message, tag, queue)));
                        }
                        Err(e) => return Some(Err(e)),
                    }
                }
                None // No messages
            }
            Err(e) => Some(Err(BrokerError::Consume(e.to_string()))),
        }
    }

    async fn ack(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (url, receipt_handle, _data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        self.broker
            .client
            .delete_message()
            .queue_url(&url)
            .receipt_handle(&receipt_handle)
            .send()
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Acknowledged SQS message {}", delivery_tag);
        Ok(())
    }

    async fn nack(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (url, receipt_handle, _data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        // Make message visible again immediately
        self.broker
            .client
            .change_message_visibility()
            .queue_url(&url)
            .receipt_handle(&receipt_handle)
            .visibility_timeout(0)
            .send()
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Nacked SQS message {}", delivery_tag);
        Ok(())
    }

    async fn nack_with_delay(&self, delivery_tag: &str, delay: Duration) -> BrokerResult<()> {
        let (url, receipt_handle, data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        // Delete original message
        self.broker
            .client
            .delete_message()
            .queue_url(&url)
            .receipt_handle(&receipt_handle)
            .send()
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        // Republish with delay
        if let Ok(mut message) = self.broker.deserialize_message(&data) {
            message.headers.retries += 1;
            // Extract queue name from URL (last part)
            let queue = url.split('/').last().unwrap_or("default");
            let queue = queue
                .strip_prefix(&format!("{}-", self.broker.config.queue_prefix))
                .unwrap_or(queue);
            self.broker.publish_delayed(queue, &message, delay).await?;
        }

        debug!("Nacked SQS message {}, scheduled retry in {:?}", delivery_tag, delay);
        Ok(())
    }

    async fn reject(&self, delivery_tag: &str) -> BrokerResult<()> {
        // In SQS, reject is the same as ack (delete the message)
        self.ack(delivery_tag).await
    }

    async fn reject_to_dlq(&self, delivery_tag: &str, reason: &str) -> BrokerResult<()> {
        let (url, receipt_handle, data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        // Delete from main queue
        self.broker
            .client
            .delete_message()
            .queue_url(&url)
            .receipt_handle(&receipt_handle)
            .send()
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        // Publish to DLQ
        if let Ok(message) = self.broker.deserialize_message(&data) {
            let queue = url.split('/').last().unwrap_or("default");
            let queue = queue
                .strip_prefix(&format!("{}-", self.broker.config.queue_prefix))
                .unwrap_or(queue);
            self.broker.publish_to_dlq(queue, &message, reason).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require AWS credentials or LocalStack
    // Run with: cargo test --package turbine-broker --features sqs -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_sqs_broker_connect() {
        let config = SqsConfig {
            endpoint_url: Some("http://localhost:4566".to_string()),
            ..Default::default()
        };
        let broker = SqsBroker::with_config(config).await.unwrap();
        assert!(broker.is_connected().await);
    }
}
