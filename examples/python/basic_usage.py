#!/usr/bin/env python3
"""
Basic Turbine usage example.

Prerequisites:
1. Start the Turbine server: ./target/release/turbine-server
2. Start the Turbine worker: ./target/release/turbine-worker
3. Install turbine-py: pip install -e turbine-py

Usage:
    python examples/python/basic_usage.py
"""

import time

from turbine import Turbine, chain, chord, group

# Create Turbine app
app = Turbine(server="localhost:50051")


# Define tasks
@app.task(queue="default")
def add(x: int, y: int) -> int:
    """Add two numbers."""
    return x + y


@app.task(queue="default")
def multiply(x: int, y: int) -> int:
    """Multiply two numbers."""
    return x * y


@app.task(queue="default")
def process_data(data: str) -> str:
    """Process some data."""
    return f"Processed: {data}"


@app.task(queue="default")
def sum_results(results: list[int]) -> int:
    """Sum a list of results."""
    return sum(results)


def example_simple_task():
    """Example: Submit a simple task."""
    print("\n=== Simple Task ===")

    # Submit task
    result = add.delay(1, 2)
    print(f"Task ID: {result.task_id}")

    # Check status
    print(f"Status: {result.status}")

    # Wait for result
    try:
        value = result.get(timeout=30)
        print(f"Result: {value}")
    except Exception as e:
        print(f"Error: {e}")


def example_task_with_options():
    """Example: Submit task with custom options."""
    print("\n=== Task with Options ===")

    # Submit with countdown (execute in 5 seconds)
    result = multiply.apply_async(
        args=[3, 4],
        countdown=5,
        queue="default",
    )
    print(f"Task ID: {result.task_id}")
    print("Waiting for delayed task...")

    value = result.get(timeout=30)
    print(f"Result: {value}")


def example_chain():
    """Example: Chain of tasks (sequential execution)."""
    print("\n=== Chain Example ===")

    # Create chain: add(1, 2) -> add(result, 3) -> multiply(result, 2)
    workflow = chain(
        add.s(1, 2),      # Returns 3
        add.s(3),         # Previous result + 3 = 6
        multiply.s(2),    # Previous result * 2 = 12
    )

    result = workflow.delay()
    print(f"Workflow started, final task ID: {result.task_id}")

    value = result.get(timeout=30)
    print(f"Final result: {value}")  # Should be 12


def example_group():
    """Example: Group of tasks (parallel execution)."""
    print("\n=== Group Example ===")

    # Execute multiple tasks in parallel
    workflow = group(
        add.s(1, 2),
        add.s(3, 4),
        add.s(5, 6),
    )

    result = workflow.delay()
    print(f"Group started, workflow ID: {result.workflow_id}")
    print(f"Tasks: {len(result.results)}")

    values = result.get(timeout=30)
    print(f"Results: {values}")  # Should be [3, 7, 11]


def example_chord():
    """Example: Chord (group + callback)."""
    print("\n=== Chord Example ===")

    # Execute tasks in parallel, then sum the results
    workflow = chord(
        [add.s(1, 2), add.s(3, 4), add.s(5, 6)],  # Parallel tasks
        sum_results.s(),  # Callback receives [3, 7, 11]
    )

    result = workflow.delay()
    print(f"Chord started, callback task ID: {result.task_id}")

    value = result.get(timeout=30)
    print(f"Sum of results: {value}")  # Should be 21


def example_health_check():
    """Example: Check server health."""
    print("\n=== Health Check ===")

    health = app.health_check()
    print(f"Status: {health['status']}")
    print(f"Version: {health['version']}")
    print(f"Uptime: {health['uptime']}s")
    print(f"Broker: {health['broker']}")
    print(f"Backend: {health['backend']}")


def example_queue_info():
    """Example: Get queue information."""
    print("\n=== Queue Info ===")

    queues = app.get_queue_info()
    for q in queues:
        print(f"Queue: {q['name']}")
        print(f"  Pending: {q['pending']}")
        print(f"  Processing: {q['processing']}")
        print(f"  Consumers: {q['consumers']}")


def example_send_task_by_name():
    """Example: Send task by name (without local registration)."""
    print("\n=== Send Task by Name ===")

    # Send to built-in echo handler
    result = app.send_task(
        "echo",
        args=["Hello from Python!"],
        queue="default",
    )
    print(f"Task ID: {result.task_id}")

    value = result.get(timeout=30)
    print(f"Echo result: {value}")


def main():
    """Run all examples."""
    print("Turbine Python SDK Examples")
    print("=" * 50)

    try:
        # Check connection first
        example_health_check()

        # Run examples
        example_simple_task()
        example_send_task_by_name()
        example_task_with_options()
        example_chain()
        example_group()
        example_chord()
        example_queue_info()

        print("\n" + "=" * 50)
        print("All examples completed!")

    except Exception as e:
        print(f"\nError: {e}")
        print("\nMake sure Turbine server and worker are running:")
        print("  ./target/release/turbine-server")
        print("  ./target/release/turbine-worker")

    finally:
        app.close()


if __name__ == "__main__":
    main()
