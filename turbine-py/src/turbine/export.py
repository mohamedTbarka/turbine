"""Task result export utilities."""

import logging
import json
import csv
from typing import Any, TextIO
from datetime import datetime

logger = logging.getLogger(__name__)


class ResultExporter:
    """
    Export task results to various formats.

    Supports: JSON, CSV, JSON Lines (JSONL)
    """

    def __init__(self, turbine_app: Any):
        """
        Initialize result exporter.

        Args:
            turbine_app: Turbine application instance
        """
        self.app = turbine_app

    def export_to_json(
        self,
        task_ids: list[str],
        output_file: str,
        pretty: bool = True,
    ) -> int:
        """
        Export task results to JSON file.

        Args:
            task_ids: List of task IDs to export
            output_file: Output file path
            pretty: Pretty-print JSON

        Returns:
            Number of results exported

        Example:
            exporter = ResultExporter(app)
            count = exporter.export_to_json(
                task_ids=["id1", "id2", "id3"],
                output_file="results.json"
            )
        """
        results = []

        for task_id in task_ids:
            try:
                status = self.app.get_task_status(task_id)
                results.append({
                    "task_id": task_id,
                    "state": status.get("state"),
                    "created_at": status.get("created_at"),
                    "finished_at": status.get("finished_at"),
                    "worker_id": status.get("worker_id"),
                    "error": status.get("error"),
                })

            except Exception as e:
                logger.error(f"Failed to get status for {task_id}: {e}")

        with open(output_file, 'w') as f:
            if pretty:
                json.dump(results, f, indent=2, default=str)
            else:
                json.dump(results, f, default=str)

        logger.info(f"Exported {len(results)} results to {output_file}")
        return len(results)

    def export_to_jsonl(
        self,
        task_ids: list[str],
        output_file: str,
    ) -> int:
        """
        Export to JSON Lines format (one JSON object per line).

        Args:
            task_ids: List of task IDs
            output_file: Output file path

        Returns:
            Number of results exported

        Example:
            count = exporter.export_to_jsonl(task_ids, "results.jsonl")
        """
        count = 0

        with open(output_file, 'w') as f:
            for task_id in task_ids:
                try:
                    status = self.app.get_task_status(task_id)
                    f.write(json.dumps(status, default=str) + "\n")
                    count += 1

                except Exception as e:
                    logger.error(f"Failed to export {task_id}: {e}")

        logger.info(f"Exported {count} results to {output_file}")
        return count

    def export_to_csv(
        self,
        task_ids: list[str],
        output_file: str,
        fields: list[str] | None = None,
    ) -> int:
        """
        Export to CSV format.

        Args:
            task_ids: List of task IDs
            output_file: Output file path
            fields: Fields to export (None = all)

        Returns:
            Number of results exported

        Example:
            count = exporter.export_to_csv(
                task_ids,
                "results.csv",
                fields=["task_id", "state", "created_at"]
            )
        """
        if fields is None:
            fields = ["task_id", "state", "created_at", "finished_at", "worker_id"]

        results = []

        for task_id in task_ids:
            try:
                status = self.app.get_task_status(task_id)
                row = {field: status.get(field) for field in fields}
                results.append(row)

            except Exception as e:
                logger.error(f"Failed to export {task_id}: {e}")

        with open(output_file, 'w', newline='') as f:
            writer = csv.DictWriter(f, fieldnames=fields)
            writer.writeheader()
            writer.writerows(results)

        logger.info(f"Exported {len(results)} results to {output_file}")
        return len(results)

    def stream_results(
        self,
        task_ids: list[str],
        format: str = "json",
    ):
        """
        Stream results as generator.

        Args:
            task_ids: List of task IDs
            format: Output format ('json', 'dict')

        Yields:
            Task results

        Example:
            for result in exporter.stream_results(task_ids):
                process(result)
        """
        for task_id in task_ids:
            try:
                status = self.app.get_task_status(task_id)

                if format == "json":
                    yield json.dumps(status, default=str)
                else:
                    yield status

            except Exception as e:
                logger.error(f"Failed to stream {task_id}: {e}")


class DLQExporter:
    """Export failed tasks from Dead Letter Queue."""

    def __init__(self, backend_url: str = "redis://localhost:6379"):
        """
        Initialize DLQ exporter.

        Args:
            backend_url: Redis backend URL
        """
        from turbine.dlq import DLQManager

        self.dlq = DLQManager(backend_url)

    def export_failed_tasks(
        self,
        output_file: str,
        format: str = "json",
        limit: int = 1000,
    ) -> int:
        """
        Export failed tasks to file.

        Args:
            output_file: Output file path
            format: Export format ('json', 'csv', 'jsonl')
            limit: Maximum tasks to export

        Returns:
            Number of tasks exported

        Example:
            exporter = DLQExporter()
            count = exporter.export_failed_tasks(
                "failed_tasks.json",
                format="json"
            )
        """
        tasks = self.dlq.list_failed_tasks(limit=limit)

        if format == "json":
            with open(output_file, 'w') as f:
                json.dump(tasks, f, indent=2, default=str)

        elif format == "jsonl":
            with open(output_file, 'w') as f:
                for task in tasks:
                    f.write(json.dumps(task, default=str) + "\n")

        elif format == "csv":
            if not tasks:
                return 0

            fields = ["task_id", "task_name", "error", "failed_at", "retries"]

            with open(output_file, 'w', newline='') as f:
                writer = csv.DictWriter(f, fieldnames=fields)
                writer.writeheader()

                for task in tasks:
                    row = {field: task.get(field) for field in fields}
                    writer.writerow(row)

        logger.info(f"Exported {len(tasks)} failed tasks to {output_file}")
        return len(tasks)

    def export_for_replay(
        self,
        output_file: str,
        include_failed_args: bool = True,
    ) -> int:
        """
        Export failed tasks in format suitable for replay.

        Args:
            output_file: Output file path (Python script)
            include_failed_args: Include original arguments

        Returns:
            Number of tasks exported

        Example:
            count = exporter.export_for_replay("replay_tasks.py")

            # Then run: python replay_tasks.py
        """
        tasks = self.dlq.list_failed_tasks()

        lines = [
            "#!/usr/bin/env python",
            '"""Auto-generated task replay script."""',
            "",
            "from turbine import Turbine",
            "",
            'app = Turbine(server="localhost:50051")',
            "",
            "# Replay failed tasks",
            "task_ids = []",
            "",
        ]

        for task in tasks:
            task_name = task["task_name"]
            args = task["args"] if include_failed_args else []
            kwargs = task["kwargs"] if include_failed_args else {}

            lines.append(f"# Task: {task['task_id']}")
            lines.append(f"# Error: {task['error'][:100]}")
            lines.append(f"result = app.client.submit_task(")
            lines.append(f"    name='{task_name}',")
            lines.append(f"    args={args},")
            lines.append(f"    kwargs={kwargs},")
            lines.append(f")")
            lines.append(f"task_ids.append(result)")
            lines.append("")

        lines.append('print(f"Replayed {len(task_ids)} tasks")')

        with open(output_file, 'w') as f:
            f.write("\n".join(lines))

        # Make executable
        import os
        os.chmod(output_file, 0o755)

        logger.info(f"Generated replay script: {output_file}")
        return len(tasks)


def export_queue_stats(
    turbine_app: Any,
    output_file: str,
    format: str = "json",
) -> int:
    """
    Export queue statistics.

    Args:
        turbine_app: Turbine application
        output_file: Output file path
        format: Export format

    Returns:
        Number of queues exported

    Example:
        from turbine import Turbine
        from turbine.export import export_queue_stats

        app = Turbine(server="localhost:50051")
        count = export_queue_stats(app, "queue_stats.json")
    """
    queues = turbine_app.client.get_queue_info()

    if format == "json":
        with open(output_file, 'w') as f:
            json.dump(queues, f, indent=2)

    elif format == "csv":
        fields = ["name", "pending", "processing", "consumers", "throughput"]

        with open(output_file, 'w', newline='') as f:
            writer = csv.DictWriter(f, fieldnames=fields)
            writer.writeheader()
            writer.writerows(queues)

    logger.info(f"Exported {len(queues)} queue stats to {output_file}")
    return len(queues)


class MetricsExporter:
    """Export Prometheus metrics to file."""

    def __init__(self, prometheus_url: str = "http://localhost:9090"):
        """
        Initialize metrics exporter.

        Args:
            prometheus_url: Prometheus metrics endpoint
        """
        self.prometheus_url = prometheus_url

    def export_metrics(
        self,
        output_file: str,
        metrics: list[str] | None = None,
    ) -> bool:
        """
        Export Prometheus metrics to file.

        Args:
            output_file: Output file path
            metrics: List of metric names (None = all)

        Returns:
            True if exported successfully

        Example:
            exporter = MetricsExporter()
            exporter.export_metrics(
                "metrics.txt",
                metrics=["turbine_tasks_total", "turbine_queue_pending_tasks"]
            )
        """
        try:
            import requests

            response = requests.get(
                f"{self.prometheus_url}/metrics",
                timeout=10
            )

            response.raise_for_status()

            metrics_data = response.text

            # Filter metrics if specified
            if metrics:
                lines = metrics_data.split("\n")
                filtered_lines = []

                for line in lines:
                    # Keep comments and matching metrics
                    if line.startswith("#") or any(m in line for m in metrics):
                        filtered_lines.append(line)

                metrics_data = "\n".join(filtered_lines)

            with open(output_file, 'w') as f:
                f.write(metrics_data)

            logger.info(f"Exported metrics to {output_file}")
            return True

        except Exception as e:
            logger.error(f"Failed to export metrics: {e}")
            return False
