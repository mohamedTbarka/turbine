"""Advanced retry utilities and strategies."""

import logging
import time
import random
from typing import Callable, Any, TypeVar
from functools import wraps
from enum import Enum

logger = logging.getLogger(__name__)

T = TypeVar("T")


class RetryStrategy(str, Enum):
    """Retry backoff strategies."""

    EXPONENTIAL = "exponential"
    LINEAR = "linear"
    CONSTANT = "constant"
    FIBONACCI = "fibonacci"
    JITTERED = "jittered"


class RetryPolicy:
    """Configurable retry policy."""

    def __init__(
        self,
        max_attempts: int = 3,
        base_delay: float = 1.0,
        max_delay: float = 300.0,
        strategy: RetryStrategy = RetryStrategy.EXPONENTIAL,
        jitter: bool = True,
        retry_on: tuple[type[Exception], ...] | None = None,
        stop_on: tuple[type[Exception], ...] | None = None,
    ):
        """
        Initialize retry policy.

        Args:
            max_attempts: Maximum number of retry attempts
            base_delay: Base delay in seconds
            max_delay: Maximum delay in seconds
            strategy: Backoff strategy
            jitter: Add random jitter to prevent thundering herd
            retry_on: Only retry on these exceptions (None = all)
            stop_on: Never retry on these exceptions

        Example:
            policy = RetryPolicy(
                max_attempts=5,
                base_delay=2.0,
                strategy=RetryStrategy.EXPONENTIAL,
                retry_on=(ConnectionError, TimeoutError),
                stop_on=(ValueError, KeyError),
            )
        """
        self.max_attempts = max_attempts
        self.base_delay = base_delay
        self.max_delay = max_delay
        self.strategy = strategy
        self.jitter = jitter
        self.retry_on = retry_on
        self.stop_on = stop_on

    def should_retry(self, exception: Exception, attempt: int) -> bool:
        """
        Check if task should be retried.

        Args:
            exception: Exception that was raised
            attempt: Current attempt number (1-indexed)

        Returns:
            True if should retry
        """
        # Check attempt limit
        if attempt >= self.max_attempts:
            return False

        # Check stop_on exceptions
        if self.stop_on and isinstance(exception, self.stop_on):
            return False

        # Check retry_on exceptions
        if self.retry_on and not isinstance(exception, self.retry_on):
            return False

        return True

    def get_delay(self, attempt: int) -> float:
        """
        Calculate delay for retry attempt.

        Args:
            attempt: Attempt number (1-indexed)

        Returns:
            Delay in seconds
        """
        if self.strategy == RetryStrategy.CONSTANT:
            delay = self.base_delay

        elif self.strategy == RetryStrategy.LINEAR:
            delay = self.base_delay * attempt

        elif self.strategy == RetryStrategy.EXPONENTIAL:
            delay = self.base_delay * (2 ** (attempt - 1))

        elif self.strategy == RetryStrategy.FIBONACCI:
            delay = self.base_delay * self._fibonacci(attempt)

        elif self.strategy == RetryStrategy.JITTERED:
            delay = self.base_delay * (2 ** (attempt - 1))
            # Add jitter
            delay *= (0.5 + random.random())

        else:
            delay = self.base_delay

        # Add jitter if enabled
        if self.jitter and self.strategy != RetryStrategy.JITTERED:
            jitter_amount = delay * 0.1 * random.random()
            delay += jitter_amount

        # Cap at max delay
        return min(delay, self.max_delay)

    @staticmethod
    def _fibonacci(n: int) -> int:
        """Calculate nth Fibonacci number."""
        if n <= 1:
            return 1
        a, b = 1, 1
        for _ in range(n - 1):
            a, b = b, a + b
        return a


def retry(
    max_attempts: int = 3,
    base_delay: float = 1.0,
    strategy: RetryStrategy = RetryStrategy.EXPONENTIAL,
    retry_on: tuple[type[Exception], ...] | None = None,
    stop_on: tuple[type[Exception], ...] | None = None,
    on_retry: Callable[[Exception, int], None] | None = None,
) -> Callable:
    """
    Decorator for automatic retry logic.

    Args:
        max_attempts: Maximum retry attempts
        base_delay: Base delay between retries
        strategy: Backoff strategy
        retry_on: Only retry on these exceptions
        stop_on: Never retry on these exceptions
        on_retry: Callback on each retry (exception, attempt)

    Example:
        @retry(max_attempts=5, strategy=RetryStrategy.EXPONENTIAL)
        def flaky_function():
            response = requests.get("http://flaky-api.com")
            return response.json()

        @retry(
            max_attempts=3,
            retry_on=(ConnectionError, TimeoutError),
            on_retry=lambda exc, attempt: logger.warning(f"Retry {attempt}: {exc}")
        )
        def api_call():
            return call_external_api()
    """
    policy = RetryPolicy(
        max_attempts=max_attempts,
        base_delay=base_delay,
        strategy=strategy,
        retry_on=retry_on,
        stop_on=stop_on,
    )

    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> T:
            attempt = 0

            while True:
                attempt += 1

                try:
                    return func(*args, **kwargs)

                except Exception as e:
                    if not policy.should_retry(e, attempt):
                        # Max attempts or non-retriable error
                        logger.error(
                            f"Function {func.__name__} failed after {attempt} attempts: {e}"
                        )
                        raise

                    # Calculate delay
                    delay = policy.get_delay(attempt)

                    logger.warning(
                        f"Function {func.__name__} failed (attempt {attempt}/{max_attempts}), "
                        f"retrying in {delay:.2f}s: {e}"
                    )

                    # Callback
                    if on_retry:
                        on_retry(e, attempt)

                    # Wait before retry
                    time.sleep(delay)

        return wrapper

    return decorator


class CircuitBreaker:
    """
    Circuit breaker pattern for preventing cascade failures.

    States:
    - CLOSED: Normal operation, requests pass through
    - OPEN: Failures exceeded threshold, reject all requests
    - HALF_OPEN: Testing if service recovered

    Example:
        breaker = CircuitBreaker(
            failure_threshold=5,
            timeout=60,
        )

        @breaker.call
        def call_external_service():
            return requests.get("http://external-api.com")
    """

    def __init__(
        self,
        failure_threshold: int = 5,
        timeout: float = 60.0,
        expected_exception: type[Exception] = Exception,
    ):
        """
        Initialize circuit breaker.

        Args:
            failure_threshold: Number of failures before opening
            timeout: Seconds to wait before attempting recovery
            expected_exception: Exception type that triggers circuit
        """
        self.failure_threshold = failure_threshold
        self.timeout = timeout
        self.expected_exception = expected_exception

        self._failure_count = 0
        self._last_failure_time: float | None = None
        self._state = "CLOSED"

    @property
    def state(self) -> str:
        """Get current circuit breaker state."""
        return self._state

    def call(self, func: Callable[..., T]) -> Callable[..., T]:
        """Decorate function with circuit breaker."""

        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> T:
            # Check circuit state
            if self._state == "OPEN":
                # Check if timeout elapsed
                if self._last_failure_time:
                    elapsed = time.time() - self._last_failure_time

                    if elapsed >= self.timeout:
                        # Try to recover
                        self._state = "HALF_OPEN"
                        logger.info(f"Circuit breaker for {func.__name__} entering HALF_OPEN")
                    else:
                        raise Exception(
                            f"Circuit breaker OPEN for {func.__name__} "
                            f"(retry in {self.timeout - elapsed:.1f}s)"
                        )

            try:
                result = func(*args, **kwargs)

                # Success - close circuit
                if self._state == "HALF_OPEN":
                    self._state = "CLOSED"
                    self._failure_count = 0
                    logger.info(f"Circuit breaker for {func.__name__} closed (recovered)")

                return result

            except self.expected_exception as e:
                self._failure_count += 1
                self._last_failure_time = time.time()

                logger.warning(
                    f"Circuit breaker failure {self._failure_count}/{self.failure_threshold} "
                    f"for {func.__name__}: {e}"
                )

                # Check if threshold exceeded
                if self._failure_count >= self.failure_threshold:
                    self._state = "OPEN"
                    logger.error(
                        f"Circuit breaker OPEN for {func.__name__} "
                        f"after {self._failure_count} failures"
                    )

                raise

        return wrapper

    def reset(self) -> None:
        """Manually reset circuit breaker."""
        self._state = "CLOSED"
        self._failure_count = 0
        self._last_failure_time = None
        logger.info("Circuit breaker manually reset")


def exponential_backoff(
    attempt: int,
    base_delay: float = 1.0,
    max_delay: float = 60.0,
    jitter: bool = True,
) -> float:
    """
    Calculate exponential backoff delay.

    Args:
        attempt: Retry attempt number (1-indexed)
        base_delay: Base delay in seconds
        max_delay: Maximum delay in seconds
        jitter: Add random jitter

    Returns:
        Delay in seconds

    Example:
        for attempt in range(1, 6):
            delay = exponential_backoff(attempt)
            print(f"Attempt {attempt}: wait {delay:.2f}s")
            time.sleep(delay)
    """
    delay = base_delay * (2 ** (attempt - 1))

    if jitter:
        jitter_amount = delay * 0.1 * random.random()
        delay += jitter_amount

    return min(delay, max_delay)


def retry_with_timeout(
    func: Callable[..., T],
    timeout: float,
    retry_interval: float = 1.0,
    *args: Any,
    **kwargs: Any,
) -> T:
    """
    Retry function until it succeeds or timeout is reached.

    Args:
        func: Function to retry
        timeout: Total timeout in seconds
        retry_interval: Interval between retries
        *args: Function arguments
        **kwargs: Function keyword arguments

    Returns:
        Function result

    Raises:
        TimeoutError: If timeout is reached
        Exception: Last exception from function

    Example:
        result = retry_with_timeout(
            check_status,
            timeout=60,
            retry_interval=2,
            task_id="abc-123"
        )
    """
    start_time = time.time()
    last_exception = None

    while time.time() - start_time < timeout:
        try:
            return func(*args, **kwargs)
        except Exception as e:
            last_exception = e
            elapsed = time.time() - start_time
            remaining = timeout - elapsed

            if remaining > retry_interval:
                time.sleep(retry_interval)
            else:
                break

    # Timeout reached
    if last_exception:
        raise last_exception
    else:
        raise TimeoutError(f"Function {func.__name__} timed out after {timeout}s")


class RetryableTask:
    """
    Wrapper for tasks with advanced retry capabilities.

    Example:
        @task
        def my_task():
            pass

        retriable = RetryableTask(my_task, max_retries=5)

        # Retry with custom policy
        result = retriable.execute_with_retry(
            args=[data],
            strategy=RetryStrategy.FIBONACCI
        )
    """

    def __init__(
        self,
        task: Any,
        max_retries: int = 3,
        base_delay: float = 1.0,
        strategy: RetryStrategy = RetryStrategy.EXPONENTIAL,
    ):
        """
        Initialize retriable task.

        Args:
            task: Turbine task
            max_retries: Maximum retry attempts
            base_delay: Base delay between retries
            strategy: Backoff strategy
        """
        self.task = task
        self.policy = RetryPolicy(
            max_attempts=max_retries,
            base_delay=base_delay,
            strategy=strategy,
        )

    def execute_with_retry(
        self,
        args: list | None = None,
        kwargs: dict | None = None,
        wait: bool = True,
        timeout: float = 300.0,
    ) -> Any:
        """
        Execute task with automatic retry on failure.

        Args:
            args: Task arguments
            kwargs: Task keyword arguments
            wait: Wait for result
            timeout: Timeout for waiting

        Returns:
            Task result if wait=True, else AsyncResult

        Example:
            retriable = RetryableTask(api_task, max_retries=5)
            result = retriable.execute_with_retry(
                args=["endpoint"],
                wait=True
            )
        """
        args = args or []
        kwargs = kwargs or {}
        attempt = 0

        while attempt < self.policy.max_attempts:
            attempt += 1

            try:
                # Submit task
                result = self.task.apply_async(args=args, kwargs=kwargs)

                if not wait:
                    return result

                # Wait for result
                value = result.get(timeout=timeout)
                return value

            except Exception as e:
                if not self.policy.should_retry(e, attempt):
                    logger.error(f"Task {self.task.name} failed permanently: {e}")
                    raise

                # Calculate delay
                delay = self.policy.get_delay(attempt)

                logger.warning(
                    f"Task {self.task.name} failed (attempt {attempt}/{self.policy.max_attempts}), "
                    f"retrying in {delay:.2f}s: {e}"
                )

                time.sleep(delay)

        raise Exception(f"Task {self.task.name} failed after {attempt} attempts")


def wait_for_condition(
    condition: Callable[[], bool],
    timeout: float = 60.0,
    interval: float = 1.0,
    timeout_message: str = "Condition not met",
) -> bool:
    """
    Wait for a condition to become true.

    Args:
        condition: Function that returns boolean
        timeout: Maximum wait time in seconds
        interval: Check interval in seconds
        timeout_message: Message for TimeoutError

    Returns:
        True if condition met

    Raises:
        TimeoutError: If timeout reached

    Example:
        # Wait for task to complete
        def is_complete():
            status = app.get_task_status(task_id)
            return status['state'] in ['SUCCESS', 'FAILURE']

        wait_for_condition(is_complete, timeout=60)
    """
    start_time = time.time()

    while time.time() - start_time < timeout:
        if condition():
            return True

        time.sleep(interval)

    raise TimeoutError(timeout_message)


def retry_on_exception(
    exceptions: tuple[type[Exception], ...],
    max_attempts: int = 3,
    delay: float = 1.0,
) -> Callable:
    """
    Simple retry decorator for specific exceptions.

    Args:
        exceptions: Exceptions to retry on
        max_attempts: Maximum attempts
        delay: Delay between retries

    Example:
        @retry_on_exception((ConnectionError, TimeoutError), max_attempts=5)
        def call_api():
            return requests.get("http://api.example.com")
    """

    def decorator(func: Callable[..., T]) -> Callable[..., T]:
        @wraps(func)
        def wrapper(*args: Any, **kwargs: Any) -> T:
            for attempt in range(1, max_attempts + 1):
                try:
                    return func(*args, **kwargs)
                except exceptions as e:
                    if attempt == max_attempts:
                        raise

                    logger.warning(
                        f"{func.__name__} failed (attempt {attempt}/{max_attempts}), "
                        f"retrying: {e}"
                    )
                    time.sleep(delay * attempt)

            raise Exception("Should not reach here")

        return wrapper

    return decorator


class RetryStats:
    """Track retry statistics for monitoring."""

    def __init__(self):
        """Initialize retry statistics."""
        self.total_attempts = 0
        self.total_retries = 0
        self.total_failures = 0
        self.retry_counts: dict[int, int] = {}  # attempt -> count

    def record_attempt(self, attempt: int, success: bool) -> None:
        """
        Record a retry attempt.

        Args:
            attempt: Attempt number
            success: Whether attempt succeeded
        """
        self.total_attempts += 1

        if attempt > 1:
            self.total_retries += 1

        if success:
            self.retry_counts[attempt] = self.retry_counts.get(attempt, 0) + 1
        else:
            self.total_failures += 1

    def get_stats(self) -> dict[str, Any]:
        """
        Get retry statistics.

        Returns:
            Dictionary of statistics
        """
        if self.total_attempts == 0:
            return {
                "total_attempts": 0,
                "total_retries": 0,
                "total_failures": 0,
                "retry_rate": 0.0,
                "success_rate": 0.0,
            }

        return {
            "total_attempts": self.total_attempts,
            "total_retries": self.total_retries,
            "total_failures": self.total_failures,
            "retry_rate": self.total_retries / self.total_attempts,
            "success_rate": (self.total_attempts - self.total_failures) / self.total_attempts,
            "retry_distribution": self.retry_counts,
        }

    def reset(self) -> None:
        """Reset statistics."""
        self.total_attempts = 0
        self.total_retries = 0
        self.total_failures = 0
        self.retry_counts.clear()
