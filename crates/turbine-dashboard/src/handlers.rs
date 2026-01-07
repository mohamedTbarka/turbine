//! HTTP request handlers for dashboard API

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use turbine_broker::Broker;
use turbine_backend::Backend;

use crate::state::{DashboardState, QueueInfo, TaskSummary, WorkerInfo};

/// API response wrapper
#[derive(Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    pub fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub broker_connected: bool,
    pub backend_connected: bool,
    pub uptime_secs: u64,
}

/// Dashboard overview statistics
#[derive(Serialize)]
pub struct OverviewStats {
    pub total_workers: usize,
    pub online_workers: usize,
    pub total_queues: usize,
    pub total_pending: usize,
    pub total_processing: usize,
    pub total_dlq: usize,
    pub tasks_per_minute: f64,
    pub sse_connections: usize,
}

/// Query parameters for task listing
#[derive(Deserialize, Default)]
pub struct TaskQuery {
    pub queue: Option<String>,
    pub state: Option<String>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
}

/// Request body for task retry
#[derive(Deserialize)]
pub struct RetryRequest {
    pub task_id: String,
}

/// Request body for task revoke
#[derive(Deserialize)]
pub struct RevokeRequest {
    pub task_id: String,
    pub terminate: Option<bool>,
}

/// Request body for DLQ reprocess
#[derive(Deserialize)]
pub struct ReprocessDlqRequest {
    pub queue: String,
    pub task_id: Option<String>,
}

// ============ Health & Overview Handlers ============

/// GET /api/health
pub async fn health_check(
    State(state): State<Arc<DashboardState>>,
) -> Json<ApiResponse<HealthResponse>> {
    // Check broker connectivity
    let broker_connected = state.broker.stats().await.is_ok();

    // Check backend connectivity
    let backend_connected = state.backend.is_connected().await;

    Json(ApiResponse::success(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        broker_connected,
        backend_connected,
        uptime_secs: 0, // TODO: track actual uptime
    }))
}

/// GET /api/overview
pub async fn get_overview(
    State(state): State<Arc<DashboardState>>,
) -> Json<ApiResponse<OverviewStats>> {
    let workers = state.get_workers().await;
    let queues = state.get_queues().await;

    let online_workers = workers
        .iter()
        .filter(|w| w.status != crate::state::WorkerStatus::Offline)
        .count();

    let total_pending: usize = queues.iter().map(|q| q.pending).sum();
    let total_processing: usize = queues.iter().map(|q| q.processing).sum();
    let total_dlq: usize = queues.iter().map(|q| q.dlq_size).sum();
    let tasks_per_minute: f64 = queues.iter().map(|q| q.throughput_per_min).sum();

    Json(ApiResponse::success(OverviewStats {
        total_workers: workers.len(),
        online_workers,
        total_queues: queues.len(),
        total_pending,
        total_processing,
        total_dlq,
        tasks_per_minute,
        sse_connections: state.get_sse_connections().await,
    }))
}

// ============ Worker Handlers ============

/// GET /api/workers
pub async fn list_workers(
    State(state): State<Arc<DashboardState>>,
) -> Json<ApiResponse<Vec<WorkerInfo>>> {
    let workers = state.get_workers().await;
    Json(ApiResponse::success(workers))
}

/// GET /api/workers/:id
pub async fn get_worker(
    State(state): State<Arc<DashboardState>>,
    Path(worker_id): Path<String>,
) -> Result<Json<ApiResponse<WorkerInfo>>, StatusCode> {
    let workers = state.workers.read().await;

    match workers.get(&worker_id) {
        Some(worker) => Ok(Json(ApiResponse::success(worker.clone()))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

// ============ Queue Handlers ============

/// GET /api/queues
pub async fn list_queues(
    State(state): State<Arc<DashboardState>>,
) -> Json<ApiResponse<Vec<QueueInfo>>> {
    let queues = state.get_queues().await;
    Json(ApiResponse::success(queues))
}

/// GET /api/queues/:name
pub async fn get_queue(
    State(state): State<Arc<DashboardState>>,
    Path(queue_name): Path<String>,
) -> Result<Json<ApiResponse<QueueInfo>>, StatusCode> {
    let queues = state.queues.read().await;

    match queues.get(&queue_name) {
        Some(queue) => Ok(Json(ApiResponse::success(queue.clone()))),
        None => Err(StatusCode::NOT_FOUND),
    }
}

/// GET /api/queues/:name/stats
pub async fn get_queue_stats(
    State(state): State<Arc<DashboardState>>,
    Path(queue_name): Path<String>,
) -> Result<Json<ApiResponse<QueueInfo>>, StatusCode> {
    // Fetch fresh stats from broker
    let length = state
        .broker
        .queue_length(&queue_name)
        .await
        .unwrap_or(0);

    let dlq_length = state
        .broker
        .dlq_length(&queue_name)
        .await
        .unwrap_or(0);

    let queue_info = QueueInfo {
        name: queue_name,
        pending: length,
        processing: 0, // Would need to track this
        dlq_size: dlq_length,
        workers: 0,
        throughput_per_min: 0.0,
        avg_processing_time_ms: 0.0,
    };

    Ok(Json(ApiResponse::success(queue_info)))
}

/// POST /api/queues/:name/purge
pub async fn purge_queue(
    State(state): State<Arc<DashboardState>>,
    Path(queue_name): Path<String>,
) -> Result<Json<ApiResponse<usize>>, StatusCode> {
    match state.broker.purge(&queue_name).await {
        Ok(count) => Ok(Json(ApiResponse::success(count))),
        Err(e) => {
            tracing::error!("Failed to purge queue {}: {}", queue_name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============ Task Handlers ============

/// GET /api/tasks
pub async fn list_tasks(
    State(state): State<Arc<DashboardState>>,
    Query(query): Query<TaskQuery>,
) -> Json<ApiResponse<Vec<TaskSummary>>> {
    let limit = query.limit.unwrap_or(50).min(100);
    let tasks = state.get_recent_tasks(limit).await;

    // Filter by queue if specified
    let filtered: Vec<TaskSummary> = if let Some(queue) = &query.queue {
        tasks.into_iter().filter(|t| &t.queue == queue).collect()
    } else {
        tasks
    };

    // Filter by state if specified
    let filtered: Vec<TaskSummary> = if let Some(state_filter) = &query.state {
        filtered
            .into_iter()
            .filter(|t| &t.state == state_filter)
            .collect()
    } else {
        filtered
    };

    Json(ApiResponse::success(filtered))
}

/// GET /api/tasks/:id
pub async fn get_task(
    State(state): State<Arc<DashboardState>>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<TaskResultResponse>>, StatusCode> {
    let task_id = turbine_core::TaskId::from(task_id);

    match state.backend.get_result(&task_id).await {
        Ok(Some(result)) => {
            let result_str = result.result.as_ref().map(|r| r.to_string());
            Ok(Json(ApiResponse::success(TaskResultResponse {
                task_id: result.task_id.to_string(),
                state: format!("{:?}", result.state),
                result: result_str,
                error: result.error,
                traceback: result.traceback,
                created_at: result.created_at,
            })))
        },
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            tracing::error!("Failed to get task {}: {}", task_id, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Serialize)]
pub struct TaskResultResponse {
    pub task_id: String,
    pub state: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub traceback: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// POST /api/tasks/:id/revoke
pub async fn revoke_task(
    State(state): State<Arc<DashboardState>>,
    Path(task_id): Path<String>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    let task_id = turbine_core::TaskId::from(task_id.clone());

    // Try to revoke from all known queues
    let queues = state.get_queues().await;

    for queue in &queues {
        if let Ok(revoked) = state.broker.revoke(&queue.name, &task_id).await {
            if revoked {
                return Ok(Json(ApiResponse::success(true)));
            }
        }
    }

    // Task not found in any queue
    Ok(Json(ApiResponse::success(false)))
}

// ============ DLQ Handlers ============

/// GET /api/dlq/:queue
pub async fn get_dlq(
    State(state): State<Arc<DashboardState>>,
    Path(queue_name): Path<String>,
) -> Result<Json<ApiResponse<DlqInfo>>, StatusCode> {
    let length = state
        .broker
        .dlq_length(&queue_name)
        .await
        .unwrap_or(0);

    Ok(Json(ApiResponse::success(DlqInfo {
        queue: queue_name,
        size: length,
    })))
}

#[derive(Serialize)]
pub struct DlqInfo {
    pub queue: String,
    pub size: usize,
}

/// POST /api/dlq/:queue/reprocess
pub async fn reprocess_dlq(
    State(state): State<Arc<DashboardState>>,
    Path(queue_name): Path<String>,
    Json(body): Json<ReprocessDlqRequest>,
) -> Result<Json<ApiResponse<bool>>, StatusCode> {
    if let Some(task_id) = body.task_id {
        let task_id = turbine_core::TaskId::from(task_id);
        match state.broker.reprocess_from_dlq(&queue_name, &task_id).await {
            Ok(success) => Ok(Json(ApiResponse::success(success))),
            Err(e) => {
                tracing::error!("Failed to reprocess from DLQ: {}", e);
                Err(StatusCode::INTERNAL_SERVER_ERROR)
            }
        }
    } else {
        // Reprocess all - not implemented yet
        Ok(Json(ApiResponse::error("Reprocess all not implemented")))
    }
}

/// POST /api/dlq/:queue/purge
pub async fn purge_dlq(
    State(state): State<Arc<DashboardState>>,
    Path(queue_name): Path<String>,
) -> Result<Json<ApiResponse<usize>>, StatusCode> {
    match state.broker.purge_dlq(&queue_name).await {
        Ok(count) => Ok(Json(ApiResponse::success(count))),
        Err(e) => {
            tracing::error!("Failed to purge DLQ {}: {}", queue_name, e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

// ============ Metrics Handler ============

/// GET /api/metrics
#[cfg(feature = "metrics")]
pub async fn get_metrics() -> impl IntoResponse {
    use turbine_telemetry::TurbineMetrics;

    let metrics = TurbineMetrics::new(Default::default());
    match metrics {
        Ok(m) => (StatusCode::OK, m.gather()),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, String::new()),
    }
}

#[cfg(not(feature = "metrics"))]
pub async fn get_metrics() -> impl IntoResponse {
    (StatusCode::NOT_IMPLEMENTED, "Metrics not enabled")
}
