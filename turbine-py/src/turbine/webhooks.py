"""Webhook notification system for task events."""

import logging
import json
from typing import Any, Callable
from dataclasses import dataclass, field
from enum import Enum
import requests
import redis

logger = logging.getLogger(__name__)


class WebhookEvent(str, Enum):
    """Webhook event types."""

    TASK_STARTED = "task.started"
    TASK_COMPLETED = "task.completed"
    TASK_FAILED = "task.failed"
    TASK_RETRY = "task.retry"
    TASK_REVOKED = "task.revoked"
    WORKFLOW_STARTED = "workflow.started"
    WORKFLOW_COMPLETED = "workflow.completed"
    WORKFLOW_FAILED = "workflow.failed"


@dataclass
class WebhookConfig:
    """Webhook configuration."""

    url: str
    events: list[WebhookEvent] = field(default_factory=list)
    secret: str | None = None
    headers: dict[str, str] = field(default_factory=dict)
    timeout: int = 10
    retry_count: int = 3


class WebhookManager:
    """
    Manager for webhook subscriptions and notifications.

    Example:
        manager = WebhookManager()

        # Subscribe to events
        manager.subscribe(
            url="https://api.example.com/webhooks/turbine",
            events=[WebhookEvent.TASK_COMPLETED, WebhookEvent.TASK_FAILED],
            secret="webhook-secret-key"
        )

        # Send notification
        manager.notify(
            event=WebhookEvent.TASK_COMPLETED,
            data={"task_id": "abc-123", "result": {"status": "ok"}}
        )
    """

    def __init__(
        self,
        redis_url: str = "redis://localhost:6379",
        key_prefix: str = "turbine:webhooks",
    ):
        """
        Initialize webhook manager.

        Args:
            redis_url: Redis URL for storing subscriptions
            key_prefix: Redis key prefix
        """
        self.redis_url = redis_url
        self.conn = redis.from_url(redis_url, decode_responses=True)
        self.key_prefix = key_prefix

    def subscribe(
        self,
        url: str,
        events: list[WebhookEvent] | None = None,
        secret: str | None = None,
        headers: dict[str, str] | None = None,
    ) -> str:
        """
        Subscribe to webhook events.

        Args:
            url: Webhook URL
            events: List of events to subscribe to (None = all)
            secret: Secret key for HMAC signing
            headers: Custom headers to include

        Returns:
            Subscription ID

        Example:
            webhook_id = manager.subscribe(
                url="https://api.example.com/hooks",
                events=[WebhookEvent.TASK_COMPLETED],
                secret="my-secret",
                headers={"X-API-Key": "key123"}
            )
        """
        import uuid

        webhook_id = str(uuid.uuid4())

        config = WebhookConfig(
            url=url,
            events=events or list(WebhookEvent),
            secret=secret,
            headers=headers or {},
        )

        # Store in Redis
        key = f"{self.key_prefix}:{webhook_id}"
        data = {
            "url": config.url,
            "events": [e.value for e in config.events],
            "secret": config.secret,
            "headers": json.dumps(config.headers),
            "timeout": config.timeout,
            "retry_count": config.retry_count,
        }

        self.conn.hset(key, mapping=data)

        logger.info(f"Created webhook subscription: {webhook_id} -> {url}")
        return webhook_id

    def unsubscribe(self, webhook_id: str) -> bool:
        """
        Unsubscribe webhook.

        Args:
            webhook_id: Subscription ID

        Returns:
            True if unsubscribed
        """
        key = f"{self.key_prefix}:{webhook_id}"
        deleted = self.conn.delete(key) > 0

        if deleted:
            logger.info(f"Deleted webhook subscription: {webhook_id}")

        return deleted

    def list_subscriptions(self) -> list[dict[str, Any]]:
        """
        List all webhook subscriptions.

        Returns:
            List of subscription dictionaries
        """
        keys = self.conn.keys(f"{self.key_prefix}:*")
        subscriptions = []

        for key in keys:
            webhook_id = key.split(":")[-1]
            data = self.conn.hgetall(key)

            if data:
                subscriptions.append({
                    "webhook_id": webhook_id,
                    "url": data.get("url"),
                    "events": json.loads(data.get("events", "[]")),
                    "has_secret": data.get("secret") is not None,
                })

        return subscriptions

    def notify(
        self,
        event: WebhookEvent,
        data: dict[str, Any],
        async_send: bool = True,
    ) -> None:
        """
        Send webhook notifications for an event.

        Args:
            event: Event type
            data: Event data to send
            async_send: Send asynchronously (recommended)

        Example:
            manager.notify(
                event=WebhookEvent.TASK_COMPLETED,
                data={
                    "task_id": "abc-123",
                    "task_name": "send_email",
                    "result": {"sent": True},
                    "duration_ms": 1050,
                }
            )
        """
        # Find subscriptions for this event
        keys = self.conn.keys(f"{self.key_prefix}:*")

        for key in keys:
            subscription = self.conn.hgetall(key)

            if not subscription:
                continue

            # Check if subscribed to this event
            subscribed_events = json.loads(subscription.get("events", "[]"))

            if event.value not in subscribed_events:
                continue

            # Build payload
            payload = {
                "event": event.value,
                "data": data,
                "timestamp": self._get_timestamp(),
            }

            # Send webhook
            if async_send:
                # Send asynchronously (don't block)
                self._send_async(subscription, payload)
            else:
                # Send synchronously
                self._send_sync(subscription, payload)

    def _send_async(self, subscription: dict, payload: dict) -> None:
        """Send webhook asynchronously (using background task)."""
        try:
            from turbine import task

            @task(queue="webhooks", max_retries=3, store_result=False)
            def send_webhook(url, payload, headers, secret, timeout):
                self._do_send(url, payload, headers, secret, timeout)

            send_webhook.delay(
                url=subscription["url"],
                payload=payload,
                headers=json.loads(subscription.get("headers", "{}")),
                secret=subscription.get("secret"),
                timeout=int(subscription.get("timeout", 10)),
            )

        except Exception as e:
            logger.error(f"Failed to queue webhook: {e}")

    def _send_sync(self, subscription: dict, payload: dict) -> None:
        """Send webhook synchronously."""
        self._do_send(
            url=subscription["url"],
            payload=payload,
            headers=json.loads(subscription.get("headers", "{}")),
            secret=subscription.get("secret"),
            timeout=int(subscription.get("timeout", 10)),
        )

    @staticmethod
    def _do_send(
        url: str,
        payload: dict,
        headers: dict,
        secret: str | None,
        timeout: int,
    ) -> None:
        """Actually send the webhook HTTP request."""
        try:
            # Add signature if secret provided
            if secret:
                import hmac
                import hashlib

                payload_str = json.dumps(payload)
                signature = hmac.new(
                    secret.encode(),
                    payload_str.encode(),
                    hashlib.sha256
                ).hexdigest()

                headers["X-Turbine-Signature"] = signature

            # Send POST request
            response = requests.post(
                url,
                json=payload,
                headers={
                    "Content-Type": "application/json",
                    "User-Agent": "Turbine-Webhook/1.0",
                    **headers
                },
                timeout=timeout,
            )

            response.raise_for_status()

            logger.debug(
                f"Webhook sent successfully: {url} "
                f"(status: {response.status_code})"
            )

        except requests.exceptions.RequestException as e:
            logger.error(f"Webhook delivery failed: {url} - {e}")
            raise

    @staticmethod
    def _get_timestamp() -> str:
        """Get ISO timestamp."""
        from datetime import datetime
        return datetime.utcnow().isoformat() + "Z"

    @staticmethod
    def verify_signature(payload: str, signature: str, secret: str) -> bool:
        """
        Verify webhook signature (for webhook receivers).

        Args:
            payload: Raw payload string
            signature: Signature from X-Turbine-Signature header
            secret: Shared secret

        Returns:
            True if signature is valid

        Example:
            # In webhook receiver
            @app.post("/webhooks/turbine")
            def handle_webhook(request):
                payload = request.body
                signature = request.headers.get("X-Turbine-Signature")

                if not WebhookManager.verify_signature(payload, signature, SECRET):
                    return {"error": "Invalid signature"}, 401

                # Process webhook
                data = json.loads(payload)
                handle_event(data)
        """
        import hmac
        import hashlib

        expected = hmac.new(
            secret.encode(),
            payload.encode(),
            hashlib.sha256
        ).hexdigest()

        return hmac.compare_digest(expected, signature)


def on_task_complete(
    webhook_url: str,
    secret: str | None = None,
):
    """
    Decorator to send webhook when task completes.

    Args:
        webhook_url: URL to send webhook to
        secret: Secret for HMAC signing

    Example:
        @task
        @on_task_complete("https://api.example.com/hooks")
        def important_task(data):
            return process(data)

        # When task completes, webhook is sent with result
    """

    def decorator(task_func):
        original_func = task_func.func if hasattr(task_func, 'func') else task_func

        @wraps(original_func)
        def wrapper(*args, **kwargs):
            task_name = getattr(task_func, 'name', original_func.__name__)

            try:
                # Execute task
                result = original_func(*args, **kwargs)

                # Send success webhook
                payload = {
                    "event": "task.completed",
                    "task_name": task_name,
                    "args": args,
                    "kwargs": kwargs,
                    "result": result,
                    "success": True,
                }

                _send_webhook(webhook_url, payload, secret)

                return result

            except Exception as e:
                # Send failure webhook
                payload = {
                    "event": "task.failed",
                    "task_name": task_name,
                    "args": args,
                    "kwargs": kwargs,
                    "error": str(e),
                    "success": False,
                }

                _send_webhook(webhook_url, payload, secret)

                raise

        if hasattr(task_func, 'func'):
            task_func.func = wrapper

        return task_func

    return decorator


def _send_webhook(url: str, payload: dict, secret: str | None) -> None:
    """Send webhook notification."""
    try:
        headers = {"Content-Type": "application/json"}

        if secret:
            import hmac
            import hashlib

            payload_str = json.dumps(payload)
            signature = hmac.new(
                secret.encode(),
                payload_str.encode(),
                hashlib.sha256
            ).hexdigest()

            headers["X-Turbine-Signature"] = signature

        requests.post(url, json=payload, headers=headers, timeout=10)

    except Exception as e:
        logger.error(f"Failed to send webhook to {url}: {e}")
