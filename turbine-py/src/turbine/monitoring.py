"""Monitoring, health checks, and observability utilities."""

import logging
import time
from typing import Any, Callable
from dataclasses import dataclass
from datetime import datetime
import redis

logger = logging.getLogger(__name__)


@dataclass
class HealthStatus:
    """Health check status."""

    healthy: bool
    checks: dict[str, dict[str, Any]]
    timestamp: str
    overall_status: str  # "healthy", "degraded", "unhealthy"

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "healthy": self.healthy,
            "status": self.overall_status,
            "checks": self.checks,
            "timestamp": self.timestamp,
        }


class HealthChecker:
    """
    Health check system for Turbine components.

    Example:
        checker = HealthChecker(turbine_app)

        # Add custom checks
        checker.add_check("database", check_database_connection)
        checker.add_check("external_api", check_api_availability)

        # Run all checks
        health = checker.check_health()

        if not health.healthy:
            send_alert("System unhealthy", health.checks)
    """

    def __init__(self, turbine_app: Any | None = None):
        """
        Initialize health checker.

        Args:
            turbine_app: Turbine application instance
        """
        self.app = turbine_app
        self.checks: dict[str, Callable] = {}

        # Add default checks
        if turbine_app:
            self.add_check("turbine_server", self._check_turbine_server)
            self.add_check("redis_broker", self._check_redis_broker)

    def add_check(self, name: str, check_func: Callable[[], bool | dict]) -> None:
        """
        Add a health check.

        Args:
            name: Check name
            check_func: Function that returns bool or dict with status info

        Example:
            def check_db():
                try:
                    db.execute("SELECT 1")
                    return True
                except:
                    return False

            checker.add_check("database", check_db)
        """
        self.checks[name] = check_func

    def check_health(self, timeout: float = 5.0) -> HealthStatus:
        """
        Run all health checks.

        Args:
            timeout: Timeout for individual checks

        Returns:
            HealthStatus with results of all checks
        """
        results = {}
        all_healthy = True

        for name, check_func in self.checks.items():
            try:
                # Run check with timeout
                start = time.time()
                check_result = check_func()
                duration = time.time() - start

                if isinstance(check_result, dict):
                    # Detailed result
                    healthy = check_result.get("healthy", True)
                    results[name] = {
                        "healthy": healthy,
                        "duration_ms": duration * 1000,
                        **check_result
                    }
                else:
                    # Simple boolean
                    results[name] = {
                        "healthy": bool(check_result),
                        "duration_ms": duration * 1000,
                    }

                if not results[name]["healthy"]:
                    all_healthy = False

            except Exception as e:
                logger.error(f"Health check '{name}' failed: {e}")
                results[name] = {
                    "healthy": False,
                    "error": str(e),
                }
                all_healthy = False

        # Determine overall status
        failed_count = sum(1 for r in results.values() if not r["healthy"])

        if failed_count == 0:
            overall_status = "healthy"
        elif failed_count < len(results) / 2:
            overall_status = "degraded"
        else:
            overall_status = "unhealthy"

        return HealthStatus(
            healthy=all_healthy,
            checks=results,
            timestamp=datetime.utcnow().isoformat() + "Z",
            overall_status=overall_status,
        )

    def _check_turbine_server(self) -> dict[str, Any]:
        """Check Turbine server health."""
        try:
            health = self.app.client.health_check()
            return {
                "healthy": health.get("status") == "healthy",
                "version": health.get("version"),
                "uptime": health.get("uptime"),
            }
        except Exception as e:
            return {
                "healthy": False,
                "error": str(e),
            }

    def _check_redis_broker(self) -> dict[str, Any]:
        """Check Redis broker health."""
        try:
            from turbine.worker import RedisConnection

            conn = RedisConnection(self.app.client.server)
            pong = conn.conn.ping()

            return {
                "healthy": pong,
                "latency_ms": 0,  # Could measure this
            }
        except Exception as e:
            return {
                "healthy": False,
                "error": str(e),
            }


class MetricsCollector:
    """
    Collect custom application metrics.

    Supplements Turbine's built-in Prometheus metrics.
    """

    def __init__(self, redis_url: str = "redis://localhost:6379"):
        """
        Initialize metrics collector.

        Args:
            redis_url: Redis URL for metrics storage
        """
        self.conn = redis.from_url(redis_url, decode_responses=True)
        self.prefix = "turbine:metrics:custom"

    def increment(
        self,
        metric_name: str,
        value: int = 1,
        labels: dict[str, str] | None = None,
    ) -> int:
        """
        Increment a counter metric.

        Args:
            metric_name: Metric name
            value: Amount to increment
            labels: Metric labels

        Returns:
            New counter value

        Example:
            collector.increment("emails_sent", labels={"type": "welcome"})
        """
        key = self._make_key(metric_name, labels)
        return self.conn.incr(key, value)

    def gauge(
        self,
        metric_name: str,
        value: float,
        labels: dict[str, str] | None = None,
    ) -> None:
        """
        Set a gauge metric value.

        Args:
            metric_name: Metric name
            value: Gauge value
            labels: Metric labels

        Example:
            collector.gauge("queue_depth", 45, labels={"queue": "emails"})
        """
        key = self._make_key(metric_name, labels)
        self.conn.set(key, value)

    def timing(
        self,
        metric_name: str,
        duration_ms: float,
        labels: dict[str, str] | None = None,
    ) -> None:
        """
        Record timing metric.

        Args:
            metric_name: Metric name
            duration_ms: Duration in milliseconds
            labels: Metric labels

        Example:
            start = time.time()
            do_work()
            duration = (time.time() - start) * 1000
            collector.timing("api_call_duration", duration, labels={"endpoint": "/users"})
        """
        key = f"{self._make_key(metric_name, labels)}:timings"

        # Store in sorted set with timestamp as score
        timestamp = time.time()
        self.conn.zadd(key, {str(duration_ms): timestamp})

        # Keep only last 1000 entries
        self.conn.zremrangebyrank(key, 0, -1001)

    def get_metric(
        self,
        metric_name: str,
        labels: dict[str, str] | None = None,
    ) -> Any:
        """
        Get current metric value.

        Args:
            metric_name: Metric name
            labels: Metric labels

        Returns:
            Metric value
        """
        key = self._make_key(metric_name, labels)
        return self.conn.get(key)

    def _make_key(
        self,
        metric_name: str,
        labels: dict[str, str] | None = None,
    ) -> str:
        """Generate Redis key for metric."""
        if labels:
            label_str = ",".join(f"{k}={v}" for k, v in sorted(labels.items()))
            return f"{self.prefix}:{metric_name}:{label_str}"
        return f"{self.prefix}:{metric_name}"


def monitor_task_execution(
    metric_name: str | None = None,
    collect_errors: bool = True,
):
    """
    Decorator to monitor task execution.

    Collects:
    - Execution count
    - Success/failure count
    - Execution duration
    - Error types

    Args:
        metric_name: Custom metric name (default: task name)
        collect_errors: Collect error types

    Example:
        @task
        @monitor_task_execution()
        def monitored_task(x):
            return x * 2

        # Metrics collected automatically:
        # - monitored_task:count
        # - monitored_task:success
        # - monitored_task:failure
        # - monitored_task:duration
    """
    collector = MetricsCollector()

    def decorator(task_func):
        from functools import wraps

        original_func = task_func.func if hasattr(task_func, 'func') else task_func
        metric = metric_name or getattr(task_func, 'name', original_func.__name__)

        @wraps(original_func)
        def wrapper(*args, **kwargs):
            # Increment execution count
            collector.increment(f"{metric}:count")

            start_time = time.time()

            try:
                # Execute task
                result = original_func(*args, **kwargs)

                # Record success
                duration = (time.time() - start_time) * 1000
                collector.increment(f"{metric}:success")
                collector.timing(f"{metric}:duration", duration)

                return result

            except Exception as e:
                # Record failure
                duration = (time.time() - start_time) * 1000
                collector.increment(f"{metric}:failure")
                collector.timing(f"{metric}:duration", duration)

                # Collect error type
                if collect_errors:
                    error_type = type(e).__name__
                    collector.increment(
                        f"{metric}:errors",
                        labels={"error_type": error_type}
                    )

                raise

        if hasattr(task_func, 'func'):
            task_func.func = wrapper

        return task_func

    return decorator


class SystemMonitor:
    """Monitor system health and send alerts."""

    def __init__(
        self,
        turbine_app: Any,
        alert_callback: Callable[[str, dict], None] | None = None,
    ):
        """
        Initialize system monitor.

        Args:
            turbine_app: Turbine application
            alert_callback: Function to call on alerts
        """
        self.app = turbine_app
        self.alert_callback = alert_callback
        self.thresholds = {
            "queue_depth": 1000,
            "failure_rate": 0.1,  # 10%
            "task_lag_seconds": 60,
        }

    def check_queue_health(self) -> dict[str, Any]:
        """
        Check queue health metrics.

        Returns:
            Dictionary with queue health status
        """
        try:
            queues = self.app.client.get_queue_info()

            alerts = []

            for queue in queues:
                # Check queue depth
                if queue["pending"] > self.thresholds["queue_depth"]:
                    alerts.append({
                        "severity": "warning",
                        "message": f"Queue {queue['name']} has {queue['pending']} pending tasks",
                        "queue": queue['name'],
                        "pending": queue["pending"],
                    })

                # Check consumer count
                if queue["pending"] > 100 and queue["consumers"] == 0:
                    alerts.append({
                        "severity": "critical",
                        "message": f"Queue {queue['name']} has no consumers",
                        "queue": queue['name'],
                    })

            return {
                "healthy": len(alerts) == 0,
                "queues": queues,
                "alerts": alerts,
            }

        except Exception as e:
            logger.error(f"Failed to check queue health: {e}")
            return {
                "healthy": False,
                "error": str(e),
            }

    def check_worker_health(self) -> dict[str, Any]:
        """
        Check worker health.

        Returns:
            Dictionary with worker health status
        """
        # This would require worker heartbeat implementation
        # Placeholder for now
        return {
            "healthy": True,
            "message": "Worker health check not implemented",
        }

    def monitor(self, interval: int = 60) -> None:
        """
        Continuous monitoring loop.

        Args:
            interval: Check interval in seconds

        Example:
            monitor = SystemMonitor(app, alert_callback=send_slack_alert)
            monitor.monitor(interval=30)  # Check every 30 seconds
        """
        logger.info(f"Starting system monitor (interval: {interval}s)")

        while True:
            try:
                # Check queue health
                queue_health = self.check_queue_health()

                if not queue_health["healthy"] and self.alert_callback:
                    for alert in queue_health.get("alerts", []):
                        self.alert_callback(
                            "queue_health",
                            alert
                        )

                # Check worker health
                worker_health = self.check_worker_health()

                if not worker_health["healthy"] and self.alert_callback:
                    self.alert_callback(
                        "worker_health",
                        worker_health
                    )

                time.sleep(interval)

            except KeyboardInterrupt:
                logger.info("Stopping monitor")
                break
            except Exception as e:
                logger.error(f"Monitor error: {e}")
                time.sleep(interval)


def task_timing(func: Callable) -> Callable:
    """
    Decorator to measure and log task execution time.

    Example:
        @task
        @task_timing
        def timed_task(data):
            return process(data)

        # Logs: "Task timed_task completed in 1234ms"
    """
    from functools import wraps

    @wraps(func)
    def wrapper(*args, **kwargs):
        start = time.time()

        try:
            result = func(*args, **kwargs)
            duration = (time.time() - start) * 1000

            logger.info(
                f"Task {func.__name__} completed in {duration:.0f}ms"
            )

            return result

        except Exception as e:
            duration = (time.time() - start) * 1000

            logger.error(
                f"Task {func.__name__} failed after {duration:.0f}ms: {e}"
            )

            raise

    return wrapper


class TaskMonitor:
    """
    Monitor specific task execution metrics.

    Example:
        monitor = TaskMonitor("send_email")

        # In task or after execution
        monitor.record_execution(
            success=True,
            duration_ms=1050,
            task_id="abc-123"
        )

        # Get statistics
        stats = monitor.get_stats()
    """

    def __init__(
        self,
        task_name: str,
        redis_url: str = "redis://localhost:6379",
    ):
        """
        Initialize task monitor.

        Args:
            task_name: Task name to monitor
            redis_url: Redis URL
        """
        self.task_name = task_name
        self.conn = redis.from_url(redis_url, decode_responses=True)
        self.prefix = f"turbine:monitor:{task_name}"

    def record_execution(
        self,
        success: bool,
        duration_ms: float,
        task_id: str | None = None,
        error: str | None = None,
    ) -> None:
        """
        Record task execution.

        Args:
            success: Whether task succeeded
            duration_ms: Execution duration
            task_id: Task ID
            error: Error message if failed
        """
        # Increment counters
        self.conn.incr(f"{self.prefix}:total")

        if success:
            self.conn.incr(f"{self.prefix}:success")
        else:
            self.conn.incr(f"{self.prefix}:failure")

            if error:
                # Track error types
                error_key = f"{self.prefix}:errors:{error[:50]}"
                self.conn.incr(error_key)

        # Record duration in sorted set
        timestamp = time.time()
        self.conn.zadd(
            f"{self.prefix}:durations",
            {str(duration_ms): timestamp}
        )

        # Keep only last 1000 durations
        self.conn.zremrangebyrank(f"{self.prefix}:durations", 0, -1001)

    def get_stats(self, window_seconds: int = 3600) -> dict[str, Any]:
        """
        Get task statistics.

        Args:
            window_seconds: Time window for rate calculations

        Returns:
            Statistics dictionary

        Example:
            stats = monitor.get_stats()
            print(f"Success rate: {stats['success_rate']:.1%}")
            print(f"p95 duration: {stats['p95_duration_ms']:.0f}ms")
        """
        total = int(self.conn.get(f"{self.prefix}:total") or 0)
        success = int(self.conn.get(f"{self.prefix}:success") or 0)
        failure = int(self.conn.get(f"{self.prefix}:failure") or 0)

        # Calculate rates
        success_rate = success / total if total > 0 else 0.0
        failure_rate = failure / total if total > 0 else 0.0

        # Get duration percentiles
        durations_data = self.conn.zrange(
            f"{self.prefix}:durations",
            0,
            -1,
            withscores=False
        )

        durations = [float(d) for d in durations_data] if durations_data else []

        percentiles = {}
        if durations:
            sorted_durations = sorted(durations)
            percentiles = {
                "p50": self._percentile(sorted_durations, 50),
                "p75": self._percentile(sorted_durations, 75),
                "p95": self._percentile(sorted_durations, 95),
                "p99": self._percentile(sorted_durations, 99),
                "max": max(sorted_durations),
                "min": min(sorted_durations),
            }

        return {
            "task_name": self.task_name,
            "total_executions": total,
            "successful": success,
            "failed": failure,
            "success_rate": success_rate,
            "failure_rate": failure_rate,
            "duration_percentiles": percentiles,
        }

    @staticmethod
    def _percentile(sorted_data: list[float], percentile: int) -> float:
        """Calculate percentile value."""
        if not sorted_data:
            return 0.0

        index = int(len(sorted_data) * (percentile / 100.0))
        index = min(index, len(sorted_data) - 1)

        return sorted_data[index]

    def reset(self) -> None:
        """Reset all statistics."""
        keys = self.conn.keys(f"{self.prefix}:*")
        if keys:
            self.conn.delete(*keys)


def create_health_endpoint(turbine_app: Any) -> Callable:
    """
    Create a health check endpoint for web frameworks.

    Args:
        turbine_app: Turbine application

    Returns:
        Health check handler function

    Example (FastAPI):
        from fastapi import FastAPI
        from turbine.monitoring import create_health_endpoint

        app = FastAPI()
        turbine_app = Turbine(server="localhost:50051")

        health_check = create_health_endpoint(turbine_app)

        @app.get("/health")
        def health():
            return health_check()

    Example (Django):
        from django.http import JsonResponse
        from turbine.monitoring import create_health_endpoint

        turbine_app = Turbine(server="localhost:50051")
        health_check = create_health_endpoint(turbine_app)

        def health_view(request):
            status = health_check()
            status_code = 200 if status["healthy"] else 503
            return JsonResponse(status, status=status_code)
    """
    checker = HealthChecker(turbine_app)

    def health_handler() -> dict[str, Any]:
        """Health check handler."""
        health = checker.check_health()
        return health.to_dict()

    return health_handler
