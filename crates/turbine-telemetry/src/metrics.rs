//! Prometheus metrics for Turbine
//!
//! Provides comprehensive metrics for monitoring task queues:
//! - Task counters (submitted, processed, failed, retried)
//! - Latency histograms (queue time, execution time)
//! - Queue depth gauges
//! - Worker status

use lazy_static::lazy_static;
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramOpts, HistogramVec, IntCounter,
    IntCounterVec, IntGauge, IntGaugeVec, Opts, Registry,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;

use crate::{TelemetryError, TelemetryResult};

lazy_static! {
    /// Global metrics registry
    pub static ref REGISTRY: Registry = Registry::new();

    // ============ Task Counters ============

    /// Total tasks submitted
    pub static ref TASKS_SUBMITTED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_tasks_submitted_total", "Total number of tasks submitted"),
        &["queue", "task_name"]
    ).expect("metric can be created");

    /// Total tasks completed successfully
    pub static ref TASKS_SUCCEEDED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_tasks_succeeded_total", "Total number of tasks completed successfully"),
        &["queue", "task_name"]
    ).expect("metric can be created");

    /// Total tasks failed
    pub static ref TASKS_FAILED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_tasks_failed_total", "Total number of tasks failed"),
        &["queue", "task_name", "error_type"]
    ).expect("metric can be created");

    /// Total tasks retried
    pub static ref TASKS_RETRIED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_tasks_retried_total", "Total number of tasks retried"),
        &["queue", "task_name"]
    ).expect("metric can be created");

    /// Total tasks sent to DLQ
    pub static ref TASKS_DEAD_LETTERED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_tasks_dead_lettered_total", "Total number of tasks sent to dead letter queue"),
        &["queue", "task_name"]
    ).expect("metric can be created");

    /// Total tasks revoked
    pub static ref TASKS_REVOKED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_tasks_revoked_total", "Total number of tasks revoked"),
        &["queue", "task_name"]
    ).expect("metric can be created");

    // ============ Queue Metrics ============

    /// Current queue depth
    pub static ref QUEUE_DEPTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new("turbine_queue_depth", "Current number of messages in queue"),
        &["queue"]
    ).expect("metric can be created");

    /// Current DLQ depth
    pub static ref DLQ_DEPTH: IntGaugeVec = IntGaugeVec::new(
        Opts::new("turbine_dlq_depth", "Current number of messages in dead letter queue"),
        &["queue"]
    ).expect("metric can be created");

    /// Messages in processing (not yet acked)
    pub static ref QUEUE_PROCESSING: IntGaugeVec = IntGaugeVec::new(
        Opts::new("turbine_queue_processing", "Number of messages currently being processed"),
        &["queue"]
    ).expect("metric can be created");

    // ============ Latency Histograms ============

    /// Time from submission to worker pickup (queue wait time)
    pub static ref TASK_QUEUE_TIME: HistogramVec = HistogramVec::new(
        HistogramOpts::new("turbine_task_queue_time_seconds", "Time spent waiting in queue")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0]),
        &["queue", "task_name"]
    ).expect("metric can be created");

    /// Task execution time
    pub static ref TASK_EXECUTION_TIME: HistogramVec = HistogramVec::new(
        HistogramOpts::new("turbine_task_execution_time_seconds", "Time spent executing task")
            .buckets(vec![0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0]),
        &["queue", "task_name"]
    ).expect("metric can be created");

    /// Total task latency (queue time + execution time)
    pub static ref TASK_TOTAL_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("turbine_task_total_latency_seconds", "Total task latency from submission to completion")
            .buckets(vec![0.001, 0.01, 0.1, 0.5, 1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0]),
        &["queue", "task_name"]
    ).expect("metric can be created");

    // ============ Worker Metrics ============

    /// Number of active workers
    pub static ref WORKERS_ACTIVE: IntGauge = IntGauge::new(
        "turbine_workers_active", "Number of active workers"
    ).expect("metric can be created");

    /// Worker utilization (0-1)
    pub static ref WORKER_UTILIZATION: GaugeVec = GaugeVec::new(
        Opts::new("turbine_worker_utilization", "Worker utilization ratio (0-1)"),
        &["worker_id"]
    ).expect("metric can be created");

    /// Tasks currently being processed per worker
    pub static ref WORKER_TASKS_PROCESSING: IntGaugeVec = IntGaugeVec::new(
        Opts::new("turbine_worker_tasks_processing", "Number of tasks currently being processed by worker"),
        &["worker_id"]
    ).expect("metric can be created");

    /// Worker heartbeat timestamp
    pub static ref WORKER_HEARTBEAT: GaugeVec = GaugeVec::new(
        Opts::new("turbine_worker_heartbeat_timestamp", "Last heartbeat timestamp"),
        &["worker_id"]
    ).expect("metric can be created");

    // ============ Broker Metrics ============

    /// Broker connection status (1 = connected, 0 = disconnected)
    pub static ref BROKER_CONNECTED: IntGauge = IntGauge::new(
        "turbine_broker_connected", "Broker connection status (1 = connected)"
    ).expect("metric can be created");

    /// Broker publish latency
    pub static ref BROKER_PUBLISH_LATENCY: Histogram = Histogram::with_opts(
        HistogramOpts::new("turbine_broker_publish_latency_seconds", "Broker publish latency")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0])
    ).expect("metric can be created");

    // ============ Backend Metrics ============

    /// Backend connection status
    pub static ref BACKEND_CONNECTED: IntGauge = IntGauge::new(
        "turbine_backend_connected", "Result backend connection status (1 = connected)"
    ).expect("metric can be created");

    /// Backend operation latency
    pub static ref BACKEND_OPERATION_LATENCY: HistogramVec = HistogramVec::new(
        HistogramOpts::new("turbine_backend_operation_latency_seconds", "Result backend operation latency")
            .buckets(vec![0.0001, 0.0005, 0.001, 0.005, 0.01, 0.05, 0.1, 0.5, 1.0]),
        &["operation"]
    ).expect("metric can be created");

    // ============ Workflow Metrics ============

    /// Active workflows
    pub static ref WORKFLOWS_ACTIVE: IntGaugeVec = IntGaugeVec::new(
        Opts::new("turbine_workflows_active", "Number of active workflows"),
        &["workflow_type"]
    ).expect("metric can be created");

    /// Completed workflows
    pub static ref WORKFLOWS_COMPLETED: IntCounterVec = IntCounterVec::new(
        Opts::new("turbine_workflows_completed_total", "Total workflows completed"),
        &["workflow_type", "status"]
    ).expect("metric can be created");

    // ============ Scheduler Metrics ============

    /// Scheduled tasks pending
    pub static ref SCHEDULER_PENDING: IntGauge = IntGauge::new(
        "turbine_scheduler_pending", "Number of scheduled tasks pending"
    ).expect("metric can be created");

    /// Scheduled tasks executed
    pub static ref SCHEDULER_EXECUTED: IntCounter = IntCounter::new(
        "turbine_scheduler_executed_total", "Total scheduled tasks executed"
    ).expect("metric can be created");
}

/// Metrics configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricsConfig {
    /// Enable metrics collection
    pub enabled: bool,

    /// HTTP endpoint for Prometheus scraping
    pub endpoint: String,

    /// Port for metrics server
    pub port: u16,

    /// Include default process metrics
    pub include_process_metrics: bool,
}

impl Default for MetricsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: "/metrics".to_string(),
            port: 9090,
            include_process_metrics: true,
        }
    }
}

/// Turbine metrics interface
pub struct TurbineMetrics {
    config: MetricsConfig,
}

impl TurbineMetrics {
    /// Initialize metrics with configuration
    pub fn new(config: MetricsConfig) -> TelemetryResult<Self> {
        // Register all metrics with the registry
        Self::register_metrics()?;

        Ok(Self { config })
    }

    fn register_metrics() -> TelemetryResult<()> {
        // Task counters
        REGISTRY
            .register(Box::new(TASKS_SUBMITTED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASKS_SUCCEEDED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASKS_FAILED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASKS_RETRIED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASKS_DEAD_LETTERED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASKS_REVOKED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Queue metrics
        REGISTRY
            .register(Box::new(QUEUE_DEPTH.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(DLQ_DEPTH.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(QUEUE_PROCESSING.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Latency histograms
        REGISTRY
            .register(Box::new(TASK_QUEUE_TIME.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASK_EXECUTION_TIME.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(TASK_TOTAL_LATENCY.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Worker metrics
        REGISTRY
            .register(Box::new(WORKERS_ACTIVE.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(WORKER_UTILIZATION.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(WORKER_TASKS_PROCESSING.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(WORKER_HEARTBEAT.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Broker metrics
        REGISTRY
            .register(Box::new(BROKER_CONNECTED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(BROKER_PUBLISH_LATENCY.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Backend metrics
        REGISTRY
            .register(Box::new(BACKEND_CONNECTED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(BACKEND_OPERATION_LATENCY.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Workflow metrics
        REGISTRY
            .register(Box::new(WORKFLOWS_ACTIVE.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(WORKFLOWS_COMPLETED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        // Scheduler metrics
        REGISTRY
            .register(Box::new(SCHEDULER_PENDING.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;
        REGISTRY
            .register(Box::new(SCHEDULER_EXECUTED.clone()))
            .map_err(|e| TelemetryError::Metrics(e.to_string()))?;

        Ok(())
    }

    /// Get metrics as Prometheus text format
    pub fn gather(&self) -> String {
        use prometheus::Encoder;
        let encoder = prometheus::TextEncoder::new();
        let metric_families = REGISTRY.gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap()
    }

    /// Record task submission
    pub fn task_submitted(queue: &str, task_name: &str) {
        TASKS_SUBMITTED
            .with_label_values(&[queue, task_name])
            .inc();
    }

    /// Record task success
    pub fn task_succeeded(queue: &str, task_name: &str) {
        TASKS_SUCCEEDED
            .with_label_values(&[queue, task_name])
            .inc();
    }

    /// Record task failure
    pub fn task_failed(queue: &str, task_name: &str, error_type: &str) {
        TASKS_FAILED
            .with_label_values(&[queue, task_name, error_type])
            .inc();
    }

    /// Record task retry
    pub fn task_retried(queue: &str, task_name: &str) {
        TASKS_RETRIED
            .with_label_values(&[queue, task_name])
            .inc();
    }

    /// Record task sent to DLQ
    pub fn task_dead_lettered(queue: &str, task_name: &str) {
        TASKS_DEAD_LETTERED
            .with_label_values(&[queue, task_name])
            .inc();
    }

    /// Set queue depth
    pub fn set_queue_depth(queue: &str, depth: i64) {
        QUEUE_DEPTH.with_label_values(&[queue]).set(depth);
    }

    /// Set DLQ depth
    pub fn set_dlq_depth(queue: &str, depth: i64) {
        DLQ_DEPTH.with_label_values(&[queue]).set(depth);
    }

    /// Record queue wait time
    pub fn record_queue_time(queue: &str, task_name: &str, duration_secs: f64) {
        TASK_QUEUE_TIME
            .with_label_values(&[queue, task_name])
            .observe(duration_secs);
    }

    /// Record execution time
    pub fn record_execution_time(queue: &str, task_name: &str, duration_secs: f64) {
        TASK_EXECUTION_TIME
            .with_label_values(&[queue, task_name])
            .observe(duration_secs);
    }

    /// Record total latency
    pub fn record_total_latency(queue: &str, task_name: &str, duration_secs: f64) {
        TASK_TOTAL_LATENCY
            .with_label_values(&[queue, task_name])
            .observe(duration_secs);
    }

    /// Set active workers count
    pub fn set_active_workers(count: i64) {
        WORKERS_ACTIVE.set(count);
    }

    /// Set worker utilization
    pub fn set_worker_utilization(worker_id: &str, utilization: f64) {
        WORKER_UTILIZATION
            .with_label_values(&[worker_id])
            .set(utilization);
    }

    /// Record worker heartbeat
    pub fn record_worker_heartbeat(worker_id: &str) {
        let now = chrono::Utc::now().timestamp() as f64;
        WORKER_HEARTBEAT.with_label_values(&[worker_id]).set(now);
    }

    /// Set broker connection status
    pub fn set_broker_connected(connected: bool) {
        BROKER_CONNECTED.set(if connected { 1 } else { 0 });
    }

    /// Record broker publish latency
    pub fn record_broker_publish_latency(duration_secs: f64) {
        BROKER_PUBLISH_LATENCY.observe(duration_secs);
    }
}

/// Timer guard for automatic metric recording
pub struct MetricTimer {
    start: Instant,
    queue: String,
    task_name: String,
    metric_type: MetricType,
}

enum MetricType {
    QueueTime,
    ExecutionTime,
    TotalLatency,
}

impl MetricTimer {
    /// Start a queue time timer
    pub fn queue_time(queue: &str, task_name: &str) -> Self {
        Self {
            start: Instant::now(),
            queue: queue.to_string(),
            task_name: task_name.to_string(),
            metric_type: MetricType::QueueTime,
        }
    }

    /// Start an execution time timer
    pub fn execution_time(queue: &str, task_name: &str) -> Self {
        Self {
            start: Instant::now(),
            queue: queue.to_string(),
            task_name: task_name.to_string(),
            metric_type: MetricType::ExecutionTime,
        }
    }

    /// Start a total latency timer
    pub fn total_latency(queue: &str, task_name: &str) -> Self {
        Self {
            start: Instant::now(),
            queue: queue.to_string(),
            task_name: task_name.to_string(),
            metric_type: MetricType::TotalLatency,
        }
    }
}

impl Drop for MetricTimer {
    fn drop(&mut self) {
        let duration = self.start.elapsed().as_secs_f64();
        match self.metric_type {
            MetricType::QueueTime => {
                TurbineMetrics::record_queue_time(&self.queue, &self.task_name, duration);
            }
            MetricType::ExecutionTime => {
                TurbineMetrics::record_execution_time(&self.queue, &self.task_name, duration);
            }
            MetricType::TotalLatency => {
                TurbineMetrics::record_total_latency(&self.queue, &self.task_name, duration);
            }
        }
    }
}
