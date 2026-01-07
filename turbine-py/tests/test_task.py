"""Tests for task decorator and Task class."""

import pytest

from turbine import Turbine
from turbine.task import Task, task
from turbine.workflow import Signature


class TestTaskDecorator:
    """Test the @task decorator."""

    def test_task_decorator_without_args(self):
        """Test @task without arguments."""

        @task
        def simple_task(x):
            return x * 2

        assert isinstance(simple_task, Task)
        assert simple_task.name.endswith("simple_task")
        assert simple_task.queue == "default"
        assert simple_task.max_retries == 3

    def test_task_decorator_with_args(self):
        """Test @task with arguments."""

        @task(queue="high-priority", max_retries=5, timeout=600)
        def custom_task(x):
            return x * 2

        assert isinstance(custom_task, Task)
        assert custom_task.queue == "high-priority"
        assert custom_task.max_retries == 5
        assert custom_task.timeout == 600

    def test_task_direct_call(self):
        """Test calling task directly (synchronous execution)."""

        @task
        def add(x, y):
            return x + y

        result = add(1, 2)
        assert result == 3

    def test_task_signature(self):
        """Test creating signatures with .s()"""

        @task
        def add(x, y):
            return x + y

        sig = add.s(1, 2)

        assert isinstance(sig, Signature)
        assert sig.task == add
        assert sig.args == (1, 2)
        assert sig.kwargs == {}

    def test_task_immutable_signature(self):
        """Test creating immutable signatures with .si()"""

        @task
        def process(data):
            return data

        sig = process.si("fixed_data")

        assert isinstance(sig, Signature)
        assert sig.immutable is True


class TestAppTask:
    """Test tasks registered with Turbine app."""

    def test_app_task_decorator(self):
        """Test @app.task decorator."""
        app = Turbine(server="localhost:50051", set_as_current=False)

        @app.task(queue="emails")
        def send_email(to, subject):
            pass

        assert isinstance(send_email, Task)
        assert send_email.queue == "emails"
        assert send_email.name in app.tasks

    def test_app_default_options(self):
        """Test app default options are inherited."""
        app = Turbine(
            server="localhost:50051",
            default_queue="custom",
            default_timeout=600,
            default_max_retries=10,
            set_as_current=False,
        )

        @app.task
        def my_task():
            pass

        assert my_task.queue == "custom"
        assert my_task.timeout == 600
        assert my_task.max_retries == 10

    def test_app_task_override_options(self):
        """Test task options override app defaults."""
        app = Turbine(
            server="localhost:50051",
            default_queue="default",
            default_timeout=300,
            set_as_current=False,
        )

        @app.task(queue="special", timeout=900)
        def special_task():
            pass

        assert special_task.queue == "special"
        assert special_task.timeout == 900

    def test_get_task_by_name(self):
        """Test retrieving task by name."""
        app = Turbine(server="localhost:50051", set_as_current=False)

        @app.task(name="myapp.tasks.process")
        def process():
            pass

        retrieved = app.get_task("myapp.tasks.process")
        assert retrieved is process


class TestTaskOptions:
    """Test TaskOptions class."""

    def test_task_options_defaults(self):
        """Test TaskOptions default values."""
        from turbine.client import TaskOptions

        opts = TaskOptions()

        assert opts.queue == "default"
        assert opts.priority == 0
        assert opts.max_retries == 3
        assert opts.timeout == 300
        assert opts.store_result is True

    def test_task_options_custom(self):
        """Test TaskOptions with custom values."""
        from turbine.client import TaskOptions

        opts = TaskOptions(
            queue="high",
            priority=10,
            max_retries=5,
            countdown=60,
            idempotency_key="unique-123",
        )

        assert opts.queue == "high"
        assert opts.priority == 10
        assert opts.countdown == 60
        assert opts.idempotency_key == "unique-123"

    def test_task_options_to_dict(self):
        """Test TaskOptions serialization."""
        from turbine.client import TaskOptions

        opts = TaskOptions(queue="test", priority=5)
        d = opts.to_dict()

        assert d["queue"] == "test"
        assert d["priority"] == 5
        assert "countdown" not in d  # None values excluded
