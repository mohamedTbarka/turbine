//! Turbine Broker - Message broker abstraction layer
//!
//! This crate provides a unified interface for different message brokers:
//! - Redis (using reliable queue pattern)
//! - RabbitMQ (coming soon)
//! - AWS SQS (coming soon)

pub mod redis;
pub mod traits;

pub use redis::RedisBroker;
pub use traits::{Broker, BrokerError, BrokerResult, Consumer, ConsumerConfig};
