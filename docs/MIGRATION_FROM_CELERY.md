# Migrating from Celery to Turbine

This guide helps you migrate your existing Celery application to Turbine.

## Why Migrate?

| Benefit | Impact |
|---------|--------|
| Memory Efficiency | ~10x less memory usage |
| True Parallelism | No GIL limitations |
| Better Reliability | Visibility timeout, automatic redelivery |
| Simpler Config | Sensible defaults, single config file |
| Modern Observability | Prometheus metrics, OpenTelemetry built-in |
| Dead Letter Queue | Native DLQ support |

## Quick Comparison

### Task Definition

**Celery:**
```python
from celery import Celery

app = Celery('myapp', broker='redis://localhost:6379')

@app.task(bind=True, max_retries=3)
def send_email(self, to, subject, body):
    try:
        # Send email
        pass
    except Exception as exc:
        raise self.retry(exc=exc, countdown=60)
```

**Turbine:**
```python
from turbine import Turbine, task

app = Turbine(server='localhost:50051')

@task(bind=True, max_retries=3, retry_delay=60)
def send_email(self, to, subject, body):
    # Send email
    pass
    # Retry is automatic on exception
```

### Task Submission

**Celery:**
```python
# Simple
result = send_email.delay('user@example.com', 'Hello', 'World')

# With options
result = send_email.apply_async(
    args=['user@example.com', 'Hello', 'World'],
    countdown=60,
    queue='emails'
)
```

**Turbine:**
```python
# Simple (identical!)
result = send_email.delay('user@example.com', 'Hello', 'World')

# With options (identical!)
result = send_email.apply_async(
    args=['user@example.com', 'Hello', 'World'],
    countdown=60,
    queue='emails'
)
```

### Workflows

**Celery:**
```python
from celery import chain, group, chord

# Chain
workflow = chain(task1.s(), task2.s(), task3.s())
workflow.delay()

# Group
workflow = group(task1.s(1), task1.s(2), task1.s(3))
workflow.delay()

# Chord
workflow = chord([task1.s(1), task1.s(2)], callback.s())
workflow.delay()
```

**Turbine:**
```python
from turbine import chain, group, chord

# Chain (identical!)
workflow = chain(task1.s(), task2.s(), task3.s())
workflow.delay()

# Group (identical!)
workflow = group(task1.s(1), task1.s(2), task1.s(3))
workflow.delay()

# Chord (identical!)
workflow = chord([task1.s(1), task1.s(2)], callback.s())
workflow.delay()
```

## Step-by-Step Migration

### 1. Infrastructure Setup

**Install Turbine Server**

```bash
# Download latest release
wget https://github.com/turbine-queue/turbine/releases/latest/download/turbine-server-linux-amd64

# Or build from source
cargo build --release -p turbine-server

# Start server
./turbine-server --broker-url redis://localhost:6379
```

**Or use Docker:**

```bash
docker run -d \
  --name turbine-server \
  -p 50051:50051 \
  -e TURBINE_BROKER_URL=redis://redis:6379 \
  turbine/turbine-server:latest
```

### 2. Update Dependencies

**Remove Celery:**

```bash
pip uninstall celery
```

**Install Turbine:**

```bash
pip install turbine-queue[worker,django]
```

**Update requirements.txt:**

```diff
- celery[redis]>=5.3.0
+ turbine-queue[worker]>=0.1.0
```

### 3. Code Changes

#### Application Initialization

**Celery:**
```python
# celery.py
from celery import Celery

app = Celery(
    'myapp',
    broker='redis://localhost:6379/0',
    backend='redis://localhost:6379/0',
)

app.conf.update(
    task_serializer='json',
    result_serializer='json',
    accept_content=['json'],
    timezone='UTC',
    enable_utc=True,
)
```

**Turbine:**
```python
# turbine_app.py
from turbine import Turbine

app = Turbine(
    server='localhost:50051',
)

# Server handles broker/backend configuration
# No serializer config needed (MessagePack by default)
```

#### Task Decorators

Most task options map directly:

| Celery | Turbine | Notes |
|--------|---------|-------|
| `@app.task()` | `@task()` | Nearly identical |
| `bind=True` | `bind=True` | Same |
| `max_retries=3` | `max_retries=3` | Same |
| `default_retry_delay=60` | `retry_delay=60` | Same concept |
| `time_limit=300` | `timeout=300` | Same |
| `soft_time_limit=250` | `soft_timeout=250` | Same |
| `queue='emails'` | `queue='emails'` | Same |
| `priority=10` | `priority=10` | Same |
| `expires=3600` | `expires=3600` | Same |
| `countdown=60` | `countdown=60` | Same (use in apply_async) |

**Changes needed:**

```python
# Celery
@app.task(bind=True, max_retries=3, default_retry_delay=60)
def my_task(self, x):
    pass

# Turbine
@task(bind=True, max_retries=3, retry_delay=60)
def my_task(self, x):
    pass
```

#### Retry Behavior

**Celery:**
```python
@app.task(bind=True, max_retries=3)
def flaky_task(self, x):
    try:
        # do something
        pass
    except SomeError as exc:
        raise self.retry(exc=exc, countdown=60)
```

**Turbine:**
```python
@task(bind=True, max_retries=3, retry_delay=60)
def flaky_task(self, x):
    # do something
    # Retry is automatic with exponential backoff
    # No need to catch and retry manually
    pass
```

### 4. Django Integration

#### Settings

**Celery:**
```python
# settings.py
CELERY_BROKER_URL = 'redis://localhost:6379/0'
CELERY_RESULT_BACKEND = 'redis://localhost:6379/0'
CELERY_TASK_SERIALIZER = 'json'
CELERY_RESULT_SERIALIZER = 'json'
CELERY_ACCEPT_CONTENT = ['json']
CELERY_TIMEZONE = 'UTC'

# celery.py
import os
from celery import Celery

os.environ.setdefault('DJANGO_SETTINGS_MODULE', 'myproject.settings')

app = Celery('myproject')
app.config_from_object('django.conf:settings', namespace='CELERY')
app.autodiscover_tasks()
```

**Turbine:**
```python
# settings.py
INSTALLED_APPS = [
    ...
    'turbine.django',
]

TURBINE_SERVER = 'localhost:50051'
TURBINE_BROKER_URL = 'redis://localhost:6379'

# No separate celery.py file needed!
# Tasks are auto-discovered from tasks.py in each app
```

#### Management Commands

**Celery:**
```bash
# Start worker
celery -A myproject worker -l info

# Inspect
celery -A myproject inspect active
celery -A myproject inspect stats

# Purge queues
celery -A myproject purge
```

**Turbine:**
```bash
# Start Python worker
python manage.py turbine_worker -Q default,emails -c 4

# Or use CLI
turbine worker -I myapp.tasks -Q default

# Inspect
python manage.py turbine_status --tasks

# Purge queues
python manage.py turbine_purge default
```

### 5. Result Retrieval

**Celery:**
```python
result = my_task.delay(x)

# Check if ready
if result.ready():
    value = result.get()

# Wait with timeout
try:
    value = result.get(timeout=10)
except TimeoutError:
    print("Task not ready")

# Forget result
result.forget()
```

**Turbine:**
```python
result = my_task.delay(x)

# Check status
status = result.app.get_task_status(result.task_id)
if status['state'] == 'SUCCESS':
    ready, value = result.get()

# Wait with timeout
try:
    value = result.get(timeout=10)
except TimeoutError:
    print("Task not ready")

# Results auto-expire (24h default)
```

### 6. Periodic Tasks (Beat)

**Celery:**
```python
# celery.py
from celery.schedules import crontab

app.conf.beat_schedule = {
    'cleanup-every-night': {
        'task': 'myapp.tasks.cleanup',
        'schedule': crontab(hour=2, minute=0),
    },
}
```

**Turbine:**
```python
# Configured in Turbine server config (turbine.toml)
[scheduler.schedules]
cleanup-every-night = { task = "myapp.tasks.cleanup", cron = "0 2 * * *" }
```

Or via gRPC API (coming soon).

### 7. Configuration Migration

| Celery Setting | Turbine Equivalent | Location |
|----------------|-------------------|----------|
| `CELERY_BROKER_URL` | `broker_url` | Server config |
| `CELERY_RESULT_BACKEND` | `backend_url` | Server config |
| `CELERYD_CONCURRENCY` | `concurrency` | Worker config |
| `CELERY_TASK_SERIALIZER` | N/A | MessagePack (fixed) |
| `CELERY_TASK_TIME_LIMIT` | `timeout` | Task option |
| `CELERY_TASK_SOFT_TIME_LIMIT` | `soft_timeout` | Task option |
| `CELERY_TASK_DEFAULT_QUEUE` | `queue` | Task option |
| `CELERY_TASK_DEFAULT_PRIORITY` | `priority` | Task option |
| `CELERY_RESULT_EXPIRES` | `result_ttl` | Task option |

### 8. Worker Deployment

**Celery:**
```bash
# Systemd service
[Unit]
Description=Celery Worker
After=network.target

[Service]
Type=forking
User=celery
Group=celery
WorkingDirectory=/app
ExecStart=/usr/bin/celery -A myproject worker --loglevel=info
Restart=always

[Install]
WantedBy=multi-user.target
```

**Turbine:**
```bash
# Use Python worker or Rust worker

# Python worker (for Python tasks)
[Unit]
Description=Turbine Python Worker
After=network.target turbine-server.service

[Service]
Type=simple
User=turbine
Group=turbine
WorkingDirectory=/app
ExecStart=/usr/bin/turbine worker -I myapp.tasks -Q default
Restart=always

[Install]
WantedBy=multi-user.target
```

## Feature Mapping

### Supported Features

✅ **Fully Compatible:**
- Task definition with @task decorator
- Task submission (delay, apply_async)
- Workflows (chain, group, chord)
- Result retrieval
- Task retry with exponential backoff
- Priority queues
- ETA and countdown
- Task expiration
- Idempotency keys
- Custom headers/metadata

✅ **Enhanced in Turbine:**
- Dead Letter Queue (native support)
- Multi-tenancy
- Result compression
- Better observability (Prometheus + OpenTelemetry)

### Differences

| Feature | Celery | Turbine | Notes |
|---------|--------|---------|-------|
| Serialization | JSON/Pickle/YAML | MessagePack | Not configurable |
| Task routing | Kombu routing | Queue-based | Simpler model |
| Result retrieval | Polling | gRPC | More efficient |
| Canvas | Signatures/chains | Signatures/chains | Same API |
| Beat scheduler | Separate process | Built into server | Simpler deployment |

### Not Yet Supported

⏳ **Coming Soon:**
- `task.apply()` (synchronous execution)
- `ResultSet` for group results
- Task replacement/revocation with `replace=True`
- Custom task classes
- Task inheritance

## Migration Checklist

- [ ] Install Turbine server
- [ ] Update Python dependencies
- [ ] Replace `from celery import Celery` with `from turbine import Turbine`
- [ ] Replace `@app.task` with `@task`
- [ ] Update worker startup commands
- [ ] Migrate Beat schedule to server config
- [ ] Update deployment scripts
- [ ] Test task execution
- [ ] Test workflows (chains, groups, chords)
- [ ] Update monitoring (switch to Prometheus metrics)
- [ ] Deploy to staging
- [ ] Run parallel with Celery (gradual migration)
- [ ] Monitor for issues
- [ ] Deploy to production
- [ ] Decommission Celery

## Gradual Migration Strategy

You can run Celery and Turbine in parallel:

### Phase 1: New Tasks to Turbine

```python
# Keep existing Celery app for old tasks
from celery import Celery
celery_app = Celery('myapp', broker='redis://localhost:6379/0')

# New Turbine app for new tasks
from turbine import Turbine
turbine_app = Turbine(server='localhost:50051')

# Old tasks still use Celery
@celery_app.task
def old_task():
    pass

# New tasks use Turbine
@task
def new_task():
    pass
```

### Phase 2: Migrate Tasks Gradually

1. Start Turbine server alongside Celery
2. Run Turbine workers for new queues
3. Migrate one queue at a time
4. Update task decorators incrementally
5. Monitor both systems
6. Once all tasks migrated, decommission Celery

### Phase 3: Clean Up

```bash
# Stop Celery workers
sudo systemctl stop celery-worker

# Remove Celery package
pip uninstall celery

# Clean up Celery config
# Remove from settings.py, celery.py, etc.
```

## Common Migration Issues

### Issue: Task Not Found

**Error**: `No handler registered for task 'myapp.tasks.my_task'`

**Solution**: Ensure task module is imported by worker

```bash
# Specify task modules explicitly
turbine worker -I myapp.tasks -I myapp.other_tasks
```

### Issue: Serialization Errors

**Error**: `TypeError: Object of type X is not JSON serializable`

**Note**: Turbine uses MessagePack which supports more types than JSON

**Solution**: For custom objects, implement `__dict__` or use dataclasses:

```python
from dataclasses import dataclass, asdict

@dataclass
class MyData:
    field1: str
    field2: int

# Serialize to dict
@task
def my_task(data: dict):
    obj = MyData(**data)
    # process...
    return asdict(obj)

# Submit
my_task.delay(asdict(MyData("test", 123)))
```

### Issue: Task Results Not Found

**Problem**: Different result backend format

**Solution**: Turbine stores results differently. Use AsyncResult.get() instead of direct backend access:

```python
# Don't do this
from redis import Redis
r = Redis()
result = r.get(f'celery-task-meta-{task_id}')  # Won't work

# Do this
from turbine import Turbine
app = Turbine(server='localhost:50051')
status = app.get_task_status(task_id)
```

### Issue: Beat Schedule Format

**Celery:** Python dict in settings

**Turbine:** TOML config file

**Migration:**

```python
# Celery (settings.py)
CELERY_BEAT_SCHEDULE = {
    'cleanup': {
        'task': 'tasks.cleanup',
        'schedule': crontab(hour=2, minute=0),
    },
}
```

```toml
# Turbine (turbine.toml)
[scheduler.schedules]
cleanup = { task = "tasks.cleanup", cron = "0 2 * * *" }
```

## Testing Migration

### 1. Test Individual Tasks

```python
# test_migration.py
import pytest
from turbine import Turbine, task

app = Turbine(server='localhost:50051')

@task
def add(x, y):
    return x + y

def test_task_execution():
    result = add.delay(2, 3)
    value = result.get(timeout=10)
    assert value == 5
```

### 2. Test Workflows

```python
def test_chain():
    from turbine import chain

    workflow = chain(
        add.s(1, 2),
        add.s(3),
    )
    workflow_id, task_ids = workflow.delay()

    # Wait for completion
    import time
    time.sleep(2)

    # Check final result
    result = AsyncResult(task_ids[-1], app.client)
    assert result.get() == 6  # (1+2)+3
```

### 3. Load Testing

```bash
# Submit many tasks to verify throughput
python -c "
from turbine import Turbine, task
import time

app = Turbine(server='localhost:50051')

@task
def noop():
    pass

start = time.time()
for i in range(10000):
    noop.delay()
elapsed = time.time() - start

print(f'Submitted 10000 tasks in {elapsed:.2f}s ({10000/elapsed:.0f} tasks/sec)')
"
```

## Performance Comparison

### Before (Celery)

```bash
# Memory usage per worker
$ ps aux | grep celery
celery    1234  5.2  2.1  450MB  ...

# Total for 10 workers: ~4.5GB
```

### After (Turbine)

```bash
# Rust worker
$ ps aux | grep turbine-worker
turbine   5678  1.2  0.2  45MB  ...

# Python worker
$ ps aux | grep "turbine worker"
turbine   9012  2.1  0.5  120MB  ...

# 10 Rust workers: ~450MB (10x improvement)
# 10 Python workers: ~1.2GB (3-4x improvement)
```

## Rollback Plan

If you need to rollback:

1. **Keep Celery installed** during migration period
2. **Run both systems** in parallel
3. **Route back to Celery** by changing decorators
4. **Gradual rollback** one queue at a time

```python
# Emergency rollback: swap imports
# from turbine import Turbine, task
from celery import Celery, task

# Minimal code changes needed
```

## Best Practices

1. **Start Small**: Migrate non-critical tasks first
2. **Monitor Closely**: Watch metrics during migration
3. **Keep Celery Running**: Run in parallel initially
4. **Test Thoroughly**: Test all workflows before production
5. **Gradual Rollout**: One queue/service at a time
6. **Document Changes**: Keep team informed

## Support

Need help with migration?
- [GitHub Discussions](https://github.com/turbine-queue/turbine/discussions)
- [GitHub Issues](https://github.com/turbine-queue/turbine/issues)
- [Discord](https://discord.gg/turbine) (coming soon)

## See Also

- [Configuration Guide](./configuration.md)
- [Task Options Reference](./tasks.md)
- [Workflow Guide](./workflows.md)
- [Multi-Tenancy](./MULTI_TENANCY.md)
