# Changelog

All notable changes to turbine-queue will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2025-01-07

### Added
- Initial release of Turbine Python SDK
- Core `Turbine` client class for connecting to Turbine server
- `@task` decorator for defining tasks
- `AsyncResult` class for tracking task results
- Workflow primitives: `chain`, `group`, `chord`
- Django integration with `turbine.django` module
  - Django app configuration
  - Management commands: `turbine_status`, `turbine_purge`
  - Auto-discovery of tasks from Django apps
- FastAPI integration with `turbine.fastapi` module
  - `TurbineExtension` for lifespan management
  - Dependency injection support
- CLI tool for task management
  - `turbine generate-proto` - Generate gRPC stubs
  - `turbine health` - Check server health
  - `turbine queues` - List queue information
  - `turbine submit` - Submit tasks
  - `turbine status` - Check task status
- gRPC-based communication with Turbine server
- Type hints throughout the codebase
- Comprehensive documentation and examples

[Unreleased]: https://github.com/turbine-queue/turbine/compare/python-v0.1.0...HEAD
[0.1.0]: https://github.com/turbine-queue/turbine/releases/tag/python-v0.1.0
