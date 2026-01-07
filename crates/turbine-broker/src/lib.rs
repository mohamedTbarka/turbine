//! Turbine Broker - Message broker abstraction layer
//!
//! This crate provides a unified interface for different message brokers:
//! - Redis (using reliable queue pattern)
//! - RabbitMQ (via lapin)
//! - AWS SQS
//!
//! Enable brokers via feature flags:
//! - `redis` (default) - Redis broker
//! - `rabbitmq` - RabbitMQ broker
//! - `sqs` - AWS SQS broker

pub mod traits;

#[cfg(feature = "redis")]
pub mod redis;

#[cfg(feature = "rabbitmq")]
pub mod rabbitmq;

#[cfg(feature = "sqs")]
pub mod sqs;

// Re-exports
pub use traits::{Broker, BrokerError, BrokerResult, BrokerStats, Consumer, ConsumerConfig, QueueStats};

#[cfg(feature = "redis")]
pub use redis::RedisBroker;

#[cfg(feature = "rabbitmq")]
pub use rabbitmq::RabbitMQBroker;

#[cfg(feature = "sqs")]
pub use sqs::SqsBroker;
