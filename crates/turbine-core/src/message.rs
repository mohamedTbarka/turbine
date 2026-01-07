//! Message serialization and transport envelope

use crate::{Error, Result, Task, TaskId};
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Serialization format for messages
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Serializer {
    /// JSON format (human-readable, larger)
    Json,
    /// MessagePack format (binary, compact, fast)
    #[default]
    MessagePack,
}

impl Serializer {
    /// Get the content type string for this serializer
    pub fn content_type(&self) -> &'static str {
        match self {
            Serializer::Json => "application/json",
            Serializer::MessagePack => "application/x-msgpack",
        }
    }

    /// Parse content type string to serializer
    pub fn from_content_type(content_type: &str) -> Option<Self> {
        match content_type {
            "application/json" => Some(Serializer::Json),
            "application/x-msgpack" => Some(Serializer::MessagePack),
            _ => None,
        }
    }

    /// Serialize a value
    pub fn serialize<T: Serialize>(&self, value: &T) -> Result<Bytes> {
        match self {
            Serializer::Json => {
                let data = serde_json::to_vec(value)?;
                Ok(Bytes::from(data))
            }
            Serializer::MessagePack => {
                let data = rmp_serde::to_vec(value)?;
                Ok(Bytes::from(data))
            }
        }
    }

    /// Deserialize a value
    pub fn deserialize<T: for<'de> Deserialize<'de>>(&self, data: &[u8]) -> Result<T> {
        match self {
            Serializer::Json => Ok(serde_json::from_slice(data)?),
            Serializer::MessagePack => Ok(rmp_serde::from_slice(data)?),
        }
    }
}

/// Headers attached to messages
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageHeaders {
    /// Task ID
    pub task_id: String,

    /// Task name
    pub task_name: String,

    /// Target queue
    pub queue: String,

    /// Priority (higher = more important)
    pub priority: u8,

    /// Correlation ID for tracing
    pub correlation_id: Option<String>,

    /// Parent task ID
    pub parent_id: Option<String>,

    /// Root task ID
    pub root_id: Option<String>,

    /// When the message was created
    pub timestamp: DateTime<Utc>,

    /// Expiration time
    pub expires: Option<DateTime<Utc>>,

    /// ETA for delayed execution
    pub eta: Option<DateTime<Utc>>,

    /// Number of retries
    pub retries: u32,

    /// Maximum retries allowed
    pub max_retries: u32,

    /// Content type of the body
    pub content_type: String,

    /// Custom headers
    #[serde(default)]
    pub custom: HashMap<String, String>,
}

impl MessageHeaders {
    /// Create headers from a task
    pub fn from_task(task: &Task, serializer: Serializer) -> Self {
        Self {
            task_id: task.id.to_string(),
            task_name: task.name.clone(),
            queue: task.options.queue.clone(),
            priority: task.options.priority,
            correlation_id: task.correlation_id.clone(),
            parent_id: task.parent_id.as_ref().map(|id| id.to_string()),
            root_id: task.root_id.as_ref().map(|id| id.to_string()),
            timestamp: Utc::now(),
            expires: task.options.expires,
            eta: task.options.effective_eta(),
            retries: task.meta.retries,
            max_retries: task.options.max_retries,
            content_type: serializer.content_type().to_string(),
            custom: task.options.headers.clone(),
        }
    }
}

/// Message envelope for broker transport
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Message headers
    pub headers: MessageHeaders,

    /// Serialized task body
    #[serde(with = "bytes_serde")]
    pub body: Bytes,

    /// Original task (for convenience, not serialized in transport)
    #[serde(skip)]
    pub task: Option<Task>,
}

/// Custom serde for Bytes
mod bytes_serde {
    use bytes::Bytes;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(bytes: &Bytes, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        bytes.as_ref().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
    where
        D: Deserializer<'de>,
    {
        let vec = Vec::<u8>::deserialize(deserializer)?;
        Ok(Bytes::from(vec))
    }
}

impl Message {
    /// Create a new message from a task
    pub fn from_task(task: Task, serializer: Serializer) -> Result<Self> {
        let headers = MessageHeaders::from_task(&task, serializer);
        let body = serializer.serialize(&task)?;

        Ok(Self {
            headers,
            body,
            task: Some(task),
        })
    }

    /// Extract the task from the message body
    pub fn extract_task(&self) -> Result<Task> {
        if let Some(ref task) = self.task {
            return Ok(task.clone());
        }

        let serializer = Serializer::from_content_type(&self.headers.content_type)
            .ok_or_else(|| Error::Deserialization(format!(
                "Unknown content type: {}",
                self.headers.content_type
            )))?;

        serializer.deserialize(&self.body)
    }

    /// Get the task ID
    pub fn task_id(&self) -> TaskId {
        TaskId::from_string(&self.headers.task_id)
    }

    /// Get the queue name
    pub fn queue(&self) -> &str {
        &self.headers.queue
    }

    /// Check if the message has expired
    pub fn is_expired(&self) -> bool {
        match self.headers.expires {
            Some(expires) => Utc::now() > expires,
            None => false,
        }
    }

    /// Check if the message should be executed now
    pub fn should_execute_now(&self) -> bool {
        match self.headers.eta {
            Some(eta) => Utc::now() >= eta,
            None => true,
        }
    }

    /// Get time until ETA (if any)
    pub fn time_until_eta(&self) -> Option<chrono::Duration> {
        self.headers.eta.map(|eta| eta - Utc::now())
    }

    /// Get the priority
    pub fn priority(&self) -> u8 {
        self.headers.priority
    }

    /// Serialize the entire message for transport
    pub fn to_bytes(&self, serializer: Serializer) -> Result<Bytes> {
        serializer.serialize(self)
    }

    /// Deserialize a message from bytes
    pub fn from_bytes(data: &[u8], serializer: Serializer) -> Result<Self> {
        serializer.deserialize(data)
    }
}

/// Acknowledgment action for a message
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckAction {
    /// Acknowledge successful processing
    Ack,
    /// Negative acknowledge - requeue the message
    Nack,
    /// Reject the message (don't requeue)
    Reject,
}

/// Delivery information for a received message
#[derive(Debug, Clone)]
pub struct Delivery {
    /// The message
    pub message: Message,

    /// Delivery tag for acknowledgment
    pub delivery_tag: String,

    /// Whether this is a redelivery
    pub redelivered: bool,

    /// Source queue
    pub queue: String,
}

impl Delivery {
    /// Create a new delivery
    pub fn new(message: Message, delivery_tag: impl Into<String>, queue: impl Into<String>) -> Self {
        Self {
            message,
            delivery_tag: delivery_tag.into(),
            redelivered: false,
            queue: queue.into(),
        }
    }

    /// Mark as redelivered
    pub fn redelivered(mut self) -> Self {
        self.redelivered = true;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_serializer_roundtrip_json() {
        let task = Task::new("test_task").arg(42).unwrap();
        let message = Message::from_task(task.clone(), Serializer::Json).unwrap();

        let bytes = message.to_bytes(Serializer::Json).unwrap();
        let restored = Message::from_bytes(&bytes, Serializer::Json).unwrap();

        assert_eq!(restored.headers.task_id, task.id.to_string());
        assert_eq!(restored.headers.task_name, "test_task");
    }

    #[test]
    fn test_serializer_roundtrip_msgpack() {
        let task = Task::new("test_task").arg("hello").unwrap();
        let message = Message::from_task(task.clone(), Serializer::MessagePack).unwrap();

        let bytes = message.to_bytes(Serializer::MessagePack).unwrap();
        let restored = Message::from_bytes(&bytes, Serializer::MessagePack).unwrap();

        assert_eq!(restored.headers.task_id, task.id.to_string());
    }

    #[test]
    fn test_extract_task() {
        let task = Task::new("my_function")
            .arg(1)
            .unwrap()
            .arg(2)
            .unwrap()
            .kwarg("key", "value")
            .unwrap();

        let message = Message::from_task(task.clone(), Serializer::MessagePack).unwrap();
        let extracted = message.extract_task().unwrap();

        assert_eq!(extracted.id, task.id);
        assert_eq!(extracted.name, task.name);
        assert_eq!(extracted.args.len(), 2);
    }
}
