//! Turbine Worker - Main entry point

use anyhow::Result;
use clap::Parser;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use turbine_core::TurbineConfig;
use turbine_worker::executor::{EchoHandler, FailHandler, SleepHandler, TaskRegistry};
use turbine_worker::pool::WorkerPool;

/// Turbine Worker - High-performance task execution engine
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, env = "TURBINE_CONFIG")]
    config: Option<String>,

    /// Worker ID (auto-generated if not provided)
    #[arg(long, env = "TURBINE_WORKER_ID")]
    worker_id: Option<String>,

    /// Number of concurrent task slots
    #[arg(short = 'n', long, env = "TURBINE_CONCURRENCY")]
    concurrency: Option<usize>,

    /// Queues to consume from (comma-separated)
    #[arg(short, long, env = "TURBINE_QUEUES", value_delimiter = ',')]
    queues: Option<Vec<String>>,

    /// Broker URL
    #[arg(long, env = "TURBINE_BROKER_URL")]
    broker_url: Option<String>,

    /// Backend URL
    #[arg(long, env = "TURBINE_BACKEND_URL")]
    backend_url: Option<String>,

    /// Log level
    #[arg(long, default_value = "info", env = "TURBINE_LOG_LEVEL")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = match args.log_level.to_lowercase().as_str() {
        "trace" => Level::TRACE,
        "debug" => Level::DEBUG,
        "info" => Level::INFO,
        "warn" => Level::WARN,
        "error" => Level::ERROR,
        _ => Level::INFO,
    };

    let subscriber = FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .init();

    // Load configuration
    let mut config = match &args.config {
        Some(path) => {
            info!("Loading configuration from {}", path);
            TurbineConfig::from_file(path)?
        }
        None => {
            info!("Using default configuration");
            TurbineConfig::default()
        }
    };

    // Override with CLI args
    if let Some(broker_url) = args.broker_url {
        config.broker.url = broker_url;
    }
    if let Some(backend_url) = args.backend_url {
        config.backend.url = Some(backend_url);
    }
    if let Some(worker_id) = args.worker_id {
        config.worker.id = Some(worker_id);
    }
    if let Some(concurrency) = args.concurrency {
        config.worker.concurrency = concurrency;
    }
    if let Some(queues) = args.queues {
        config.worker.queues = queues;
    }

    info!("Starting Turbine Worker v{}", env!("CARGO_PKG_VERSION"));
    info!("Broker: {}", config.broker.url);
    info!(
        "Backend: {}",
        config.backend.url.as_ref().unwrap_or(&config.broker.url)
    );

    // Create task registry with built-in handlers
    let registry = Arc::new(TaskRegistry::new());

    // Register built-in handlers
    registry.register("echo", Arc::new(EchoHandler)).await;
    registry.register("sleep", Arc::new(SleepHandler)).await;
    registry.register("fail", Arc::new(FailHandler)).await;

    info!("Registered built-in task handlers: echo, sleep, fail");

    // Create and run worker pool
    let pool = Arc::new(WorkerPool::new(config, registry).await?);

    info!("Turbine Worker ready");

    // Run the worker
    pool.run().await?;

    Ok(())
}
