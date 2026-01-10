"""Turbine CLI utilities."""

from __future__ import annotations

import argparse
import json
import sys


def main() -> int:
    """Main CLI entry point."""
    parser = argparse.ArgumentParser(
        prog="turbine",
        description="Turbine task queue CLI",
    )
    subparsers = parser.add_subparsers(dest="command", help="Available commands")

    # Generate proto stubs
    gen_parser = subparsers.add_parser("generate-proto", help="Generate gRPC stubs from proto")
    gen_parser.add_argument(
        "--output",
        "-o",
        type=str,
        help="Output directory (default: turbine/_proto)",
    )

    # Health check
    health_parser = subparsers.add_parser("health", help="Check server health")
    health_parser.add_argument(
        "--server",
        "-s",
        type=str,
        default="localhost:50051",
        help="Server address",
    )

    # Queue info
    queue_parser = subparsers.add_parser("queues", help="Show queue information")
    queue_parser.add_argument(
        "--server",
        "-s",
        type=str,
        default="localhost:50051",
        help="Server address",
    )
    queue_parser.add_argument(
        "--queue",
        "-q",
        type=str,
        help="Specific queue name",
    )

    # Submit task
    submit_parser = subparsers.add_parser("submit", help="Submit a task")
    submit_parser.add_argument(
        "task_name",
        type=str,
        help="Task name to execute",
    )
    submit_parser.add_argument(
        "--server",
        "-s",
        type=str,
        default="localhost:50051",
        help="Server address",
    )
    submit_parser.add_argument(
        "--args",
        "-a",
        type=str,
        help="Task arguments (JSON array)",
    )
    submit_parser.add_argument(
        "--kwargs",
        "-k",
        type=str,
        help="Task keyword arguments (JSON object)",
    )
    submit_parser.add_argument(
        "--queue",
        "-q",
        type=str,
        default="default",
        help="Queue name",
    )
    submit_parser.add_argument(
        "--wait",
        "-w",
        action="store_true",
        help="Wait for result",
    )
    submit_parser.add_argument(
        "--timeout",
        "-t",
        type=int,
        default=30,
        help="Wait timeout in seconds",
    )

    # Task status
    status_parser = subparsers.add_parser("status", help="Get task status")
    status_parser.add_argument(
        "task_id",
        type=str,
        help="Task ID",
    )
    status_parser.add_argument(
        "--server",
        "-s",
        type=str,
        default="localhost:50051",
        help="Server address",
    )

    # Worker
    worker_parser = subparsers.add_parser("worker", help="Start a Python task worker")
    worker_parser.add_argument(
        "--broker-url",
        "-b",
        type=str,
        default="redis://localhost:6379",
        help="Broker URL (Redis)",
    )
    worker_parser.add_argument(
        "--backend-url",
        type=str,
        default="redis://localhost:6379",
        help="Backend URL (Redis)",
    )
    worker_parser.add_argument(
        "--queues",
        "-q",
        type=str,
        default="default",
        help="Comma-separated list of queues to consume from",
    )
    worker_parser.add_argument(
        "--concurrency",
        "-c",
        type=int,
        default=4,
        help="Number of concurrent task slots",
    )
    worker_parser.add_argument(
        "--include",
        "-I",
        type=str,
        action="append",
        help="Module to import for task discovery (can be repeated)",
    )
    worker_parser.add_argument(
        "--log-level",
        "-l",
        type=str,
        default="INFO",
        choices=["DEBUG", "INFO", "WARNING", "ERROR"],
        help="Log level",
    )

    # DLQ management
    dlq_parser = subparsers.add_parser("dlq", help="Manage Dead Letter Queue")
    dlq_subparsers = dlq_parser.add_subparsers(dest="dlq_command", help="DLQ commands")

    # DLQ list
    dlq_list_parser = dlq_subparsers.add_parser("list", help="List failed tasks in DLQ")
    dlq_list_parser.add_argument(
        "--backend-url",
        "-b",
        type=str,
        default="redis://localhost:6379",
        help="Backend URL (Redis)",
    )
    dlq_list_parser.add_argument(
        "--limit",
        "-n",
        type=int,
        default=20,
        help="Maximum number of tasks to display",
    )

    # DLQ stats
    dlq_stats_parser = dlq_subparsers.add_parser("stats", help="Show DLQ statistics")
    dlq_stats_parser.add_argument(
        "--backend-url",
        "-b",
        type=str,
        default="redis://localhost:6379",
        help="Backend URL (Redis)",
    )

    # DLQ inspect
    dlq_inspect_parser = dlq_subparsers.add_parser("inspect", help="Inspect a specific failed task")
    dlq_inspect_parser.add_argument(
        "task_id",
        type=str,
        help="Task ID to inspect",
    )
    dlq_inspect_parser.add_argument(
        "--backend-url",
        "-b",
        type=str,
        default="redis://localhost:6379",
        help="Backend URL (Redis)",
    )

    # DLQ remove
    dlq_remove_parser = dlq_subparsers.add_parser("remove", help="Remove a task from DLQ")
    dlq_remove_parser.add_argument(
        "task_id",
        type=str,
        help="Task ID to remove",
    )
    dlq_remove_parser.add_argument(
        "--backend-url",
        "-b",
        type=str,
        default="redis://localhost:6379",
        help="Backend URL (Redis)",
    )

    # DLQ clear
    dlq_clear_parser = dlq_subparsers.add_parser("clear", help="Clear all tasks from DLQ")
    dlq_clear_parser.add_argument(
        "--backend-url",
        "-b",
        type=str,
        default="redis://localhost:6379",
        help="Backend URL (Redis)",
    )
    dlq_clear_parser.add_argument(
        "--force",
        "-f",
        action="store_true",
        help="Skip confirmation prompt",
    )

    # Tenant management
    tenant_parser = subparsers.add_parser("tenant", help="Manage multi-tenancy")
    tenant_subparsers = tenant_parser.add_subparsers(dest="tenant_command", help="Tenant commands")

    # Tenant create
    tenant_create_parser = tenant_subparsers.add_parser("create", help="Create a new tenant")
    tenant_create_parser.add_argument("tenant_id", type=str, help="Tenant ID")
    tenant_create_parser.add_argument("name", type=str, help="Tenant name")
    tenant_create_parser.add_argument(
        "--backend-url", "-b", type=str, default="redis://localhost:6379", help="Backend URL"
    )

    # Tenant list
    tenant_list_parser = tenant_subparsers.add_parser("list", help="List all tenants")
    tenant_list_parser.add_argument(
        "--backend-url", "-b", type=str, default="redis://localhost:6379", help="Backend URL"
    )

    # Tenant get
    tenant_get_parser = tenant_subparsers.add_parser("get", help="Get tenant details")
    tenant_get_parser.add_argument("tenant_id", type=str, help="Tenant ID")
    tenant_get_parser.add_argument(
        "--backend-url", "-b", type=str, default="redis://localhost:6379", help="Backend URL"
    )

    # Tenant update
    tenant_update_parser = tenant_subparsers.add_parser("update", help="Update tenant")
    tenant_update_parser.add_argument("tenant_id", type=str, help="Tenant ID")
    tenant_update_parser.add_argument("--name", type=str, help="New tenant name")
    tenant_update_parser.add_argument(
        "--enabled", type=lambda x: x.lower() == "true", help="Enable/disable tenant (true/false)"
    )
    tenant_update_parser.add_argument(
        "--backend-url", "-b", type=str, default="redis://localhost:6379", help="Backend URL"
    )

    # Tenant delete
    tenant_delete_parser = tenant_subparsers.add_parser("delete", help="Delete a tenant")
    tenant_delete_parser.add_argument("tenant_id", type=str, help="Tenant ID")
    tenant_delete_parser.add_argument(
        "--backend-url", "-b", type=str, default="redis://localhost:6379", help="Backend URL"
    )
    tenant_delete_parser.add_argument(
        "--force", "-f", action="store_true", help="Skip confirmation"
    )

    # Tenant stats
    tenant_stats_parser = tenant_subparsers.add_parser("stats", help="Get tenant statistics")
    tenant_stats_parser.add_argument("tenant_id", type=str, help="Tenant ID")
    tenant_stats_parser.add_argument(
        "--backend-url", "-b", type=str, default="redis://localhost:6379", help="Backend URL"
    )

    args = parser.parse_args()

    if args.command == "generate-proto":
        from turbine.generate_proto import main as gen_main

        return gen_main()

    elif args.command == "health":
        return cmd_health(args)

    elif args.command == "queues":
        return cmd_queues(args)

    elif args.command == "submit":
        return cmd_submit(args)

    elif args.command == "status":
        return cmd_status(args)

    elif args.command == "worker":
        return cmd_worker(args)

    elif args.command == "dlq":
        return cmd_dlq(args)

    elif args.command == "tenant":
        return cmd_tenant(args)

    else:
        parser.print_help()
        return 0


def cmd_health(args) -> int:
    """Health check command."""
    from turbine import TurbineClient

    try:
        with TurbineClient(server=args.server) as client:
            health = client.health_check()
            print(json.dumps(health, indent=2))
            return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_queues(args) -> int:
    """Queue info command."""
    from turbine import TurbineClient

    try:
        with TurbineClient(server=args.server) as client:
            queues = client.get_queue_info(args.queue)
            print(json.dumps(queues, indent=2))
            return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_submit(args) -> int:
    """Submit task command."""
    from turbine import Turbine

    try:
        # Parse arguments
        task_args = json.loads(args.args) if args.args else []
        task_kwargs = json.loads(args.kwargs) if args.kwargs else {}

        app = Turbine(server=args.server)
        result = app.send_task(
            args.task_name,
            args=task_args,
            kwargs=task_kwargs,
            queue=args.queue,
        )

        print(f"Task submitted: {result.task_id}")

        if args.wait:
            print(f"Waiting for result (timeout: {args.timeout}s)...")
            try:
                value = result.get(timeout=args.timeout)
                print(f"Result: {json.dumps(value, indent=2)}")
            except Exception as e:
                print(f"Error waiting for result: {e}", file=sys.stderr)
                return 1

        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_status(args) -> int:
    """Task status command."""
    from turbine import TurbineClient

    try:
        with TurbineClient(server=args.server) as client:
            status = client.get_status(args.task_id)
            print(json.dumps(status, indent=2))
            return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_worker(args) -> int:
    """Start a Python task worker."""
    from turbine.worker import run_worker

    queues = [q.strip() for q in args.queues.split(",")]
    modules = args.include or []

    try:
        run_worker(
            broker_url=args.broker_url,
            backend_url=args.backend_url,
            queues=queues,
            concurrency=args.concurrency,
            task_modules=modules,
            log_level=args.log_level,
        )
        return 0
    except KeyboardInterrupt:
        return 0
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_dlq(args) -> int:
    """DLQ management commands."""
    from turbine.dlq import DLQManager

    try:
        manager = DLQManager(backend_url=args.backend_url)

        if args.dlq_command == "list":
            tasks = manager.list_failed_tasks(limit=args.limit)
            if not tasks:
                print("No failed tasks in DLQ")
                return 0

            print(f"Failed tasks in DLQ (showing {len(tasks)}):\n")
            for task in tasks:
                print(f"Task ID: {task['task_id']}")
                print(f"  Name: {task['task_name']}")
                print(f"  Failed at: {task['failed_at']}")
                print(f"  Retries: {task['retries']}")
                print(f"  Error: {task['error'][:100]}...")
                print()
            return 0

        elif args.dlq_command == "stats":
            stats = manager.get_dlq_stats()
            print(json.dumps(stats, indent=2))
            return 0

        elif args.dlq_command == "inspect":
            task = manager.get_failed_task(args.task_id)
            if not task:
                print(f"Task {args.task_id} not found in DLQ", file=sys.stderr)
                return 1
            print(json.dumps(task, indent=2, default=str))
            return 0

        elif args.dlq_command == "remove":
            if manager.remove_failed_task(args.task_id):
                print(f"Removed task {args.task_id} from DLQ")
                return 0
            else:
                print(f"Failed to remove task {args.task_id}", file=sys.stderr)
                return 1

        elif args.dlq_command == "clear":
            if not args.force:
                count = manager.count_failed_tasks()
                response = input(f"Are you sure you want to clear {count} tasks from DLQ? (y/N): ")
                if response.lower() != 'y':
                    print("Cancelled")
                    return 0

            count = manager.clear_dlq()
            print(f"Cleared {count} tasks from DLQ")
            return 0

        else:
            print("Unknown DLQ command", file=sys.stderr)
            return 1

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


def cmd_tenant(args) -> int:
    """Tenant management commands."""
    from turbine.tenancy import TenantManager, TenantQuotas

    try:
        manager = TenantManager(backend_url=args.backend_url)

        if args.tenant_command == "create":
            tenant = manager.create_tenant(
                tenant_id=args.tenant_id,
                name=args.name,
            )
            print(f"Created tenant: {tenant.tenant_id}")
            print(json.dumps(tenant.to_dict(), indent=2))
            return 0

        elif args.tenant_command == "list":
            tenants = manager.list_tenants()
            if not tenants:
                print("No tenants found")
                return 0

            print(f"Tenants ({len(tenants)}):\n")
            for tenant in tenants:
                status = "enabled" if tenant.enabled else "disabled"
                print(f"  {tenant.tenant_id}")
                print(f"    Name: {tenant.name}")
                print(f"    Status: {status}")
                print()
            return 0

        elif args.tenant_command == "get":
            tenant = manager.get_tenant(args.tenant_id)
            if not tenant:
                print(f"Tenant '{args.tenant_id}' not found", file=sys.stderr)
                return 1
            print(json.dumps(tenant.to_dict(), indent=2))
            return 0

        elif args.tenant_command == "update":
            updates = {}
            if args.name:
                updates["name"] = args.name
            if args.enabled is not None:
                updates["enabled"] = args.enabled

            tenant = manager.update_tenant(args.tenant_id, **updates)
            print(f"Updated tenant: {tenant.tenant_id}")
            print(json.dumps(tenant.to_dict(), indent=2))
            return 0

        elif args.tenant_command == "delete":
            if not args.force:
                response = input(f"Delete tenant '{args.tenant_id}'? (y/N): ")
                if response.lower() != 'y':
                    print("Cancelled")
                    return 0

            if manager.delete_tenant(args.tenant_id):
                print(f"Deleted tenant: {args.tenant_id}")
                return 0
            else:
                print(f"Tenant '{args.tenant_id}' not found", file=sys.stderr)
                return 1

        elif args.tenant_command == "stats":
            tenant = manager.get_tenant(args.tenant_id)
            if not tenant:
                print(f"Tenant '{args.tenant_id}' not found", file=sys.stderr)
                return 1

            stats = manager.get_tenant_stats(args.tenant_id)
            print(f"Statistics for tenant '{args.tenant_id}':")
            print(json.dumps(stats, indent=2))
            return 0

        else:
            print("Unknown tenant command", file=sys.stderr)
            return 1

    except ValueError as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1
    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
