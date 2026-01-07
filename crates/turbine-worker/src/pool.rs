//! Worker pool management

use crate::executor::{TaskExecutor, TaskRegistry};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, Semaphore};
use tracing::{debug, error, info, warn};
use turbine_backend::{Backend, RedisBackend};
use turbine_broker::{Broker, Consumer, ConsumerConfig, RedisBroker};
use turbine_core::{TaskState, TurbineConfig, WorkerConfig};

/// Worker pool - manages concurrent task execution
pub struct WorkerPool {
    /// Worker ID
    worker_id: String,

    /// Configuration
    config: WorkerConfig,

    /// Broker
    broker: RedisBroker,

    /// Backend
    backend: RedisBackend,

    /// Task registry
    registry: Arc<TaskRegistry>,

    /// Concurrency semaphore
    semaphore: Arc<Semaphore>,

    /// Shutdown flag
    shutdown: Arc<AtomicBool>,

    /// Tasks processed counter
    tasks_processed: Arc<AtomicU64>,

    /// Tasks failed counter
    tasks_failed: Arc<AtomicU64>,
}

impl WorkerPool {
    /// Create a new worker pool
    pub async fn new(
        config: TurbineConfig,
        registry: Arc<TaskRegistry>,
    ) -> anyhow::Result<Self> {
        let worker_config = config.worker.clone();
        let worker_id = worker_config.get_id();

        // Connect to broker
        let broker = RedisBroker::connect(&config.broker.url).await?;

        // Connect to backend
        let backend_url = config
            .backend
            .url
            .clone()
            .unwrap_or_else(|| config.broker.url.clone());
        let backend = RedisBackend::connect(&backend_url).await?;

        let concurrency = worker_config.concurrency;

        Ok(Self {
            worker_id,
            config: worker_config,
            broker,
            backend,
            registry,
            semaphore: Arc::new(Semaphore::new(concurrency)),
            shutdown: Arc::new(AtomicBool::new(false)),
            tasks_processed: Arc::new(AtomicU64::new(0)),
            tasks_failed: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Get the worker ID
    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    /// Get tasks processed count
    pub fn tasks_processed(&self) -> u64 {
        self.tasks_processed.load(Ordering::Relaxed)
    }

    /// Get tasks failed count
    pub fn tasks_failed(&self) -> u64 {
        self.tasks_failed.load(Ordering::Relaxed)
    }

    /// Signal shutdown
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Check if shutdown was requested
    pub fn is_shutdown(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Run the worker pool
    pub async fn run(self: Arc<Self>) -> anyhow::Result<()> {
        info!(
            "Starting worker {} with {} concurrent slots",
            self.worker_id, self.config.concurrency
        );
        info!("Consuming from queues: {:?}", self.config.queues);

        // Set up signal handlers
        let pool_clone = Arc::clone(&self);
        tokio::spawn(async move {
            if let Ok(()) = tokio::signal::ctrl_c().await {
                info!("Received shutdown signal");
                pool_clone.shutdown();
            }
        });

        // Create consumer
        let consumer_config = ConsumerConfig::new(self.config.queues.clone())
            .prefetch(1)
            .block_timeout(Duration::from_secs(5))
            .visibility_timeout(Duration::from_secs(self.config.task_time_limit * 2));

        let mut consumer = self.broker.consume(consumer_config).await?;

        // Start heartbeat
        let pool_clone = Arc::clone(&self);
        let heartbeat_interval = self.config.heartbeat_interval;
        tokio::spawn(async move {
            pool_clone.heartbeat_loop(heartbeat_interval).await;
        });

        // Main consumption loop
        while !self.is_shutdown() {
            // Acquire semaphore permit
            let permit = match self.semaphore.clone().acquire_owned().await {
                Ok(p) => p,
                Err(_) => {
                    // Semaphore closed
                    break;
                }
            };

            // Get next message
            let delivery = match consumer.next().await {
                Some(Ok(d)) => d,
                Some(Err(e)) => {
                    warn!("Error receiving message: {}", e);
                    drop(permit);
                    continue;
                }
                None => {
                    // Timeout, continue
                    drop(permit);
                    continue;
                }
            };

            let task_id = delivery.message.task_id();
            debug!("Received task {}", task_id);

            // Spawn task execution
            let pool = Arc::clone(&self);
            let backend = self.backend.clone();
            let registry = Arc::clone(&self.registry);
            let worker_id = self.worker_id.clone();
            let consumer_ref = &consumer;

            // Store delivery tag for ack/nack
            let delivery_tag = delivery.delivery_tag.clone();

            tokio::spawn(async move {
                // Create executor
                let executor = TaskExecutor::new(worker_id, registry, backend);

                // Execute task
                let result = executor.execute(&delivery).await;

                // Acknowledge or nack based on result
                if result.success {
                    pool.tasks_processed.fetch_add(1, Ordering::Relaxed);
                    // Note: In a real implementation, we'd need to pass the consumer
                    // or use a channel to communicate ack/nack
                } else {
                    pool.tasks_failed.fetch_add(1, Ordering::Relaxed);
                    // Check if we should retry
                    let task = delivery.message.extract_task();
                    if let Ok(task) = task {
                        if task.can_retry() {
                            info!("Task {} will be retried", task_id);
                        }
                    }
                }

                // Release permit
                drop(permit);
            });

            // Ack the message (we're doing at-least-once delivery)
            // In a more sophisticated setup, we'd ack after successful execution
            if let Err(e) = consumer.ack(&delivery_tag).await {
                warn!("Failed to acknowledge message: {}", e);
            }
        }

        // Graceful shutdown
        info!("Worker shutting down...");

        // Wait for in-flight tasks
        let shutdown_timeout = Duration::from_secs(self.config.shutdown_timeout);
        let shutdown_deadline = tokio::time::Instant::now() + shutdown_timeout;

        while tokio::time::Instant::now() < shutdown_deadline {
            let available = self.semaphore.available_permits();
            if available >= self.config.concurrency {
                break;
            }
            info!(
                "Waiting for {} in-flight tasks to complete...",
                self.config.concurrency - available
            );
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        info!(
            "Worker {} stopped. Processed: {}, Failed: {}",
            self.worker_id,
            self.tasks_processed(),
            self.tasks_failed()
        );

        Ok(())
    }

    /// Heartbeat loop
    async fn heartbeat_loop(&self, interval: u64) {
        let mut interval = tokio::time::interval(Duration::from_secs(interval));

        while !self.is_shutdown() {
            interval.tick().await;

            debug!(
                "Worker {} heartbeat - processed: {}, failed: {}, available slots: {}",
                self.worker_id,
                self.tasks_processed(),
                self.tasks_failed(),
                self.semaphore.available_permits()
            );

            // TODO: Store heartbeat in backend for monitoring
        }
    }
}

/// Worker statistics
#[derive(Debug, Clone)]
pub struct WorkerStats {
    /// Worker ID
    pub worker_id: String,

    /// Tasks processed
    pub tasks_processed: u64,

    /// Tasks failed
    pub tasks_failed: u64,

    /// Available slots
    pub available_slots: usize,

    /// Total slots
    pub total_slots: usize,

    /// Uptime in seconds
    pub uptime: u64,
}
