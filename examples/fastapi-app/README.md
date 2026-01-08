# FastAPI + Turbine Example

This example demonstrates how to use Turbine as a task queue with FastAPI.

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

3. Start the FastAPI server:
```bash
uvicorn main:app --reload
```

## API Documentation

Once running, visit:
- Swagger UI: http://localhost:8000/docs
- ReDoc: http://localhost:8000/redoc

## Usage

### Submit tasks:
```bash
# Send email
curl -X POST http://localhost:8000/tasks/email \
  -H "Content-Type: application/json" \
  -d '{"to": "user@example.com", "subject": "Hello", "body": "World"}'

# Process data
curl -X POST http://localhost:8000/tasks/process \
  -H "Content-Type: application/json" \
  -d '{"data": [1, 2, 3, 4, 5]}'

# Generate report
curl -X POST http://localhost:8000/tasks/report \
  -H "Content-Type: application/json" \
  -d '{"report_type": "daily", "start_date": "2024-01-01", "end_date": "2024-01-31"}'

# Check task status
curl http://localhost:8000/tasks/TASK_ID

# Wait for result (with timeout)
curl "http://localhost:8000/tasks/TASK_ID/wait?timeout=30"
```

## Project Structure

```
fastapi-app/
├── main.py           # FastAPI application and routes
├── tasks.py          # Turbine task definitions
├── schemas.py        # Pydantic models
└── requirements.txt
```
