"""Batch processing utilities for Turbine tasks."""

import logging
from typing import Any, Callable, Iterable, TypeVar
from itertools import islice

from turbine.result import AsyncResult

logger = logging.getLogger(__name__)

T = TypeVar("T")


def chunks(iterable: Iterable[T], size: int) -> Iterable[list[T]]:
    """
    Split an iterable into chunks of specified size.

    Args:
        iterable: Input iterable
        size: Chunk size

    Yields:
        Lists of items with max length = size

    Example:
        >>> list(chunks([1, 2, 3, 4, 5], 2))
        [[1, 2], [3, 4], [5]]
    """
    iterator = iter(iterable)
    while chunk := list(islice(iterator, size)):
        yield chunk


class BatchProcessor:
    """
    Utility for efficient batch task processing.

    Handles:
    - Automatic chunking
    - Progress tracking
    - Error handling
    - Result aggregation
    """

    def __init__(
        self,
        task: Any,
        chunk_size: int = 100,
        max_concurrent: int = 10,
        on_progress: Callable[[int, int], None] | None = None,
        on_error: Callable[[Exception, Any], None] | None = None,
    ):
        """
        Initialize batch processor.

        Args:
            task: Turbine task to execute
            chunk_size: Number of items per batch
            max_concurrent: Maximum concurrent task submissions
            on_progress: Callback for progress updates (processed, total)
            on_error: Callback for error handling (error, item)
        """
        self.task = task
        self.chunk_size = chunk_size
        self.max_concurrent = max_concurrent
        self.on_progress = on_progress
        self.on_error = on_error

    def map(
        self,
        items: list[Any],
        *,
        use_group: bool = True,
    ) -> list[AsyncResult]:
        """
        Map task over a list of items.

        Args:
            items: List of items to process
            use_group: Use workflow group for parallel execution

        Returns:
            List of AsyncResult objects

        Example:
            processor = BatchProcessor(process_item)
            results = processor.map([1, 2, 3, 4, 5])

            for result in results:
                print(result.get())
        """
        if not items:
            return []

        results = []

        if use_group:
            # Use Turbine group for efficient parallel execution
            from turbine import group

            # Process in chunks to avoid overwhelming the system
            for chunk in chunks(items, self.max_concurrent):
                signatures = [self.task.s(item) for item in chunk]
                workflow_id, task_ids = group(*signatures).delay()

                # Create AsyncResult for each task
                chunk_results = [
                    AsyncResult(task_id, self.task.app.client, task_name=self.task.name)
                    for task_id in task_ids
                ]
                results.extend(chunk_results)

                # Progress callback
                if self.on_progress:
                    self.on_progress(len(results), len(items))

        else:
            # Submit tasks individually
            for i, item in enumerate(items):
                try:
                    result = self.task.delay(item)
                    results.append(result)

                    if self.on_progress:
                        self.on_progress(i + 1, len(items))

                except Exception as e:
                    logger.error(f"Failed to submit task for item {item}: {e}")
                    if self.on_error:
                        self.on_error(e, item)

        return results

    def map_reduce(
        self,
        items: list[Any],
        reducer: Callable[[list[Any]], Any],
        *,
        use_chord: bool = True,
    ) -> AsyncResult:
        """
        Map task over items and reduce results with a callback.

        Args:
            items: List of items to process
            reducer: Function to aggregate results
            use_chord: Use workflow chord

        Returns:
            AsyncResult for the final result

        Example:
            def sum_reducer(results):
                return sum(results)

            processor = BatchProcessor(square)
            result = processor.map_reduce([1, 2, 3, 4], sum_reducer)
            print(result.get())  # 30 (1+4+9+16)
        """
        if not items:
            raise ValueError("Cannot map_reduce over empty list")

        if use_chord:
            from turbine import chord

            # Create group of map tasks
            map_signatures = [self.task.s(item) for item in items]

            # Create reducer task signature
            # Note: This assumes reducer is also a Turbine task
            if hasattr(reducer, 's'):
                reducer_sig = reducer.s()
            else:
                raise ValueError("Reducer must be a Turbine task for chord")

            # Execute chord
            workflow_id, task_ids = chord(map_signatures, reducer_sig).delay()

            # Return result of the callback (last task)
            callback_id = task_ids[-1]
            return AsyncResult(
                callback_id,
                self.task.app.client,
                task_name=getattr(reducer, 'name', 'reducer')
            )

        else:
            # Manual map-reduce
            map_results = self.map(items, use_group=False)

            # Wait for all results
            values = []
            for result in map_results:
                try:
                    value = result.get(timeout=300)
                    values.append(value)
                except Exception as e:
                    logger.error(f"Task failed: {e}")
                    if self.on_error:
                        self.on_error(e, result.task_id)

            # Apply reducer
            return reducer(values)

    def starmap(
        self,
        items: list[tuple],
        *,
        use_group: bool = True,
    ) -> list[AsyncResult]:
        """
        Map task over list of argument tuples.

        Args:
            items: List of tuples (args for each task)
            use_group: Use workflow group

        Returns:
            List of AsyncResult objects

        Example:
            processor = BatchProcessor(add)
            results = processor.starmap([(1, 2), (3, 4), (5, 6)])
        """
        if not items:
            return []

        results = []

        if use_group:
            from turbine import group

            for chunk in chunks(items, self.max_concurrent):
                signatures = [self.task.s(*args) for args in chunk]
                workflow_id, task_ids = group(*signatures).delay()

                chunk_results = [
                    AsyncResult(task_id, self.task.app.client, task_name=self.task.name)
                    for task_id in task_ids
                ]
                results.extend(chunk_results)

                if self.on_progress:
                    self.on_progress(len(results), len(items))

        else:
            for i, args in enumerate(items):
                try:
                    result = self.task.delay(*args)
                    results.append(result)

                    if self.on_progress:
                        self.on_progress(i + 1, len(items))

                except Exception as e:
                    logger.error(f"Failed to submit task for args {args}: {e}")
                    if self.on_error:
                        self.on_error(e, args)

        return results


class Batcher:
    """
    Accumulator for batching tasks before submission.

    Useful for reducing overhead when submitting many small tasks.
    """

    def __init__(
        self,
        task: Any,
        batch_size: int = 100,
        auto_submit: bool = True,
    ):
        """
        Initialize batcher.

        Args:
            task: Turbine task to batch
            batch_size: Size of batch before auto-submit
            auto_submit: Automatically submit when batch is full
        """
        self.task = task
        self.batch_size = batch_size
        self.auto_submit = auto_submit
        self._buffer: list[tuple[tuple, dict]] = []
        self._results: list[AsyncResult] = []

    def add(self, *args: Any, **kwargs: Any) -> None:
        """
        Add task to batch.

        Args:
            *args: Task positional arguments
            **kwargs: Task keyword arguments
        """
        self._buffer.append((args, kwargs))

        if self.auto_submit and len(self._buffer) >= self.batch_size:
            self.submit()

    def submit(self) -> list[AsyncResult]:
        """
        Submit all buffered tasks.

        Returns:
            List of AsyncResult objects
        """
        if not self._buffer:
            return []

        from turbine import group

        logger.debug(f"Submitting batch of {len(self._buffer)} tasks")

        # Create signatures for all buffered tasks
        signatures = [
            self.task.s(*args, **kwargs)
            for args, kwargs in self._buffer
        ]

        # Submit as group
        try:
            workflow_id, task_ids = group(*signatures).delay()

            results = [
                AsyncResult(task_id, self.task.app.client, task_name=self.task.name)
                for task_id in task_ids
            ]

            self._results.extend(results)
            self._buffer.clear()

            logger.info(f"Submitted batch: {len(results)} tasks")
            return results

        except Exception as e:
            logger.error(f"Batch submission failed: {e}")
            raise

    def flush(self) -> list[AsyncResult]:
        """
        Submit any remaining buffered tasks.

        Returns:
            List of AsyncResult objects
        """
        return self.submit()

    def results(self) -> list[AsyncResult]:
        """
        Get all submitted results.

        Returns:
            List of all AsyncResult objects
        """
        return self._results.copy()

    def __enter__(self):
        """Context manager entry."""
        return self

    def __exit__(self, exc_type, exc_val, exc_tb):
        """Context manager exit - auto-flush."""
        if self._buffer:
            self.submit()


def batch_map(
    task: Any,
    items: list[Any],
    batch_size: int = 100,
) -> list[AsyncResult]:
    """
    Convenience function for batch processing.

    Args:
        task: Turbine task
        items: Items to process
        batch_size: Batch size

    Returns:
        List of AsyncResult objects

    Example:
        from turbine.batch import batch_map

        results = batch_map(process_item, items, batch_size=50)
    """
    processor = BatchProcessor(task, chunk_size=batch_size)
    return processor.map(items)
