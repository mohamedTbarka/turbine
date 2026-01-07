"""Workflow primitives for Turbine: chain, group, chord."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

if TYPE_CHECKING:
    from turbine.app import Turbine
    from turbine.result import AsyncResult, GroupResult
    from turbine.task import Task


class Signature:
    """
    Represents a task call with frozen arguments.

    Signatures are used to build workflows. They capture the task,
    arguments, and options that will be used when the task is executed.

    Example:
        sig = add.s(1, 2)  # Create signature
        sig.delay()        # Execute with frozen args

        # Use in workflows
        chain(add.s(1, 2), add.s(3))
    """

    def __init__(
        self,
        task: "Task",
        args: tuple[Any, ...] | None = None,
        kwargs: dict[str, Any] | None = None,
        *,
        options: dict[str, Any] | None = None,
        immutable: bool = False,
    ):
        """
        Initialize a Signature.

        Args:
            task: The Task object
            args: Positional arguments
            kwargs: Keyword arguments
            options: Task options (queue, priority, etc.)
            immutable: If True, ignore results from previous tasks in chain
        """
        self.task = task
        self.args = tuple(args) if args else ()
        self.kwargs = dict(kwargs) if kwargs else {}
        self.options = dict(options) if options else {}
        self.immutable = immutable

    def __repr__(self) -> str:
        return f"<Signature: {self.task.name}({self.args}, {self.kwargs})>"

    @property
    def name(self) -> str:
        """Return task name."""
        return self.task.name

    def clone(
        self,
        args: tuple[Any, ...] | None = None,
        kwargs: dict[str, Any] | None = None,
        **options: Any,
    ) -> "Signature":
        """
        Create a copy of this signature with modified arguments.

        Args:
            args: New positional arguments (prepended to existing)
            kwargs: New keyword arguments (merged with existing)
            **options: Task options to override

        Returns:
            New Signature instance
        """
        new_args = tuple(args or ()) + self.args
        new_kwargs = {**self.kwargs, **(kwargs or {})}
        new_options = {**self.options, **options}

        return Signature(
            task=self.task,
            args=new_args,
            kwargs=new_kwargs,
            options=new_options,
            immutable=self.immutable,
        )

    def set(self, **options: Any) -> "Signature":
        """
        Set task options on this signature.

        Args:
            **options: Task options (queue, priority, countdown, etc.)

        Returns:
            Self for chaining
        """
        self.options.update(options)
        return self

    def delay(self, *args: Any, **kwargs: Any) -> "AsyncResult":
        """
        Execute this signature with optional additional arguments.

        Args:
            *args: Additional positional arguments (prepended)
            **kwargs: Additional keyword arguments (merged)

        Returns:
            AsyncResult for tracking the task
        """
        final_args = args + self.args
        final_kwargs = {**self.kwargs, **kwargs}
        return self.task.apply_async(
            args=final_args,
            kwargs=final_kwargs,
            **self.options,
        )

    def apply_async(self, **options: Any) -> "AsyncResult":
        """
        Execute this signature with option overrides.

        Args:
            **options: Task options to override

        Returns:
            AsyncResult for tracking the task
        """
        final_options = {**self.options, **options}
        return self.task.apply_async(
            args=self.args,
            kwargs=self.kwargs,
            **final_options,
        )

    def __or__(self, other: "Signature | Chain") -> "Chain":
        """
        Create a chain using | operator.

        Example:
            add.s(1, 2) | add.s(3) | mul.s(2)
        """
        if isinstance(other, Chain):
            return Chain(self, *other.tasks)
        return Chain(self, other)

    def to_dict(self) -> dict[str, Any]:
        """Convert to dictionary for serialization."""
        return {
            "name": self.task.name,
            "args": list(self.args),
            "kwargs": self.kwargs,
            "options": self.options,
            "immutable": self.immutable,
        }


class Chain:
    """
    A sequence of tasks to be executed one after another.

    The result of each task is passed as the first argument
    to the next task (unless the signature is immutable).

    Example:
        # Using chain()
        workflow = chain(
            fetch_data.s(url),
            process_data.s(),
            store_results.s()
        )
        result = workflow.delay()

        # Using | operator
        workflow = fetch_data.s(url) | process_data.s() | store_results.s()
    """

    def __init__(self, *tasks: Signature | "Chain"):
        """
        Initialize a Chain.

        Args:
            *tasks: Signatures or nested chains to execute sequentially
        """
        self.tasks: list[Signature] = []
        for task in tasks:
            if isinstance(task, Chain):
                self.tasks.extend(task.tasks)
            else:
                self.tasks.append(task)

    def __repr__(self) -> str:
        task_names = " | ".join(t.name for t in self.tasks)
        return f"<Chain: {task_names}>"

    def __len__(self) -> int:
        return len(self.tasks)

    def __or__(self, other: Signature | "Chain") -> "Chain":
        """Append to chain using | operator."""
        if isinstance(other, Chain):
            return Chain(*self.tasks, *other.tasks)
        return Chain(*self.tasks, other)

    def delay(self, *args: Any, **kwargs: Any) -> "AsyncResult":
        """
        Execute the chain.

        Args:
            *args: Arguments for the first task
            **kwargs: Keyword arguments for the first task

        Returns:
            AsyncResult for the final task
        """
        return self.apply_async(args=args, kwargs=kwargs)

    def apply_async(
        self,
        args: tuple[Any, ...] | list[Any] | None = None,
        kwargs: dict[str, Any] | None = None,
    ) -> "AsyncResult":
        """
        Execute the chain.

        Args:
            args: Arguments for the first task
            kwargs: Keyword arguments for the first task

        Returns:
            AsyncResult for the final task
        """
        if not self.tasks:
            raise ValueError("Chain has no tasks")

        # Get the app from the first task
        app = self.tasks[0].task.app

        # Build task list for submission
        task_dicts = []
        for i, sig in enumerate(self.tasks):
            task_dict = sig.to_dict()
            # Add initial args to first task
            if i == 0 and args:
                task_dict["args"] = list(args) + task_dict["args"]
            if i == 0 and kwargs:
                task_dict["kwargs"] = {**kwargs, **task_dict["kwargs"]}
            task_dicts.append(task_dict)

        # Submit chain to server
        workflow_id, task_ids = app.client.submit_chain(task_dicts)

        # Return AsyncResult for the last task
        from turbine.result import AsyncResult

        return AsyncResult(
            task_ids[-1],
            app.client,
            task_name=self.tasks[-1].name,
        )


class Group:
    """
    A group of tasks to be executed in parallel.

    Example:
        workflow = group(
            send_email.s(to="user1@example.com"),
            send_email.s(to="user2@example.com"),
            send_email.s(to="user3@example.com"),
        )
        result = workflow.delay()
        results = result.get()  # [result1, result2, result3]
    """

    def __init__(self, *tasks: Signature):
        """
        Initialize a Group.

        Args:
            *tasks: Signatures to execute in parallel
        """
        self.tasks = list(tasks)

    def __repr__(self) -> str:
        return f"<Group: {len(self.tasks)} tasks>"

    def __len__(self) -> int:
        return len(self.tasks)

    def __iter__(self):
        return iter(self.tasks)

    def delay(self, *args: Any, **kwargs: Any) -> "GroupResult":
        """
        Execute the group.

        Args:
            *args: Arguments passed to all tasks
            **kwargs: Keyword arguments passed to all tasks

        Returns:
            GroupResult for tracking all tasks
        """
        return self.apply_async(args=args, kwargs=kwargs)

    def apply_async(
        self,
        args: tuple[Any, ...] | list[Any] | None = None,
        kwargs: dict[str, Any] | None = None,
    ) -> "GroupResult":
        """
        Execute the group.

        Args:
            args: Arguments passed to all tasks
            kwargs: Keyword arguments passed to all tasks

        Returns:
            GroupResult for tracking all tasks
        """
        if not self.tasks:
            raise ValueError("Group has no tasks")

        # Get the app from the first task
        app = self.tasks[0].task.app

        # Build task list for submission
        task_dicts = []
        for sig in self.tasks:
            task_dict = sig.to_dict()
            # Add provided args to each task
            if args:
                task_dict["args"] = list(args) + task_dict["args"]
            if kwargs:
                task_dict["kwargs"] = {**kwargs, **task_dict["kwargs"]}
            task_dicts.append(task_dict)

        # Submit group to server
        workflow_id, task_ids = app.client.submit_group(task_dicts)

        # Create AsyncResult for each task
        from turbine.result import AsyncResult, GroupResult

        results = [
            AsyncResult(task_id, app.client, task_name=sig.name)
            for task_id, sig in zip(task_ids, self.tasks)
        ]

        return GroupResult(workflow_id, results, app.client)

    def __or__(self, callback: Signature) -> "Chord":
        """Create a chord using | operator."""
        return Chord(self, callback)


class Chord:
    """
    A chord is a group of tasks with a callback.

    The callback is executed after all tasks in the group complete,
    receiving the results as its first argument.

    Example:
        workflow = chord(
            [add.s(1, 2), add.s(3, 4), add.s(5, 6)],
            sum_results.s()
        )
        result = workflow.delay()
        final = result.get()  # sum of [3, 7, 11] = 21

        # Or using | operator
        workflow = group(add.s(1, 2), add.s(3, 4)) | sum_results.s()
    """

    def __init__(self, group: Group | list[Signature], callback: Signature):
        """
        Initialize a Chord.

        Args:
            group: Group of tasks or list of signatures
            callback: Callback signature to execute after group completes
        """
        if isinstance(group, list):
            group = Group(*group)
        self.group = group
        self.callback = callback

    def __repr__(self) -> str:
        return f"<Chord: {len(self.group)} tasks -> {self.callback.name}>"

    def delay(self, *args: Any, **kwargs: Any) -> "AsyncResult":
        """
        Execute the chord.

        Args:
            *args: Arguments for group tasks
            **kwargs: Keyword arguments for group tasks

        Returns:
            AsyncResult for the callback task
        """
        return self.apply_async(args=args, kwargs=kwargs)

    def apply_async(
        self,
        args: tuple[Any, ...] | list[Any] | None = None,
        kwargs: dict[str, Any] | None = None,
    ) -> "AsyncResult":
        """
        Execute the chord.

        Args:
            args: Arguments for group tasks
            kwargs: Keyword arguments for group tasks

        Returns:
            AsyncResult for the callback task
        """
        if not self.group.tasks:
            raise ValueError("Chord group has no tasks")

        # Get the app from the first task
        app = self.group.tasks[0].task.app

        # Build task list for group
        group_dicts = []
        for sig in self.group.tasks:
            task_dict = sig.to_dict()
            if args:
                task_dict["args"] = list(args) + task_dict["args"]
            if kwargs:
                task_dict["kwargs"] = {**kwargs, **task_dict["kwargs"]}
            group_dicts.append(task_dict)

        # Build callback
        callback_dict = self.callback.to_dict()

        # Submit chord to server
        workflow_id, task_ids = app.client.submit_chord(
            tasks=group_dicts,
            callback=callback_dict,
        )

        # Return AsyncResult for the callback (last task)
        from turbine.result import AsyncResult

        return AsyncResult(
            task_ids[-1],
            app.client,
            task_name=self.callback.name,
        )


# Convenience functions


def chain(*tasks: Signature | Chain) -> Chain:
    """
    Create a chain of tasks.

    Args:
        *tasks: Signatures or chains to execute sequentially

    Returns:
        Chain instance
    """
    return Chain(*tasks)


def group(*tasks: Signature) -> Group:
    """
    Create a group of parallel tasks.

    Args:
        *tasks: Signatures to execute in parallel

    Returns:
        Group instance
    """
    return Group(*tasks)


def chord(tasks: Group | list[Signature], callback: Signature) -> Chord:
    """
    Create a chord (group + callback).

    Args:
        tasks: Group or list of signatures
        callback: Callback signature

    Returns:
        Chord instance
    """
    return Chord(tasks, callback)
