"""
API views for task submission and status checking.
"""

from rest_framework import status
from rest_framework.response import Response
from rest_framework.views import APIView

from .email import send_email, send_welcome_email, send_bulk_emails
from .processing import process_data, generate_report

from myproject import turbine


class SendEmailView(APIView):
    """Submit an email sending task."""

    def post(self, request):
        """
        Send an email.

        POST /api/send-email/
        {
            "to": "user@example.com",
            "subject": "Hello",
            "body": "World",
            "html": false
        }
        """
        to = request.data.get('to')
        subject = request.data.get('subject')
        body = request.data.get('body')
        html = request.data.get('html', False)

        if not all([to, subject, body]):
            return Response(
                {'error': 'Missing required fields: to, subject, body'},
                status=status.HTTP_400_BAD_REQUEST
            )

        # Submit the task
        result = send_email.delay(to=to, subject=subject, body=body, html=html)

        return Response({
            'task_id': result.task_id,
            'status': 'submitted',
            'message': f'Email task submitted for {to}',
        }, status=status.HTTP_202_ACCEPTED)


class SendWelcomeEmailView(APIView):
    """Submit a welcome email task."""

    def post(self, request):
        """
        Send a welcome email to a new user.

        POST /api/send-welcome/
        {
            "user_id": 123,
            "email": "user@example.com",
            "name": "John Doe"
        }
        """
        user_id = request.data.get('user_id')
        email = request.data.get('email')
        name = request.data.get('name')

        if not all([user_id, email, name]):
            return Response(
                {'error': 'Missing required fields: user_id, email, name'},
                status=status.HTTP_400_BAD_REQUEST
            )

        result = send_welcome_email.delay(user_id=user_id, email=email, name=name)

        return Response({
            'task_id': result.task_id,
            'status': 'submitted',
            'message': f'Welcome email task submitted for user {user_id}',
        }, status=status.HTTP_202_ACCEPTED)


class SendBulkEmailsView(APIView):
    """Submit a bulk email task."""

    def post(self, request):
        """
        Send emails to multiple recipients.

        POST /api/send-bulk/
        {
            "recipients": ["a@example.com", "b@example.com"],
            "subject": "Newsletter",
            "body": "Hello everyone!"
        }
        """
        recipients = request.data.get('recipients', [])
        subject = request.data.get('subject')
        body = request.data.get('body')

        if not recipients or not subject or not body:
            return Response(
                {'error': 'Missing required fields: recipients, subject, body'},
                status=status.HTTP_400_BAD_REQUEST
            )

        result = send_bulk_emails.delay(
            recipients=recipients,
            subject=subject,
            body=body
        )

        return Response({
            'task_id': result.task_id,
            'status': 'submitted',
            'recipient_count': len(recipients),
            'message': f'Bulk email task submitted for {len(recipients)} recipients',
        }, status=status.HTTP_202_ACCEPTED)


class ProcessDataView(APIView):
    """Submit a data processing task."""

    def post(self, request):
        """
        Process data.

        POST /api/process-data/
        {
            "data": [1, 2, 3, 4, 5]
        }
        """
        data = request.data.get('data', [])

        if not data:
            return Response(
                {'error': 'Missing required field: data'},
                status=status.HTTP_400_BAD_REQUEST
            )

        result = process_data.delay(data=data)

        return Response({
            'task_id': result.task_id,
            'status': 'submitted',
            'item_count': len(data),
            'message': f'Processing task submitted for {len(data)} items',
        }, status=status.HTTP_202_ACCEPTED)


class GenerateReportView(APIView):
    """Submit a report generation task."""

    def post(self, request):
        """
        Generate a report.

        POST /api/generate-report/
        {
            "report_type": "daily",
            "start_date": "2024-01-01",
            "end_date": "2024-01-31"
        }
        """
        report_type = request.data.get('report_type', 'daily')
        start_date = request.data.get('start_date')
        end_date = request.data.get('end_date')

        if not start_date or not end_date:
            return Response(
                {'error': 'Missing required fields: start_date, end_date'},
                status=status.HTTP_400_BAD_REQUEST
            )

        result = generate_report.delay(
            report_type=report_type,
            start_date=start_date,
            end_date=end_date
        )

        return Response({
            'task_id': result.task_id,
            'status': 'submitted',
            'report_type': report_type,
            'message': f'Report generation task submitted',
        }, status=status.HTTP_202_ACCEPTED)


class TaskStatusView(APIView):
    """Check the status of a task."""

    def get(self, request, task_id):
        """
        Get task status and result.

        GET /api/task/{task_id}/
        """
        try:
            result = turbine.get_result(task_id)

            if result is None:
                return Response({
                    'task_id': task_id,
                    'status': 'pending',
                    'message': 'Task is pending or not found',
                })

            return Response({
                'task_id': task_id,
                'status': result.status,
                'result': result.result if result.successful else None,
                'error': result.error if result.failed else None,
                'traceback': result.traceback if result.failed else None,
            })

        except Exception as e:
            return Response({
                'task_id': task_id,
                'status': 'error',
                'error': str(e),
            }, status=status.HTTP_500_INTERNAL_SERVER_ERROR)
