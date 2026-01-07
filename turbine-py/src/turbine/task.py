"""Task decorator and Task class for Turbine."""

from __future__ import annotations

import functools
import inspect
from typing import TYPE_CHECKING, Any, Callable, TypeVar

from turbine.client import TaskOptions
from turbine.result import AsyncResult
from turbine.workflow import Signature

if TYPE_CHECKING:
    from turbine.app import Turbine

F = TypeVar("F", bound=Callable[..., Any])


class Task:
    """
    Represents a registered task that can be executed asynchronously.

    Tasks are created using the @task decorator or @app.task decorator.

    Example:
        @app.task(queue="default", retry=3)
        def add(x: int, y: int) -> int:
            return x + y

        # Submit task
        result = add.delay(1, 2)

        # Submit with options
        result = add.apply_async(args=[1, 2], countdown=60)

        # Create signature for workflows
        sig = add.s(1, 2)
    """

    def __init__(
        self,
        func: Callable[..., Any],
        *,
        name: str | None = None,
        queue: str = "default",
        priority: int = 0,
        max_retries: int = 3,
        retry_delay: int = 60,
        timeout: int = 300,
        soft_timeout: int | None = None,
        store_result: bool = True,
        result_ttl: int = 86400,
        bind: bool = False,
        app: "Turbine | None" = None,
    ):
        """
        Initialize a Task.

        Args:
            func: The callable to execute
            name: Task name (defaults to function name)
            queue: Default queue name
            priority: Default priority
            max_retries: Maximum number of retries
            retry_delay: Delay between retries in seconds
            timeout: Hard timeout in seconds
            soft_timeout: Soft timeout in seconds
            store_result: Whether to store the result
            result_ttl: Result TTL in seconds
            bind: Whether to pass the task as the first argument
            app: Turbine application instance
        """
        self.func = func
        self.name = name or f"{func.__module__}.{func.__qualname__}"
        self.queue = queue
        self.priority = priority
        self.max_retries = max_retries
        self.retry_delay = retry_delay
        self.timeout = timeout
        self.soft_timeout = soft_timeout
        self.store_result = store_result
        self.result_ttl = result_ttl
        self.bind = bind
        self._app = app

        # Copy function metadata
        functools.update_wrapper(self, func)

    def __repr__(self) -> str:
        return f"<Task: {self.name}>"

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        """Execute the task synchronously (for local testing)."""
        if self.bind:
            return self.func(self, *args, **kwargs)
        return self.func(*args, **kwargs)

    @property
    def app(self) -> "Turbine":
        """Get the Turbine app instance."""
        if self._app is None:
            from turbine.app import get_current_app

            return get_current_app()
        return self._app

    @app.setter
    def app(self, app: "Turbine") -> None:
        """Set the Turbine app instance."""
        self._app = app

    def _get_options(self, **overrides: Any) -> TaskOptions:
        """Build TaskOptions from defaults and overrides."""
        return TaskOptions(
            queue=overrides.get("queue", self.queue),
            priority=overrides.get("priority", self.priority),
            max_retries=overrides.get("max_retries", self.max_retries),
            retry_delay=overrides.get("retry_delay", self.retry_delay),
            timeout=overrides.get("timeout", self.timeout),
            soft_timeout=overrides.get("soft_timeout", self.soft_timeout),
            store_result=overrides.get("store_result", self.store_result),
            result_ttl=overrides.get("result_ttl", self.result_ttl),
            eta=overrides.get("eta"),
            countdown=overrides.get("countdown"),
            expires=overrides.get("expires"),
            idempotency_key=overrides.get("idempotency_key"),
            headers=overrides.get("headers"),
        )

    def delay(self, *args: Any, **kwargs: Any) -> AsyncResult:
        """
        Submit task with default options.

        This is a shortcut for apply_async(args=args, kwargs=kwargs).

        Args:
            *args: Positional arguments for the task
            **kwargs: Keyword arguments for the task

        Returns:
            AsyncResult for tracking the task
        """
        return self.apply_async(args=args, kwargs=kwargs)

    def apply_async(
        self,
        args: tuple[Any, ...] | list[Any] | None = None,
        kwargs: dict[str, Any] | None = None,
        *,
        queue: str | None = None,
        priority: int | None = None,
        countdown: int | None = None,
        eta: int | None = None,
        expires: int | None = None,
        max_retries: int | None = None,
        retry_delay: int | None = None,
        timeout: int | None = None,
        soft_timeout: int | None = None,
        task_id: str | None = None,
        idempotency_key: str | None = None,
        headers: dict[str, str] | None = None,
    ) -> AsyncResult:
        """
        Submit task with custom options.

        Args:
            args: Positional arguments for the task
            kwargs: Keyword arguments for the task
            queue: Queue name (overrides default)
            priority: Priority (overrides default)
            countdown: Execute after this many seconds
            eta: Execute at this Unix timestamp
            expires: Task expires at this Unix timestamp
            max_retries: Maximum retries (overrides default)
            retry_delay: Retry delay (overrides default)
            timeout: Hard timeout (overrides default)
            soft_timeout: Soft timeout (overrides default)
            task_id: Custom task ID
            idempotency_key: Idempotency key for deduplication
            headers: Custom headers/metadata

        Returns:
            AsyncResult for tracking the task
        """
        args = list(args) if args else []
        kwargs = kwargs or {}

        # Build options with overrides
        option_overrides = {}
        if queue is not None:
            option_overrides["queue"] = queue
        if priority is not None:
            option_overrides["priority"] = priority
        if countdown is not None:
            option_overrides["countdown"] = countdown
        if eta is not None:
            option_overrides["eta"] = eta
        if expires is not None:
            option_overrides["expires"] = expires
        if max_retries is not None:
            option_overrides["max_retries"] = max_retries
        if retry_delay is not None:
            option_overrides["retry_delay"] = retry_delay
        if timeout is not None:
            option_overrides["timeout"] = timeout
        if soft_timeout is not None:
            option_overrides["soft_timeout"] = soft_timeout
        if idempotency_key is not None:
            option_overrides["idempotency_key"] = idempotency_key
        if headers is not None:
            option_overrides["headers"] = headers

        options = self._get_options(**option_overrides)

        # Submit task
        client = self.app.client
        result_task_id = client.submit_task(
            name=self.name,
            args=args,
            kwargs=kwargs,
            options=options,
            task_id=task_id,
        )

        return AsyncResult(
            result_task_id,
            client,
            task_name=self.name,
        )

    def s(self, *args: Any, **kwargs: Any) -> Signature:
        """
        Create a signature (immutable) for use in workflows.

        Args:
            *args: Positional arguments
            **kwargs: Keyword arguments

        Returns:
            Signature object
        """
        return Signature(
            task=self,
            args=args,
            kwargs=kwargs,
        )

    def si(self, *args: Any, **kwargs: Any) -> Signature:
        """
        Create an immutable signature that ignores previous results.

        Args:
            *args: Positional arguments
            **kwargs: Keyword arguments

        Returns:
            Signature object with immutable=True
        """
        return Signature(
            task=self,
            args=args,
            kwargs=kwargs,
            immutable=True,
        )

    def signature(
        self,
        args: tuple[Any, ...] | None = None,
        kwargs: dict[str, Any] | None = None,
        **options: Any,
    ) -> Signature:
        """
        Create a signature with full control over arguments and options.

        Args:
            args: Positional arguments
            kwargs: Keyword arguments
            **options: Task options

        Returns:
            Signature object
        """
        return Signature(
            task=self,
            args=args or (),
            kwargs=kwargs or {},
            options=options,
        )

    def retry(
        self,
        *,
        countdown: int | None = None,
        max_retries: int | None = None,
        exc: Exception | None = None,
    ) -> None:
        """
        Request a task retry (to be called from within a task).

        This is typically used in bound tasks:

            @app.task(bind=True)
            def my_task(self, x):
                try:
                    # do something
                except SomeError as e:
                    self.retry(exc=e, countdown=60)

        Args:
            countdown: Delay before retry in seconds
            max_retries: Override max retries
            exc: Exception that caused the retry
        """
        from turbine.exceptions import TaskError

        raise TaskError(
            f"Task retry requested: {exc or 'manual retry'}",
            task_id=None,
        )


def task(
    func: F | None = None,
    *,
    name: str | None = None,
    queue: str = "default",
    priority: int = 0,
    max_retries: int = 3,
    retry_delay: int = 60,
    timeout: int = 300,
    soft_timeout: int | None = None,
    store_result: bool = True,
    result_ttl: int = 86400,
    bind: bool = False,
) -> Task | Callable[[F], Task]:
    """
    Decorator to create a Turbine task.

    Can be used with or without arguments:

        @task
        def simple_task(x):
            return x * 2

        @task(queue="high-priority", timeout=600)
        def important_task(x):
            return x * 2

    Args:
        func: The function to wrap (when used without arguments)
        name: Task name (defaults to function path)
        queue: Default queue name
        priority: Default priority
        max_retries: Maximum number of retries
        retry_delay: Delay between retries in seconds
        timeout: Hard timeout in seconds
        soft_timeout: Soft timeout in seconds
        store_result: Whether to store the result
        result_ttl: Result TTL in seconds
        bind: Whether to pass the task as the first argument

    Returns:
        Task object or decorator function
    """

    def decorator(f: F) -> Task:
        return Task(
            f,
            name=name,
            queue=queue,
            priority=priority,
            max_retries=max_retries,
            retry_delay=retry_delay,
            timeout=timeout,
            soft_timeout=soft_timeout,
            store_result=store_result,
            result_ttl=result_ttl,
            bind=bind,
        )

    if func is not None:
        return decorator(func)
    return decorator
