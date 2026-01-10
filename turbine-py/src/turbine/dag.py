"""Task dependency graph (DAG) utilities."""

import logging
from typing import Any, Callable
from collections import defaultdict, deque
from dataclasses import dataclass, field

from turbine.result import AsyncResult

logger = logging.getLogger(__name__)


@dataclass
class TaskNode:
    """Represents a task node in a DAG."""

    task: Any
    args: tuple = field(default_factory=tuple)
    kwargs: dict = field(default_factory=dict)
    dependencies: list[str] = field(default_factory=list)
    node_id: str | None = None

    def __post_init__(self):
        """Generate node ID if not provided."""
        if self.node_id is None:
            import uuid
            self.node_id = str(uuid.uuid4())


class DAG:
    """
    Directed Acyclic Graph for task dependencies.

    Allows defining complex task relationships and executing them
    in the correct order based on dependencies.

    Example:
        dag = DAG()

        # Add tasks
        task1_id = dag.add_task(fetch_data, args=["url"])
        task2_id = dag.add_task(process_data, dependencies=[task1_id])
        task3_id = dag.add_task(store_data, dependencies=[task2_id])

        # Execute DAG
        results = dag.execute()
    """

    def __init__(self, name: str | None = None):
        """
        Initialize DAG.

        Args:
            name: Optional name for the DAG
        """
        self.name = name or "unnamed-dag"
        self.nodes: dict[str, TaskNode] = {}
        self.results: dict[str, AsyncResult] = {}

    def add_task(
        self,
        task: Any,
        args: tuple | None = None,
        kwargs: dict | None = None,
        dependencies: list[str] | None = None,
        node_id: str | None = None,
    ) -> str:
        """
        Add a task to the DAG.

        Args:
            task: Turbine task to execute
            args: Task arguments
            kwargs: Task keyword arguments
            dependencies: List of node IDs this task depends on
            node_id: Optional custom node ID

        Returns:
            Node ID for this task

        Raises:
            ValueError: If dependencies create a cycle
        """
        node = TaskNode(
            task=task,
            args=args or (),
            kwargs=kwargs or {},
            dependencies=dependencies or [],
            node_id=node_id,
        )

        self.nodes[node.node_id] = node

        # Check for cycles
        if self._has_cycle():
            del self.nodes[node.node_id]
            raise ValueError(f"Adding task would create a cycle in DAG")

        logger.debug(f"Added task node {node.node_id} to DAG '{self.name}'")
        return node.node_id

    def _has_cycle(self) -> bool:
        """Check if the DAG has a cycle using DFS."""
        visited = set()
        rec_stack = set()

        def visit(node_id: str) -> bool:
            visited.add(node_id)
            rec_stack.add(node_id)

            node = self.nodes.get(node_id)
            if node:
                for dep in node.dependencies:
                    if dep not in visited:
                        if visit(dep):
                            return True
                    elif dep in rec_stack:
                        return True

            rec_stack.remove(node_id)
            return False

        for node_id in self.nodes:
            if node_id not in visited:
                if visit(node_id):
                    return True

        return False

    def _topological_sort(self) -> list[str]:
        """
        Get topological ordering of tasks.

        Returns:
            List of node IDs in execution order
        """
        in_degree = defaultdict(int)
        graph = defaultdict(list)

        # Build graph and calculate in-degrees
        for node_id, node in self.nodes.items():
            for dep in node.dependencies:
                graph[dep].append(node_id)
                in_degree[node_id] += 1

        # Find nodes with no dependencies
        queue = deque([
            node_id for node_id in self.nodes
            if in_degree[node_id] == 0
        ])

        result = []

        while queue:
            node_id = queue.popleft()
            result.append(node_id)

            # Reduce in-degree for dependent nodes
            for dependent in graph[node_id]:
                in_degree[dependent] -= 1
                if in_degree[dependent] == 0:
                    queue.append(dependent)

        # Check if all nodes were processed
        if len(result) != len(self.nodes):
            raise ValueError("DAG has a cycle")

        return result

    def execute(
        self,
        wait: bool = False,
        timeout: int = 300,
    ) -> dict[str, AsyncResult]:
        """
        Execute the DAG.

        Args:
            wait: Wait for all tasks to complete
            timeout: Timeout for waiting (seconds)

        Returns:
            Dictionary mapping node IDs to AsyncResult objects

        Example:
            results = dag.execute(wait=True)
            final_result = results[final_node_id].get()
        """
        order = self._topological_sort()
        logger.info(f"Executing DAG '{self.name}' with {len(order)} tasks")

        completed = {}
        results = {}

        for node_id in order:
            node = self.nodes[node_id]

            # Wait for dependencies to complete
            if node.dependencies:
                logger.debug(f"Waiting for dependencies of {node_id}")

                for dep_id in node.dependencies:
                    if dep_id not in completed:
                        # Wait for dependency
                        dep_result = results[dep_id]
                        try:
                            dep_value = dep_result.get(timeout=timeout)
                            completed[dep_id] = dep_value
                        except Exception as e:
                            logger.error(
                                f"Dependency {dep_id} failed: {e}, "
                                f"skipping {node_id}"
                            )
                            raise

            # Submit task
            logger.debug(f"Submitting task {node_id}")

            result = node.task.delay(*node.args, **node.kwargs)
            results[node_id] = result

        # Wait for all tasks if requested
        if wait:
            logger.debug("Waiting for all tasks to complete")

            for node_id in order:
                if node_id not in completed:
                    try:
                        value = results[node_id].get(timeout=timeout)
                        completed[node_id] = value
                    except Exception as e:
                        logger.error(f"Task {node_id} failed: {e}")
                        raise

        self.results = results
        return results

    def get_results(self, wait: bool = True, timeout: int = 300) -> dict[str, Any]:
        """
        Get results from all tasks.

        Args:
            wait: Wait for tasks to complete
            timeout: Timeout for each task

        Returns:
            Dictionary mapping node IDs to result values
        """
        if not self.results:
            raise RuntimeError("DAG has not been executed yet")

        values = {}

        for node_id, result in self.results.items():
            try:
                if wait:
                    values[node_id] = result.get(timeout=timeout)
                else:
                    ready, value = result.get()
                    if ready:
                        values[node_id] = value

            except Exception as e:
                logger.error(f"Failed to get result for {node_id}: {e}")
                values[node_id] = None

        return values

    def visualize(self) -> str:
        """
        Generate a simple text visualization of the DAG.

        Returns:
            Text representation of the DAG
        """
        lines = [f"DAG: {self.name}", "=" * 40]

        for node_id, node in self.nodes.items():
            task_name = getattr(node.task, 'name', str(node.task))
            lines.append(f"\n{node_id[:8]}... ({task_name})")

            if node.dependencies:
                lines.append(f"  Depends on:")
                for dep in node.dependencies:
                    lines.append(f"    - {dep[:8]}...")
            else:
                lines.append(f"  No dependencies (root)")

        return "\n".join(lines)


def parallel(*tasks: Any, wait: bool = False) -> list[AsyncResult]:
    """
    Execute tasks in parallel with no dependencies.

    Args:
        *tasks: Task signatures to execute
        wait: Wait for all to complete

    Returns:
        List of AsyncResult objects

    Example:
        from turbine.dag import parallel

        results = parallel(
            task1.s(arg1),
            task2.s(arg2),
            task3.s(arg3),
            wait=True
        )
    """
    from turbine import group

    workflow_id, task_ids = group(*tasks).delay()

    results = [
        AsyncResult(
            task_id,
            tasks[0].task.app.client,
            task_name=getattr(tasks[0].task, 'name', 'unknown')
        )
        for task_id in task_ids
    ]

    if wait:
        for result in results:
            result.get()

    return results
