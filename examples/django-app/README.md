# Django + Turbine Example

This example demonstrates how to use Turbine as a task queue with Django.

## Setup

1. Install dependencies:
```bash
pip install -r requirements.txt
```

2. Start Turbine services:
```bash
cd ../../docker
docker-compose up -d redis turbine-server turbine-worker
```

3. Run migrations:
```bash
python manage.py migrate
```

4. Start the Django development server:
```bash
python manage.py runserver
```

## Usage

### Submit a task via API:
```bash
# Send welcome email
curl -X POST http://localhost:8000/api/send-email/ \
  -H "Content-Type: application/json" \
  -d '{"to": "user@example.com", "subject": "Welcome!", "body": "Thanks for signing up"}'

# Process data
curl -X POST http://localhost:8000/api/process-data/ \
  -H "Content-Type: application/json" \
  -d '{"data": [1, 2, 3, 4, 5]}'

# Check task status
curl http://localhost:8000/api/task/TASK_ID/
```

### Using the Django shell:
```python
from tasks.email import send_email
from tasks.processing import process_data

# Submit tasks
result = send_email.delay(to="user@example.com", subject="Hello", body="World")
print(f"Task ID: {result.task_id}")

# Check status
print(f"Status: {result.status}")
print(f"Result: {result.get(timeout=30)}")
```

## Project Structure

```
django-app/
├── manage.py
├── requirements.txt
├── myproject/
│   ├── __init__.py
│   ├── settings.py
│   ├── urls.py
│   └── wsgi.py
└── tasks/
    ├── __init__.py
    ├── email.py
    └── processing.py
```
