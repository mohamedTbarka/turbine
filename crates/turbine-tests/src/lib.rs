//! Turbine Integration Tests
//!
//! This crate contains integration tests that require external services.
//! Run with: `cargo test -p turbine-tests`
//!
//! For tests that require Redis:
//! ```sh
//! docker run -d -p 6379:6379 redis:7-alpine
//! cargo test -p turbine-tests -- --ignored
//! ```

pub mod common;
