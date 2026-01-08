"""URL patterns for the tasks API."""

from django.urls import path
from . import views

urlpatterns = [
    # Email endpoints
    path('send-email/', views.SendEmailView.as_view(), name='send-email'),
    path('send-welcome/', views.SendWelcomeEmailView.as_view(), name='send-welcome'),
    path('send-bulk/', views.SendBulkEmailsView.as_view(), name='send-bulk'),

    # Processing endpoints
    path('process-data/', views.ProcessDataView.as_view(), name='process-data'),
    path('generate-report/', views.GenerateReportView.as_view(), name='generate-report'),

    # Task status endpoint
    path('task/<str:task_id>/', views.TaskStatusView.as_view(), name='task-status'),
]
