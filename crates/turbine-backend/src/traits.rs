//! Backend trait definitions

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;
use turbine_core::{TaskId, TaskMeta, TaskResult, TaskState};

/// Result type for backend operations
pub type BackendResult<T> = Result<T, BackendError>;

/// Errors that can occur during backend operations
#[derive(Error, Debug)]
pub enum BackendError {
    /// Connection error
    #[error("connection error: {0}")]
    Connection(String),

    /// Storage error
    #[error("storage error: {0}")]
    Storage(String),

    /// Retrieval error
    #[error("retrieval error: {0}")]
    Retrieval(String),

    /// Result not found
    #[error("result not found: {0}")]
    NotFound(String),

    /// Serialization error
    #[error("serialization error: {0}")]
    Serialization(String),

    /// Pool error
    #[error("connection pool error: {0}")]
    Pool(String),

    /// Internal error
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<turbine_core::Error> for BackendError {
    fn from(err: turbine_core::Error) -> Self {
        BackendError::Internal(err.to_string())
    }
}

/// Main backend trait for storing and retrieving task results
#[async_trait]
pub trait Backend: Send + Sync + Clone {
    /// Connect to the backend
    async fn connect(url: &str) -> BackendResult<Self>
    where
        Self: Sized;

    /// Check if connected
    async fn is_connected(&self) -> bool;

    /// Close the connection
    async fn close(&self) -> BackendResult<()>;

    /// Store a task result
    async fn store_result(&self, result: &TaskResult, ttl: Option<Duration>) -> BackendResult<()>;

    /// Get a task result
    async fn get_result(&self, task_id: &TaskId) -> BackendResult<Option<TaskResult>>;

    /// Delete a task result
    async fn delete_result(&self, task_id: &TaskId) -> BackendResult<bool>;

    /// Store task metadata (state, progress, etc.)
    async fn store_meta(&self, task_id: &TaskId, meta: &TaskMeta) -> BackendResult<()>;

    /// Get task metadata
    async fn get_meta(&self, task_id: &TaskId) -> BackendResult<Option<TaskMeta>>;

    /// Update task state
    async fn update_state(&self, task_id: &TaskId, state: TaskState) -> BackendResult<()>;

    /// Get task state
    async fn get_state(&self, task_id: &TaskId) -> BackendResult<Option<TaskState>>;

    /// Check if a result exists
    async fn exists(&self, task_id: &TaskId) -> BackendResult<bool> {
        Ok(self.get_result(task_id).await?.is_some())
    }

    /// Wait for a result with timeout
    async fn wait_for_result(
        &self,
        task_id: &TaskId,
        timeout: Duration,
        poll_interval: Duration,
    ) -> BackendResult<Option<TaskResult>> {
        let start = std::time::Instant::now();

        loop {
            if let Some(result) = self.get_result(task_id).await? {
                return Ok(Some(result));
            }

            if start.elapsed() >= timeout {
                return Ok(None);
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Get multiple results
    async fn get_results(&self, task_ids: &[TaskId]) -> BackendResult<Vec<Option<TaskResult>>> {
        let mut results = Vec::with_capacity(task_ids.len());
        for task_id in task_ids {
            results.push(self.get_result(task_id).await?);
        }
        Ok(results)
    }

    /// Store multiple results
    async fn store_results(
        &self,
        results: &[TaskResult],
        ttl: Option<Duration>,
    ) -> BackendResult<()> {
        for result in results {
            self.store_result(result, ttl).await?;
        }
        Ok(())
    }

    /// Clean up expired results (if supported)
    async fn cleanup_expired(&self) -> BackendResult<usize> {
        // Default implementation does nothing
        Ok(0)
    }

    /// Get backend statistics
    async fn stats(&self) -> BackendResult<BackendStats>;

    /// Store raw bytes (for workflow state, etc.)
    async fn store_raw(&self, key: &str, data: &[u8], ttl: Option<Duration>) -> BackendResult<()>;

    /// Get raw bytes
    async fn get_raw(&self, key: &str) -> BackendResult<Option<Vec<u8>>>;

    /// Delete raw data
    async fn delete_raw(&self, key: &str) -> BackendResult<bool>;
}

/// Backend statistics
#[derive(Debug, Clone, Default)]
pub struct BackendStats {
    /// Number of stored results
    pub total_results: u64,

    /// Total storage size in bytes
    pub storage_bytes: u64,

    /// Number of pending tasks
    pub pending_count: u64,

    /// Number of running tasks
    pub running_count: u64,

    /// Number of completed tasks (success + failure)
    pub completed_count: u64,
}
