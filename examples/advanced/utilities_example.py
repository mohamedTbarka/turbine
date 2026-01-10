"""
Advanced example: Using Turbine Utility Features

This example demonstrates:
- Result caching
- Advanced retry strategies
- Webhook notifications
- Health monitoring
- Result export
"""

from turbine import Turbine, task
from turbine.cache import cached_task, ResultCache
from turbine.retry import retry, RetryStrategy, CircuitBreaker
from turbine.webhooks import WebhookManager, WebhookEvent, on_task_complete
from turbine.monitoring import HealthChecker, TaskMonitor, create_health_endpoint
from turbine.export import ResultExporter, DLQExporter

# Initialize
app = Turbine(server="localhost:50051")


# Example 1: Result Caching
# =========================

@task(queue="expensive")
@cached_task(ttl=3600)  # Cache for 1 hour
def expensive_computation(n: int) -> int:
    """Expensive computation that benefits from caching."""
    import time
    print(f"Computing (expensive)... n={n}")
    time.sleep(2)  # Simulate expensive operation

    result = sum(range(n))
    return result


def example_caching():
    print("=" * 60)
    print("Example 1: Result Caching")
    print("=" * 60)

    # First call - executes task
    print("\n1st call (cache miss):")
    import time
    start = time.time()
    result1 = expensive_computation.delay(1000).get(timeout=30)
    duration1 = time.time() - start
    print(f"  Result: {result1}")
    print(f"  Duration: {duration1:.2f}s")

    # Second call - returns cached result
    print("\n2nd call (cache hit):")
    start = time.time()
    result2 = expensive_computation.delay(1000).get(timeout=30)
    duration2 = time.time() - start
    print(f"  Result: {result2}")
    print(f"  Duration: {duration2:.2f}s")

    print(f"\n✓ Speedup: {duration1/duration2:.1f}x faster with cache")


# Example 2: Advanced Retry
# =========================

@task(queue="api")
@retry(max_attempts=5, strategy=RetryStrategy.EXPONENTIAL)
def flaky_api_call(attempt_id: int) -> dict:
    """API call that might fail."""
    import random

    print(f"Attempting API call #{attempt_id}")

    if random.random() < 0.7:  # 70% failure rate
        raise Exception("API temporarily unavailable")

    return {"success": True, "attempt": attempt_id}


def example_retry():
    print("\n" + "=" * 60)
    print("Example 2: Advanced Retry Strategies")
    print("=" * 60)

    print("\nCalling flaky API (70% failure rate, auto-retry)...")

    try:
        result = flaky_api_call(1)
        print(f"✓ Success after retries: {result}")
    except Exception as e:
        print(f"✗ Failed after all retries: {e}")


# Example 3: Circuit Breaker
# ===========================

breaker = CircuitBreaker(failure_threshold=3, timeout=10)


@task(queue="external")
def call_external_service(service_id: int) -> dict:
    """Call external service with circuit breaker."""
    import random

    @breaker.call
    def _call():
        if random.random() < 0.5:
            raise Exception("Service unavailable")
        return {"status": "ok"}

    return _call()


def example_circuit_breaker():
    print("\n" + "=" * 60)
    print("Example 3: Circuit Breaker Pattern")
    print("=" * 60)

    print("\nCalling external service (50% failure rate)...")
    print(f"Circuit breaker state: {breaker.state}\n")

    for i in range(1, 11):
        try:
            result = call_external_service.delay(i).get(timeout=10)
            print(f"  Call {i}: SUCCESS - {breaker.state}")
        except Exception as e:
            print(f"  Call {i}: FAILED - {breaker.state} - {str(e)[:50]}")

    print(f"\n✓ Circuit breaker final state: {breaker.state}")


# Example 4: Webhook Notifications
# =================================

webhook_manager = WebhookManager()


@task(queue="notifications")
@on_task_complete("http://localhost:8000/webhooks/turbine")
def important_task_with_webhook(data: dict) -> dict:
    """Task that sends webhook on completion."""
    import time
    time.sleep(0.5)

    return {
        "processed": True,
        "data_size": len(str(data)),
    }


def example_webhooks():
    print("\n" + "=" * 60)
    print("Example 4: Webhook Notifications")
    print("=" * 60)

    # Subscribe to events
    webhook_id = webhook_manager.subscribe(
        url="http://localhost:8000/webhooks",
        events=[WebhookEvent.TASK_COMPLETED, WebhookEvent.TASK_FAILED],
        secret="webhook-secret-key"
    )

    print(f"\nWebhook subscription created: {webhook_id}")
    print("Events: TASK_COMPLETED, TASK_FAILED")

    # Submit task
    result = important_task_with_webhook.delay({"key": "value"})

    print(f"Task submitted: {result.task_id}")
    print("Webhook will be sent on completion...")

    # Cleanup
    webhook_manager.unsubscribe(webhook_id)

    print("\n✓ Webhook example complete")


# Example 5: Health Monitoring
# =============================

def example_health_monitoring():
    print("\n" + "=" * 60)
    print("Example 5: Health Monitoring")
    print("=" * 60)

    # Create health checker
    checker = HealthChecker(app)

    # Add custom check
    def check_database():
        # Simulate database check
        return {"healthy": True, "connections": 10}

    checker.add_check("database", check_database)

    # Run health check
    print("\nRunning health checks...")
    health = checker.check_health()

    print(f"\nOverall Status: {health.overall_status.upper()}")
    print(f"Healthy: {health.healthy}")
    print("\nComponent Health:")

    for name, check in health.checks.items():
        status = "✓" if check["healthy"] else "✗"
        duration = check.get("duration_ms", 0)
        print(f"  {status} {name}: {duration:.1f}ms")


# Example 6: Task Performance Monitoring
# =======================================

@task(queue="monitored")
def monitored_task(x: int) -> int:
    """Task with performance monitoring."""
    import time
    time.sleep(0.1 + (x % 10) * 0.01)
    return x * 2


def example_task_monitoring():
    print("\n" + "=" * 60)
    print("Example 6: Task Performance Monitoring")
    print("=" * 60)

    monitor = TaskMonitor("monitored_task")

    print("\nSubmitting 10 monitored tasks...")

    results = []
    for i in range(10):
        result = monitored_task.delay(i)
        results.append(result)

    # Wait and record
    import time
    for result in results:
        start = time.time()
        try:
            value = result.get(timeout=30)
            duration = (time.time() - start) * 1000

            monitor.record_execution(
                success=True,
                duration_ms=duration,
                task_id=result.task_id
            )

        except Exception as e:
            duration = (time.time() - start) * 1000
            monitor.record_execution(
                success=False,
                duration_ms=duration,
                error=str(e)
            )

    # Get statistics
    stats = monitor.get_stats()

    print(f"\nTask Statistics:")
    print(f"  Total: {stats['total_executions']}")
    print(f"  Success: {stats['successful']}")
    print(f"  Failed: {stats['failed']}")
    print(f"  Success Rate: {stats['success_rate']:.1%}")

    if stats['duration_percentiles']:
        print(f"\nDuration Percentiles:")
        for percentile, value in stats['duration_percentiles'].items():
            print(f"  {percentile}: {value:.0f}ms")


# Example 7: Export Results
# ==========================

def example_export():
    print("\n" + "=" * 60)
    print("Example 7: Export Task Results")
    print("=" * 60)

    # Submit some tasks
    print("\nSubmitting tasks for export...")

    task_ids = []
    for i in range(5):
        result = monitored_task.delay(i)
        task_ids.append(result.task_id)

    # Wait for completion
    import time
    time.sleep(2)

    # Export to JSON
    exporter = ResultExporter(app)

    print("\nExporting results to files...")

    count_json = exporter.export_to_json(task_ids, "/tmp/turbine-results.json")
    print(f"  ✓ Exported {count_json} results to JSON")

    count_csv = exporter.export_to_csv(task_ids, "/tmp/turbine-results.csv")
    print(f"  ✓ Exported {count_csv} results to CSV")

    count_jsonl = exporter.export_to_jsonl(task_ids, "/tmp/turbine-results.jsonl")
    print(f"  ✓ Exported {count_jsonl} results to JSONL")

    print("\nFiles created:")
    print("  - /tmp/turbine-results.json")
    print("  - /tmp/turbine-results.csv")
    print("  - /tmp/turbine-results.jsonl")


if __name__ == "__main__":
    import sys

    examples = {
        "1": ("Caching", example_caching),
        "2": ("Retry", example_retry),
        "3": ("Circuit breaker", example_circuit_breaker),
        "4": ("Webhooks", example_webhooks),
        "5": ("Health monitoring", example_health_monitoring),
        "6": ("Task monitoring", example_task_monitoring),
        "7": ("Export results", example_export),
        "all": ("Run all", None),
    }

    if len(sys.argv) > 1 and sys.argv[1] in examples:
        choice = sys.argv[1]
    else:
        print("Turbine Utilities Examples\n")
        print("Available examples:")
        for key, (name, _) in examples.items():
            print(f"  {key}. {name}")
        print()
        choice = input("Select example (or 'all'): ").strip()

    if choice == "all":
        for key, (name, func) in examples.items():
            if func:
                try:
                    func()
                except Exception as e:
                    print(f"\n✗ Example failed: {e}")
                    import traceback
                    traceback.print_exc()
    elif choice in examples and examples[choice][1]:
        try:
            examples[choice][1]()
        except Exception as e:
            print(f"\n✗ Example failed: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)
    else:
        print("Invalid choice")
        sys.exit(1)
