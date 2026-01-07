//! Turbine Telemetry - Observability layer
//!
//! This crate provides comprehensive observability for Turbine:
//! - Prometheus metrics for monitoring
//! - OpenTelemetry tracing for distributed tracing
//! - Structured JSON logging
//!
//! Enable features via feature flags:
//! - `metrics` (default) - Prometheus metrics
//! - `tracing` (default) - OpenTelemetry tracing
//! - `logging` (default) - Structured logging

#[cfg(feature = "metrics")]
pub mod metrics;

#[cfg(feature = "tracing")]
pub mod otel;

pub mod logging;

// Re-exports
#[cfg(feature = "metrics")]
pub use metrics::{MetricsConfig, TurbineMetrics};

#[cfg(feature = "tracing")]
pub use otel::{init_tracing, TracingConfig};

pub use logging::{init_logging, LoggingConfig};

use thiserror::Error;

/// Telemetry errors
#[derive(Error, Debug)]
pub enum TelemetryError {
    #[error("Metrics error: {0}")]
    Metrics(String),

    #[error("Tracing error: {0}")]
    Tracing(String),

    #[error("Logging error: {0}")]
    Logging(String),

    #[error("Configuration error: {0}")]
    Config(String),
}

pub type TelemetryResult<T> = Result<T, TelemetryError>;
