//! Rate limiting for Turbine
//!
//! Provides configurable rate limiting for:
//! - Per-task rate limits (e.g., max 10 email tasks per second)
//! - Per-queue rate limits (e.g., max 100 tasks per second on "default" queue)
//! - Per-worker rate limits
//! - Global rate limits
//!
//! Uses token bucket algorithm for smooth rate limiting.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, warn};

/// Rate limiter configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Enable rate limiting
    pub enabled: bool,

    /// Default rate limit (requests per second)
    pub default_rate: f64,

    /// Default burst size
    pub default_burst: u32,

    /// Per-task rate limits
    pub task_limits: HashMap<String, RateLimit>,

    /// Per-queue rate limits
    pub queue_limits: HashMap<String, RateLimit>,

    /// Global rate limit (0 = unlimited)
    pub global_limit: Option<RateLimit>,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_rate: 100.0,  // 100 tasks per second default
            default_burst: 50,
            task_limits: HashMap::new(),
            queue_limits: HashMap::new(),
            global_limit: None,
        }
    }
}

/// Rate limit specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Requests per second
    pub rate: f64,

    /// Burst size (max tokens)
    pub burst: u32,
}

impl RateLimit {
    pub fn new(rate: f64, burst: u32) -> Self {
        Self { rate, burst }
    }

    /// Create a rate limit for N requests per time period
    pub fn per_second(n: u32) -> Self {
        Self { rate: n as f64, burst: n }
    }

    pub fn per_minute(n: u32) -> Self {
        Self { rate: n as f64 / 60.0, burst: n.max(1) }
    }

    pub fn per_hour(n: u32) -> Self {
        Self { rate: n as f64 / 3600.0, burst: (n / 60).max(1) }
    }
}

/// Token bucket for rate limiting
#[derive(Debug)]
struct TokenBucket {
    /// Current number of tokens
    tokens: f64,

    /// Maximum tokens (burst size)
    max_tokens: f64,

    /// Tokens added per second
    refill_rate: f64,

    /// Last refill time
    last_refill: Instant,
}

impl TokenBucket {
    fn new(rate: f64, burst: u32) -> Self {
        Self {
            tokens: burst as f64,
            max_tokens: burst as f64,
            refill_rate: rate,
            last_refill: Instant::now(),
        }
    }

    /// Try to acquire a token, returns true if successful
    fn try_acquire(&mut self) -> bool {
        self.refill();

        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }

    /// Check if a token is available without consuming it
    fn is_available(&mut self) -> bool {
        self.refill();
        self.tokens >= 1.0
    }

    /// Get time until next token is available
    fn time_until_available(&mut self) -> Duration {
        self.refill();

        if self.tokens >= 1.0 {
            Duration::ZERO
        } else {
            let tokens_needed = 1.0 - self.tokens;
            let seconds = tokens_needed / self.refill_rate;
            Duration::from_secs_f64(seconds)
        }
    }

    /// Refill tokens based on elapsed time
    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let new_tokens = elapsed * self.refill_rate;

        self.tokens = (self.tokens + new_tokens).min(self.max_tokens);
        self.last_refill = now;
    }

    /// Get current token count
    fn available_tokens(&mut self) -> f64 {
        self.refill();
        self.tokens
    }
}

/// Rate limiter result
#[derive(Debug, Clone)]
pub enum RateLimitResult {
    /// Request is allowed
    Allowed,

    /// Request is rate limited, includes wait time
    Limited {
        /// Time to wait before retrying
        retry_after: Duration,
        /// Which limit was hit
        limit_type: LimitType,
    },
}

/// Type of rate limit that was hit
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LimitType {
    Global,
    Queue,
    Task,
}

impl std::fmt::Display for LimitType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LimitType::Global => write!(f, "global"),
            LimitType::Queue => write!(f, "queue"),
            LimitType::Task => write!(f, "task"),
        }
    }
}

/// Rate limiter for Turbine
pub struct RateLimiter {
    config: RateLimitConfig,

    /// Global rate limit bucket
    global_bucket: Option<RwLock<TokenBucket>>,

    /// Per-queue buckets
    queue_buckets: RwLock<HashMap<String, TokenBucket>>,

    /// Per-task buckets
    task_buckets: RwLock<HashMap<String, TokenBucket>>,
}

impl RateLimiter {
    /// Create a new rate limiter with configuration
    pub fn new(config: RateLimitConfig) -> Self {
        let global_bucket = config.global_limit.as_ref().map(|limit| {
            RwLock::new(TokenBucket::new(limit.rate, limit.burst))
        });

        Self {
            config,
            global_bucket,
            queue_buckets: RwLock::new(HashMap::new()),
            task_buckets: RwLock::new(HashMap::new()),
        }
    }

    /// Check if a task submission is allowed
    pub async fn check(&self, task_name: &str, queue: &str) -> RateLimitResult {
        if !self.config.enabled {
            return RateLimitResult::Allowed;
        }

        // Check global limit first
        if let Some(ref bucket) = self.global_bucket {
            let mut bucket = bucket.write().await;
            if !bucket.try_acquire() {
                let retry_after = bucket.time_until_available();
                debug!(
                    "Global rate limit hit, retry after {:?}",
                    retry_after
                );
                return RateLimitResult::Limited {
                    retry_after,
                    limit_type: LimitType::Global,
                };
            }
        }

        // Check queue limit
        let queue_result = self.check_queue_limit(queue).await;
        if let RateLimitResult::Limited { .. } = queue_result {
            return queue_result;
        }

        // Check task limit
        let task_result = self.check_task_limit(task_name).await;
        if let RateLimitResult::Limited { .. } = task_result {
            return task_result;
        }

        RateLimitResult::Allowed
    }

    /// Check queue-specific rate limit
    async fn check_queue_limit(&self, queue: &str) -> RateLimitResult {
        let limit = self.config.queue_limits.get(queue).cloned().unwrap_or_else(|| {
            RateLimit::new(self.config.default_rate, self.config.default_burst)
        });

        let mut buckets = self.queue_buckets.write().await;
        let bucket = buckets
            .entry(queue.to_string())
            .or_insert_with(|| TokenBucket::new(limit.rate, limit.burst));

        if bucket.try_acquire() {
            RateLimitResult::Allowed
        } else {
            let retry_after = bucket.time_until_available();
            debug!(
                "Queue '{}' rate limit hit, retry after {:?}",
                queue, retry_after
            );
            RateLimitResult::Limited {
                retry_after,
                limit_type: LimitType::Queue,
            }
        }
    }

    /// Check task-specific rate limit
    async fn check_task_limit(&self, task_name: &str) -> RateLimitResult {
        // Only apply task limits if explicitly configured
        let limit = match self.config.task_limits.get(task_name) {
            Some(limit) => limit.clone(),
            None => return RateLimitResult::Allowed,
        };

        let mut buckets = self.task_buckets.write().await;
        let bucket = buckets
            .entry(task_name.to_string())
            .or_insert_with(|| TokenBucket::new(limit.rate, limit.burst));

        if bucket.try_acquire() {
            RateLimitResult::Allowed
        } else {
            let retry_after = bucket.time_until_available();
            debug!(
                "Task '{}' rate limit hit, retry after {:?}",
                task_name, retry_after
            );
            RateLimitResult::Limited {
                retry_after,
                limit_type: LimitType::Task,
            }
        }
    }

    /// Get current rate limit status for a queue
    pub async fn queue_status(&self, queue: &str) -> RateLimitStatus {
        let buckets = self.queue_buckets.read().await;
        if let Some(bucket) = buckets.get(queue) {
            // Need mutable access for refill, but we're just reading
            let limit = self.config.queue_limits.get(queue).cloned().unwrap_or_else(|| {
                RateLimit::new(self.config.default_rate, self.config.default_burst)
            });
            RateLimitStatus {
                rate: limit.rate,
                burst: limit.burst,
                available: bucket.tokens as u32,
            }
        } else {
            let limit = self.config.queue_limits.get(queue).cloned().unwrap_or_else(|| {
                RateLimit::new(self.config.default_rate, self.config.default_burst)
            });
            RateLimitStatus {
                rate: limit.rate,
                burst: limit.burst,
                available: limit.burst,
            }
        }
    }

    /// Set rate limit for a queue
    pub async fn set_queue_limit(&mut self, queue: &str, limit: RateLimit) {
        self.config.queue_limits.insert(queue.to_string(), limit.clone());

        // Reset bucket with new limit
        let mut buckets = self.queue_buckets.write().await;
        buckets.insert(queue.to_string(), TokenBucket::new(limit.rate, limit.burst));
    }

    /// Set rate limit for a task
    pub async fn set_task_limit(&mut self, task_name: &str, limit: RateLimit) {
        self.config.task_limits.insert(task_name.to_string(), limit.clone());

        // Reset bucket with new limit
        let mut buckets = self.task_buckets.write().await;
        buckets.insert(task_name.to_string(), TokenBucket::new(limit.rate, limit.burst));
    }

    /// Remove rate limit for a queue (fall back to default)
    pub async fn remove_queue_limit(&mut self, queue: &str) {
        self.config.queue_limits.remove(queue);
        let mut buckets = self.queue_buckets.write().await;
        buckets.remove(queue);
    }

    /// Remove rate limit for a task
    pub async fn remove_task_limit(&mut self, task_name: &str) {
        self.config.task_limits.remove(task_name);
        let mut buckets = self.task_buckets.write().await;
        buckets.remove(task_name);
    }

    /// Clean up old buckets that haven't been used recently
    pub async fn cleanup(&self, max_age: Duration) {
        // In a production implementation, we'd track last access time
        // and remove buckets that haven't been used
        let mut queue_buckets = self.queue_buckets.write().await;
        let mut task_buckets = self.task_buckets.write().await;

        // For now, just log the counts
        debug!(
            "Rate limiter has {} queue buckets and {} task buckets",
            queue_buckets.len(),
            task_buckets.len()
        );
    }
}

/// Rate limit status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitStatus {
    /// Configured rate (requests per second)
    pub rate: f64,

    /// Configured burst size
    pub burst: u32,

    /// Currently available tokens
    pub available: u32,
}

/// Builder for rate limit configuration
pub struct RateLimitConfigBuilder {
    config: RateLimitConfig,
}

impl RateLimitConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: RateLimitConfig::default(),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn default_rate(mut self, rate: f64, burst: u32) -> Self {
        self.config.default_rate = rate;
        self.config.default_burst = burst;
        self
    }

    pub fn global_limit(mut self, rate: f64, burst: u32) -> Self {
        self.config.global_limit = Some(RateLimit::new(rate, burst));
        self
    }

    pub fn queue_limit(mut self, queue: &str, rate: f64, burst: u32) -> Self {
        self.config.queue_limits.insert(
            queue.to_string(),
            RateLimit::new(rate, burst),
        );
        self
    }

    pub fn task_limit(mut self, task_name: &str, rate: f64, burst: u32) -> Self {
        self.config.task_limits.insert(
            task_name.to_string(),
            RateLimit::new(rate, burst),
        );
        self
    }

    pub fn build(self) -> RateLimitConfig {
        self.config
    }
}

impl Default for RateLimitConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_allows_within_limit() {
        let config = RateLimitConfigBuilder::new()
            .default_rate(10.0, 5)
            .build();
        let limiter = RateLimiter::new(config);

        // Should allow first 5 requests (burst)
        for _ in 0..5 {
            let result = limiter.check("test_task", "default").await;
            assert!(matches!(result, RateLimitResult::Allowed));
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_blocks_over_limit() {
        let config = RateLimitConfigBuilder::new()
            .default_rate(10.0, 2)
            .build();
        let limiter = RateLimiter::new(config);

        // First 2 should be allowed
        assert!(matches!(limiter.check("test", "default").await, RateLimitResult::Allowed));
        assert!(matches!(limiter.check("test", "default").await, RateLimitResult::Allowed));

        // Third should be limited
        let result = limiter.check("test", "default").await;
        assert!(matches!(result, RateLimitResult::Limited { .. }));
    }

    #[tokio::test]
    async fn test_task_specific_limit() {
        let config = RateLimitConfigBuilder::new()
            .default_rate(100.0, 100)
            .task_limit("slow_task", 1.0, 1)
            .build();
        let limiter = RateLimiter::new(config);

        // First request allowed
        assert!(matches!(limiter.check("slow_task", "default").await, RateLimitResult::Allowed));

        // Second should be limited
        let result = limiter.check("slow_task", "default").await;
        assert!(matches!(result, RateLimitResult::Limited { limit_type: LimitType::Task, .. }));

        // Other tasks should still be allowed
        assert!(matches!(limiter.check("fast_task", "default").await, RateLimitResult::Allowed));
    }
}
