"""FastAPI integration for Turbine task queue."""

from __future__ import annotations

import logging
from contextlib import asynccontextmanager
from typing import TYPE_CHECKING, Any, Callable, TypeVar

if TYPE_CHECKING:
    from fastapi import FastAPI

    from turbine import Turbine

logger = logging.getLogger(__name__)

F = TypeVar("F", bound=Callable[..., Any])

# Global app reference
_turbine_app: "Turbine | None" = None


def get_turbine() -> "Turbine":
    """
    Get the configured Turbine app.

    Use this as a FastAPI dependency:

        from fastapi import Depends
        from turbine.fastapi import get_turbine

        @app.post("/tasks")
        async def create_task(turbine: Turbine = Depends(get_turbine)):
            result = turbine.send_task("my_task", args=[1, 2])
            return {"task_id": result.task_id}
    """
    if _turbine_app is None:
        raise RuntimeError(
            "Turbine not configured. Add TurbineExtension to your FastAPI app lifespan."
        )
    return _turbine_app


class TurbineExtension:
    """
    FastAPI extension for Turbine task queue.

    Example:
        from fastapi import FastAPI
        from turbine.fastapi import TurbineExtension

        turbine = TurbineExtension(server="localhost:50051")

        app = FastAPI(lifespan=turbine.lifespan)

        # Or manually in lifespan
        @asynccontextmanager
        async def lifespan(app: FastAPI):
            await turbine.startup()
            yield
            await turbine.shutdown()

        app = FastAPI(lifespan=lifespan)

    Configuration via environment:
        TURBINE_SERVER: Server address
        TURBINE_SECURE: Use TLS (true/false)
        TURBINE_DEFAULT_QUEUE: Default queue name
    """

    def __init__(
        self,
        server: str = "localhost:50051",
        *,
        secure: bool = False,
        default_queue: str = "default",
        default_timeout: int = 300,
        default_max_retries: int = 3,
        default_retry_delay: int = 60,
        default_result_ttl: int = 86400,
        autodiscover_packages: list[str] | None = None,
    ):
        """
        Initialize TurbineExtension.

        Args:
            server: Turbine server address
            secure: Whether to use TLS
            default_queue: Default queue name
            default_timeout: Default task timeout
            default_max_retries: Default max retries
            default_retry_delay: Default retry delay
            default_result_ttl: Default result TTL
            autodiscover_packages: Packages to search for tasks
        """
        import os

        self.server = os.environ.get("TURBINE_SERVER", server)
        self.secure = os.environ.get("TURBINE_SECURE", "").lower() == "true" or secure
        self.default_queue = os.environ.get("TURBINE_DEFAULT_QUEUE", default_queue)
        self.default_timeout = default_timeout
        self.default_max_retries = default_max_retries
        self.default_retry_delay = default_retry_delay
        self.default_result_ttl = default_result_ttl
        self.autodiscover_packages = autodiscover_packages

        self._app: "Turbine | None" = None

    @property
    def app(self) -> "Turbine":
        """Get the Turbine app instance."""
        if self._app is None:
            raise RuntimeError("Turbine not started. Call startup() first.")
        return self._app

    async def startup(self) -> None:
        """Start the Turbine connection."""
        global _turbine_app

        from turbine import Turbine

        self._app = Turbine(
            server=self.server,
            secure=self.secure,
            default_queue=self.default_queue,
            default_timeout=self.default_timeout,
            default_max_retries=self.default_max_retries,
            default_retry_delay=self.default_retry_delay,
            default_result_ttl=self.default_result_ttl,
            set_as_current=True,
        )

        _turbine_app = self._app

        # Autodiscover tasks
        if self.autodiscover_packages:
            self._app.autodiscover_tasks(self.autodiscover_packages)

        # Verify connection
        try:
            health = self._app.health_check()
            logger.info(f"Connected to Turbine server: {self.server}")
            logger.info(f"Server version: {health.get('version')}")
        except Exception as e:
            logger.warning(f"Could not verify Turbine connection: {e}")

    async def shutdown(self) -> None:
        """Close the Turbine connection."""
        global _turbine_app

        if self._app is not None:
            self._app.close()
            self._app = None
            _turbine_app = None
            logger.info("Turbine connection closed")

    @asynccontextmanager
    async def lifespan(self, app: "FastAPI"):
        """
        FastAPI lifespan context manager.

        Use this as the lifespan parameter:

            turbine = TurbineExtension()
            app = FastAPI(lifespan=turbine.lifespan)
        """
        await self.startup()
        yield
        await self.shutdown()

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
        bind: bool = False,
    ) -> Any:
        """
        Decorator to register a task.

        Example:
            turbine = TurbineExtension()

            @turbine.task(queue="emails")
            def send_email(to: str, subject: str):
                pass
        """
        from turbine.task import Task

        def decorator(f: F) -> Task:
            task_obj = Task(
                f,
                name=name,
                queue=queue or self.default_queue,
                priority=priority or 0,
                max_retries=max_retries or self.default_max_retries,
                retry_delay=retry_delay or self.default_retry_delay,
                timeout=timeout or self.default_timeout,
                bind=bind,
                app=None,  # Will use current app
            )

            # Register if app is ready
            if self._app is not None:
                self._app.register_task(task_obj)

            return task_obj

        if func is not None:
            return decorator(func)
        return decorator


# Standalone decorator that uses global app
def task(
    func: F | None = None,
    *,
    name: str | None = None,
    queue: str = "default",
    priority: int = 0,
    max_retries: int = 3,
    retry_delay: int = 60,
    timeout: int = 300,
    bind: bool = False,
) -> Any:
    """
    Decorator to create a task using the global Turbine app.

    Example:
        from turbine.fastapi import task

        @task(queue="notifications")
        def send_notification(user_id: int, message: str):
            pass

        # In endpoint
        @app.post("/notify")
        async def notify(user_id: int, message: str):
            result = send_notification.delay(user_id, message)
            return {"task_id": result.task_id}
    """
    from turbine.task import Task

    def decorator(f: F) -> Task:
        return Task(
            f,
            name=name,
            queue=queue,
            priority=priority,
            max_retries=max_retries,
            retry_delay=retry_delay,
            timeout=timeout,
            bind=bind,
            app=None,  # Will use current app
        )

    if func is not None:
        return decorator(func)
    return decorator
