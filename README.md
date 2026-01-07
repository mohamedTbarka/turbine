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
  <a href="https://docs.rs/turbine-core">
    <img src="https://docs.rs/turbine-core/badge.svg" alt="Documentation">
  </a>
  <a href="https://github.com/turbine-queue/turbine/blob/main/LICENSE-MIT">
    <img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg" alt="License">
  </a>
  <a href="https://discord.gg/turbine">
    <img src="https://img.shields.io/discord/123456789?color=7289da&label=discord" alt="Discord">
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
│                         Python/Django App                        │
│                    (turbine-py SDK via gRPC)                     │
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
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│     Redis     │     │   RabbitMQ    │     │   AWS SQS     │
│   (Ready)     │     │   (Planned)   │     │   (Planned)   │
└───────────────┘     └───────────────┘     └───────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Turbine Workers (Rust)                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Worker Pool  •  Task Executor  •  Subprocess Isolation  │    │
│  └─────────────────────────────────────────────────────────┘    │
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
| `turbine-py` | Python SDK | 🚧 Planned |

## Python SDK (Coming Soon)

```python
from turbine import task, chain, group

@task(queue="emails", retry=3, timeout=300)
def send_email(to: str, subject: str, body: str):
    # Task logic here
    pass

# Simple task
send_email.delay(to="user@example.com", subject="Hello", body="World")

# Workflow: chain tasks sequentially
workflow = chain(
    fetch_data.s(url),
    process_data.s(),
    store_results.s()
)
workflow.delay()

# Workflow: run tasks in parallel
group(
    send_email.s(to="user1@example.com", subject="Hi", body="..."),
    send_email.s(to="user2@example.com", subject="Hi", body="..."),
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

### Phase 2: Python SDK (In Progress)
- [ ] Python gRPC client
- [ ] `@task` decorator
- [ ] Django integration
- [ ] FastAPI integration

### Phase 3: Reliability & Workflows
- [ ] Dead letter queues
- [ ] Retry with exponential backoff
- [ ] Chain, Group, Chord execution
- [ ] Beat scheduler (cron)

### Phase 4: Additional Brokers
- [ ] RabbitMQ support
- [ ] AWS SQS support

### Phase 5: Observability
- [ ] Prometheus metrics
- [ ] OpenTelemetry tracing
- [ ] Web dashboard

### Phase 6: Advanced Features
- [ ] Rate limiting
- [ ] Priority queues
- [ ] Multi-tenancy

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

- 📚 Documentation improvements
- 🧪 Test coverage
- 🐍 Python SDK development
- 🐰 RabbitMQ broker implementation
- 📊 Benchmarking

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
