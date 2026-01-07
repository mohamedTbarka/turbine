//! Dashboard shared state

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use turbine_broker::{RedisBroker, redis::RedisBrokerConfig};
use turbine_backend::{RedisBackend, redis::RedisBackendConfig};

/// Dashboard configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DashboardConfig {
    /// HTTP server host
    pub host: String,

    /// HTTP server port
    pub port: u16,

    /// Redis URL for broker
    pub redis_url: String,

    /// Enable CORS
    pub enable_cors: bool,

    /// Allowed origins for CORS
    pub cors_origins: Vec<String>,

    /// SSE heartbeat interval in seconds
    pub sse_heartbeat_secs: u64,

    /// Maximum SSE connections
    pub max_sse_connections: usize,
}

impl Default for DashboardConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 8080,
            redis_url: "redis://localhost:6379".to_string(),
            enable_cors: true,
            cors_origins: vec!["*".to_string()],
            sse_heartbeat_secs: 30,
            max_sse_connections: 1000,
        }
    }
}

/// Worker information tracked by dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub id: String,
    pub hostname: String,
    pub queues: Vec<String>,
    pub concurrency: usize,
    pub status: WorkerStatus,
    pub tasks_processed: u64,
    pub tasks_failed: u64,
    pub current_tasks: Vec<String>,
    pub last_heartbeat: chrono::DateTime<chrono::Utc>,
    pub started_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkerStatus {
    Online,
    Busy,
    Idle,
    Offline,
    Draining,
}

/// Queue statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct QueueInfo {
    pub name: String,
    pub pending: usize,
    pub processing: usize,
    pub dlq_size: usize,
    pub workers: usize,
    pub throughput_per_min: f64,
    pub avg_processing_time_ms: f64,
}

/// Task summary for dashboard display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSummary {
    pub id: String,
    pub name: String,
    pub queue: String,
    pub state: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub retries: u32,
    pub worker_id: Option<String>,
    pub error: Option<String>,
}

/// Dashboard shared state
pub struct DashboardState {
    pub config: DashboardConfig,
    pub broker: Arc<RedisBroker>,
    pub backend: Arc<RedisBackend>,

    /// Cached worker information
    pub workers: Arc<RwLock<HashMap<String, WorkerInfo>>>,

    /// Cached queue statistics
    pub queues: Arc<RwLock<HashMap<String, QueueInfo>>>,

    /// Recent tasks cache
    pub recent_tasks: Arc<RwLock<Vec<TaskSummary>>>,

    /// SSE subscriber count
    pub sse_connections: Arc<RwLock<usize>>,
}

impl DashboardState {
    /// Create new dashboard state
    pub async fn new(config: DashboardConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let broker_config = RedisBrokerConfig {
            url: config.redis_url.clone(),
            ..Default::default()
        };
        let broker = RedisBroker::with_config(broker_config).await?;

        let backend_config = RedisBackendConfig {
            url: config.redis_url.clone(),
            ..Default::default()
        };
        let backend = RedisBackend::with_config(backend_config).await?;

        Ok(Self {
            config,
            broker: Arc::new(broker),
            backend: Arc::new(backend),
            workers: Arc::new(RwLock::new(HashMap::new())),
            queues: Arc::new(RwLock::new(HashMap::new())),
            recent_tasks: Arc::new(RwLock::new(Vec::new())),
            sse_connections: Arc::new(RwLock::new(0)),
        })
    }

    /// Update worker information
    pub async fn update_worker(&self, worker: WorkerInfo) {
        let mut workers = self.workers.write().await;
        workers.insert(worker.id.clone(), worker);
    }

    /// Remove worker
    pub async fn remove_worker(&self, worker_id: &str) {
        let mut workers = self.workers.write().await;
        workers.remove(worker_id);
    }

    /// Mark worker as offline if heartbeat expired
    pub async fn check_worker_heartbeats(&self, timeout_secs: i64) {
        let now = chrono::Utc::now();
        let mut workers = self.workers.write().await;

        for worker in workers.values_mut() {
            let elapsed = now.signed_duration_since(worker.last_heartbeat);
            if elapsed.num_seconds() > timeout_secs {
                worker.status = WorkerStatus::Offline;
            }
        }
    }

    /// Get all workers
    pub async fn get_workers(&self) -> Vec<WorkerInfo> {
        let workers = self.workers.read().await;
        workers.values().cloned().collect()
    }

    /// Update queue statistics
    pub async fn update_queue(&self, queue: QueueInfo) {
        let mut queues = self.queues.write().await;
        queues.insert(queue.name.clone(), queue);
    }

    /// Get all queues
    pub async fn get_queues(&self) -> Vec<QueueInfo> {
        let queues = self.queues.read().await;
        queues.values().cloned().collect()
    }

    /// Add recent task
    pub async fn add_recent_task(&self, task: TaskSummary) {
        let mut tasks = self.recent_tasks.write().await;
        tasks.insert(0, task);

        // Keep only last 100 tasks
        if tasks.len() > 100 {
            tasks.truncate(100);
        }
    }

    /// Get recent tasks
    pub async fn get_recent_tasks(&self, limit: usize) -> Vec<TaskSummary> {
        let tasks = self.recent_tasks.read().await;
        tasks.iter().take(limit).cloned().collect()
    }

    /// Increment SSE connections
    pub async fn inc_sse_connections(&self) -> bool {
        let mut count = self.sse_connections.write().await;
        if *count >= self.config.max_sse_connections {
            return false;
        }
        *count += 1;
        true
    }

    /// Decrement SSE connections
    pub async fn dec_sse_connections(&self) {
        let mut count = self.sse_connections.write().await;
        if *count > 0 {
            *count -= 1;
        }
    }

    /// Get SSE connection count
    pub async fn get_sse_connections(&self) -> usize {
        let count = self.sse_connections.read().await;
        *count
    }
}
