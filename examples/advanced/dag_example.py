"""
Advanced example: Task Dependencies with DAG

This example demonstrates:
- Building complex task dependency graphs
- DAG execution with proper ordering
- Handling task failures in dependencies
- Visualizing task relationships
"""

from turbine import Turbine, task
from turbine.dag import DAG, TaskNode, parallel


# Initialize Turbine
app = Turbine(server="localhost:50051")


@task(queue="data")
def fetch_user_data(user_id: int) -> dict:
    """Fetch user data from database."""
    import time
    time.sleep(0.5)

    return {
        "user_id": user_id,
        "name": f"User {user_id}",
        "email": f"user{user_id}@example.com",
    }


@task(queue="data")
def fetch_order_data(user_id: int) -> list[dict]:
    """Fetch user's orders."""
    import time
    time.sleep(0.3)

    return [
        {"order_id": i, "amount": i * 10}
        for i in range(1, 4)
    ]


@task(queue="processing")
def calculate_totals(orders: list[dict]) -> dict:
    """Calculate order totals."""
    total_amount = sum(o["amount"] for o in orders)
    order_count = len(orders)

    return {
        "order_count": order_count,
        "total_amount": total_amount,
    }


@task(queue="processing")
def generate_user_report(user_data: dict, order_data: dict) -> dict:
    """Generate comprehensive user report."""
    return {
        "user_id": user_data["user_id"],
        "name": user_data["name"],
        "email": user_data["email"],
        "order_count": order_data["order_count"],
        "total_spent": order_data["total_amount"],
        "report_generated": True,
    }


@task(queue="notifications")
def send_report(report: dict) -> dict:
    """Send report via email."""
    import time
    time.sleep(0.2)

    return {
        "sent": True,
        "recipient": report["email"],
        "report_id": f"report-{report['user_id']}",
    }


def example_simple_dag():
    """Simple linear DAG."""
    print("=" * 60)
    print("Example 1: Simple Linear DAG")
    print("=" * 60)

    dag = DAG(name="user-report-pipeline")

    # Build pipeline: fetch -> calculate -> report -> send
    user_id = 12345

    node1 = dag.add_task(
        fetch_order_data,
        args=(user_id,),
    )

    node2 = dag.add_task(
        calculate_totals,
        dependencies=[node1],
    )

    node3 = dag.add_task(
        fetch_user_data,
        args=(user_id,),
    )

    node4 = dag.add_task(
        generate_user_report,
        dependencies=[node3, node2],
    )

    node5 = dag.add_task(
        send_report,
        dependencies=[node4],
    )

    # Visualize
    print("\nDAG Structure:")
    print(dag.visualize())

    # Execute
    print("\nExecuting DAG...")
    results = dag.execute(wait=True, timeout=60)

    # Get final result
    final_result = results[node5].get()

    print(f"\n✓ Pipeline completed!")
    print(f"  Report sent: {final_result['sent']}")
    print(f"  Report ID: {final_result['report_id']}")


def example_parallel_branches():
    """DAG with parallel branches that merge."""
    print("\n" + "=" * 60)
    print("Example 2: DAG with Parallel Branches")
    print("=" * 60)

    @task(queue="analytics")
    def analyze_purchases(orders: list[dict]) -> dict:
        """Analyze purchase patterns."""
        import time
        time.sleep(0.4)

        return {
            "analysis": "high-value-customer",
            "avg_order": sum(o["amount"] for o in orders) / len(orders),
        }

    @task(queue="analytics")
    def analyze_behavior(user_data: dict) -> dict:
        """Analyze user behavior."""
        import time
        time.sleep(0.4)

        return {
            "activity_score": 85,
            "segment": "active",
        }

    @task(queue="ml")
    def predict_churn(analytics: dict, behavior: dict) -> dict:
        """Predict customer churn."""
        import time
        time.sleep(0.6)

        return {
            "churn_probability": 0.15,
            "risk_level": "low",
        }

    dag = DAG(name="customer-analytics")

    # Fetch data
    fetch_user = dag.add_task(fetch_user_data, args=(12345,))
    fetch_orders = dag.add_task(fetch_order_data, args=(12345,))

    # Parallel analysis
    analyze_purch = dag.add_task(analyze_purchases, dependencies=[fetch_orders])
    analyze_behav = dag.add_task(analyze_behavior, dependencies=[fetch_user])

    # Merge and predict
    predict = dag.add_task(
        predict_churn,
        dependencies=[analyze_purch, analyze_behav]
    )

    print("\nDAG Structure:")
    print(dag.visualize())

    print("\nExecuting DAG...")
    results = dag.execute(wait=True, timeout=120)

    prediction = results[predict].get()

    print(f"\n✓ Analysis completed!")
    print(f"  Churn probability: {prediction['churn_probability']:.1%}")
    print(f"  Risk level: {prediction['risk_level']}")


def example_diamond_dag():
    """Diamond-shaped DAG (fan-out then fan-in)."""
    print("\n" + "=" * 60)
    print("Example 3: Diamond DAG (Fan-out/Fan-in)")
    print("=" * 60)

    @task(queue="processing")
    def split_data(data: dict) -> dict:
        """Split data for parallel processing."""
        return {
            "chunk1": data["values"][:len(data["values"])//2],
            "chunk2": data["values"][len(data["values"])//2:],
        }

    @task(queue="processing")
    def process_chunk(chunk: list) -> int:
        """Process a data chunk."""
        return sum(chunk) * 2

    @task(queue="processing")
    def merge_results(result1: int, result2: int) -> dict:
        """Merge processed results."""
        return {
            "total": result1 + result2,
            "merged": True,
        }

    dag = DAG(name="diamond-pipeline")

    # Root: split data
    split = dag.add_task(
        split_data,
        kwargs={"data": {"values": list(range(1, 11))}}
    )

    # Two parallel branches
    process1 = dag.add_task(process_chunk, dependencies=[split])
    process2 = dag.add_task(process_chunk, dependencies=[split])

    # Merge
    merge = dag.add_task(merge_results, dependencies=[process1, process2])

    print("\nDAG Structure:")
    print(dag.visualize())

    print("\nExecuting diamond DAG...")
    results = dag.execute(wait=True, timeout=60)

    final = results[merge].get()

    print(f"\n✓ Diamond DAG completed!")
    print(f"  Final result: {final}")


def example_conditional_dag():
    """DAG with conditional execution."""
    print("\n" + "=" * 60)
    print("Example 4: Conditional DAG Execution")
    print("=" * 60)

    @task(queue="validation")
    def validate_input(data: dict) -> dict:
        """Validate input data."""
        is_valid = data.get("amount", 0) > 0

        return {
            "valid": is_valid,
            "data": data,
        }

    @task(queue="processing")
    def process_valid_data(validation: dict) -> dict:
        """Process if validation passed."""
        if not validation["valid"]:
            raise ValueError("Validation failed")

        return {
            "processed": True,
            "result": validation["data"]["amount"] * 2,
        }

    dag = DAG(name="conditional-pipeline")

    # Validate then process
    validate = dag.add_task(
        validate_input,
        kwargs={"data": {"amount": 100}}
    )

    process = dag.add_task(
        process_valid_data,
        dependencies=[validate]
    )

    print("\nExecuting conditional DAG...")

    try:
        results = dag.execute(wait=True, timeout=60)
        final = results[process].get()
        print(f"\n✓ Processing completed!")
        print(f"  Result: {final}")

    except Exception as e:
        print(f"\n✗ Processing failed: {e}")


def example_parallel_helper():
    """Using parallel() helper for simple parallel execution."""
    print("\n" + "=" * 60)
    print("Example 5: Parallel Execution Helper")
    print("=" * 60)

    @task(queue="io")
    def fetch_api(api_id: int) -> dict:
        """Fetch data from API."""
        import time
        time.sleep(0.5)

        return {
            "api_id": api_id,
            "data": f"data-{api_id}",
        }

    print("Fetching from 5 APIs in parallel...")

    # Execute tasks in parallel
    results = parallel(
        fetch_api.s(1),
        fetch_api.s(2),
        fetch_api.s(3),
        fetch_api.s(4),
        fetch_api.s(5),
        wait=True
    )

    print(f"\n✓ Fetched {len(results)} API responses")
    for i, result in enumerate(results, 1):
        value = result.get()
        print(f"  API {i}: {value['data']}")


if __name__ == "__main__":
    import sys

    examples = {
        "1": ("Simple batch", example_simple_batch),
        "2": ("With progress", example_batch_processor_with_progress),
        "3": ("Map-reduce", example_map_reduce),
        "4": ("Batcher", example_batcher_accumulator),
        "5": ("Parallel", example_parallel_helper),
        "all": ("Run all", None),
    }

    if len(sys.argv) > 1 and sys.argv[1] in examples:
        choice = sys.argv[1]
    else:
        print("DAG & Batch Processing Examples\n")
        print("Available examples:")
        for key, (name, _) in examples.items():
            print(f"  {key}. {name}")
        print()
        choice = input("Select example (or 'all'): ").strip()

    if choice == "all":
        for key, (name, func) in examples.items():
            if func:
                try:
                    func()
                except Exception as e:
                    print(f"\n✗ Example failed: {e}")
    elif choice in examples and examples[choice][1]:
        try:
            examples[choice][1]()
        except Exception as e:
            print(f"\n✗ Example failed: {e}")
            sys.exit(1)
    else:
        print("Invalid choice")
        sys.exit(1)
