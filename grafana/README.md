# Grafana Dashboards for Turbine

This directory contains pre-built Grafana dashboard templates for monitoring Turbine task queues.

## Available Dashboards

### 1. Turbine Overview (`turbine-overview.json`)

Main dashboard showing:
- Task throughput (tasks/sec)
- Task duration percentiles (p50, p95, p99)
- Task states over time (success, failed, retry)
- Queue depths
- Success rate
- Active workers

### 2. Turbine Queues (Coming Soon)

Per-queue detailed metrics:
- Queue-specific throughput
- Queue depth over time
- Consumer count
- Average wait time
- Task processing time

### 3. Turbine Workers (Coming Soon)

Worker monitoring:
- Per-worker task count
- Worker CPU/memory usage
- Worker uptime
- Task distribution across workers

## Setup

### Prerequisites

- Grafana 8.0+
- Prometheus data source configured
- Turbine server running with Prometheus metrics enabled

### Installation

1. **Import Dashboard**

   In Grafana:
   - Go to Dashboards → Import
   - Upload `turbine-overview.json`
   - Select your Prometheus data source
   - Click Import

2. **Configure Prometheus**

   Ensure Turbine metrics are being scraped:

   ```yaml
   # prometheus.yml
   scrape_configs:
     - job_name: 'turbine'
       static_configs:
         - targets: ['localhost:9090']
       scrape_interval: 15s
   ```

3. **Verify Metrics**

   Check that metrics are available:
   ```bash
   curl http://localhost:9090/metrics | grep turbine
   ```

## Turbine Metrics

The following Prometheus metrics are exported by Turbine:

### Task Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `turbine_tasks_total` | Counter | Total tasks by state (success, failed, etc.) |
| `turbine_task_duration_seconds` | Histogram | Task execution duration |
| `turbine_tasks_running` | Gauge | Currently running tasks |
| `turbine_tasks_pending` | Gauge | Pending tasks across all queues |

### Queue Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `turbine_queue_pending_tasks` | Gauge | Pending tasks per queue |
| `turbine_queue_processing_tasks` | Gauge | Processing tasks per queue |
| `turbine_queue_consumers` | Gauge | Number of consumers per queue |
| `turbine_queue_throughput` | Gauge | Tasks/sec per queue |

### Worker Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `turbine_active_workers` | Gauge | Number of active workers |
| `turbine_worker_tasks_processed` | Counter | Tasks processed per worker |
| `turbine_worker_uptime_seconds` | Gauge | Worker uptime |

### System Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `turbine_server_uptime_seconds` | Gauge | Server uptime |
| `turbine_broker_connections` | Gauge | Active broker connections |
| `turbine_backend_operations_total` | Counter | Backend operations (get, set, delete) |

## Dashboard Customization

### Adding Alerts

1. Click on a panel
2. Go to Alert tab
3. Create alert rule

Example alert for high failure rate:

```yaml
condition:
  - query: increase(turbine_tasks_total{state="failed"}[5m])
    threshold: 100
    operator: ">="
```

### Adding Variables

Add template variables for filtering:

1. Dashboard Settings → Variables → Add variable
2. Name: `queue`
3. Type: Query
4. Query: `label_values(turbine_queue_pending_tasks, queue)`
5. Use in queries: `{queue="$queue"}`

### Custom Panels

Add custom panels using PromQL:

```promql
# Task success rate by queue
100 * (
  rate(turbine_tasks_total{state="success"}[5m]) /
  (rate(turbine_tasks_total{state="success"}[5m]) + rate(turbine_tasks_total{state="failed"}[5m]))
)

# Average task duration
rate(turbine_task_duration_seconds_sum[5m]) / rate(turbine_task_duration_seconds_count[5m])

# Queue backlog growth rate
deriv(turbine_queue_pending_tasks[5m])
```

## Troubleshooting

### No Data Showing

1. **Check Prometheus scraping**:
   ```bash
   curl http://localhost:9090/api/v1/targets
   ```

2. **Verify Turbine metrics endpoint**:
   ```bash
   curl http://localhost:9090/metrics
   ```

3. **Check Grafana data source**:
   - Configuration → Data Sources → Prometheus
   - Click "Test" button

### Metrics Not Available

Ensure Turbine server is started with telemetry enabled:

```bash
./turbine-server --prometheus-port 9090
```

Or via config file:

```toml
[telemetry]
prometheus_enabled = true
prometheus_port = 9090
```

## Advanced Configuration

### Multi-Tenant Dashboards

Filter by tenant:

```promql
turbine_tasks_total{tenant_id="acme-corp"}
```

Add tenant variable:
1. Dashboard Settings → Variables
2. Name: `tenant`
3. Query: `label_values(turbine_tasks_total, tenant_id)`

### Annotations

Add deployment annotations:

```json
{
  "datasource": "Prometheus",
  "enable": true,
  "expr": "turbine_server_uptime_seconds < 60",
  "iconColor": "red",
  "name": "Restarts",
  "tagKeys": "deployment"
}
```

## Export and Share

### Export Dashboard

1. Dashboard Settings → JSON Model
2. Copy JSON
3. Save to file

### Share via Grafana.com

1. Dashboard Settings → Save As
2. Click "Export for sharing externally"
3. Upload to grafana.com

## Examples

### Alert on High Failure Rate

```yaml
alert: HighTaskFailureRate
expr: |
  (
    rate(turbine_tasks_total{state="failed"}[5m]) /
    rate(turbine_tasks_total[5m])
  ) > 0.1
for: 5m
labels:
  severity: warning
annotations:
  summary: "High task failure rate detected"
  description: "{{ $value }}% of tasks are failing"
```

### Alert on Queue Backlog

```yaml
alert: QueueBacklog
expr: turbine_queue_pending_tasks > 1000
for: 10m
labels:
  severity: warning
annotations:
  summary: "Queue {{ $labels.queue }} has high backlog"
  description: "{{ $value }} pending tasks"
```

## Resources

- [Grafana Documentation](https://grafana.com/docs/)
- [Prometheus Query Language](https://prometheus.io/docs/prometheus/latest/querying/basics/)
- [Turbine Metrics Reference](../docs/metrics.md)
