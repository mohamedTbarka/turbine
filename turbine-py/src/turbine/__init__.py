"""
Turbine - High-performance distributed task queue

A modern, reliable alternative to Celery with first-class Python support.

Example usage:
    from turbine import Turbine, task

    app = Turbine(server="localhost:50051")

    @app.task(queue="default")
    def add(x: int, y: int) -> int:
        return x + y

    # Submit task
    result = add.delay(1, 2)

    # Wait for result
    print(result.get())  # Output: 3

    # Workflows
    from turbine import chain, group

    workflow = chain(
        add.s(1, 2),
        add.s(3),
    )
    workflow.delay()
"""

from turbine.app import Turbine
from turbine.task import task, Task
from turbine.result import AsyncResult
from turbine.workflow import chain, group, chord, Signature
from turbine.client import TurbineClient
from turbine.worker import Worker, run_worker
from turbine.exceptions import (
    TurbineError,
    TaskError,
    TimeoutError,
    ConnectionError,
    TaskNotFound,
    TaskRevoked,
)

__version__ = "0.1.0"
__all__ = [
    # Core
    "Turbine",
    "TurbineClient",
    # Task
    "task",
    "Task",
    # Result
    "AsyncResult",
    # Workflow
    "chain",
    "group",
    "chord",
    "Signature",
    # Worker
    "Worker",
    "run_worker",
    # Exceptions
    "TurbineError",
    "TaskError",
    "TimeoutError",
    "ConnectionError",
    "TaskNotFound",
    "TaskRevoked",
]
