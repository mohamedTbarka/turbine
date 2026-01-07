//! Structured logging for Turbine
//!
//! Provides JSON-formatted structured logging with:
//! - Configurable log levels
//! - Environment-based filtering
//! - Optional JSON output
//! - Request/task context injection

use serde::{Deserialize, Serialize};
use std::io;
use tracing::Level;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan, time::UtcTime},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

use crate::{TelemetryError, TelemetryResult};

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Enable logging
    pub enabled: bool,

    /// Log level (trace, debug, info, warn, error)
    pub level: String,

    /// Output format (json, pretty, compact)
    pub format: LogFormat,

    /// Include span events
    pub include_spans: bool,

    /// Include file/line information
    pub include_location: bool,

    /// Include target (module path)
    pub include_target: bool,

    /// Include thread IDs
    pub include_thread_ids: bool,

    /// Include thread names
    pub include_thread_names: bool,

    /// Environment filter string (e.g., "turbine=debug,tower=warn")
    pub env_filter: Option<String>,
}

/// Log output format
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// JSON format (machine-readable)
    Json,
    /// Pretty format (human-readable, colored)
    #[default]
    Pretty,
    /// Compact format (single line)
    Compact,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: "info".to_string(),
            format: LogFormat::Pretty,
            include_spans: false,
            include_location: true,
            include_target: true,
            include_thread_ids: false,
            include_thread_names: false,
            env_filter: None,
        }
    }
}

/// Initialize logging with configuration
pub fn init_logging(config: LoggingConfig) -> TelemetryResult<()> {
    if !config.enabled {
        return Ok(());
    }

    // Build env filter
    let filter = if let Some(filter_str) = &config.env_filter {
        EnvFilter::try_new(filter_str)
            .map_err(|e| TelemetryError::Logging(e.to_string()))?
    } else {
        EnvFilter::try_new(&config.level)
            .map_err(|e| TelemetryError::Logging(e.to_string()))?
    };

    // Determine span events
    let span_events = if config.include_spans {
        FmtSpan::NEW | FmtSpan::CLOSE
    } else {
        FmtSpan::NONE
    };

    match config.format {
        LogFormat::Json => {
            let layer = fmt::layer()
                .json()
                .with_timer(UtcTime::rfc_3339())
                .with_span_events(span_events)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_target(config.include_target)
                .with_thread_ids(config.include_thread_ids)
                .with_thread_names(config.include_thread_names);

            tracing_subscriber::registry()
                .with(filter)
                .with(layer)
                .try_init()
                .map_err(|e| TelemetryError::Logging(e.to_string()))?;
        }
        LogFormat::Pretty => {
            let layer = fmt::layer()
                .pretty()
                .with_timer(UtcTime::rfc_3339())
                .with_span_events(span_events)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_target(config.include_target)
                .with_thread_ids(config.include_thread_ids)
                .with_thread_names(config.include_thread_names);

            tracing_subscriber::registry()
                .with(filter)
                .with(layer)
                .try_init()
                .map_err(|e| TelemetryError::Logging(e.to_string()))?;
        }
        LogFormat::Compact => {
            let layer = fmt::layer()
                .compact()
                .with_timer(UtcTime::rfc_3339())
                .with_span_events(span_events)
                .with_file(config.include_location)
                .with_line_number(config.include_location)
                .with_target(config.include_target)
                .with_thread_ids(config.include_thread_ids)
                .with_thread_names(config.include_thread_names);

            tracing_subscriber::registry()
                .with(filter)
                .with(layer)
                .try_init()
                .map_err(|e| TelemetryError::Logging(e.to_string()))?;
        }
    }

    tracing::info!(
        "Logging initialized with level: {}, format: {:?}",
        config.level,
        config.format
    );

    Ok(())
}

/// Log context for task execution
#[derive(Debug, Clone)]
pub struct TaskLogContext {
    pub task_id: String,
    pub task_name: String,
    pub queue: String,
    pub correlation_id: Option<String>,
}

impl TaskLogContext {
    /// Create a new task log context
    pub fn new(task_id: &str, task_name: &str, queue: &str) -> Self {
        Self {
            task_id: task_id.to_string(),
            task_name: task_name.to_string(),
            queue: queue.to_string(),
            correlation_id: None,
        }
    }

    /// Set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: &str) -> Self {
        self.correlation_id = Some(correlation_id.to_string());
        self
    }

    /// Create a tracing span with this context
    pub fn span(&self) -> tracing::Span {
        if let Some(ref correlation_id) = self.correlation_id {
            tracing::info_span!(
                "task",
                task_id = %self.task_id,
                task_name = %self.task_name,
                queue = %self.queue,
                correlation_id = %correlation_id
            )
        } else {
            tracing::info_span!(
                "task",
                task_id = %self.task_id,
                task_name = %self.task_name,
                queue = %self.queue
            )
        }
    }
}

/// Worker log context
#[derive(Debug, Clone)]
pub struct WorkerLogContext {
    pub worker_id: String,
    pub queues: Vec<String>,
}

impl WorkerLogContext {
    /// Create a new worker log context
    pub fn new(worker_id: &str, queues: Vec<String>) -> Self {
        Self {
            worker_id: worker_id.to_string(),
            queues,
        }
    }

    /// Create a tracing span with this context
    pub fn span(&self) -> tracing::Span {
        tracing::info_span!(
            "worker",
            worker_id = %self.worker_id,
            queues = ?self.queues
        )
    }
}

/// Workflow log context
#[derive(Debug, Clone)]
pub struct WorkflowLogContext {
    pub workflow_id: String,
    pub workflow_type: String,
}

impl WorkflowLogContext {
    /// Create a new workflow log context
    pub fn new(workflow_id: &str, workflow_type: &str) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            workflow_type: workflow_type.to_string(),
        }
    }

    /// Create a tracing span with this context
    pub fn span(&self) -> tracing::Span {
        tracing::info_span!(
            "workflow",
            workflow_id = %self.workflow_id,
            workflow_type = %self.workflow_type
        )
    }
}

/// Macros for structured logging with context

/// Log task submission
#[macro_export]
macro_rules! log_task_submitted {
    ($task_id:expr, $task_name:expr, $queue:expr) => {
        tracing::info!(
            task_id = %$task_id,
            task_name = %$task_name,
            queue = %$queue,
            event = "task.submitted",
            "Task submitted"
        )
    };
}

/// Log task started
#[macro_export]
macro_rules! log_task_started {
    ($task_id:expr, $task_name:expr, $queue:expr) => {
        tracing::info!(
            task_id = %$task_id,
            task_name = %$task_name,
            queue = %$queue,
            event = "task.started",
            "Task execution started"
        )
    };
}

/// Log task completed
#[macro_export]
macro_rules! log_task_completed {
    ($task_id:expr, $task_name:expr, $queue:expr, $duration_ms:expr) => {
        tracing::info!(
            task_id = %$task_id,
            task_name = %$task_name,
            queue = %$queue,
            duration_ms = $duration_ms,
            event = "task.completed",
            "Task completed successfully"
        )
    };
}

/// Log task failed
#[macro_export]
macro_rules! log_task_failed {
    ($task_id:expr, $task_name:expr, $queue:expr, $error:expr) => {
        tracing::error!(
            task_id = %$task_id,
            task_name = %$task_name,
            queue = %$queue,
            error = %$error,
            event = "task.failed",
            "Task execution failed"
        )
    };
}

/// Log task retried
#[macro_export]
macro_rules! log_task_retried {
    ($task_id:expr, $task_name:expr, $queue:expr, $retry_count:expr, $error:expr) => {
        tracing::warn!(
            task_id = %$task_id,
            task_name = %$task_name,
            queue = %$queue,
            retry_count = $retry_count,
            error = %$error,
            event = "task.retried",
            "Task scheduled for retry"
        )
    };
}
