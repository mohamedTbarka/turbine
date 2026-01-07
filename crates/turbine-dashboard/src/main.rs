//! Turbine Dashboard - Standalone web server
//!
//! Run with: cargo run -p turbine-dashboard -- --help

use clap::Parser;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use turbine_dashboard::{DashboardApi, state::DashboardConfig};

/// Turbine Dashboard - Web UI for task queue monitoring
#[derive(Parser, Debug)]
#[command(name = "turbine-dashboard")]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Host to bind to
    #[arg(short = 'H', long, default_value = "0.0.0.0", env = "TURBINE_DASHBOARD_HOST")]
    host: String,

    /// Port to listen on
    #[arg(short, long, default_value = "8080", env = "TURBINE_DASHBOARD_PORT")]
    port: u16,

    /// Redis URL for broker connection
    #[arg(short, long, default_value = "redis://localhost:6379", env = "TURBINE_REDIS_URL")]
    redis_url: String,

    /// Enable CORS
    #[arg(long, default_value = "true", env = "TURBINE_DASHBOARD_CORS")]
    cors: bool,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info", env = "TURBINE_LOG_LEVEL")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Initialize logging
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new(&args.log_level)
        }))
        .with(fmt::layer())
        .init();

    // Build configuration
    let config = DashboardConfig {
        host: args.host,
        port: args.port,
        redis_url: args.redis_url,
        enable_cors: args.cors,
        ..Default::default()
    };

    // Print startup banner
    println!(
        r#"
╔════════════════════════════════════════════════════════════════╗
║                     TURBINE DASHBOARD                          ║
║                    Task Queue Monitoring                       ║
╠════════════════════════════════════════════════════════════════╣
║  Version: {}                                               ║
║  Host:    {}:{}                                       ║
╚════════════════════════════════════════════════════════════════╝
"#,
        env!("CARGO_PKG_VERSION"),
        config.host,
        config.port
    );

    // Create and run dashboard
    let dashboard = DashboardApi::new(config).await?;

    // Print available endpoints
    println!("Available API endpoints:");
    for endpoint in turbine_dashboard::api::api_endpoints() {
        println!("  {} {} - {}", endpoint.method, endpoint.path, endpoint.description);
    }
    println!();

    // Run server
    dashboard.run().await?;

    Ok(())
}
