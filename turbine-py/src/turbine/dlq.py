"""Dead Letter Queue (DLQ) management utilities."""

import logging
from typing import Any
from datetime import datetime
import msgpack
import redis

logger = logging.getLogger(__name__)


class DLQManager:
    """Manager for inspecting and replaying tasks from the Dead Letter Queue."""

    def __init__(self, backend_url: str = "redis://localhost:6379"):
        """
        Initialize DLQ manager.

        Args:
            backend_url: Redis URL for the backend
        """
        self.backend_url = backend_url
        self.conn = redis.from_url(backend_url, decode_responses=False)

    def list_failed_tasks(
        self,
        limit: int = 100,
        offset: int = 0,
    ) -> list[dict[str, Any]]:
        """
        List failed tasks in the DLQ.

        Args:
            limit: Maximum number of tasks to return
            offset: Offset for pagination

        Returns:
            List of failed task entries
        """
        try:
            # Get task IDs from the sorted set index
            task_ids = self.conn.zrevrange(
                "turbine:dlq:index",
                offset,
                offset + limit - 1
            )

            tasks = []
            for task_id_bytes in task_ids:
                task_id = task_id_bytes.decode('utf-8')
                task_data = self.get_failed_task(task_id)
                if task_data:
                    tasks.append(task_data)

            return tasks

        except Exception as e:
            logger.error(f"Failed to list DLQ tasks: {e}")
            return []

    def get_failed_task(self, task_id: str) -> dict[str, Any] | None:
        """
        Get a specific failed task from the DLQ.

        Args:
            task_id: Task ID

        Returns:
            Task data or None if not found
        """
        try:
            dlq_key = f"turbine:dlq:{task_id}"
            data = self.conn.get(dlq_key)

            if not data:
                return None

            task_data = msgpack.unpackb(data, raw=False)
            return task_data

        except Exception as e:
            logger.error(f"Failed to get DLQ task {task_id}: {e}")
            return None

    def count_failed_tasks(self) -> int:
        """
        Count total number of tasks in the DLQ.

        Returns:
            Number of failed tasks
        """
        try:
            return self.conn.zcard("turbine:dlq:index")
        except Exception as e:
            logger.error(f"Failed to count DLQ tasks: {e}")
            return 0

    def remove_failed_task(self, task_id: str) -> bool:
        """
        Remove a task from the DLQ.

        Args:
            task_id: Task ID to remove

        Returns:
            True if removed, False otherwise
        """
        try:
            dlq_key = f"turbine:dlq:{task_id}"

            # Remove from storage
            self.conn.delete(dlq_key)

            # Remove from index
            self.conn.zrem("turbine:dlq:index", task_id)

            logger.info(f"Removed task {task_id} from DLQ")
            return True

        except Exception as e:
            logger.error(f"Failed to remove task {task_id} from DLQ: {e}")
            return False

    def clear_dlq(self) -> int:
        """
        Clear all tasks from the DLQ.

        Returns:
            Number of tasks cleared
        """
        try:
            # Get all task IDs
            task_ids = self.conn.zrange("turbine:dlq:index", 0, -1)

            count = 0
            for task_id_bytes in task_ids:
                task_id = task_id_bytes.decode('utf-8')
                dlq_key = f"turbine:dlq:{task_id}"
                self.conn.delete(dlq_key)
                count += 1

            # Clear the index
            self.conn.delete("turbine:dlq:index")

            logger.info(f"Cleared {count} tasks from DLQ")
            return count

        except Exception as e:
            logger.error(f"Failed to clear DLQ: {e}")
            return 0

    def replay_task(self, task_id: str, turbine_client) -> str | None:
        """
        Replay a failed task by resubmitting it.

        Args:
            task_id: Task ID to replay
            turbine_client: Turbine client instance for submission

        Returns:
            New task ID if successful, None otherwise
        """
        try:
            task_data = self.get_failed_task(task_id)

            if not task_data:
                logger.error(f"Task {task_id} not found in DLQ")
                return None

            # Submit the task again
            new_task_id = turbine_client.client.submit_task(
                name=task_data["task_name"],
                args=task_data["args"],
                kwargs=task_data["kwargs"],
            )

            logger.info(
                f"Replayed task {task_id} as {new_task_id}"
            )

            # Optionally remove from DLQ after successful replay
            self.remove_failed_task(task_id)

            return new_task_id

        except Exception as e:
            logger.error(f"Failed to replay task {task_id}: {e}")
            return None

    def get_dlq_stats(self) -> dict[str, Any]:
        """
        Get statistics about the DLQ.

        Returns:
            Dictionary with DLQ statistics
        """
        try:
            total = self.count_failed_tasks()

            # Get oldest and newest task timestamps
            oldest = None
            newest = None

            if total > 0:
                oldest_task = self.conn.zrange("turbine:dlq:index", 0, 0, withscores=True)
                newest_task = self.conn.zrevrange("turbine:dlq:index", 0, 0, withscores=True)

                if oldest_task:
                    oldest = datetime.fromtimestamp(oldest_task[0][1]).isoformat()
                if newest_task:
                    newest = datetime.fromtimestamp(newest_task[0][1]).isoformat()

            return {
                "total_failed_tasks": total,
                "oldest_failure": oldest,
                "newest_failure": newest,
            }

        except Exception as e:
            logger.error(f"Failed to get DLQ stats: {e}")
            return {
                "total_failed_tasks": 0,
                "oldest_failure": None,
                "newest_failure": None,
            }
