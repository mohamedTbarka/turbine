"""Turbine gRPC client implementation."""

from __future__ import annotations

import json
import logging
from typing import Any, TYPE_CHECKING

from turbine.exceptions import ConnectionError, TaskError, TaskNotFound, TaskRevoked

logger = logging.getLogger(__name__)

# Optional gRPC support
try:
    import grpc
    GRPC_INSTALLED = True
except ImportError:
    grpc = None  # type: ignore
    GRPC_INSTALLED = False

# We'll use dynamic proto loading or pre-generated stubs
# For now, implement a simple HTTP fallback and gRPC when available
GRPC_AVAILABLE = False
turbine_pb2 = None
turbine_pb2_grpc = None

if GRPC_INSTALLED:
    try:
        from turbine._proto import turbine_pb2, turbine_pb2_grpc
        GRPC_AVAILABLE = True
    except ImportError:
        logger.warning("gRPC stubs not generated. Run 'python -m turbine.generate_proto' first.")


class TaskState:
    """Task state constants."""

    PENDING = "PENDING"
    RECEIVED = "RECEIVED"
    RUNNING = "RUNNING"
    SUCCESS = "SUCCESS"
    FAILURE = "FAILURE"
    RETRY = "RETRY"
    REVOKED = "REVOKED"

    @classmethod
    def from_proto(cls, state: int) -> str:
        """Convert proto enum to string."""
        mapping = {
            0: "UNSPECIFIED",
            1: cls.PENDING,
            2: cls.RECEIVED,
            3: cls.RUNNING,
            4: cls.SUCCESS,
            5: cls.FAILURE,
            6: cls.RETRY,
            7: cls.REVOKED,
        }
        return mapping.get(state, "UNKNOWN")


class TaskOptions:
    """Options for task execution."""

    def __init__(
        self,
        queue: str = "default",
        priority: int = 0,
        max_retries: int = 3,
        retry_delay: int = 60,
        timeout: int = 300,
        soft_timeout: int | None = None,
        eta: int | None = None,
        countdown: int | None = None,
        expires: int | None = None,
        store_result: bool = True,
        result_ttl: int = 86400,
        idempotency_key: str | None = None,
        headers: dict[str, str] | None = None,
    ):
        self.queue = queue
        self.priority = priority
        self.max_retries = max_retries
        self.retry_delay = retry_delay
        self.timeout = timeout
        self.soft_timeout = soft_timeout
        self.eta = eta
        self.countdown = countdown
        self.expires = expires
        self.store_result = store_result
        self.result_ttl = result_ttl
        self.idempotency_key = idempotency_key
        self.headers = headers or {}

    def to_proto(self) -> Any:
        """Convert to protobuf message."""
        if not GRPC_AVAILABLE:
            raise RuntimeError("gRPC stubs not available")

        opts = turbine_pb2.TaskOptions(
            queue=self.queue,
            priority=self.priority,
            max_retries=self.max_retries,
            retry_delay=self.retry_delay,
            timeout=self.timeout,
            store_result=self.store_result,
            result_ttl=self.result_ttl,
            headers=self.headers,
        )

        if self.soft_timeout is not None:
            opts.soft_timeout = self.soft_timeout
        if self.eta is not None:
            opts.eta = self.eta
        if self.countdown is not None:
            opts.countdown = self.countdown
        if self.expires is not None:
            opts.expires = self.expires
        if self.idempotency_key is not None:
            opts.idempotency_key = self.idempotency_key

        return opts

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for JSON serialization."""
        d = {
            "queue": self.queue,
            "priority": self.priority,
            "max_retries": self.max_retries,
            "retry_delay": self.retry_delay,
            "timeout": self.timeout,
            "store_result": self.store_result,
            "result_ttl": self.result_ttl,
            "headers": self.headers,
        }
        if self.soft_timeout is not None:
            d["soft_timeout"] = self.soft_timeout
        if self.eta is not None:
            d["eta"] = self.eta
        if self.countdown is not None:
            d["countdown"] = self.countdown
        if self.expires is not None:
            d["expires"] = self.expires
        if self.idempotency_key is not None:
            d["idempotency_key"] = self.idempotency_key
        return d


class TurbineClient:
    """Client for communicating with Turbine server."""

    def __init__(
        self,
        server: str = "localhost:50051",
        *,
        secure: bool = False,
        credentials: Any = None,
        timeout: float = 30.0,
    ):
        """
        Initialize Turbine client.

        Args:
            server: Turbine server address (host:port)
            secure: Whether to use TLS
            credentials: Optional gRPC credentials for secure connection
            timeout: Default timeout for RPC calls in seconds
        """
        self.server = server
        self.secure = secure
        self.timeout = timeout
        self._channel: Any = None
        self._stub: Any = None
        self._workflow_stub: Any = None

        if secure and credentials is None and GRPC_INSTALLED:
            credentials = grpc.ssl_channel_credentials()

        self._credentials = credentials

    def _ensure_connected(self) -> None:
        """Ensure gRPC channel is connected."""
        if not GRPC_INSTALLED:
            raise RuntimeError(
                "gRPC not installed. Install with: pip install grpcio grpcio-tools"
            )
        if not GRPC_AVAILABLE:
            raise RuntimeError(
                "gRPC stubs not available. Run 'python -m turbine.generate_proto' first."
            )

        if self._channel is None:
            if self.secure:
                self._channel = grpc.secure_channel(self.server, self._credentials)
            else:
                self._channel = grpc.insecure_channel(self.server)

            self._stub = turbine_pb2_grpc.TurbineServiceStub(self._channel)
            self._workflow_stub = turbine_pb2_grpc.WorkflowServiceStub(self._channel)

    def close(self) -> None:
        """Close the gRPC channel."""
        if self._channel is not None:
            self._channel.close()
            self._channel = None
            self._stub = None
            self._workflow_stub = None

    def __enter__(self) -> "TurbineClient":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    def health_check(self) -> dict[str, Any]:
        """Check server health."""
        self._ensure_connected()
        try:
            response = self._stub.HealthCheck(
                turbine_pb2.HealthCheckRequest(),
                timeout=self.timeout,
            )
            return {
                "status": response.status,
                "version": response.version,
                "uptime": response.uptime,
                "broker": response.broker_status,
                "backend": response.backend_status,
            }
        except grpc.RpcError as e:
            raise ConnectionError(f"Health check failed: {e}") from e

    def submit_task(
        self,
        name: str,
        args: list[Any] | None = None,
        kwargs: dict[str, Any] | None = None,
        options: TaskOptions | None = None,
        task_id: str | None = None,
        correlation_id: str | None = None,
    ) -> str:
        """
        Submit a task for execution.

        Args:
            name: Task name (handler name)
            args: Positional arguments
            kwargs: Keyword arguments
            options: Task options
            task_id: Optional task ID (auto-generated if not provided)
            correlation_id: Optional correlation ID for tracing

        Returns:
            Task ID
        """
        self._ensure_connected()

        args = args or []
        kwargs = kwargs or {}
        options = options or TaskOptions()

        # Serialize arguments to JSON bytes
        args_bytes = [json.dumps(arg).encode() for arg in args]
        kwargs_bytes = {k: json.dumps(v).encode() for k, v in kwargs.items()}

        request = turbine_pb2.SubmitTaskRequest(
            name=name,
            args=args_bytes,
            kwargs=kwargs_bytes,
            options=options.to_proto(),
        )

        if task_id:
            request.task_id = task_id
        if correlation_id:
            request.correlation_id = correlation_id

        try:
            response = self._stub.SubmitTask(request, timeout=self.timeout)
            return response.task_id
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to submit task: {e}") from e

    def submit_batch(
        self,
        tasks: list[dict[str, Any]],
    ) -> list[str]:
        """
        Submit multiple tasks in batch.

        Args:
            tasks: List of task dictionaries with name, args, kwargs, options

        Returns:
            List of task IDs
        """
        self._ensure_connected()

        requests = []
        for task in tasks:
            args = task.get("args", [])
            kwargs = task.get("kwargs", {})
            options = task.get("options", TaskOptions())

            if isinstance(options, dict):
                options = TaskOptions(**options)

            args_bytes = [json.dumps(arg).encode() for arg in args]
            kwargs_bytes = {k: json.dumps(v).encode() for k, v in kwargs.items()}

            req = turbine_pb2.SubmitTaskRequest(
                name=task["name"],
                args=args_bytes,
                kwargs=kwargs_bytes,
                options=options.to_proto(),
            )
            requests.append(req)

        try:
            response = self._stub.SubmitBatch(
                turbine_pb2.SubmitBatchRequest(tasks=requests),
                timeout=self.timeout,
            )
            return list(response.task_ids)
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to submit batch: {e}") from e

    def get_task_status(self, task_id: str) -> dict[str, Any]:
        """
        Get task status.

        Args:
            task_id: Task ID

        Returns:
            Task metadata dictionary
        """
        self._ensure_connected()

        try:
            response = self._stub.GetTaskStatus(
                turbine_pb2.GetTaskStatusRequest(task_id=task_id),
                timeout=self.timeout,
            )

            meta = response.meta
            result = {
                "task_id": response.task_id,
                "state": TaskState.from_proto(meta.state),
                "retries": meta.retries,
                "created_at": meta.created_at,
            }

            if meta.HasField("started_at"):
                result["started_at"] = meta.started_at
            if meta.HasField("finished_at"):
                result["finished_at"] = meta.finished_at
            if meta.HasField("worker_id"):
                result["worker_id"] = meta.worker_id
            if meta.HasField("error"):
                result["error"] = meta.error
            if meta.HasField("traceback"):
                result["traceback"] = meta.traceback

            return result
        except grpc.RpcError as e:
            if e.code() == grpc.StatusCode.NOT_FOUND:
                raise TaskNotFound(task_id) from e
            raise ConnectionError(f"Failed to get status: {e}") from e

    # Alias for backward compatibility
    get_status = get_task_status

    def get_result(self, task_id: str) -> tuple[bool, Any]:
        """
        Get task result (non-blocking).

        Args:
            task_id: Task ID

        Returns:
            Tuple of (ready, result)
        """
        self._ensure_connected()

        try:
            response = self._stub.GetTaskResult(
                turbine_pb2.GetTaskResultRequest(task_id=task_id),
                timeout=self.timeout,
            )

            if not response.ready:
                return False, None

            result = response.result
            state = TaskState.from_proto(result.state)

            if state == TaskState.SUCCESS:
                if result.HasField("result"):
                    return True, json.loads(result.result.decode())
                return True, None
            elif state == TaskState.FAILURE:
                error = result.error if result.HasField("error") else "Unknown error"
                traceback = result.traceback if result.HasField("traceback") else None
                raise TaskError(error, task_id=task_id, traceback=traceback)
            elif state == TaskState.REVOKED:
                raise TaskRevoked(task_id)
            else:
                return False, None

        except grpc.RpcError as e:
            if e.code() == grpc.StatusCode.NOT_FOUND:
                raise TaskNotFound(task_id) from e
            raise ConnectionError(f"Failed to get result: {e}") from e

    def wait_for_result(self, task_id: str, timeout: float | None = None) -> Any:
        """
        Wait for task result (blocking).

        Args:
            task_id: Task ID
            timeout: Timeout in seconds (default: client timeout)

        Returns:
            Task result value

        Raises:
            TaskError: If task failed
            TaskRevoked: If task was revoked
            TimeoutError: If timeout exceeded
        """
        self._ensure_connected()

        timeout = timeout or self.timeout

        try:
            response = self._stub.WaitForResult(
                turbine_pb2.WaitForResultRequest(
                    task_id=task_id,
                    timeout=int(timeout),
                ),
                timeout=timeout + 5,  # Add buffer for RPC overhead
            )

            if not response.success:
                from turbine.exceptions import TimeoutError

                raise TimeoutError(
                    f"Timeout waiting for task {task_id}",
                    task_id=task_id,
                    timeout=timeout,
                )

            result = response.result
            state = TaskState.from_proto(result.state)

            if state == TaskState.SUCCESS:
                if result.HasField("result"):
                    return json.loads(result.result.decode())
                return None
            elif state == TaskState.FAILURE:
                error = result.error if result.HasField("error") else "Unknown error"
                traceback = result.traceback if result.HasField("traceback") else None
                raise TaskError(error, task_id=task_id, traceback=traceback)
            elif state == TaskState.REVOKED:
                raise TaskRevoked(task_id)
            else:
                from turbine.exceptions import TimeoutError

                raise TimeoutError(
                    f"Task {task_id} not ready",
                    task_id=task_id,
                    timeout=timeout,
                )

        except grpc.RpcError as e:
            if e.code() == grpc.StatusCode.NOT_FOUND:
                raise TaskNotFound(task_id) from e
            if e.code() == grpc.StatusCode.DEADLINE_EXCEEDED:
                from turbine.exceptions import TimeoutError

                raise TimeoutError(
                    f"Timeout waiting for task {task_id}",
                    task_id=task_id,
                    timeout=timeout,
                ) from e
            raise ConnectionError(f"Failed to wait for result: {e}") from e

    def revoke(self, task_id: str, terminate: bool = False) -> bool:
        """
        Revoke/cancel a task.

        Args:
            task_id: Task ID
            terminate: Whether to terminate if already running

        Returns:
            True if revocation was successful
        """
        self._ensure_connected()

        try:
            response = self._stub.RevokeTask(
                turbine_pb2.RevokeTaskRequest(
                    task_id=task_id,
                    terminate=terminate,
                ),
                timeout=self.timeout,
            )
            return response.success
        except grpc.RpcError as e:
            if e.code() == grpc.StatusCode.NOT_FOUND:
                raise TaskNotFound(task_id) from e
            raise ConnectionError(f"Failed to revoke task: {e}") from e

    def retry(self, task_id: str) -> str:
        """
        Retry a failed task.

        Args:
            task_id: Task ID

        Returns:
            New task ID
        """
        self._ensure_connected()

        try:
            response = self._stub.RetryTask(
                turbine_pb2.RetryTaskRequest(task_id=task_id),
                timeout=self.timeout,
            )

            if not response.success:
                raise TaskError(f"Failed to retry task {task_id}", task_id=task_id)

            return response.new_task_id
        except grpc.RpcError as e:
            if e.code() == grpc.StatusCode.NOT_FOUND:
                raise TaskNotFound(task_id) from e
            raise ConnectionError(f"Failed to retry task: {e}") from e

    def get_queue_info(self, queue: str | None = None) -> list[dict[str, Any]]:
        """
        Get queue information.

        Args:
            queue: Queue name (optional, returns all if not specified)

        Returns:
            List of queue info dictionaries
        """
        self._ensure_connected()

        request = turbine_pb2.GetQueueInfoRequest()
        if queue:
            request.queue = queue

        try:
            response = self._stub.GetQueueInfo(request, timeout=self.timeout)
            return [
                {
                    "name": q.name,
                    "pending": q.pending,
                    "processing": q.processing,
                    "consumers": q.consumers,
                    "throughput": q.throughput,
                }
                for q in response.queues
            ]
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to get queue info: {e}") from e

    def purge_queue(self, queue: str) -> int:
        """
        Purge all messages from a queue.

        Args:
            queue: Queue name

        Returns:
            Number of messages purged
        """
        self._ensure_connected()

        try:
            response = self._stub.PurgeQueue(
                turbine_pb2.PurgeQueueRequest(queue=queue),
                timeout=self.timeout,
            )
            return response.purged
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to purge queue: {e}") from e

    # Workflow methods

    def submit_chain(
        self,
        tasks: list[dict[str, Any]],
        *,
        stop_on_failure: bool = True,
        pass_results: bool = True,
    ) -> tuple[str, list[str]]:
        """
        Submit a chain of tasks.

        Args:
            tasks: List of task dictionaries
            stop_on_failure: Whether to stop chain on first failure
            pass_results: Whether to pass results between tasks

        Returns:
            Tuple of (workflow_id, task_ids)
        """
        self._ensure_connected()

        requests = self._build_task_requests(tasks)

        try:
            response = self._workflow_stub.SubmitChain(
                turbine_pb2.SubmitChainRequest(
                    tasks=requests,
                    options=turbine_pb2.ChainOptions(
                        stop_on_failure=stop_on_failure,
                        pass_results=pass_results,
                    ),
                ),
                timeout=self.timeout,
            )
            return response.workflow_id, list(response.task_ids)
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to submit chain: {e}") from e

    def submit_group(
        self,
        tasks: list[dict[str, Any]],
        *,
        max_concurrency: int = 0,
        continue_on_failure: bool = False,
    ) -> tuple[str, list[str]]:
        """
        Submit a group of parallel tasks.

        Args:
            tasks: List of task dictionaries
            max_concurrency: Maximum concurrent tasks (0 = unlimited)
            continue_on_failure: Whether to continue on task failure

        Returns:
            Tuple of (workflow_id, task_ids)
        """
        self._ensure_connected()

        requests = self._build_task_requests(tasks)

        try:
            response = self._workflow_stub.SubmitGroup(
                turbine_pb2.SubmitGroupRequest(
                    tasks=requests,
                    options=turbine_pb2.GroupOptions(
                        max_concurrency=max_concurrency,
                        continue_on_failure=continue_on_failure,
                    ),
                ),
                timeout=self.timeout,
            )
            return response.workflow_id, list(response.task_ids)
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to submit group: {e}") from e

    def submit_chord(
        self,
        tasks: list[dict[str, Any]],
        callback: dict[str, Any],
        *,
        max_concurrency: int = 0,
        continue_on_failure: bool = False,
        pass_results: bool = True,
    ) -> tuple[str, list[str]]:
        """
        Submit a chord (group + callback).

        Args:
            tasks: List of task dictionaries for the group
            callback: Callback task dictionary
            max_concurrency: Maximum concurrent tasks in group
            continue_on_failure: Whether to continue on task failure
            pass_results: Whether to pass group results to callback

        Returns:
            Tuple of (workflow_id, task_ids)
        """
        self._ensure_connected()

        group_requests = self._build_task_requests(tasks)
        callback_request = self._build_task_requests([callback])[0]

        try:
            response = self._workflow_stub.SubmitChord(
                turbine_pb2.SubmitChordRequest(
                    group=turbine_pb2.SubmitGroupRequest(
                        tasks=group_requests,
                        options=turbine_pb2.GroupOptions(
                            max_concurrency=max_concurrency,
                            continue_on_failure=continue_on_failure,
                        ),
                    ),
                    callback=callback_request,
                    options=turbine_pb2.ChordOptions(
                        pass_results=pass_results,
                        execute_on_partial_failure=continue_on_failure,
                    ),
                ),
                timeout=self.timeout,
            )
            return response.workflow_id, list(response.task_ids)
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to submit chord: {e}") from e

    def get_workflow_status(self, workflow_id: str) -> dict[str, Any]:
        """
        Get workflow status.

        Args:
            workflow_id: Workflow ID

        Returns:
            Workflow status dictionary
        """
        self._ensure_connected()

        try:
            response = self._workflow_stub.GetWorkflowStatus(
                turbine_pb2.GetWorkflowStatusRequest(workflow_id=workflow_id),
                timeout=self.timeout,
            )

            return {
                "workflow_id": response.workflow_id,
                "state": TaskState.from_proto(response.state),
                "completed": response.completed,
                "total": response.total,
                "task_statuses": [
                    {
                        "state": TaskState.from_proto(t.state),
                        "retries": t.retries,
                        "created_at": t.created_at,
                        "started_at": t.started_at if t.HasField("started_at") else None,
                        "finished_at": t.finished_at if t.HasField("finished_at") else None,
                        "worker_id": t.worker_id if t.HasField("worker_id") else None,
                        "error": t.error if t.HasField("error") else None,
                    }
                    for t in response.task_statuses
                ],
            }
        except grpc.RpcError as e:
            raise ConnectionError(f"Failed to get workflow status: {e}") from e

    def _build_task_requests(self, tasks: list[dict[str, Any]]) -> list[Any]:
        """Build list of SubmitTaskRequest messages."""
        requests = []
        for task in tasks:
            args = task.get("args", [])
            kwargs = task.get("kwargs", {})
            options = task.get("options", TaskOptions())

            if isinstance(options, dict):
                options = TaskOptions(**options)

            args_bytes = [json.dumps(arg).encode() for arg in args]
            kwargs_bytes = {k: json.dumps(v).encode() for k, v in kwargs.items()}

            req = turbine_pb2.SubmitTaskRequest(
                name=task["name"],
                args=args_bytes,
                kwargs=kwargs_bytes,
                options=options.to_proto(),
            )
            requests.append(req)

        return requests
