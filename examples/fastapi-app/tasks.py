"""
Turbine task definitions for FastAPI example.
"""

from turbine import task, chain, group, chord
import logging
import time
import random
from typing import Any

logger = logging.getLogger(__name__)


# =============================================================================
# Email Tasks
# =============================================================================

@task(name='send_email', queue='emails', max_retries=3, timeout=120)
def send_email(to: str, subject: str, body: str, html: bool = False) -> dict:
    """Send an email to the specified recipient."""
    logger.info(f"Sending email to {to}: {subject}")

    # Simulate email sending
    time.sleep(0.5)

    message_id = f"msg_{int(time.time())}_{random.randint(1000, 9999)}"

    return {
        'success': True,
        'message_id': message_id,
        'recipient': to,
    }


@task(name='send_welcome_email', queue='emails', max_retries=3)
def send_welcome_email(user_id: int, email: str, name: str) -> dict:
    """Send a welcome email to a new user."""
    logger.info(f"Sending welcome email to user {user_id}")

    subject = f"Welcome, {name}!"
    body = f"Hi {name}, welcome to our platform!"

    result = send_email(to=email, subject=subject, body=body)

    return {
        'user_id': user_id,
        'email_sent': result['success'],
        'message_id': result.get('message_id'),
    }


@task(name='send_bulk_emails', queue='emails', timeout=3600)
def send_bulk_emails(recipients: list, subject: str, body: str) -> dict:
    """Send emails to multiple recipients."""
    logger.info(f"Sending bulk email to {len(recipients)} recipients")

    success_count = 0
    failures = []

    for recipient in recipients:
        try:
            result = send_email(to=recipient, subject=subject, body=body)
            if result['success']:
                success_count += 1
            else:
                failures.append(recipient)
        except Exception as e:
            logger.error(f"Failed to send to {recipient}: {e}")
            failures.append(recipient)

    return {
        'total': len(recipients),
        'success': success_count,
        'failed': len(failures),
        'failures': failures,
    }


# =============================================================================
# Processing Tasks
# =============================================================================

@task(name='process_data', queue='processing', timeout=600)
def process_data(data: list) -> dict:
    """Process a list of data items."""
    logger.info(f"Processing {len(data)} items")

    results = []
    for item in data:
        time.sleep(0.05)  # Simulate processing
        if isinstance(item, (int, float)):
            results.append(item * 2)
        else:
            results.append(item)

    return {
        'input_count': len(data),
        'output_count': len(results),
        'results': results,
        'sum': sum(r for r in results if isinstance(r, (int, float))),
    }


@task(name='generate_report', queue='processing', timeout=300)
def generate_report(report_type: str, start_date: str, end_date: str) -> dict:
    """Generate a report for the specified date range."""
    logger.info(f"Generating {report_type} report: {start_date} to {end_date}")

    time.sleep(1)  # Simulate report generation

    return {
        'report_type': report_type,
        'start_date': start_date,
        'end_date': end_date,
        'metrics': {
            'total_users': random.randint(100, 1000),
            'active_users': random.randint(50, 500),
            'total_orders': random.randint(50, 500),
            'revenue': round(random.uniform(10000, 100000), 2),
        },
        'generated_at': time.strftime('%Y-%m-%d %H:%M:%S'),
    }


@task(name='resize_image', queue='media', timeout=60)
def resize_image(image_path: str, width: int, height: int) -> dict:
    """Resize an image to the specified dimensions."""
    logger.info(f"Resizing {image_path} to {width}x{height}")

    time.sleep(0.3)  # Simulate processing

    output_path = image_path.rsplit('.', 1)
    if len(output_path) == 2:
        output_path = f"{output_path[0]}_{width}x{height}.{output_path[1]}"
    else:
        output_path = f"{image_path}_{width}x{height}"

    return {
        'original': image_path,
        'resized': output_path,
        'dimensions': {'width': width, 'height': height},
    }


@task(name='aggregate_results', queue='processing')
def aggregate_results(results: list) -> dict:
    """Aggregate results from multiple tasks (chord callback)."""
    total = 0
    for r in results:
        if isinstance(r, dict) and 'sum' in r:
            total += r['sum']

    return {
        'count': len(results),
        'total': total,
        'results': results,
    }


# =============================================================================
# Workflow Helpers
# =============================================================================

def create_data_pipeline(data_chunks: list[list]) -> str:
    """
    Process multiple data chunks in parallel and aggregate results.

    Returns the task ID of the chord.
    """
    tasks = group([process_data.s(chunk) for chunk in data_chunks])
    workflow = chord(tasks, aggregate_results.s())
    result = workflow.delay()
    return result.task_id


def create_report_chain(date: str) -> str:
    """
    Generate daily, weekly, and monthly reports sequentially.

    Returns the task ID of the chain.
    """
    workflow = chain(
        generate_report.s('daily', date, date),
        generate_report.s('weekly', date, date),
        generate_report.s('monthly', date, date),
    )
    result = workflow.delay()
    return result.task_id
