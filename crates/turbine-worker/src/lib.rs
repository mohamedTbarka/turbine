//! Turbine Worker - Task execution engine
//!
//! This crate provides the worker process for Turbine:
//! - Task consumption from broker
//! - Task execution with subprocess isolation
//! - Heartbeat reporting
//! - Graceful shutdown handling

pub mod executor;
pub mod pool;

pub use executor::{TaskExecutor, TaskHandler, ExecutionResult};
pub use pool::WorkerPool;
