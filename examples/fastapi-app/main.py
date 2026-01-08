"""
FastAPI application with Turbine task queue integration.
"""

from fastapi import FastAPI, HTTPException, Query
from fastapi.responses import JSONResponse
from contextlib import asynccontextmanager
import logging

from turbine import Turbine
from tasks import (
    send_email,
    send_welcome_email,
    send_bulk_emails,
    process_data,
    generate_report,
    resize_image,
    create_data_pipeline,
    create_report_chain,
)
from schemas import (
    EmailRequest,
    WelcomeEmailRequest,
    BulkEmailRequest,
    ProcessDataRequest,
    ReportRequest,
    TaskSubmittedResponse,
    TaskStatusResponse,
    TaskResultResponse,
    ErrorResponse,
    HealthResponse,
    TaskStatus,
)

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

# Global Turbine client
turbine: Turbine = None


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager."""
    global turbine

    # Startup
    logger.info("Connecting to Turbine server...")
    turbine = Turbine(server="localhost:50051")
    await turbine.connect()
    logger.info("Connected to Turbine server")

    yield

    # Shutdown
    logger.info("Disconnecting from Turbine server...")
    await turbine.disconnect()
    logger.info("Disconnected from Turbine server")


app = FastAPI(
    title="Turbine FastAPI Example",
    description="Example FastAPI application demonstrating Turbine task queue integration",
    version="1.0.0",
    lifespan=lifespan,
)


# =============================================================================
# Health Check
# =============================================================================

@app.get("/health", response_model=HealthResponse, tags=["Health"])
async def health_check():
    """Check application and Turbine connection health."""
    try:
        connected = turbine is not None and await turbine.ping()
    except Exception:
        connected = False

    return HealthResponse(
        status="healthy" if connected else "degraded",
        turbine_connected=connected,
        version="1.0.0",
    )


# =============================================================================
# Email Tasks
# =============================================================================

@app.post(
    "/tasks/email",
    response_model=TaskSubmittedResponse,
    responses={400: {"model": ErrorResponse}},
    tags=["Email"],
)
async def submit_email_task(request: EmailRequest):
    """Submit an email sending task."""
    result = send_email.delay(
        to=request.to,
        subject=request.subject,
        body=request.body,
        html=request.html,
    )

    return TaskSubmittedResponse(
        task_id=result.task_id,
        status="submitted",
        message=f"Email task submitted for {request.to}",
    )


@app.post(
    "/tasks/welcome-email",
    response_model=TaskSubmittedResponse,
    responses={400: {"model": ErrorResponse}},
    tags=["Email"],
)
async def submit_welcome_email_task(request: WelcomeEmailRequest):
    """Submit a welcome email task for a new user."""
    result = send_welcome_email.delay(
        user_id=request.user_id,
        email=request.email,
        name=request.name,
    )

    return TaskSubmittedResponse(
        task_id=result.task_id,
        status="submitted",
        message=f"Welcome email task submitted for user {request.user_id}",
    )


@app.post(
    "/tasks/bulk-email",
    response_model=TaskSubmittedResponse,
    responses={400: {"model": ErrorResponse}},
    tags=["Email"],
)
async def submit_bulk_email_task(request: BulkEmailRequest):
    """Submit a bulk email task."""
    result = send_bulk_emails.delay(
        recipients=request.recipients,
        subject=request.subject,
        body=request.body,
    )

    return TaskSubmittedResponse(
        task_id=result.task_id,
        status="submitted",
        message=f"Bulk email task submitted for {len(request.recipients)} recipients",
    )


# =============================================================================
# Processing Tasks
# =============================================================================

@app.post(
    "/tasks/process",
    response_model=TaskSubmittedResponse,
    responses={400: {"model": ErrorResponse}},
    tags=["Processing"],
)
async def submit_process_task(request: ProcessDataRequest):
    """Submit a data processing task."""
    result = process_data.delay(data=request.data)

    return TaskSubmittedResponse(
        task_id=result.task_id,
        status="submitted",
        message=f"Processing task submitted for {len(request.data)} items",
    )


@app.post(
    "/tasks/report",
    response_model=TaskSubmittedResponse,
    responses={400: {"model": ErrorResponse}},
    tags=["Processing"],
)
async def submit_report_task(request: ReportRequest):
    """Submit a report generation task."""
    result = generate_report.delay(
        report_type=request.report_type.value,
        start_date=request.start_date,
        end_date=request.end_date,
    )

    return TaskSubmittedResponse(
        task_id=result.task_id,
        status="submitted",
        message=f"Report generation task submitted ({request.report_type.value})",
    )


# =============================================================================
# Workflow Endpoints
# =============================================================================

@app.post(
    "/workflows/data-pipeline",
    response_model=TaskSubmittedResponse,
    tags=["Workflows"],
)
async def submit_data_pipeline(chunks: list[list] = [[1, 2, 3], [4, 5, 6], [7, 8, 9]]):
    """
    Submit a data processing pipeline.

    Processes multiple data chunks in parallel and aggregates results.
    """
    task_id = create_data_pipeline(chunks)

    return TaskSubmittedResponse(
        task_id=task_id,
        status="submitted",
        message=f"Data pipeline submitted with {len(chunks)} chunks",
    )


@app.post(
    "/workflows/report-chain",
    response_model=TaskSubmittedResponse,
    tags=["Workflows"],
)
async def submit_report_chain(date: str = Query(..., pattern=r"^\d{4}-\d{2}-\d{2}$")):
    """
    Submit a report generation chain.

    Generates daily, weekly, and monthly reports sequentially.
    """
    task_id = create_report_chain(date)

    return TaskSubmittedResponse(
        task_id=task_id,
        status="submitted",
        message=f"Report chain submitted for {date}",
    )


# =============================================================================
# Task Status and Results
# =============================================================================

@app.get(
    "/tasks/{task_id}",
    response_model=TaskStatusResponse,
    responses={404: {"model": ErrorResponse}},
    tags=["Tasks"],
)
async def get_task_status(task_id: str):
    """Get the status of a task."""
    try:
        result = await turbine.get_result(task_id)

        if result is None:
            return TaskStatusResponse(
                task_id=task_id,
                status=TaskStatus.PENDING,
            )

        # Map Turbine status to our enum
        status_map = {
            "pending": TaskStatus.PENDING,
            "running": TaskStatus.RUNNING,
            "success": TaskStatus.SUCCESS,
            "failure": TaskStatus.FAILURE,
            "retry": TaskStatus.RETRY,
            "revoked": TaskStatus.REVOKED,
        }

        return TaskStatusResponse(
            task_id=task_id,
            status=status_map.get(result.status, TaskStatus.PENDING),
            result=result.result if result.successful else None,
            error=result.error if result.failed else None,
            traceback=result.traceback if result.failed else None,
            created_at=result.created_at,
            completed_at=result.completed_at,
        )

    except Exception as e:
        logger.error(f"Error getting task status: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.get(
    "/tasks/{task_id}/wait",
    response_model=TaskResultResponse,
    responses={
        404: {"model": ErrorResponse},
        408: {"model": ErrorResponse},
    },
    tags=["Tasks"],
)
async def wait_for_task(
    task_id: str,
    timeout: float = Query(default=30.0, ge=1.0, le=300.0, description="Timeout in seconds"),
):
    """Wait for a task to complete and return the result."""
    try:
        result = await turbine.get_result(task_id, timeout=timeout)

        if result is None:
            raise HTTPException(
                status_code=408,
                detail=f"Task {task_id} did not complete within {timeout} seconds",
            )

        if result.failed:
            raise HTTPException(
                status_code=500,
                detail=f"Task failed: {result.error}",
            )

        status_map = {
            "pending": TaskStatus.PENDING,
            "running": TaskStatus.RUNNING,
            "success": TaskStatus.SUCCESS,
            "failure": TaskStatus.FAILURE,
            "retry": TaskStatus.RETRY,
            "revoked": TaskStatus.REVOKED,
        }

        return TaskResultResponse(
            task_id=task_id,
            status=status_map.get(result.status, TaskStatus.SUCCESS),
            result=result.result,
            execution_time=result.execution_time,
        )

    except HTTPException:
        raise
    except Exception as e:
        logger.error(f"Error waiting for task: {e}")
        raise HTTPException(status_code=500, detail=str(e))


@app.delete(
    "/tasks/{task_id}",
    responses={404: {"model": ErrorResponse}},
    tags=["Tasks"],
)
async def revoke_task(task_id: str):
    """Revoke a pending or running task."""
    try:
        await turbine.revoke(task_id)
        return {"task_id": task_id, "status": "revoked"}
    except Exception as e:
        logger.error(f"Error revoking task: {e}")
        raise HTTPException(status_code=500, detail=str(e))


# =============================================================================
# Error Handlers
# =============================================================================

@app.exception_handler(Exception)
async def global_exception_handler(request, exc):
    """Global exception handler."""
    logger.error(f"Unhandled exception: {exc}")
    return JSONResponse(
        status_code=500,
        content={"error": "Internal server error", "detail": str(exc)},
    )


if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8000)
