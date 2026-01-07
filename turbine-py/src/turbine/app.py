"""Turbine application class - the main entry point."""

from __future__ import annotations

import threading
from typing import Any, Callable, TypeVar

from turbine.client import TurbineClient
from turbine.task import Task

F = TypeVar("F", bound=Callable[..., Any])

# Global current app
_current_app: "Turbine | None" = None
_app_lock = threading.Lock()


def get_current_app() -> "Turbine":
    """Get the current Turbine application."""
    global _current_app
    if _current_app is None:
        raise RuntimeError(
            "No Turbine app configured. Create one with: app = Turbine(server='host:port')"
        )
    return _current_app


def set_current_app(app: "Turbine") -> None:
    """Set the current Turbine application."""
    global _current_app
    with _app_lock:
        _current_app = app


class Turbine:
    """
    Turbine application - the main entry point for task queue operations.

    Example:
        from turbine import Turbine

        app = Turbine(server="localhost:50051")

        @app.task(queue="default")
        def add(x: int, y: int) -> int:
            return x + y

        # Submit task
        result = add.delay(1, 2)
        print(result.get())  # 3

    Configuration:
        # From environment
        app = Turbine()  # Uses TURBINE_SERVER env var

        # Explicit server
        app = Turbine(server="localhost:50051")

        # With options
        app = Turbine(
            server="localhost:50051",
            default_queue="high-priority",
            default_timeout=600,
        )
    """

    def __init__(
        self,
        server: str = "localhost:50051",
        *,
        secure: bool = False,
        default_queue: str = "default",
        default_priority: int = 0,
        default_max_retries: int = 3,
        default_retry_delay: int = 60,
        default_timeout: int = 300,
        default_result_ttl: int = 86400,
        set_as_current: bool = True,
    ):
        """
        Initialize Turbine application.

        Args:
            server: Turbine server address (host:port)
            secure: Whether to use TLS
            default_queue: Default queue for tasks
            default_priority: Default task priority
            default_max_retries: Default max retries
            default_retry_delay: Default retry delay in seconds
            default_timeout: Default task timeout in seconds
            default_result_ttl: Default result TTL in seconds
            set_as_current: Whether to set this as the current app
        """
        import os

        # Allow env var override
        server = os.environ.get("TURBINE_SERVER", server)

        self.server = server
        self.secure = secure
        self.default_queue = default_queue
        self.default_priority = default_priority
        self.default_max_retries = default_max_retries
        self.default_retry_delay = default_retry_delay
        self.default_timeout = default_timeout
        self.default_result_ttl = default_result_ttl

        # Task registry
        self._tasks: dict[str, Task] = {}

        # Lazy client initialization
        self._client: TurbineClient | None = None

        # Set as current app
        if set_as_current:
            set_current_app(self)

    @property
    def client(self) -> TurbineClient:
        """Get or create the gRPC client."""
        if self._client is None:
            self._client = TurbineClient(
                server=self.server,
                secure=self.secure,
            )
        return self._client

    def close(self) -> None:
        """Close the client connection."""
        if self._client is not None:
            self._client.close()
            self._client = None

    def __enter__(self) -> "Turbine":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    def task(
        self,
        func: F | None = None,
        *,
        name: str | None = None,
        queue: str | None = None,
        priority: int | None = None,
        max_retries: int | None = None,
        retry_delay: int | None = None,
        timeout: int | None = None,
        soft_timeout: int | None = None,
        store_result: bool = True,
        result_ttl: int | None = None,
        bind: bool = False,
    ) -> Task | Callable[[F], Task]:
        """
        Decorator to register a task with this application.

        Args:
            func: The function to wrap
            name: Task name (defaults to function path)
            queue: Queue name (defaults to app default)
            priority: Task priority (defaults to app default)
            max_retries: Max retries (defaults to app default)
            retry_delay: Retry delay (defaults to app default)
            timeout: Task timeout (defaults to app default)
            soft_timeout: Soft timeout in seconds
            store_result: Whether to store the result
            result_ttl: Result TTL (defaults to app default)
            bind: Whether to pass the task as first argument

        Returns:
            Task object or decorator
        """

        def decorator(f: F) -> Task:
            task = Task(
                f,
                name=name,
                queue=queue or self.default_queue,
                priority=priority if priority is not None else self.default_priority,
                max_retries=max_retries if max_retries is not None else self.default_max_retries,
                retry_delay=retry_delay if retry_delay is not None else self.default_retry_delay,
                timeout=timeout if timeout is not None else self.default_timeout,
                soft_timeout=soft_timeout,
                store_result=store_result,
                result_ttl=result_ttl if result_ttl is not None else self.default_result_ttl,
                bind=bind,
                app=self,
            )
            self._tasks[task.name] = task
            return task

        if func is not None:
            return decorator(func)
        return decorator

    def register_task(self, task: Task) -> None:
        """
        Register an existing task with this application.

        Args:
            task: Task object to register
        """
        task.app = self
        self._tasks[task.name] = task

    def get_task(self, name: str) -> Task | None:
        """
        Get a registered task by name.

        Args:
            name: Task name

        Returns:
            Task object or None
        """
        return self._tasks.get(name)

    @property
    def tasks(self) -> dict[str, Task]:
        """Return all registered tasks."""
        return self._tasks.copy()

    def health_check(self) -> dict[str, Any]:
        """
        Check server health.

        Returns:
            Health status dictionary
        """
        return self.client.health_check()

    def get_task_status(self, task_id: str) -> dict[str, Any]:
        """
        Get task status.

        Args:
            task_id: Task ID

        Returns:
            Task status dictionary
        """
        return self.client.get_task_status(task_id)

    def get_queue_info(self, queue: str | None = None) -> list[dict[str, Any]]:
        """
        Get queue information.

        Args:
            queue: Queue name (optional)

        Returns:
            List of queue info dictionaries
        """
        return self.client.get_queue_info(queue)

    def purge_queue(self, queue: str) -> int:
        """
        Purge all messages from a queue.

        Args:
            queue: Queue name

        Returns:
            Number of messages purged
        """
        return self.client.purge_queue(queue)

    def send_task(
        self,
        name: str,
        args: list[Any] | None = None,
        kwargs: dict[str, Any] | None = None,
        **options: Any,
    ) -> "AsyncResult":
        """
        Send a task by name (without requiring the task to be registered locally).

        This is useful for sending tasks defined in other services.

        Args:
            name: Task name
            args: Positional arguments
            kwargs: Keyword arguments
            **options: Task options

        Returns:
            AsyncResult for tracking the task
        """
        from turbine.client import TaskOptions
        from turbine.result import AsyncResult

        task_options = TaskOptions(
            queue=options.get("queue", self.default_queue),
            priority=options.get("priority", self.default_priority),
            max_retries=options.get("max_retries", self.default_max_retries),
            retry_delay=options.get("retry_delay", self.default_retry_delay),
            timeout=options.get("timeout", self.default_timeout),
            soft_timeout=options.get("soft_timeout"),
            eta=options.get("eta"),
            countdown=options.get("countdown"),
            expires=options.get("expires"),
            store_result=options.get("store_result", True),
            result_ttl=options.get("result_ttl", self.default_result_ttl),
            idempotency_key=options.get("idempotency_key"),
            headers=options.get("headers"),
        )

        task_id = self.client.submit_task(
            name=name,
            args=args or [],
            kwargs=kwargs or {},
            options=task_options,
            task_id=options.get("task_id"),
        )

        return AsyncResult(task_id, self.client, task_name=name)

    def autodiscover_tasks(self, packages: list[str]) -> None:
        """
        Auto-discover and import tasks from packages.

        Args:
            packages: List of package names to search for tasks
        """
        import importlib
        import pkgutil

        for package_name in packages:
            try:
                package = importlib.import_module(package_name)
            except ImportError:
                continue

            # Try to find tasks module
            for module_name in ["tasks", "celery", "jobs"]:
                try:
                    importlib.import_module(f"{package_name}.{module_name}")
                except ImportError:
                    pass

            # Also check subpackages
            if hasattr(package, "__path__"):
                for _, name, _ in pkgutil.iter_modules(package.__path__):
                    if name in ["tasks", "celery", "jobs"]:
                        try:
                            importlib.import_module(f"{package_name}.{name}")
                        except ImportError:
                            pass
