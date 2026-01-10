"""Unit tests for multi-tenancy functionality."""

import pytest
from unittest.mock import Mock, MagicMock, patch
from turbine.tenancy import TenantManager, Tenant, TenantQuotas


@pytest.fixture
def mock_redis():
    """Mock Redis connection."""
    with patch('turbine.tenancy.redis.from_url') as mock:
        conn = MagicMock()
        mock.return_value = conn
        yield conn


@pytest.fixture
def tenant_manager(mock_redis):
    """Create tenant manager with mocked Redis."""
    return TenantManager(backend_url="redis://localhost:6379")


def test_tenant_quotas_defaults():
    """Test that TenantQuotas has sensible defaults."""
    quotas = TenantQuotas()

    assert quotas.max_tasks_per_hour is None
    assert quotas.max_tasks_per_day is None
    assert quotas.max_concurrent_tasks is None
    assert quotas.allowed_queues is None


def test_tenant_quotas_to_dict():
    """Test TenantQuotas serialization."""
    quotas = TenantQuotas(
        max_tasks_per_hour=1000,
        max_concurrent_tasks=50,
        allowed_queues=["default", "emails"],
    )

    data = quotas.to_dict()

    assert data["max_tasks_per_hour"] == 1000
    assert data["max_concurrent_tasks"] == 50
    assert data["allowed_queues"] == ["default", "emails"]


def test_tenant_creation():
    """Test Tenant object creation."""
    quotas = TenantQuotas(max_tasks_per_hour=1000)
    tenant = Tenant(
        tenant_id="test-tenant",
        name="Test Tenant",
        enabled=True,
        quotas=quotas,
        metadata={"plan": "pro"}
    )

    assert tenant.tenant_id == "test-tenant"
    assert tenant.name == "Test Tenant"
    assert tenant.enabled is True
    assert tenant.quotas.max_tasks_per_hour == 1000
    assert tenant.metadata["plan"] == "pro"


def test_tenant_to_dict():
    """Test Tenant serialization."""
    tenant = Tenant(
        tenant_id="test-tenant",
        name="Test Tenant",
        quotas=TenantQuotas(max_tasks_per_hour=500),
    )

    data = tenant.to_dict()

    assert data["tenant_id"] == "test-tenant"
    assert data["name"] == "Test Tenant"
    assert data["enabled"] is True
    assert data["quotas"]["max_tasks_per_hour"] == 500


def test_create_tenant(tenant_manager, mock_redis):
    """Test creating a new tenant."""
    mock_redis.exists.return_value = False

    tenant = tenant_manager.create_tenant(
        tenant_id="acme-corp",
        name="ACME Corporation",
    )

    assert tenant.tenant_id == "acme-corp"
    assert tenant.name == "ACME Corporation"
    assert tenant.enabled is True

    # Verify Redis calls
    mock_redis.exists.assert_called_once()
    mock_redis.set.assert_called_once()


def test_create_duplicate_tenant(tenant_manager, mock_redis):
    """Test that creating duplicate tenant raises error."""
    mock_redis.exists.return_value = True

    with pytest.raises(ValueError, match="already exists"):
        tenant_manager.create_tenant(
            tenant_id="existing-tenant",
            name="Existing Tenant",
        )


def test_get_tenant(tenant_manager, mock_redis):
    """Test getting a tenant."""
    import json

    tenant_data = {
        "tenant_id": "test-tenant",
        "name": "Test Tenant",
        "enabled": True,
        "quotas": {},
        "metadata": {}
    }

    mock_redis.get.return_value = json.dumps(tenant_data)

    tenant = tenant_manager.get_tenant("test-tenant")

    assert tenant is not None
    assert tenant.tenant_id == "test-tenant"
    assert tenant.name == "Test Tenant"


def test_get_nonexistent_tenant(tenant_manager, mock_redis):
    """Test getting a non-existent tenant returns None."""
    mock_redis.get.return_value = None

    tenant = tenant_manager.get_tenant("nonexistent")

    assert tenant is None


def test_update_tenant(tenant_manager, mock_redis):
    """Test updating a tenant."""
    import json

    existing_tenant = {
        "tenant_id": "test-tenant",
        "name": "Old Name",
        "enabled": True,
        "quotas": {},
        "metadata": {}
    }

    mock_redis.get.return_value = json.dumps(existing_tenant)

    tenant = tenant_manager.update_tenant(
        tenant_id="test-tenant",
        name="New Name",
        enabled=False,
    )

    assert tenant.name == "New Name"
    assert tenant.enabled is False

    # Verify save was called
    mock_redis.set.assert_called_once()


def test_delete_tenant(tenant_manager, mock_redis):
    """Test deleting a tenant."""
    mock_redis.delete.return_value = 1
    mock_redis.keys.return_value = []

    result = tenant_manager.delete_tenant("test-tenant")

    assert result is True
    mock_redis.delete.assert_called()


def test_check_quota_hourly_limit(tenant_manager, mock_redis):
    """Test quota check for hourly limit."""
    import json

    tenant_data = {
        "tenant_id": "test-tenant",
        "name": "Test",
        "enabled": True,
        "quotas": {"max_tasks_per_hour": 100},
        "metadata": {}
    }

    mock_redis.get.side_effect = [
        json.dumps(tenant_data),  # get_tenant call
        "95",  # get_usage call
    ]

    # Under limit
    allowed, error = tenant_manager.check_quota("test-tenant", "tasks_per_hour")
    assert allowed is True
    assert error is None

    # Over limit
    mock_redis.get.side_effect = [
        json.dumps(tenant_data),
        "150",  # Over limit
    ]

    allowed, error = tenant_manager.check_quota("test-tenant", "tasks_per_hour")
    assert allowed is False
    assert "quota exceeded" in error.lower()


def test_check_quota_disabled_tenant(tenant_manager, mock_redis):
    """Test quota check for disabled tenant."""
    import json

    tenant_data = {
        "tenant_id": "test-tenant",
        "name": "Test",
        "enabled": False,
        "quotas": {},
        "metadata": {}
    }

    mock_redis.get.return_value = json.dumps(tenant_data)

    allowed, error = tenant_manager.check_quota("test-tenant", "tasks_per_hour")

    assert allowed is False
    assert "disabled" in error.lower()


def test_increment_usage(tenant_manager, mock_redis):
    """Test incrementing usage counter."""
    mock_redis.incr.return_value = 5

    count = tenant_manager.increment_usage("test-tenant", "tasks_hour", ttl=3600)

    assert count == 5
    mock_redis.incr.assert_called_once()
    mock_redis.expire.assert_called_once_with(
        "turbine:tenant:test-tenant:usage:tasks_hour",
        3600
    )


def test_get_usage(tenant_manager, mock_redis):
    """Test getting usage count."""
    mock_redis.get.return_value = "42"

    count = tenant_manager.get_usage("test-tenant", "tasks_hour")

    assert count == 42


def test_get_usage_no_data(tenant_manager, mock_redis):
    """Test getting usage when no data exists."""
    mock_redis.get.return_value = None

    count = tenant_manager.get_usage("test-tenant", "tasks_hour")

    assert count == 0


def test_get_tenant_stats(tenant_manager, mock_redis):
    """Test getting aggregated tenant stats."""
    mock_redis.get.side_effect = ["10", "150", "5", "1500"]

    stats = tenant_manager.get_tenant_stats("test-tenant")

    assert stats["tasks_hour"] == 10
    assert stats["tasks_day"] == 150
    assert stats["concurrent_tasks"] == 5
    assert stats["total_tasks"] == 1500
