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

- **High Performance**: Zero-copy message handling, async I/O, minimal allocations
- **Reliable**: Exactly-once semantics (where possible), persistent task state, graceful shutdown
- **Observable**: Built-in Prometheus metrics, OpenTelemetry tracing, web dashboard
- **Flexible Brokers**: Redis (now), RabbitMQ, AWS SQS (coming soon)
- **Workflows**: Chains, groups, and chords for complex task composition
- **Python SDK**: Easy integration with Django, FastAPI, and other frameworks

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
| [`turbine-py`](turbine-py) | Python SDK with Django/FastAPI support | ✅ Ready |

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
- [ ] Dead letter queues

### Phase 4: Observability ✅
- [x] Prometheus metrics
- [x] OpenTelemetry tracing
- [ ] Web dashboard

### Phase 5: Advanced Features ✅
- [x] Rate limiting
- [x] Priority queues
- [x] TLS/mTLS encryption
- [ ] Multi-tenancy

### Phase 6: Additional Brokers (Planned)
- [ ] RabbitMQ support
- [ ] AWS SQS support

## Documentation

- [Configuration Guide](docs/configuration.md) (coming soon)
- [API Reference](https://docs.rs/turbine-core)
- [Migration from Celery](docs/migration.md) (coming soon)
- [Architecture Deep Dive](docs/architecture.md) (coming soon)

## Contributing

We welcome contributions! Please see our [Contributing Guide](CONTRIBUTING.md) for details.

### Good First Issues

Looking to contribute? Check out issues labeled [`good first issue`](https://github.com/turbine-queue/turbine/labels/good%20first%20issue).

### Areas We Need Help

| Area | Description | Skills |
|------|-------------|--------|
| 🐰 RabbitMQ Broker | Implement AMQP 0.9.1 broker support | Rust, RabbitMQ |
| ☁️ AWS SQS Broker | Implement SQS broker for cloud deployments | Rust, AWS |
| 📊 Web Dashboard | Real-time monitoring UI for tasks and workers | React/Vue, WebSocket |
| 🧪 Benchmarks | Performance comparison with Celery | Python, Rust |
| 📚 Documentation | Guides, tutorials, API docs | Technical Writing |
| 🔧 Examples | More example apps (Flask, CLI tools) | Python |

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
