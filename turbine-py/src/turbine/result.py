"""Async result handling for Turbine tasks."""

from __future__ import annotations

import time
from typing import TYPE_CHECKING, Any

from turbine.client import TaskState
from turbine.exceptions import TaskError, TaskNotFound, TaskRevoked, TimeoutError

if TYPE_CHECKING:
    from turbine.client import TurbineClient


class AsyncResult:
    """
    Represents the result of an asynchronous task.

    This object can be used to check the status of a task,
    wait for its completion, and retrieve its result.

    Example:
        result = add.delay(1, 2)

        # Check if ready
        if result.ready():
            print(result.get())

        # Wait for result with timeout
        value = result.get(timeout=30)

        # Check status
        print(result.status)  # 'SUCCESS'
    """

    def __init__(
        self,
        task_id: str,
        client: "TurbineClient",
        *,
        task_name: str | None = None,
    ):
        """
        Initialize AsyncResult.

        Args:
            task_id: The task ID
            client: TurbineClient instance
            task_name: Optional task name for reference
        """
        self.task_id = task_id
        self._client = client
        self.task_name = task_name
        self._result: Any = None
        self._status: str | None = None
        self._ready: bool = False
        self._error: str | None = None
        self._traceback: str | None = None

    def __repr__(self) -> str:
        return f"<AsyncResult: {self.task_id}>"

    @property
    def id(self) -> str:
        """Return task ID."""
        return self.task_id

    @property
    def status(self) -> str:
        """
        Return current task status.

        Returns one of: PENDING, RECEIVED, RUNNING, SUCCESS, FAILURE, RETRY, REVOKED
        """
        if self._status is None:
            self._refresh_status()
        return self._status or TaskState.PENDING

    @property
    def state(self) -> str:
        """Alias for status."""
        return self.status

    @property
    def result(self) -> Any:
        """
        Return task result if available.

        Returns None if task is not complete.
        Raises TaskError if task failed.
        """
        if not self._ready:
            self._refresh_result()
        return self._result

    def _refresh_status(self) -> None:
        """Refresh status from server."""
        try:
            info = self._client.get_status(self.task_id)
            self._status = info.get("state", TaskState.PENDING)
            self._error = info.get("error")
            self._traceback = info.get("traceback")
        except TaskNotFound:
            self._status = TaskState.PENDING

    def _refresh_result(self) -> None:
        """Refresh result from server."""
        try:
            ready, result = self._client.get_result(self.task_id)
            self._ready = ready
            if ready:
                self._result = result
                self._status = TaskState.SUCCESS
        except TaskError as e:
            self._ready = True
            self._status = TaskState.FAILURE
            self._error = str(e)
            self._traceback = e.traceback
            raise
        except TaskRevoked:
            self._ready = True
            self._status = TaskState.REVOKED
            raise
        except TaskNotFound:
            self._ready = False
            self._status = TaskState.PENDING

    def ready(self) -> bool:
        """
        Check if the task has completed.

        Returns:
            True if the task has finished (success, failure, or revoked)
        """
        if self._ready:
            return True

        try:
            ready, _ = self._client.get_result(self.task_id)
            self._ready = ready
            return ready
        except (TaskError, TaskRevoked):
            self._ready = True
            return True
        except TaskNotFound:
            return False

    def successful(self) -> bool:
        """
        Check if the task completed successfully.

        Returns:
            True if the task finished with SUCCESS state
        """
        if not self.ready():
            return False
        return self.status == TaskState.SUCCESS

    def failed(self) -> bool:
        """
        Check if the task failed.

        Returns:
            True if the task finished with FAILURE state
        """
        if not self.ready():
            return False
        return self.status == TaskState.FAILURE

    def get(
        self,
        timeout: float | None = None,
        *,
        interval: float = 0.5,
        propagate: bool = True,
    ) -> Any:
        """
        Wait for task to complete and return the result.

        Args:
            timeout: Maximum time to wait in seconds (None = wait forever)
            interval: Polling interval in seconds
            propagate: Whether to re-raise exceptions from failed tasks

        Returns:
            Task result value

        Raises:
            TaskError: If task failed and propagate=True
            TaskRevoked: If task was revoked and propagate=True
            TimeoutError: If timeout exceeded
        """
        # If already ready, return cached result
        if self._ready:
            if propagate and self._status == TaskState.FAILURE:
                raise TaskError(
                    self._error or "Task failed",
                    task_id=self.task_id,
                    traceback=self._traceback,
                )
            if propagate and self._status == TaskState.REVOKED:
                raise TaskRevoked(self.task_id)
            return self._result

        # Try to get result via long polling first
        try:
            result = self._client.wait_for_result(self.task_id, timeout=timeout or 30)
            self._ready = True
            self._result = result
            self._status = TaskState.SUCCESS
            return result
        except TimeoutError:
            # Fall back to polling
            pass
        except TaskError as e:
            self._ready = True
            self._status = TaskState.FAILURE
            self._error = str(e)
            self._traceback = e.traceback
            if propagate:
                raise
            return None
        except TaskRevoked:
            self._ready = True
            self._status = TaskState.REVOKED
            if propagate:
                raise
            return None

        # Polling fallback
        start_time = time.time()
        while True:
            try:
                ready, result = self._client.get_result(self.task_id)
                if ready:
                    self._ready = True
                    self._result = result
                    self._status = TaskState.SUCCESS
                    return result
            except TaskError as e:
                self._ready = True
                self._status = TaskState.FAILURE
                self._error = str(e)
                self._traceback = e.traceback
                if propagate:
                    raise
                return None
            except TaskRevoked:
                self._ready = True
                self._status = TaskState.REVOKED
                if propagate:
                    raise
                return None

            # Check timeout
            if timeout is not None:
                elapsed = time.time() - start_time
                if elapsed >= timeout:
                    raise TimeoutError(
                        f"Timeout waiting for task {self.task_id}",
                        task_id=self.task_id,
                        timeout=timeout,
                    )

            time.sleep(interval)

    def forget(self) -> None:
        """
        Forget the result (delete from backend).

        Note: This does not revoke the task, just removes the result.
        """
        # Currently not implemented in the server
        pass

    def revoke(self, terminate: bool = False) -> bool:
        """
        Revoke/cancel the task.

        Args:
            terminate: Whether to terminate if already running

        Returns:
            True if revocation was successful
        """
        return self._client.revoke(self.task_id, terminate=terminate)

    def retry(self) -> "AsyncResult":
        """
        Retry a failed task.

        Returns:
            New AsyncResult for the retried task
        """
        new_task_id = self._client.retry(self.task_id)
        return AsyncResult(
            new_task_id,
            self._client,
            task_name=self.task_name,
        )

    def info(self) -> dict[str, Any]:
        """
        Get full task information.

        Returns:
            Dictionary with task metadata
        """
        return self._client.get_status(self.task_id)

    # Celery-compatible aliases
    def wait(
        self,
        timeout: float | None = None,
        *,
        interval: float = 0.5,
        propagate: bool = True,
    ) -> Any:
        """Alias for get() for Celery compatibility."""
        return self.get(timeout=timeout, interval=interval, propagate=propagate)


class GroupResult:
    """
    Represents the result of a group of parallel tasks.

    Example:
        result = group(add.s(1, 2), add.s(3, 4)).delay()

        # Wait for all results
        results = result.get()  # [3, 7]

        # Check if all complete
        if result.ready():
            print("All done!")
    """

    def __init__(
        self,
        workflow_id: str,
        results: list[AsyncResult],
        client: "TurbineClient",
    ):
        """
        Initialize GroupResult.

        Args:
            workflow_id: The workflow ID
            results: List of AsyncResult for each task
            client: TurbineClient instance
        """
        self.workflow_id = workflow_id
        self.results = results
        self._client = client

    def __repr__(self) -> str:
        return f"<GroupResult: {self.workflow_id} ({len(self.results)} tasks)>"

    def __iter__(self):
        return iter(self.results)

    def __len__(self) -> int:
        return len(self.results)

    @property
    def id(self) -> str:
        """Return workflow ID."""
        return self.workflow_id

    def ready(self) -> bool:
        """Check if all tasks have completed."""
        return all(r.ready() for r in self.results)

    def successful(self) -> bool:
        """Check if all tasks completed successfully."""
        return all(r.successful() for r in self.results)

    def failed(self) -> bool:
        """Check if any task failed."""
        return any(r.failed() for r in self.results)

    def completed_count(self) -> int:
        """Return number of completed tasks."""
        return sum(1 for r in self.results if r.ready())

    def get(
        self,
        timeout: float | None = None,
        *,
        interval: float = 0.5,
        propagate: bool = True,
    ) -> list[Any]:
        """
        Wait for all tasks to complete and return results.

        Args:
            timeout: Maximum time to wait in seconds
            interval: Polling interval in seconds
            propagate: Whether to re-raise exceptions

        Returns:
            List of results from each task
        """
        results = []
        start_time = time.time()

        for result in self.results:
            # Calculate remaining timeout
            if timeout is not None:
                elapsed = time.time() - start_time
                remaining = timeout - elapsed
                if remaining <= 0:
                    raise TimeoutError(
                        f"Timeout waiting for group {self.workflow_id}",
                        task_id=self.workflow_id,
                        timeout=timeout,
                    )
            else:
                remaining = None

            try:
                value = result.get(timeout=remaining, interval=interval, propagate=propagate)
                results.append(value)
            except (TaskError, TaskRevoked):
                if propagate:
                    raise
                results.append(None)

        return results

    def revoke(self, terminate: bool = False) -> None:
        """Revoke all tasks in the group."""
        for result in self.results:
            result.revoke(terminate=terminate)

    def info(self) -> dict[str, Any]:
        """Get workflow status information."""
        return self._client.get_workflow_status(self.workflow_id)
