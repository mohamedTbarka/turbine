//! Dashboard API router and server

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::handlers;
use crate::sse;
use crate::state::{DashboardConfig, DashboardState};

/// Dashboard API server
pub struct DashboardApi {
    state: Arc<DashboardState>,
}

impl DashboardApi {
    /// Create new dashboard API
    pub async fn new(config: DashboardConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let state = DashboardState::new(config).await?;
        Ok(Self {
            state: Arc::new(state),
        })
    }

    /// Build the API router
    pub fn router(&self) -> Router {
        let state = self.state.clone();

        // Build CORS layer
        let cors = if self.state.config.enable_cors {
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any)
        } else {
            CorsLayer::new()
        };

        Router::new()
            // Health & Overview
            .route("/api/health", get(handlers::health_check))
            .route("/api/overview", get(handlers::get_overview))
            // Workers
            .route("/api/workers", get(handlers::list_workers))
            .route("/api/workers/:id", get(handlers::get_worker))
            // Queues
            .route("/api/queues", get(handlers::list_queues))
            .route("/api/queues/:name", get(handlers::get_queue))
            .route("/api/queues/:name/stats", get(handlers::get_queue_stats))
            .route("/api/queues/:name/purge", post(handlers::purge_queue))
            // Tasks
            .route("/api/tasks", get(handlers::list_tasks))
            .route("/api/tasks/:id", get(handlers::get_task))
            .route("/api/tasks/:id/revoke", post(handlers::revoke_task))
            // DLQ
            .route("/api/dlq/:queue", get(handlers::get_dlq))
            .route("/api/dlq/:queue/reprocess", post(handlers::reprocess_dlq))
            .route("/api/dlq/:queue/purge", post(handlers::purge_dlq))
            // Metrics
            .route("/api/metrics", get(handlers::get_metrics))
            // SSE Events
            .route("/api/events", get(sse::events_handler))
            // State
            .with_state(state)
            // Middleware
            .layer(cors)
            .layer(TraceLayer::new_for_http())
    }

    /// Run the dashboard server
    pub async fn run(self) -> Result<(), Box<dyn std::error::Error>> {
        let addr = SocketAddr::from((
            self.state.config.host.parse::<std::net::IpAddr>()?,
            self.state.config.port,
        ));

        let router = self.router();

        tracing::info!("Starting Turbine Dashboard on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;

        Ok(())
    }

    /// Get shared state for integration
    pub fn state(&self) -> Arc<DashboardState> {
        self.state.clone()
    }
}

/// API endpoints summary for documentation
pub fn api_endpoints() -> Vec<ApiEndpoint> {
    vec![
        // Health & Overview
        ApiEndpoint {
            method: "GET",
            path: "/api/health",
            description: "Health check endpoint",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/overview",
            description: "Dashboard overview statistics",
        },
        // Workers
        ApiEndpoint {
            method: "GET",
            path: "/api/workers",
            description: "List all workers",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/workers/:id",
            description: "Get worker details",
        },
        // Queues
        ApiEndpoint {
            method: "GET",
            path: "/api/queues",
            description: "List all queues",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/queues/:name",
            description: "Get queue details",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/queues/:name/stats",
            description: "Get queue statistics",
        },
        ApiEndpoint {
            method: "POST",
            path: "/api/queues/:name/purge",
            description: "Purge all messages from queue",
        },
        // Tasks
        ApiEndpoint {
            method: "GET",
            path: "/api/tasks",
            description: "List recent tasks",
        },
        ApiEndpoint {
            method: "GET",
            path: "/api/tasks/:id",
            description: "Get task result",
        },
        ApiEndpoint {
            method: "POST",
            path: "/api/tasks/:id/revoke",
            description: "Revoke a task",
        },
        // DLQ
        ApiEndpoint {
            method: "GET",
            path: "/api/dlq/:queue",
            description: "Get DLQ info for queue",
        },
        ApiEndpoint {
            method: "POST",
            path: "/api/dlq/:queue/reprocess",
            description: "Reprocess messages from DLQ",
        },
        ApiEndpoint {
            method: "POST",
            path: "/api/dlq/:queue/purge",
            description: "Purge DLQ",
        },
        // Metrics
        ApiEndpoint {
            method: "GET",
            path: "/api/metrics",
            description: "Prometheus metrics endpoint",
        },
        // SSE
        ApiEndpoint {
            method: "GET",
            path: "/api/events",
            description: "Server-Sent Events for real-time updates",
        },
    ]
}

/// API endpoint documentation
pub struct ApiEndpoint {
    pub method: &'static str,
    pub path: &'static str,
    pub description: &'static str,
}
