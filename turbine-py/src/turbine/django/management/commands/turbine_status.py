"""Django management command to check Turbine server status."""

from django.core.management.base import BaseCommand

from turbine.django import TurbineConfig


class Command(BaseCommand):
    """Check Turbine server status and queue information."""

    help = "Check Turbine server status and queue information"

    def add_arguments(self, parser):
        parser.add_argument(
            "--queue",
            "-q",
            type=str,
            help="Show info for specific queue",
        )
        parser.add_argument(
            "--tasks",
            "-t",
            action="store_true",
            help="List registered tasks",
        )

    def handle(self, *args, **options):
        try:
            app = TurbineConfig.get_app()
        except RuntimeError as e:
            self.stderr.write(self.style.ERROR(str(e)))
            return

        # Health check
        try:
            health = app.health_check()
            self.stdout.write(self.style.SUCCESS("Server Status:"))
            self.stdout.write(f"  Status: {health['status']}")
            self.stdout.write(f"  Version: {health['version']}")
            self.stdout.write(f"  Uptime: {health['uptime']}s")
            self.stdout.write(f"  Broker: {health['broker']}")
            self.stdout.write(f"  Backend: {health['backend']}")
        except Exception as e:
            self.stderr.write(self.style.ERROR(f"Failed to connect to server: {e}"))
            return

        # Queue info
        self.stdout.write("")
        self.stdout.write(self.style.SUCCESS("Queue Information:"))
        try:
            queues = app.get_queue_info(options.get("queue"))
            if not queues:
                self.stdout.write("  No queues found")
            for q in queues:
                self.stdout.write(f"  {q['name']}:")
                self.stdout.write(f"    Pending: {q['pending']}")
                self.stdout.write(f"    Processing: {q['processing']}")
                self.stdout.write(f"    Consumers: {q['consumers']}")
                self.stdout.write(f"    Throughput: {q['throughput']:.2f}/s")
        except Exception as e:
            self.stderr.write(self.style.ERROR(f"Failed to get queue info: {e}"))

        # Registered tasks
        if options.get("tasks"):
            self.stdout.write("")
            self.stdout.write(self.style.SUCCESS("Registered Tasks:"))
            tasks = app.tasks
            if not tasks:
                self.stdout.write("  No tasks registered")
            for name, task in sorted(tasks.items()):
                self.stdout.write(f"  {name}")
                self.stdout.write(f"    Queue: {task.queue}")
                self.stdout.write(f"    Timeout: {task.timeout}s")
                self.stdout.write(f"    Retries: {task.max_retries}")
