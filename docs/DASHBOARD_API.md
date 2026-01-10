# Turbine Dashboard API Documentation

## Base URL

```
http://localhost:8080
```

## Authentication

Currently, the API does not require authentication. Future versions may include:
- API keys
- JWT tokens
- OAuth 2.0

## Response Format

All responses are in JSON format with the following structure:

```json
{
  "status": "success|error",
  "data": { ... },
  "error": "error message if applicable"
}
```

## Endpoints

### Health & Overview

#### GET `/api/health`

Health check endpoint.

**Response:**
```json
{
  "status": "healthy",
  "uptime_seconds": 3600,
  "version": "0.1.0"
}
```

---

#### GET `/api/overview`

Dashboard overview statistics.

**Response:**
```json
{
  "tasks": {
    "total_processed": 15420,
    "pending": 45,
    "running": 12,
    "success": 14890,
    "failed": 530
  },
  "queues": {
    "total": 5,
    "active": 4
  },
  "workers": {
    "total": 8,
    "active": 7,
    "idle": 1
  },
  "throughput": {
    "tasks_per_second": 15.3,
    "tasks_per_minute": 918
  }
}
```

---

### Workers

#### GET `/api/workers`

List all workers.

**Query Parameters:**
- `status` (optional): Filter by status (`active`, `idle`, `offline`)

**Response:**
```json
{
  "workers": [
    {
      "id": "worker-abc123",
      "hostname": "server-01",
      "pid": 12345,
      "status": "active",
      "queues": ["default", "emails"],
      "concurrency": 4,
      "tasks_processed": 1240,
      "current_task": "send_email[task-id]",
      "started_at": "2024-01-09T10:00:00Z",
      "last_heartbeat": "2024-01-09T12:30:00Z"
    }
  ]
}
```

---

#### GET `/api/workers/:id`

Get details for a specific worker.

**Path Parameters:**
- `id`: Worker ID

**Response:**
```json
{
  "id": "worker-abc123",
  "hostname": "server-01",
  "pid": 12345,
  "status": "active",
  "queues": ["default", "emails"],
  "concurrency": 4,
  "tasks_processed": 1240,
  "current_task": {
    "task_id": "task-xyz",
    "task_name": "send_email",
    "started_at": "2024-01-09T12:29:45Z"
  },
  "statistics": {
    "success_count": 1200,
    "failure_count": 40,
    "avg_duration_ms": 523.5
  },
  "started_at": "2024-01-09T10:00:00Z",
  "last_heartbeat": "2024-01-09T12:30:00Z"
}
```

---

### Queues

#### GET `/api/queues`

List all queues.

**Response:**
```json
{
  "queues": [
    {
      "name": "default",
      "pending": 45,
      "processing": 12,
      "consumers": 8,
      "throughput": 15.3
    },
    {
      "name": "emails",
      "pending": 120,
      "processing": 5,
      "consumers": 3,
      "throughput": 8.7
    }
  ]
}
```

---

#### GET `/api/queues/:name`

Get details for a specific queue.

**Path Parameters:**
- `name`: Queue name

**Response:**
```json
{
  "name": "default",
  "pending": 45,
  "processing": 12,
  "completed_total": 14890,
  "failed_total": 530,
  "consumers": 8,
  "throughput": 15.3,
  "avg_wait_time_ms": 125.3,
  "avg_processing_time_ms": 543.2
}
```

---

#### GET `/api/queues/:name/stats`

Get detailed statistics for a queue.

**Path Parameters:**
- `name`: Queue name

**Query Parameters:**
- `period` (optional): Time period (`1h`, `24h`, `7d`, `30d`) - default: `24h`

**Response:**
```json
{
  "name": "default",
  "period": "24h",
  "throughput_timeline": [
    {
      "timestamp": "2024-01-09T00:00:00Z",
      "tasks_per_minute": 45.2
    },
    {
      "timestamp": "2024-01-09T01:00:00Z",
      "tasks_per_minute": 52.1
    }
  ],
  "state_distribution": {
    "success": 14890,
    "failed": 530,
    "retry": 45,
    "revoked": 12
  }
}
```

---

#### POST `/api/queues/:name/purge`

Purge all messages from a queue.

**Path Parameters:**
- `name`: Queue name

**Response:**
```json
{
  "queue": "default",
  "purged": 45
}
```

---

### Tasks

#### GET `/api/tasks`

List recent tasks.

**Query Parameters:**
- `limit` (optional): Max number of tasks (default: 100, max: 1000)
- `offset` (optional): Offset for pagination (default: 0)
- `state` (optional): Filter by state (`pending`, `running`, `success`, `failed`)
- `queue` (optional): Filter by queue name
- `since` (optional): ISO 8601 timestamp

**Response:**
```json
{
  "tasks": [
    {
      "task_id": "abc-123",
      "task_name": "send_email",
      "state": "success",
      "queue": "emails",
      "created_at": "2024-01-09T12:00:00Z",
      "started_at": "2024-01-09T12:00:05Z",
      "finished_at": "2024-01-09T12:00:06Z",
      "duration_ms": 1050,
      "retries": 0,
      "worker_id": "worker-abc123"
    }
  ],
  "total": 15420,
  "limit": 100,
  "offset": 0
}
```

---

#### GET `/api/tasks/:id`

Get details for a specific task.

**Path Parameters:**
- `id`: Task ID

**Response:**
```json
{
  "task_id": "abc-123",
  "task_name": "send_email",
  "state": "success",
  "queue": "emails",
  "args": ["user@example.com", "Welcome!"],
  "kwargs": {
    "html": true,
    "priority": "high"
  },
  "result": {
    "message_id": "msg-xyz",
    "sent": true
  },
  "error": null,
  "traceback": null,
  "retries": 0,
  "max_retries": 3,
  "created_at": "2024-01-09T12:00:00Z",
  "started_at": "2024-01-09T12:00:05Z",
  "finished_at": "2024-01-09T12:00:06Z",
  "duration_ms": 1050,
  "worker_id": "worker-abc123"
}
```

---

#### POST `/api/tasks/:id/revoke`

Revoke a pending or running task.

**Path Parameters:**
- `id`: Task ID

**Request Body:**
```json
{
  "terminate": true
}
```

**Response:**
```json
{
  "task_id": "abc-123",
  "revoked": true,
  "state": "revoked"
}
```

---

### Dead Letter Queue (DLQ)

#### GET `/api/dlq/:queue`

Get failed tasks from the DLQ for a specific queue.

**Path Parameters:**
- `queue`: Queue name

**Query Parameters:**
- `limit` (optional): Max number of tasks (default: 100)
- `offset` (optional): Offset for pagination (default: 0)

**Response:**
```json
{
  "queue": "emails",
  "total_failed": 45,
  "tasks": [
    {
      "task_id": "failed-123",
      "task_name": "send_email",
      "args": ["invalid@email"],
      "kwargs": {},
      "error": "Invalid email address",
      "traceback": "Traceback...",
      "retries": 3,
      "failed_at": "2024-01-09T12:00:00Z",
      "worker_id": "worker-abc123"
    }
  ]
}
```

---

#### POST `/api/dlq/:queue/reprocess`

Reprocess failed tasks from the DLQ.

**Path Parameters:**
- `queue`: Queue name

**Request Body:**
```json
{
  "task_ids": ["failed-123", "failed-456"],
  "all": false
}
```

**Response:**
```json
{
  "queue": "emails",
  "reprocessed": 2,
  "new_task_ids": ["new-123", "new-456"]
}
```

---

#### POST `/api/dlq/:queue/purge`

Remove all failed tasks from the DLQ.

**Path Parameters:**
- `queue`: Queue name

**Response:**
```json
{
  "queue": "emails",
  "purged": 45
}
```

---

### Metrics

#### GET `/api/metrics`

Get Prometheus-formatted metrics.

**Response:**
```
# TYPE turbine_tasks_total counter
turbine_tasks_total{state="success"} 14890
turbine_tasks_total{state="failed"} 530

# TYPE turbine_task_duration_seconds histogram
turbine_task_duration_seconds_bucket{le="0.1"} 5430
turbine_task_duration_seconds_bucket{le="0.5"} 12340
turbine_task_duration_seconds_bucket{le="1.0"} 14200
turbine_task_duration_seconds_bucket{le="+Inf"} 15420
turbine_task_duration_seconds_sum 8234.5
turbine_task_duration_seconds_count 15420
```

---

### Real-time Events (SSE)

#### GET `/api/events`

Subscribe to real-time server-sent events.

**Headers:**
```
Accept: text/event-stream
Cache-Control: no-cache
Connection: keep-alive
```

**Event Types:**

1. **task_started**
```json
{
  "type": "task_started",
  "task_id": "abc-123",
  "task_name": "send_email",
  "queue": "emails",
  "worker_id": "worker-abc123",
  "timestamp": "2024-01-09T12:00:00Z"
}
```

2. **task_completed**
```json
{
  "type": "task_completed",
  "task_id": "abc-123",
  "task_name": "send_email",
  "state": "success",
  "duration_ms": 1050,
  "timestamp": "2024-01-09T12:00:01Z"
}
```

3. **task_failed**
```json
{
  "type": "task_failed",
  "task_id": "abc-123",
  "task_name": "send_email",
  "error": "Connection timeout",
  "retries": 2,
  "will_retry": true,
  "timestamp": "2024-01-09T12:00:01Z"
}
```

4. **worker_connected**
```json
{
  "type": "worker_connected",
  "worker_id": "worker-abc123",
  "hostname": "server-01",
  "queues": ["default", "emails"],
  "timestamp": "2024-01-09T12:00:00Z"
}
```

5. **worker_disconnected**
```json
{
  "type": "worker_disconnected",
  "worker_id": "worker-abc123",
  "reason": "shutdown",
  "timestamp": "2024-01-09T12:00:00Z"
}
```

---

## Error Responses

All error responses follow this format:

```json
{
  "error": {
    "code": "TASK_NOT_FOUND",
    "message": "Task with ID 'abc-123' not found",
    "details": {}
  }
}
```

### Common Error Codes

| Code | HTTP Status | Description |
|------|-------------|-------------|
| `TASK_NOT_FOUND` | 404 | Task does not exist |
| `WORKER_NOT_FOUND` | 404 | Worker does not exist |
| `QUEUE_NOT_FOUND` | 404 | Queue does not exist |
| `INVALID_REQUEST` | 400 | Invalid request parameters |
| `INTERNAL_ERROR` | 500 | Internal server error |
| `SERVICE_UNAVAILABLE` | 503 | Service temporarily unavailable |

---

## Rate Limiting

Currently, there is no rate limiting. Future versions may include:
- Per-IP rate limits
- Per-API-key limits
- Burst allowances

---

## CORS

CORS is enabled by default and allows:
- All origins (`*`)
- All methods
- All headers

This can be configured via the `--cors` flag.

---

## WebSocket Support (Future)

Future versions may include WebSocket support as an alternative to SSE for:
- Bidirectional communication
- Lower latency
- Better mobile support

---

## Versioning

The API currently has no versioning. Future versions will use:
- URL versioning: `/api/v2/...`
- Header versioning: `API-Version: 2`

---

## Examples

### cURL

```bash
# Get overview
curl http://localhost:8080/api/overview

# List workers
curl http://localhost:8080/api/workers

# Get task
curl http://localhost:8080/api/tasks/abc-123

# Revoke task
curl -X POST http://localhost:8080/api/tasks/abc-123/revoke \
  -H "Content-Type: application/json" \
  -d '{"terminate": true}'

# Subscribe to events
curl -N http://localhost:8080/api/events
```

### JavaScript

```javascript
// Fetch overview
const response = await fetch('http://localhost:8080/api/overview');
const data = await response.json();
console.log(data);

// Subscribe to SSE
const eventSource = new EventSource('http://localhost:8080/api/events');
eventSource.onmessage = (event) => {
  const data = JSON.parse(event.data);
  console.log('Event:', data);
};
```

### Python

```python
import requests

# Get overview
response = requests.get('http://localhost:8080/api/overview')
data = response.json()
print(data)

# Revoke task
response = requests.post(
    'http://localhost:8080/api/tasks/abc-123/revoke',
    json={'terminate': True}
)
print(response.json())
```

---

## Support

For questions or issues:
- GitHub Issues: [turbine-queue/turbine](https://github.com/turbine-queue/turbine/issues)
- Discussions: [GitHub Discussions](https://github.com/turbine-queue/turbine/discussions)
