//! Scheduler (Beat) for periodic task execution
//!
//! This module provides:
//! - Cron-based scheduling
//! - Interval-based scheduling
//! - One-time delayed tasks
//! - Persistent schedule state

use chrono::{DateTime, Utc};
use cron::Schedule as CronSchedule;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use turbine_backend::Backend;
use turbine_broker::Broker;
use turbine_core::{Message, Serializer, Task, TaskId, TaskOptions};

/// Schedule entry ID
pub type ScheduleId = String;

/// Schedule type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ScheduleType {
    /// Cron expression (e.g., "0 0 * * * *" = every hour)
    Cron(String),
    /// Fixed interval in seconds
    Interval(u64),
    /// One-time execution at a specific time
    OneTime(DateTime<Utc>),
}

/// A scheduled task entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleEntry {
    /// Unique schedule ID
    pub id: ScheduleId,

    /// Task name to execute
    pub task_name: String,

    /// Task arguments
    pub args: Vec<serde_json::Value>,

    /// Task keyword arguments
    pub kwargs: HashMap<String, serde_json::Value>,

    /// Task options
    pub options: TaskOptions,

    /// Schedule type
    pub schedule: ScheduleType,

    /// Whether the schedule is enabled
    pub enabled: bool,

    /// Last run time
    pub last_run: Option<DateTime<Utc>>,

    /// Next run time
    pub next_run: Option<DateTime<Utc>>,

    /// Total run count
    pub run_count: u64,

    /// Created at
    pub created_at: DateTime<Utc>,

    /// Description
    pub description: Option<String>,
}

impl ScheduleEntry {
    /// Create a new cron-based schedule
    pub fn cron(
        id: impl Into<String>,
        task_name: impl Into<String>,
        cron_expr: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        let cron_str = cron_expr.into();
        let next_run = Self::calculate_next_cron(&cron_str, now);

        Self {
            id: id.into(),
            task_name: task_name.into(),
            args: Vec::new(),
            kwargs: HashMap::new(),
            options: TaskOptions::default(),
            schedule: ScheduleType::Cron(cron_str),
            enabled: true,
            last_run: None,
            next_run,
            run_count: 0,
            created_at: now,
            description: None,
        }
    }

    /// Create an interval-based schedule
    pub fn interval(
        id: impl Into<String>,
        task_name: impl Into<String>,
        seconds: u64,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            task_name: task_name.into(),
            args: Vec::new(),
            kwargs: HashMap::new(),
            options: TaskOptions::default(),
            schedule: ScheduleType::Interval(seconds),
            enabled: true,
            last_run: None,
            next_run: Some(now + chrono::Duration::seconds(seconds as i64)),
            run_count: 0,
            created_at: now,
            description: None,
        }
    }

    /// Create a one-time schedule
    pub fn one_time(
        id: impl Into<String>,
        task_name: impl Into<String>,
        run_at: DateTime<Utc>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: id.into(),
            task_name: task_name.into(),
            args: Vec::new(),
            kwargs: HashMap::new(),
            options: TaskOptions::default(),
            schedule: ScheduleType::OneTime(run_at),
            enabled: true,
            last_run: None,
            next_run: Some(run_at),
            run_count: 0,
            created_at: now,
            description: None,
        }
    }

    /// Add arguments
    pub fn with_args(mut self, args: Vec<serde_json::Value>) -> Self {
        self.args = args;
        self
    }

    /// Add keyword arguments
    pub fn with_kwargs(mut self, kwargs: HashMap<String, serde_json::Value>) -> Self {
        self.kwargs = kwargs;
        self
    }

    /// Set task options
    pub fn with_options(mut self, options: TaskOptions) -> Self {
        self.options = options;
        self
    }

    /// Set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Check if the task should run now
    pub fn should_run(&self, now: DateTime<Utc>) -> bool {
        if !self.enabled {
            return false;
        }

        match &self.next_run {
            Some(next) => now >= *next,
            None => false,
        }
    }

    /// Update after a successful run
    pub fn mark_run(&mut self) {
        let now = Utc::now();
        self.last_run = Some(now);
        self.run_count += 1;

        // Calculate next run
        self.next_run = match &self.schedule {
            ScheduleType::Cron(expr) => Self::calculate_next_cron(expr, now),
            ScheduleType::Interval(seconds) => {
                Some(now + chrono::Duration::seconds(*seconds as i64))
            }
            ScheduleType::OneTime(_) => {
                // Disable after one-time run
                self.enabled = false;
                None
            }
        };
    }

    /// Calculate next cron execution time
    fn calculate_next_cron(cron_expr: &str, after: DateTime<Utc>) -> Option<DateTime<Utc>> {
        match CronSchedule::from_str(cron_expr) {
            Ok(schedule) => schedule.after(&after).next(),
            Err(e) => {
                warn!("Invalid cron expression '{}': {}", cron_expr, e);
                None
            }
        }
    }

    /// Create a task from this schedule entry
    pub fn create_task(&self) -> Task {
        let mut task = Task::new(&self.task_name);
        task.args = self.args.clone();
        task.kwargs = self.kwargs.clone();
        task.options = self.options.clone();
        task.correlation_id = Some(format!("schedule:{}", self.id));
        task
    }
}

/// Scheduler configuration
#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    /// How often to check for due tasks (seconds)
    pub tick_interval: u64,

    /// Whether to persist schedule state
    pub persist_state: bool,

    /// Key prefix for persistence
    pub persist_prefix: String,

    /// Maximum jitter to add to scheduled times (seconds)
    pub max_jitter: u64,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            tick_interval: 1,
            persist_state: true,
            persist_prefix: "scheduler".to_string(),
            max_jitter: 0,
        }
    }
}

/// The scheduler (Beat) for periodic task execution
pub struct Scheduler<B: Broker, K: Backend> {
    /// Broker for publishing tasks
    broker: B,

    /// Backend for persistence
    backend: K,

    /// Configuration
    config: SchedulerConfig,

    /// Schedule entries
    entries: Arc<RwLock<HashMap<ScheduleId, ScheduleEntry>>>,

    /// Shutdown flag
    shutdown: Arc<std::sync::atomic::AtomicBool>,

    /// Serializer
    serializer: Serializer,
}

impl<B: Broker, K: Backend> Scheduler<B, K> {
    /// Create a new scheduler
    pub fn new(broker: B, backend: K, config: SchedulerConfig) -> Self {
        Self {
            broker,
            backend,
            config,
            entries: Arc::new(RwLock::new(HashMap::new())),
            shutdown: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            serializer: Serializer::MessagePack,
        }
    }

    /// Add a schedule entry
    pub async fn add(&self, entry: ScheduleEntry) -> anyhow::Result<()> {
        let id = entry.id.clone();

        {
            let mut entries = self.entries.write().await;
            entries.insert(id.clone(), entry);
        }

        if self.config.persist_state {
            self.persist_entry(&id).await?;
        }

        info!("Added schedule entry: {}", id);
        Ok(())
    }

    /// Remove a schedule entry
    pub async fn remove(&self, id: &str) -> anyhow::Result<bool> {
        let removed = {
            let mut entries = self.entries.write().await;
            entries.remove(id).is_some()
        };

        if removed && self.config.persist_state {
            let key = format!("{}:entry:{}", self.config.persist_prefix, id);
            self.backend.delete_raw(&key).await?;
        }

        if removed {
            info!("Removed schedule entry: {}", id);
        }

        Ok(removed)
    }

    /// Get a schedule entry
    pub async fn get(&self, id: &str) -> Option<ScheduleEntry> {
        let entries = self.entries.read().await;
        entries.get(id).cloned()
    }

    /// List all schedule entries
    pub async fn list(&self) -> Vec<ScheduleEntry> {
        let entries = self.entries.read().await;
        entries.values().cloned().collect()
    }

    /// Enable a schedule entry
    pub async fn enable(&self, id: &str) -> anyhow::Result<bool> {
        let updated = {
            let mut entries = self.entries.write().await;
            if let Some(entry) = entries.get_mut(id) {
                entry.enabled = true;
                true
            } else {
                false
            }
        };

        if updated && self.config.persist_state {
            self.persist_entry(id).await?;
        }

        Ok(updated)
    }

    /// Disable a schedule entry
    pub async fn disable(&self, id: &str) -> anyhow::Result<bool> {
        let updated = {
            let mut entries = self.entries.write().await;
            if let Some(entry) = entries.get_mut(id) {
                entry.enabled = false;
                true
            } else {
                false
            }
        };

        if updated && self.config.persist_state {
            self.persist_entry(id).await?;
        }

        Ok(updated)
    }

    /// Persist a schedule entry
    async fn persist_entry(&self, id: &str) -> anyhow::Result<()> {
        let entry = {
            let entries = self.entries.read().await;
            entries.get(id).cloned()
        };

        if let Some(entry) = entry {
            let key = format!("{}:entry:{}", self.config.persist_prefix, id);
            let data = serde_json::to_vec(&entry)?;
            self.backend.store_raw(&key, &data, None).await?;
        }

        Ok(())
    }

    /// Load schedule entries from persistence
    pub async fn load_entries(&self) -> anyhow::Result<usize> {
        // This would typically scan for all persisted entries
        // For now, we just return 0 as entries are loaded on demand
        Ok(0)
    }

    /// Signal shutdown
    pub fn shutdown(&self) {
        self.shutdown
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if shutdown was requested
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Run the scheduler loop
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!(
            "Scheduler starting with tick interval of {}s",
            self.config.tick_interval
        );

        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.tick_interval));

        while !self.is_shutdown() {
            interval.tick().await;

            if let Err(e) = self.tick().await {
                error!("Scheduler tick error: {}", e);
            }
        }

        info!("Scheduler stopped");
        Ok(())
    }

    /// Process one tick - check for and execute due tasks
    async fn tick(&self) -> anyhow::Result<()> {
        let now = Utc::now();
        let due_entries: Vec<ScheduleEntry>;

        // Find due entries
        {
            let entries = self.entries.read().await;
            due_entries = entries
                .values()
                .filter(|e| e.should_run(now))
                .cloned()
                .collect();
        }

        // Execute due tasks
        for entry in due_entries {
            debug!("Executing scheduled task: {} ({})", entry.id, entry.task_name);

            // Create and submit task
            let task = entry.create_task();
            let queue = task.options.queue.clone();

            match Message::from_task(task, self.serializer) {
                Ok(message) => {
                    if let Err(e) = self.broker.publish(&queue, &message).await {
                        error!(
                            "Failed to publish scheduled task {}: {}",
                            entry.id, e
                        );
                    } else {
                        // Update entry after successful submission
                        let mut entries = self.entries.write().await;
                        if let Some(e) = entries.get_mut(&entry.id) {
                            e.mark_run();
                        }

                        // Persist updated state
                        if self.config.persist_state {
                            drop(entries);
                            if let Err(e) = self.persist_entry(&entry.id).await {
                                warn!("Failed to persist schedule state: {}", e);
                            }
                        }

                        info!(
                            "Submitted scheduled task {} ({}), next run: {:?}",
                            entry.id,
                            entry.task_name,
                            self.get(&entry.id).await.and_then(|e| e.next_run)
                        );
                    }
                }
                Err(e) => {
                    error!("Failed to create message for scheduled task {}: {}", entry.id, e);
                }
            }
        }

        Ok(())
    }
}

/// Helper to create common cron expressions
pub mod cron_presets {
    /// Every minute
    pub const EVERY_MINUTE: &str = "0 * * * * *";

    /// Every hour at minute 0
    pub const EVERY_HOUR: &str = "0 0 * * * *";

    /// Every day at midnight
    pub const EVERY_DAY: &str = "0 0 0 * * *";

    /// Every Monday at midnight
    pub const EVERY_WEEK: &str = "0 0 0 * * MON";

    /// First day of every month at midnight
    pub const EVERY_MONTH: &str = "0 0 0 1 * *";

    /// Every 5 minutes
    pub const EVERY_5_MINUTES: &str = "0 */5 * * * *";

    /// Every 15 minutes
    pub const EVERY_15_MINUTES: &str = "0 */15 * * * *";

    /// Every 30 minutes
    pub const EVERY_30_MINUTES: &str = "0 */30 * * * *";

    /// At 9 AM every weekday
    pub const WEEKDAY_9AM: &str = "0 0 9 * * MON-FRI";

    /// At 6 PM every day
    pub const DAILY_6PM: &str = "0 0 18 * * *";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_schedule() {
        let entry = ScheduleEntry::interval("test", "my_task", 60);
        assert_eq!(entry.task_name, "my_task");
        assert!(entry.enabled);
        assert!(entry.next_run.is_some());
    }

    #[test]
    fn test_cron_schedule() {
        let entry = ScheduleEntry::cron("test", "my_task", "0 0 * * * *");
        assert_eq!(entry.task_name, "my_task");
        assert!(entry.enabled);
        // Next run should be calculated
        assert!(entry.next_run.is_some());
    }

    #[test]
    fn test_one_time_schedule() {
        let run_at = Utc::now() + chrono::Duration::hours(1);
        let entry = ScheduleEntry::one_time("test", "my_task", run_at);
        assert_eq!(entry.next_run, Some(run_at));
    }

    #[test]
    fn test_should_run() {
        let mut entry = ScheduleEntry::interval("test", "my_task", 60);

        // Should not run before next_run
        let before = Utc::now() - chrono::Duration::hours(1);
        entry.next_run = Some(Utc::now() + chrono::Duration::hours(1));
        assert!(!entry.should_run(before));

        // Should run after next_run
        entry.next_run = Some(Utc::now() - chrono::Duration::seconds(1));
        assert!(entry.should_run(Utc::now()));
    }

    #[test]
    fn test_mark_run() {
        let mut entry = ScheduleEntry::interval("test", "my_task", 60);
        let original_next = entry.next_run;

        entry.mark_run();

        assert_eq!(entry.run_count, 1);
        assert!(entry.last_run.is_some());
        assert_ne!(entry.next_run, original_next);
    }
}
