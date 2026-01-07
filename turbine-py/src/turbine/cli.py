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


if __name__ == "__main__":
    sys.exit(main())
