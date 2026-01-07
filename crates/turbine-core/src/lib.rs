//! Turbine Core - Core types and traits for the Turbine distributed task queue
//!
//! This crate provides the fundamental building blocks for Turbine:
//! - Task definitions and state management
//! - Message serialization and transport
//! - Configuration structures
//! - Error types
//! - Encryption support (with `encryption` feature)
//! - TLS configuration for secure transport

pub mod config;
pub mod encryption;
pub mod error;
pub mod message;
pub mod task;
pub mod tls;
pub mod workflow;

pub use config::TurbineConfig;
pub use config::WorkerConfig;
pub use encryption::EncryptionConfig;
#[cfg(feature = "encryption")]
pub use encryption::{EncryptedData, EncryptionError, EncryptionKey, Encryptor, KeyManager};
pub use error::{Error, Result};
pub use message::{Delivery, Message, MessageHeaders, Serializer};
pub use task::{Task, TaskId, TaskMeta, TaskOptions, TaskResult, TaskState};
pub use tls::{TlsConfig, TlsConfigError, TlsVersion, ClientAuthMode, GrpcTlsConfig, RedisTlsConfig, AmqpTlsConfig};
pub use workflow::{Chain, Chord, Group, Workflow};
