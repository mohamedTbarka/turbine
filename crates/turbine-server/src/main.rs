//! Turbine Server - Main entry point

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use tonic::transport::Server;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use turbine_core::TurbineConfig;
use turbine_server::proto::turbine_service_server::TurbineServiceServer;
use turbine_server::{ServerState, TurbineServiceImpl};

/// Turbine Server - High-performance distributed task queue
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long, env = "TURBINE_CONFIG")]
    config: Option<String>,

    /// gRPC server host
    #[arg(long, default_value = "0.0.0.0", env = "TURBINE_HOST")]
    host: String,

    /// gRPC server port
    #[arg(short, long, default_value = "50051", env = "TURBINE_PORT")]
    port: u16,

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
    config.server.host = args.host;
    config.server.grpc_port = args.port;

    info!("Starting Turbine Server v{}", env!("CARGO_PKG_VERSION"));
    info!("Broker: {}", config.broker.url);
    info!(
        "Backend: {}",
        config.backend.url.as_ref().unwrap_or(&config.broker.url)
    );

    // Initialize server state
    let state = ServerState::new(config.clone()).await?;
    let service = TurbineServiceImpl::new(state.clone());

    // Build gRPC server
    let addr: SocketAddr = config.server.grpc_addr().parse()?;

    info!("gRPC server listening on {}", addr);
    info!("Turbine Server ready");

    // Start tonic gRPC server
    Server::builder()
        .add_service(TurbineServiceServer::new(service))
        .serve(addr)
        .await?;

    Ok(())
}

