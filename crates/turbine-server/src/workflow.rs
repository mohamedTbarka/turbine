//! Workflow engine for chains, groups, and chords
//!
//! This module handles the orchestration of complex workflows:
//! - Chain: Sequential task execution where each task's result is passed to the next
//! - Group: Parallel task execution
//! - Chord: Group + callback (callback receives all group results)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};
use turbine_backend::Backend;
use turbine_broker::Broker;
use turbine_core::{Message, Serializer, Task, TaskId, TaskMeta, TaskState};

/// Workflow type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkflowType {
    Chain,
    Group,
    Chord,
}

/// Workflow state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowState {
    /// Workflow ID
    pub id: String,

    /// Workflow type
    pub workflow_type: WorkflowType,

    /// Overall state
    pub state: TaskState,

    /// Task IDs in this workflow
    pub task_ids: Vec<TaskId>,

    /// Task states
    pub task_states: HashMap<TaskId, TaskState>,

    /// Task results (for passing between chain steps)
    pub task_results: HashMap<TaskId, serde_json::Value>,

    /// Current step index (for chains)
    pub current_step: usize,

    /// Callback task ID (for chords)
    pub callback_id: Option<TaskId>,

    /// Options
    pub options: WorkflowOptions,

    /// Creation time
    pub created_at: DateTime<Utc>,

    /// Completion time
    pub completed_at: Option<DateTime<Utc>>,
}

/// Workflow options
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WorkflowOptions {
    /// Stop chain on first failure
    pub stop_on_failure: bool,

    /// Pass results between chain tasks
    pub pass_results: bool,

    /// Continue group on failure
    pub continue_on_failure: bool,

    /// Max concurrency for groups
    pub max_concurrency: u32,

    /// Execute chord callback on partial failure
    pub execute_on_partial_failure: bool,
}

impl WorkflowState {
    /// Create a new chain workflow
    pub fn new_chain(id: String, task_ids: Vec<TaskId>, options: WorkflowOptions) -> Self {
        let mut task_states = HashMap::new();
        for (i, task_id) in task_ids.iter().enumerate() {
            // First task is pending, rest are waiting
            let state = if i == 0 {
                TaskState::Pending
            } else {
                TaskState::Pending // Will be held until previous completes
            };
            task_states.insert(task_id.clone(), state);
        }

        Self {
            id,
            workflow_type: WorkflowType::Chain,
            state: TaskState::Pending,
            task_ids,
            task_states,
            task_results: HashMap::new(),
            current_step: 0,
            callback_id: None,
            options,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Create a new group workflow
    pub fn new_group(id: String, task_ids: Vec<TaskId>, options: WorkflowOptions) -> Self {
        let task_states = task_ids
            .iter()
            .map(|id| (id.clone(), TaskState::Pending))
            .collect();

        Self {
            id,
            workflow_type: WorkflowType::Group,
            state: TaskState::Pending,
            task_ids,
            task_states,
            task_results: HashMap::new(),
            current_step: 0,
            callback_id: None,
            options,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Create a new chord workflow
    pub fn new_chord(
        id: String,
        task_ids: Vec<TaskId>,
        callback_id: TaskId,
        options: WorkflowOptions,
    ) -> Self {
        let mut task_states: HashMap<TaskId, TaskState> = task_ids
            .iter()
            .map(|id| (id.clone(), TaskState::Pending))
            .collect();
        task_states.insert(callback_id.clone(), TaskState::Pending);

        Self {
            id,
            workflow_type: WorkflowType::Chord,
            state: TaskState::Pending,
            task_ids,
            task_states,
            task_results: HashMap::new(),
            current_step: 0,
            callback_id: Some(callback_id),
            options,
            created_at: Utc::now(),
            completed_at: None,
        }
    }

    /// Count completed tasks
    pub fn completed_count(&self) -> usize {
        self.task_states
            .values()
            .filter(|s| matches!(s, TaskState::Success | TaskState::Failure | TaskState::Revoked))
            .count()
    }

    /// Count successful tasks
    pub fn success_count(&self) -> usize {
        self.task_states
            .values()
            .filter(|s| matches!(s, TaskState::Success))
            .count()
    }

    /// Check if workflow is complete
    pub fn is_complete(&self) -> bool {
        self.state.is_terminal()
    }

    /// Update task state and recalculate workflow state
    pub fn update_task_state(&mut self, task_id: &TaskId, state: TaskState, result: Option<serde_json::Value>) {
        self.task_states.insert(task_id.clone(), state);

        if let Some(r) = result {
            self.task_results.insert(task_id.clone(), r);
        }

        // Recalculate overall state
        self.recalculate_state();
    }

    fn recalculate_state(&mut self) {
        match self.workflow_type {
            WorkflowType::Chain => self.recalculate_chain_state(),
            WorkflowType::Group => self.recalculate_group_state(),
            WorkflowType::Chord => self.recalculate_chord_state(),
        }
    }

    fn recalculate_chain_state(&mut self) {
        // Chain is running if any task is running
        let any_running = self.task_states.values().any(|s| *s == TaskState::Running);
        if any_running {
            self.state = TaskState::Running;
            return;
        }

        // Check for failures
        let has_failure = self.task_states.values().any(|s| *s == TaskState::Failure);
        if has_failure && self.options.stop_on_failure {
            self.state = TaskState::Failure;
            self.completed_at = Some(Utc::now());
            return;
        }

        // Check if all complete
        let all_success = self.task_states.values().all(|s| *s == TaskState::Success);
        if all_success && self.task_states.len() == self.task_ids.len() {
            self.state = TaskState::Success;
            self.completed_at = Some(Utc::now());
            return;
        }

        // Still pending
        if self.state != TaskState::Running {
            self.state = TaskState::Pending;
        }
    }

    fn recalculate_group_state(&mut self) {
        let total = self.task_ids.len();
        let completed = self.completed_count();

        // Check if any running
        let any_running = self.task_states.values().any(|s| *s == TaskState::Running);
        if any_running {
            self.state = TaskState::Running;
            return;
        }

        // All completed?
        if completed == total {
            let all_success = self.success_count() == total;
            if all_success {
                self.state = TaskState::Success;
            } else if self.options.continue_on_failure {
                // Partial success
                self.state = TaskState::Success;
            } else {
                self.state = TaskState::Failure;
            }
            self.completed_at = Some(Utc::now());
            return;
        }

        // Check for early failure
        let has_failure = self.task_states.values().any(|s| *s == TaskState::Failure);
        if has_failure && !self.options.continue_on_failure {
            self.state = TaskState::Failure;
            self.completed_at = Some(Utc::now());
            return;
        }

        // Still running
        if completed > 0 {
            self.state = TaskState::Running;
        }
    }

    fn recalculate_chord_state(&mut self) {
        // Chord state depends on group + callback
        let group_task_count = self.task_ids.len();
        let group_completed = self
            .task_ids
            .iter()
            .filter(|id| {
                self.task_states
                    .get(*id)
                    .map(|s| s.is_terminal())
                    .unwrap_or(false)
            })
            .count();

        // If callback is done, we're done
        if let Some(ref callback_id) = self.callback_id {
            if let Some(state) = self.task_states.get(callback_id) {
                if state.is_terminal() {
                    self.state = *state;
                    self.completed_at = Some(Utc::now());
                    return;
                }
            }
        }

        // Check if any running
        let any_running = self.task_states.values().any(|s| *s == TaskState::Running);
        if any_running {
            self.state = TaskState::Running;
            return;
        }

        // Check group completion for chord
        if group_completed == group_task_count {
            // Group is complete, callback should be pending/running
            self.state = TaskState::Running;
        } else if group_completed > 0 {
            self.state = TaskState::Running;
        }
    }
}

/// Workflow engine for managing workflow state and coordination
pub struct WorkflowEngine<B: Broker, K: Backend> {
    /// Broker for publishing tasks
    broker: B,

    /// Backend for storing state
    backend: K,

    /// Active workflows
    workflows: Arc<RwLock<HashMap<String, WorkflowState>>>,

    /// Task to workflow mapping
    task_workflow_map: Arc<RwLock<HashMap<TaskId, String>>>,

    /// Serializer
    serializer: Serializer,
}

impl<B: Broker, K: Backend> WorkflowEngine<B, K> {
    /// Create a new workflow engine
    pub fn new(broker: B, backend: K) -> Self {
        Self {
            broker,
            backend,
            workflows: Arc::new(RwLock::new(HashMap::new())),
            task_workflow_map: Arc::new(RwLock::new(HashMap::new())),
            serializer: Serializer::MessagePack,
        }
    }

    /// Submit a chain workflow
    pub async fn submit_chain(
        &self,
        tasks: Vec<Task>,
        options: WorkflowOptions,
    ) -> anyhow::Result<(String, Vec<TaskId>)> {
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let task_ids: Vec<TaskId> = tasks.iter().map(|t| t.id.clone()).collect();

        // Create workflow state
        let workflow = WorkflowState::new_chain(workflow_id.clone(), task_ids.clone(), options.clone());

        // Store workflow
        {
            let mut workflows = self.workflows.write().await;
            workflows.insert(workflow_id.clone(), workflow);
        }

        // Map tasks to workflow
        {
            let mut map = self.task_workflow_map.write().await;
            for task_id in &task_ids {
                map.insert(task_id.clone(), workflow_id.clone());
            }
        }

        // Submit only the first task (chain executes sequentially)
        if let Some(first_task) = tasks.into_iter().next() {
            let queue = first_task.options.queue.clone();
            let message = Message::from_task(first_task, self.serializer)?;
            self.broker.publish(&queue, &message).await?;
        }

        info!("Created chain workflow {} with {} tasks", workflow_id, task_ids.len());
        Ok((workflow_id, task_ids))
    }

    /// Submit a group workflow
    pub async fn submit_group(
        &self,
        tasks: Vec<Task>,
        options: WorkflowOptions,
    ) -> anyhow::Result<(String, Vec<TaskId>)> {
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let task_ids: Vec<TaskId> = tasks.iter().map(|t| t.id.clone()).collect();

        // Create workflow state
        let workflow = WorkflowState::new_group(workflow_id.clone(), task_ids.clone(), options);

        // Store workflow
        {
            let mut workflows = self.workflows.write().await;
            workflows.insert(workflow_id.clone(), workflow);
        }

        // Map tasks to workflow
        {
            let mut map = self.task_workflow_map.write().await;
            for task_id in &task_ids {
                map.insert(task_id.clone(), workflow_id.clone());
            }
        }

        // Submit all tasks in parallel
        for task in tasks {
            let queue = task.options.queue.clone();
            let message = Message::from_task(task, self.serializer)?;
            self.broker.publish(&queue, &message).await?;
        }

        info!("Created group workflow {} with {} tasks", workflow_id, task_ids.len());
        Ok((workflow_id, task_ids))
    }

    /// Submit a chord workflow
    pub async fn submit_chord(
        &self,
        group_tasks: Vec<Task>,
        callback_task: Task,
        options: WorkflowOptions,
    ) -> anyhow::Result<(String, Vec<TaskId>)> {
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let task_ids: Vec<TaskId> = group_tasks.iter().map(|t| t.id.clone()).collect();
        let callback_id = callback_task.id.clone();

        let mut all_ids = task_ids.clone();
        all_ids.push(callback_id.clone());

        // Create workflow state
        let workflow = WorkflowState::new_chord(
            workflow_id.clone(),
            task_ids.clone(),
            callback_id.clone(),
            options,
        );

        // Store workflow
        {
            let mut workflows = self.workflows.write().await;
            workflows.insert(workflow_id.clone(), workflow);
        }

        // Map tasks to workflow
        {
            let mut map = self.task_workflow_map.write().await;
            for task_id in &all_ids {
                map.insert(task_id.clone(), workflow_id.clone());
            }
        }

        // Submit group tasks (callback will be triggered when group completes)
        for task in group_tasks {
            let queue = task.options.queue.clone();
            let message = Message::from_task(task, self.serializer)?;
            self.broker.publish(&queue, &message).await?;
        }

        // Store callback task for later execution
        let callback_key = format!("workflow:{}:callback", workflow_id);
        let callback_json = serde_json::to_vec(&callback_task)?;
        self.backend
            .store_raw(&callback_key, &callback_json, None)
            .await?;

        info!(
            "Created chord workflow {} with {} group tasks + callback",
            workflow_id,
            task_ids.len()
        );
        Ok((workflow_id, all_ids))
    }

    /// Handle task completion - update workflow state and trigger next steps
    pub async fn on_task_complete(
        &self,
        task_id: &TaskId,
        state: TaskState,
        result: Option<serde_json::Value>,
    ) -> anyhow::Result<()> {
        // Find workflow for this task
        let workflow_id = {
            let map = self.task_workflow_map.read().await;
            map.get(task_id).cloned()
        };

        let workflow_id = match workflow_id {
            Some(id) => id,
            None => return Ok(()), // Not part of a workflow
        };

        // Update workflow state
        let (workflow_type, should_trigger_next, next_task_index, is_chord_callback_ready) = {
            let mut workflows = self.workflows.write().await;
            let workflow = workflows
                .get_mut(&workflow_id)
                .ok_or_else(|| anyhow::anyhow!("Workflow not found"))?;

            workflow.update_task_state(task_id, state, result.clone());

            let workflow_type = workflow.workflow_type;
            let should_trigger_next = matches!(workflow_type, WorkflowType::Chain)
                && state == TaskState::Success
                && workflow.current_step < workflow.task_ids.len() - 1;

            let next_task_index = if should_trigger_next {
                workflow.current_step += 1;
                Some(workflow.current_step)
            } else {
                None
            };

            // Check if chord callback should run
            let is_chord_callback_ready = if workflow_type == WorkflowType::Chord {
                let group_done = workflow.task_ids.iter().all(|id| {
                    workflow
                        .task_states
                        .get(id)
                        .map(|s| s.is_terminal())
                        .unwrap_or(false)
                });
                let callback_pending = workflow.callback_id.as_ref().map(|id| {
                    workflow.task_states.get(id) == Some(&TaskState::Pending)
                }).unwrap_or(false);
                group_done && callback_pending
            } else {
                false
            };

            (workflow_type, should_trigger_next, next_task_index, is_chord_callback_ready)
        };

        // Trigger next chain step if needed
        if should_trigger_next {
            if let Some(idx) = next_task_index {
                debug!("Triggering chain step {} for workflow {}", idx, workflow_id);
                // In a full implementation, we'd fetch the next task and submit it
                // For now, tasks should already be in the queue with dependencies
            }
        }

        // Trigger chord callback if ready
        if is_chord_callback_ready {
            self.trigger_chord_callback(&workflow_id).await?;
        }

        Ok(())
    }

    /// Trigger chord callback with group results
    async fn trigger_chord_callback(&self, workflow_id: &str) -> anyhow::Result<()> {
        // Load callback task
        let callback_key = format!("workflow:{}:callback", workflow_id);
        let callback_data = self.backend.get_raw(&callback_key).await?;

        let callback_data = match callback_data {
            Some(d) => d,
            None => return Err(anyhow::anyhow!("Callback task not found")),
        };

        let mut callback_task: Task = serde_json::from_slice(&callback_data)?;

        // Get group results
        let results = {
            let workflows = self.workflows.read().await;
            let workflow = workflows
                .get(workflow_id)
                .ok_or_else(|| anyhow::anyhow!("Workflow not found"))?;

            if workflow.options.pass_results {
                let results: Vec<serde_json::Value> = workflow
                    .task_ids
                    .iter()
                    .filter_map(|id| workflow.task_results.get(id).cloned())
                    .collect();
                Some(results)
            } else {
                None
            }
        };

        // Prepend group results to callback args
        if let Some(results) = results {
            callback_task.args.insert(0, serde_json::to_value(results)?);
        }

        // Submit callback
        let queue = callback_task.options.queue.clone();
        let message = Message::from_task(callback_task, self.serializer)?;
        self.broker.publish(&queue, &message).await?;

        info!("Triggered chord callback for workflow {}", workflow_id);
        Ok(())
    }

    /// Get workflow status
    pub async fn get_workflow_status(&self, workflow_id: &str) -> Option<WorkflowState> {
        let workflows = self.workflows.read().await;
        workflows.get(workflow_id).cloned()
    }
}
