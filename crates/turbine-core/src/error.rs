//! Error types for Turbine

use thiserror::Error;

/// Result type alias using Turbine's Error type
pub type Result<T> = std::result::Result<T, Error>;

/// Main error type for Turbine operations
#[derive(Error, Debug)]
pub enum Error {
    /// Task not found
    #[error("task not found: {0}")]
    TaskNotFound(String),

    /// Task already exists
    #[error("task already exists: {0}")]
    TaskAlreadyExists(String),

    /// Task execution failed
    #[error("task execution failed: {0}")]
    TaskExecutionFailed(String),

    /// Task timed out
    #[error("task timed out after {0} seconds")]
    TaskTimeout(u64),

    /// Task was cancelled
    #[error("task was cancelled: {0}")]
    TaskCancelled(String),

    /// Maximum retries exceeded
    #[error("maximum retries exceeded for task {task_id}: {message}")]
    MaxRetriesExceeded { task_id: String, message: String },

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Deserialization error
    #[error("deserialization error: {0}")]
    Deserialization(String),

    /// Broker connection error
    #[error("broker connection error: {0}")]
    BrokerConnection(String),

    /// Broker publish error
    #[error("broker publish error: {0}")]
    BrokerPublish(String),

    /// Broker consume error
    #[error("broker consume error: {0}")]
    BrokerConsume(String),

    /// Backend storage error
    #[error("backend storage error: {0}")]
    BackendStorage(String),

    /// Backend retrieval error
    #[error("backend retrieval error: {0}")]
    BackendRetrieval(String),

    /// Configuration error
    #[error("configuration error: {0}")]
    Configuration(String),

    /// Invalid queue name
    #[error("invalid queue name: {0}")]
    InvalidQueue(String),

    /// Workflow error
    #[error("workflow error: {0}")]
    Workflow(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Check if this error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Error::BrokerConnection(_)
                | Error::BrokerPublish(_)
                | Error::BrokerConsume(_)
                | Error::BackendStorage(_)
                | Error::BackendRetrieval(_)
                | Error::TaskTimeout(_)
        )
    }

    /// Check if this is a task-related error
    pub fn is_task_error(&self) -> bool {
        matches!(
            self,
            Error::TaskNotFound(_)
                | Error::TaskAlreadyExists(_)
                | Error::TaskExecutionFailed(_)
                | Error::TaskTimeout(_)
                | Error::TaskCancelled(_)
                | Error::MaxRetriesExceeded { .. }
        )
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<rmp_serde::encode::Error> for Error {
    fn from(err: rmp_serde::encode::Error) -> Self {
        Error::Serialization(err.to_string())
    }
}

impl From<rmp_serde::decode::Error> for Error {
    fn from(err: rmp_serde::decode::Error) -> Self {
        Error::Deserialization(err.to_string())
    }
}
