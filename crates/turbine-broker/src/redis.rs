//! Redis broker implementation using reliable queue pattern
//!
//! This implementation uses the BRPOPLPUSH pattern for reliable message delivery:
//! - Messages are stored in a list (queue)
//! - BRPOPLPUSH atomically moves messages to a processing list
//! - Messages are removed from processing list on ack
//! - A reaper process moves stale messages back to the main queue

use crate::traits::{Broker, BrokerError, BrokerResult, BrokerStats, Consumer, ConsumerConfig, QueueStats};
use async_trait::async_trait;
use deadpool_redis::{Config, Pool, Runtime};
use redis::AsyncCommands;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use turbine_core::{Delivery, Message, Serializer, TaskId};
use uuid::Uuid;

type Connection = deadpool_redis::Connection;

/// Key prefix for all Turbine data in Redis
const KEY_PREFIX: &str = "turbine";

/// Redis broker implementation
#[derive(Clone)]
pub struct RedisBroker {
    pool: Pool,
    config: Arc<RedisBrokerConfig>,
    /// Track processing messages: delivery_tag -> (queue, message_bytes)
    processing: Arc<RwLock<HashMap<String, (String, Vec<u8>)>>>,
}

/// Configuration for Redis broker
#[derive(Debug, Clone)]
pub struct RedisBrokerConfig {
    /// Connection URL
    pub url: String,

    /// Pool size
    pub pool_size: usize,

    /// Default serializer
    pub serializer: Serializer,

    /// Visibility timeout in seconds
    pub visibility_timeout: u64,
}

impl Default for RedisBrokerConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            serializer: Serializer::MessagePack,
            visibility_timeout: 1800, // 30 minutes
        }
    }
}

impl RedisBroker {
    /// Create a new Redis broker with custom config
    pub async fn with_config(config: RedisBrokerConfig) -> BrokerResult<Self> {
        let cfg = Config::from_url(&config.url);
        let pool = cfg
            .builder()
            .map_err(|e| BrokerError::Connection(e.to_string()))?
            .max_size(config.pool_size)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        // Test connection
        let mut conn = pool
            .get()
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        let _: String = redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| BrokerError::Connection(e.to_string()))?;

        info!("Connected to Redis broker at {}", config.url);

        Ok(Self {
            pool,
            config: Arc::new(config),
            processing: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Get a connection from the pool
    async fn get_conn(&self) -> BrokerResult<Connection> {
        self.pool
            .get()
            .await
            .map_err(|e| BrokerError::Pool(e.to_string()))
    }

    /// Get the queue key
    fn queue_key(&self, queue: &str) -> String {
        format!("{}:queue:{}", KEY_PREFIX, queue)
    }

    /// Get the processing key for a queue
    fn processing_key(&self, queue: &str) -> String {
        format!("{}:processing:{}", KEY_PREFIX, queue)
    }

    /// Get the delayed queue key
    fn delayed_key(&self) -> String {
        format!("{}:delayed", KEY_PREFIX)
    }

    /// Get the message key for a specific task
    fn message_key(&self, queue: &str, task_id: &TaskId) -> String {
        format!("{}:msg:{}:{}", KEY_PREFIX, queue, task_id)
    }

    /// Get the revoked set key
    fn revoked_key(&self) -> String {
        format!("{}:revoked", KEY_PREFIX)
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

    /// Check if a task has been revoked
    async fn is_revoked(&self, task_id: &TaskId) -> BrokerResult<bool> {
        let mut conn = self.get_conn().await?;
        let result: bool = conn
            .sismember(self.revoked_key(), task_id.to_string())
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;
        Ok(result)
    }

    /// Start the delayed message processor
    pub fn start_delayed_processor(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));
            loop {
                interval.tick().await;
                if let Err(e) = self.process_delayed_messages().await {
                    error!("Error processing delayed messages: {}", e);
                }
            }
        })
    }

    /// Process delayed messages that are due
    async fn process_delayed_messages(&self) -> BrokerResult<()> {
        let mut conn = self.get_conn().await?;
        let now = chrono::Utc::now().timestamp() as f64;

        // Get messages with score <= now
        let messages: Vec<(Vec<u8>, f64)> = redis::cmd("ZRANGEBYSCORE")
            .arg(self.delayed_key())
            .arg("-inf")
            .arg(now)
            .arg("WITHSCORES")
            .arg("LIMIT")
            .arg(0)
            .arg(100)
            .query_async(&mut *conn)
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        for (data, _score) in messages {
            // Remove from delayed set
            let removed: i32 = conn
                .zrem(self.delayed_key(), &data)
                .await
                .map_err(|e| BrokerError::Internal(e.to_string()))?;

            if removed > 0 {
                // Deserialize and publish to target queue
                if let Ok(message) = self.deserialize_message(&data) {
                    let queue = message.queue().to_string();
                    debug!("Moving delayed message {} to queue {}", message.task_id(), queue);

                    conn.lpush(self.queue_key(&queue), &data)
                        .await
                        .map_err(|e| BrokerError::Publish(e.to_string()))?;
                }
            }
        }

        Ok(())
    }

    /// Start the visibility timeout reaper
    pub fn start_reaper(self: Arc<Self>, queues: Vec<String>) -> tokio::task::JoinHandle<()> {
        let visibility_timeout = self.config.visibility_timeout;

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(60));
            loop {
                interval.tick().await;
                for queue in &queues {
                    if let Err(e) = self.reap_stale_messages(queue, visibility_timeout).await {
                        error!("Error reaping stale messages from {}: {}", queue, e);
                    }
                }
            }
        })
    }

    /// Move stale messages from processing back to queue
    async fn reap_stale_messages(&self, queue: &str, timeout_secs: u64) -> BrokerResult<()> {
        let mut conn = self.get_conn().await?;
        let processing_key = self.processing_key(queue);
        let queue_key = self.queue_key(queue);

        // Get all messages in processing
        let messages: Vec<Vec<u8>> = conn
            .lrange(&processing_key, 0, -1)
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let now = chrono::Utc::now();
        let timeout = chrono::Duration::seconds(timeout_secs as i64);

        for data in messages {
            if let Ok(message) = self.deserialize_message(&data) {
                // Check if message has exceeded visibility timeout
                let elapsed = now - message.headers.timestamp;
                if elapsed > timeout {
                    warn!(
                        "Message {} exceeded visibility timeout, requeueing",
                        message.task_id()
                    );

                    // Remove from processing and add back to queue
                    let _: i32 = conn
                        .lrem(&processing_key, 1, &data)
                        .await
                        .map_err(|e| BrokerError::Internal(e.to_string()))?;

                    conn.rpush(&queue_key, &data)
                        .await
                        .map_err(|e| BrokerError::Internal(e.to_string()))?;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Broker for RedisBroker {
    type Consumer = RedisConsumer;

    async fn connect(url: &str) -> BrokerResult<Self> {
        let config = RedisBrokerConfig {
            url: url.to_string(),
            ..Default::default()
        };
        Self::with_config(config).await
    }

    async fn is_connected(&self) -> bool {
        if let Ok(mut conn) = self.get_conn().await {
            redis::cmd("PING")
                .query_async::<_, String>(&mut *conn)
                .await
                .is_ok()
        } else {
            false
        }
    }

    async fn close(&self) -> BrokerResult<()> {
        // Deadpool handles connection cleanup
        Ok(())
    }

    async fn declare_queue(&self, queue: &str) -> BrokerResult<()> {
        // Redis lists are created automatically
        // Just verify we can access it
        let mut conn = self.get_conn().await?;
        let _: i64 = conn
            .llen(self.queue_key(queue))
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn delete_queue(&self, queue: &str) -> BrokerResult<()> {
        let mut conn = self.get_conn().await?;
        conn.del(self.queue_key(queue))
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;
        conn.del(self.processing_key(queue))
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn queue_length(&self, queue: &str) -> BrokerResult<usize> {
        let mut conn = self.get_conn().await?;
        let len: i64 = conn
            .llen(self.queue_key(queue))
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;
        Ok(len as usize)
    }

    async fn publish(&self, queue: &str, message: &Message) -> BrokerResult<()> {
        // Check if task is revoked
        if self.is_revoked(&message.task_id()).await? {
            debug!("Skipping revoked task {}", message.task_id());
            return Ok(());
        }

        let mut conn = self.get_conn().await?;
        let data = self.serialize_message(message)?;

        // Check if message should be delayed
        if let Some(duration) = message.time_until_eta() {
            if duration.num_seconds() > 0 {
                return self
                    .publish_delayed(queue, message, duration.to_std().unwrap_or_default())
                    .await;
            }
        }

        // LPUSH adds to the left, BRPOPLPUSH will pop from the right (FIFO)
        conn.lpush(self.queue_key(queue), data)
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
        let mut conn = self.get_conn().await?;

        // Update message with new ETA
        let mut message = message.clone();
        message.headers.eta = Some(chrono::Utc::now() + chrono::Duration::from_std(delay).unwrap());
        message.headers.queue = queue.to_string();

        let data = self.serialize_message(&message)?;
        let execute_at = (chrono::Utc::now() + chrono::Duration::from_std(delay).unwrap())
            .timestamp() as f64;

        // Add to sorted set with execute_at as score
        conn.zadd(self.delayed_key(), &data[..], execute_at)
            .await
            .map_err(|e| BrokerError::Publish(e.to_string()))?;

        debug!(
            "Scheduled message {} for execution in {:?}",
            message.task_id(),
            delay
        );
        Ok(())
    }

    async fn consume(&self, config: ConsumerConfig) -> BrokerResult<Self::Consumer> {
        Ok(RedisConsumer::new(self.clone(), config))
    }

    async fn get_message(&self, queue: &str, task_id: &TaskId) -> BrokerResult<Option<Message>> {
        let mut conn = self.get_conn().await?;

        // Search in queue
        let messages: Vec<Vec<u8>> = conn
            .lrange(self.queue_key(queue), 0, -1)
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        for data in messages {
            if let Ok(msg) = self.deserialize_message(&data) {
                if &msg.task_id() == task_id {
                    return Ok(Some(msg));
                }
            }
        }

        // Search in processing
        let processing: Vec<Vec<u8>> = conn
            .lrange(self.processing_key(queue), 0, -1)
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        for data in processing {
            if let Ok(msg) = self.deserialize_message(&data) {
                if &msg.task_id() == task_id {
                    return Ok(Some(msg));
                }
            }
        }

        Ok(None)
    }

    async fn revoke(&self, _queue: &str, task_id: &TaskId) -> BrokerResult<bool> {
        let mut conn = self.get_conn().await?;

        // Add to revoked set
        let added: i32 = conn
            .sadd(self.revoked_key(), task_id.to_string())
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        // Set expiration on revoked entry (24 hours)
        conn.expire(self.revoked_key(), 86400)
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        debug!("Revoked task {}", task_id);
        Ok(added > 0)
    }

    async fn purge(&self, queue: &str) -> BrokerResult<usize> {
        let len = self.queue_length(queue).await?;
        self.delete_queue(queue).await?;
        Ok(len)
    }

    async fn stats(&self) -> BrokerResult<BrokerStats> {
        let mut conn = self.get_conn().await?;

        // Get client count
        let info: String = redis::cmd("INFO")
            .arg("clients")
            .query_async(&mut *conn)
            .await
            .map_err(|e| BrokerError::Internal(e.to_string()))?;

        let connected_clients = info
            .lines()
            .find(|l| l.starts_with("connected_clients:"))
            .and_then(|l| l.split(':').nth(1))
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);

        Ok(BrokerStats {
            connected_clients,
            ..Default::default()
        })
    }
}

/// Redis consumer implementation
pub struct RedisConsumer {
    broker: RedisBroker,
    config: ConsumerConfig,
    current_queue_index: usize,
}

impl RedisConsumer {
    fn new(broker: RedisBroker, config: ConsumerConfig) -> Self {
        Self {
            broker,
            config,
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
impl Consumer for RedisConsumer {
    async fn next(&mut self) -> Option<BrokerResult<Delivery>> {
        let queue = self.next_queue().to_string();
        let queue_key = self.broker.queue_key(&queue);
        let processing_key = self.broker.processing_key(&queue);

        let mut conn = match self.broker.get_conn().await {
            Ok(c) => c,
            Err(e) => return Some(Err(e)),
        };

        // BRPOPLPUSH atomically moves message from queue to processing
        let timeout_secs = self.config.block_timeout.as_secs() as i64;

        let result: Option<Vec<u8>> = match redis::cmd("BRPOPLPUSH")
            .arg(&queue_key)
            .arg(&processing_key)
            .arg(timeout_secs)
            .query_async(&mut *conn)
            .await
        {
            Ok(data) => data,
            Err(e) => {
                // Timeout is not an error
                if e.kind() == redis::ErrorKind::TypeError {
                    return None;
                }
                return Some(Err(BrokerError::Consume(e.to_string())));
            }
        };

        let data = match result {
            Some(d) => d,
            None => return None, // Timeout
        };

        // Deserialize message
        let message = match self.broker.deserialize_message(&data) {
            Ok(m) => m,
            Err(e) => return Some(Err(e)),
        };

        // Check if revoked
        match self.broker.is_revoked(&message.task_id()).await {
            Ok(true) => {
                // Remove from processing and skip
                let _: Result<i32, _> = conn.lrem(&processing_key, 1, &data).await;
                debug!("Skipping revoked task {}", message.task_id());
                return self.next().await; // Get next message
            }
            Ok(false) => {}
            Err(e) => return Some(Err(e)),
        }

        // Check if expired
        if message.is_expired() {
            let _: Result<i32, _> = conn.lrem(&processing_key, 1, &data).await;
            debug!("Skipping expired message {}", message.task_id());
            return self.next().await;
        }

        // Generate delivery tag
        let delivery_tag = Uuid::new_v4().to_string();

        // Store mapping for ack/nack
        {
            let mut processing = self.broker.processing.write().await;
            processing.insert(delivery_tag.clone(), (queue.clone(), data));
        }

        let delivery = Delivery::new(message, delivery_tag, queue);

        Some(Ok(delivery))
    }

    async fn ack(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (queue, data) = {
            let mut processing = self.broker.processing.write().await;
            processing
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        let mut conn = self.broker.get_conn().await?;
        let processing_key = self.broker.processing_key(&queue);

        // Remove from processing list
        let _: i32 = conn
            .lrem(&processing_key, 1, &data)
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Acknowledged message with delivery tag {}", delivery_tag);
        Ok(())
    }

    async fn nack(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (queue, data) = {
            let mut processing = self.broker.processing.write().await;
            processing
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        let mut conn = self.broker.get_conn().await?;
        let processing_key = self.broker.processing_key(&queue);
        let queue_key = self.broker.queue_key(&queue);

        // Remove from processing
        let _: i32 = conn
            .lrem(&processing_key, 1, &data)
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        // Add back to queue (at the end for retry)
        conn.rpush(&queue_key, &data)
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Nacked message with delivery tag {}, requeued", delivery_tag);
        Ok(())
    }

    async fn reject(&self, delivery_tag: &str) -> BrokerResult<()> {
        let (queue, data) = {
            let mut processing = self.broker.processing.write().await;
            processing
                .remove(delivery_tag)
                .ok_or_else(|| BrokerError::MessageNotFound(delivery_tag.to_string()))?
        };

        let mut conn = self.broker.get_conn().await?;
        let processing_key = self.broker.processing_key(&queue);

        // Just remove from processing, don't requeue
        let _: i32 = conn
            .lrem(&processing_key, 1, &data)
            .await
            .map_err(|e| BrokerError::Ack(e.to_string()))?;

        debug!("Rejected message with delivery tag {}", delivery_tag);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbine_core::Task;

    // Note: These tests require a running Redis instance
    // Run with: cargo test --package turbine-broker -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_redis_broker_connect() {
        let broker = RedisBroker::connect("redis://localhost:6379").await.unwrap();
        assert!(broker.is_connected().await);
    }

    #[tokio::test]
    #[ignore]
    async fn test_redis_publish_consume() {
        let broker = RedisBroker::connect("redis://localhost:6379").await.unwrap();

        // Create and publish a task
        let task = Task::new("test_task").arg(42).unwrap();
        let message = Message::from_task(task, Serializer::MessagePack).unwrap();

        broker.publish("test_queue", &message).await.unwrap();

        // Consume the message
        let config = ConsumerConfig::new(vec!["test_queue".to_string()]);
        let mut consumer = broker.consume(config).await.unwrap();

        let delivery = consumer.next().await.unwrap().unwrap();
        assert_eq!(delivery.message.headers.task_name, "test_task");

        // Acknowledge
        consumer.ack(&delivery.delivery_tag).await.unwrap();

        // Cleanup
        broker.delete_queue("test_queue").await.unwrap();
    }
}
