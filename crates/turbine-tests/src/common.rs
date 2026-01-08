//! Common test utilities and fixtures

use std::sync::Once;

static INIT: Once = Once::new();

/// Initialize test environment (logging, etc.)
pub fn init() {
    INIT.call_once(|| {
        // Set up test logging
        let _ = tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter("turbine=debug")
            .try_init();
    });
}

/// Check if Redis is available for integration tests
pub async fn redis_available() -> bool {
    use std::time::Duration;

    match tokio::time::timeout(
        Duration::from_secs(1),
        tokio::net::TcpStream::connect("localhost:6379")
    ).await {
        Ok(Ok(_)) => true,
        _ => false,
    }
}

/// Generate a unique test queue name
pub fn unique_queue(prefix: &str) -> String {
    format!("{}_{}", prefix, &uuid::Uuid::new_v4().to_string().replace("-", "")[..8])
}

/// Generate a unique task ID
pub fn unique_task_id() -> String {
    format!("test_task_{}", uuid::Uuid::new_v4())
}
