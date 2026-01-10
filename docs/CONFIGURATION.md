# Turbine Configuration Guide

Complete guide to configuring Turbine server, workers, and Python SDK.

## Server Configuration

### Configuration Methods

Turbine server can be configured via (in order of precedence):

1. Command-line arguments
2. Environment variables
3. Configuration file (TOML)
4. Default values

### Configuration File

Create `turbine.toml`:

```toml
[broker]
broker_type = "redis"
url = "redis://localhost:6379"
pool_size = 10
connection_timeout = 30
visibility_timeout = 1800  # 30 minutes
prefetch = 4

[backend]
backend_type = "redis"  # or "postgres", "s3"
# url = "redis://localhost:6379"  # Uses broker URL if not specified
result_ttl = 86400  # 24 hours
max_result_size = 1048576  # 1MB before S3 offload

[worker]
concurrency = 4
queues = ["default"]
heartbeat_interval = 30
max_tasks_per_worker = 0  # Unlimited
max_memory_mb = 0  # Unlimited
task_time_limit = 300
subprocess_isolation = false
shutdown_timeout = 60

[server]
host = "0.0.0.0"
grpc_port = 50051
rest_port = 8080
dashboard_port = 8081
enable_rest = true
enable_dashboard = true
max_request_size = 16777216  # 16MB

[queues.default]
priority = 0
rate_limit = 0  # No limit

[queues.high]
priority = 10
rate_limit = 0

[queues.low]
priority = -10
rate_limit = 100  # 100 tasks/sec

[logging]
level = "info"  # trace, debug, info, warn, error
format = "text"  # or "json"
timestamps = true

[telemetry]
prometheus_enabled = true
prometheus_port = 9090
otel_enabled = false
# otel_endpoint = "http://localhost:4317"
service_name = "turbine"

[security]
# TLS configuration
tls_enabled = false
tls_cert_path = "/path/to/cert.pem"
tls_key_path = "/path/to/key.pem"
# mTLS
mtls_enabled = false
mtls_ca_path = "/path/to/ca.pem"

[ratelimit]
enabled = true
default_rate = 1000  # requests per period
default_period = 60  # seconds
```

### Environment Variables

All configuration can be set via environment variables with `TURBINE_` prefix:

```bash
# Broker
export TURBINE_BROKER_URL=redis://localhost:6379
export TURBINE_BROKER_POOL_SIZE=10

# Backend
export TURBINE_BACKEND_TYPE=redis
export TURBINE_BACKEND_URL=redis://localhost:6379

# Server
export TURBINE_GRPC_PORT=50051
export TURBINE_REST_PORT=8080

# Logging
export TURBINE_LOG_LEVEL=info

# Telemetry
export TURBINE_PROMETHEUS_PORT=9090
```

### Command-Line Arguments

```bash
./turbine-server \
  --broker-url redis://localhost:6379 \
  --backend-url redis://localhost:6379 \
  --grpc-port 50051 \
  --rest-port 8080 \
  --log-level info
```

## Python Worker Configuration

### Worker Configuration

```python
from turbine.worker import Worker, WorkerConfig

config = WorkerConfig(
    broker_url="redis://localhost:6379",
    backend_url="redis://localhost:6379",
    queues=["default", "emails", "processing"],
    concurrency=4,
    prefetch_count=1,
    worker_id="worker-01",  # Auto-generated if not provided
    task_modules=["myapp.tasks", "myapp.other_tasks"],
    default_timeout=300,
    log_level="INFO",

    # DLQ configuration
    dlq_enabled=True,
    dlq_queue_name="turbine.dlq",
    dlq_ttl=604800,  # 7 days

    # Compression
    result_compression=True,
    result_compression_min_size=1024,  # 1KB
    result_compression_type="gzip",  # or "zlib", "brotli", "lz4"
)

worker = Worker(**vars(config))
worker.start()
```

### CLI Configuration

```bash
turbine worker \
  --broker-url redis://localhost:6379 \
  --backend-url redis://localhost:6379 \
  --queues default,emails \
  --concurrency 4 \
  --include myapp.tasks \
  --include myapp.other_tasks \
  --log-level INFO
```

### Django Management Command

```bash
python manage.py turbine_worker \
  --broker-url redis://localhost:6379 \
  --backend-url redis://localhost:6379 \
  -Q default,emails \
  -c 4 \
  -I myapp.tasks \
  --log-level INFO
```

## Python SDK Configuration

### Application Setup

```python
from turbine import Turbine

# Basic configuration
app = Turbine(
    server="localhost:50051",  # Turbine server address
    secure=False,  # Use TLS
    timeout=30.0,  # Default RPC timeout
)

# With TLS
app = Turbine(
    server="turbine.example.com:50051",
    secure=True,
    # credentials=grpc.ssl_channel_credentials()  # Custom credentials
)
```

### Django Settings

```python
# settings.py

INSTALLED_APPS = [
    ...
    'turbine.django',
]

# Turbine configuration
TURBINE_SERVER = "localhost:50051"
TURBINE_BROKER_URL = "redis://localhost:6379"
TURBINE_BACKEND_URL = "redis://localhost:6379"

# Optional: default tenant
TURBINE_TENANT_ID = os.environ.get('TENANT_ID', 'default')

# Optional: task modules for auto-discovery
TURBINE_TASK_MODULES = [
    'myapp.tasks',
    'another_app.tasks',
]

# Optional: worker configuration
TURBINE_WORKER_QUEUES = ["default", "emails"]
TURBINE_WORKER_CONCURRENCY = 4
```

### FastAPI Setup

```python
from fastapi import FastAPI
from turbine import Turbine

app = FastAPI()

# Initialize Turbine on startup
@app.on_event("startup")
async def startup():
    global turbine_app
    turbine_app = Turbine(server="localhost:50051")

@app.on_event("shutdown")
async def shutdown():
    turbine_app.client.close()
```

## Task Configuration

### Task Options

```python
from turbine import task

@task(
    name="custom_task_name",  # Default: module.function
    queue="emails",           # Default: "default"
    priority=5,               # Default: 0 (higher = more important)
    max_retries=3,           # Default: 3
    retry_delay=60,          # Default: 60 seconds (exponential backoff)
    timeout=300,             # Default: 300 seconds (hard timeout)
    soft_timeout=250,        # Default: None (warning timeout)
    store_result=True,       # Default: True
    result_ttl=86400,        # Default: 86400 (24 hours)
    bind=False,              # Default: False (pass task as first arg)
    tenant_id="acme-corp",   # Default: None (multi-tenancy)
)
def my_task(x, y):
    return x + y
```

### Runtime Options Override

```python
# Override options at submission time
result = my_task.apply_async(
    args=[1, 2],
    queue="high-priority",  # Override default queue
    priority=10,            # Higher priority
    countdown=60,           # Execute after 60 seconds
    eta=1704067200,        # Execute at specific timestamp
    expires=1704153600,    # Task expires at timestamp
    max_retries=5,         # Override retry count
    timeout=600,           # Override timeout
    tenant_id="other-tenant",  # Override tenant
    idempotency_key="unique-key",  # Prevent duplicates
    headers={"source": "api"},  # Custom metadata
)
```

## Backend Configuration

### Redis Backend

```python
# Default - built into worker
# No additional configuration needed
```

### S3 Backend

```python
from turbine.backends import S3Backend

backend = S3Backend(
    bucket="my-results-bucket",
    region="us-east-1",
    prefix="turbine/results/",
    # Optional: explicit credentials
    # aws_access_key_id="...",
    # aws_secret_access_key="...",
)

# Use with worker (custom implementation needed)
```

### PostgreSQL Backend

```python
from turbine.backends import PostgreSQLBackend

backend = PostgreSQLBackend(
    dsn="postgresql://user:password@localhost:5432/turbine",
    table_name="task_results",
    auto_create_table=True,
)

# Cleanup expired results (run periodically)
deleted = backend.cleanup_expired()
```

### Hybrid Backend

```python
from turbine.backends import HybridBackend

backend = HybridBackend(
    redis_url="redis://localhost:6379",
    s3_bucket="large-results",
    s3_region="us-east-1",
    size_threshold=1048576,  # 1MB threshold
)

# Results < 1MB → Redis (fast)
# Results ≥ 1MB → S3 (cost-effective)
```

## Multi-Tenancy Configuration

### Creating Tenants

```python
from turbine.tenancy import TenantManager, TenantQuotas

manager = TenantManager(backend_url="redis://localhost:6379")

# Free tier
free_quotas = TenantQuotas(
    max_tasks_per_hour=100,
    max_tasks_per_day=1000,
    max_concurrent_tasks=2,
    max_task_size_bytes=102400,  # 100KB
)

# Pro tier
pro_quotas = TenantQuotas(
    max_tasks_per_hour=10000,
    max_tasks_per_day=100000,
    max_concurrent_tasks=50,
    max_task_size_bytes=1048576,  # 1MB
)

# Create tenants
manager.create_tenant("free-customer", "Free Customer", quotas=free_quotas)
manager.create_tenant("pro-customer", "Pro Customer", quotas=pro_quotas)
```

### Tenant-Aware Tasks

```python
# Method 1: Fixed tenant
@task(tenant_id="acme-corp")
def tenant_specific_task():
    pass

# Method 2: Dynamic tenant
@task
def multi_tenant_task():
    pass

# Submit with tenant
multi_tenant_task.apply_async(
    args=[data],
    tenant_id=request.tenant_id  # From request context
)
```

## Security Configuration

### TLS/mTLS

**Server (turbine.toml):**

```toml
[security]
tls_enabled = true
tls_cert_path = "/etc/turbine/certs/server.crt"
tls_key_path = "/etc/turbine/certs/server.key"

# For mTLS (mutual authentication)
mtls_enabled = true
mtls_ca_path = "/etc/turbine/certs/ca.crt"
```

**Python Client:**

```python
import grpc
from turbine import Turbine

# Load credentials
with open('/path/to/ca.crt', 'rb') as f:
    ca_cert = f.read()

credentials = grpc.ssl_channel_credentials(
    root_certificates=ca_cert
)

app = Turbine(
    server="turbine.example.com:50051",
    secure=True,
    credentials=credentials,
)
```

### Redis Authentication

```bash
# With password
export TURBINE_BROKER_URL=redis://:password@localhost:6379

# With SSL
export TURBINE_BROKER_URL=rediss://localhost:6380
```

## Production Configuration Examples

### High-Throughput Configuration

```toml
[broker]
url = "redis://localhost:6379"
pool_size = 50  # Increased pool
prefetch = 10   # Fetch multiple tasks

[worker]
concurrency = 16  # More workers
queues = ["default", "high", "normal", "low"]

[backend]
result_ttl = 3600  # 1 hour (shorter retention)

[ratelimit]
enabled = true
default_rate = 10000
default_period = 60
```

### Low-Latency Configuration

```toml
[broker]
prefetch = 1  # Single task at a time
visibility_timeout = 300  # 5 minutes

[worker]
concurrency = 4
heartbeat_interval = 10  # Faster heartbeats

[backend]
# Use Redis for speed
backend_type = "redis"
```

### High-Reliability Configuration

```toml
[broker]
visibility_timeout = 3600  # 1 hour
prefetch = 1

[worker]
max_tasks_per_worker = 1000  # Restart after 1000 tasks
max_memory_mb = 1024  # Restart if memory > 1GB
shutdown_timeout = 120  # Graceful shutdown

[backend]
backend_type = "postgres"  # Durable storage
url = "postgresql://turbine:password@localhost/turbine"
result_ttl = 604800  # 7 days
```

### Multi-Tenant SaaS Configuration

```toml
[server]
enable_rest = true
enable_dashboard = true

[ratelimit]
enabled = true
default_rate = 1000
default_period = 3600  # Per hour

# Different queue priorities per tenant tier
[queues.enterprise]
priority = 10
rate_limit = 0  # No limit

[queues.pro]
priority = 5
rate_limit = 1000  # 1000/sec

[queues.free]
priority = 0
rate_limit = 10  # 10/sec
```

## Environment-Specific Configuration

### Development

```toml
[logging]
level = "debug"
format = "text"

[telemetry]
prometheus_enabled = false
otel_enabled = false

[worker]
concurrency = 2  # Low resource usage
```

### Staging

```toml
[logging]
level = "info"
format = "json"

[telemetry]
prometheus_enabled = true
otel_enabled = true
otel_endpoint = "http://jaeger:4317"

[worker]
concurrency = 4
```

### Production

```toml
[logging]
level = "warn"
format = "json"

[telemetry]
prometheus_enabled = true
otel_enabled = true
otel_endpoint = "http://otel-collector:4317"

[security]
tls_enabled = true
tls_cert_path = "/etc/turbine/tls/cert.pem"
tls_key_path = "/etc/turbine/tls/key.pem"

[worker]
concurrency = 8
max_memory_mb = 2048
```

## Queue Configuration

### Creating Queues

Queues are created automatically when tasks are submitted. Configure them in `turbine.toml`:

```toml
[queues.emails]
priority = 5
rate_limit = 100  # 100 emails/sec

[queues.webhooks]
priority = 8
rate_limit = 50

[queues.reports]
priority = 0
rate_limit = 10

[queues.background]
priority = -5
rate_limit = 0
```

### Queue Naming Conventions

Recommended patterns:

- **By Type**: `emails`, `webhooks`, `processing`
- **By Priority**: `high`, `medium`, `low`
- **By Tenant**: `tenant-{id}`, `customer-{id}`
- **By Partition**: `partition-0`, `partition-1`, ...
- **By Environment**: `prod-emails`, `staging-emails`

## Monitoring Configuration

### Prometheus

```toml
[telemetry]
prometheus_enabled = true
prometheus_port = 9090
```

Configure Prometheus scraping:

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'turbine'
    static_configs:
      - targets: ['localhost:9090']
    scrape_interval: 15s
```

### OpenTelemetry

```toml
[telemetry]
otel_enabled = true
otel_endpoint = "http://localhost:4317"
service_name = "turbine-production"
```

### Logging

```toml
[logging]
# JSON for production (structured logging)
level = "info"
format = "json"
timestamps = true
```

Example JSON log output:

```json
{
  "timestamp": "2024-01-09T12:00:00Z",
  "level": "INFO",
  "target": "turbine_server",
  "message": "Task completed",
  "task_id": "abc-123",
  "duration_ms": 1050
}
```

## Resource Limits

### Worker Limits

```toml
[worker]
# Memory limit - restart worker if exceeded
max_memory_mb = 1024

# Task limit - restart after N tasks (prevent memory leaks)
max_tasks_per_worker = 10000

# Time limits per task
task_time_limit = 300  # Hard limit
```

### Request Limits

```toml
[server]
# Maximum gRPC message size
max_request_size = 16777216  # 16MB

# Rate limiting
[ratelimit]
enabled = true
default_rate = 1000
default_period = 60
```

## Advanced Configuration

### Custom Queue Routing

```python
# In task definition
@task(queue=lambda *args, **kwargs: f"user-{kwargs['user_id'] % 10}")
def user_task(user_id, data):
    pass

# Partitions across user-0, user-1, ..., user-9
```

### Dynamic Worker Configuration

```python
import os
from turbine.worker import run_worker

# Scale based on environment
concurrency = int(os.environ.get('WORKER_CONCURRENCY', 4))
queues = os.environ.get('WORKER_QUEUES', 'default').split(',')

run_worker(
    broker_url=os.environ.get('TURBINE_BROKER_URL'),
    queues=queues,
    concurrency=concurrency,
)
```

## Troubleshooting

### Connection Issues

```
Error: Failed to connect to server
```

**Check:**
1. Server is running: `curl http://localhost:50051/health`
2. Firewall allows port 50051
3. Correct server address in configuration

### High Memory Usage

```toml
# Reduce memory usage
[worker]
concurrency = 2  # Fewer concurrent tasks
max_memory_mb = 512  # Auto-restart threshold
max_tasks_per_worker = 1000  # Periodic restart
```

### Slow Task Processing

```toml
# Increase throughput
[broker]
prefetch = 10  # Fetch more tasks
pool_size = 20  # More connections

[worker]
concurrency = 16  # More workers
```

### Result Storage Issues

```toml
# Use hybrid backend for large results
[backend]
backend_type = "hybrid"
size_threshold = 1048576  # 1MB
s3_bucket = "large-results"
```

## Best Practices

1. **Separate Queues by Criticality**: `critical`, `normal`, `background`
2. **Set Appropriate Timeouts**: Match to task duration
3. **Use Result TTL**: Clean up old results automatically
4. **Enable Monitoring**: Prometheus + Grafana in production
5. **Configure DLQ**: Capture and analyze failures
6. **Use Multi-Tenancy**: Isolate customers in SaaS
7. **Enable Compression**: For large results (>10KB)
8. **Set Worker Limits**: Prevent runaway resource usage
9. **Use TLS in Production**: Encrypt server communication
10. **Regular Backups**: Backup Redis/PostgreSQL data

## See Also

- [Multi-Tenancy Guide](./MULTI_TENANCY.md)
- [Dashboard API](./DASHBOARD_API.md)
- [Migration from Celery](./MIGRATION_FROM_CELERY.md)
