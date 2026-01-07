//! Redis result backend implementation

use crate::traits::{Backend, BackendError, BackendResult, BackendStats};
use async_trait::async_trait;
use deadpool_redis::{Config, Connection, Pool, Runtime};
use redis::AsyncCommands;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info};
use turbine_core::{Serializer, TaskId, TaskMeta, TaskResult, TaskState};

/// Key prefix for result storage
const KEY_PREFIX: &str = "turbine:result";

/// Key prefix for metadata storage
const META_PREFIX: &str = "turbine:meta";

/// Key prefix for state storage
const STATE_PREFIX: &str = "turbine:state";

/// Redis backend implementation
#[derive(Clone)]
pub struct RedisBackend {
    pool: Pool,
    config: Arc<RedisBackendConfig>,
}

/// Configuration for Redis backend
#[derive(Debug, Clone)]
pub struct RedisBackendConfig {
    /// Connection URL
    pub url: String,

    /// Pool size
    pub pool_size: usize,

    /// Default result TTL in seconds
    pub default_ttl: u64,

    /// Serializer to use
    pub serializer: Serializer,
}

impl Default for RedisBackendConfig {
    fn default() -> Self {
        Self {
            url: "redis://localhost:6379".to_string(),
            pool_size: 10,
            default_ttl: 86400, // 24 hours
            serializer: Serializer::MessagePack,
        }
    }
}

impl RedisBackend {
    /// Create a new Redis backend with custom config
    pub async fn with_config(config: RedisBackendConfig) -> BackendResult<Self> {
        let cfg = Config::from_url(&config.url);
        let pool = cfg
            .builder()
            .map_err(|e| BackendError::Connection(e.to_string()))?
            .max_size(config.pool_size)
            .runtime(Runtime::Tokio1)
            .build()
            .map_err(|e| BackendError::Connection(e.to_string()))?;

        // Test connection
        let mut conn = pool
            .get()
            .await
            .map_err(|e| BackendError::Connection(e.to_string()))?;

        let _: String = redis::cmd("PING")
            .query_async(&mut *conn)
            .await
            .map_err(|e| BackendError::Connection(e.to_string()))?;

        info!("Connected to Redis backend at {}", config.url);

        Ok(Self {
            pool,
            config: Arc::new(config),
        })
    }

    /// Get a connection from the pool
    async fn get_conn(&self) -> BackendResult<Connection> {
        self.pool
            .get()
            .await
            .map_err(|e| BackendError::Pool(e.to_string()))
    }

    /// Get the result key for a task
    fn result_key(&self, task_id: &TaskId) -> String {
        format!("{}:{}", KEY_PREFIX, task_id)
    }

    /// Get the metadata key for a task
    fn meta_key(&self, task_id: &TaskId) -> String {
        format!("{}:{}", META_PREFIX, task_id)
    }

    /// Get the state key for a task
    fn state_key(&self, task_id: &TaskId) -> String {
        format!("{}:{}", STATE_PREFIX, task_id)
    }

    /// Serialize a value
    fn serialize<T: serde::Serialize>(&self, value: &T) -> BackendResult<Vec<u8>> {
        self.config
            .serializer
            .serialize(value)
            .map(|b| b.to_vec())
            .map_err(|e| BackendError::Serialization(e.to_string()))
    }

    /// Deserialize a value
    fn deserialize<T: for<'de> serde::Deserialize<'de>>(&self, data: &[u8]) -> BackendResult<T> {
        self.config
            .serializer
            .deserialize(data)
            .map_err(|e| BackendError::Serialization(e.to_string()))
    }
}

#[async_trait]
impl Backend for RedisBackend {
    async fn connect(url: &str) -> BackendResult<Self> {
        let config = RedisBackendConfig {
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

    async fn close(&self) -> BackendResult<()> {
        // Deadpool handles connection cleanup
        Ok(())
    }

    async fn store_result(&self, result: &TaskResult, ttl: Option<Duration>) -> BackendResult<()> {
        let mut conn = self.get_conn().await?;
        let key = self.result_key(&result.task_id);
        let data = self.serialize(result)?;

        let ttl_secs = ttl
            .map(|d| d.as_secs() as i64)
            .unwrap_or(self.config.default_ttl as i64);

        conn.set_ex(&key, &data[..], ttl_secs as u64)
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;

        debug!("Stored result for task {} (TTL: {}s)", result.task_id, ttl_secs);
        Ok(())
    }

    async fn get_result(&self, task_id: &TaskId) -> BackendResult<Option<TaskResult>> {
        let mut conn = self.get_conn().await?;
        let key = self.result_key(task_id);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| BackendError::Retrieval(e.to_string()))?;

        match data {
            Some(bytes) => {
                let result = self.deserialize(&bytes)?;
                Ok(Some(result))
            }
            None => Ok(None),
        }
    }

    async fn delete_result(&self, task_id: &TaskId) -> BackendResult<bool> {
        let mut conn = self.get_conn().await?;
        let key = self.result_key(task_id);

        let deleted: i32 = conn
            .del(&key)
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;

        Ok(deleted > 0)
    }

    async fn store_meta(&self, task_id: &TaskId, meta: &TaskMeta) -> BackendResult<()> {
        let mut conn = self.get_conn().await?;
        let key = self.meta_key(task_id);
        let data = self.serialize(meta)?;

        // Store metadata with same TTL as results
        conn.set_ex(&key, &data[..], self.config.default_ttl)
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;

        // Also update state key for quick lookups
        self.update_state(task_id, meta.state).await?;

        Ok(())
    }

    async fn get_meta(&self, task_id: &TaskId) -> BackendResult<Option<TaskMeta>> {
        let mut conn = self.get_conn().await?;
        let key = self.meta_key(task_id);

        let data: Option<Vec<u8>> = conn
            .get(&key)
            .await
            .map_err(|e| BackendError::Retrieval(e.to_string()))?;

        match data {
            Some(bytes) => {
                let meta = self.deserialize(&bytes)?;
                Ok(Some(meta))
            }
            None => Ok(None),
        }
    }

    async fn update_state(&self, task_id: &TaskId, state: TaskState) -> BackendResult<()> {
        let mut conn = self.get_conn().await?;
        let key = self.state_key(task_id);

        // Store state as simple string for fast access
        let state_str = state.to_string();

        conn.set_ex(&key, &state_str, self.config.default_ttl)
            .await
            .map_err(|e| BackendError::Storage(e.to_string()))?;

        debug!("Updated state for task {} to {}", task_id, state);
        Ok(())
    }

    async fn get_state(&self, task_id: &TaskId) -> BackendResult<Option<TaskState>> {
        let mut conn = self.get_conn().await?;
        let key = self.state_key(task_id);

        let state_str: Option<String> = conn
            .get(&key)
            .await
            .map_err(|e| BackendError::Retrieval(e.to_string()))?;

        Ok(state_str.and_then(|s| match s.as_str() {
            "pending" => Some(TaskState::Pending),
            "received" => Some(TaskState::Received),
            "running" => Some(TaskState::Running),
            "success" => Some(TaskState::Success),
            "failure" => Some(TaskState::Failure),
            "retry" => Some(TaskState::Retry),
            "revoked" => Some(TaskState::Revoked),
            _ => None,
        }))
    }

    async fn stats(&self) -> BackendResult<BackendStats> {
        let mut conn = self.get_conn().await?;

        // Count keys by pattern (this is expensive, use sparingly)
        let result_count: u64 = {
            let keys: Vec<String> = redis::cmd("KEYS")
                .arg(format!("{}:*", KEY_PREFIX))
                .query_async(&mut *conn)
                .await
                .unwrap_or_default();
            keys.len() as u64
        };

        // Count states
        let mut pending_count = 0u64;
        let mut running_count = 0u64;
        let mut completed_count = 0u64;

        let state_keys: Vec<String> = redis::cmd("KEYS")
            .arg(format!("{}:*", STATE_PREFIX))
            .query_async(&mut *conn)
            .await
            .unwrap_or_default();

        for key in state_keys {
            if let Ok(Some(state_str)) = conn.get::<_, Option<String>>(&key).await {
                match state_str.as_str() {
                    "pending" | "received" | "retry" => pending_count += 1,
                    "running" => running_count += 1,
                    "success" | "failure" | "revoked" => completed_count += 1,
                    _ => {}
                }
            }
        }

        Ok(BackendStats {
            total_results: result_count,
            storage_bytes: 0, // Would need INFO command to get this
            pending_count,
            running_count,
            completed_count,
        })
    }
}

/// Async result handle for waiting on task completion
pub struct AsyncResult<B: Backend> {
    task_id: TaskId,
    backend: B,
}

impl<B: Backend> AsyncResult<B> {
    /// Create a new async result handle
    pub fn new(task_id: TaskId, backend: B) -> Self {
        Self { task_id, backend }
    }

    /// Get the task ID
    pub fn task_id(&self) -> &TaskId {
        &self.task_id
    }

    /// Check if the result is ready
    pub async fn ready(&self) -> BackendResult<bool> {
        let state = self.backend.get_state(&self.task_id).await?;
        Ok(state.map(|s| s.is_terminal()).unwrap_or(false))
    }

    /// Get the current state
    pub async fn state(&self) -> BackendResult<Option<TaskState>> {
        self.backend.get_state(&self.task_id).await
    }

    /// Get the result (returns None if not ready)
    pub async fn get(&self) -> BackendResult<Option<TaskResult>> {
        self.backend.get_result(&self.task_id).await
    }

    /// Wait for the result with timeout
    pub async fn wait(&self, timeout: Duration) -> BackendResult<Option<TaskResult>> {
        self.backend
            .wait_for_result(&self.task_id, timeout, Duration::from_millis(100))
            .await
    }

    /// Wait indefinitely for the result
    pub async fn wait_forever(&self) -> BackendResult<TaskResult> {
        loop {
            if let Some(result) = self.get().await? {
                return Ok(result);
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a running Redis instance
    // Run with: cargo test --package turbine-backend -- --ignored

    #[tokio::test]
    #[ignore]
    async fn test_redis_backend_connect() {
        let backend = RedisBackend::connect("redis://localhost:6379")
            .await
            .unwrap();
        assert!(backend.is_connected().await);
    }

    #[tokio::test]
    #[ignore]
    async fn test_store_and_get_result() {
        let backend = RedisBackend::connect("redis://localhost:6379")
            .await
            .unwrap();

        let task_id = TaskId::new();
        let result = TaskResult::success(task_id.clone(), "test result").unwrap();

        backend.store_result(&result, None).await.unwrap();

        let retrieved = backend.get_result(&task_id).await.unwrap();
        assert!(retrieved.is_some());

        let retrieved = retrieved.unwrap();
        assert_eq!(retrieved.task_id, task_id);
        assert_eq!(retrieved.state, TaskState::Success);

        // Cleanup
        backend.delete_result(&task_id).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_state_updates() {
        let backend = RedisBackend::connect("redis://localhost:6379")
            .await
            .unwrap();

        let task_id = TaskId::new();

        backend
            .update_state(&task_id, TaskState::Running)
            .await
            .unwrap();

        let state = backend.get_state(&task_id).await.unwrap();
        assert_eq!(state, Some(TaskState::Running));

        backend
            .update_state(&task_id, TaskState::Success)
            .await
            .unwrap();

        let state = backend.get_state(&task_id).await.unwrap();
        assert_eq!(state, Some(TaskState::Success));
    }
}
