//! Broker trait definitions

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;
use tokio_stream::Stream;
use turbine_core::{Delivery, Message, TaskId};

/// Result type for broker operations
pub type BrokerResult<T> = Result<T, BrokerError>;

/// Errors that can occur during broker operations
#[derive(Error, Debug)]
pub enum BrokerError {
    /// Connection error
    #[error("connection error: {0}")]
    Connection(String),

    /// Publish error
    #[error("publish error: {0}")]
    Publish(String),

    /// Consume error
    #[error("consume error: {0}")]
    Consume(String),

    /// Acknowledgment error
    #[error("acknowledgment error: {0}")]
    Ack(String),

    /// Queue not found
    #[error("queue not found: {0}")]
    QueueNotFound(String),

    /// Message not found
    #[error("message not found: {0}")]
    MessageNotFound(String),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Timeout error
    #[error("operation timed out")]
    Timeout,

    /// Pool error
    #[error("connection pool error: {0}")]
    Pool(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<turbine_core::Error> for BrokerError {
    fn from(err: turbine_core::Error) -> Self {
        BrokerError::Internal(err.to_string())
    }
}

/// Configuration for a consumer
#[derive(Debug, Clone)]
pub struct ConsumerConfig {
    /// Queues to consume from
    pub queues: Vec<String>,

    /// Prefetch count (how many messages to buffer)
    pub prefetch: usize,

    /// Block timeout when waiting for messages
    pub block_timeout: Duration,

    /// Visibility timeout (how long until message is re-delivered if not acked)
    pub visibility_timeout: Duration,
}

impl Default for ConsumerConfig {
    fn default() -> Self {
        Self {
            queues: vec!["default".to_string()],
            prefetch: 1,
            block_timeout: Duration::from_secs(5),
            visibility_timeout: Duration::from_secs(1800), // 30 minutes
        }
    }
}

impl ConsumerConfig {
    /// Create a new consumer config
    pub fn new(queues: Vec<String>) -> Self {
        Self {
            queues,
            ..Default::default()
        }
    }

    /// Set the prefetch count
    pub fn prefetch(mut self, count: usize) -> Self {
        self.prefetch = count;
        self
    }

    /// Set the block timeout
    pub fn block_timeout(mut self, timeout: Duration) -> Self {
        self.block_timeout = timeout;
        self
    }

    /// Set the visibility timeout
    pub fn visibility_timeout(mut self, timeout: Duration) -> Self {
        self.visibility_timeout = timeout;
        self
    }
}

/// A message consumer that yields deliveries
#[async_trait]
pub trait Consumer: Send + Sync {
    /// Get the next delivery
    async fn next(&mut self) -> Option<BrokerResult<Delivery>>;

    /// Acknowledge a message
    async fn ack(&self, delivery_tag: &str) -> BrokerResult<()>;

    /// Negative acknowledge - requeue the message
    async fn nack(&self, delivery_tag: &str) -> BrokerResult<()>;

    /// Negative acknowledge with delay - requeue after a delay (for exponential backoff)
    async fn nack_with_delay(&self, delivery_tag: &str, delay: Duration) -> BrokerResult<()>;

    /// Reject the message without requeuing
    async fn reject(&self, delivery_tag: &str) -> BrokerResult<()>;

    /// Reject and send to dead letter queue
    async fn reject_to_dlq(&self, delivery_tag: &str, reason: &str) -> BrokerResult<()>;
}

/// Main broker trait for publishing and consuming messages
#[async_trait]
pub trait Broker: Send + Sync + Clone {
    /// The consumer type produced by this broker
    type Consumer: Consumer;

    /// Connect to the broker
    async fn connect(url: &str) -> BrokerResult<Self>
    where
        Self: Sized;

    /// Check if connected
    async fn is_connected(&self) -> bool;

    /// Close the connection
    async fn close(&self) -> BrokerResult<()>;

    /// Declare a queue
    async fn declare_queue(&self, queue: &str) -> BrokerResult<()>;

    /// Delete a queue
    async fn delete_queue(&self, queue: &str) -> BrokerResult<()>;

    /// Get queue length
    async fn queue_length(&self, queue: &str) -> BrokerResult<usize>;

    /// Publish a message to a queue
    async fn publish(&self, queue: &str, message: &Message) -> BrokerResult<()>;

    /// Publish a message with a delay
    async fn publish_delayed(
        &self,
        queue: &str,
        message: &Message,
        delay: Duration,
    ) -> BrokerResult<()>;

    /// Publish multiple messages (batch)
    async fn publish_batch(&self, queue: &str, messages: &[Message]) -> BrokerResult<()> {
        for message in messages {
            self.publish(queue, message).await?;
        }
        Ok(())
    }

    /// Create a consumer
    async fn consume(&self, config: ConsumerConfig) -> BrokerResult<Self::Consumer>;

    /// Get a message by ID (if supported)
    async fn get_message(&self, queue: &str, task_id: &TaskId) -> BrokerResult<Option<Message>>;

    /// Revoke/cancel a message
    async fn revoke(&self, queue: &str, task_id: &TaskId) -> BrokerResult<bool>;

    /// Purge all messages from a queue
    async fn purge(&self, queue: &str) -> BrokerResult<usize>;

    /// Get broker statistics
    async fn stats(&self) -> BrokerResult<BrokerStats>;

    /// Get the dead letter queue name for a queue
    fn dlq_name(&self, queue: &str) -> String {
        format!("{}.dlq", queue)
    }

    /// Publish to dead letter queue
    async fn publish_to_dlq(
        &self,
        queue: &str,
        message: &Message,
        reason: &str,
    ) -> BrokerResult<()>;

    /// Get dead letter queue length
    async fn dlq_length(&self, queue: &str) -> BrokerResult<usize>;

    /// Reprocess a message from DLQ back to main queue
    async fn reprocess_from_dlq(&self, queue: &str, task_id: &TaskId) -> BrokerResult<bool>;

    /// Purge dead letter queue
    async fn purge_dlq(&self, queue: &str) -> BrokerResult<usize>;
}

/// Broker statistics
#[derive(Debug, Clone, Default)]
pub struct BrokerStats {
    /// Number of connected clients
    pub connected_clients: usize,

    /// Total messages published
    pub messages_published: u64,

    /// Total messages consumed
    pub messages_consumed: u64,

    /// Messages currently in queues
    pub messages_pending: u64,

    /// Queue-specific stats
    pub queue_stats: std::collections::HashMap<String, QueueStats>,
}

/// Statistics for a single queue
#[derive(Debug, Clone, Default)]
pub struct QueueStats {
    /// Number of pending messages
    pub pending: usize,

    /// Number of messages being processed
    pub processing: usize,

    /// Number of consumers
    pub consumers: usize,
}
