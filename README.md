<p align="center">
  <h1 align="center">Turbine</h1>
  <p align="center">
    <strong>High-performance distributed task queue written in Rust</strong>
  </p>
  <p align="center">
    A modern, reliable alternative to Celery with first-class Python support
  </p>
</p>

<p align="center">
  <a href="https://github.com/turbine-queue/turbine/actions/workflows/ci.yml">
    <img src="https://github.com/turbine-queue/turbine/actions/workflows/ci.yml/badge.svg" alt="CI Status">
  </a>
  <a href="https://crates.io/crates/turbine-core">
    <img src="https://img.shields.io/crates/v/turbine-core.svg" alt="Crates.io">
  </a>
  <a href="https://pypi.org/project/turbine-queue/">
    <img src="https://img.shields.io/pypi/v/turbine-queue.svg" alt="PyPI">
  </a>
  <a href="https://pypi.org/project/turbine-queue/">
    <img src="https://img.shields.io/pypi/pyversions/turbine-queue.svg" alt="Python Versions">
  </a>
  <a href="https://github.com/turbine-queue/turbine/blob/main/LICENSE-MIT">
    <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License">
  </a>
</p>

<p align="center">
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#documentation">Documentation</a> •
  <a href="#benchmarks">Benchmarks</a> •
  <a href="#roadmap">Roadmap</a> •
  <a href="#contributing">Contributing</a>
</p>

---

## Why Turbine?

Turbine was built to solve the common pain points of Celery while maintaining a familiar API for Python developers:

| Celery Pain Point | Turbine Solution |
|-------------------|------------------|
| High memory usage (Python workers) | Rust workers use ~10x less memory |
| GIL limits concurrency | True parallelism with no GIL |
| Task loss on worker crash | Visibility timeout + automatic redelivery |
| Complex configuration | Sensible defaults, single config file |
| Poor monitoring | Built-in Prometheus metrics + Dashboard |
| Slow cold start | Instant startup, no Python import overhead |
| Result backend reliability | Optional results with TTL, S3 offload |

## Features

- **High Performance**: Zero-copy message handling, async I/O, minimal allocations, result compression
- **Reliable**: Dead Letter Queue (DLQ), retry with exponential backoff, persistent task state
- **Observable**: Prometheus metrics, OpenTelemetry tracing, REST API, Grafana dashboards
- **Flexible Brokers**: Redis (ready), RabbitMQ, AWS SQS (coming soon)
- **Workflows**: Chains, groups, chords, and batch processing utilities
- **Python SDK**: Easy integration with Django, FastAPI, and other frameworks
- **Multi-Tenancy**: Resource quotas, usage tracking, tenant isolation
- **Celery Compatible**: Easy migration with familiar API

## Quick Start

### Using Docker Compose

```bash
# Clone the repository
git clone https://github.com/turbine-queue/turbine.git
cd turbine

# Start Turbine with Redis
docker-compose -f docker/docker-compose.yml up -d

# Check health
curl http://localhost:50051/health
```

### Building from Source

```bash
# Prerequisites: Rust 1.75+, Redis

# Build all crates
cargo build --release

# Run server
./target/release/turbine-server

# Run worker (in another terminal)
./target/release/turbine-worker
```

### Configuration

Turbine can be configured via TOML file, environment variables, or CLI arguments:

```bash
# Using environment variables
export TURBINE_BROKER_URL=redis://localhost:6379
export TURBINE_CONCURRENCY=4
./target/release/turbine-worker

# Using config file
./target/release/turbine-server --config turbine.toml
```

See [`turbine.toml`](turbine.toml) for all configuration options.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    Python/Django/FastAPI App                     │
│                     (turbine-py SDK via gRPC)                    │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Turbine Server (Rust)                       │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────────┐  │
│  │  gRPC API   │  │  REST API   │  │   Web Dashboard         │  │
│  └─────────────┘  └─────────────┘  └─────────────────────────┘  │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  Task Router  •  Workflow Engine  •  Scheduler (Beat)     │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Message Broker                            │
│     Redis (Ready)  │  RabbitMQ (Planned)  │  AWS SQS (Planned)  │
└─────────────────────────────────────────────────────────────────┘
                                │
        ┌───────────────────────┴───────────────────────┐
        ▼                                               ▼
┌─────────────────────────────┐     ┌─────────────────────────────┐
│   Turbine Workers (Rust)    │     │   Python Workers            │
│  • High-performance tasks   │     │  • Django/FastAPI tasks     │
│  • Subprocess isolation     │     │  • Native Python execution  │
│  • Memory efficient         │     │  • Auto-discovery           │
└─────────────────────────────┘     └─────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        Result Backend                            │
│              Redis  │  PostgreSQL (Planned)  │  S3               │
└─────────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Description | Status |
|-------|-------------|--------|
| [`turbine-core`](crates/turbine-core) | Core types, traits, configuration | ✅ Ready |
| [`turbine-broker`](crates/turbine-broker) | Message broker abstraction | ✅ Redis |
| [`turbine-backend`](crates/turbine-backend) | Result backend abstraction | ✅ Redis |
| [`turbine-server`](crates/turbine-server) | gRPC/REST server | ✅ Ready |
| [`turbine-worker`](crates/turbine-worker) | Task execution engine | ✅ Ready |
| [`turbine-dashboard`](crates/turbine-dashboard) | Web dashboard backend (API) | ✅ Ready |
| [`turbine-py`](turbine-py) | Python SDK with Django/FastAPI support | ✅ Ready |

## Examples

| Example | Description | Features |
|---------|-------------|----------|
| [`examples/django-app`](examples/django-app) | Django integration | Tasks, views, management commands |
| [`examples/fastapi-app`](examples/fastapi-app) | FastAPI integration | API endpoints, background tasks |
| [`examples/python`](examples/python) | Basic Python usage | Simple task submission |
| [`examples/advanced`](examples/advanced) | Advanced patterns | Batch, DAG, routing, multi-tenancy |

## Python SDK

Install the Python SDK:

```bash
pip install turbine-queue

# With worker dependencies (for running Python workers)
pip install turbine-queue[worker]

# With Django integration
pip install turbine-queue[django]
```

### Basic Usage

```python
from turbine import Turbine, task

# Initialize client
turbine = Turbine(server="localhost:50051")

# Define a task
@task(queue="emails", max_retries=3, timeout=300)
def send_email(to: str, subject: str, body: str):
    # Task logic here
    pass

# Submit task
task_id = send_email.delay(to="user@example.com", subject="Hello", body="World")

# Check status
status = turbine.get_task_status(task_id)
print(f"Task state: {status['state']}")
```

### Django Integration

```python
# settings.py
INSTALLED_APPS = [
    ...
    'turbine.django',
]

TURBINE_SERVER = "localhost:50051"
TURBINE_BROKER_URL = "redis://localhost:6379"

# tasks.py
from turbine import task

@task(queue="emails")
def send_welcome_email(user_id: int):
    user = User.objects.get(id=user_id)
    # Send email...

# views.py
from .tasks import send_welcome_email

def signup(request):
    user = create_user(request.POST)
    send_welcome_email.delay(user_id=user.id)
    return redirect("home")
```

Run the Python worker:

```bash
# Using CLI
turbine worker --broker-url redis://localhost:6379 -I myapp.tasks

# Using Django management command
python manage.py turbine_worker -Q emails,default
```

### Workflows

```python
from turbine import chain, group, chord

# Chain: tasks run sequentially, passing results
workflow = chain(
    fetch_data.s(url),
    process_data.s(),
    store_results.s()
)
workflow.delay()

# Group: tasks run in parallel
group(
    send_email.s(to="user1@example.com", subject="Hi", body="..."),
    send_email.s(to="user2@example.com", subject="Hi", body="..."),
).delay()

# Chord: group + callback after all complete
chord(
    [process_chunk.s(chunk) for chunk in chunks],
    aggregate_results.s()
).delay()
```

### Dead Letter Queue (DLQ)

Failed tasks that exceed retry limits are automatically sent to the DLQ for inspection:

```bash
# List failed tasks
turbine dlq list

# Show DLQ statistics
turbine dlq stats

# Inspect a specific failed task
turbine dlq inspect <task-id>

# Remove a task from DLQ
turbine dlq remove <task-id>

# Clear all tasks from DLQ
turbine dlq clear --force
```

### Multi-Tenancy

Isolate tasks and enforce quotas per tenant:

```bash
# Create tenant
turbine tenant create acme-corp "ACME Corporation"

# List tenants
turbine tenant list

# Get tenant stats
turbine tenant stats acme-corp
```

```python
from turbine import task

# Task assigned to specific tenant
@task(queue="emails", tenant_id="acme-corp")
def send_tenant_email(to: str, subject: str):
    pass

# Submit with tenant override
process_data.apply_async(
    args=[data],
    tenant_id="acme-corp"
)
```

Configure quotas per tenant:

```python
from turbine.tenancy import TenantManager, TenantQuotas

manager = TenantManager()
quotas = TenantQuotas(
    max_tasks_per_hour=1000,
    max_concurrent_tasks=50,
    max_queue_length=500,
)

tenant = manager.create_tenant(
    tenant_id="acme-corp",
    name="ACME Corporation",
    quotas=quotas
)
```

See [Multi-Tenancy Guide](docs/MULTI_TENANCY.md) for details.

### Batch Processing

Efficiently process large datasets with batch utilities:

```python
from turbine import task
from turbine.batch import BatchProcessor, Batcher, batch_map

@task(queue="processing")
def process_item(item):
    # Process single item
    return item * 2

# Simple batch processing
results = batch_map(process_item, items, batch_size=100)

# Advanced batch processing with progress
processor = BatchProcessor(
    task=process_item,
    chunk_size=100,
    max_concurrent=10,
    on_progress=lambda done, total: print(f"{done}/{total}"),
)
results = processor.map(items)

# Batch accumulator (auto-submit when full)
with Batcher(process_item, batch_size=100) as batcher:
    for item in large_dataset:
        batcher.add(item)
    # Auto-submits on exit
```

### Result Compression

Automatic compression for large task results:

```python
# Worker automatically compresses results > 1KB
# Supports: gzip, zlib, brotli, lz4

from turbine.compression import Compressor, CompressionType

# Auto-compression with size threshold
compressor = Compressor(CompressionType.GZIP)
compressed, comp_type = compressor.auto_compress(data, min_size=1024)

# Worker config
# Results > 1KB automatically compressed
```

### Task Dependencies (DAG)

Build complex task dependency graphs:

```python
from turbine import task
from turbine.dag import DAG, parallel

@task
def fetch_data(source):
    return data

@task
def process_data(data):
    return processed

@task
def store_data(processed):
    return success

# Build dependency graph
dag = DAG("data-pipeline")
fetch_id = dag.add_task(fetch_data, args=["api"])
process_id = dag.add_task(process_data, dependencies=[fetch_id])
store_id = dag.add_task(store_data, dependencies=[process_id])

# Execute with proper ordering
results = dag.execute(wait=True)

# Or simple parallel execution
results = parallel(
    task1.s(arg1),
    task2.s(arg2),
    task3.s(arg3),
    wait=True
)
```

### Load Balancing & Routing

Smart task routing across queues:

```python
from turbine.routing import LoadBalancer, TaskRouter, RoutingStrategy, consistent_hash_router

# Load balance across queues
balancer = LoadBalancer(app, queues=["q1", "q2", "q3"])
result = balancer.route_task(my_task, args=[data], strategy="least_loaded")

# Consistent hashing for partitioning
queue = consistent_hash_router(
    my_task,
    partition_key=f"user:{user_id}",
    num_queues=8
)

# Round-robin routing
router = TaskRouter(queues=["q1", "q2", "q3"], strategy=RoutingStrategy.ROUND_ROBIN)
queue = router.route()
```

### Alternative Result Backends

Store results in S3 for large payloads:

```python
from turbine.backends import S3Backend, HybridBackend

# S3 backend
s3_backend = S3Backend(
    bucket="my-results-bucket",
    region="us-east-1"
)

# Hybrid: Redis for small, S3 for large (>1MB)
backend = HybridBackend(
    redis_url="redis://localhost:6379",
    s3_bucket="my-results-bucket",
    size_threshold=1048576  # 1MB
)
```

## Web Dashboard

Turbine includes a comprehensive REST API for real-time monitoring and management:

### Starting the Dashboard

```bash
# Build the dashboard
cargo build --release -p turbine-dashboard

# Run with default settings
./target/release/turbine-dashboard

# Custom configuration
./target/release/turbine-dashboard \
  --host 0.0.0.0 \
  --port 8080 \
  --redis-url redis://localhost:6379
```

### API Endpoints

The dashboard provides the following REST endpoints:

**Health & Overview:**
- `GET /api/health` - Health check
- `GET /api/overview` - Dashboard overview statistics

**Workers:**
- `GET /api/workers` - List all workers
- `GET /api/workers/:id` - Get worker details

**Queues:**
- `GET /api/queues` - List all queues
- `GET /api/queues/:name` - Get queue details
- `GET /api/queues/:name/stats` - Queue statistics
- `POST /api/queues/:name/purge` - Purge queue

**Tasks:**
- `GET /api/tasks` - List recent tasks
- `GET /api/tasks/:id` - Get task details
- `POST /api/tasks/:id/revoke` - Revoke a task

**Dead Letter Queue:**
- `GET /api/dlq/:queue` - Get DLQ info
- `POST /api/dlq/:queue/reprocess` - Reprocess DLQ messages
- `POST /api/dlq/:queue/purge` - Purge DLQ

**Metrics & Events:**
- `GET /api/metrics` - Prometheus metrics
- `GET /api/events` - Server-Sent Events for real-time updates

### Example API Usage

```bash
# Get overview
curl http://localhost:8080/api/overview

# List queues
curl http://localhost:8080/api/queues

# Get task status
curl http://localhost:8080/api/tasks/task-id-here

# Listen to real-time events
curl -N http://localhost:8080/api/events
```

**Frontend UI:** Coming soon! The backend API is complete and ready for a React/Vue/Svelte frontend.

## Monitoring with Grafana

Turbine provides ready-to-use Grafana dashboards:

```bash
# Import dashboards
cd grafana/
# Import turbine-overview.json into Grafana

# Configure Prometheus scraping
# See grafana/README.md for full setup
```

**Included Dashboards:**
- Task throughput and latency
- Queue depths and consumers
- Worker statistics
- Success/failure rates
- DLQ monitoring

See [Grafana Setup Guide](grafana/README.md) for details.

## Migrating from Celery

Turbine provides a Celery-compatible API for easy migration:

```python
# Most Celery code works with minimal changes
# from celery import Celery, task
from turbine import Turbine, task

# app = Celery('myapp', broker='redis://...')
app = Turbine(server='localhost:50051')

# Tasks work the same way!
@task(queue="emails", max_retries=3)
def send_email(to, subject, body):
    pass

send_email.delay("user@example.com", "Hello", "World")
```

See [Migration Guide](docs/MIGRATION_FROM_CELERY.md) for complete migration steps.

## Benchmarks

Coming soon! We're working on comprehensive benchmarks comparing:

- Memory usage vs Celery
- Throughput (tasks/second)
- Latency (p50, p95, p99)
- Cold start time

## Roadmap

### Phase 1: Core Foundation ✅
- [x] Task types and serialization
- [x] Redis broker implementation
- [x] Redis result backend
- [x] Basic worker with task execution
- [x] gRPC server structure

### Phase 2: Python SDK ✅
- [x] Python gRPC client
- [x] `@task` decorator
- [x] Django integration
- [x] FastAPI integration
- [x] Python worker for executing tasks

### Phase 3: Reliability & Workflows ✅
- [x] Retry with exponential backoff
- [x] Chain, Group, Chord execution
- [x] Beat scheduler (cron)
- [x] Dead letter queues (DLQ)

### Phase 4: Observability
- [x] Prometheus metrics
- [x] OpenTelemetry tracing
- [x] Web dashboard backend (REST API + SSE)
- [ ] Web dashboard frontend (UI)

### Phase 5: Advanced Features ✅
- [x] Rate limiting
- [x] Priority queues
- [x] TLS/mTLS encryption
- [x] Multi-tenancy with quotas and usage tracking

### Phase 6: Additional Brokers (Planned)
- [ ] RabbitMQ support
- [ ] AWS SQS support
- [ ] Kafka support

### Phase 7: Optimization & Tools
- [x] Task result compression (gzip, zlib, brotli, lz4)
- [x] Batch processing utilities (BatchProcessor, Batcher)
- [x] Task dependencies and DAG execution
- [x] Load balancing strategies (round-robin, hash, least-loaded)
- [x] Result backend: S3 for large payloads
- [x] Result backend: Hybrid (Redis + S3)
- [x] Grafana dashboard templates
- [x] Migration guide from Celery
- [x] Advanced examples (batch, DAG, routing)
- [ ] Dashboard web UI (React/Vue/Svelte)
- [ ] Result backend: PostgreSQL

## Documentation

### Guides
- [Migration from Celery](docs/MIGRATION_FROM_CELERY.md) ✅
- [Multi-Tenancy Guide](docs/MULTI_TENANCY.md) ✅
- [Dashboard API Reference](docs/DASHBOARD_API.md) ✅
- [Dashboard Frontend Proposal](docs/DASHBOARD_PROPOSAL.md) ✅
- [Grafana Setup](grafana/README.md) ✅

### Coming Soon
- Configuration Guide
- Task Best Practices
- Workflow Patterns
- Performance Tuning
- Security Guide

### API Reference
- [Rust Crates Documentation](https://docs.rs/turbine-core)
- Python SDK: See docstrings in source code

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Good First Issues

Looking to contribute? Check out issues labeled [`good first issue`](https://github.com/turbine-queue/turbine/labels/good%20first%20issue).

### Areas We Need Help

| Area | Description | Skills |
|------|-------------|--------|
| 🎨 Dashboard Frontend | Build React/Vue/Svelte UI consuming the REST API | TypeScript, React/Vue, SSE |
| 🐰 RabbitMQ Broker | Implement AMQP 0.9.1 broker support | Rust, RabbitMQ |
| ☁️ AWS SQS Broker | Implement SQS broker for cloud deployments | Rust, AWS SDK |
| 🧪 Benchmarks | Performance comparison with Celery (memory, throughput, latency) | Python, Rust, Testing |
| 🏢 Multi-tenancy | Add tenant isolation and resource quotas | Rust, Distributed Systems |
| 📚 Documentation | Migration guides, best practices, tutorials | Technical Writing |
| 🔧 Examples | More example apps (Flask, Sanic, CLI tools) | Python |

### Tech Stack

- **Backend**: Rust (Tokio, Tonic gRPC, Serde)
- **Broker**: Redis (RabbitMQ/SQS planned)
- **Python SDK**: Python 3.9+, gRPC, MessagePack
- **Frameworks**: Django, FastAPI

## Community

- [GitHub Discussions](https://github.com/turbine-queue/turbine/discussions)
- [Discord](https://discord.gg/turbine) (coming soon)
- [Twitter](https://twitter.com/turbine_queue) (coming soon)

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Acknowledgments

Turbine is inspired by:
- [Celery](https://github.com/celery/celery) - The original Python task queue
- [Sidekiq](https://github.com/mperham/sidekiq) - Ruby's excellent background job processor
- [Tokio](https://tokio.rs/) - Rust's async runtime

---

<p align="center">
  <sub>Built with ❤️ in Rust</sub>
</p>
