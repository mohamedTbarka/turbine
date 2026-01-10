# Multi-Tenancy Guide

Turbine supports multi-tenancy to isolate tasks, queues, and resources between different tenants (customers, teams, environments).

## Overview

Multi-tenancy in Turbine provides:

- **Tenant Isolation**: Separate namespaces for tasks and results
- **Resource Quotas**: Limit tasks, queue length, and resource usage per tenant
- **Usage Tracking**: Monitor per-tenant metrics
- **Flexible Assignment**: Tasks can be assigned to tenants at definition or submission time

## Quick Start

### Creating Tenants

```bash
# Create a new tenant
turbine tenant create acme-corp "ACME Corporation"

# List all tenants
turbine tenant list

# Get tenant details
turbine tenant get acme-corp

# Get tenant statistics
turbine tenant stats acme-corp
```

### Defining Tenant-Specific Tasks

```python
from turbine import task

# Task always runs in specific tenant
@task(queue="emails", tenant_id="acme-corp")
def send_tenant_email(to: str, subject: str):
    # This task is isolated to acme-corp tenant
    pass

# Task can accept tenant at runtime
@task(queue="processing")
def process_data(data: dict):
    # Tenant specified when submitting
    pass

# Submit with tenant override
process_data.apply_async(
    args=[{"key": "value"}],
    tenant_id="acme-corp"
)
```

## Tenant Configuration

### Creating Tenants with Quotas

```python
from turbine.tenancy import TenantManager, TenantQuotas

manager = TenantManager(backend_url="redis://localhost:6379")

# Create tenant with quotas
quotas = TenantQuotas(
    max_tasks_per_hour=1000,
    max_tasks_per_day=10000,
    max_concurrent_tasks=50,
    max_queue_length=500,
    max_task_size_bytes=1048576,  # 1MB
    max_result_size_bytes=5242880,  # 5MB
    allowed_queues=["default", "emails", "processing"],
    max_retry_count=5,
)

tenant = manager.create_tenant(
    tenant_id="acme-corp",
    name="ACME Corporation",
    quotas=quotas,
    metadata={
        "plan": "enterprise",
        "contact": "admin@acme-corp.com"
    }
)
```

### Quota Types

| Quota | Description | Behavior on Exceed |
|-------|-------------|-------------------|
| `max_tasks_per_hour` | Maximum tasks per hour | Reject new tasks |
| `max_tasks_per_day` | Maximum tasks per day | Reject new tasks |
| `max_concurrent_tasks` | Maximum running tasks | Queue tasks |
| `max_queue_length` | Maximum pending tasks | Reject new tasks |
| `max_task_size_bytes` | Maximum task payload size | Reject task |
| `max_result_size_bytes` | Maximum result size | Reject result storage |
| `allowed_queues` | List of permitted queues | Reject task |
| `max_retry_count` | Maximum retry attempts | Send to DLQ earlier |

## Usage Tracking

Turbine automatically tracks usage metrics per tenant:

```python
from turbine.tenancy import TenantManager

manager = TenantManager()

# Get current usage
stats = manager.get_tenant_stats("acme-corp")
print(stats)
# {
#   "tasks_hour": 523,
#   "tasks_day": 8734,
#   "concurrent_tasks": 12,
#   "total_tasks": 125403
# }

# Check if tenant can submit task
allowed, error = manager.check_quota("acme-corp", "tasks_per_hour")
if not allowed:
    print(f"Quota exceeded: {error}")
```

## Redis Key Isolation

Tenant data is isolated using Redis key prefixes:

```
turbine:tenant:{tenant_id}                 - Tenant configuration
turbine:tenant:{tenant_id}:usage:tasks_hour - Hourly task counter
turbine:tenant:{tenant_id}:usage:tasks_day  - Daily task counter
turbine:tenant:{tenant_id}:usage:concurrent - Concurrent task counter
turbine:tenant:{tenant_id}:usage:total      - Total task counter
```

Task results include tenant information but remain in shared keys for efficiency. The tenant_id is stored in task metadata and headers.

## Tenant Management

### Updating Tenants

```bash
# Update tenant name
turbine tenant update acme-corp --name "ACME Inc."

# Disable tenant
turbine tenant update acme-corp --enabled false

# Enable tenant
turbine tenant update acme-corp --enabled true
```

```python
from turbine.tenancy import TenantManager, TenantQuotas

manager = TenantManager()

# Update quotas
new_quotas = TenantQuotas(
    max_tasks_per_hour=2000,  # Increased limit
    max_concurrent_tasks=100,
)

manager.update_tenant(
    tenant_id="acme-corp",
    quotas=new_quotas
)
```

### Deleting Tenants

```bash
# Delete tenant (with confirmation)
turbine tenant delete acme-corp

# Force delete without confirmation
turbine tenant delete acme-corp --force
```

```python
manager.delete_tenant("acme-corp")
```

**Note**: Deleting a tenant removes:
- Tenant configuration
- Usage counters
- Does NOT delete task results (for audit purposes)

## Django Integration

### Settings Configuration

```python
# settings.py

# Default tenant for all tasks
TURBINE_TENANT_ID = "my-tenant"

# Or use tenant-per-environment
import os
TURBINE_TENANT_ID = os.environ.get('TENANT_ID', 'default')
```

### Per-Request Tenant

```python
# views.py
from turbine import task

@task(queue="emails")
def send_email(to, subject, body):
    pass

def my_view(request):
    # Extract tenant from request (subdomain, header, JWT, etc.)
    tenant_id = request.META.get('HTTP_X_TENANT_ID')

    # Submit with specific tenant
    send_email.apply_async(
        args=["user@example.com", "Hello", "World"],
        tenant_id=tenant_id
    )
```

### Middleware for Automatic Tenant Detection

```python
# middleware.py
class TenantMiddleware:
    def __init__(self, get_response):
        self.get_response = get_response

    def __call__(self, request):
        # Extract tenant from subdomain, header, or JWT
        tenant_id = self.get_tenant_from_request(request)

        # Store in request for views
        request.tenant_id = tenant_id

        return self.get_response(request)

    def get_tenant_from_request(self, request):
        # Option 1: From subdomain
        host = request.get_host()
        if '.' in host:
            subdomain = host.split('.')[0]
            return subdomain

        # Option 2: From header
        return request.META.get('HTTP_X_TENANT_ID', 'default')
```

## FastAPI Integration

```python
from fastapi import FastAPI, Header
from turbine import task

app = FastAPI()

@task(queue="api-tasks")
def process_request(data: dict):
    pass

@app.post("/process")
async def process(
    data: dict,
    x_tenant_id: str = Header(None, alias="X-Tenant-ID")
):
    # Submit with tenant from header
    result = process_request.apply_async(
        args=[data],
        tenant_id=x_tenant_id or "default"
    )

    return {"task_id": result.task_id}
```

## Best Practices

### 1. Tenant Identification

Choose a tenant identification strategy:

- **Subdomain**: `acme.yourapp.com` → tenant_id = `acme`
- **Path**: `/api/acme/tasks` → tenant_id = `acme`
- **Header**: `X-Tenant-ID: acme`
- **JWT Claim**: Extract from authenticated user token

### 2. Quota Planning

Set quotas based on tenant tier:

```python
# Free tier
free_quotas = TenantQuotas(
    max_tasks_per_hour=100,
    max_concurrent_tasks=2,
    max_task_size_bytes=102400,  # 100KB
)

# Pro tier
pro_quotas = TenantQuotas(
    max_tasks_per_hour=10000,
    max_concurrent_tasks=50,
    max_task_size_bytes=1048576,  # 1MB
)

# Enterprise tier (no limits)
enterprise_quotas = TenantQuotas()
```

### 3. Monitoring

Track tenant usage and set up alerts:

```python
# Check if tenant is approaching limits
stats = manager.get_tenant_stats(tenant_id)
quotas = manager.get_tenant(tenant_id).quotas

if quotas.max_tasks_per_hour:
    usage_pct = (stats["tasks_hour"] / quotas.max_tasks_per_hour) * 100
    if usage_pct > 80:
        # Send alert
        print(f"Tenant {tenant_id} at {usage_pct}% hourly quota")
```

### 4. Graceful Degradation

Handle quota exceeded gracefully:

```python
from turbine.tenancy import TenantManager

manager = TenantManager()

# Check before submission
allowed, error = manager.check_quota(tenant_id, "tasks_per_hour")

if not allowed:
    # Return error to user
    return {"error": error, "code": "QUOTA_EXCEEDED"}

# Submit task
task_id = my_task.apply_async(
    args=[data],
    tenant_id=tenant_id
)
```

## Limitations

### Current Limitations

1. **No Automatic Enforcement**: Quotas are checked by the application, not enforced by Turbine server
2. **Shared Queues**: All tenants currently share the same Redis queues
3. **No Billing Integration**: Usage tracking is separate from billing

### Future Enhancements

- Server-side quota enforcement
- Separate Redis databases per tenant
- Billing/metering integration
- Tenant-specific rate limiting
- Resource pool isolation

## CLI Reference

```bash
# Tenant management
turbine tenant create <tenant-id> <name>
turbine tenant list
turbine tenant get <tenant-id>
turbine tenant update <tenant-id> [--name NAME] [--enabled true|false]
turbine tenant delete <tenant-id> [--force]
turbine tenant stats <tenant-id>

# All commands support --backend-url flag
turbine tenant list --backend-url redis://custom:6379
```

## API Reference

### TenantManager

```python
from turbine.tenancy import TenantManager, TenantQuotas

manager = TenantManager(backend_url="redis://localhost:6379")

# Create
tenant = manager.create_tenant(tenant_id, name, quotas, metadata)

# Read
tenant = manager.get_tenant(tenant_id)
tenants = manager.list_tenants()

# Update
tenant = manager.update_tenant(tenant_id, name, enabled, quotas, metadata)

# Delete
deleted = manager.delete_tenant(tenant_id)

# Quota checking
allowed, error = manager.check_quota(tenant_id, quota_type, current_value)

# Usage tracking
count = manager.increment_usage(tenant_id, metric, amount, ttl)
count = manager.decrement_usage(tenant_id, metric, amount)
count = manager.get_usage(tenant_id, metric)
stats = manager.get_tenant_stats(tenant_id)
```

## Examples

### SaaS Application with Tenant Tiers

```python
from turbine import Turbine, task
from turbine.tenancy import TenantManager, TenantQuotas

# Initialize
turbine = Turbine(server="localhost:50051")
tenant_mgr = TenantManager()

# Setup tenant tiers
TIER_QUOTAS = {
    "free": TenantQuotas(
        max_tasks_per_hour=100,
        max_concurrent_tasks=2,
    ),
    "pro": TenantQuotas(
        max_tasks_per_hour=10000,
        max_concurrent_tasks=50,
    ),
    "enterprise": TenantQuotas(
        # No limits
    ),
}

# Create tenant with tier
def create_customer(customer_id: str, name: str, tier: str):
    tenant = tenant_mgr.create_tenant(
        tenant_id=customer_id,
        name=name,
        quotas=TIER_QUOTAS[tier],
        metadata={"tier": tier}
    )
    return tenant

# Task with quota checking
@task(queue="api-tasks")
def api_task(data: dict):
    pass

def submit_task(customer_id: str, data: dict):
    # Check quota
    allowed, error = tenant_mgr.check_quota(customer_id, "tasks_per_hour")

    if not allowed:
        return {"error": error, "status": "quota_exceeded"}

    # Increment usage
    tenant_mgr.increment_usage(customer_id, "tasks_hour", ttl=3600)
    tenant_mgr.increment_usage(customer_id, "tasks_day", ttl=86400)

    # Submit task
    result = api_task.apply_async(args=[data], tenant_id=customer_id)

    return {"task_id": result.task_id, "status": "submitted"}
```

### Multi-Environment Setup

```python
# Different tenants for different environments
TENANTS = {
    "dev": "my-app-dev",
    "staging": "my-app-staging",
    "prod": "my-app-prod",
}

import os
env = os.environ.get('ENV', 'dev')
tenant_id = TENANTS[env]

@task(queue="jobs", tenant_id=tenant_id)
def background_job():
    pass
```

## Security Considerations

1. **Validate Tenant IDs**: Always validate tenant IDs from user input
2. **Check Tenant Enabled**: Ensure tenant is enabled before submitting tasks
3. **Audit Logging**: Log tenant operations for audit trails
4. **Rate Limiting**: Combine quotas with rate limiting for DDoS protection
5. **Data Isolation**: Task results include tenant_id but are not fully isolated

## Migration

### Migrating to Multi-Tenancy

If you're adding multi-tenancy to an existing deployment:

1. **Create Default Tenant**
   ```bash
   turbine tenant create default "Default Tenant"
   ```

2. **Update Existing Tasks**
   ```python
   # Before
   @task(queue="emails")
   def send_email():
       pass

   # After (backward compatible)
   @task(queue="emails", tenant_id="default")
   def send_email():
       pass
   ```

3. **Gradually Migrate**: New customers get their own tenant_id

## Troubleshooting

### Quota Exceeded Errors

```
Error: Hourly task quota exceeded (1000)
```

**Solution**: Increase quota or wait for counter reset

```python
# Increase quota
manager.update_tenant(
    tenant_id="acme-corp",
    quotas=TenantQuotas(max_tasks_per_hour=2000)
)
```

### Tenant Not Found

```
Error: Tenant 'acme-corp' not found
```

**Solution**: Create the tenant first

```bash
turbine tenant create acme-corp "ACME Corp"
```

### Tasks Not Isolated

**Issue**: Tasks from different tenants can see each other

**Current State**: This is expected. Task results are stored with tenant_id in metadata but share the same Redis keys for performance.

**Future**: Full isolation will be available with separate Redis databases or key prefixes.

## FAQ

**Q: Are tasks from different tenants processed by different workers?**

A: No, workers process tasks from all tenants. Isolation is logical, not physical.

**Q: Can quotas be enforced at the server level?**

A: Not yet. Quotas are currently checked by the Python SDK. Server-side enforcement is planned.

**Q: How are tenant IDs authenticated?**

A: Turbine doesn't handle authentication. Your application should validate tenant IDs before submitting tasks.

**Q: Can I use different Redis instances per tenant?**

A: Not currently. All tenants share the same Redis broker and backend.

**Q: What happens to tasks when a tenant is deleted?**

A: Task results remain for audit purposes. Only tenant configuration and usage counters are deleted.

## See Also

- [Configuration Guide](./configuration.md)
- [Task Options](./tasks.md)
- [Dashboard API](./DASHBOARD_API.md)
