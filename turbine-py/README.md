# Turbine Python SDK

High-performance distributed task queue - Python SDK for Turbine.

## Installation

```bash
pip install turbine-queue
```

For Django support:
```bash
pip install turbine-queue[django]
```

For FastAPI support:
```bash
pip install turbine-queue[fastapi]
```

## Quick Start

### Basic Usage

```python
from turbine import Turbine, task

# Create app
app = Turbine(server="localhost:50051")

# Define a task
@app.task(queue="default")
def add(x: int, y: int) -> int:
    return x + y

# Submit task
result = add.delay(1, 2)

# Wait for result
print(result.get())  # Output: 3

# Check status
print(result.status)  # 'SUCCESS'
```

### Workflows

```python
from turbine import chain, group, chord

# Chain: sequential execution
workflow = chain(
    fetch_data.s(url),
    process_data.s(),
    store_results.s()
)
result = workflow.delay()

# Group: parallel execution
workflow = group(
    send_email.s(to="user1@example.com"),
    send_email.s(to="user2@example.com"),
)
results = workflow.delay().get()  # [result1, result2]

# Chord: parallel + callback
workflow = chord(
    [add.s(1, 2), add.s(3, 4), add.s(5, 6)],
    sum_results.s()
)
result = workflow.delay().get()  # sum of [3, 7, 11] = 21
```

### Django Integration

```python
# settings.py
INSTALLED_APPS = [
    ...
    'turbine.django',
]

TURBINE_SERVER = "localhost:50051"
TURBINE_DEFAULT_QUEUE = "django"

# myapp/tasks.py
from turbine.django import task

@task(queue="emails")
def send_email(to: str, subject: str, body: str):
    # Send email
    pass

# myapp/views.py
from myapp.tasks import send_email

def signup(request):
    user = create_user(request.POST)
    send_email.delay(
        to=user.email,
        subject="Welcome!",
        body="Thanks for signing up"
    )
    return redirect("home")
```

Management commands:
```bash
python manage.py turbine_status
python manage.py turbine_purge <queue_name>
```

### FastAPI Integration

```python
from fastapi import FastAPI, Depends
from turbine.fastapi import TurbineExtension, get_turbine, task

# Create extension
turbine = TurbineExtension(server="localhost:50051")

# Create app with lifespan
app = FastAPI(lifespan=turbine.lifespan)

# Define task
@task(queue="notifications")
def send_notification(user_id: int, message: str):
    pass

# Use in endpoint
@app.post("/notify")
async def notify(user_id: int, message: str):
    result = send_notification.delay(user_id, message)
    return {"task_id": result.task_id}

# Use Turbine as dependency
@app.get("/health")
async def health(turbine: Turbine = Depends(get_turbine)):
    return turbine.health_check()
```

## Configuration

### Environment Variables

- `TURBINE_SERVER`: Server address (default: `localhost:50051`)

### App Configuration

```python
app = Turbine(
    server="localhost:50051",
    secure=False,  # Use TLS
    default_queue="default",
    default_priority=0,
    default_max_retries=3,
    default_retry_delay=60,
    default_timeout=300,
    default_result_ttl=86400,
)
```

### Task Options

```python
@app.task(
    name="myapp.tasks.my_task",  # Custom name
    queue="high-priority",
    priority=10,
    max_retries=5,
    retry_delay=120,
    timeout=600,
    soft_timeout=540,
    store_result=True,
    result_ttl=3600,
    bind=False,  # Pass task as first arg
)
def my_task(x):
    return x * 2
```

### Submission Options

```python
# Delay execution
result = my_task.apply_async(
    args=[1, 2],
    kwargs={"key": "value"},
    countdown=60,  # Execute in 60 seconds
    eta=1704067200,  # Execute at Unix timestamp
    expires=1704153600,  # Expire at Unix timestamp
    task_id="custom-id",
    idempotency_key="unique-key",
    headers={"custom": "header"},
)
```

## CLI

```bash
# Generate gRPC stubs (required before first use)
turbine generate-proto

# Check server health
turbine health --server localhost:50051

# Show queue information
turbine queues --server localhost:50051

# Submit a task
turbine submit my_task --args '[1, 2]' --queue default --wait

# Get task status
turbine status <task_id>
```

## Development

```bash
# Install dev dependencies
pip install -e ".[dev]"

# Generate proto stubs
python -m turbine.generate_proto

# Run tests
pytest

# Format code
black src/
ruff check src/
```

## License

MIT License
