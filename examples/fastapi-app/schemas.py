"""Pydantic schemas for request/response validation."""

from pydantic import BaseModel, EmailStr, Field
from typing import Optional, Any
from datetime import datetime
from enum import Enum


# Task status enum
class TaskStatus(str, Enum):
    PENDING = "pending"
    RUNNING = "running"
    SUCCESS = "success"
    FAILURE = "failure"
    RETRY = "retry"
    REVOKED = "revoked"


# Email schemas
class EmailRequest(BaseModel):
    """Request schema for sending an email."""
    to: EmailStr = Field(..., description="Recipient email address")
    subject: str = Field(..., min_length=1, max_length=200, description="Email subject")
    body: str = Field(..., min_length=1, description="Email body content")
    html: bool = Field(default=False, description="Whether body is HTML")

    model_config = {
        "json_schema_extra": {
            "examples": [
                {
                    "to": "user@example.com",
                    "subject": "Welcome!",
                    "body": "Thanks for signing up.",
                    "html": False
                }
            ]
        }
    }


class WelcomeEmailRequest(BaseModel):
    """Request schema for welcome email."""
    user_id: int = Field(..., gt=0, description="User ID")
    email: EmailStr = Field(..., description="User's email address")
    name: str = Field(..., min_length=1, max_length=100, description="User's name")


class BulkEmailRequest(BaseModel):
    """Request schema for bulk emails."""
    recipients: list[EmailStr] = Field(..., min_length=1, description="List of recipients")
    subject: str = Field(..., min_length=1, max_length=200)
    body: str = Field(..., min_length=1)


# Processing schemas
class ProcessDataRequest(BaseModel):
    """Request schema for data processing."""
    data: list[Any] = Field(..., min_length=1, description="Data to process")

    model_config = {
        "json_schema_extra": {
            "examples": [
                {"data": [1, 2, 3, 4, 5]}
            ]
        }
    }


class ReportType(str, Enum):
    DAILY = "daily"
    WEEKLY = "weekly"
    MONTHLY = "monthly"


class ReportRequest(BaseModel):
    """Request schema for report generation."""
    report_type: ReportType = Field(default=ReportType.DAILY)
    start_date: str = Field(..., pattern=r"^\d{4}-\d{2}-\d{2}$", description="YYYY-MM-DD")
    end_date: str = Field(..., pattern=r"^\d{4}-\d{2}-\d{2}$", description="YYYY-MM-DD")

    model_config = {
        "json_schema_extra": {
            "examples": [
                {
                    "report_type": "daily",
                    "start_date": "2024-01-01",
                    "end_date": "2024-01-31"
                }
            ]
        }
    }


# Response schemas
class TaskSubmittedResponse(BaseModel):
    """Response when a task is submitted."""
    task_id: str = Field(..., description="Unique task identifier")
    status: str = Field(default="submitted")
    message: str = Field(..., description="Human-readable message")


class TaskStatusResponse(BaseModel):
    """Response for task status check."""
    task_id: str
    status: TaskStatus
    result: Optional[Any] = None
    error: Optional[str] = None
    traceback: Optional[str] = None
    created_at: Optional[datetime] = None
    completed_at: Optional[datetime] = None


class TaskResultResponse(BaseModel):
    """Response with task result."""
    task_id: str
    status: TaskStatus
    result: Any
    execution_time: Optional[float] = None


class ErrorResponse(BaseModel):
    """Error response schema."""
    error: str
    detail: Optional[str] = None


# Health check
class HealthResponse(BaseModel):
    """Health check response."""
    status: str = "healthy"
    turbine_connected: bool
    version: str
