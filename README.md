# Turbine

**High-performance distributed task queue written in Rust**

Turbine is a modern alternative to Celery, designed to solve common pain points while providing first-class Python/Django support via gRPC API.

## Features

- **High Performance**: Rust workers with ~10x less memory than Python
- **Reliable**: Visibility timeout, automatic redelivery, dead letter queues
- **Observable**: Built-in Prometheus metrics, OpenTelemetry tracing, web dashboard
- **Flexible**: Support for Redis, RabbitMQ, and AWS SQS brokers
- **Workflows**: Chains, groups, and chords for complex task composition
- **Python SDK**: Easy integration with Django, FastAPI, and other frameworks

## Quick Start

### Using Docker Compose

```bash
# Start Turbine with Redis
docker-compose -f docker/docker-compose.yml up -d

# Check health
curl http://localhost:50051/health
```

### Building from Source

```bash
# Build all crates
cargo build --release

# Run server
./target/release/turbine-server

# Run worker (in another terminal)
./target/release/turbine-worker
```

## Configuration

Turbine can be configured via:
1. TOML configuration file (`turbine.toml`)
2. Environment variables (`TURBINE_*`)
3. CLI arguments

See `turbine.toml` for all available options.

### Environment Variables

```bash
# Broker
TURBINE_BROKER_URL=redis://localhost:6379

# Server
TURBINE_HOST=0.0.0.0
TURBINE_PORT=50051

# Worker
TURBINE_CONCURRENCY=4
TURBINE_QUEUES=default,high,low

# Logging
TURBINE_LOG_LEVEL=info
```

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
└─────────────────────────────────────────────────────────────────┘
                                │
        ┌───────────────────────┼───────────────────────┐
        ▼                       ▼                       ▼
┌───────────────┐     ┌───────────────┐     ┌───────────────┐
│     Redis     │     │   RabbitMQ    │     │   AWS SQS     │
└───────────────┘     └───────────────┘     └───────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Turbine Workers (Rust)                      │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │  Worker Pool (configurable concurrency per worker)       │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

## Crates

| Crate | Description |
|-------|-------------|
| `turbine-core` | Core types, traits, and configuration |
| `turbine-broker` | Message broker abstraction (Redis, RabbitMQ, SQS) |
| `turbine-backend` | Result backend abstraction (Redis, PostgreSQL, S3) |
| `turbine-server` | gRPC/REST server and task coordination |
| `turbine-worker` | Task execution engine |

## Python SDK (Coming Soon)

```python
from turbine import task, chain, group

@task(queue="emails", retry=3, timeout=300)
def send_email(to: str, subject: str, body: str):
    # Task logic here
    pass

# Simple task
send_email.delay(to="user@example.com", subject="Hello", body="World")

# Workflow
workflow = chain(
    fetch_data.s(url),
    process_data.s(),
    store_results.s()
)
workflow.delay()
```

## Celery Pain Points Addressed

| Celery Issue | Turbine Solution |
|--------------|------------------|
| High memory usage | Rust workers, ~10x less memory |
| GIL limits concurrency | True parallelism, no GIL |
| Task loss on crash | Visibility timeout + redelivery |
| Complex configuration | Sensible defaults, single config file |
| Poor monitoring | Built-in Prometheus + Dashboard |
| Slow cold start | Instant startup |

## License

MIT OR Apache-2.0
