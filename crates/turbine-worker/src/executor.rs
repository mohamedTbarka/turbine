//! Task executor implementation

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::process::Command;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use turbine_backend::Backend;
use turbine_broker::Consumer;
use turbine_core::{Delivery, Task, TaskId, TaskMeta, TaskResult, TaskState};

/// Result of task execution
#[derive(Debug, Clone)]
pub struct ExecutionResult {
    /// Task ID
    pub task_id: TaskId,

    /// Success or failure
    pub success: bool,

    /// Result value (JSON)
    pub result: Option<serde_json::Value>,

    /// Error message if failed
    pub error: Option<String>,

    /// Traceback if failed
    pub traceback: Option<String>,

    /// Execution duration
    pub duration: Duration,
}

impl ExecutionResult {
    /// Create a success result
    pub fn success(task_id: TaskId, result: serde_json::Value, duration: Duration) -> Self {
        Self {
            task_id,
            success: true,
            result: Some(result),
            error: None,
            traceback: None,
            duration,
        }
    }

    /// Create a failure result
    pub fn failure(
        task_id: TaskId,
        error: String,
        traceback: Option<String>,
        duration: Duration,
    ) -> Self {
        Self {
            task_id,
            success: false,
            result: None,
            error: Some(error),
            traceback,
            duration,
        }
    }

    /// Convert to TaskResult
    pub fn to_task_result(&self) -> TaskResult {
        if self.success {
            TaskResult {
                task_id: self.task_id.clone(),
                state: TaskState::Success,
                result: self.result.clone(),
                error: None,
                traceback: None,
                created_at: chrono::Utc::now(),
            }
        } else {
            TaskResult {
                task_id: self.task_id.clone(),
                state: TaskState::Failure,
                result: None,
                error: self.error.clone(),
                traceback: self.traceback.clone(),
                created_at: chrono::Utc::now(),
            }
        }
    }
}

/// Trait for task handlers
#[async_trait]
pub trait TaskHandler: Send + Sync {
    /// Execute a task
    async fn execute(&self, task: &Task) -> ExecutionResult;

    /// Get the handler name
    fn name(&self) -> &str;
}

/// Built-in task handlers registry
pub struct TaskRegistry {
    handlers: RwLock<HashMap<String, Arc<dyn TaskHandler>>>,
}

impl TaskRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            handlers: RwLock::new(HashMap::new()),
        }
    }

    /// Register a handler
    pub async fn register(&self, name: impl Into<String>, handler: Arc<dyn TaskHandler>) {
        let mut handlers = self.handlers.write().await;
        handlers.insert(name.into(), handler);
    }

    /// Get a handler
    pub async fn get(&self, name: &str) -> Option<Arc<dyn TaskHandler>> {
        let handlers = self.handlers.read().await;
        handlers.get(name).cloned()
    }

    /// Check if a handler exists
    pub async fn has(&self, name: &str) -> bool {
        let handlers = self.handlers.read().await;
        handlers.contains_key(name)
    }
}

impl Default for TaskRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Task executor - runs tasks and manages execution
pub struct TaskExecutor<B: Backend> {
    /// Worker ID
    worker_id: String,

    /// Task registry
    registry: Arc<TaskRegistry>,

    /// Result backend
    backend: B,

    /// Execution timeout
    timeout: Duration,

    /// Whether to use subprocess isolation
    subprocess_isolation: bool,
}

impl<B: Backend> TaskExecutor<B> {
    /// Create a new executor
    pub fn new(worker_id: String, registry: Arc<TaskRegistry>, backend: B) -> Self {
        Self {
            worker_id,
            registry,
            backend,
            timeout: Duration::from_secs(300),
            subprocess_isolation: false,
        }
    }

    /// Set execution timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Enable subprocess isolation
    pub fn with_subprocess_isolation(mut self, enabled: bool) -> Self {
        self.subprocess_isolation = enabled;
        self
    }

    /// Execute a task from a delivery
    pub async fn execute(&self, delivery: &Delivery) -> ExecutionResult {
        let task = match delivery.message.extract_task() {
            Ok(t) => t,
            Err(e) => {
                return ExecutionResult::failure(
                    delivery.message.task_id(),
                    format!("Failed to extract task: {}", e),
                    None,
                    Duration::ZERO,
                );
            }
        };

        info!(
            "Executing task {} ({}) on worker {}",
            task.id, task.name, self.worker_id
        );

        // Update state to running
        let mut meta = TaskMeta::new();
        meta.mark_started(&self.worker_id);
        if let Err(e) = self.backend.store_meta(&task.id, &meta).await {
            warn!("Failed to update task state: {}", e);
        }

        let start = Instant::now();

        // Get task timeout
        let timeout = Duration::from_secs(task.options.timeout);

        // Execute with timeout
        let result = tokio::time::timeout(timeout, self.execute_task(&task)).await;

        let duration = start.elapsed();

        let execution_result = match result {
            Ok(r) => r,
            Err(_) => {
                error!("Task {} timed out after {:?}", task.id, timeout);
                ExecutionResult::failure(
                    task.id.clone(),
                    format!("Task timed out after {} seconds", timeout.as_secs()),
                    None,
                    duration,
                )
            }
        };

        // Update final state
        if execution_result.success {
            meta.mark_success();
            info!(
                "Task {} completed successfully in {:?}",
                task.id, duration
            );
        } else {
            meta.mark_failure(
                execution_result.error.clone().unwrap_or_default(),
                execution_result.traceback.clone(),
            );
            error!(
                "Task {} failed: {}",
                task.id,
                execution_result.error.as_deref().unwrap_or("unknown error")
            );
        }

        // Store metadata
        if let Err(e) = self.backend.store_meta(&task.id, &meta).await {
            warn!("Failed to store task metadata: {}", e);
        }

        // Store result if requested
        if task.options.store_result {
            let task_result = execution_result.to_task_result();
            let ttl = Some(Duration::from_secs(task.options.result_ttl));
            if let Err(e) = self.backend.store_result(&task_result, ttl).await {
                warn!("Failed to store task result: {}", e);
            }
        }

        execution_result
    }

    /// Execute the actual task
    async fn execute_task(&self, task: &Task) -> ExecutionResult {
        // Check if we have a registered handler
        if let Some(handler) = self.registry.get(&task.name).await {
            return handler.execute(task).await;
        }

        // If subprocess isolation is enabled, run via subprocess
        if self.subprocess_isolation {
            return self.execute_subprocess(task).await;
        }

        // No handler found
        ExecutionResult::failure(
            task.id.clone(),
            format!("No handler registered for task '{}'", task.name),
            None,
            Duration::ZERO,
        )
    }

    /// Execute task in a subprocess
    async fn execute_subprocess(&self, task: &Task) -> ExecutionResult {
        let start = Instant::now();

        // Serialize task to JSON for subprocess
        let task_json = match serde_json::to_string(task) {
            Ok(j) => j,
            Err(e) => {
                return ExecutionResult::failure(
                    task.id.clone(),
                    format!("Failed to serialize task: {}", e),
                    None,
                    start.elapsed(),
                );
            }
        };

        // Run subprocess
        // This would typically run a Python script or other executor
        let output = Command::new("turbine-task-runner")
            .arg("--task")
            .arg(&task_json)
            .output()
            .await;

        let duration = start.elapsed();

        match output {
            Ok(output) => {
                if output.status.success() {
                    // Parse result from stdout
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    match serde_json::from_str::<serde_json::Value>(&stdout) {
                        Ok(result) => ExecutionResult::success(task.id.clone(), result, duration),
                        Err(_) => {
                            // Return raw output as string
                            ExecutionResult::success(
                                task.id.clone(),
                                serde_json::Value::String(stdout.to_string()),
                                duration,
                            )
                        }
                    }
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    ExecutionResult::failure(
                        task.id.clone(),
                        format!("Task process exited with code {:?}", output.status.code()),
                        Some(stderr.to_string()),
                        duration,
                    )
                }
            }
            Err(e) => ExecutionResult::failure(
                task.id.clone(),
                format!("Failed to spawn subprocess: {}", e),
                None,
                duration,
            ),
        }
    }
}

/// Example built-in task handler for testing
pub struct EchoHandler;

#[async_trait]
impl TaskHandler for EchoHandler {
    async fn execute(&self, task: &Task) -> ExecutionResult {
        let start = Instant::now();

        // Simply return the args as result
        let result = serde_json::json!({
            "task_id": task.id.to_string(),
            "task_name": task.name,
            "args": task.args,
            "kwargs": task.kwargs,
        });

        ExecutionResult::success(task.id.clone(), result, start.elapsed())
    }

    fn name(&self) -> &str {
        "echo"
    }
}

/// Sleep handler for testing delays
pub struct SleepHandler;

#[async_trait]
impl TaskHandler for SleepHandler {
    async fn execute(&self, task: &Task) -> ExecutionResult {
        let start = Instant::now();

        // Get sleep duration from args
        let seconds = task
            .args
            .first()
            .and_then(|v| v.as_u64())
            .unwrap_or(1);

        tokio::time::sleep(Duration::from_secs(seconds)).await;

        let result = serde_json::json!({
            "slept_for": seconds,
        });

        ExecutionResult::success(task.id.clone(), result, start.elapsed())
    }

    fn name(&self) -> &str {
        "sleep"
    }
}

/// Failing handler for testing error handling
pub struct FailHandler;

#[async_trait]
impl TaskHandler for FailHandler {
    async fn execute(&self, task: &Task) -> ExecutionResult {
        let start = Instant::now();

        let message = task
            .args
            .first()
            .and_then(|v| v.as_str())
            .unwrap_or("Intentional failure");

        ExecutionResult::failure(
            task.id.clone(),
            message.to_string(),
            Some("FailHandler traceback".to_string()),
            start.elapsed(),
        )
    }

    fn name(&self) -> &str {
        "fail"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_echo_handler() {
        let handler = EchoHandler;
        let task = Task::new("echo").arg("hello").unwrap();

        let result = handler.execute(&task).await;
        assert!(result.success);
        assert!(result.result.is_some());
    }

    #[tokio::test]
    async fn test_fail_handler() {
        let handler = FailHandler;
        let task = Task::new("fail").arg("test error").unwrap();

        let result = handler.execute(&task).await;
        assert!(!result.success);
        assert_eq!(result.error, Some("test error".to_string()));
    }

    #[tokio::test]
    async fn test_registry() {
        let registry = TaskRegistry::new();

        registry
            .register("echo", Arc::new(EchoHandler))
            .await;

        assert!(registry.has("echo").await);
        assert!(!registry.has("unknown").await);

        let handler = registry.get("echo").await;
        assert!(handler.is_some());
    }
}
