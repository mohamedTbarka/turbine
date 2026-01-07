//! OpenTelemetry tracing integration
//!
//! Provides distributed tracing for Turbine using OpenTelemetry:
//! - Automatic trace context propagation
//! - Span creation for task lifecycle
//! - Export to OTLP-compatible backends (Jaeger, Zipkin, etc.)

use opentelemetry::trace::{Span, SpanKind, Status, Tracer};
use opentelemetry::{global, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    runtime::Tokio,
    trace::{BatchConfig, RandomIdGenerator, Sampler},
    Resource,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, Registry};

use crate::{TelemetryError, TelemetryResult};

/// OpenTelemetry tracing configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    /// Enable tracing
    pub enabled: bool,

    /// Service name
    pub service_name: String,

    /// Service version
    pub service_version: String,

    /// OTLP endpoint
    pub otlp_endpoint: String,

    /// Sample rate (0.0 - 1.0)
    pub sample_rate: f64,

    /// Batch export timeout in milliseconds
    pub export_timeout_ms: u64,

    /// Maximum batch size
    pub max_batch_size: usize,

    /// Additional resource attributes
    pub resource_attributes: HashMap<String, String>,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            service_name: "turbine".to_string(),
            service_version: env!("CARGO_PKG_VERSION").to_string(),
            otlp_endpoint: "http://localhost:4317".to_string(),
            sample_rate: 1.0,
            export_timeout_ms: 30000,
            max_batch_size: 512,
            resource_attributes: HashMap::new(),
        }
    }
}

/// Initialize OpenTelemetry tracing
pub fn init_tracing(config: TracingConfig) -> TelemetryResult<()> {
    if !config.enabled {
        return Ok(());
    }

    // Build resource attributes
    let mut attributes = vec![
        KeyValue::new("service.name", config.service_name.clone()),
        KeyValue::new("service.version", config.service_version.clone()),
    ];

    for (key, value) in &config.resource_attributes {
        attributes.push(KeyValue::new(key.clone(), value.clone()));
    }

    let resource = Resource::new(attributes);

    // Configure OTLP exporter
    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&config.otlp_endpoint)
        .with_timeout(Duration::from_millis(config.export_timeout_ms));

    // Build tracer via OTLP pipeline
    // Note: install_batch() returns a Tracer directly and sets up the global tracer provider
    let tracer = opentelemetry_otlp::new_pipeline()
        .tracing()
        .with_exporter(exporter)
        .with_trace_config(
            opentelemetry_sdk::trace::Config::default()
                .with_sampler(Sampler::TraceIdRatioBased(config.sample_rate))
                .with_id_generator(RandomIdGenerator::default())
                .with_resource(resource),
        )
        .with_batch_config(
            BatchConfig::default()
                .with_max_export_batch_size(config.max_batch_size),
        )
        .install_batch(Tokio)
        .map_err(|e| TelemetryError::Tracing(e.to_string()))?;

    // Create OpenTelemetry layer for tracing integration
    let otel_layer = OpenTelemetryLayer::new(tracer);

    // Initialize tracing subscriber
    Registry::default()
        .with(otel_layer)
        .try_init()
        .map_err(|e| TelemetryError::Tracing(e.to_string()))?;

    tracing::info!(
        "OpenTelemetry tracing initialized with endpoint: {}",
        config.otlp_endpoint
    );

    Ok(())
}

/// Shutdown tracing (flush pending spans)
pub fn shutdown_tracing() {
    global::shutdown_tracer_provider();
}

/// Task tracing context
#[derive(Debug, Clone)]
pub struct TaskTraceContext {
    /// Trace ID
    pub trace_id: String,
    /// Span ID
    pub span_id: String,
    /// Parent span ID (if any)
    pub parent_span_id: Option<String>,
    /// Baggage items
    pub baggage: HashMap<String, String>,
}

impl TaskTraceContext {
    /// Create a new trace context
    pub fn new() -> Self {
        let tracer = global::tracer("turbine");
        let span = tracer.start("task_context");
        let cx = span.span_context();

        Self {
            trace_id: cx.trace_id().to_string(),
            span_id: cx.span_id().to_string(),
            parent_span_id: None,
            baggage: HashMap::new(),
        }
    }

    /// Create from existing IDs
    pub fn from_ids(trace_id: String, span_id: String, parent_span_id: Option<String>) -> Self {
        Self {
            trace_id,
            span_id,
            parent_span_id,
            baggage: HashMap::new(),
        }
    }

    /// Add baggage item
    pub fn with_baggage(mut self, key: &str, value: &str) -> Self {
        self.baggage.insert(key.to_string(), value.to_string());
        self
    }
}

impl Default for TaskTraceContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Task span builder for creating traced task execution
pub struct TaskSpanBuilder {
    task_name: String,
    task_id: String,
    queue: String,
    attributes: Vec<KeyValue>,
}

impl TaskSpanBuilder {
    /// Create a new task span builder
    pub fn new(task_name: &str, task_id: &str, queue: &str) -> Self {
        Self {
            task_name: task_name.to_string(),
            task_id: task_id.to_string(),
            queue: queue.to_string(),
            attributes: Vec::new(),
        }
    }

    /// Add custom attribute
    pub fn with_attribute(mut self, key: &str, value: &str) -> Self {
        self.attributes.push(KeyValue::new(key.to_owned(), value.to_owned()));
        self
    }

    /// Add retry count
    pub fn with_retry_count(mut self, count: u32) -> Self {
        self.attributes
            .push(KeyValue::new("task.retry_count", count as i64));
        self
    }

    /// Build and start the span
    pub fn start(self) -> TaskSpan {
        let tracer = global::tracer("turbine");

        let mut attributes = vec![
            KeyValue::new("task.name", self.task_name.clone()),
            KeyValue::new("task.id", self.task_id.clone()),
            KeyValue::new("task.queue", self.queue.clone()),
            KeyValue::new("messaging.system", "turbine"),
            KeyValue::new("messaging.destination", self.queue.clone()),
            KeyValue::new("messaging.operation", "process"),
        ];
        attributes.extend(self.attributes);

        let span = tracer
            .span_builder(format!("task/{}", self.task_name))
            .with_kind(SpanKind::Consumer)
            .with_attributes(attributes)
            .start(&tracer);

        TaskSpan {
            span,
            task_name: self.task_name,
            task_id: self.task_id,
        }
    }
}

/// Active task span
pub struct TaskSpan {
    span: opentelemetry::global::BoxedSpan,
    task_name: String,
    task_id: String,
}

impl TaskSpan {
    /// Add an event to the span
    pub fn add_event(&mut self, name: &str, attributes: Vec<KeyValue>) {
        self.span.add_event(name.to_string(), attributes);
    }

    /// Record task success
    pub fn success(mut self) {
        self.span.add_event(
            "task.completed",
            vec![KeyValue::new("task.status", "success")],
        );
        self.span.set_status(Status::Ok);
        self.span.end();
    }

    /// Record task failure
    pub fn failure(mut self, error: &str) {
        self.span.add_event(
            "task.failed",
            vec![
                KeyValue::new("task.status", "failure"),
                KeyValue::new("error.message", error.to_string()),
            ],
        );
        self.span.set_status(Status::error(error.to_string()));
        self.span.end();
    }

    /// Record task retry
    pub fn retry(mut self, retry_count: u32, error: &str) {
        self.span.add_event(
            "task.retry",
            vec![
                KeyValue::new("task.retry_count", retry_count as i64),
                KeyValue::new("error.message", error.to_string()),
            ],
        );
        self.span.end();
    }
}

/// Create a span for publishing a task
pub fn span_publish_task(queue: &str, task_name: &str, task_id: &str) -> opentelemetry::global::BoxedSpan {
    let tracer = global::tracer("turbine");

    tracer
        .span_builder(format!("publish/{}", task_name))
        .with_kind(SpanKind::Producer)
        .with_attributes(vec![
            KeyValue::new("task.name", task_name.to_owned()),
            KeyValue::new("task.id", task_id.to_owned()),
            KeyValue::new("task.queue", queue.to_owned()),
            KeyValue::new("messaging.system", "turbine"),
            KeyValue::new("messaging.destination", queue.to_owned()),
            KeyValue::new("messaging.operation", "publish"),
        ])
        .start(&tracer)
}

/// Create a span for workflow execution
pub fn span_workflow(workflow_type: &str, workflow_id: &str) -> opentelemetry::global::BoxedSpan {
    let tracer = global::tracer("turbine");

    tracer
        .span_builder(format!("workflow/{}", workflow_type))
        .with_kind(SpanKind::Internal)
        .with_attributes(vec![
            KeyValue::new("workflow.type", workflow_type.to_owned()),
            KeyValue::new("workflow.id", workflow_id.to_owned()),
        ])
        .start(&tracer)
}
