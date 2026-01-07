//! gRPC service implementation

use crate::generated::turbine::{self as proto, turbine_service_server::TurbineService};
use crate::state::{ServerState, WorkflowState, WorkflowType};
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};
use tracing::info;
use turbine_backend::Backend;
use turbine_broker::Broker;
use turbine_core::{Message, Serializer, Task, TaskId, TaskMeta, TaskOptions, TaskResult, TaskState};

/// Convert internal TaskState to proto TaskState
fn task_state_to_proto(state: TaskState) -> i32 {
    match state {
        TaskState::Pending => proto::TaskState::Pending as i32,
        TaskState::Received => proto::TaskState::Received as i32,
        TaskState::Running => proto::TaskState::Running as i32,
        TaskState::Success => proto::TaskState::Success as i32,
        TaskState::Failure => proto::TaskState::Failure as i32,
        TaskState::Retry => proto::TaskState::Retry as i32,
        TaskState::Revoked => proto::TaskState::Revoked as i32,
    }
}

/// Turbine service implementation
#[derive(Clone)]
pub struct TurbineServiceImpl {
    pub state: Arc<ServerState>,
}

impl TurbineServiceImpl {
    /// Create a new service instance
    pub fn new(state: Arc<ServerState>) -> Self {
        Self { state }
    }

    /// Submit a task
    pub async fn submit_task(
        &self,
        name: String,
        args: Vec<serde_json::Value>,
        kwargs: HashMap<String, serde_json::Value>,
        options: TaskOptions,
        task_id: Option<String>,
        correlation_id: Option<String>,
    ) -> Result<(String, TaskState), Status> {
        // Create task
        let id = task_id.map(TaskId::from).unwrap_or_else(TaskId::new);
        let mut task = Task::with_id(id.clone(), name);
        task.args = args;
        task.kwargs = kwargs;
        task.options = options.clone();
        task.correlation_id = correlation_id;

        // Create message
        let message = Message::from_task(task, Serializer::MessagePack)
            .map_err(|e| Status::internal(format!("Failed to create message: {}", e)))?;

        // Publish to broker
        self.state
            .broker
            .publish(&options.queue, &message)
            .await
            .map_err(|e| Status::internal(format!("Failed to publish task: {}", e)))?;

        // Store initial metadata
        let meta = TaskMeta::new();
        self.state
            .backend
            .store_meta(&id, &meta)
            .await
            .map_err(|e| Status::internal(format!("Failed to store metadata: {}", e)))?;

        info!("Submitted task {} to queue {}", id, options.queue);

        Ok((id.to_string(), TaskState::Pending))
    }

    /// Get task status
    pub async fn get_task_status(&self, task_id: &str) -> Result<Option<TaskMeta>, Status> {
        let id = TaskId::from(task_id);
        self.state
            .backend
            .get_meta(&id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get task status: {}", e)))
    }

    /// Get task result
    pub async fn get_task_result(&self, task_id: &str) -> Result<Option<TaskResult>, Status> {
        let id = TaskId::from(task_id);
        self.state
            .backend
            .get_result(&id)
            .await
            .map_err(|e| Status::internal(format!("Failed to get task result: {}", e)))
    }

    /// Wait for task result
    pub async fn wait_for_result(
        &self,
        task_id: &str,
        timeout_secs: u64,
    ) -> Result<Option<TaskResult>, Status> {
        let id = TaskId::from(task_id);
        let timeout = Duration::from_secs(timeout_secs.max(1).min(300)); // 1-300 seconds

        self.state
            .backend
            .wait_for_result(&id, timeout, Duration::from_millis(100))
            .await
            .map_err(|e| Status::internal(format!("Failed to wait for result: {}", e)))
    }

    /// Revoke a task
    pub async fn revoke_task(&self, task_id: &str) -> Result<bool, Status> {
        let id = TaskId::from(task_id);

        // Try to revoke in broker
        let revoked = self
            .state
            .broker
            .revoke("default", &id)
            .await
            .map_err(|e| Status::internal(format!("Failed to revoke task: {}", e)))?;

        // Update state in backend
        if revoked {
            self.state
                .backend
                .update_state(&id, TaskState::Revoked)
                .await
                .map_err(|e| Status::internal(format!("Failed to update state: {}", e)))?;
        }

        info!("Revoked task {}: {}", task_id, revoked);
        Ok(revoked)
    }

    /// Get queue info
    pub async fn get_queue_info(&self, queue: Option<&str>) -> Result<Vec<QueueInfo>, Status> {
        let queues = match queue {
            Some(q) => vec![q.to_string()],
            None => vec!["default".to_string()], // TODO: track all queues
        };

        let mut infos = Vec::new();
        for queue_name in queues {
            let pending = self
                .state
                .broker
                .queue_length(&queue_name)
                .await
                .unwrap_or(0);

            infos.push(QueueInfo {
                name: queue_name,
                pending: pending as u64,
                processing: 0, // TODO: track processing
                consumers: 0,  // TODO: track consumers
                throughput: 0.0,
            });
        }

        Ok(infos)
    }

    /// Purge a queue
    pub async fn purge_queue(&self, queue: &str) -> Result<u64, Status> {
        let purged = self
            .state
            .broker
            .purge(queue)
            .await
            .map_err(|e| Status::internal(format!("Failed to purge queue: {}", e)))?;

        info!("Purged {} messages from queue {}", purged, queue);
        Ok(purged as u64)
    }

    /// Health check
    pub async fn health_check(&self) -> HealthStatus {
        let broker_ok = self.state.broker_healthy().await;
        let backend_ok = self.state.backend_healthy().await;

        HealthStatus {
            status: if broker_ok && backend_ok {
                "healthy"
            } else {
                "unhealthy"
            }
            .to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime: self.state.uptime(),
            broker_status: if broker_ok { "connected" } else { "disconnected" }.to_string(),
            backend_status: if backend_ok { "connected" } else { "disconnected" }.to_string(),
        }
    }

    /// Submit a chain of tasks
    pub async fn submit_chain(
        &self,
        tasks: Vec<(String, Vec<serde_json::Value>, HashMap<String, serde_json::Value>, TaskOptions)>,
        stop_on_failure: bool,
    ) -> Result<(String, Vec<String>), Status> {
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let mut task_ids = Vec::new();

        // Create tasks with proper parent-child relationships
        let mut previous_id: Option<TaskId> = None;

        for (name, args, kwargs, mut options) in tasks {
            let task_id = TaskId::new();
            let mut task = Task::with_id(task_id.clone(), name);
            task.args = args;
            task.kwargs = kwargs;
            task.root_id = Some(TaskId::from(workflow_id.clone()));

            if let Some(prev) = previous_id.take() {
                task.parent_id = Some(prev);
            }

            // Add workflow metadata to headers
            options.headers.insert("workflow_id".to_string(), workflow_id.clone());
            options.headers.insert("workflow_type".to_string(), "chain".to_string());
            options
                .headers
                .insert("stop_on_failure".to_string(), stop_on_failure.to_string());

            task.options = options.clone();

            // Only publish the first task immediately
            // Subsequent tasks will be published by workers
            if task_ids.is_empty() {
                let message = Message::from_task(task.clone(), Serializer::MessagePack)
                    .map_err(|e| Status::internal(format!("Failed to create message: {}", e)))?;

                self.state
                    .broker
                    .publish(&options.queue, &message)
                    .await
                    .map_err(|e| Status::internal(format!("Failed to publish task: {}", e)))?;
            }

            task_ids.push(task_id.to_string());
            previous_id = Some(task_id);
        }

        // Store workflow state
        {
            let mut workflows = self.state.workflows.write().await;
            workflows.insert(
                workflow_id.clone(),
                WorkflowState::new(workflow_id.clone(), WorkflowType::Chain, task_ids.clone()),
            );
        }

        info!("Submitted chain workflow {} with {} tasks", workflow_id, task_ids.len());
        Ok((workflow_id, task_ids))
    }

    /// Submit a group of parallel tasks
    pub async fn submit_group(
        &self,
        tasks: Vec<(String, Vec<serde_json::Value>, HashMap<String, serde_json::Value>, TaskOptions)>,
    ) -> Result<(String, Vec<String>), Status> {
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let mut task_ids = Vec::new();

        for (name, args, kwargs, mut options) in tasks {
            let task_id = TaskId::new();
            let mut task = Task::with_id(task_id.clone(), name);
            task.args = args;
            task.kwargs = kwargs;
            task.root_id = Some(TaskId::from(workflow_id.clone()));
            task.parent_id = Some(TaskId::from(workflow_id.clone()));

            options.headers.insert("workflow_id".to_string(), workflow_id.clone());
            options.headers.insert("workflow_type".to_string(), "group".to_string());

            task.options = options.clone();

            let message = Message::from_task(task, Serializer::MessagePack)
                .map_err(|e| Status::internal(format!("Failed to create message: {}", e)))?;

            self.state
                .broker
                .publish(&options.queue, &message)
                .await
                .map_err(|e| Status::internal(format!("Failed to publish task: {}", e)))?;

            task_ids.push(task_id.to_string());
        }

        // Store workflow state
        {
            let mut workflows = self.state.workflows.write().await;
            workflows.insert(
                workflow_id.clone(),
                WorkflowState::new(workflow_id.clone(), WorkflowType::Group, task_ids.clone()),
            );
        }

        info!("Submitted group workflow {} with {} tasks", workflow_id, task_ids.len());
        Ok((workflow_id, task_ids))
    }

    /// Get workflow status
    pub async fn get_workflow_status(&self, workflow_id: &str) -> Result<Option<WorkflowState>, Status> {
        let workflows = self.state.workflows.read().await;
        Ok(workflows.get(workflow_id).cloned())
    }
}

/// Queue information
#[derive(Debug, Clone)]
pub struct QueueInfo {
    pub name: String,
    pub pending: u64,
    pub processing: u64,
    pub consumers: u32,
    pub throughput: f64,
}

/// Health status
#[derive(Debug, Clone)]
pub struct HealthStatus {
    pub status: String,
    pub version: String,
    pub uptime: u64,
    pub broker_status: String,
    pub backend_status: String,
}

// Implement the generated TurbineService trait
#[tonic::async_trait]
impl TurbineService for TurbineServiceImpl {
    async fn submit_task(
        &self,
        request: Request<proto::SubmitTaskRequest>,
    ) -> Result<Response<proto::SubmitTaskResponse>, Status> {
        let req = request.into_inner();

        // Parse args from bytes
        let args: Vec<serde_json::Value> = req.args
            .iter()
            .filter_map(|b| serde_json::from_slice(b).ok())
            .collect();

        // Parse kwargs from bytes
        let kwargs: HashMap<String, serde_json::Value> = req.kwargs
            .iter()
            .filter_map(|(k, v)| {
                serde_json::from_slice(v).ok().map(|val| (k.clone(), val))
            })
            .collect();

        // Build options
        let proto_opts = req.options.unwrap_or_default();
        let options = TaskOptions {
            queue: if proto_opts.queue.is_empty() { "default".to_string() } else { proto_opts.queue },
            priority: proto_opts.priority as u8,
            max_retries: proto_opts.max_retries,
            retry_delay: proto_opts.retry_delay,
            timeout: proto_opts.timeout,
            soft_timeout: proto_opts.soft_timeout,
            store_result: proto_opts.store_result,
            result_ttl: proto_opts.result_ttl,
            headers: proto_opts.headers,
            ..Default::default()
        };

        let (task_id, state) = self.submit_task(
            req.name,
            args,
            kwargs,
            options,
            req.task_id,
            req.correlation_id,
        ).await?;

        Ok(Response::new(proto::SubmitTaskResponse {
            task_id,
            state: task_state_to_proto(state),
        }))
    }

    async fn submit_batch(
        &self,
        request: Request<proto::SubmitBatchRequest>,
    ) -> Result<Response<proto::SubmitBatchResponse>, Status> {
        let req = request.into_inner();
        let mut task_ids = Vec::new();

        for task_req in req.tasks {
            let args: Vec<serde_json::Value> = task_req.args
                .iter()
                .filter_map(|b| serde_json::from_slice(b).ok())
                .collect();

            let kwargs: HashMap<String, serde_json::Value> = task_req.kwargs
                .iter()
                .filter_map(|(k, v)| {
                    serde_json::from_slice(v).ok().map(|val| (k.clone(), val))
                })
                .collect();

            let proto_opts = task_req.options.unwrap_or_default();
            let options = TaskOptions {
                queue: if proto_opts.queue.is_empty() { "default".to_string() } else { proto_opts.queue },
                priority: proto_opts.priority as u8,
                max_retries: proto_opts.max_retries,
                ..Default::default()
            };

            let (task_id, _) = self.submit_task(
                task_req.name,
                args,
                kwargs,
                options,
                task_req.task_id,
                task_req.correlation_id,
            ).await?;

            task_ids.push(task_id);
        }

        let count = task_ids.len() as u32;
        Ok(Response::new(proto::SubmitBatchResponse {
            task_ids,
            count,
        }))
    }

    async fn get_task_status(
        &self,
        request: Request<proto::GetTaskStatusRequest>,
    ) -> Result<Response<proto::GetTaskStatusResponse>, Status> {
        let req = request.into_inner();
        let meta = self.get_task_status(&req.task_id).await?;

        let proto_meta = meta.map(|m| proto::TaskMeta {
            state: task_state_to_proto(m.state),
            retries: m.retries,
            created_at: m.created_at.timestamp_millis(),
            started_at: m.started_at.map(|t| t.timestamp_millis()),
            finished_at: m.finished_at.map(|t| t.timestamp_millis()),
            worker_id: m.worker_id,
            error: m.error,
            traceback: m.traceback,
        }).unwrap_or_default();

        Ok(Response::new(proto::GetTaskStatusResponse {
            task_id: req.task_id,
            meta: Some(proto_meta),
        }))
    }

    async fn get_task_result(
        &self,
        request: Request<proto::GetTaskResultRequest>,
    ) -> Result<Response<proto::GetTaskResultResponse>, Status> {
        let req = request.into_inner();
        let result = self.get_task_result(&req.task_id).await?;

        let (ready, proto_result) = match result {
            Some(r) => {
                let result_bytes = r.result
                    .map(|v| serde_json::to_vec(&v).unwrap_or_default());
                (true, Some(proto::TaskResult {
                    task_id: req.task_id.clone(),
                    state: task_state_to_proto(r.state),
                    result: result_bytes,
                    error: r.error,
                    traceback: r.traceback,
                    created_at: r.created_at.timestamp_millis(),
                }))
            }
            None => (false, None),
        };

        Ok(Response::new(proto::GetTaskResultResponse {
            result: proto_result,
            ready,
        }))
    }

    async fn wait_for_result(
        &self,
        request: Request<proto::WaitForResultRequest>,
    ) -> Result<Response<proto::WaitForResultResponse>, Status> {
        let req = request.into_inner();
        let timeout = req.timeout.max(1).min(300);

        let result = self.wait_for_result(&req.task_id, timeout).await?;

        let (success, proto_result) = match result {
            Some(r) => {
                let result_bytes = r.result
                    .map(|v| serde_json::to_vec(&v).unwrap_or_default());
                (true, Some(proto::TaskResult {
                    task_id: req.task_id.clone(),
                    state: task_state_to_proto(r.state),
                    result: result_bytes,
                    error: r.error,
                    traceback: r.traceback,
                    created_at: r.created_at.timestamp_millis(),
                }))
            }
            None => (false, None),
        };

        Ok(Response::new(proto::WaitForResultResponse {
            result: proto_result,
            success,
        }))
    }

    async fn revoke_task(
        &self,
        request: Request<proto::RevokeTaskRequest>,
    ) -> Result<Response<proto::RevokeTaskResponse>, Status> {
        let req = request.into_inner();
        let success = self.revoke_task(&req.task_id).await?;

        Ok(Response::new(proto::RevokeTaskResponse {
            success,
            previous_state: proto::TaskState::Pending as i32,
        }))
    }

    async fn retry_task(
        &self,
        request: Request<proto::RetryTaskRequest>,
    ) -> Result<Response<proto::RetryTaskResponse>, Status> {
        // For now, just return not implemented
        Err(Status::unimplemented("Retry not implemented yet"))
    }

    async fn get_queue_info(
        &self,
        request: Request<proto::GetQueueInfoRequest>,
    ) -> Result<Response<proto::GetQueueInfoResponse>, Status> {
        let req = request.into_inner();
        let infos = self.get_queue_info(req.queue.as_deref()).await?;

        let queues = infos.into_iter().map(|i| proto::QueueInfo {
            name: i.name,
            pending: i.pending,
            processing: i.processing,
            consumers: i.consumers,
            throughput: i.throughput,
        }).collect();

        Ok(Response::new(proto::GetQueueInfoResponse { queues }))
    }

    async fn purge_queue(
        &self,
        request: Request<proto::PurgeQueueRequest>,
    ) -> Result<Response<proto::PurgeQueueResponse>, Status> {
        let req = request.into_inner();
        let purged = self.purge_queue(&req.queue).await?;

        Ok(Response::new(proto::PurgeQueueResponse { purged }))
    }

    type SubscribeEventsStream = Pin<Box<dyn Stream<Item = Result<proto::TaskEvent, Status>> + Send>>;

    async fn subscribe_events(
        &self,
        _request: Request<proto::SubscribeEventsRequest>,
    ) -> Result<Response<Self::SubscribeEventsStream>, Status> {
        Err(Status::unimplemented("Event streaming not implemented yet"))
    }

    async fn health_check(
        &self,
        _request: Request<proto::HealthCheckRequest>,
    ) -> Result<Response<proto::HealthCheckResponse>, Status> {
        let health = TurbineServiceImpl::health_check(self).await;

        Ok(Response::new(proto::HealthCheckResponse {
            status: health.status,
            version: health.version,
            uptime: health.uptime,
            broker_status: health.broker_status,
            backend_status: health.backend_status,
        }))
    }
}
