//! Workflow definitions for task composition

use crate::{Task, TaskId};
use serde::{Deserialize, Serialize};

/// A workflow represents a composition of tasks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Workflow {
    /// A single task
    Task(Task),

    /// A chain of tasks executed sequentially
    Chain(Chain),

    /// A group of tasks executed in parallel
    Group(Group),

    /// A group followed by a callback task
    Chord(Chord),
}

impl Workflow {
    /// Get the workflow ID
    pub fn id(&self) -> TaskId {
        match self {
            Workflow::Task(task) => task.id.clone(),
            Workflow::Chain(chain) => chain.id.clone(),
            Workflow::Group(group) => group.id.clone(),
            Workflow::Chord(chord) => chord.id.clone(),
        }
    }

    /// Get all tasks in this workflow (flattened)
    pub fn tasks(&self) -> Vec<&Task> {
        match self {
            Workflow::Task(task) => vec![task],
            Workflow::Chain(chain) => chain.tasks.iter().collect(),
            Workflow::Group(group) => group.tasks.iter().collect(),
            Workflow::Chord(chord) => {
                let mut tasks: Vec<&Task> = chord.group.tasks.iter().collect();
                tasks.push(&chord.callback);
                tasks
            }
        }
    }

    /// Get the total number of tasks
    pub fn task_count(&self) -> usize {
        match self {
            Workflow::Task(_) => 1,
            Workflow::Chain(chain) => chain.tasks.len(),
            Workflow::Group(group) => group.tasks.len(),
            Workflow::Chord(chord) => chord.group.tasks.len() + 1,
        }
    }
}

/// A chain executes tasks sequentially, passing results between them
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chain {
    /// Unique identifier for this chain
    pub id: TaskId,

    /// Tasks to execute in order
    pub tasks: Vec<Task>,

    /// Options for the chain
    #[serde(default)]
    pub options: ChainOptions,
}

/// Options for chain execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChainOptions {
    /// Stop chain on first failure
    #[serde(default = "default_true")]
    pub stop_on_failure: bool,

    /// Pass result of each task to the next
    #[serde(default = "default_true")]
    pub pass_results: bool,
}

fn default_true() -> bool {
    true
}

impl Chain {
    /// Create a new chain from tasks
    pub fn new(tasks: Vec<Task>) -> Self {
        let id = TaskId::new();

        // Set up parent-child relationships
        let tasks: Vec<Task> = tasks
            .into_iter()
            .map(|mut t| {
                t.root_id = Some(id.clone());
                t
            })
            .collect();

        Self {
            id,
            tasks,
            options: ChainOptions::default(),
        }
    }

    /// Add a task to the chain
    pub fn then(mut self, mut task: Task) -> Self {
        task.root_id = Some(self.id.clone());
        if let Some(last) = self.tasks.last() {
            task.parent_id = Some(last.id.clone());
        }
        self.tasks.push(task);
        self
    }

    /// Set chain options
    pub fn options(mut self, options: ChainOptions) -> Self {
        self.options = options;
        self
    }

    /// Get the first task
    pub fn first(&self) -> Option<&Task> {
        self.tasks.first()
    }

    /// Get the last task
    pub fn last(&self) -> Option<&Task> {
        self.tasks.last()
    }

    /// Check if the chain is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get the number of tasks
    pub fn len(&self) -> usize {
        self.tasks.len()
    }
}

/// A group executes tasks in parallel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    /// Unique identifier for this group
    pub id: TaskId,

    /// Tasks to execute in parallel
    pub tasks: Vec<Task>,

    /// Options for the group
    #[serde(default)]
    pub options: GroupOptions,
}

/// Options for group execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GroupOptions {
    /// Maximum number of tasks to run concurrently (0 = unlimited)
    #[serde(default)]
    pub max_concurrency: usize,

    /// Continue on failure
    #[serde(default)]
    pub continue_on_failure: bool,
}

impl Group {
    /// Create a new group from tasks
    pub fn new(tasks: Vec<Task>) -> Self {
        let id = TaskId::new();

        // Set up parent relationships
        let tasks: Vec<Task> = tasks
            .into_iter()
            .map(|mut t| {
                t.root_id = Some(id.clone());
                t.parent_id = Some(id.clone());
                t
            })
            .collect();

        Self {
            id,
            tasks,
            options: GroupOptions::default(),
        }
    }

    /// Add a task to the group
    pub fn add(mut self, mut task: Task) -> Self {
        task.root_id = Some(self.id.clone());
        task.parent_id = Some(self.id.clone());
        self.tasks.push(task);
        self
    }

    /// Set group options
    pub fn options(mut self, options: GroupOptions) -> Self {
        self.options = options;
        self
    }

    /// Check if the group is empty
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    /// Get the number of tasks
    pub fn len(&self) -> usize {
        self.tasks.len()
    }
}

/// A chord is a group with a callback that executes after all group tasks complete
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chord {
    /// Unique identifier for this chord
    pub id: TaskId,

    /// Group of tasks to execute in parallel
    pub group: Group,

    /// Callback task to execute after group completes
    pub callback: Task,

    /// Options for the chord
    #[serde(default)]
    pub options: ChordOptions,
}

/// Options for chord execution
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ChordOptions {
    /// Pass group results to callback
    #[serde(default = "default_true")]
    pub pass_results: bool,

    /// Execute callback even if some group tasks fail
    #[serde(default)]
    pub execute_on_partial_failure: bool,
}

impl Chord {
    /// Create a new chord
    pub fn new(group: Group, callback: Task) -> Self {
        let id = TaskId::new();

        // Update group's root ID
        let mut group = group;
        group.id = TaskId::new();
        for task in &mut group.tasks {
            task.root_id = Some(id.clone());
        }

        // Set up callback
        let mut callback = callback;
        callback.root_id = Some(id.clone());
        callback.parent_id = Some(group.id.clone());

        Self {
            id,
            group,
            callback,
            options: ChordOptions::default(),
        }
    }

    /// Set chord options
    pub fn options(mut self, options: ChordOptions) -> Self {
        self.options = options;
        self
    }
}

/// Builder for creating task signatures (partial tasks)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Signature {
    /// Task name
    pub name: String,

    /// Bound arguments
    pub args: Vec<serde_json::Value>,

    /// Bound keyword arguments
    pub kwargs: std::collections::HashMap<String, serde_json::Value>,

    /// Task options
    pub options: crate::TaskOptions,
}

impl Signature {
    /// Create a new signature for a task
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            args: Vec::new(),
            kwargs: std::collections::HashMap::new(),
            options: crate::TaskOptions::default(),
        }
    }

    /// Bind positional arguments
    pub fn args<T: Serialize>(mut self, args: Vec<T>) -> crate::Result<Self> {
        self.args = args
            .into_iter()
            .map(|a| serde_json::to_value(a))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(self)
    }

    /// Bind a single argument
    pub fn arg<T: Serialize>(mut self, arg: T) -> crate::Result<Self> {
        self.args.push(serde_json::to_value(arg)?);
        Ok(self)
    }

    /// Bind keyword arguments
    pub fn kwargs<T: Serialize>(
        mut self,
        kwargs: std::collections::HashMap<String, T>,
    ) -> crate::Result<Self> {
        self.kwargs = kwargs
            .into_iter()
            .map(|(k, v)| Ok((k, serde_json::to_value(v)?)))
            .collect::<crate::Result<std::collections::HashMap<_, _>>>()?;
        Ok(self)
    }

    /// Set options
    pub fn options(mut self, options: crate::TaskOptions) -> Self {
        self.options = options;
        self
    }

    /// Convert to a Task
    pub fn to_task(self) -> Task {
        let mut task = Task::new(self.name);
        task.args = self.args;
        task.kwargs = self.kwargs;
        task.options = self.options;
        task
    }
}

/// Helper trait for creating signatures
pub trait IntoSignature {
    /// Create a signature
    fn s(self) -> Signature;
}

impl IntoSignature for &str {
    fn s(self) -> Signature {
        Signature::new(self)
    }
}

impl IntoSignature for String {
    fn s(self) -> Signature {
        Signature::new(self)
    }
}

/// Create a chain from signatures
pub fn chain(tasks: Vec<Signature>) -> Chain {
    Chain::new(tasks.into_iter().map(|s| s.to_task()).collect())
}

/// Create a group from signatures
pub fn group(tasks: Vec<Signature>) -> Group {
    Group::new(tasks.into_iter().map(|s| s.to_task()).collect())
}

/// Create a chord from a group and callback
pub fn chord(group: Group, callback: Signature) -> Chord {
    Chord::new(group, callback.to_task())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chain_creation() {
        let chain = Chain::new(vec![
            Task::new("task1"),
            Task::new("task2"),
            Task::new("task3"),
        ]);

        assert_eq!(chain.len(), 3);
        assert!(!chain.is_empty());
        assert_eq!(chain.first().unwrap().name, "task1");
        assert_eq!(chain.last().unwrap().name, "task3");
    }

    #[test]
    fn test_group_creation() {
        let group = Group::new(vec![
            Task::new("task1"),
            Task::new("task2"),
        ]);

        assert_eq!(group.len(), 2);

        // All tasks should have the same parent (group ID)
        for task in &group.tasks {
            assert_eq!(task.parent_id.as_ref(), Some(&group.id));
        }
    }

    #[test]
    fn test_chord_creation() {
        let group = Group::new(vec![
            Task::new("fetch1"),
            Task::new("fetch2"),
        ]);
        let callback = Task::new("aggregate");

        let chord = Chord::new(group, callback);

        assert_eq!(chord.group.len(), 2);
        assert_eq!(chord.callback.name, "aggregate");
    }

    #[test]
    fn test_signature() {
        let sig = Signature::new("my_task")
            .arg(42).unwrap()
            .arg("hello").unwrap();

        let task = sig.to_task();
        assert_eq!(task.name, "my_task");
        assert_eq!(task.args.len(), 2);
    }

    #[test]
    fn test_workflow_helpers() {
        let c = chain(vec![
            "task1".s(),
            "task2".s(),
        ]);
        assert_eq!(c.len(), 2);

        let g = group(vec![
            "task1".s(),
            "task2".s(),
        ]);
        assert_eq!(g.len(), 2);
    }
}
