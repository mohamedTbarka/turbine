"""Task routing and load balancing strategies."""

import logging
import hashlib
from typing import Any, Callable
from enum import Enum

logger = logging.getLogger(__name__)


class RoutingStrategy(str, Enum):
    """Task routing strategies."""

    ROUND_ROBIN = "round_robin"
    LEAST_LOADED = "least_loaded"
    HASH = "hash"
    RANDOM = "random"
    PRIORITY = "priority"


class TaskRouter:
    """
    Routes tasks to appropriate queues based on various strategies.

    Example:
        router = TaskRouter(strategy=RoutingStrategy.HASH)

        # Route based on user_id
        queue = router.route(user_id=12345)
        task.apply_async(args=[data], queue=queue)
    """

    def __init__(
        self,
        queues: list[str] | None = None,
        strategy: RoutingStrategy = RoutingStrategy.ROUND_ROBIN,
        hash_key: Callable[[dict], str] | None = None,
    ):
        """
        Initialize task router.

        Args:
            queues: List of available queues
            strategy: Routing strategy to use
            hash_key: Function to extract hash key from task args (for HASH strategy)
        """
        self.queues = queues or ["default"]
        self.strategy = strategy
        self.hash_key = hash_key or (lambda x: str(x))
        self._round_robin_index = 0

    def route(self, **context: Any) -> str:
        """
        Route to a queue based on strategy and context.

        Args:
            **context: Routing context (user_id, tenant_id, etc.)

        Returns:
            Queue name

        Example:
            queue = router.route(user_id=123, tenant_id="acme")
        """
        if self.strategy == RoutingStrategy.ROUND_ROBIN:
            return self._round_robin()

        elif self.strategy == RoutingStrategy.HASH:
            return self._hash_route(context)

        elif self.strategy == RoutingStrategy.RANDOM:
            return self._random_route()

        elif self.strategy == RoutingStrategy.PRIORITY:
            return self._priority_route(context)

        else:
            return self.queues[0]

    def _round_robin(self) -> str:
        """Round-robin routing."""
        queue = self.queues[self._round_robin_index % len(self.queues)]
        self._round_robin_index += 1
        return queue

    def _hash_route(self, context: dict[str, Any]) -> str:
        """Hash-based routing for consistent assignment."""
        key = self.hash_key(context)
        hash_value = int(hashlib.md5(str(key).encode()).hexdigest(), 16)
        index = hash_value % len(self.queues)
        return self.queues[index]

    def _random_route(self) -> str:
        """Random routing."""
        import random
        return random.choice(self.queues)

    def _priority_route(self, context: dict[str, Any]) -> str:
        """Route based on priority in context."""
        priority = context.get("priority", 0)

        # Map priority to queue (assumes queues are ordered by priority)
        if priority >= 10:
            return self.queues[0] if len(self.queues) > 0 else "default"
        elif priority >= 5:
            return self.queues[1] if len(self.queues) > 1 else "default"
        else:
            return self.queues[-1] if self.queues else "default"


class LoadBalancer:
    """
    Balances task load across multiple queues or workers.

    Monitors queue depths and routes tasks to least loaded queues.
    """

    def __init__(
        self,
        turbine_client: Any,
        queues: list[str] | None = None,
        refresh_interval: int = 5,
    ):
        """
        Initialize load balancer.

        Args:
            turbine_client: Turbine client for queue stats
            queues: List of queues to balance across
            refresh_interval: How often to refresh stats (seconds)
        """
        self.client = turbine_client
        self.queues = queues or ["default"]
        self.refresh_interval = refresh_interval
        self._queue_stats: dict[str, dict] = {}
        self._last_refresh = 0

    def _refresh_stats(self) -> None:
        """Refresh queue statistics from server."""
        import time

        now = time.time()

        if now - self._last_refresh < self.refresh_interval:
            return

        try:
            for queue in self.queues:
                stats = self.client.get_queue_info(queue)
                if stats:
                    self._queue_stats[queue] = stats[0]

            self._last_refresh = now

        except Exception as e:
            logger.error(f"Failed to refresh queue stats: {e}")

    def get_least_loaded_queue(self) -> str:
        """
        Get the queue with the least pending tasks.

        Returns:
            Queue name
        """
        self._refresh_stats()

        if not self._queue_stats:
            return self.queues[0]

        # Find queue with minimum pending tasks
        least_loaded = min(
            self._queue_stats.items(),
            key=lambda x: x[1].get('pending', 0)
        )

        return least_loaded[0]

    def get_queue_by_throughput(self) -> str:
        """
        Get the queue with highest throughput capacity.

        Returns:
            Queue name
        """
        self._refresh_stats()

        if not self._queue_stats:
            return self.queues[0]

        # Find queue with highest throughput
        highest_throughput = max(
            self._queue_stats.items(),
            key=lambda x: x[1].get('throughput', 0)
        )

        return highest_throughput[0]

    def route_task(
        self,
        task: Any,
        args: tuple | None = None,
        kwargs: dict | None = None,
        strategy: str = "least_loaded",
    ) -> AsyncResult:
        """
        Route and submit task based on load balancing strategy.

        Args:
            task: Turbine task
            args: Task arguments
            kwargs: Task keyword arguments
            strategy: Routing strategy ('least_loaded', 'throughput', 'round_robin')

        Returns:
            AsyncResult

        Example:
            balancer = LoadBalancer(turbine_client, queues=['q1', 'q2', 'q3'])
            result = balancer.route_task(my_task, args=[data])
        """
        if strategy == "least_loaded":
            queue = self.get_least_loaded_queue()
        elif strategy == "throughput":
            queue = self.get_queue_by_throughput()
        else:
            # Default to first queue
            queue = self.queues[0]

        logger.debug(f"Routing task to queue '{queue}' (strategy: {strategy})")

        return task.apply_async(
            args=args or (),
            kwargs=kwargs or {},
            queue=queue
        )

    def get_stats(self) -> dict[str, dict]:
        """
        Get current queue statistics.

        Returns:
            Dictionary mapping queue names to stats
        """
        self._refresh_stats()
        return self._queue_stats.copy()


class RateLimiter:
    """
    Rate limiter for task submission.

    Prevents overwhelming the system with too many tasks.
    """

    def __init__(
        self,
        redis_url: str = "redis://localhost:6379",
        max_rate: int = 100,
        period: int = 60,
    ):
        """
        Initialize rate limiter.

        Args:
            redis_url: Redis URL for rate limit tracking
            max_rate: Maximum number of operations
            period: Time period in seconds
        """
        import redis

        self.conn = redis.from_url(redis_url, decode_responses=True)
        self.max_rate = max_rate
        self.period = period

    def check_rate_limit(
        self,
        key: str,
        cost: int = 1,
    ) -> tuple[bool, int]:
        """
        Check if operation is within rate limit.

        Args:
            key: Rate limit key (e.g., user_id, tenant_id)
            cost: Cost of this operation

        Returns:
            Tuple of (allowed, remaining)

        Example:
            limiter = RateLimiter(max_rate=100, period=60)
            allowed, remaining = limiter.check_rate_limit(f"user:{user_id}")

            if not allowed:
                raise Exception(f"Rate limit exceeded. Try again later.")
        """
        import time

        redis_key = f"turbine:ratelimit:{key}"
        now = int(time.time())

        # Use Redis sorted set for sliding window
        pipe = self.conn.pipeline()

        # Remove old entries
        pipe.zremrangebyscore(redis_key, 0, now - self.period)

        # Count current entries
        pipe.zcard(redis_key)

        # Add new entry
        pipe.zadd(redis_key, {f"{now}:{id(self)}": now})

        # Set expiration
        pipe.expire(redis_key, self.period)

        results = pipe.execute()

        current_count = results[1]  # Count before adding

        if current_count + cost > self.max_rate:
            # Remove the entry we just added
            self.conn.zremrangebyscore(redis_key, now, now)
            return False, 0

        remaining = self.max_rate - current_count - cost
        return True, max(0, remaining)

    def wait_if_needed(self, key: str, cost: int = 1) -> None:
        """
        Wait if rate limit is exceeded.

        Args:
            key: Rate limit key
            cost: Cost of operation
        """
        import time

        while True:
            allowed, remaining = self.check_rate_limit(key, cost)

            if allowed:
                return

            # Wait before retrying
            wait_time = min(self.period / self.max_rate, 1.0)
            logger.debug(f"Rate limit exceeded for {key}, waiting {wait_time}s")
            time.sleep(wait_time)


def consistent_hash_router(
    task: Any,
    partition_key: str,
    num_queues: int = 4,
    queue_prefix: str = "partition",
) -> str:
    """
    Route task to queue using consistent hashing.

    Useful for partitioning work by user_id, tenant_id, etc.

    Args:
        task: Task to route
        partition_key: Key to hash (e.g., user_id)
        num_queues: Number of partition queues
        queue_prefix: Queue name prefix

    Returns:
        Queue name

    Example:
        # Route all tasks for same user to same queue
        queue = consistent_hash_router(
            my_task,
            partition_key=f"user:{user_id}",
            num_queues=8
        )

        my_task.apply_async(args=[data], queue=queue)
    """
    hash_value = int(hashlib.md5(partition_key.encode()).hexdigest(), 16)
    partition = hash_value % num_queues
    return f"{queue_prefix}-{partition}"
