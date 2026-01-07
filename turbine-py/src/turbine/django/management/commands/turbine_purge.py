"""Django management command to purge Turbine queues."""

from django.core.management.base import BaseCommand, CommandError

from turbine.django import TurbineConfig


class Command(BaseCommand):
    """Purge messages from a Turbine queue."""

    help = "Purge all messages from a Turbine queue"

    def add_arguments(self, parser):
        parser.add_argument(
            "queue",
            type=str,
            help="Queue name to purge",
        )
        parser.add_argument(
            "--force",
            "-f",
            action="store_true",
            help="Skip confirmation prompt",
        )

    def handle(self, *args, **options):
        queue = options["queue"]

        try:
            app = TurbineConfig.get_app()
        except RuntimeError as e:
            raise CommandError(str(e))

        # Confirm unless --force
        if not options["force"]:
            confirm = input(f"Are you sure you want to purge queue '{queue}'? [y/N] ")
            if confirm.lower() != "y":
                self.stdout.write("Aborted.")
                return

        try:
            purged = app.purge_queue(queue)
            self.stdout.write(
                self.style.SUCCESS(f"Purged {purged} messages from queue '{queue}'")
            )
        except Exception as e:
            raise CommandError(f"Failed to purge queue: {e}")
