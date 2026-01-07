"""Django app configuration for Turbine."""

from __future__ import annotations

import logging
from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from turbine import Turbine

logger = logging.getLogger(__name__)


class TurbineConfig:
    """
    Django app configuration for Turbine.

    Add 'turbine.django' to INSTALLED_APPS in your Django settings.

    Settings:
        TURBINE_SERVER: Server address (default: "localhost:50051")
        TURBINE_SECURE: Use TLS (default: False)
        TURBINE_DEFAULT_QUEUE: Default queue name (default: "default")
        TURBINE_DEFAULT_TIMEOUT: Default task timeout (default: 300)
        TURBINE_DEFAULT_RETRIES: Default max retries (default: 3)
        TURBINE_DEFAULT_RETRY_DELAY: Default retry delay (default: 60)
        TURBINE_DEFAULT_RESULT_TTL: Default result TTL (default: 86400)
        TURBINE_AUTODISCOVER: Auto-discover tasks (default: True)
        TURBINE_TASK_PACKAGES: List of packages to search for tasks

    Example settings.py:
        INSTALLED_APPS = [
            ...
            'turbine.django',
        ]

        TURBINE_SERVER = "localhost:50051"
        TURBINE_DEFAULT_QUEUE = "django"
        TURBINE_AUTODISCOVER = True
    """

    name = "turbine.django"
    verbose_name = "Turbine Task Queue"
    default = True

    _app: "Turbine | None" = None

    def ready(self) -> None:
        """Called when Django starts."""
        self._configure_app()
        self._autodiscover_tasks()

    def _configure_app(self) -> None:
        """Configure the Turbine app from Django settings."""
        from django.conf import settings

        from turbine import Turbine

        self._app = Turbine(
            server=getattr(settings, "TURBINE_SERVER", "localhost:50051"),
            secure=getattr(settings, "TURBINE_SECURE", False),
            default_queue=getattr(settings, "TURBINE_DEFAULT_QUEUE", "default"),
            default_timeout=getattr(settings, "TURBINE_DEFAULT_TIMEOUT", 300),
            default_max_retries=getattr(settings, "TURBINE_DEFAULT_RETRIES", 3),
            default_retry_delay=getattr(settings, "TURBINE_DEFAULT_RETRY_DELAY", 60),
            default_result_ttl=getattr(settings, "TURBINE_DEFAULT_RESULT_TTL", 86400),
            set_as_current=True,
        )

        logger.info(f"Turbine configured with server: {self._app.server}")

    def _autodiscover_tasks(self) -> None:
        """Auto-discover tasks from installed apps."""
        from django.conf import settings

        if not getattr(settings, "TURBINE_AUTODISCOVER", True):
            return

        if self._app is None:
            return

        # Get packages to search
        packages = getattr(settings, "TURBINE_TASK_PACKAGES", None)

        if packages is None:
            # Default: search all installed apps
            packages = list(settings.INSTALLED_APPS)

        self._app.autodiscover_tasks(packages)
        logger.info(f"Discovered {len(self._app.tasks)} tasks")

    @classmethod
    def get_app(cls) -> "Turbine":
        """Get the configured Turbine app."""
        if cls._app is None:
            raise RuntimeError("Turbine app not configured. Is 'turbine.django' in INSTALLED_APPS?")
        return cls._app


def get_app() -> "Turbine":
    """Get the configured Turbine application."""
    return TurbineConfig.get_app()


# Django-style decorator that uses the configured app
def task(
    func: Any = None,
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
    Decorator to create a Turbine task using Django settings.

    This decorator automatically uses the Turbine app configured
    via Django settings.

    Example:
        # myapp/tasks.py
        from turbine.django import task

        @task(queue="emails")
        def send_email(to: str, subject: str, body: str):
            # Send email
            pass

        # myapp/views.py
        from myapp.tasks import send_email

        def signup(request):
            user = create_user(request.POST)
            send_email.delay(
                to=user.email,
                subject="Welcome!",
                body="Thanks for signing up"
            )
            return redirect("home")
    """
    from turbine.task import Task

    def decorator(f: Any) -> Task:
        # We defer app lookup to support import-time decoration
        task_obj = Task(
            f,
            name=name,
            queue=queue or "default",
            priority=priority or 0,
            max_retries=max_retries or 3,
            retry_delay=retry_delay or 60,
            timeout=timeout or 300,
            bind=bind,
            app=None,  # Will be set lazily
        )

        # Register with app when ready
        try:
            app = TurbineConfig.get_app()
            app.register_task(task_obj)
        except RuntimeError:
            # App not ready yet, will be registered on autodiscover
            pass

        return task_obj

    if func is not None:
        return decorator(func)
    return decorator
