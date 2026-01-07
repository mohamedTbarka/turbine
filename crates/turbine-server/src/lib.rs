//! Turbine Server - gRPC API and task coordination
//!
//! This crate provides the central server for Turbine:
//! - gRPC API for task submission
//! - Task routing and scheduling
//! - Workflow orchestration
//! - Rate limiting
//! - Priority queues
//! - Multi-tenancy

pub mod api;
pub mod generated;
pub mod multitenancy;
pub mod ratelimit;
pub mod router;
pub mod scheduler;
pub mod state;
pub mod workflow;

pub use api::grpc::TurbineServiceImpl;
pub use generated::turbine as proto;
pub use multitenancy::{Tenant, TenantId, TenantManager, TenantContext, MultiTenancyConfig};
pub use ratelimit::{RateLimit, RateLimitConfig, RateLimiter, RateLimitResult};
pub use router::{RoutingRule, MatchCondition, RoutingAction, RouterConfig, TaskRouter, RoutingDecision};
pub use scheduler::{ScheduleEntry, Scheduler, SchedulerConfig};
pub use state::ServerState;
pub use workflow::{WorkflowEngine, WorkflowOptions, WorkflowState, WorkflowType};
