//! RabbitMQ broker implementation using lapin
//!
//! This implementation uses AMQP 0.9.1 with:
//! - Direct exchange for queue routing
//! - Publisher confirms for reliability
//! - Consumer acknowledgments
//! - Dead letter exchanges for failed messages

use crate::traits::{Broker, BrokerError, BrokerResult, BrokerStats, Consumer, ConsumerConfig, QueueStats};
use async_trait::async_trait;
use deadpool_lapin::{Config, Pool, Runtime};
use futures::StreamExt;
use lapin::{
    options::*,
    types::{AMQPValue, FieldTable},
    BasicProperties, Channel, Connection, ConnectionProperties,
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use turbine_core::{Delivery, Message, Serializer, TaskId};
use uuid::Uuid;

/// Key prefix for Turbine exchanges
const EXCHANGE_PREFIX: &str = "turbine";

/// Default exchange name
const DEFAULT_EXCHANGE: &str = "turbine.direct";

/// Dead letter exchange name
const DLX_EXCHANGE: &str = "turbine.dlx";

/// RabbitMQ broker implementation
#[derive(Clone)]
pub struct RabbitMQBroker {
    pool: Pool,
    config: Arc<RabbitMQConfig>,
    /// Track pending deliveries: delivery_tag -> (queue, message_bytes)
    pending_deliveries: Arc<RwLock<HashMap<String, (String, Vec<u8>)>>>,
}

/// Configuration for RabbitMQ broker
#[derive(Debug, Clone)]
pub struct RabbitMQConfig {
    /// Connection URL (amqp://user:pass@host:port/vhost)
    pub url: String,

    /// Pool size
    pub pool_size: usize,

    /// Default serializer
    pub serializer: Serializer,

    /// Enable publisher confirms
    pub publisher_confirms: bool,

    /// Prefetch count for consumers
    pub prefetch_count: u16,

    /// Message TTL in milliseconds (0 = no TTL)
    pub message_ttl: u64,

    /// Enable dead letter exchange
    pub enable_dlx: bool,
}

impl Default for RabbitMQConfig {
    fn default() -> Self {
        Self {
            url: "amqp://guest:guest@localhost:5672/%2f".to_string(),
            pool_size: 10,
            serializer: Serializer::MessagePack,
            publisher_confirms: true,
            prefetch_count: 10,
            message_ttl: 0,
            enable_dlx: true,
        }
    }
}

impl RabbitMQBroker {
    /// Create a new RabbitMQ broker with custom config
    pub async fn with_config(config: RabbitMQConfig) -> BrokerResult<Self> {
        let cfg = Config {
            url: Some(config.url.clone()),
            ..Default::default()
        };

        let pool = cfg
            .builder(Some(Runtime::Tokio1))
            .max_size(config.pool_size)
            .build()
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        // Test connection
        let conn = pool
            .get()
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        // Declare exchanges
        let channel = conn
            .create_channel()
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        // Main direct exchange
        channel
            .exchange_declare(
                DEFAULT_EXCHANGE,
                lapin::ExchangeKind::Direct,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        // Dead letter exchange
        if config.enable_dlx {
            channel
                .exchange_declare(
                    DLX_EXCHANGE,
                    lapin::ExchangeKind::Direct,
                    ExchangeDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .map_err(|e| BrokerError::Connection(e.to_string()))?;
        }

        info!("Connected to RabbitMQ broker at {}", config.url);

        Ok(Self {
            pool,
            config: Arc::new(config),
            pending_deliveries: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get a channel from the pool
    async fn get_channel(&self) -> BrokerResult<Channel> {
        let conn = self
            .pool
            .get()
            .await
            .map_err(|e| BrokerError::Pool(e.to_string()))?;

        conn.create_channel()
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))
    }

    /// Get the queue name
    fn queue_name(&self, queue: &str) -> String {
        format!("{}.queue.{}", EXCHANGE_PREFIX, queue)
    }

    /// Get the DLQ name
    fn dlq_name(&self, queue: &str) -> String {
        format!("{}.dlq.{}", EXCHANGE_PREFIX, queue)
    }

    /// Get the routing key for a queue
    fn routing_key(&self, queue: &str) -> String {
        queue.to_string()
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

    /// Build queue arguments with DLX
    fn queue_arguments(&self, queue: &str) -> FieldTable {
        let mut args = FieldTable::default();

        if self.config.enable_dlx {
            args.insert(
                "x-dead-letter-exchange".into(),
                AMQPValue::LongString(DLX_EXCHANGE.into()),
            );
            args.insert(
                "x-dead-letter-routing-key".into(),
                AMQPValue::LongString(self.dlq_name(queue).into()),
            );
        }

        if self.config.message_ttl > 0 {
            args.insert(
                "x-message-ttl".into(),
                AMQPValue::LongLongInt(self.config.message_ttl as i64),
            );
        }

        args
    }
}

#[async_trait]
impl Broker for RabbitMQBroker {
    type Consumer = RabbitMQConsumer;

    async fn connect(url: &str) -> BrokerResult<Self> {
        let config = RabbitMQConfig {
            url: url.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    async fn is_connected(&self) -> bool {
        self.pool.get().await.is_ok()
    }

    async fn close(&self) -> BrokerResult<()> {
        // Pool handles connection cleanup
        Ok(())
    }

    async fn declare_queue(&self, queue: &str) -> BrokerResult<()> {
        let channel = self.get_channel().await?;
        let queue_name = self.queue_name(queue);
        let routing_key = self.routing_key(queue);

        // Declare main queue
        channel
            .queue_declare(
                &queue_name,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                self.queue_arguments(queue),
            )
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Bind to exchange
        channel
            .queue_bind(
                &queue_name,
                DEFAULT_EXCHANGE,
                &routing_key,
                QueueBindOptions::default(),
                FieldTable::default(),
            )
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Declare DLQ if enabled
        if self.config.enable_dlx {
            let dlq_name = self.dlq_name(queue);

            channel
                .queue_declare(
                    &dlq_name,
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    FieldTable::default(),
                )
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

            channel
                .queue_bind(
                    &dlq_name,
                    DLX_EXCHANGE,
                    &dlq_name,
                    QueueBindOptions::default(),
                    FieldTable::default(),
                )
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;
        }

        debug!("Declared queue: {}", queue_name);
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> BrokerResult<()> {
        let channel = self.get_channel().await?;
        let queue_name = self.queue_name(queue);

        channel
            .queue_delete(&queue_name, QueueDeleteOptions::default())
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Also delete DLQ
        if self.config.enable_dlx {
            let dlq_name = self.dlq_name(queue);
            let _ = channel
                .queue_delete(&dlq_name, QueueDeleteOptions::default())
                .await;
        }

        Ok(())
    }

    async fn queue_length(&self, queue: &str) -> BrokerResult<usize> {
        let channel = self.get_channel().await?;
        let queue_name = self.queue_name(queue);

        let info = channel
            .queue_declare(
                &queue_name,
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        Ok(info.message_count() as usize)
    }

    async fn publish(&self, queue: &str, message: &Message) -> BrokerResult<()> {
        let channel = self.get_channel().await?;
        let routing_key = self.routing_key(queue);
        let data = self.serialize_message(message)?;

        // Ensure queue exists
        self.declare_queue(queue).await?;

        let properties = BasicProperties::default()
            .with_delivery_mode(2) // Persistent
            .with_content_type("application/octet-stream".into())
            .with_message_id(message.task_id().to_string().into())
            .with_timestamp(chrono::Utc::now().timestamp() as u64);

        if self.config.publisher_confirms {
            channel
                .confirm_select(ConfirmSelectOptions::default())
                .await
                .map_err(|e| BrokerError::Publish(e.to_string()))?;
        }

        channel
            .basic_publish(
                DEFAULT_EXCHANGE,
                &routing_key,
                BasicPublishOptions::default(),
                &data,
                properties,
            )
            .await
            .map_err(|e| BrokerError::Publish(e.to_string()))?
            .await
            .map_err(|e| BrokerError::Publish(e.to_string()))?;

        debug!("Published message {} to queue {}", message.task_id(), queue);
        Ok(())
    }

    async fn publish_delayed(
        &self,
        queue: &str,
        message: &Message,
        delay: Duration,
    ) -> BrokerResult<()> {
        let channel = self.get_channel().await?;
        let routing_key = self.routing_key(queue);
        let data = self.serialize_message(message)?;

        // Create a delayed queue with TTL that routes to the main queue
        let delayed_queue = format!("{}.delayed.{}", self.queue_name(queue), delay.as_millis());

        let mut args = FieldTable::default();
        args.insert(
            "x-dead-letter-exchange".into(),
            AMQPValue::LongString(DEFAULT_EXCHANGE.into()),
        );
        args.insert(
            "x-dead-letter-routing-key".into(),
            AMQPValue::LongString(routing_key.clone().into()),
        );
        args.insert(
            "x-message-ttl".into(),
            AMQPValue::LongLongInt(delay.as_millis() as i64),
        );
        args.insert(
            "x-expires".into(),
            AMQPValue::LongLongInt((delay.as_millis() + 60000) as i64), // Queue expires after delay + 1 minute
        );

        channel
            .queue_declare(
                &delayed_queue,
                QueueDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                args,
            )
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let properties = BasicProperties::default()
            .with_delivery_mode(2)
            .with_message_id(message.task_id().to_string().into());

        channel
            .basic_publish(
                "",
                &delayed_queue,
                BasicPublishOptions::default(),
                &data,
                properties,
            )
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
        RabbitMQConsumer::new(self.clone(), config).await
    }

    async fn get_message(&self, queue: &str, task_id: &TaskId) -> BrokerResult<Option<Message>> {
        // RabbitMQ doesn't support direct message lookup
        // This would require maintaining a separate index
        warn!("get_message not efficiently supported in RabbitMQ");
        Ok(None)
    }

    async fn revoke(&self, _queue: &str, _task_id: &TaskId) -> BrokerResult<bool> {
        // Revocation in RabbitMQ is complex - would need to consume and discard
        warn!("revoke not efficiently supported in RabbitMQ, use result backend for revocation");
        Ok(false)
    }

    async fn purge(&self, queue: &str) -> BrokerResult<usize> {
        let channel = self.get_channel().await?;
        let queue_name = self.queue_name(queue);

        let purged = channel
            .queue_purge(&queue_name, QueuePurgeOptions::default())
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        Ok(purged as usize)
    }

    async fn stats(&self) -> BrokerResult<BrokerStats> {
        // RabbitMQ stats would require management API
        Ok(BrokerStats::default())
    }

    async fn publish_to_dlq(
        &self,
        queue: &str,
        message: &Message,
        reason: &str,
    ) -> BrokerResult<()> {
        let channel = self.get_channel().await?;
        let dlq_name = self.dlq_name(queue);
        let data = self.serialize_message(message)?;

        let mut headers = FieldTable::default();
        headers.insert(
            "x-death-reason".into(),
            AMQPValue::LongString(reason.into()),
        );
        headers.insert(
            "x-original-queue".into(),
            AMQPValue::LongString(queue.into()),
        );

        let properties = BasicProperties::default()
            .with_delivery_mode(2)
            .with_headers(headers)
            .with_message_id(message.task_id().to_string().into());

        channel
            .basic_publish(
                DLX_EXCHANGE,
                &dlq_name,
                BasicPublishOptions::default(),
                &data,
                properties,
            )
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
        let channel = self.get_channel().await?;
        let dlq_name = self.dlq_name(queue);

        let info = channel
            .queue_declare(
                &dlq_name,
                QueueDeclareOptions {
                    passive: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        Ok(info.message_count() as usize)
    }

    async fn reprocess_from_dlq(&self, queue: &str, _task_id: &TaskId) -> BrokerResult<bool> {
        // Would need to consume from DLQ and republish
        warn!("reprocess_from_dlq requires consuming from DLQ");
        Ok(false)
    }

    async fn purge_dlq(&self, queue: &str) -> BrokerResult<usize> {
        let channel = self.get_channel().await?;
        let dlq_name = self.dlq_name(queue);

        let purged = channel
            .queue_purge(&dlq_name, QueuePurgeOptions::default())
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        Ok(purged as usize)
    }
}

/// RabbitMQ consumer implementation
pub struct RabbitMQConsumer {
    broker: RabbitMQBroker,
    config: ConsumerConfig,
    channel: Channel,
    consumer_tag: String,
    /// Pending deliveries for ack/nack
    pending: Arc<RwLock<HashMap<String, (u64, String, Vec<u8>)>>>, // tag -> (delivery_tag, queue, data)
}

impl RabbitMQConsumer {
    async fn new(broker: RabbitMQBroker, config: ConsumerConfig) -> BrokerResult<Self> {
        let channel = broker.get_channel().await?;

        // Set prefetch
        channel
            .basic_qos(
                config.prefetch as u16,
                BasicQosOptions::default(),
            )
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Ensure all queues exist
        for queue in &config.queues {
            broker.declare_queue(queue).await?;
        }

        let consumer_tag = format!("turbine-{}", Uuid::new_v4());

        Ok(Self {
            broker,
            config,
            channel,
            consumer_tag,
            pending: Arc::new(RwLock::new(HashMap::new())),
        })
    }
}

#[async_trait]
impl Consumer for RabbitMQConsumer {
    async fn next(&mut self) -> Option<BrokerResult<Delivery>> {
        // Round-robin through queues
        for queue in &self.config.queues {
            let queue_name = self.broker.queue_name(queue);

            match self
                .channel
                .basic_get(&queue_name, BasicGetOptions { no_ack: false })
                .await
            {
                Ok(Some(delivery)) => {
                    let data = delivery.data.clone();

                    match self.broker.deserialize_message(&data) {
                        Ok(message) => {
                            let tag = Uuid::new_v4().to_string();

                            // Store for ack/nack
                            {
                                let mut pending = self.pending.write().await;
                                pending.insert(
                                    tag.clone(),
                                    (delivery.delivery_tag, queue.clone(), data),
                                );
                            }

                            return Some(Ok(Delivery::new(message, tag, queue.clone())));
                        }
                        Err(e) => {
                            // Reject malformed message
                            let _ = self
                                .channel
                                .basic_reject(
                                    delivery.delivery_tag,
                                    BasicRejectOptions { requeue: false },
                                )
                                .await;
                            return Some(Err(e));
                        }
                    }
                }
                Ok(None) => continue, // No message in this queue
                Err(e) => return Some(Err(BrokerError::Consume(e.to_string()))),
            }
        }

        // No messages available, wait a bit
        tokio::time::sleep(self.config.block_timeout).await;
        None
    }

    async fn ack(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (amqp_tag, _queue, _data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        self.channel
            .basic_ack(amqp_tag, BasicAckOptions::default())
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Acknowledged message {}", delivery_tag);
        Ok(())
    }

    async fn nack(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (amqp_tag, _queue, _data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        self.channel
            .basic_nack(
                amqp_tag,
                BasicNackOptions {
                    requeue: true,
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Nacked message {}, requeued", delivery_tag);
        Ok(())
    }

    async fn nack_with_delay(&self, delivery_tag: &str, delay: Duration) -> BrokerResult<()> {
        let (amqp_tag, queue, data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        // Ack the original
        self.channel
            .basic_ack(amqp_tag, BasicAckOptions::default())
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        // Republish with delay
        if let Ok(mut message) = self.broker.deserialize_message(&data) {
            message.headers.retries += 1;
            self.broker.publish_delayed(&queue, &message, delay).await?;
        }

        debug!("Nacked message {}, scheduled retry in {:?}", delivery_tag, delay);
        Ok(())
    }

    async fn reject(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (amqp_tag, _queue, _data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        self.channel
            .basic_reject(amqp_tag, BasicRejectOptions { requeue: false })
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Rejected message {}", delivery_tag);
        Ok(())
    }

    async fn reject_to_dlq(&self, delivery_tag: &str, reason: &str) -> BrokerResult<()> {
        let (amqp_tag, queue, data) = {
            let mut pending = self.pending.write().await;
            pending
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        // Ack original
        self.channel
            .basic_ack(amqp_tag, BasicAckOptions::default())
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        // Publish to DLQ
        if let Ok(message) = self.broker.deserialize_message(&data) {
            self.broker.publish_to_dlq(&queue, &message, reason).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running RabbitMQ instance
    // Run with: cargo test --package turbine-broker --features rabbitmq -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_rabbitmq_broker_connect() {
        let broker = RabbitMQBroker::connect("amqp://guest:guest@localhost:5672/%2f")
            .await
            .unwrap();
        assert!(broker.is_connected().await);
    }
}
