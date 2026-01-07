//! Server state management

use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use turbine_backend::{Backend, RedisBackend};
use turbine_broker::{Broker, RedisBroker};
use turbine_core::TurbineConfig;

/// Shared server state
pub struct ServerState {
    /// Configuration
    pub config: TurbineConfig,

    /// Message broker
    pub broker: RedisBroker,

    /// Result backend
    pub backend: RedisBackend,

    /// Server start time
    pub start_time: Instant,

    /// Active workflows
    pub workflows: RwLock<std::collections::HashMap<String, WorkflowState>>,
}

impl ServerState {
    /// Create new server state
    pub async fn new(config: TurbineConfig) -> anyhow::Result<Arc<Self>> {
        // Connect to broker
        let broker = RedisBroker::connect(&config.broker.url).await?;

        // Connect to backend
        let backend_url = config
            .backend
            .url
            .clone()
            .unwrap_or_else(|| config.broker.url.clone());
        let backend = RedisBackend::connect(&backend_url).await?;

        Ok(Arc::new(Self {
            config,
            broker,
            backend,
            start_time: Instant::now(),
            workflows: RwLock::new(std::collections::HashMap::new()),
        }))
    }

    /// Get uptime in seconds
    pub fn uptime(&self) -> u64 {
        self.start_time.elapsed().as_secs()
    }

    /// Check broker health
    pub async fn broker_healthy(&self) -> bool {
        self.broker.is_connected().await
    }

    /// Check backend health
    pub async fn backend_healthy(&self) -> bool {
        self.backend.is_connected().await
    }
}

/// State of a workflow execution
#[derive(Debug, Clone)]
pub struct WorkflowState {
    /// Workflow ID
    pub id: String,

    /// Workflow type
    pub workflow_type: WorkflowType,

    /// Task IDs in the workflow
    pub task_ids: Vec<String>,

    /// Completed task count
    pub completed: usize,

    /// Failed task count
    pub failed: usize,

    /// Overall state
    pub state: turbine_core::TaskState,

    /// Creation time
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Type of workflow
#[derive(Debug, Clone, Copy)]
pub enum WorkflowType {
    Chain,
    Group,
    Chord,
}

impl WorkflowState {
    /// Create a new workflow state
    pub fn new(id: String, workflow_type: WorkflowType, task_ids: Vec<String>) -> Self {
        Self {
            id,
            workflow_type,
            task_ids,
            completed: 0,
            failed: 0,
            state: turbine_core::TaskState::Pending,
            created_at: chrono::Utc::now(),
        }
    }

    /// Check if workflow is complete
    pub fn is_complete(&self) -> bool {
        self.completed + self.failed >= self.task_ids.len()
    }

    /// Update workflow based on task completion
    pub fn task_completed(&mut self, success: bool) {
        if success {
            self.completed += 1;
        } else {
            self.failed += 1;
        }

        // Update overall state
        if self.is_complete() {
            if self.failed > 0 {
                self.state = turbine_core::TaskState::Failure;
            } else {
                self.state = turbine_core::TaskState::Success;
            }
        } else {
            self.state = turbine_core::TaskState::Running;
        }
    }
}
