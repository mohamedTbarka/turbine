"""Multi-tenancy support for Turbine."""

import logging
from dataclasses import dataclass, field
from typing import Any
import redis
import json

logger = logging.getLogger(__name__)


@dataclass
class TenantQuotas:
    """Resource quotas for a tenant."""

    max_tasks_per_hour: int | None = None
    max_tasks_per_day: int | None = None
    max_concurrent_tasks: int | None = None
    max_queue_length: int | None = None
    max_task_size_bytes: int | None = None
    max_result_size_bytes: int | None = None
    allowed_queues: list[str] | None = None
    max_retry_count: int | None = None

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "max_tasks_per_hour": self.max_tasks_per_hour,
            "max_tasks_per_day": self.max_tasks_per_day,
            "max_concurrent_tasks": self.max_concurrent_tasks,
            "max_queue_length": self.max_queue_length,
            "max_task_size_bytes": self.max_task_size_bytes,
            "max_result_size_bytes": self.max_result_size_bytes,
            "allowed_queues": self.allowed_queues,
            "max_retry_count": self.max_retry_count,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "TenantQuotas":
        """Create from dictionary."""
        return cls(**{k: v for k, v in data.items() if v is not None})


@dataclass
class Tenant:
    """Represents a tenant in multi-tenant environment."""

    tenant_id: str
    name: str
    enabled: bool = True
    quotas: TenantQuotas = field(default_factory=TenantQuotas)
    metadata: dict[str, Any] = field(default_factory=dict)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary."""
        return {
            "tenant_id": self.tenant_id,
            "name": self.name,
            "enabled": self.enabled,
            "quotas": self.quotas.to_dict(),
            "metadata": self.metadata,
        }

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "Tenant":
        """Create from dictionary."""
        quotas_data = data.get("quotas", {})
        return cls(
            tenant_id=data["tenant_id"],
            name=data["name"],
            enabled=data.get("enabled", True),
            quotas=TenantQuotas.from_dict(quotas_data),
            metadata=data.get("metadata", {}),
        )


class TenantManager:
    """Manager for tenant operations and quota enforcement."""

    def __init__(self, backend_url: str = "redis://localhost:6379"):
        """
        Initialize tenant manager.

        Args:
            backend_url: Redis URL for tenant data storage
        """
        self.backend_url = backend_url
        self.conn = redis.from_url(backend_url, decode_responses=True)
        self._prefix = "turbine:tenant"

    def _tenant_key(self, tenant_id: str) -> str:
        """Get Redis key for tenant data."""
        return f"{self._prefix}:{tenant_id}"

    def _usage_key(self, tenant_id: str, metric: str) -> str:
        """Get Redis key for tenant usage metrics."""
        return f"{self._prefix}:{tenant_id}:usage:{metric}"

    def create_tenant(
        self,
        tenant_id: str,
        name: str,
        quotas: TenantQuotas | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> Tenant:
        """
        Create a new tenant.

        Args:
            tenant_id: Unique tenant identifier
            name: Tenant display name
            quotas: Resource quotas for the tenant
            metadata: Additional tenant metadata

        Returns:
            Created tenant

        Raises:
            ValueError: If tenant already exists
        """
        key = self._tenant_key(tenant_id)

        if self.conn.exists(key):
            raise ValueError(f"Tenant '{tenant_id}' already exists")

        tenant = Tenant(
            tenant_id=tenant_id,
            name=name,
            quotas=quotas or TenantQuotas(),
            metadata=metadata or {},
        )

        self.conn.set(key, json.dumps(tenant.to_dict()))
        logger.info(f"Created tenant: {tenant_id}")

        return tenant

    def get_tenant(self, tenant_id: str) -> Tenant | None:
        """
        Get tenant by ID.

        Args:
            tenant_id: Tenant identifier

        Returns:
            Tenant or None if not found
        """
        key = self._tenant_key(tenant_id)
        data = self.conn.get(key)

        if not data:
            return None

        return Tenant.from_dict(json.loads(data))

    def update_tenant(
        self,
        tenant_id: str,
        name: str | None = None,
        enabled: bool | None = None,
        quotas: TenantQuotas | None = None,
        metadata: dict[str, Any] | None = None,
    ) -> Tenant:
        """
        Update tenant configuration.

        Args:
            tenant_id: Tenant identifier
            name: New tenant name
            enabled: Enable/disable tenant
            quotas: New quotas
            metadata: New metadata

        Returns:
            Updated tenant

        Raises:
            ValueError: If tenant not found
        """
        tenant = self.get_tenant(tenant_id)

        if not tenant:
            raise ValueError(f"Tenant '{tenant_id}' not found")

        if name is not None:
            tenant.name = name
        if enabled is not None:
            tenant.enabled = enabled
        if quotas is not None:
            tenant.quotas = quotas
        if metadata is not None:
            tenant.metadata = metadata

        key = self._tenant_key(tenant_id)
        self.conn.set(key, json.dumps(tenant.to_dict()))
        logger.info(f"Updated tenant: {tenant_id}")

        return tenant

    def delete_tenant(self, tenant_id: str) -> bool:
        """
        Delete a tenant.

        Args:
            tenant_id: Tenant identifier

        Returns:
            True if deleted, False if not found
        """
        key = self._tenant_key(tenant_id)
        deleted = self.conn.delete(key) > 0

        if deleted:
            # Clean up usage metrics
            usage_keys = self.conn.keys(f"{self._prefix}:{tenant_id}:usage:*")
            if usage_keys:
                self.conn.delete(*usage_keys)
            logger.info(f"Deleted tenant: {tenant_id}")

        return deleted

    def list_tenants(self) -> list[Tenant]:
        """
        List all tenants.

        Returns:
            List of tenants
        """
        keys = self.conn.keys(f"{self._prefix}:*")
        tenants = []

        for key in keys:
            if ":usage:" not in key:  # Skip usage metric keys
                data = self.conn.get(key)
                if data:
                    try:
                        tenants.append(Tenant.from_dict(json.loads(data)))
                    except Exception as e:
                        logger.error(f"Failed to parse tenant data from {key}: {e}")

        return tenants

    def check_quota(
        self,
        tenant_id: str,
        quota_type: str,
        current_value: int | None = None,
    ) -> tuple[bool, str | None]:
        """
        Check if a tenant can perform an operation based on quotas.

        Args:
            tenant_id: Tenant identifier
            quota_type: Type of quota to check
            current_value: Current value to check against quota

        Returns:
            Tuple of (allowed, error_message)
        """
        tenant = self.get_tenant(tenant_id)

        if not tenant:
            return False, f"Tenant '{tenant_id}' not found"

        if not tenant.enabled:
            return False, f"Tenant '{tenant_id}' is disabled"

        quotas = tenant.quotas

        # Check specific quota types
        if quota_type == "tasks_per_hour" and quotas.max_tasks_per_hour:
            count = self.get_usage(tenant_id, "tasks_hour")
            if count >= quotas.max_tasks_per_hour:
                return False, f"Hourly task quota exceeded ({quotas.max_tasks_per_hour})"

        elif quota_type == "tasks_per_day" and quotas.max_tasks_per_day:
            count = self.get_usage(tenant_id, "tasks_day")
            if count >= quotas.max_tasks_per_day:
                return False, f"Daily task quota exceeded ({quotas.max_tasks_per_day})"

        elif quota_type == "concurrent_tasks" and quotas.max_concurrent_tasks:
            count = self.get_usage(tenant_id, "concurrent")
            if count >= quotas.max_concurrent_tasks:
                return False, f"Concurrent task limit reached ({quotas.max_concurrent_tasks})"

        elif quota_type == "queue_length" and quotas.max_queue_length and current_value:
            if current_value >= quotas.max_queue_length:
                return False, f"Queue length limit reached ({quotas.max_queue_length})"

        elif quota_type == "task_size" and quotas.max_task_size_bytes and current_value:
            if current_value > quotas.max_task_size_bytes:
                return False, f"Task size exceeds limit ({quotas.max_task_size_bytes} bytes)"

        elif quota_type == "result_size" and quotas.max_result_size_bytes and current_value:
            if current_value > quotas.max_result_size_bytes:
                return False, f"Result size exceeds limit ({quotas.max_result_size_bytes} bytes)"

        return True, None

    def increment_usage(
        self,
        tenant_id: str,
        metric: str,
        amount: int = 1,
        ttl: int | None = None,
    ) -> int:
        """
        Increment usage counter for a tenant.

        Args:
            tenant_id: Tenant identifier
            metric: Metric name (e.g., 'tasks_hour', 'tasks_day')
            amount: Amount to increment
            ttl: Time-to-live in seconds for the counter

        Returns:
            New counter value
        """
        key = self._usage_key(tenant_id, metric)
        value = self.conn.incr(key, amount)

        if ttl and value == amount:  # First increment, set TTL
            self.conn.expire(key, ttl)

        return value

    def decrement_usage(
        self,
        tenant_id: str,
        metric: str,
        amount: int = 1,
    ) -> int:
        """
        Decrement usage counter for a tenant.

        Args:
            tenant_id: Tenant identifier
            metric: Metric name
            amount: Amount to decrement

        Returns:
            New counter value
        """
        key = self._usage_key(tenant_id, metric)
        value = self.conn.decr(key, amount)
        return max(0, value)  # Don't go below 0

    def get_usage(self, tenant_id: str, metric: str) -> int:
        """
        Get current usage for a metric.

        Args:
            tenant_id: Tenant identifier
            metric: Metric name

        Returns:
            Current usage count
        """
        key = self._usage_key(tenant_id, metric)
        value = self.conn.get(key)
        return int(value) if value else 0

    def get_tenant_stats(self, tenant_id: str) -> dict[str, Any]:
        """
        Get statistics for a tenant.

        Args:
            tenant_id: Tenant identifier

        Returns:
            Dictionary of statistics
        """
        return {
            "tasks_hour": self.get_usage(tenant_id, "tasks_hour"),
            "tasks_day": self.get_usage(tenant_id, "tasks_day"),
            "concurrent_tasks": self.get_usage(tenant_id, "concurrent"),
            "total_tasks": self.get_usage(tenant_id, "total"),
        }
