//! Turbine Dashboard - Web UI for task queue monitoring
//!
//! This crate provides a built-in web dashboard for Turbine:
//! - Real-time task monitoring
//! - Worker status and health
//! - Queue inspection and management
//! - Task retry/revoke operations
//! - Metrics visualization

pub mod api;
pub mod handlers;
pub mod state;
pub mod sse;

pub use api::DashboardApi;
pub use state::DashboardState;

use thiserror::Error;

/// Dashboard errors
#[derive(Error, Debug)]
pub enum DashboardError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Broker error: {0}")]
    Broker(String),

    #[error("Backend error: {0}")]
    Backend(String),
}

pub type DashboardResult<T> = Result<T, DashboardError>;
