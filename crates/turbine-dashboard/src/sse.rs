//! Server-Sent Events for real-time dashboard updates

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use futures::stream::Stream;
use serde::Serialize;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::StreamExt;

use crate::state::{DashboardState, QueueInfo, TaskSummary, WorkerInfo};

/// SSE event types
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
pub enum SseEvent {
    /// Worker status changed
    WorkerUpdate(WorkerInfo),

    /// Worker went offline
    WorkerOffline { worker_id: String },

    /// Queue stats updated
    QueueUpdate(QueueInfo),

    /// New task submitted
    TaskSubmitted(TaskSummary),

    /// Task state changed
    TaskStateChange {
        task_id: String,
        old_state: String,
        new_state: String,
    },

    /// Task completed
    TaskCompleted(TaskSummary),

    /// Task failed
    TaskFailed {
        task_id: String,
        error: String,
    },

    /// Task sent to DLQ
    TaskDeadLettered {
        task_id: String,
        queue: String,
        reason: String,
    },

    /// Heartbeat (keep-alive)
    Heartbeat {
        timestamp: i64,
        connections: usize,
    },

    /// Overview stats update
    OverviewUpdate {
        total_workers: usize,
        online_workers: usize,
        total_pending: usize,
        total_processing: usize,
        tasks_per_minute: f64,
    },
}

impl SseEvent {
    /// Convert to SSE Event
    pub fn to_event(&self) -> Result<Event, Infallible> {
        let event_type = match self {
            SseEvent::WorkerUpdate(_) => "worker_update",
            SseEvent::WorkerOffline { .. } => "worker_offline",
            SseEvent::QueueUpdate(_) => "queue_update",
            SseEvent::TaskSubmitted(_) => "task_submitted",
            SseEvent::TaskStateChange { .. } => "task_state_change",
            SseEvent::TaskCompleted(_) => "task_completed",
            SseEvent::TaskFailed { .. } => "task_failed",
            SseEvent::TaskDeadLettered { .. } => "task_dead_lettered",
            SseEvent::Heartbeat { .. } => "heartbeat",
            SseEvent::OverviewUpdate { .. } => "overview_update",
        };

        let data = serde_json::to_string(self).unwrap_or_default();

        Ok(Event::default().event(event_type).data(data))
    }
}

/// SSE handler for real-time updates
/// GET /api/events
pub async fn events_handler(
    State(state): State<Arc<DashboardState>>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Track connection
    let _ = state.inc_sse_connections().await;

    let state_clone = state.clone();

    // Create event stream
    let stream = async_stream::stream! {
        let heartbeat_interval = Duration::from_secs(state_clone.config.sse_heartbeat_secs);
        let mut heartbeat_ticker = tokio::time::interval(heartbeat_interval);

        // Stats refresh interval
        let stats_interval = Duration::from_secs(5);
        let mut stats_ticker = tokio::time::interval(stats_interval);

        loop {
            tokio::select! {
                _ = heartbeat_ticker.tick() => {
                    let event = SseEvent::Heartbeat {
                        timestamp: chrono::Utc::now().timestamp(),
                        connections: state_clone.get_sse_connections().await,
                    };
                    yield event.to_event();
                }

                _ = stats_ticker.tick() => {
                    // Send overview update
                    let workers = state_clone.get_workers().await;
                    let queues = state_clone.get_queues().await;

                    let online_workers = workers.iter()
                        .filter(|w| w.status != crate::state::WorkerStatus::Offline)
                        .count();

                    let total_pending: usize = queues.iter().map(|q| q.pending).sum();
                    let total_processing: usize = queues.iter().map(|q| q.processing).sum();
                    let tasks_per_minute: f64 = queues.iter().map(|q| q.throughput_per_min).sum();

                    let event = SseEvent::OverviewUpdate {
                        total_workers: workers.len(),
                        online_workers,
                        total_pending,
                        total_processing,
                        tasks_per_minute,
                    };
                    yield event.to_event();
                }
            }
        }
    };

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    )
}

/// Event broadcaster for pushing events from other parts of the system
pub struct EventBroadcaster {
    sender: tokio::sync::broadcast::Sender<SseEvent>,
}

impl EventBroadcaster {
    /// Create new broadcaster with specified capacity
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = tokio::sync::broadcast::channel(capacity);
        Self { sender }
    }

    /// Broadcast an event to all subscribers
    pub fn broadcast(&self, event: SseEvent) {
        // Ignore send errors (no subscribers)
        let _ = self.sender.send(event);
    }

    /// Subscribe to events
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<SseEvent> {
        self.sender.subscribe()
    }

    /// Broadcast worker update
    pub fn worker_update(&self, worker: WorkerInfo) {
        self.broadcast(SseEvent::WorkerUpdate(worker));
    }

    /// Broadcast worker offline
    pub fn worker_offline(&self, worker_id: &str) {
        self.broadcast(SseEvent::WorkerOffline {
            worker_id: worker_id.to_string(),
        });
    }

    /// Broadcast queue update
    pub fn queue_update(&self, queue: QueueInfo) {
        self.broadcast(SseEvent::QueueUpdate(queue));
    }

    /// Broadcast task submitted
    pub fn task_submitted(&self, task: TaskSummary) {
        self.broadcast(SseEvent::TaskSubmitted(task));
    }

    /// Broadcast task completed
    pub fn task_completed(&self, task: TaskSummary) {
        self.broadcast(SseEvent::TaskCompleted(task));
    }

    /// Broadcast task failed
    pub fn task_failed(&self, task_id: &str, error: &str) {
        self.broadcast(SseEvent::TaskFailed {
            task_id: task_id.to_string(),
            error: error.to_string(),
        });
    }

    /// Broadcast task dead-lettered
    pub fn task_dead_lettered(&self, task_id: &str, queue: &str, reason: &str) {
        self.broadcast(SseEvent::TaskDeadLettered {
            task_id: task_id.to_string(),
            queue: queue.to_string(),
            reason: reason.to_string(),
        });
    }
}

impl Clone for EventBroadcaster {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}
