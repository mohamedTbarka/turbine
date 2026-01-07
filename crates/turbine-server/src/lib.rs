//! Turbine Server - gRPC API and task coordination
//!
//! This crate provides the central server for Turbine:
//! - gRPC API for task submission
//! - Task routing and scheduling
//! - Workflow orchestration

pub mod api;
pub mod generated;
pub mod scheduler;
pub mod state;
pub mod workflow;

pub use api::grpc::TurbineServiceImpl;
pub use generated::turbine as proto;
pub use scheduler::{ScheduleEntry, Scheduler, SchedulerConfig};
pub use state::ServerState;
pub use workflow::{WorkflowEngine, WorkflowOptions, WorkflowState, WorkflowType};
