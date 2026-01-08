"""
Data processing tasks for Django example.

These tasks demonstrate CPU-intensive and long-running operations.
"""

from turbine import task, chain, group, chord
import logging
import time
import random

logger = logging.getLogger(__name__)


@task(
    name='process_data',
    queue='processing',
    timeout=600,
)
def process_data(data: list) -> dict:
    """
    Process a list of data items.

    Args:
        data: List of items to process

    Returns:
        dict with processing results
    """
    logger.info(f"Processing {len(data)} items")

    results = []
    for item in data:
        # Simulate processing
        time.sleep(0.1)
        results.append(item * 2)

    return {
        'input_count': len(data),
        'output_count': len(results),
        'results': results,
        'sum': sum(results),
    }


@task(
    name='generate_report',
    queue='processing',
    timeout=300,
)
def generate_report(report_type: str, start_date: str, end_date: str) -> dict:
    """
    Generate a report for the specified date range.

    Args:
        report_type: Type of report (daily, weekly, monthly)
        start_date: Start date (YYYY-MM-DD)
        end_date: End date (YYYY-MM-DD)

    Returns:
        dict with report data
    """
    logger.info(f"Generating {report_type} report: {start_date} to {end_date}")

    # Simulate report generation
    time.sleep(2)

    return {
        'report_type': report_type,
        'start_date': start_date,
        'end_date': end_date,
        'total_users': random.randint(100, 1000),
        'total_orders': random.randint(50, 500),
        'revenue': round(random.uniform(10000, 100000), 2),
        'generated_at': time.strftime('%Y-%m-%d %H:%M:%S'),
    }


@task(
    name='resize_image',
    queue='media',
    timeout=60,
)
def resize_image(image_path: str, width: int, height: int) -> dict:
    """
    Resize an image to the specified dimensions.

    Args:
        image_path: Path to the image file
        width: Target width
        height: Target height

    Returns:
        dict with resized image info
    """
    logger.info(f"Resizing image {image_path} to {width}x{height}")

    # Simulate image processing
    time.sleep(0.5)

    output_path = image_path.replace('.', f'_{width}x{height}.')

    return {
        'original': image_path,
        'resized': output_path,
        'width': width,
        'height': height,
    }


@task(
    name='aggregate_results',
    queue='processing',
)
def aggregate_results(results: list) -> dict:
    """
    Aggregate results from multiple tasks (used as chord callback).

    Args:
        results: List of results from previous tasks

    Returns:
        dict with aggregated data
    """
    logger.info(f"Aggregating {len(results)} results")

    total = sum(r.get('sum', 0) for r in results if isinstance(r, dict))

    return {
        'count': len(results),
        'total': total,
        'results': results,
    }


# Workflow examples
def process_data_pipeline(data_chunks: list) -> str:
    """
    Process multiple data chunks in parallel and aggregate results.

    This demonstrates using a chord (parallel tasks + callback).

    Args:
        data_chunks: List of data chunks to process

    Returns:
        Task ID of the chord
    """
    # Create a group of tasks for parallel processing
    tasks = group([
        process_data.s(chunk) for chunk in data_chunks
    ])

    # Create a chord that aggregates results after all tasks complete
    workflow = chord(tasks, aggregate_results.s())

    # Execute the workflow
    result = workflow.delay()
    return result.task_id


def generate_all_reports(date: str) -> str:
    """
    Generate all report types for a given date.

    This demonstrates using a chain (sequential tasks).

    Args:
        date: Date to generate reports for (YYYY-MM-DD)

    Returns:
        Task ID of the chain
    """
    # Create a chain of report generation tasks
    workflow = chain(
        generate_report.s('daily', date, date),
        generate_report.s('weekly', date, date),
        generate_report.s('monthly', date, date),
    )

    result = workflow.delay()
    return result.task_id
