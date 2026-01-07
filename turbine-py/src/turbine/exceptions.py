"""Turbine exceptions."""


class TurbineError(Exception):
    """Base exception for all Turbine errors."""

    pass


class ConnectionError(TurbineError):
    """Failed to connect to Turbine server."""

    pass


class TaskError(TurbineError):
    """Task execution failed."""

    def __init__(self, message: str, task_id: str | None = None, traceback: str | None = None):
        super().__init__(message)
        self.task_id = task_id
        self.traceback = traceback


class TimeoutError(TurbineError):
    """Task execution timed out."""

    def __init__(self, message: str, task_id: str | None = None, timeout: float | None = None):
        super().__init__(message)
        self.task_id = task_id
        self.timeout = timeout


class TaskNotFound(TurbineError):
    """Task with given ID was not found."""

    def __init__(self, task_id: str):
        super().__init__(f"Task not found: {task_id}")
        self.task_id = task_id


class TaskRevoked(TurbineError):
    """Task was revoked."""

    def __init__(self, task_id: str):
        super().__init__(f"Task revoked: {task_id}")
        self.task_id = task_id


class SerializationError(TurbineError):
    """Failed to serialize/deserialize task arguments or result."""

    pass


class WorkflowError(TurbineError):
    """Workflow execution failed."""

    pass
