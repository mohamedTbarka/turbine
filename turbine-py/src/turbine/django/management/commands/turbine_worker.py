"""Django management command to run the Turbine Python worker."""

import logging
import sys

from django.conf import settings
from django.core.management.base import BaseCommand
from django.utils.module_loading import autodiscover_modules


class Command(BaseCommand):
    """Run a Turbine Python worker to execute tasks."""

    help = "Run a Turbine Python worker to execute tasks from queues"

    def add_arguments(self, parser):
        parser.add_argument(
            "--broker-url",
            "-b",
            type=str,
            default=None,
            help="Redis broker URL (default: from TURBINE_BROKER_URL setting)",
        )
        parser.add_argument(
            "--backend-url",
            "-B",
            type=str,
            default=None,
            help="Redis backend URL (default: from TURBINE_BACKEND_URL setting)",
        )
        parser.add_argument(
            "--queues",
            "-Q",
            type=str,
            default=None,
            help="Comma-separated list of queues to consume from (default: all registered)",
        )
        parser.add_argument(
            "--concurrency",
            "-c",
            type=int,
            default=4,
            help="Number of concurrent task slots (default: 4)",
        )
        parser.add_argument(
            "--include",
            "-I",
            action="append",
            default=None,
            help="Additional module to import for task discovery",
        )
        parser.add_argument(
            "--log-level",
            "-l",
            type=str,
            default="INFO",
            choices=["DEBUG", "INFO", "WARNING", "ERROR", "CRITICAL"],
            help="Logging level (default: INFO)",
        )
        parser.add_argument(
            "--no-autodiscover",
            action="store_true",
            help="Disable automatic task discovery from Django apps",
        )

    def handle(self, *args, **options):
        try:
            from turbine.worker import Worker, get_registry
        except ImportError as e:
            self.stderr.write(
                self.style.ERROR(
                    f"Failed to import worker module: {e}\n"
                    "Make sure you have installed the worker dependencies:\n"
                    "  pip install turbine-queue[worker]"
                )
            )
            sys.exit(1)

        # Configure logging
        log_level = options["log_level"]
        logging.basicConfig(
            level=getattr(logging, log_level),
            format="%(asctime)s %(levelname)s %(name)s: %(message)s",
        )

        # Get configuration from Django settings
        broker_url = options["broker_url"] or getattr(
            settings, "TURBINE_BROKER_URL", "redis://localhost:6379"
        )
        backend_url = options["backend_url"] or getattr(
            settings, "TURBINE_BACKEND_URL", broker_url
        )

        # Determine queues
        if options["queues"]:
            queues = [q.strip() for q in options["queues"].split(",")]
        else:
            queues = getattr(settings, "TURBINE_WORKER_QUEUES", ["default"])

        # Auto-discover tasks from Django apps
        if not options["no_autodiscover"]:
            self.stdout.write("Auto-discovering tasks from Django apps...")
            autodiscover_modules("tasks")

        # Build list of task modules to import
        task_modules = options["include"] or []

        # Add modules from settings
        task_modules.extend(getattr(settings, "TURBINE_TASK_MODULES", []))

        # Create and start the worker
        self.stdout.write(
            self.style.SUCCESS(
                f"Starting Turbine worker...\n"
                f"  Broker: {broker_url}\n"
                f"  Backend: {backend_url}\n"
                f"  Queues: {queues}\n"
                f"  Concurrency: {options['concurrency']}"
            )
        )

        worker = Worker(
            broker_url=broker_url,
            backend_url=backend_url,
            queues=queues,
            concurrency=options["concurrency"],
            task_modules=task_modules,
        )

        try:
            worker.start()
        except KeyboardInterrupt:
            self.stdout.write(self.style.WARNING("\nShutting down worker..."))
        except Exception as e:
            self.stderr.write(self.style.ERROR(f"Worker error: {e}"))
            sys.exit(1)
