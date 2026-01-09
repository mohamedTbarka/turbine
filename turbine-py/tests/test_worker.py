"""Unit tests for the Turbine Python worker."""

import pytest
from unittest.mock import Mock, patch, MagicMock
from turbine.worker import Worker, WorkerConfig, get_registry
from turbine.task import task
import msgpack


@pytest.fixture
def worker_config():
    """Create a worker configuration for testing."""
    return WorkerConfig(
        worker_id="test-worker",
        broker_url="redis://localhost:6379",
        backend_url="redis://localhost:6379",
        queues=["default"],
        concurrency=2,
        task_modules=[],
    )


@pytest.fixture
def mock_redis_connection():
    """Mock Redis connection."""
    with patch('turbine.worker.RedisConnection') as mock:
        conn_instance = MagicMock()
        mock.return_value = conn_instance
        yield conn_instance


@pytest.fixture
def worker(worker_config, mock_redis_connection):
    """Create a worker instance with mocked Redis."""
    return Worker(
        broker_url=worker_config.broker_url,
        backend_url=worker_config.backend_url,
        queues=worker_config.queues,
        concurrency=worker_config.concurrency,
    )


def test_worker_config_defaults():
    """Test that worker config has sensible defaults."""
    config = WorkerConfig(
        broker_url="redis://localhost:6379",
        backend_url="redis://localhost:6379",
        queues=["default"],
    )

    assert config.concurrency == 4
    assert config.task_modules == []
    assert config.default_timeout == 300
    assert "worker-" in config.worker_id


def test_worker_initialization(worker, worker_config):
    """Test worker initialization."""
    assert worker.config.worker_id == worker_config.worker_id
    assert worker.config.queues == ["default"]
    assert worker.config.concurrency == 2


def test_task_registry():
    """Test task registration."""
    registry = get_registry()
    registry.clear()

    @task(name="test_task")
    def test_task_func():
        return "test result"

    assert "test_task" in registry
    assert registry["test_task"].func == test_task_func


def test_store_result_format(worker):
    """Test that results are stored in the correct MessagePack format."""
    task_id = "test-task-id"
    status = "success"
    result = {"key": "value"}

    worker._store_result(
        task_id=task_id,
        status=status,
        result=result,
        error=None,
        traceback=None,
        execution_time=1.5,
    )

    # Verify state key was set
    worker.backend.conn.set.assert_any_call(
        f"turbine:state:{task_id}",
        status,
        ex=86400
    )

    # Verify result and meta were stored
    calls = worker.backend.conn.set.call_args_list
    assert len(calls) == 3  # state, result, meta

    # Check that data is MessagePack encoded
    for call in calls:
        key = call[0][0]
        if key.startswith("turbine:result:") or key.startswith("turbine:meta:"):
            data = call[0][1]
            # Should be bytes (MessagePack)
            assert isinstance(data, bytes)
            # Should be decodeable
            decoded = msgpack.unpackb(data, raw=False)
            assert isinstance(decoded, dict)


def test_store_result_with_error(worker):
    """Test storing a failed task result."""
    task_id = "test-task-id-2"
    status = "failure"
    error = "Test error message"
    traceback_str = "Traceback (most recent call last)..."

    worker._store_result(
        task_id=task_id,
        status=status,
        result=None,
        error=error,
        traceback=traceback_str,
        execution_time=0.5,
    )

    # Verify state was set to failure
    worker.backend.conn.set.assert_any_call(
        f"turbine:state:{task_id}",
        status,
        ex=86400
    )


def test_execute_task_success(worker):
    """Test successful task execution."""
    @task(name="test_success_task")
    def success_task(x, y):
        return x + y

    task_id = "test-task-success"
    task_name = "test_success_task"
    task_args = [2, 3]
    task_kwargs = {}

    with patch.object(worker, '_store_result') as mock_store:
        worker._execute_task(task_id, task_name, task_args, task_kwargs)

        # Verify result was stored
        mock_store.assert_called_once()
        call_args = mock_store.call_args
        assert call_args[1]['task_id'] == task_id
        assert call_args[1]['status'] == 'success'
        assert call_args[1]['result'] == 5
        assert call_args[1]['error'] is None


def test_execute_task_failure(worker):
    """Test task execution with exception."""
    @task(name="test_failure_task")
    def failure_task():
        raise ValueError("Test error")

    task_id = "test-task-failure"
    task_name = "test_failure_task"

    with patch.object(worker, '_store_result') as mock_store:
        worker._execute_task(task_id, task_name, [], {})

        # Verify error was stored
        mock_store.assert_called_once()
        call_args = mock_store.call_args
        assert call_args[1]['task_id'] == task_id
        assert call_args[1]['status'] == 'failure'
        assert call_args[1]['result'] is None
        assert "Test error" in call_args[1]['error']
        assert call_args[1]['traceback'] is not None


def test_execute_task_not_found(worker):
    """Test execution of non-existent task."""
    task_id = "test-task-notfound"
    task_name = "nonexistent_task"

    with patch.object(worker, '_store_result') as mock_store:
        worker._execute_task(task_id, task_name, [], {})

        # Verify error was stored
        mock_store.assert_called_once()
        call_args = mock_store.call_args
        assert call_args[1]['status'] == 'failure'
        assert "not found" in call_args[1]['error'].lower()


def test_worker_config_validation():
    """Test worker config validation."""
    # Test empty queues
    with pytest.raises(ValueError):
        WorkerConfig(
            broker_url="redis://localhost",
            backend_url="redis://localhost",
            queues=[],
        )

    # Test invalid concurrency
    with pytest.raises(ValueError):
        WorkerConfig(
            broker_url="redis://localhost",
            backend_url="redis://localhost",
            queues=["default"],
            concurrency=0,
        )
