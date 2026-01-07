# Contributing to Turbine

Thank you for your interest in contributing to Turbine! This document provides guidelines and information for contributors.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [How to Contribute](#how-to-contribute)
- [Pull Request Process](#pull-request-process)
- [Coding Standards](#coding-standards)
- [Testing](#testing)
- [Documentation](#documentation)
- [Community](#community)

## Code of Conduct

This project adheres to a [Code of Conduct](CODE_OF_CONDUCT.md). By participating, you are expected to uphold this code. Please report unacceptable behavior to the maintainers.

## Getting Started

### Prerequisites

- **Rust**: 1.75 or later ([install](https://rustup.rs/))
- **Redis**: 6.0 or later (for testing)
- **Docker** (optional): For running integration tests

### Development Setup

1. **Fork and clone the repository**
   ```bash
   git clone https://github.com/YOUR_USERNAME/turbine.git
   cd turbine
   ```

2. **Build the project**
   ```bash
   cargo build
   ```

3. **Run tests**
   ```bash
   cargo test
   ```

4. **Start Redis for integration tests**
   ```bash
   docker run -d -p 6379:6379 redis:7-alpine
   ```

5. **Run integration tests**
   ```bash
   cargo test -- --ignored
   ```

## How to Contribute

### Reporting Bugs

Before submitting a bug report:
- Check existing issues to avoid duplicates
- Collect information about the bug (version, OS, steps to reproduce)

When submitting:
- Use the bug report template
- Include a clear title and description
- Provide reproduction steps
- Include relevant logs or error messages

### Suggesting Features

- Check existing issues and discussions first
- Use the feature request template
- Explain the use case and expected behavior
- Consider if this fits the project's scope

### Good First Issues

Look for issues labeled [`good first issue`](https://github.com/turbine-queue/turbine/labels/good%20first%20issue) - these are beginner-friendly tasks perfect for getting started.

### Areas Where We Need Help

- **Documentation**: Improving docs, examples, tutorials
- **Testing**: Adding unit tests, integration tests
- **Brokers**: RabbitMQ and SQS implementations
- **Python SDK**: Building the `turbine-py` package
- **Benchmarks**: Performance testing and optimization
- **Dashboard**: Web UI for monitoring

## Pull Request Process

1. **Create a branch**
   ```bash
   git checkout -b feature/your-feature-name
   ```

2. **Make your changes**
   - Follow the coding standards
   - Add tests for new functionality
   - Update documentation if needed

3. **Test your changes**
   ```bash
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   ```

4. **Commit with a clear message**
   ```bash
   git commit -m "feat: add support for task priorities"
   ```

   We follow [Conventional Commits](https://www.conventionalcommits.org/):
   - `feat:` new feature
   - `fix:` bug fix
   - `docs:` documentation changes
   - `test:` adding tests
   - `refactor:` code refactoring
   - `perf:` performance improvements
   - `chore:` maintenance tasks

5. **Push and create a PR**
   ```bash
   git push origin feature/your-feature-name
   ```

6. **PR Review**
   - Respond to feedback promptly
   - Make requested changes
   - Keep the PR focused and reasonably sized

### PR Checklist

- [ ] Code follows the project's style guidelines
- [ ] Tests pass locally (`cargo test`)
- [ ] Clippy passes (`cargo clippy -- -D warnings`)
- [ ] Code is formatted (`cargo fmt`)
- [ ] Documentation is updated (if applicable)
- [ ] CHANGELOG.md is updated (for significant changes)

## Coding Standards

### Rust Style

- Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Write documentation comments for public APIs

```rust
/// Submits a task to the specified queue.
///
/// # Arguments
///
/// * `queue` - The target queue name
/// * `task` - The task to submit
///
/// # Returns
///
/// The task ID on success
///
/// # Errors
///
/// Returns an error if the broker is unavailable
pub async fn submit(&self, queue: &str, task: Task) -> Result<TaskId> {
    // implementation
}
```

### Error Handling

- Use `thiserror` for error types
- Provide meaningful error messages
- Don't panic in library code

### Async Code

- Use `tokio` for async runtime
- Prefer `async-trait` for async trait methods
- Handle cancellation gracefully

## Testing

### Unit Tests

Place unit tests in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_creation() {
        let task = Task::new("my_task");
        assert_eq!(task.name, "my_task");
    }

    #[tokio::test]
    async fn test_async_operation() {
        // async test
    }
}
```

### Integration Tests

Place integration tests in `tests/` directory. Tests requiring Redis should be marked with `#[ignore]`:

```rust
#[tokio::test]
#[ignore] // Requires Redis
async fn test_redis_broker() {
    // test code
}
```

Run ignored tests with:
```bash
cargo test -- --ignored
```

## Documentation

- Update README.md for user-facing changes
- Add doc comments for all public APIs
- Update CHANGELOG.md for significant changes
- Add examples in `examples/` directory

### Building Docs

```bash
cargo doc --open
```

## Project Structure

```
turbine/
├── crates/
│   ├── turbine-core/      # Core types and traits
│   ├── turbine-broker/    # Message broker implementations
│   ├── turbine-backend/   # Result backend implementations
│   ├── turbine-server/    # gRPC server
│   └── turbine-worker/    # Task execution engine
├── proto/                 # Protocol buffer definitions
├── turbine-py/           # Python SDK (future)
├── examples/             # Example applications
└── docker/               # Docker configurations
```

## Community

- **Discussions**: Use GitHub Discussions for questions
- **Issues**: Use GitHub Issues for bugs and features
- **Discord**: [Join our Discord](#) (coming soon)

## Recognition

Contributors will be recognized in:
- CHANGELOG.md for their contributions
- README.md contributors section
- Release notes

## Questions?

Feel free to:
- Open a GitHub Discussion
- Ask in an existing issue
- Reach out to maintainers

Thank you for contributing to Turbine!
