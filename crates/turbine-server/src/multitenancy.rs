//! Multi-tenancy support for Turbine
//!
//! Provides tenant isolation and management:
//! - Tenant-specific queue namespaces
//! - Per-tenant rate limits
//! - Per-tenant routing rules
//! - Tenant authentication
//! - Resource quotas

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::ratelimit::{RateLimit, RateLimitConfig};
use crate::router::RouterConfig;

/// Tenant identifier
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(String);

impl TenantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for TenantId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for TenantId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for TenantId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Multi-tenancy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultiTenancyConfig {
    /// Enable multi-tenancy
    pub enabled: bool,

    /// Isolation mode
    pub isolation_mode: IsolationMode,

    /// Default tenant for requests without tenant context
    pub default_tenant: Option<TenantId>,

    /// Queue namespace pattern (e.g., "{tenant}:{queue}")
    pub queue_namespace_pattern: String,

    /// Result key namespace pattern
    pub result_namespace_pattern: String,

    /// Allow creating tenants dynamically
    pub allow_dynamic_tenants: bool,

    /// Default quotas for new tenants
    pub default_quotas: ResourceQuotas,
}

impl Default for MultiTenancyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            isolation_mode: IsolationMode::Namespace,
            default_tenant: None,
            queue_namespace_pattern: "{tenant}:{queue}".to_string(),
            result_namespace_pattern: "{tenant}:result:{task_id}".to_string(),
            allow_dynamic_tenants: false,
            default_quotas: ResourceQuotas::default(),
        }
    }
}

/// Isolation mode for tenants
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum IsolationMode {
    /// Namespace isolation (prefix queues/keys with tenant ID)
    #[default]
    Namespace,

    /// Database isolation (separate Redis databases per tenant)
    Database,

    /// Full isolation (separate broker instances per tenant)
    Full,
}

/// Resource quotas for a tenant
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotas {
    /// Maximum tasks per second
    pub max_tasks_per_second: Option<f64>,

    /// Maximum concurrent tasks
    pub max_concurrent_tasks: Option<u32>,

    /// Maximum queues
    pub max_queues: Option<u32>,

    /// Maximum workers
    pub max_workers: Option<u32>,

    /// Maximum task payload size (bytes)
    pub max_task_size: Option<usize>,

    /// Maximum result TTL (seconds)
    pub max_result_ttl: Option<u64>,

    /// Maximum scheduled tasks
    pub max_scheduled_tasks: Option<u32>,

    /// Maximum workflows
    pub max_workflows: Option<u32>,
}

impl Default for ResourceQuotas {
    fn default() -> Self {
        Self {
            max_tasks_per_second: Some(100.0),
            max_concurrent_tasks: Some(1000),
            max_queues: Some(100),
            max_workers: Some(50),
            max_task_size: Some(1024 * 1024), // 1MB
            max_result_ttl: Some(86400 * 7),  // 7 days
            max_scheduled_tasks: Some(1000),
            max_workflows: Some(100),
        }
    }
}

impl ResourceQuotas {
    /// Unlimited quotas
    pub fn unlimited() -> Self {
        Self {
            max_tasks_per_second: None,
            max_concurrent_tasks: None,
            max_queues: None,
            max_workers: None,
            max_task_size: None,
            max_result_ttl: None,
            max_scheduled_tasks: None,
            max_workflows: None,
        }
    }
}

/// Tenant information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tenant {
    /// Tenant ID
    pub id: TenantId,

    /// Tenant name
    pub name: String,

    /// Tenant status
    pub status: TenantStatus,

    /// Resource quotas
    pub quotas: ResourceQuotas,

    /// API key for authentication
    pub api_key: Option<String>,

    /// Rate limit configuration override
    pub rate_limit_config: Option<RateLimitConfig>,

    /// Router configuration override
    pub router_config: Option<RouterConfig>,

    /// Custom metadata
    pub metadata: HashMap<String, String>,

    /// When the tenant was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// When the tenant was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Tenant {
    pub fn new(id: impl Into<TenantId>, name: impl Into<String>) -> Self {
        let now = chrono::Utc::now();
        Self {
            id: id.into(),
            name: name.into(),
            status: TenantStatus::Active,
            quotas: ResourceQuotas::default(),
            api_key: None,
            rate_limit_config: None,
            router_config: None,
            metadata: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }

    pub fn with_quotas(mut self, quotas: ResourceQuotas) -> Self {
        self.quotas = quotas;
        self
    }

    pub fn with_api_key(mut self, api_key: impl Into<String>) -> Self {
        self.api_key = Some(api_key.into());
        self
    }

    /// Generate a new API key for this tenant
    pub fn generate_api_key(&mut self) -> String {
        let key = format!("tk_{}_{}", self.id, Uuid::new_v4().to_string().replace("-", ""));
        self.api_key = Some(key.clone());
        self.updated_at = chrono::Utc::now();
        key
    }
}

/// Tenant status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TenantStatus {
    /// Tenant is active and can submit tasks
    #[default]
    Active,

    /// Tenant is suspended (no new tasks, existing tasks continue)
    Suspended,

    /// Tenant is disabled (no tasks allowed)
    Disabled,

    /// Tenant is being deleted
    Deleting,
}

/// Tenant usage statistics
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TenantUsage {
    /// Tasks submitted today
    pub tasks_submitted_today: u64,

    /// Tasks processed today
    pub tasks_processed_today: u64,

    /// Current concurrent tasks
    pub concurrent_tasks: u32,

    /// Number of active queues
    pub active_queues: u32,

    /// Number of active workers
    pub active_workers: u32,

    /// Storage used (bytes)
    pub storage_used: u64,

    /// Last activity timestamp
    pub last_activity: Option<chrono::DateTime<chrono::Utc>>,
}

/// Tenant context for request processing
#[derive(Debug, Clone)]
pub struct TenantContext {
    /// The tenant
    pub tenant: Tenant,

    /// Current usage
    pub usage: TenantUsage,
}

impl TenantContext {
    /// Get namespaced queue name
    pub fn namespace_queue(&self, queue: &str, pattern: &str) -> String {
        pattern
            .replace("{tenant}", self.tenant.id.as_str())
            .replace("{queue}", queue)
    }

    /// Get namespaced result key
    pub fn namespace_result(&self, task_id: &str, pattern: &str) -> String {
        pattern
            .replace("{tenant}", self.tenant.id.as_str())
            .replace("{task_id}", task_id)
    }

    /// Check if quota allows the operation
    pub fn check_quota(&self, quota_type: QuotaType) -> QuotaCheckResult {
        match quota_type {
            QuotaType::TasksPerSecond => {
                if let Some(limit) = self.tenant.quotas.max_tasks_per_second {
                    // This would be checked by rate limiter
                    QuotaCheckResult::Allowed
                } else {
                    QuotaCheckResult::Allowed
                }
            }

            QuotaType::ConcurrentTasks => {
                if let Some(limit) = self.tenant.quotas.max_concurrent_tasks {
                    if self.usage.concurrent_tasks >= limit {
                        return QuotaCheckResult::Exceeded {
                            quota: "concurrent_tasks".to_string(),
                            limit: limit as u64,
                            current: self.usage.concurrent_tasks as u64,
                        };
                    }
                }
                QuotaCheckResult::Allowed
            }

            QuotaType::Queues => {
                if let Some(limit) = self.tenant.quotas.max_queues {
                    if self.usage.active_queues >= limit {
                        return QuotaCheckResult::Exceeded {
                            quota: "queues".to_string(),
                            limit: limit as u64,
                            current: self.usage.active_queues as u64,
                        };
                    }
                }
                QuotaCheckResult::Allowed
            }

            QuotaType::Workers => {
                if let Some(limit) = self.tenant.quotas.max_workers {
                    if self.usage.active_workers >= limit {
                        return QuotaCheckResult::Exceeded {
                            quota: "workers".to_string(),
                            limit: limit as u64,
                            current: self.usage.active_workers as u64,
                        };
                    }
                }
                QuotaCheckResult::Allowed
            }

            QuotaType::TaskSize(size) => {
                if let Some(limit) = self.tenant.quotas.max_task_size {
                    if size > limit {
                        return QuotaCheckResult::Exceeded {
                            quota: "task_size".to_string(),
                            limit: limit as u64,
                            current: size as u64,
                        };
                    }
                }
                QuotaCheckResult::Allowed
            }
        }
    }
}

/// Type of quota to check
#[derive(Debug, Clone)]
pub enum QuotaType {
    TasksPerSecond,
    ConcurrentTasks,
    Queues,
    Workers,
    TaskSize(usize),
}

/// Result of quota check
#[derive(Debug, Clone)]
pub enum QuotaCheckResult {
    Allowed,
    Exceeded {
        quota: String,
        limit: u64,
        current: u64,
    },
}

/// Tenant manager
pub struct TenantManager {
    config: MultiTenancyConfig,

    /// All tenants
    tenants: RwLock<HashMap<TenantId, Tenant>>,

    /// API key to tenant mapping
    api_keys: RwLock<HashMap<String, TenantId>>,

    /// Tenant usage tracking
    usage: RwLock<HashMap<TenantId, TenantUsage>>,
}

impl TenantManager {
    /// Create a new tenant manager
    pub fn new(config: MultiTenancyConfig) -> Self {
        Self {
            config,
            tenants: RwLock::new(HashMap::new()),
            api_keys: RwLock::new(HashMap::new()),
            usage: RwLock::new(HashMap::new()),
        }
    }

    /// Check if multi-tenancy is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get tenant by ID
    pub async fn get_tenant(&self, id: &TenantId) -> Option<Tenant> {
        let tenants = self.tenants.read().await;
        tenants.get(id).cloned()
    }

    /// Get tenant by API key
    pub async fn get_tenant_by_api_key(&self, api_key: &str) -> Option<Tenant> {
        let api_keys = self.api_keys.read().await;
        if let Some(tenant_id) = api_keys.get(api_key) {
            let tenants = self.tenants.read().await;
            return tenants.get(tenant_id).cloned();
        }
        None
    }

    /// Create a new tenant
    pub async fn create_tenant(&self, tenant: Tenant) -> Result<Tenant, TenantError> {
        let mut tenants = self.tenants.write().await;

        if tenants.contains_key(&tenant.id) {
            return Err(TenantError::AlreadyExists(tenant.id.to_string()));
        }

        // Index API key if present
        if let Some(ref api_key) = tenant.api_key {
            let mut api_keys = self.api_keys.write().await;
            api_keys.insert(api_key.clone(), tenant.id.clone());
        }

        // Initialize usage
        let mut usage = self.usage.write().await;
        usage.insert(tenant.id.clone(), TenantUsage::default());

        info!("Created tenant: {} ({})", tenant.name, tenant.id);
        tenants.insert(tenant.id.clone(), tenant.clone());

        Ok(tenant)
    }

    /// Update a tenant
    pub async fn update_tenant(&self, tenant: Tenant) -> Result<Tenant, TenantError> {
        let mut tenants = self.tenants.write().await;

        if !tenants.contains_key(&tenant.id) {
            return Err(TenantError::NotFound(tenant.id.to_string()));
        }

        // Update API key index
        let old_tenant = tenants.get(&tenant.id);
        if let Some(old) = old_tenant {
            if old.api_key != tenant.api_key {
                let mut api_keys = self.api_keys.write().await;

                // Remove old key
                if let Some(ref old_key) = old.api_key {
                    api_keys.remove(old_key);
                }

                // Add new key
                if let Some(ref new_key) = tenant.api_key {
                    api_keys.insert(new_key.clone(), tenant.id.clone());
                }
            }
        }

        let mut updated = tenant.clone();
        updated.updated_at = chrono::Utc::now();
        tenants.insert(tenant.id.clone(), updated.clone());

        info!("Updated tenant: {} ({})", updated.name, updated.id);
        Ok(updated)
    }

    /// Delete a tenant
    pub async fn delete_tenant(&self, id: &TenantId) -> Result<(), TenantError> {
        let mut tenants = self.tenants.write().await;

        let tenant = tenants.remove(id)
            .ok_or_else(|| TenantError::NotFound(id.to_string()))?;

        // Remove API key
        if let Some(api_key) = tenant.api_key {
            let mut api_keys = self.api_keys.write().await;
            api_keys.remove(&api_key);
        }

        // Remove usage
        let mut usage = self.usage.write().await;
        usage.remove(id);

        info!("Deleted tenant: {} ({})", tenant.name, id);
        Ok(())
    }

    /// List all tenants
    pub async fn list_tenants(&self) -> Vec<Tenant> {
        let tenants = self.tenants.read().await;
        tenants.values().cloned().collect()
    }

    /// Get tenant context for a request
    pub async fn get_context(&self, id: &TenantId) -> Option<TenantContext> {
        let tenant = self.get_tenant(id).await?;
        let usage = {
            let usage_map = self.usage.read().await;
            usage_map.get(id).cloned().unwrap_or_default()
        };

        Some(TenantContext { tenant, usage })
    }

    /// Get context from API key
    pub async fn get_context_by_api_key(&self, api_key: &str) -> Option<TenantContext> {
        let tenant = self.get_tenant_by_api_key(api_key).await?;
        let usage = {
            let usage_map = self.usage.read().await;
            usage_map.get(&tenant.id).cloned().unwrap_or_default()
        };

        Some(TenantContext { tenant, usage })
    }

    /// Get default tenant context
    pub async fn get_default_context(&self) -> Option<TenantContext> {
        if let Some(ref default_id) = self.config.default_tenant {
            return self.get_context(default_id).await;
        }
        None
    }

    /// Update tenant usage
    pub async fn update_usage(&self, id: &TenantId, update: impl FnOnce(&mut TenantUsage)) {
        let mut usage_map = self.usage.write().await;
        if let Some(usage) = usage_map.get_mut(id) {
            update(usage);
            usage.last_activity = Some(chrono::Utc::now());
        }
    }

    /// Increment concurrent tasks
    pub async fn inc_concurrent_tasks(&self, id: &TenantId) {
        self.update_usage(id, |u| u.concurrent_tasks += 1).await;
    }

    /// Decrement concurrent tasks
    pub async fn dec_concurrent_tasks(&self, id: &TenantId) {
        self.update_usage(id, |u| {
            if u.concurrent_tasks > 0 {
                u.concurrent_tasks -= 1;
            }
        }).await;
    }

    /// Record task submission
    pub async fn record_task_submitted(&self, id: &TenantId) {
        self.update_usage(id, |u| u.tasks_submitted_today += 1).await;
    }

    /// Record task completion
    pub async fn record_task_completed(&self, id: &TenantId) {
        self.update_usage(id, |u| u.tasks_processed_today += 1).await;
    }

    /// Get namespaced queue name
    pub fn namespace_queue(&self, tenant_id: &TenantId, queue: &str) -> String {
        if !self.config.enabled {
            return queue.to_string();
        }

        self.config.queue_namespace_pattern
            .replace("{tenant}", tenant_id.as_str())
            .replace("{queue}", queue)
    }

    /// Get namespaced result key
    pub fn namespace_result(&self, tenant_id: &TenantId, task_id: &str) -> String {
        if !self.config.enabled {
            return task_id.to_string();
        }

        self.config.result_namespace_pattern
            .replace("{tenant}", tenant_id.as_str())
            .replace("{task_id}", task_id)
    }

    /// Extract tenant ID from namespaced queue
    pub fn extract_tenant_from_queue(&self, namespaced_queue: &str) -> Option<TenantId> {
        if !self.config.enabled {
            return None;
        }

        // Simple extraction based on pattern "{tenant}:{queue}"
        let parts: Vec<&str> = namespaced_queue.splitn(2, ':').collect();
        if parts.len() == 2 {
            Some(TenantId::new(parts[0]))
        } else {
            None
        }
    }
}

/// Tenant errors
#[derive(Debug, thiserror::Error)]
pub enum TenantError {
    #[error("Tenant already exists: {0}")]
    AlreadyExists(String),

    #[error("Tenant not found: {0}")]
    NotFound(String),

    #[error("Tenant suspended: {0}")]
    Suspended(String),

    #[error("Tenant disabled: {0}")]
    Disabled(String),

    #[error("Quota exceeded: {0}")]
    QuotaExceeded(String),

    #[error("Invalid API key")]
    InvalidApiKey,

    #[error("Internal error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_tenant() {
        let config = MultiTenancyConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = TenantManager::new(config);

        let tenant = Tenant::new("test-tenant", "Test Tenant");
        let created = manager.create_tenant(tenant).await.unwrap();

        assert_eq!(created.name, "Test Tenant");
        assert!(matches!(created.status, TenantStatus::Active));
    }

    #[tokio::test]
    async fn test_tenant_by_api_key() {
        let config = MultiTenancyConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = TenantManager::new(config);

        let mut tenant = Tenant::new("test-tenant", "Test Tenant");
        let api_key = tenant.generate_api_key();
        manager.create_tenant(tenant).await.unwrap();

        let found = manager.get_tenant_by_api_key(&api_key).await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id.as_str(), "test-tenant");
    }

    #[tokio::test]
    async fn test_queue_namespacing() {
        let config = MultiTenancyConfig {
            enabled: true,
            queue_namespace_pattern: "{tenant}:{queue}".to_string(),
            ..Default::default()
        };
        let manager = TenantManager::new(config);

        let tenant_id = TenantId::new("acme");
        let namespaced = manager.namespace_queue(&tenant_id, "emails");
        assert_eq!(namespaced, "acme:emails");
    }

    #[tokio::test]
    async fn test_quota_check() {
        let tenant = Tenant::new("test", "Test")
            .with_quotas(ResourceQuotas {
                max_concurrent_tasks: Some(10),
                ..Default::default()
            });

        let usage = TenantUsage {
            concurrent_tasks: 5,
            ..Default::default()
        };

        let ctx = TenantContext { tenant, usage };

        // Should be allowed
        let result = ctx.check_quota(QuotaType::ConcurrentTasks);
        assert!(matches!(result, QuotaCheckResult::Allowed));

        // At limit
        let mut ctx2 = ctx.clone();
        ctx2.usage.concurrent_tasks = 10;
        let result = ctx2.check_quota(QuotaType::ConcurrentTasks);
        assert!(matches!(result, QuotaCheckResult::Exceeded { .. }));
    }
}
