//! Turbine Server - gRPC API and task coordination
//!
//! This crate provides the central server for Turbine:
//! - gRPC API for task submission
//! - Task routing and scheduling
//! - Workflow orchestration

pub mod api;
pub mod generated;
pub mod state;

pub use api::grpc::TurbineServiceImpl;
pub use generated::turbine as proto;
pub use state::ServerState;
