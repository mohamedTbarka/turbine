//! Task Router - Routing rules and priority queues
//!
//! Provides intelligent task routing based on:
//! - Task name patterns
//! - Task arguments
//! - Headers and metadata
//! - Load balancing across queues
//! - Priority-based routing

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};
use turbine_core::{Message, Task};

/// Router configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Enable routing rules
    pub enabled: bool,

    /// Default queue for unmatched tasks
    pub default_queue: String,

    /// Routing rules (evaluated in order)
    pub rules: Vec<RoutingRule>,

    /// Priority queue configuration
    pub priority_queues: PriorityQueueConfig,
}

impl Default for RouterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            default_queue: "default".to_string(),
            rules: Vec::new(),
            priority_queues: PriorityQueueConfig::default(),
        }
    }
}

/// A routing rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    /// Rule name for identification
    pub name: String,

    /// Rule priority (lower = evaluated first)
    pub priority: i32,

    /// Match conditions (all must match)
    pub conditions: Vec<MatchCondition>,

    /// Action to take when matched
    pub action: RoutingAction,

    /// Whether the rule is enabled
    pub enabled: bool,
}

impl RoutingRule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            priority: 0,
            conditions: Vec::new(),
            action: RoutingAction::Route { queue: "default".to_string() },
            enabled: true,
        }
    }

    pub fn with_priority(mut self, priority: i32) -> Self {
        self.priority = priority;
        self
    }

    pub fn with_condition(mut self, condition: MatchCondition) -> Self {
        self.conditions.push(condition);
        self
    }

    pub fn route_to(mut self, queue: &str) -> Self {
        self.action = RoutingAction::Route { queue: queue.to_string() };
        self
    }

    pub fn reject(mut self, reason: &str) -> Self {
        self.action = RoutingAction::Reject { reason: reason.to_string() };
        self
    }
}

/// Condition for matching tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MatchCondition {
    /// Match task name exactly
    TaskNameEquals { name: String },

    /// Match task name with prefix
    TaskNamePrefix { prefix: String },

    /// Match task name with suffix
    TaskNameSuffix { suffix: String },

    /// Match task name with regex pattern
    TaskNamePattern { pattern: String },

    /// Match queue name
    QueueEquals { queue: String },

    /// Match header value
    HeaderEquals { key: String, value: String },

    /// Match header exists
    HeaderExists { key: String },

    /// Match priority range
    PriorityRange { min: u8, max: u8 },

    /// Match retry count
    RetryCountGreaterThan { count: u32 },

    /// Always match (useful for default rules)
    Always,

    /// Never match (useful for disabled rules)
    Never,

    /// Logical AND of conditions
    And { conditions: Vec<MatchCondition> },

    /// Logical OR of conditions
    Or { conditions: Vec<MatchCondition> },

    /// Logical NOT of condition
    Not { condition: Box<MatchCondition> },
}

impl MatchCondition {
    /// Check if condition matches the task/message
    pub fn matches(&self, task: &Task, message: &Message) -> bool {
        match self {
            MatchCondition::TaskNameEquals { name } => &task.name == name,

            MatchCondition::TaskNamePrefix { prefix } => task.name.starts_with(prefix),

            MatchCondition::TaskNameSuffix { suffix } => task.name.ends_with(suffix),

            MatchCondition::TaskNamePattern { pattern } => {
                // Simple glob-style matching (* = any)
                let regex_pattern = pattern
                    .replace(".", "\\.")
                    .replace("*", ".*")
                    .replace("?", ".");
                regex::Regex::new(&format!("^{}$", regex_pattern))
                    .map(|re| re.is_match(&task.name))
                    .unwrap_or(false)
            }

            MatchCondition::QueueEquals { queue } => &message.headers.queue == queue,

            MatchCondition::HeaderEquals { key, value } => {
                message.headers.correlation_id.as_ref() == Some(value) && key == "correlation_id"
                // In a real implementation, we'd have a proper headers map
            }

            MatchCondition::HeaderExists { key } => {
                key == "correlation_id" && message.headers.correlation_id.is_some()
            }

            MatchCondition::PriorityRange { min, max } => {
                let priority = message.headers.priority;
                priority >= *min && priority <= *max
            }

            MatchCondition::RetryCountGreaterThan { count } => {
                message.headers.retries > *count
            }

            MatchCondition::Always => true,

            MatchCondition::Never => false,

            MatchCondition::And { conditions } => {
                conditions.iter().all(|c| c.matches(task, message))
            }

            MatchCondition::Or { conditions } => {
                conditions.iter().any(|c| c.matches(task, message))
            }

            MatchCondition::Not { condition } => !condition.matches(task, message),
        }
    }
}

/// Action to take when a rule matches
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RoutingAction {
    /// Route to specific queue
    Route { queue: String },

    /// Route to queue based on priority
    PriorityRoute,

    /// Reject the task
    Reject { reason: String },

    /// Route to multiple queues (fan-out)
    FanOut { queues: Vec<String> },

    /// Round-robin across queues
    RoundRobin { queues: Vec<String> },

    /// Route to least-loaded queue
    LeastLoaded { queues: Vec<String> },
}

/// Priority queue configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityQueueConfig {
    /// Enable priority queues
    pub enabled: bool,

    /// Number of priority levels (1-10)
    pub levels: u8,

    /// Queue name pattern (e.g., "{queue}:priority:{level}")
    pub queue_pattern: String,

    /// Priority to queue level mapping
    pub priority_mapping: PriorityMapping,
}

impl Default for PriorityQueueConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            levels: 3,
            queue_pattern: "{queue}:p{level}".to_string(),
            priority_mapping: PriorityMapping::default(),
        }
    }
}

/// How to map task priority to queue level
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PriorityMapping {
    /// Direct mapping: priority 0-2 -> level 0, 3-5 -> level 1, etc.
    #[default]
    Range,

    /// Priority is the level directly
    Direct,

    /// Custom mapping
    Custom(HashMap<u8, u8>),
}

/// Routing decision
#[derive(Debug, Clone)]
pub enum RoutingDecision {
    /// Route to a single queue
    SingleQueue(String),

    /// Route to multiple queues
    MultipleQueues(Vec<String>),

    /// Reject the task
    Rejected { reason: String },
}

/// Task Router
pub struct TaskRouter {
    config: Arc<RwLock<RouterConfig>>,

    /// Round-robin counters for each rule
    round_robin_counters: RwLock<HashMap<String, usize>>,
}

impl TaskRouter {
    /// Create a new router with configuration
    pub fn new(config: RouterConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            round_robin_counters: RwLock::new(HashMap::new()),
        }
    }

    /// Route a task to the appropriate queue(s)
    pub async fn route(&self, task: &Task, message: &Message) -> RoutingDecision {
        let config = self.config.read().await;

        if !config.enabled {
            return RoutingDecision::SingleQueue(
                message.headers.queue.clone()
            );
        }

        // Sort rules by priority
        let mut rules: Vec<_> = config.rules.iter()
            .filter(|r| r.enabled)
            .collect();
        rules.sort_by_key(|r| r.priority);

        // Find first matching rule
        for rule in rules {
            if rule.conditions.iter().all(|c| c.matches(task, message)) {
                debug!("Task '{}' matched rule '{}'", task.name, rule.name);
                return self.apply_action(&rule.action, &rule.name, task, message, &config).await;
            }
        }

        // No rule matched, use default queue or priority routing
        if config.priority_queues.enabled {
            let queue = self.get_priority_queue(
                &config.default_queue,
                message.headers.priority,
                &config.priority_queues,
            );
            RoutingDecision::SingleQueue(queue)
        } else {
            RoutingDecision::SingleQueue(config.default_queue.clone())
        }
    }

    /// Apply routing action
    async fn apply_action(
        &self,
        action: &RoutingAction,
        rule_name: &str,
        task: &Task,
        message: &Message,
        config: &RouterConfig,
    ) -> RoutingDecision {
        match action {
            RoutingAction::Route { queue } => {
                if config.priority_queues.enabled {
                    let queue = self.get_priority_queue(
                        queue,
                        message.headers.priority,
                        &config.priority_queues,
                    );
                    RoutingDecision::SingleQueue(queue)
                } else {
                    RoutingDecision::SingleQueue(queue.clone())
                }
            }

            RoutingAction::PriorityRoute => {
                let queue = self.get_priority_queue(
                    &message.headers.queue,
                    message.headers.priority,
                    &config.priority_queues,
                );
                RoutingDecision::SingleQueue(queue)
            }

            RoutingAction::Reject { reason } => {
                warn!("Task '{}' rejected by rule '{}': {}", task.name, rule_name, reason);
                RoutingDecision::Rejected { reason: reason.clone() }
            }

            RoutingAction::FanOut { queues } => {
                RoutingDecision::MultipleQueues(queues.clone())
            }

            RoutingAction::RoundRobin { queues } => {
                let queue = self.get_round_robin_queue(rule_name, queues).await;
                RoutingDecision::SingleQueue(queue)
            }

            RoutingAction::LeastLoaded { queues } => {
                // In a real implementation, we'd check queue depths
                // For now, just use round-robin
                let queue = self.get_round_robin_queue(rule_name, queues).await;
                RoutingDecision::SingleQueue(queue)
            }
        }
    }

    /// Get priority queue name based on priority level
    fn get_priority_queue(
        &self,
        base_queue: &str,
        priority: u8,
        config: &PriorityQueueConfig,
    ) -> String {
        let level = match &config.priority_mapping {
            PriorityMapping::Range => {
                let range_size = 256 / config.levels as u16;
                (priority as u16 / range_size).min(config.levels as u16 - 1) as u8
            }
            PriorityMapping::Direct => priority.min(config.levels - 1),
            PriorityMapping::Custom(mapping) => {
                mapping.get(&priority).copied().unwrap_or(0)
            }
        };

        config.queue_pattern
            .replace("{queue}", base_queue)
            .replace("{level}", &level.to_string())
    }

    /// Get next queue using round-robin
    async fn get_round_robin_queue(&self, rule_name: &str, queues: &[String]) -> String {
        if queues.is_empty() {
            return "default".to_string();
        }

        let mut counters = self.round_robin_counters.write().await;
        let counter = counters.entry(rule_name.to_string()).or_insert(0);
        let queue = queues[*counter % queues.len()].clone();
        *counter = counter.wrapping_add(1);
        queue
    }

    /// Add a routing rule
    pub async fn add_rule(&self, rule: RoutingRule) {
        let mut config = self.config.write().await;
        config.rules.push(rule);
    }

    /// Remove a routing rule by name
    pub async fn remove_rule(&self, name: &str) {
        let mut config = self.config.write().await;
        config.rules.retain(|r| r.name != name);
    }

    /// Enable/disable a rule
    pub async fn set_rule_enabled(&self, name: &str, enabled: bool) {
        let mut config = self.config.write().await;
        if let Some(rule) = config.rules.iter_mut().find(|r| r.name == name) {
            rule.enabled = enabled;
        }
    }

    /// Get all rules
    pub async fn get_rules(&self) -> Vec<RoutingRule> {
        let config = self.config.read().await;
        config.rules.clone()
    }

    /// Set default queue
    pub async fn set_default_queue(&self, queue: &str) {
        let mut config = self.config.write().await;
        config.default_queue = queue.to_string();
    }
}

/// Builder for router configuration
pub struct RouterConfigBuilder {
    config: RouterConfig,
}

impl RouterConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: RouterConfig::default(),
        }
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.config.enabled = enabled;
        self
    }

    pub fn default_queue(mut self, queue: &str) -> Self {
        self.config.default_queue = queue.to_string();
        self
    }

    pub fn rule(mut self, rule: RoutingRule) -> Self {
        self.config.rules.push(rule);
        self
    }

    pub fn priority_queues(mut self, enabled: bool, levels: u8) -> Self {
        self.config.priority_queues.enabled = enabled;
        self.config.priority_queues.levels = levels;
        self
    }

    pub fn build(self) -> RouterConfig {
        self.config
    }
}

impl Default for RouterConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use turbine_core::{Task, Message, MessageHeaders};

    fn create_test_task(name: &str) -> Task {
        Task::new(name, serde_json::json!({}))
    }

    fn create_test_message(queue: &str, priority: u8) -> Message {
        let task = create_test_task("test");
        let mut message = Message::from_task(&task, Default::default()).unwrap();
        message.headers.queue = queue.to_string();
        message.headers.priority = priority;
        message
    }

    #[tokio::test]
    async fn test_default_routing() {
        let config = RouterConfigBuilder::new()
            .default_queue("default")
            .build();
        let router = TaskRouter::new(config);

        let task = create_test_task("my_task");
        let message = create_test_message("original", 5);

        let decision = router.route(&task, &message).await;
        assert!(matches!(decision, RoutingDecision::SingleQueue(q) if q == "original"));
    }

    #[tokio::test]
    async fn test_rule_based_routing() {
        let config = RouterConfigBuilder::new()
            .default_queue("default")
            .rule(
                RoutingRule::new("email_tasks")
                    .with_condition(MatchCondition::TaskNamePrefix { prefix: "send_email".to_string() })
                    .route_to("emails")
            )
            .build();
        let router = TaskRouter::new(config);

        let task = create_test_task("send_email_welcome");
        let message = create_test_message("default", 5);

        let decision = router.route(&task, &message).await;
        assert!(matches!(decision, RoutingDecision::SingleQueue(q) if q == "emails"));
    }

    #[tokio::test]
    async fn test_priority_queue_routing() {
        let config = RouterConfigBuilder::new()
            .default_queue("tasks")
            .priority_queues(true, 3)
            .build();
        let router = TaskRouter::new(config);

        // High priority (priority 200+)
        let task = create_test_task("urgent");
        let message = create_test_message("tasks", 250);
        let decision = router.route(&task, &message).await;
        assert!(matches!(decision, RoutingDecision::SingleQueue(q) if q == "tasks:p2"));

        // Low priority (priority 0-84)
        let message = create_test_message("tasks", 50);
        let decision = router.route(&task, &message).await;
        assert!(matches!(decision, RoutingDecision::SingleQueue(q) if q == "tasks:p0"));
    }
}
