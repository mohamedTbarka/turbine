"""
Advanced example: Task Routing and Load Balancing

This example demonstrates:
- Dynamic queue routing strategies
- Load balancing across multiple queues
- Consistent hashing for partitioning
- Rate limiting for task submission
"""

from turbine import Turbine, task
from turbine.routing import (
    TaskRouter,
    LoadBalancer,
    RateLimiter,
    RoutingStrategy,
    consistent_hash_router,
)


# Initialize Turbine
app = Turbine(server="localhost:50051")


@task(queue="default")
def process_request(request_id: int, user_id: int) -> dict:
    """Process a user request."""
    import time
    time.sleep(0.1)

    return {
        "request_id": request_id,
        "user_id": user_id,
        "processed": True,
    }


def example_round_robin():
    """Round-robin routing across queues."""
    print("=" * 60)
    print("Example 1: Round-Robin Routing")
    print("=" * 60)

    queues = ["queue-1", "queue-2", "queue-3"]
    router = TaskRouter(queues=queues, strategy=RoutingStrategy.ROUND_ROBIN)

    print(f"Routing tasks across {len(queues)} queues...")

    for i in range(1, 11):
        queue = router.route()
        print(f"  Request {i} -> {queue}")

        process_request.apply_async(
            args=[i, 100],
            queue=queue
        )

    print("\n✓ Tasks distributed evenly across queues")


def example_hash_routing():
    """Consistent hash routing by user_id."""
    print("\n" + "=" * 60)
    print("Example 2: Hash-Based Routing (User Partitioning)")
    print("=" * 60)

    queues = ["user-queue-0", "user-queue-1", "user-queue-2", "user-queue-3"]

    router = TaskRouter(
        queues=queues,
        strategy=RoutingStrategy.HASH,
        hash_key=lambda ctx: ctx.get("user_id", 0)
    )

    print("Routing tasks by user_id (same user always -> same queue)...\n")

    # Submit tasks for different users
    user_tasks = [
        (1, 101),  # Request 1, User 101
        (2, 102),
        (3, 101),  # Same user as request 1
        (4, 103),
        (5, 101),  # Same user as request 1
        (6, 102),  # Same user as request 2
    ]

    for request_id, user_id in user_tasks:
        queue = router.route(user_id=user_id)
        print(f"  Request {request_id} (User {user_id}) -> {queue}")

        process_request.apply_async(
            args=[request_id, user_id],
            queue=queue
        )

    print("\n✓ Tasks partitioned by user_id for consistent routing")


def example_load_balancing():
    """Dynamic load balancing based on queue depth."""
    print("\n" + "=" * 60)
    print("Example 3: Dynamic Load Balancing")
    print("=" * 60)

    queues = ["lb-queue-1", "lb-queue-2", "lb-queue-3"]

    balancer = LoadBalancer(
        turbine_client=app,
        queues=queues,
        refresh_interval=2
    )

    print("Routing tasks to least loaded queues...\n")

    for i in range(1, 11):
        # Get least loaded queue
        queue = balancer.get_least_loaded_queue()

        print(f"  Request {i} -> {queue}")

        result = balancer.route_task(
            task=process_request,
            args=[i, 200],
            strategy="least_loaded"
        )

        # Add small delay to allow queue stats to update
        import time
        time.sleep(0.1)

    # Show queue stats
    print("\nQueue Statistics:")
    stats = balancer.get_stats()
    for queue_name, queue_stats in stats.items():
        print(f"  {queue_name}:")
        print(f"    Pending: {queue_stats.get('pending', 0)}")
        print(f"    Processing: {queue_stats.get('processing', 0)}")
        print(f"    Throughput: {queue_stats.get('throughput', 0):.2f}/s")

    print("\n✓ Tasks balanced across queues")


def example_consistent_hashing():
    """Consistent hashing for user partitioning."""
    print("\n" + "=" * 60)
    print("Example 4: Consistent Hashing Router")
    print("=" * 60)

    print("Routing tasks using consistent hashing...\n")

    # Process requests for different users
    users = [101, 102, 103, 104, 101, 102]  # Some repeats

    for i, user_id in enumerate(users, 1):
        # Route to partition queue based on user_id
        queue = consistent_hash_router(
            task=process_request,
            partition_key=f"user:{user_id}",
            num_queues=4,
            queue_prefix="partition"
        )

        print(f"  Request {i} (User {user_id}) -> {queue}")

        process_request.apply_async(
            args=[i, user_id],
            queue=queue
        )

    print("\n✓ Tasks partitioned using consistent hashing")
    print("  Same user always goes to same partition queue")


def example_rate_limiting():
    """Rate limiting for task submission."""
    print("\n" + "=" * 60)
    print("Example 5: Rate Limiting")
    print("=" * 60)

    # Allow max 5 tasks per 10 seconds
    limiter = RateLimiter(
        redis_url="redis://localhost:6379",
        max_rate=5,
        period=10
    )

    print("Submitting tasks with rate limit (5 tasks/10s)...\n")

    for i in range(1, 11):
        # Check rate limit
        allowed, remaining = limiter.check_rate_limit(f"user:12345")

        if allowed:
            print(f"  Request {i}: ALLOWED (remaining: {remaining})")

            process_request.apply_async(
                args=[i, 12345],
                queue="rate-limited"
            )

        else:
            print(f"  Request {i}: RATE LIMITED (waiting...)")

            # Wait and retry
            limiter.wait_if_needed(f"user:12345")

            print(f"  Request {i}: RETRY -> ALLOWED")

            process_request.apply_async(
                args=[i, 12345],
                queue="rate-limited"
            )

    print("\n✓ All tasks submitted within rate limits")


def example_tenant_routing():
    """Route tasks by tenant with quotas."""
    print("\n" + "=" * 60)
    print("Example 6: Multi-Tenant Routing with Quotas")
    print("=" * 60)

    from turbine.tenancy import TenantManager, TenantQuotas

    # Setup tenants with different quotas
    manager = TenantManager()

    try:
        # Create tenants
        for tenant_id in ["tenant-a", "tenant-b"]:
            try:
                manager.create_tenant(
                    tenant_id=tenant_id,
                    name=f"Tenant {tenant_id.upper()}",
                    quotas=TenantQuotas(max_tasks_per_hour=3)
                )
            except ValueError:
                # Tenant already exists
                pass

        print("Submitting tasks for different tenants...\n")

        # Submit tasks for tenants
        for i in range(1, 8):
            tenant_id = "tenant-a" if i % 2 == 0 else "tenant-b"

            # Check quota
            allowed, error = manager.check_quota(tenant_id, "tasks_per_hour")

            if allowed:
                # Increment usage
                count = manager.increment_usage(tenant_id, "tasks_hour", ttl=3600)

                print(f"  Request {i} ({tenant_id}): ALLOWED (usage: {count}/3)")

                process_request.apply_async(
                    args=[i, i * 10],
                    tenant_id=tenant_id
                )

            else:
                print(f"  Request {i} ({tenant_id}): QUOTA EXCEEDED - {error}")

    finally:
        # Cleanup
        manager.delete_tenant("tenant-a")
        manager.delete_tenant("tenant-b")

    print("\n✓ Tenant quotas enforced successfully")


if __name__ == "__main__":
    import sys

    examples = {
        "1": ("Round-robin", example_round_robin),
        "2": ("Hash routing", example_hash_routing),
        "3": ("Load balancing", example_load_balancing),
        "4": ("Consistent hashing", example_consistent_hashing),
        "5": ("Rate limiting", example_rate_limiting),
        "6": ("Tenant routing", example_tenant_routing),
        "all": ("Run all", None),
    }

    if len(sys.argv) > 1 and sys.argv[1] in examples:
        choice = sys.argv[1]
    else:
        print("Routing & Load Balancing Examples\n")
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
                    import traceback
                    traceback.print_exc()
    elif choice in examples and examples[choice][1]:
        try:
            examples[choice][1]()
        except Exception as e:
            print(f"\n✗ Example failed: {e}")
            import traceback
            traceback.print_exc()
            sys.exit(1)
    else:
        print("Invalid choice")
        sys.exit(1)
