# Turbine Best Practices

Production-tested patterns and recommendations for building reliable task queues with Turbine.

## Task Design

### Keep Tasks Small and Focused

**❌ Bad - Monolithic task:**

```python
@task
def process_user_signup(user_data):
    # Too many responsibilities
    user = create_user(user_data)
    send_welcome_email(user)
    create_profile(user)
    send_to_crm(user)
    generate_analytics_event(user)
    notify_admin(user)
    # Hard to retry individual steps
```

**✅ Good - Separate tasks:**

```python
@task
def create_user(user_data):
    return User.objects.create(**user_data)

@task
def send_welcome_email(user_id):
    user = User.objects.get(id=user_id)
    send_email(user.email, "Welcome!")

@task
def create_profile(user_id):
    Profile.objects.create(user_id=user_id)

# Use chain for sequential execution
from turbine import chain

workflow = chain(
    create_user.s(user_data),
    send_welcome_email.s(),
    create_profile.s(),
)
workflow.delay()
```

### Idempotent Tasks

Tasks should be safe to run multiple times:

**❌ Not idempotent:**

```python
@task
def increment_counter(user_id):
    user = User.objects.get(id=user_id)
    user.login_count += 1  # ❌ Duplicate execution = wrong count
    user.save()
```

**✅ Idempotent:**

```python
@task
def record_login(user_id, login_timestamp):
    # Use unique constraint or upsert
    Login.objects.get_or_create(
        user_id=user_id,
        timestamp=login_timestamp,
    )
    # Running twice has no effect
```

### Explicit is Better Than Implicit

**❌ Implicit dependencies:**

```python
@task
def process_order(order_id):
    order = Order.objects.get(id=order_id)
    # ❌ Relies on global state
    user = get_current_user()
    tenant = get_current_tenant()
```

**✅ Explicit parameters:**

```python
@task
def process_order(order_id, user_id, tenant_id):
    # ✅ All dependencies explicit
    order = Order.objects.get(id=order_id)
    user = User.objects.get(id=user_id)
    # Process with context
```

## Error Handling

### Let Tasks Fail Fast

```python
@task(max_retries=3, retry_delay=60)
def send_email(to, subject, body):
    # Validate early
    if not validate_email(to):
        raise ValueError("Invalid email")  # Don't retry

    # Retry on transient errors
    try:
        smtp.send(to, subject, body)
    except SMTPServerError:
        raise  # Will retry with exponential backoff
```

### Distinguish Transient vs Permanent Errors

```python
class PermanentError(Exception):
    """Non-retriable error."""
    pass

class TransientError(Exception):
    """Retriable error."""
    pass

@task(max_retries=3)
def api_call(endpoint):
    try:
        response = requests.get(endpoint)
        response.raise_for_status()
        return response.json()

    except requests.HTTPError as e:
        if e.response.status_code in [400, 401, 404]:
            # Permanent error - don't retry
            raise PermanentError(f"Client error: {e}")
        else:
            # Transient error - retry
            raise TransientError(f"Server error: {e}")
```

### Use DLQ for Failed Tasks

```python
# Worker automatically sends failed tasks to DLQ
# Inspect and handle failed tasks

# List failed tasks
from turbine.dlq import DLQManager

dlq = DLQManager()
failed_tasks = dlq.list_failed_tasks()

for task in failed_tasks:
    print(f"Failed: {task['task_name']} - {task['error']}")

    # Analyze and fix root cause
    # Then replay if needed
    # dlq.replay_task(task['task_id'], turbine_app)
```

## Performance Optimization

### Choose Right Queue Strategy

```python
# Separate queues by characteristics
@task(queue="fast")  # <100ms tasks
def quick_task():
    pass

@task(queue="slow")  # >1s tasks
def long_running_task():
    pass

@task(queue="io-bound")  # I/O heavy
def download_file():
    pass

@task(queue="cpu-bound")  # CPU heavy
def process_image():
    pass
```

### Batch Similar Operations

**❌ Individual tasks:**

```python
# Sending 1000 emails = 1000 tasks = high overhead
for user in users:
    send_email.delay(user.email, "Newsletter", body)
```

**✅ Batched:**

```python
from turbine.batch import BatchProcessor

# Process in batches
processor = BatchProcessor(send_email, chunk_size=100)
results = processor.map([(u.email, "Newsletter", body) for u in users])

# Or use batch task
@task
def send_batch_emails(email_list, subject, body):
    for email in email_list:
        send_single_email(email, subject, body)

# Submit fewer, larger tasks
for chunk in chunks(users, 100):
    emails = [u.email for u in chunk]
    send_batch_emails.delay(emails, "Newsletter", body)
```

### Use Compression for Large Results

```python
# Automatic compression for results > 1KB
# Configured in worker

# Or explicit
from turbine import task

@task(queue="reports")
def generate_large_report(report_id):
    report_data = build_report(report_id)  # 10MB of data

    # Worker auto-compresses if > 1KB
    # Saves Redis memory
    return report_data
```

### Disable Result Storage When Not Needed

```python
@task(store_result=False)
def send_notification(user_id, message):
    # Fire and forget - no need to store result
    notify_user(user_id, message)

# Or for entire workflow
@task(store_result=False)
def cleanup_old_data():
    # Background maintenance - don't care about result
    delete_old_records()
```

## Reliability

### Set Appropriate Timeouts

```python
# Match timeout to expected duration + buffer
@task(timeout=30)  # 30 seconds
def api_call():
    return requests.get(url, timeout=25)  # Leave 5s buffer

@task(timeout=600)  # 10 minutes
def generate_report():
    # Long-running task
    pass
```

### Use Soft Timeouts for Warnings

```python
@task(timeout=300, soft_timeout=250)
def processing_task():
    # At 250s, task receives warning (SoftTimeLimitExceeded)
    # Can save state and gracefully stop
    # At 300s, hard killed
    pass
```

### Handle Database Connections

```python
# ❌ Don't share connections
db_connection = create_connection()  # Global - will fail

@task
def query_data():
    return db_connection.query("SELECT ...")  # ❌

# ✅ Create connection per task
@task
def query_data():
    with create_connection() as conn:  # ✅ Fresh connection
        return conn.query("SELECT ...")
```

## Workflow Patterns

### Pattern: ETL Pipeline

```python
from turbine import chain

@task
def extract(source):
    return fetch_data(source)

@task
def transform(data):
    return process_data(data)

@task
def load(data):
    return store_data(data)

# Sequential execution
pipeline = chain(
    extract.s("source.csv"),
    transform.s(),
    load.s()
)
pipeline.delay()
```

### Pattern: Fan-Out/Fan-In

```python
from turbine import chord

@task
def split_work(data):
    return [chunk1, chunk2, chunk3]

@task
def process_chunk(chunk):
    return process(chunk)

@task
def aggregate(results):
    return combine(results)

# Parallel processing with aggregation
workflow = chord(
    [process_chunk.s(chunk) for chunk in chunks],
    aggregate.s()
)
workflow.delay()
```

### Pattern: Retry with Backoff

```python
@task(max_retries=5, retry_delay=60)
def flaky_api_call(url):
    # Automatic exponential backoff:
    # 1st retry: 60s
    # 2nd retry: 120s
    # 3rd retry: 240s
    # 4th retry: 480s
    # 5th retry: 960s

    return requests.get(url, timeout=30)
```

## Multi-Tenancy

### Tenant Context Pattern

```python
# Django middleware
class TenantMiddleware:
    def __call__(self, request):
        tenant_id = extract_tenant(request)
        request.tenant_id = tenant_id
        return self.get_response(request)

# View
def api_view(request):
    # Submit with tenant from request
    result = my_task.apply_async(
        args=[data],
        tenant_id=request.tenant_id
    )
```

### Quota Enforcement

```python
from turbine.tenancy import TenantManager

manager = TenantManager()

def submit_with_quota_check(tenant_id, task, *args, **kwargs):
    # Check quota before submission
    allowed, error = manager.check_quota(tenant_id, "tasks_per_hour")

    if not allowed:
        raise QuotaExceededError(error)

    # Increment usage
    manager.increment_usage(tenant_id, "tasks_hour", ttl=3600)

    # Submit task
    return task.apply_async(
        args=args,
        kwargs=kwargs,
        tenant_id=tenant_id
    )
```

## Monitoring and Observability

### Structured Logging

```python
import logging
import json

logger = logging.getLogger(__name__)

@task
def important_task(user_id, data):
    # Structured log for better searchability
    logger.info(json.dumps({
        "event": "task_started",
        "user_id": user_id,
        "data_size": len(json.dumps(data)),
    }))

    result = process(data)

    logger.info(json.dumps({
        "event": "task_completed",
        "user_id": user_id,
        "result_size": len(json.dumps(result)),
    }))

    return result
```

### Custom Metrics

```python
from prometheus_client import Counter, Histogram

task_duration = Histogram(
    'my_task_duration_seconds',
    'Task execution duration',
    ['task_name']
)

@task
def monitored_task():
    import time
    start = time.time()

    try:
        result = do_work()
        return result
    finally:
        duration = time.time() - start
        task_duration.labels(task_name='monitored_task').observe(duration)
```

### Health Checks

```python
# Periodic health check task
@task(queue="monitoring")
def health_check():
    checks = {
        "database": check_database(),
        "redis": check_redis(),
        "external_api": check_api(),
    }

    if not all(checks.values()):
        send_alert("Health check failed", checks)

    return checks

# Schedule via Beat
# Or run periodically from cron
```

## Testing

### Test Tasks Independently

```python
import pytest
from myapp.tasks import process_data

def test_process_data():
    # Test task logic without Turbine
    result = process_data.func(test_data)  # Call underlying function
    assert result == expected
```

### Integration Tests

```python
def test_task_execution(turbine_app):
    # Submit task
    result = process_data.delay(test_data)

    # Wait for result
    value = result.get(timeout=10)

    # Verify
    assert value == expected
```

### Mock External Dependencies

```python
from unittest.mock import patch

@task
def call_external_api(url):
    return requests.get(url).json()

def test_api_task():
    with patch('requests.get') as mock_get:
        mock_get.return_value.json.return_value = {"status": "ok"}

        result = call_external_api.func("http://api.example.com")

        assert result["status"] == "ok"
```

## Common Anti-Patterns

### ❌ Don't: Store Large Data in Tasks

```python
# ❌ Passing 10MB file as argument
@task
def process_file(file_contents):  # Bad - 10MB in Redis
    return process(file_contents)

# ✅ Pass reference instead
@task
def process_file(file_path):  # Good - small reference
    with open(file_path, 'rb') as f:
        contents = f.read()
    return process(contents)
```

### ❌ Don't: Chain Too Many Tasks

```python
# ❌ 100 chained tasks = slow, fragile
workflow = chain(*[process_item.s(i) for i in range(100)])

# ✅ Use batch processing
from turbine.batch import batch_map
results = batch_map(process_item, range(100), batch_size=20)
```

### ❌ Don't: Ignore Failures

```python
# ❌ Fire and forget without error handling
for item in items:
    process_item.delay(item)
# No way to know if any failed

# ✅ Track results
results = [process_item.delay(item) for item in items]

# Check for failures
for result in results:
    try:
        value = result.get(timeout=60)
    except Exception as e:
        logger.error(f"Task failed: {e}")
        handle_failure(result.task_id)
```

### ❌ Don't: Use Tasks for Synchronous Operations

```python
# ❌ Blocking on immediate result
def api_endpoint(request):
    result = heavy_task.delay(request.data)
    value = result.get(timeout=30)  # Defeats purpose of async
    return Response(value)

# ✅ Return task ID, poll separately
def api_endpoint(request):
    result = heavy_task.delay(request.data)
    return Response({"task_id": result.task_id})

def check_result(request, task_id):
    status = app.get_task_status(task_id)
    return Response(status)
```

## Deployment

### Blue-Green Deployment

```bash
# Deploy new version alongside old
docker run -d --name turbine-server-green turbine:v2
docker run -d --name turbine-worker-green turbine-worker:v2

# Gradually shift traffic
# Once stable, remove old version
docker stop turbine-server-blue
docker rm turbine-server-blue
```

### Zero-Downtime Worker Updates

```bash
# Start new workers
./start-new-workers.sh

# Gracefully stop old workers (finish current tasks)
kill -SIGTERM $(cat /var/run/turbine-worker.pid)

# Wait for graceful shutdown
wait

# Old workers stopped, new workers running
```

### Capacity Planning

**Rules of thumb:**

- **Concurrency**: Start with 2x CPU cores
- **Queue count**: 3-5 queues (by priority/type)
- **Worker count**: Start with 2-3 workers per queue
- **Redis**: 1GB RAM per 100k results/day
- **Monitor**: Adjust based on metrics

## Disaster Recovery

### Backup Strategy

```bash
# Redis backup
redis-cli BGSAVE

# Copy RDB file
cp /var/lib/redis/dump.rdb /backup/redis-$(date +%Y%m%d).rdb

# PostgreSQL backup
pg_dump turbine > /backup/turbine-$(date +%Y%m%d).sql
```

### Recovery Procedure

```bash
# 1. Stop workers
systemctl stop turbine-worker

# 2. Restore Redis
redis-cli SHUTDOWN
cp /backup/redis-20240109.rdb /var/lib/redis/dump.rdb
systemctl start redis

# 3. Restore PostgreSQL
psql turbine < /backup/turbine-20240109.sql

# 4. Start workers
systemctl start turbine-worker

# 5. Verify
turbine queues
```

## Scaling

### Horizontal Scaling

```bash
# Add more workers (same configuration)
for i in {1..5}; do
    ./turbine-worker --queues default &
done

# Add more servers
# Workers on server-1, server-2, server-3, ...
# All connect to same Redis broker
```

### Vertical Scaling

```toml
# Increase concurrency
[worker]
concurrency = 32  # More concurrent tasks per worker

# Increase prefetch
[broker]
prefetch = 20  # Fetch multiple tasks
```

### Queue Partitioning

```python
# Partition by user for isolation
from turbine.routing import consistent_hash_router

queue = consistent_hash_router(
    my_task,
    partition_key=f"user:{user_id}",
    num_queues=16  # 16 partition queues
)

my_task.apply_async(args=[data], queue=queue)

# Run dedicated workers per partition
# turbine worker --queues partition-0,partition-1
# turbine worker --queues partition-2,partition-3
# ...
```

## Cost Optimization

### Optimize Redis Usage

```python
# Shorter TTL for results you don't need
@task(result_ttl=3600)  # 1 hour instead of 24
def temporary_result():
    pass

# Don't store results at all
@task(store_result=False)
def fire_and_forget():
    pass

# Use compression
# Worker auto-compresses results > 1KB
```

### Use S3 for Large Results

```python
from turbine.backends import HybridBackend

# Results > 1MB go to S3 (cheaper than Redis)
backend = HybridBackend(
    redis_url="redis://localhost:6379",
    s3_bucket="cheap-large-storage",
    size_threshold=1048576
)
```

### Batch to Reduce Overhead

```python
# 1000 individual tasks = 1000 Redis writes
# 10 batch tasks (100 items each) = 10 Redis writes

from turbine.batch import Batcher

with Batcher(my_task, batch_size=100) as batcher:
    for item in large_dataset:
        batcher.add(item)
```

## Documentation

### Document Your Tasks

```python
@task(queue="emails", max_retries=3, timeout=120)
def send_welcome_email(user_id: int, email: str) -> dict:
    """
    Send welcome email to newly registered user.

    Args:
        user_id: User database ID
        email: User email address

    Returns:
        dict with 'sent' status and 'message_id'

    Raises:
        ValueError: If email is invalid
        SMTPError: If email server is down (will retry)

    Examples:
        >>> send_welcome_email.delay(123, "user@example.com")
        <AsyncResult: abc-123>

    Configuration:
        - Queue: emails
        - Retry: 3 times with 60s exponential backoff
        - Timeout: 120 seconds

    SLO: 99% success rate, p95 < 5 seconds
    """
    pass
```

## Summary Checklist

**Task Design:**
- [ ] Tasks are small and focused
- [ ] Tasks are idempotent
- [ ] Explicit dependencies
- [ ] Proper error handling
- [ ] Appropriate timeouts

**Performance:**
- [ ] Separate queues by type
- [ ] Batch similar operations
- [ ] Compression enabled
- [ ] Results disabled when not needed
- [ ] Monitoring configured

**Reliability:**
- [ ] DLQ enabled
- [ ] Retry strategy configured
- [ ] Graceful degradation
- [ ] Health checks in place
- [ ] Backup strategy defined

**Security:**
- [ ] TLS enabled
- [ ] No secrets in task args
- [ ] Input validation
- [ ] Rate limiting
- [ ] Audit logging

**Operations:**
- [ ] Monitoring and alerting
- [ ] Capacity planning
- [ ] Disaster recovery plan
- [ ] Documentation complete
- [ ] Team training done

## See Also

- [Configuration Guide](./CONFIGURATION.md)
- [Security Guide](./SECURITY.md)
- [Migration from Celery](./MIGRATION_FROM_CELERY.md)
- [Multi-Tenancy](./MULTI_TENANCY.md)
