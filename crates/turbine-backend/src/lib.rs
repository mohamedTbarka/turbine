//! Turbine Backend - Result storage abstraction layer
//!
//! This crate provides a unified interface for storing task results:
//! - Redis (default, for small-medium results)
//! - PostgreSQL (coming soon)
//! - S3 (coming soon, for large results)

pub mod redis;
pub mod traits;

pub use redis::RedisBackend;
pub use traits::{Backend, BackendError, BackendResult};
