"""Pytest configuration and fixtures."""

import pytest


@pytest.fixture
def turbine_app():
    """Create a Turbine app for testing."""
    from turbine import Turbine

    app = Turbine(
        server="localhost:50051",
        set_as_current=True,
    )
    yield app
    app.close()


@pytest.fixture
def mock_client(mocker):
    """Create a mock TurbineClient."""
    from turbine.client import TurbineClient

    client = mocker.MagicMock(spec=TurbineClient)
    client.submit_task.return_value = "test-task-id-123"
    client.get_result.return_value = (True, {"result": "test"})
    client.get_status.return_value = {
        "task_id": "test-task-id-123",
        "state": "SUCCESS",
        "retries": 0,
        "created_at": 1704067200000,
    }
    return client
