# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial project structure with Cargo workspace
- `turbine-core`: Core types and traits
  - Task, TaskId, TaskState, TaskMeta, TaskResult types
  - Message serialization (JSON and MessagePack)
  - Workflow types: Chain, Group, Chord
  - Configuration system with TOML support
  - Error types with comprehensive error handling
- `turbine-broker`: Message broker abstraction
  - Broker trait for pluggable backends
  - Redis implementation with reliable queue pattern (BRPOPLPUSH)
  - Delayed task scheduling via sorted sets
  - Visibility timeout and automatic redelivery
  - Task revocation support
- `turbine-backend`: Result storage abstraction
  - Backend trait for pluggable storage
  - Redis implementation with TTL support
  - Task state tracking
  - Async result waiting
- `turbine-server`: Central coordinator
  - gRPC API definitions (proto files)
  - Task submission and routing
  - Workflow orchestration (chains, groups, chords)
  - Health check endpoint
- `turbine-worker`: Task execution engine
  - Worker pool with configurable concurrency
  - Task executor with timeout support
  - Graceful shutdown handling
  - Built-in task handlers (echo, sleep, fail)
  - Task registry for custom handlers
- Docker support
  - Dockerfile for server
  - Dockerfile for worker
  - docker-compose.yml for local development
- Documentation
  - README with architecture overview
  - Configuration file examples
  - API documentation

### Infrastructure
- GitHub Actions CI workflow
- Issue templates (bug report, feature request)
- Contributing guidelines
- Code of Conduct

## [0.1.0] - TBD

### Planned
- Python SDK (`turbine-py`)
- Django integration
- FastAPI integration
- RabbitMQ broker support
- AWS SQS broker support
- PostgreSQL backend support
- S3 backend for large results
- Web dashboard
- Prometheus metrics
- OpenTelemetry tracing

---

## Release Notes Format

### Added
For new features.

### Changed
For changes in existing functionality.

### Deprecated
For soon-to-be removed features.

### Removed
For now removed features.

### Fixed
For any bug fixes.

### Security
In case of vulnerabilities.
