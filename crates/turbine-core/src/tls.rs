//! TLS configuration for Turbine
//!
//! Provides TLS/SSL configuration for:
//! - gRPC server and client connections
//! - Broker connections (Redis TLS, RabbitMQ TLS)
//! - Dashboard HTTPS

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS
    pub enabled: bool,

    /// Path to CA certificate file (PEM format)
    /// Used for verifying server certificates
    pub ca_cert: Option<PathBuf>,

    /// Path to client/server certificate file (PEM format)
    pub cert: Option<PathBuf>,

    /// Path to private key file (PEM format)
    pub key: Option<PathBuf>,

    /// Skip certificate verification (INSECURE - only for development)
    #[serde(default)]
    pub skip_verify: bool,

    /// Server name for SNI (Server Name Indication)
    pub server_name: Option<String>,

    /// Minimum TLS version
    #[serde(default)]
    pub min_version: TlsVersion,

    /// Client authentication mode (for servers)
    #[serde(default)]
    pub client_auth: ClientAuthMode,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            ca_cert: None,
            cert: None,
            key: None,
            skip_verify: false,
            server_name: None,
            min_version: TlsVersion::Tls12,
            client_auth: ClientAuthMode::None,
        }
    }
}

impl TlsConfig {
    /// Create TLS config for a client connection
    pub fn client(ca_cert: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            ca_cert: Some(ca_cert.into()),
            ..Default::default()
        }
    }

    /// Create TLS config for a server
    pub fn server(cert: impl Into<PathBuf>, key: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            cert: Some(cert.into()),
            key: Some(key.into()),
            ..Default::default()
        }
    }

    /// Create TLS config with mutual TLS (mTLS)
    pub fn mtls(
        ca_cert: impl Into<PathBuf>,
        cert: impl Into<PathBuf>,
        key: impl Into<PathBuf>,
    ) -> Self {
        Self {
            enabled: true,
            ca_cert: Some(ca_cert.into()),
            cert: Some(cert.into()),
            key: Some(key.into()),
            client_auth: ClientAuthMode::Required,
            ..Default::default()
        }
    }

    /// Disable TLS (use insecure connection)
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }

    /// Check if mTLS is configured
    pub fn is_mtls(&self) -> bool {
        self.enabled
            && self.ca_cert.is_some()
            && self.cert.is_some()
            && self.key.is_some()
            && matches!(self.client_auth, ClientAuthMode::Required | ClientAuthMode::Optional)
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), TlsConfigError> {
        if !self.enabled {
            return Ok(());
        }

        // For client connections, we need either CA cert or skip_verify
        if self.ca_cert.is_none() && !self.skip_verify {
            // This is okay if we're using system CA store
        }

        // For server connections, we need cert and key
        if self.cert.is_some() != self.key.is_some() {
            return Err(TlsConfigError::MissingCertOrKey);
        }

        // Verify files exist
        if let Some(ref path) = self.ca_cert {
            if !path.exists() {
                return Err(TlsConfigError::FileNotFound(path.clone()));
            }
        }

        if let Some(ref path) = self.cert {
            if !path.exists() {
                return Err(TlsConfigError::FileNotFound(path.clone()));
            }
        }

        if let Some(ref path) = self.key {
            if !path.exists() {
                return Err(TlsConfigError::FileNotFound(path.clone()));
            }
        }

        Ok(())
    }
}

/// Minimum TLS version to accept
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TlsVersion {
    /// TLS 1.2
    #[default]
    Tls12,
    /// TLS 1.3
    Tls13,
}

impl std::fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TlsVersion::Tls12 => write!(f, "TLS 1.2"),
            TlsVersion::Tls13 => write!(f, "TLS 1.3"),
        }
    }
}

/// Client authentication mode for TLS servers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ClientAuthMode {
    /// No client authentication required
    #[default]
    None,
    /// Client authentication is optional
    Optional,
    /// Client authentication is required (mTLS)
    Required,
}

/// TLS configuration errors
#[derive(Debug, thiserror::Error)]
pub enum TlsConfigError {
    #[error("TLS requires both cert and key, or neither")]
    MissingCertOrKey,

    #[error("File not found: {0}")]
    FileNotFound(PathBuf),

    #[error("Invalid certificate: {0}")]
    InvalidCertificate(String),

    #[error("Invalid private key: {0}")]
    InvalidKey(String),

    #[error("TLS configuration error: {0}")]
    Configuration(String),
}

/// gRPC-specific TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GrpcTlsConfig {
    /// Base TLS configuration
    #[serde(flatten)]
    pub tls: TlsConfig,

    /// Enable ALPN for HTTP/2
    #[serde(default = "default_true")]
    pub alpn: bool,
}

fn default_true() -> bool {
    true
}

impl Default for GrpcTlsConfig {
    fn default() -> Self {
        Self {
            tls: TlsConfig::default(),
            alpn: true,
        }
    }
}

impl GrpcTlsConfig {
    /// Create from base TLS config
    pub fn from_tls(tls: TlsConfig) -> Self {
        Self { tls, alpn: true }
    }
}

/// Redis-specific TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisTlsConfig {
    /// Base TLS configuration
    #[serde(flatten)]
    pub tls: TlsConfig,

    /// Use STARTTLS (not recommended, prefer native TLS)
    #[serde(default)]
    pub starttls: bool,
}

impl Default for RedisTlsConfig {
    fn default() -> Self {
        Self {
            tls: TlsConfig::default(),
            starttls: false,
        }
    }
}

/// Build a TLS connection URL for Redis
impl RedisTlsConfig {
    /// Convert Redis URL to TLS URL if TLS is enabled
    pub fn apply_to_url(&self, url: &str) -> String {
        if !self.tls.enabled {
            return url.to_string();
        }

        // Convert redis:// to rediss://
        if url.starts_with("redis://") {
            format!("rediss://{}", &url[8..])
        } else {
            url.to_string()
        }
    }
}

/// RabbitMQ-specific TLS configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AmqpTlsConfig {
    /// Base TLS configuration
    #[serde(flatten)]
    pub tls: TlsConfig,

    /// Use EXTERNAL authentication (client certificate)
    #[serde(default)]
    pub external_auth: bool,
}

impl Default for AmqpTlsConfig {
    fn default() -> Self {
        Self {
            tls: TlsConfig::default(),
            external_auth: false,
        }
    }
}

impl AmqpTlsConfig {
    /// Convert AMQP URL to TLS URL if TLS is enabled
    pub fn apply_to_url(&self, url: &str) -> String {
        if !self.tls.enabled {
            return url.to_string();
        }

        // Convert amqp:// to amqps://
        if url.starts_with("amqp://") {
            format!("amqps://{}", &url[7..])
        } else {
            url.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = TlsConfig::default();
        assert!(!config.enabled);
        assert!(!config.skip_verify);
    }

    #[test]
    fn test_client_config() {
        let config = TlsConfig::client("/path/to/ca.pem");
        assert!(config.enabled);
        assert_eq!(config.ca_cert, Some(PathBuf::from("/path/to/ca.pem")));
    }

    #[test]
    fn test_server_config() {
        let config = TlsConfig::server("/path/to/cert.pem", "/path/to/key.pem");
        assert!(config.enabled);
        assert_eq!(config.cert, Some(PathBuf::from("/path/to/cert.pem")));
        assert_eq!(config.key, Some(PathBuf::from("/path/to/key.pem")));
    }

    #[test]
    fn test_mtls_config() {
        let config = TlsConfig::mtls(
            "/path/to/ca.pem",
            "/path/to/cert.pem",
            "/path/to/key.pem",
        );
        assert!(config.enabled);
        assert!(config.is_mtls());
    }

    #[test]
    fn test_redis_url_conversion() {
        let config = RedisTlsConfig {
            tls: TlsConfig { enabled: true, ..Default::default() },
            starttls: false,
        };

        assert_eq!(
            config.apply_to_url("redis://localhost:6379"),
            "rediss://localhost:6379"
        );
    }

    #[test]
    fn test_amqp_url_conversion() {
        let config = AmqpTlsConfig {
            tls: TlsConfig { enabled: true, ..Default::default() },
            external_auth: false,
        };

        assert_eq!(
            config.apply_to_url("amqp://localhost:5672"),
            "amqps://localhost:5672"
        );
    }

    #[test]
    fn test_validation_disabled() {
        let config = TlsConfig::disabled();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validation_missing_key() {
        let config = TlsConfig {
            enabled: true,
            cert: Some(PathBuf::from("/path/to/cert.pem")),
            key: None,
            ..Default::default()
        };
        assert!(matches!(config.validate(), Err(TlsConfigError::MissingCertOrKey)));
    }
}
