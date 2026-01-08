"""
Email tasks for Django example.

These tasks demonstrate how to define and use Turbine tasks for email operations.
"""

from turbine import task
import logging
import time

logger = logging.getLogger(__name__)


@task(
    name='send_email',
    queue='emails',
    retry=3,
    retry_delay=60,
    timeout=120,
)
def send_email(to: str, subject: str, body: str, html: bool = False) -> dict:
    """
    Send an email to the specified recipient.

    Args:
        to: Email recipient address
        subject: Email subject line
        body: Email body content
        html: Whether the body is HTML

    Returns:
        dict with send status and message_id
    """
    logger.info(f"Sending email to {to}: {subject}")

    # Simulate email sending
    time.sleep(1)

    # In a real application, you would use Django's email backend:
    # from django.core.mail import send_mail
    # send_mail(subject, body, 'from@example.com', [to], html_message=body if html else None)

    message_id = f"msg_{int(time.time())}"
    logger.info(f"Email sent successfully: {message_id}")

    return {
        'success': True,
        'message_id': message_id,
        'recipient': to,
    }


@task(
    name='send_welcome_email',
    queue='emails',
    retry=3,
)
def send_welcome_email(user_id: int, email: str, name: str) -> dict:
    """
    Send a welcome email to a new user.

    Args:
        user_id: The user's ID
        email: User's email address
        name: User's display name

    Returns:
        dict with send status
    """
    logger.info(f"Sending welcome email to user {user_id}: {email}")

    subject = f"Welcome to our platform, {name}!"
    body = f"""
    Hi {name},

    Welcome to our platform! We're excited to have you on board.

    Here are some things you can do to get started:
    - Complete your profile
    - Explore our features
    - Connect with other users

    If you have any questions, feel free to reach out to our support team.

    Best regards,
    The Team
    """

    # Use the send_email task
    result = send_email(to=email, subject=subject, body=body)

    return {
        'user_id': user_id,
        'email_sent': result['success'],
        'message_id': result.get('message_id'),
    }


@task(
    name='send_bulk_emails',
    queue='emails',
    timeout=3600,  # 1 hour for bulk operations
)
def send_bulk_emails(recipients: list, subject: str, body: str) -> dict:
    """
    Send emails to multiple recipients.

    Args:
        recipients: List of email addresses
        subject: Email subject
        body: Email body

    Returns:
        dict with success/failure counts
    """
    logger.info(f"Sending bulk email to {len(recipients)} recipients")

    success_count = 0
    failure_count = 0
    failures = []

    for recipient in recipients:
        try:
            result = send_email(to=recipient, subject=subject, body=body)
            if result['success']:
                success_count += 1
            else:
                failure_count += 1
                failures.append(recipient)
        except Exception as e:
            logger.error(f"Failed to send email to {recipient}: {e}")
            failure_count += 1
            failures.append(recipient)

    return {
        'total': len(recipients),
        'success': success_count,
        'failed': failure_count,
        'failures': failures,
    }
