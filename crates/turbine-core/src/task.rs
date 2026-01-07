//! Task definitions and state management

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use uuid::Uuid;

/// Unique identifier for a task
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub String);

impl TaskId {
    /// Generate a new random task ID
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Create a task ID from an existing string
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the string representation
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TaskId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TaskId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Current state of a task
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskState {
    /// Task is waiting to be processed
    Pending,
    /// Task has been received by a worker
    Received,
    /// Task is currently being executed
    Running,
    /// Task completed successfully
    Success,
    /// Task failed with an error
    Failure,
    /// Task is scheduled for retry
    Retry,
    /// Task was revoked/cancelled
    Revoked,
}

impl TaskState {
    /// Check if this is a terminal state
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskState::Success | TaskState::Failure | TaskState::Revoked)
    }

    /// Check if the task is still active
    pub fn is_active(&self) -> bool {
        matches!(self, TaskState::Pending | TaskState::Received | TaskState::Running | TaskState::Retry)
    }
}

impl std::fmt::Display for TaskState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskState::Pending => write!(f, "pending"),
            TaskState::Received => write!(f, "received"),
            TaskState::Running => write!(f, "running"),
            TaskState::Success => write!(f, "success"),
            TaskState::Failure => write!(f, "failure"),
            TaskState::Retry => write!(f, "retry"),
            TaskState::Revoked => write!(f, "revoked"),
        }
    }
}

/// Options for task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskOptions {
    /// Queue to route the task to
    #[serde(default = "default_queue")]
    pub queue: String,

    /// Task priority (higher = more important)
    #[serde(default)]
    pub priority: u8,

    /// Maximum number of retries
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,

    /// Delay between retries in seconds (exponential backoff base)
    #[serde(default = "default_retry_delay")]
    pub retry_delay: u64,

    /// Hard timeout for task execution in seconds
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Soft timeout (warning) in seconds
    pub soft_timeout: Option<u64>,

    /// Execute after this time (delayed execution)
    pub eta: Option<DateTime<Utc>>,

    /// Execute after this many seconds from now
    pub countdown: Option<u64>,

    /// Expiration time - task won't run after this
    pub expires: Option<DateTime<Utc>>,

    /// Whether to store the result
    #[serde(default = "default_store_result")]
    pub store_result: bool,

    /// Result TTL in seconds
    #[serde(default = "default_result_ttl")]
    pub result_ttl: u64,

    /// Idempotency key for deduplication
    pub idempotency_key: Option<String>,

    /// Custom headers/metadata
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

fn default_queue() -> String {
    "default".to_string()
}

fn default_max_retries() -> u32 {
    3
}

fn default_retry_delay() -> u64 {
    60
}

fn default_timeout() -> u64 {
    300
}

fn default_store_result() -> bool {
    true
}

fn default_result_ttl() -> u64 {
    86400 // 24 hours
}

impl Default for TaskOptions {
    fn default() -> Self {
        Self {
            queue: default_queue(),
            priority: 0,
            max_retries: default_max_retries(),
            retry_delay: default_retry_delay(),
            timeout: default_timeout(),
            soft_timeout: None,
            eta: None,
            countdown: None,
            expires: None,
            store_result: default_store_result(),
            result_ttl: default_result_ttl(),
            idempotency_key: None,
            headers: HashMap::new(),
        }
    }
}

impl TaskOptions {
    /// Create new task options with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the queue
    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.queue = queue.into();
        self
    }

    /// Set the priority
    pub fn priority(mut self, priority: u8) -> Self {
        self.priority = priority;
        self
    }

    /// Set max retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.max_retries = retries;
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout.as_secs();
        self
    }

    /// Set ETA for delayed execution
    pub fn eta(mut self, eta: DateTime<Utc>) -> Self {
        self.eta = Some(eta);
        self
    }

    /// Set countdown for delayed execution
    pub fn countdown(mut self, seconds: u64) -> Self {
        self.countdown = Some(seconds);
        self
    }

    /// Disable result storage
    pub fn no_result(mut self) -> Self {
        self.store_result = false;
        self
    }

    /// Set idempotency key
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.idempotency_key = Some(key.into());
        self
    }

    /// Add a custom header
    pub fn header(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.insert(key.into(), value.into());
        self
    }

    /// Calculate the effective ETA considering countdown
    pub fn effective_eta(&self) -> Option<DateTime<Utc>> {
        if let Some(eta) = self.eta {
            Some(eta)
        } else if let Some(countdown) = self.countdown {
            Some(Utc::now() + chrono::Duration::seconds(countdown as i64))
        } else {
            None
        }
    }
}

/// Metadata about a task's execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMeta {
    /// Current state
    pub state: TaskState,

    /// Number of retries attempted
    pub retries: u32,

    /// When the task was created
    pub created_at: DateTime<Utc>,

    /// When the task started executing
    pub started_at: Option<DateTime<Utc>>,

    /// When the task finished
    pub finished_at: Option<DateTime<Utc>>,

    /// Worker ID that processed/is processing the task
    pub worker_id: Option<String>,

    /// Error message if failed
    pub error: Option<String>,

    /// Error traceback if available
    pub traceback: Option<String>,
}

impl Default for TaskMeta {
    fn default() -> Self {
        Self {
            state: TaskState::Pending,
            retries: 0,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            worker_id: None,
            error: None,
            traceback: None,
        }
    }
}

impl TaskMeta {
    /// Create new task metadata
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark task as started
    pub fn mark_started(&mut self, worker_id: impl Into<String>) {
        self.state = TaskState::Running;
        self.started_at = Some(Utc::now());
        self.worker_id = Some(worker_id.into());
    }

    /// Mark task as succeeded
    pub fn mark_success(&mut self) {
        self.state = TaskState::Success;
        self.finished_at = Some(Utc::now());
    }

    /// Mark task as failed
    pub fn mark_failure(&mut self, error: impl Into<String>, traceback: Option<String>) {
        self.state = TaskState::Failure;
        self.finished_at = Some(Utc::now());
        self.error = Some(error.into());
        self.traceback = traceback;
    }

    /// Mark task for retry
    pub fn mark_retry(&mut self, error: impl Into<String>) {
        self.state = TaskState::Retry;
        self.retries += 1;
        self.error = Some(error.into());
        self.started_at = None;
        self.worker_id = None;
    }

    /// Mark task as revoked
    pub fn mark_revoked(&mut self) {
        self.state = TaskState::Revoked;
        self.finished_at = Some(Utc::now());
    }

    /// Get execution duration if available
    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.started_at, self.finished_at) {
            (Some(start), Some(end)) => Some(end - start),
            (Some(start), None) if self.state == TaskState::Running => Some(Utc::now() - start),
            _ => None,
        }
    }
}

/// A task to be executed
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier
    pub id: TaskId,

    /// Task name (function/handler name)
    pub name: String,

    /// Positional arguments (JSON-serialized)
    pub args: Vec<serde_json::Value>,

    /// Keyword arguments (JSON-serialized)
    pub kwargs: HashMap<String, serde_json::Value>,

    /// Task options
    pub options: TaskOptions,

    /// Task metadata
    pub meta: TaskMeta,

    /// Parent task ID (for workflows)
    pub parent_id: Option<TaskId>,

    /// Root task ID (for nested workflows)
    pub root_id: Option<TaskId>,

    /// Correlation ID for tracing
    pub correlation_id: Option<String>,
}

impl Task {
    /// Create a new task
    pub fn new(name: impl Into<String>) -> Self {
        let id = TaskId::new();
        Self {
            id: id.clone(),
            name: name.into(),
            args: Vec::new(),
            kwargs: HashMap::new(),
            options: TaskOptions::default(),
            meta: TaskMeta::default(),
            parent_id: None,
            root_id: Some(id),
            correlation_id: None,
        }
    }

    /// Create a task with a specific ID
    pub fn with_id(id: TaskId, name: impl Into<String>) -> Self {
        Self {
            id: id.clone(),
            name: name.into(),
            args: Vec::new(),
            kwargs: HashMap::new(),
            options: TaskOptions::default(),
            meta: TaskMeta::default(),
            parent_id: None,
            root_id: Some(id),
            correlation_id: None,
        }
    }

    /// Add positional arguments
    pub fn args<T: Serialize>(mut self, args: Vec<T>) -> crate::Result<Self> {
        self.args = args
            .into_iter()
            .map(|a| serde_json::to_value(a))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    /// Add a single positional argument
    pub fn arg<T: Serialize>(mut self, arg: T) -> crate::Result<Self> {
        self.args.push(serde_json::to_value(arg)?);
        Ok(self)
    }

    /// Add keyword arguments
    pub fn kwargs<T: Serialize>(mut self, kwargs: HashMap<String, T>) -> crate::Result<Self> {
        self.kwargs = kwargs
            .into_iter()
            .map(|(k, v)| Ok((k, serde_json::to_value(v)?)))
            .collect::<crate::Result<HashMap<_, _>>>()?;
        Ok(self)
    }

    /// Add a single keyword argument
    pub fn kwarg<T: Serialize>(mut self, key: impl Into<String>, value: T) -> crate::Result<Self> {
        self.kwargs.insert(key.into(), serde_json::to_value(value)?);
        Ok(self)
    }

    /// Set task options
    pub fn options(mut self, options: TaskOptions) -> Self {
        self.options = options;
        self
    }

    /// Set the queue
    pub fn queue(mut self, queue: impl Into<String>) -> Self {
        self.options.queue = queue.into();
        self
    }

    /// Set max retries
    pub fn max_retries(mut self, retries: u32) -> Self {
        self.options.max_retries = retries;
        self
    }

    /// Set timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.options.timeout = timeout.as_secs();
        self
    }

    /// Set priority
    pub fn priority(mut self, priority: u8) -> Self {
        self.options.priority = priority;
        self
    }

    /// Set parent task (for workflows)
    pub fn parent(mut self, parent_id: TaskId) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Set root task (for nested workflows)
    pub fn root(mut self, root_id: TaskId) -> Self {
        self.root_id = Some(root_id);
        self
    }

    /// Set correlation ID
    pub fn correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Check if task should be executed now
    pub fn should_execute_now(&self) -> bool {
        match self.options.effective_eta() {
            Some(eta) => Utc::now() >= eta,
            None => true,
        }
    }

    /// Check if task has expired
    pub fn is_expired(&self) -> bool {
        match self.options.expires {
            Some(expires) => Utc::now() > expires,
            None => false,
        }
    }

    /// Check if task can be retried
    pub fn can_retry(&self) -> bool {
        self.meta.retries < self.options.max_retries
    }

    /// Calculate delay for next retry (exponential backoff)
    pub fn retry_delay(&self) -> Duration {
        let base = self.options.retry_delay;
        let multiplier = 2_u64.pow(self.meta.retries);
        let delay = base.saturating_mul(multiplier);
        // Cap at 1 hour
        Duration::from_secs(delay.min(3600))
    }
}

/// Result of a task execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task ID
    pub task_id: TaskId,

    /// Final state
    pub state: TaskState,

    /// Result value (if successful)
    pub result: Option<serde_json::Value>,

    /// Error message (if failed)
    pub error: Option<String>,

    /// Traceback (if failed)
    pub traceback: Option<String>,

    /// When the result was created
    pub created_at: DateTime<Utc>,
}

impl TaskResult {
    /// Create a success result
    pub fn success<T: Serialize>(task_id: TaskId, result: T) -> crate::Result<Self> {
        Ok(Self {
            task_id,
            state: TaskState::Success,
            result: Some(serde_json::to_value(result)?),
            error: None,
            traceback: None,
            created_at: Utc::now(),
        })
    }

    /// Create a failure result
    pub fn failure(task_id: TaskId, error: impl Into<String>, traceback: Option<String>) -> Self {
        Self {
            task_id,
            state: TaskState::Failure,
            result: None,
            error: Some(error.into()),
            traceback,
            created_at: Utc::now(),
        }
    }

    /// Create a revoked result
    pub fn revoked(task_id: TaskId) -> Self {
        Self {
            task_id,
            state: TaskState::Revoked,
            result: None,
            error: Some("Task was revoked".to_string()),
            traceback: None,
            created_at: Utc::now(),
        }
    }

    /// Get the result as a specific type
    pub fn get<T: for<'de> Deserialize<'de>>(&self) -> crate::Result<T> {
        match &self.result {
            Some(value) => Ok(serde_json::from_value(value.clone())?),
            None => Err(crate::Error::TaskExecutionFailed(
                self.error.clone().unwrap_or_else(|| "No result".to_string()),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("my_task");
        assert_eq!(task.name, "my_task");
        assert_eq!(task.meta.state, TaskState::Pending);
        assert_eq!(task.options.queue, "default");
    }

    #[test]
    fn test_task_with_args() {
        let task = Task::new("add")
            .arg(1)
            .unwrap()
            .arg(2)
            .unwrap()
            .kwarg("extra", "value")
            .unwrap();

        assert_eq!(task.args.len(), 2);
        assert_eq!(task.kwargs.len(), 1);
    }

    #[test]
    fn test_task_state_transitions() {
        let mut meta = TaskMeta::new();
        assert!(meta.state.is_active());
        assert!(!meta.state.is_terminal());

        meta.mark_started("worker-1");
        assert_eq!(meta.state, TaskState::Running);
        assert!(meta.started_at.is_some());

        meta.mark_success();
        assert_eq!(meta.state, TaskState::Success);
        assert!(meta.state.is_terminal());
    }

    #[test]
    fn test_retry_delay_exponential_backoff() {
        let mut task = Task::new("test");
        task.options.retry_delay = 10;

        assert_eq!(task.retry_delay(), Duration::from_secs(10));

        task.meta.retries = 1;
        assert_eq!(task.retry_delay(), Duration::from_secs(20));

        task.meta.retries = 2;
        assert_eq!(task.retry_delay(), Duration::from_secs(40));

        task.meta.retries = 3;
        assert_eq!(task.retry_delay(), Duration::from_secs(80));
    }
}
