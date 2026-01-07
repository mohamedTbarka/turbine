//! Turbine Server - Main entry point

use anyhow::Result;
use clap::Parser;
use std::net::SocketAddr;
use std::sync::Arc;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;
use turbine_core::TurbineConfig;
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

    // For now, we'll create a simple TCP listener since we need the generated proto code
    // In production, this would use tonic's server builder

    // Simple health check endpoint using a TCP listener
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    info!("Turbine Server ready");
    info!("Health check: {} is listening", addr);

    // Simple request handler loop
    loop {
        let (socket, peer) = listener.accept().await?;
        let service = service.clone();

        tokio::spawn(async move {
            // For now, just handle basic connections
            // Full gRPC implementation requires generated code
            info!("Connection from {}", peer);

            let (mut reader, mut writer) = socket.into_split();

            // Read request
            let mut buf = vec![0u8; 4096];
            match tokio::io::AsyncReadExt::read(&mut reader, &mut buf).await {
                Ok(n) if n > 0 => {
                    let request = String::from_utf8_lossy(&buf[..n]);

                    // Simple HTTP health check
                    if request.contains("GET /health") {
                        let health = service.health_check().await;
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{}",
                            serde_json::json!({
                                "status": health.status,
                                "version": health.version,
                                "uptime": health.uptime,
                                "broker": health.broker_status,
                                "backend": health.backend_status
                            })
                        );
                        let _ = tokio::io::AsyncWriteExt::write_all(&mut writer, response.as_bytes()).await;
                    }
                }
                _ => {}
            }
        });
    }
}

