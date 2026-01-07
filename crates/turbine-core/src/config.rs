//! Configuration structures for Turbine

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Main configuration for Turbine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurbineConfig {
    /// Broker configuration
    #[serde(default)]
    pub broker: BrokerConfig,

    /// Result backend configuration
    #[serde(default)]
    pub backend: BackendConfig,

    /// Worker configuration
    #[serde(default)]
    pub worker: WorkerConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Queue configurations
    #[serde(default)]
    pub queues: HashMap<String, QueueConfig>,

    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// Telemetry configuration
    #[serde(default)]
    pub telemetry: TelemetryConfig,
}

impl Default for TurbineConfig {
    fn default() -> Self {
        Self {
            broker: BrokerConfig::default(),
            backend: BackendConfig::default(),
            worker: WorkerConfig::default(),
            server: ServerConfig::default(),
            queues: HashMap::new(),
            logging: LoggingConfig::default(),
            telemetry: TelemetryConfig::default(),
        }
    }
}

impl TurbineConfig {
    /// Load configuration from a TOML file
    pub fn from_file(path: impl AsRef<Path>) -> crate::Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| crate::Error::Configuration(format!("Failed to read config file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| crate::Error::Configuration(format!("Failed to parse config: {}", e)))
    }

    /// Load configuration from environment variables
    pub fn from_env() -> crate::Result<Self> {
        let mut config = Self::default();

        // Broker
        if let Ok(url) = std::env::var("TURBINE_BROKER_URL") {
            config.broker.url = url;
        }
        if let Ok(broker_type) = std::env::var("TURBINE_BROKER_TYPE") {
            config.broker.broker_type = broker_type.parse().unwrap_or_default();
        }

        // Backend
        if let Ok(url) = std::env::var("TURBINE_BACKEND_URL") {
            config.backend.url = Some(url);
        }

        // Server
        if let Ok(host) = std::env::var("TURBINE_SERVER_HOST") {
            config.server.host = host;
        }
        if let Ok(port) = std::env::var("TURBINE_SERVER_PORT") {
            config.server.grpc_port = port.parse().unwrap_or(50051);
        }

        // Worker
        if let Ok(concurrency) = std::env::var("TURBINE_WORKER_CONCURRENCY") {
            config.worker.concurrency = concurrency.parse().unwrap_or(4);
        }

        Ok(config)
    }

    /// Merge configuration from file and environment
    pub fn load(path: Option<impl AsRef<Path>>) -> crate::Result<Self> {
        let mut config = match path {
            Some(p) => Self::from_file(p)?,
            None => Self::default(),
        };

        // Override with environment variables
        let env_config = Self::from_env()?;

        // Simple merge - env vars take precedence
        if std::env::var("TURBINE_BROKER_URL").is_ok() {
            config.broker.url = env_config.broker.url;
        }
        if std::env::var("TURBINE_BACKEND_URL").is_ok() {
            config.backend.url = env_config.backend.url;
        }
        if std::env::var("TURBINE_SERVER_HOST").is_ok() {
            config.server.host = env_config.server.host;
        }
        if std::env::var("TURBINE_SERVER_PORT").is_ok() {
            config.server.grpc_port = env_config.server.grpc_port;
        }
        if std::env::var("TURBINE_WORKER_CONCURRENCY").is_ok() {
            config.worker.concurrency = env_config.worker.concurrency;
        }

        Ok(config)
    }
}

/// Broker type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrokerType {
    #[default]
    Redis,
    RabbitMQ,
    Sqs,
}

impl std::str::FromStr for BrokerType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "redis" => Ok(BrokerType::Redis),
            "rabbitmq" | "amqp" => Ok(BrokerType::RabbitMQ),
            "sqs" => Ok(BrokerType::Sqs),
            _ => Err(format!("Unknown broker type: {}", s)),
        }
    }
}

/// Broker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerConfig {
    /// Broker type
    #[serde(default)]
    pub broker_type: BrokerType,

    /// Connection URL
    #[serde(default = "default_broker_url")]
    pub url: String,

    /// Connection pool size
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,

    /// Connection timeout in seconds
    #[serde(default = "default_connection_timeout")]
    pub connection_timeout: u64,

    /// Visibility timeout for messages in seconds
    #[serde(default = "default_visibility_timeout")]
    pub visibility_timeout: u64,

    /// Prefetch count (how many messages to fetch at once)
    #[serde(default = "default_prefetch")]
    pub prefetch: u32,

    /// Enable TLS
    #[serde(default)]
    pub tls: bool,
}

fn default_broker_url() -> String {
    "redis://localhost:6379".to_string()
}

fn default_pool_size() -> usize {
    10
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_visibility_timeout() -> u64 {
    1800 // 30 minutes
}

fn default_prefetch() -> u32 {
    4
}

impl Default for BrokerConfig {
    fn default() -> Self {
        Self {
            broker_type: BrokerType::default(),
            url: default_broker_url(),
            pool_size: default_pool_size(),
            connection_timeout: default_connection_timeout(),
            visibility_timeout: default_visibility_timeout(),
            prefetch: default_prefetch(),
            tls: false,
        }
    }
}

/// Backend type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackendType {
    #[default]
    Redis,
    Postgres,
    S3,
    None,
}

/// Result backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Backend type
    #[serde(default)]
    pub backend_type: BackendType,

    /// Connection URL (uses broker URL if not specified)
    pub url: Option<String>,

    /// Default result TTL in seconds
    #[serde(default = "default_result_ttl")]
    pub result_ttl: u64,

    /// Maximum result size in bytes before offloading to S3
    #[serde(default = "default_max_result_size")]
    pub max_result_size: usize,
}

fn default_result_ttl() -> u64 {
    86400 // 24 hours
}

fn default_max_result_size() -> usize {
    1024 * 1024 // 1MB
}

impl Default for BackendConfig {
    fn default() -> Self {
        Self {
            backend_type: BackendType::default(),
            url: None,
            result_ttl: default_result_ttl(),
            max_result_size: default_max_result_size(),
        }
    }
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerConfig {
    /// Worker ID (auto-generated if not specified)
    pub id: Option<String>,

    /// Hostname
    pub hostname: Option<String>,

    /// Number of concurrent task slots
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,

    /// Queues to consume from (with optional priorities)
    #[serde(default = "default_worker_queues")]
    pub queues: Vec<String>,

    /// Heartbeat interval in seconds
    #[serde(default = "default_heartbeat_interval")]
    pub heartbeat_interval: u64,

    /// Maximum tasks before worker restarts (0 = unlimited)
    #[serde(default)]
    pub max_tasks_per_worker: u64,

    /// Maximum memory before worker restarts in MB (0 = unlimited)
    #[serde(default)]
    pub max_memory_mb: u64,

    /// Task time limit in seconds (hard limit)
    #[serde(default = "default_task_time_limit")]
    pub task_time_limit: u64,

    /// Soft time limit in seconds (sends warning)
    pub task_soft_time_limit: Option<u64>,

    /// Enable task subprocess isolation
    #[serde(default = "default_true")]
    pub subprocess_isolation: bool,

    /// Graceful shutdown timeout in seconds
    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout: u64,
}

fn default_concurrency() -> usize {
    std::thread::available_parallelism()
        .map(|p| p.get())
        .unwrap_or(4)
}

fn default_worker_queues() -> Vec<String> {
    vec!["default".to_string()]
}

fn default_heartbeat_interval() -> u64 {
    30
}

fn default_task_time_limit() -> u64 {
    300
}

fn default_true() -> bool {
    true
}

fn default_shutdown_timeout() -> u64 {
    60
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            id: None,
            hostname: None,
            concurrency: default_concurrency(),
            queues: default_worker_queues(),
            heartbeat_interval: default_heartbeat_interval(),
            max_tasks_per_worker: 0,
            max_memory_mb: 0,
            task_time_limit: default_task_time_limit(),
            task_soft_time_limit: None,
            subprocess_isolation: default_true(),
            shutdown_timeout: default_shutdown_timeout(),
        }
    }
}

impl WorkerConfig {
    /// Get the worker ID, generating one if not set
    pub fn get_id(&self) -> String {
        self.id.clone().unwrap_or_else(|| {
            let hostname = self.get_hostname();
            let pid = std::process::id();
            format!("{}@{}", hostname, pid)
        })
    }

    /// Get the hostname
    pub fn get_hostname(&self) -> String {
        self.hostname.clone().unwrap_or_else(|| {
            std::env::var("HOSTNAME")
                .or_else(|_| std::env::var("HOST"))
                .unwrap_or_else(|_| "unknown".to_string())
        })
    }
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Host to bind to
    #[serde(default = "default_host")]
    pub host: String,

    /// gRPC port
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,

    /// REST API port
    #[serde(default = "default_rest_port")]
    pub rest_port: u16,

    /// Dashboard port
    #[serde(default = "default_dashboard_port")]
    pub dashboard_port: u16,

    /// Enable REST API
    #[serde(default = "default_true")]
    pub enable_rest: bool,

    /// Enable dashboard
    #[serde(default = "default_true")]
    pub enable_dashboard: bool,

    /// Maximum request size in bytes
    #[serde(default = "default_max_request_size")]
    pub max_request_size: usize,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_grpc_port() -> u16 {
    50051
}

fn default_rest_port() -> u16 {
    8080
}

fn default_dashboard_port() -> u16 {
    8081
}

fn default_max_request_size() -> usize {
    16 * 1024 * 1024 // 16MB
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            grpc_port: default_grpc_port(),
            rest_port: default_rest_port(),
            dashboard_port: default_dashboard_port(),
            enable_rest: default_true(),
            enable_dashboard: default_true(),
            max_request_size: default_max_request_size(),
        }
    }
}

impl ServerConfig {
    /// Get the gRPC address
    pub fn grpc_addr(&self) -> String {
        format!("{}:{}", self.host, self.grpc_port)
    }

    /// Get the REST address
    pub fn rest_addr(&self) -> String {
        format!("{}:{}", self.host, self.rest_port)
    }

    /// Get the dashboard address
    pub fn dashboard_addr(&self) -> String {
        format!("{}:{}", self.host, self.dashboard_port)
    }
}

/// Queue-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Queue priority (higher = more important)
    #[serde(default)]
    pub priority: u8,

    /// Rate limit (tasks per second, 0 = unlimited)
    #[serde(default)]
    pub rate_limit: u32,

    /// Maximum concurrent tasks for this queue
    pub max_concurrency: Option<usize>,

    /// Default task timeout for this queue
    pub default_timeout: Option<u64>,

    /// Default max retries for this queue
    pub default_max_retries: Option<u32>,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            priority: 0,
            rate_limit: 0,
            max_concurrency: None,
            default_timeout: None,
            default_max_retries: None,
        }
    }
}

/// Logging configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Log level
    #[serde(default = "default_log_level")]
    pub level: String,

    /// Log format (json or text)
    #[serde(default = "default_log_format")]
    pub format: String,

    /// Include timestamps
    #[serde(default = "default_true")]
    pub timestamps: bool,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "text".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: default_log_level(),
            format: default_log_format(),
            timestamps: default_true(),
        }
    }
}

/// Telemetry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    /// Enable Prometheus metrics
    #[serde(default = "default_true")]
    pub prometheus_enabled: bool,

    /// Prometheus metrics port
    #[serde(default = "default_prometheus_port")]
    pub prometheus_port: u16,

    /// Enable OpenTelemetry tracing
    #[serde(default)]
    pub otel_enabled: bool,

    /// OpenTelemetry endpoint
    pub otel_endpoint: Option<String>,

    /// Service name for tracing
    #[serde(default = "default_service_name")]
    pub service_name: String,
}

fn default_prometheus_port() -> u16 {
    9090
}

fn default_service_name() -> String {
    "turbine".to_string()
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            prometheus_enabled: default_true(),
            prometheus_port: default_prometheus_port(),
            otel_enabled: false,
            otel_endpoint: None,
            service_name: default_service_name(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TurbineConfig::default();
        assert_eq!(config.broker.broker_type, BrokerType::Redis);
        assert_eq!(config.server.grpc_port, 50051);
    }

    #[test]
    fn test_worker_id_generation() {
        let config = WorkerConfig::default();
        let id = config.get_id();
        assert!(id.contains("@"));
    }

    #[test]
    fn test_server_addresses() {
        let config = ServerConfig::default();
        assert_eq!(config.grpc_addr(), "0.0.0.0:50051");
        assert_eq!(config.rest_addr(), "0.0.0.0:8080");
    }
}
