"""Result caching utilities for Turbine tasks."""

import logging
import hashlib
import json
from typing import Any, Callable
from functools import wraps
import redis

logger = logging.getLogger(__name__)


class ResultCache:
    """
    Cache layer for task results to avoid recomputation.

    Uses Redis for distributed caching across workers.
    """

    def __init__(
        self,
        redis_url: str = "redis://localhost:6379",
        default_ttl: int = 3600,
        key_prefix: str = "turbine:cache",
    ):
        """
        Initialize result cache.

        Args:
            redis_url: Redis connection URL
            default_ttl: Default cache TTL in seconds
            key_prefix: Prefix for cache keys
        """
        self.conn = redis.from_url(redis_url, decode_responses=False)
        self.default_ttl = default_ttl
        self.key_prefix = key_prefix

    def _make_key(self, task_name: str, args: tuple, kwargs: dict) -> str:
        """
        Generate cache key from task name and arguments.

        Args:
            task_name: Task name
            args: Task arguments
            kwargs: Task keyword arguments

        Returns:
            Cache key string
        """
        # Create deterministic hash of arguments
        arg_str = json.dumps({"args": args, "kwargs": kwargs}, sort_keys=True)
        arg_hash = hashlib.md5(arg_str.encode()).hexdigest()

        return f"{self.key_prefix}:{task_name}:{arg_hash}"

    def get(
        self,
        task_name: str,
        args: tuple | None = None,
        kwargs: dict | None = None,
    ) -> Any | None:
        """
        Get cached result.

        Args:
            task_name: Task name
            args: Task arguments
            kwargs: Task keyword arguments

        Returns:
            Cached result or None
        """
        args = args or ()
        kwargs = kwargs or {}

        try:
            import msgpack

            key = self._make_key(task_name, args, kwargs)
            data = self.conn.get(key)

            if data:
                result = msgpack.unpackb(data, raw=False)
                logger.debug(f"Cache hit for {task_name}")
                return result

            logger.debug(f"Cache miss for {task_name}")
            return None

        except Exception as e:
            logger.error(f"Failed to get from cache: {e}")
            return None

    def set(
        self,
        task_name: str,
        args: tuple | None,
        kwargs: dict | None,
        result: Any,
        ttl: int | None = None,
    ) -> bool:
        """
        Store result in cache.

        Args:
            task_name: Task name
            args: Task arguments
            kwargs: Task keyword arguments
            result: Result to cache
            ttl: Cache TTL (uses default if None)

        Returns:
            True if stored successfully
        """
        args = args or ()
        kwargs = kwargs or {}
        ttl = ttl or self.default_ttl

        try:
            import msgpack

            key = self._make_key(task_name, args, kwargs)
            data = msgpack.packb(result, use_bin_type=True)

            self.conn.set(key, data, ex=ttl)
            logger.debug(f"Cached result for {task_name} (TTL: {ttl}s)")
            return True

        except Exception as e:
            logger.error(f"Failed to cache result: {e}")
            return False

    def delete(
        self,
        task_name: str,
        args: tuple | None = None,
        kwargs: dict | None = None,
    ) -> bool:
        """
        Delete cached result.

        Args:
            task_name: Task name
            args: Task arguments
            kwargs: Task keyword arguments

        Returns:
            True if deleted
        """
        args = args or ()
        kwargs = kwargs or {}

        try:
            key = self._make_key(task_name, args, kwargs)
            deleted = self.conn.delete(key) > 0
            return deleted

        except Exception as e:
            logger.error(f"Failed to delete from cache: {e}")
            return False

    def clear(self, pattern: str = "*") -> int:
        """
        Clear cached results matching pattern.

        Args:
            pattern: Key pattern (e.g., "*", "task_name:*")

        Returns:
            Number of keys deleted
        """
        try:
            full_pattern = f"{self.key_prefix}:{pattern}"
            keys = self.conn.keys(full_pattern)

            if keys:
                return self.conn.delete(*keys)

            return 0

        except Exception as e:
            logger.error(f"Failed to clear cache: {e}")
            return 0


def cached_task(
    ttl: int = 3600,
    cache_key: Callable[..., str] | None = None,
    redis_url: str = "redis://localhost:6379",
):
    """
    Decorator to cache task results.

    Args:
        ttl: Cache TTL in seconds
        cache_key: Custom cache key function (args, kwargs) -> key
        redis_url: Redis URL for caching

    Example:
        @task
        @cached_task(ttl=7200)  # Cache for 2 hours
        def expensive_task(x, y):
            # Expensive computation
            return result

        # First call executes task
        result1 = expensive_task.delay(1, 2).get()

        # Second call returns cached result (instant)
        result2 = expensive_task.delay(1, 2).get()
    """
    cache = ResultCache(redis_url=redis_url, default_ttl=ttl)

    def decorator(task_func):
        original_func = task_func.func if hasattr(task_func, 'func') else task_func

        @wraps(original_func)
        def wrapper(*args, **kwargs):
            # Generate cache key
            if cache_key:
                key = cache_key(*args, **kwargs)
            else:
                task_name = getattr(task_func, 'name', original_func.__name__)
                key = task_name

            # Check cache
            cached_result = cache.get(key, args, kwargs)

            if cached_result is not None:
                logger.info(f"Returning cached result for {key}")
                return cached_result

            # Execute task
            result = original_func(*args, **kwargs)

            # Cache result
            cache.set(key, args, kwargs, result, ttl)

            return result

        # Replace function if it's a Task
        if hasattr(task_func, 'func'):
            task_func.func = wrapper

        return task_func

    return decorator


class MemoizedTask:
    """
    Memoization for tasks - cache results indefinitely.

    Useful for pure functions with deterministic outputs.
    """

    def __init__(self, redis_url: str = "redis://localhost:6379"):
        """Initialize memoized task cache."""
        self.cache = ResultCache(
            redis_url=redis_url,
            default_ttl=0,  # No expiration
            key_prefix="turbine:memo"
        )

    def __call__(self, task_func):
        """Decorate task with memoization."""
        original_func = task_func.func if hasattr(task_func, 'func') else task_func

        @wraps(original_func)
        def wrapper(*args, **kwargs):
            task_name = getattr(task_func, 'name', original_func.__name__)

            # Check memo cache
            cached = self.cache.get(task_name, args, kwargs)

            if cached is not None:
                logger.debug(f"Memoized result for {task_name}")
                return cached

            # Compute
            result = original_func(*args, **kwargs)

            # Memo (no TTL)
            self.cache.conn.set(
                self.cache._make_key(task_name, args, kwargs),
                json.dumps(result).encode()
            )

            return result

        if hasattr(task_func, 'func'):
            task_func.func = wrapper

        return task_func

    def clear(self, task_name: str | None = None) -> int:
        """
        Clear memoized results.

        Args:
            task_name: Specific task to clear (None = all)

        Returns:
            Number of entries cleared
        """
        pattern = f"{task_name}:*" if task_name else "*"
        return self.cache.clear(pattern)


def invalidate_cache(task_name: str, *args, **kwargs) -> bool:
    """
    Invalidate cached result for specific task arguments.

    Args:
        task_name: Task name
        *args: Task arguments
        **kwargs: Task keyword arguments

    Returns:
        True if cache was invalidated

    Example:
        # Invalidate specific cached result
        invalidate_cache("expensive_task", 1, 2)

        # Re-run will compute fresh result
        result = expensive_task.delay(1, 2).get()
    """
    cache = ResultCache()
    return cache.delete(task_name, args, kwargs)
