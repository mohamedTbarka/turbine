"""
Turbine Python Worker - executes Python tasks from the queue.

This worker connects directly to Redis to consume tasks and execute
Python functions registered with the @task decorator.

Usage:
    from turbine.worker import Worker

    worker = Worker(
        broker_url="redis://localhost:6379",
        backend_url="redis://localhost:6379",
        queues=["default", "emails"],
    )
    worker.start()
"""

from __future__ import annotations

import importlib
import json
import logging
import signal
import sys
import threading
import time
import traceback
import uuid
from concurrent.futures import ThreadPoolExecutor
from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Callable

logger = logging.getLogger(__name__)


@dataclass
class WorkerConfig:
    """Worker configuration."""

    broker_url: str = "redis://localhost:6379"
    backend_url: str = "redis://localhost:6379"
    queues: list[str] = field(default_factory=lambda: ["default"])
    concurrency: int = 4
    prefetch_count: int = 1
    worker_id: str | None = None
    task_modules: list[str] = field(default_factory=list)
    default_timeout: int = 300
    log_level: str = "INFO"
    dlq_enabled: bool = True
    dlq_queue_name: str = "turbine.dlq"
    dlq_ttl: int = 604800  # 7 days

    def __post_init__(self):
        """Validate configuration after initialization."""
        if not self.queues:
            raise ValueError("At least one queue must be specified")
        if self.concurrency < 1:
            raise ValueError("Concurrency must be at least 1")


class TaskRegistry:
    """Registry for task handlers."""

    def __init__(self):
        self._tasks: dict[str, Callable] = {}
        self._lock = threading.Lock()

    def register(self, name: str, func: Callable) -> None:
        """Register a task handler."""
        with self._lock:
            self._tasks[name] = func
            logger.debug(f"Registered task: {name}")

    def get(self, name: str) -> Callable | None:
        """Get a task handler by name."""
        return self._tasks.get(name)

    def all(self) -> dict[str, Callable]:
        """Get all registered tasks."""
        return self._tasks.copy()

    def autodiscover(self, modules: list[str]) -> None:
        """
        Auto-discover tasks from modules.

        This imports the specified modules, which should trigger
        @task decorators to register tasks.
        """
        from turbine.task import Task

        for module_name in modules:
            try:
                module = importlib.import_module(module_name)
                # Find all Task objects in the module
                for name in dir(module):
                    obj = getattr(module, name)
                    if isinstance(obj, Task):
                        self.register(obj.name, obj)
                        logger.info(f"Discovered task: {obj.name} from {module_name}")
            except ImportError as e:
                logger.warning(f"Could not import {module_name}: {e}")
            except Exception as e:
                logger.error(f"Error discovering tasks in {module_name}: {e}")


# Global task registry
_registry = TaskRegistry()


def get_registry() -> TaskRegistry:
    """Get the global task registry."""
    return _registry


class RedisConnection:
    """Redis connection wrapper."""

    def __init__(self, url: str, decode_responses: bool = True):
        self.url = url
        self.decode_responses = decode_responses
        self._conn = None

    def connect(self):
        """Connect to Redis."""
        try:
            import redis
        except ImportError:
            raise ImportError(
                "redis package is required for the worker. "
                "Install it with: pip install redis"
            )

        self._conn = redis.from_url(self.url, decode_responses=self.decode_responses)
        # Test connection
        self._conn.ping()
        logger.info(f"Connected to Redis at {self.url}")

    def close(self):
        """Close the connection."""
        if self._conn:
            self._conn.close()
            self._conn = None

    @property
    def conn(self):
        """Get the Redis connection."""
        if self._conn is None:
            self.connect()
        return self._conn


class Worker:
    """
    Turbine Python Worker.

    Consumes tasks from Redis queues and executes Python functions.

    Example:
        worker = Worker(
            broker_url="redis://localhost:6379",
            backend_url="redis://localhost:6379",
            queues=["default", "emails"],
            concurrency=4,
        )

        # Discover tasks from modules
        worker.autodiscover(["myapp.tasks", "otherapp.tasks"])

        # Start processing
        worker.start()
    """

    def __init__(
        self,
        broker_url: str = "redis://localhost:6379",
        backend_url: str = "redis://localhost:6379",
        queues: list[str] | None = None,
        concurrency: int = 4,
        worker_id: str | None = None,
        task_modules: list[str] | None = None,
        default_timeout: int = 300,
    ):
        """
        Initialize the worker.

        Args:
            broker_url: Redis URL for the message broker
            backend_url: Redis URL for the result backend
            queues: List of queues to consume from
            concurrency: Number of concurrent task slots
            worker_id: Unique worker identifier
            task_modules: List of modules to import for task discovery
            default_timeout: Default task timeout in seconds
        """
        self.config = WorkerConfig(
            broker_url=broker_url,
            backend_url=backend_url,
            queues=queues or ["default"],
            concurrency=concurrency,
            worker_id=worker_id or f"worker-{uuid.uuid4().hex[:8]}",
            task_modules=task_modules or [],
            default_timeout=default_timeout,
        )

        # Both broker and backend use raw bytes for MessagePack
        self.broker = RedisConnection(broker_url, decode_responses=False)
        self.backend = RedisConnection(backend_url, decode_responses=False)
        self.registry = get_registry()

        self._executor: ThreadPoolExecutor | None = None
        self._running = False
        self._shutdown_event = threading.Event()

    def autodiscover(self, modules: list[str]) -> None:
        """
        Auto-discover tasks from modules.

        Args:
            modules: List of module names to import
        """
        self.registry.autodiscover(modules)

    def register_task(self, name: str, func: Callable) -> None:
        """
        Manually register a task handler.

        Args:
            name: Task name
            func: Task function
        """
        self.registry.register(name, func)

    def start(self) -> None:
        """Start the worker and begin processing tasks."""
        logger.info(f"Starting Turbine Python Worker {self.config.worker_id}")
        logger.info(f"Broker: {self.config.broker_url}")
        logger.info(f"Backend: {self.config.backend_url}")
        logger.info(f"Queues: {self.config.queues}")
        logger.info(f"Concurrency: {self.config.concurrency}")

        # Connect to Redis
        self.broker.connect()
        self.backend.connect()

        # Discover tasks from configured modules
        if self.config.task_modules:
            self.autodiscover(self.config.task_modules)

        # Log registered tasks
        tasks = self.registry.all()
        if tasks:
            logger.info(f"Registered tasks: {', '.join(tasks.keys())}")
        else:
            logger.warning("No tasks registered!")

        # Set up signal handlers
        signal.signal(signal.SIGINT, self._signal_handler)
        signal.signal(signal.SIGTERM, self._signal_handler)

        # Start executor
        self._executor = ThreadPoolExecutor(max_workers=self.config.concurrency)
        self._running = True

        logger.info("Worker ready, waiting for tasks...")

        try:
            self._consume_loop()
        except KeyboardInterrupt:
            logger.info("Received keyboard interrupt")
        finally:
            self.stop()

    def stop(self) -> None:
        """Stop the worker gracefully."""
        logger.info("Shutting down worker...")
        self._running = False
        self._shutdown_event.set()

        if self._executor:
            self._executor.shutdown(wait=True, cancel_futures=False)
            self._executor = None

        self.broker.close()
        self.backend.close()
        logger.info("Worker stopped")

    def _signal_handler(self, signum, frame):
        """Handle shutdown signals."""
        logger.info(f"Received signal {signum}, shutting down...")
        self._running = False
        self._shutdown_event.set()

    def _consume_loop(self) -> None:
        """Main consumption loop."""
        # Build queue keys for Redis
        queue_keys = [f"turbine:queue:{q}" for q in self.config.queues]

        # Try to import msgpack for MessagePack support
        try:
            import msgpack
            use_msgpack = True
        except ImportError:
            use_msgpack = False
            logger.warning("msgpack not installed, falling back to JSON only")

        while self._running:
            try:
                # BLPOP from multiple queues with timeout
                result = self.broker.conn.blpop(queue_keys, timeout=1)

                if result is None:
                    continue

                queue_key, message_data = result

                # Parse the message (try MessagePack first, then JSON)
                message = None
                logger.debug(f"Received raw message: {message_data[:100] if len(message_data) > 100 else message_data}")

                if use_msgpack and isinstance(message_data, bytes):
                    try:
                        message = msgpack.unpackb(message_data, raw=False)
                        logger.debug(f"Parsed MessagePack: {message}")
                    except Exception as e:
                        logger.debug(f"MessagePack parse failed: {e}")

                if message is None:
                    try:
                        if isinstance(message_data, bytes):
                            message_data = message_data.decode('utf-8')
                        message = json.loads(message_data)
                        logger.debug(f"Parsed JSON: {message}")
                    except (json.JSONDecodeError, UnicodeDecodeError) as e:
                        logger.error(f"Invalid message format: {e}")
                        continue

                # Handle Rust server message format: [metadata_array, task_bytes]
                if isinstance(message, list) and len(message) == 2:
                    metadata = message[0]
                    task_payload = message[1]

                    # Extract from metadata array: [task_id, name, queue, priority, ...]
                    if isinstance(metadata, list) and len(metadata) >= 2:
                        task_id = metadata[0]
                        task_name = metadata[1]

                        # Parse nested task payload if it's bytes or list of bytes
                        args = []
                        kwargs = {}

                        if isinstance(task_payload, (bytes, bytearray)):
                            try:
                                task_data = msgpack.unpackb(task_payload, raw=False)
                                logger.debug(f"Parsed nested task: {task_data}")
                            except:
                                task_data = None
                        elif isinstance(task_payload, list):
                            # Already decoded as list of ints (bytes)
                            try:
                                task_bytes = bytes(task_payload)
                                task_data = msgpack.unpackb(task_bytes, raw=False)
                                logger.debug(f"Parsed nested task from list: {task_data}")
                            except Exception as e:
                                logger.debug(f"Failed to parse nested task: {e}")
                                task_data = None
                        else:
                            task_data = task_payload

                        # Extract args/kwargs from task_data
                        if isinstance(task_data, list) and len(task_data) >= 4:
                            # Format: [task_id, name, args, kwargs, ...]
                            args = task_data[2] if len(task_data) > 2 and isinstance(task_data[2], list) else []
                            kwargs = task_data[3] if len(task_data) > 3 and isinstance(task_data[3], dict) else {}
                        elif isinstance(task_data, dict):
                            args = task_data.get("args", [])
                            kwargs = task_data.get("kwargs", {})

                        message = {
                            "task_id": task_id,
                            "name": task_name,
                            "args": args,
                            "kwargs": kwargs,
                        }
                        logger.debug(f"Extracted message: {message}")

                # Handle dict format with nested task
                elif isinstance(message, dict) and "task" in message and isinstance(message["task"], dict):
                    task_data = message["task"]
                    message = {
                        "task_id": message.get("task_id", task_data.get("id")),
                        "name": task_data.get("name"),
                        "args": task_data.get("args", []),
                        "kwargs": task_data.get("kwargs", {}),
                    }

                # Submit task for execution
                if self._executor:
                    self._executor.submit(self._execute_task, message)

            except Exception as e:
                if self._running:
                    logger.error(f"Error in consume loop: {e}")
                    time.sleep(1)

    def _execute_task(self, message: dict) -> None:
        """Execute a single task."""
        task_id = message.get("task_id", "unknown")
        task_name = message.get("name", "unknown")
        args = message.get("args", [])
        kwargs = message.get("kwargs", {})
        retries = message.get("retries", 0)
        max_retries = message.get("max_retries", 3)

        logger.info(f"Executing task {task_name}[{task_id}]")

        # Update status to running
        self._store_status(task_id, "running")

        start_time = time.time()

        try:
            # Get task handler
            handler = self.registry.get(task_name)

            if handler is None:
                raise ValueError(f"No handler registered for task '{task_name}'")

            # Execute the task
            result = handler(*args, **kwargs)

            # Calculate execution time
            execution_time = time.time() - start_time

            # Store success result
            self._store_result(
                task_id,
                status="success",
                result=result,
                execution_time=execution_time,
            )

            logger.info(
                f"Task {task_name}[{task_id}] succeeded in {execution_time:.3f}s"
            )

        except Exception as e:
            execution_time = time.time() - start_time
            error_msg = str(e)
            tb = traceback.format_exc()

            # Store failure result
            self._store_result(
                task_id,
                status="failure",
                error=error_msg,
                traceback=tb,
                execution_time=execution_time,
            )

            # Route to DLQ if retries exceeded and DLQ is enabled
            if self.config.dlq_enabled and retries >= max_retries:
                self._send_to_dlq(
                    task_id=task_id,
                    task_name=task_name,
                    args=args,
                    kwargs=kwargs,
                    error=error_msg,
                    traceback=tb,
                    retries=retries,
                )

            logger.error(
                f"Task {task_name}[{task_id}] failed in {execution_time:.3f}s: {error_msg}"
            )

    def _store_status(self, task_id: str, status: str) -> None:
        """Store task status in the backend (compatible with Rust backend)."""
        # Store state string for quick lookups (turbine:state:{task_id})
        state_key = f"turbine:state:{task_id}"
        self.backend.conn.set(state_key, status, ex=86400)

    def _store_result(
        self,
        task_id: str,
        status: str,
        result: Any = None,
        error: str | None = None,
        traceback: str | None = None,
        execution_time: float | None = None,
    ) -> None:
        """Store task result in the backend (compatible with Rust backend)."""
        try:
            import msgpack
        except ImportError:
            logger.warning("msgpack not installed, skipping result storage")
            return

        now = datetime.utcnow().isoformat() + "Z"

        # Build TaskResult structure matching Rust's TaskResult
        task_result = {
            "task_id": task_id,
            "state": status,  # "success", "failure", etc.
            "result": result,  # Will be serialized as-is
            "error": error,
            "traceback": traceback,
            "created_at": now,
        }

        # Store serialized TaskResult (turbine:result:{task_id})
        result_key = f"turbine:result:{task_id}"
        result_data = msgpack.packb(task_result, use_bin_type=True)
        self.backend.conn.set(result_key, result_data, ex=86400)

        # Store state string for quick lookups (turbine:state:{task_id})
        state_key = f"turbine:state:{task_id}"
        self.backend.conn.set(state_key, status, ex=86400)

        # Store TaskMeta for status queries (turbine:meta:{task_id})
        task_meta = {
            "state": status,
            "retries": 0,
            "created_at": now,
            "started_at": now if status == "running" else None,
            "finished_at": now if status in ("success", "failure") else None,
            "worker_id": self.config.worker_id,
            "error": error,
            "traceback": traceback,
        }
        meta_key = f"turbine:meta:{task_id}"
        meta_data = msgpack.packb(task_meta, use_bin_type=True)
        self.backend.conn.set(meta_key, meta_data, ex=86400)

    def _send_to_dlq(
        self,
        task_id: str,
        task_name: str,
        args: list,
        kwargs: dict,
        error: str,
        traceback: str,
        retries: int,
    ) -> None:
        """
        Send a permanently failed task to the Dead Letter Queue.

        Args:
            task_id: Task ID
            task_name: Task name
            args: Task arguments
            kwargs: Task keyword arguments
            error: Error message
            traceback: Error traceback
            retries: Number of retries attempted
        """
        try:
            import msgpack
        except ImportError:
            logger.warning("msgpack not installed, cannot send to DLQ")
            return

        try:
            dlq_entry = {
                "task_id": task_id,
                "task_name": task_name,
                "args": args,
                "kwargs": kwargs,
                "error": error,
                "traceback": traceback,
                "retries": retries,
                "failed_at": datetime.utcnow().isoformat() + "Z",
                "worker_id": self.config.worker_id,
            }

            # Store in DLQ with task_id as key
            dlq_key = f"turbine:dlq:{task_id}"
            dlq_data = msgpack.packb(dlq_entry, use_bin_type=True)
            self.backend.conn.set(dlq_key, dlq_data, ex=self.config.dlq_ttl)

            # Also add to DLQ index (sorted set by timestamp)
            timestamp = int(time.time())
            self.backend.conn.zadd(
                f"turbine:dlq:index",
                {task_id: timestamp}
            )

            logger.warning(
                f"Task {task_name}[{task_id}] sent to DLQ after {retries} retries"
            )

        except Exception as e:
            logger.error(f"Failed to send task {task_id} to DLQ: {e}")


def run_worker(
    broker_url: str = "redis://localhost:6379",
    backend_url: str = "redis://localhost:6379",
    queues: list[str] | None = None,
    concurrency: int = 4,
    task_modules: list[str] | None = None,
    log_level: str = "INFO",
) -> None:
    """
    Convenience function to run a worker.

    Args:
        broker_url: Redis URL for the broker
        backend_url: Redis URL for the backend
        queues: Queues to consume from
        concurrency: Number of concurrent workers
        task_modules: Modules to import for task discovery
        log_level: Logging level
    """
    # Configure logging
    logging.basicConfig(
        level=getattr(logging, log_level.upper()),
        format="%(asctime)s %(levelname)s %(name)s: %(message)s",
    )

    worker = Worker(
        broker_url=broker_url,
        backend_url=backend_url,
        queues=queues,
        concurrency=concurrency,
        task_modules=task_modules,
    )

    worker.start()
