# Turbine Security Guide

Security best practices and configuration for production Turbine deployments.

## Overview

Security layers in Turbine:

1. **Transport Security**: TLS/mTLS for gRPC communication
2. **Network Security**: Firewall rules and network isolation
3. **Authentication**: Client authentication (coming soon)
4. **Authorization**: Tenant isolation and quotas
5. **Data Security**: Encryption at rest and in transit
6. **Audit Logging**: Track all operations

## Transport Security (TLS/mTLS)

### Server TLS Configuration

Generate certificates:

```bash
# Generate CA
openssl genrsa -out ca.key 4096
openssl req -new -x509 -days 365 -key ca.key -out ca.crt

# Generate server certificate
openssl genrsa -out server.key 4096
openssl req -new -key server.key -out server.csr
openssl x509 -req -days 365 -in server.csr -CA ca.crt -CAkey ca.key -set_serial 01 -out server.crt
```

Configure server:

```toml
# turbine.toml
[security]
tls_enabled = true
tls_cert_path = "/etc/turbine/certs/server.crt"
tls_key_path = "/etc/turbine/certs/server.key"

# For mutual TLS (mTLS)
mtls_enabled = true
mtls_ca_path = "/etc/turbine/certs/ca.crt"
```

### Python Client TLS

```python
import grpc
from turbine import Turbine

# Load certificates
with open('/etc/turbine/certs/ca.crt', 'rb') as f:
    ca_cert = f.read()

with open('/etc/turbine/certs/client.crt', 'rb') as f:
    client_cert = f.read()

with open('/etc/turbine/certs/client.key', 'rb') as f:
    client_key = f.read()

# Create credentials for mTLS
credentials = grpc.ssl_channel_credentials(
    root_certificates=ca_cert,
    private_key=client_key,
    certificate_chain=client_cert,
)

app = Turbine(
    server="turbine.example.com:50051",
    secure=True,
    credentials=credentials,
)
```

## Network Security

### Firewall Rules

**Recommended ports:**
- `50051` - gRPC server (internal only)
- `8080` - REST API (internal or authenticated)
- `8081` - Dashboard (internal or VPN only)
- `9090` - Prometheus metrics (monitoring network only)

**iptables example:**

```bash
# Allow gRPC from application servers only
iptables -A INPUT -p tcp --dport 50051 -s 10.0.1.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 50051 -j DROP

# Allow dashboard from VPN only
iptables -A INPUT -p tcp --dport 8081 -s 10.0.100.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 8081 -j DROP

# Allow Prometheus from monitoring network
iptables -A INPUT -p tcp --dport 9090 -s 10.0.200.0/24 -j ACCEPT
iptables -A INPUT -p tcp --dport 9090 -j DROP
```

### Network Isolation

Use separate networks:

```yaml
# docker-compose.yml
version: '3.8'
services:
  turbine-server:
    networks:
      - app-network  # Application access
      - metrics-network  # Monitoring access
    ports:
      - "127.0.0.1:50051:50051"  # Bind to localhost only

networks:
  app-network:
    internal: false
  metrics-network:
    internal: true
```

## Redis Security

### Authentication

```bash
# Redis with password
export TURBINE_BROKER_URL=redis://:strongpassword@localhost:6379

# Redis with SSL
export TURBINE_BROKER_URL=rediss://:password@localhost:6380
```

### Redis ACLs

```bash
# Create dedicated Turbine user
redis-cli ACL SETUSER turbine on >strongpassword ~turbine:* +@all -@dangerous

# Use in connection
export TURBINE_BROKER_URL=redis://turbine:strongpassword@localhost:6379
```

### Redis Isolation

```toml
# Use separate Redis database
[broker]
url = "redis://localhost:6379/1"  # Database 1

[backend]
url = "redis://localhost:6379/2"  # Database 2
```

## Data Security

### Sensitive Data Handling

**DON'T store secrets in tasks:**

```python
# ❌ BAD - Secret in task arguments
@task
def send_email(api_key, to, subject):
    client = EmailClient(api_key)
    client.send(to, subject)

# ✅ GOOD - Secret from environment
@task
def send_email(to, subject):
    import os
    api_key = os.environ['EMAIL_API_KEY']
    client = EmailClient(api_key)
    client.send(to, subject)
```

### Encryption at Rest

**Redis:**

```bash
# Use Redis with disk encryption (LUKS, dm-crypt)
# Or encrypt backups

# Redis config
requirepass strongpassword
```

**PostgreSQL:**

```sql
-- Use PostgreSQL with transparent data encryption
-- Or encrypt tablespace
```

**S3:**

```python
from turbine.backends import S3Backend

backend = S3Backend(
    bucket="encrypted-results",
    # S3 server-side encryption enabled via bucket policy
)
```

### PII and GDPR Compliance

```python
# Use short result TTL for PII
@task(result_ttl=3600)  # 1 hour only
def process_user_data(user_data):
    # Process PII
    return results

# Or disable result storage
@task(store_result=False)
def process_sensitive_data(data):
    # Process and don't store result
    pass
```

## Multi-Tenancy Security

### Tenant Isolation

```python
from turbine.tenancy import TenantManager

manager = TenantManager()

# Validate tenant before operations
def process_tenant_task(tenant_id, data):
    # 1. Validate tenant exists and is enabled
    tenant = manager.get_tenant(tenant_id)
    if not tenant or not tenant.enabled:
        raise ValueError("Invalid or disabled tenant")

    # 2. Check quotas
    allowed, error = manager.check_quota(tenant_id, "tasks_per_hour")
    if not allowed:
        raise ValueError(error)

    # 3. Submit with tenant_id
    result = my_task.apply_async(
        args=[data],
        tenant_id=tenant_id
    )

    # 4. Track usage
    manager.increment_usage(tenant_id, "tasks_hour", ttl=3600)

    return result
```

### Tenant Quotas as Security

```python
# Prevent abuse with strict quotas
abuse_prevention_quotas = TenantQuotas(
    max_tasks_per_hour=100,
    max_task_size_bytes=102400,  # 100KB max
    max_result_size_bytes=524288,  # 500KB max
    allowed_queues=["default"],  # Restrict queue access
    max_retry_count=3,  # Limit retries
)
```

## Rate Limiting

### Global Rate Limiting

```toml
[ratelimit]
enabled = true
default_rate = 1000  # tasks per period
default_period = 60  # seconds
```

### Per-Tenant Rate Limiting

```python
from turbine.routing import RateLimiter

limiter = RateLimiter(
    redis_url="redis://localhost:6379",
    max_rate=100,
    period=60
)

# Check rate limit before submission
allowed, remaining = limiter.check_rate_limit(f"tenant:{tenant_id}")

if not allowed:
    raise Exception("Rate limit exceeded")

my_task.apply_async(args=[data])
```

## Input Validation

### Validate Task Arguments

```python
@task
def process_user_input(data: dict):
    # Validate input
    if not isinstance(data, dict):
        raise ValueError("Invalid data type")

    # Sanitize
    email = data.get("email", "").strip().lower()
    if not validate_email(email):
        raise ValueError("Invalid email")

    # Size limits
    if len(json.dumps(data)) > 100_000:
        raise ValueError("Data too large")

    # Process safely
    return process(data)
```

### Size Limits

```python
# Enforce in tenant quotas
quotas = TenantQuotas(
    max_task_size_bytes=1048576,  # 1MB
    max_result_size_bytes=5242880,  # 5MB
)

# Or validate in task
@task
def limited_task(data):
    import sys
    size = sys.getsizeof(data)

    if size > 1_000_000:
        raise ValueError(f"Data too large: {size} bytes")

    return process(data)
```

## Audit Logging

### Enable Audit Logs

```python
import logging

# Configure audit logger
audit_logger = logging.getLogger('turbine.audit')
audit_logger.setLevel(logging.INFO)

handler = logging.FileHandler('/var/log/turbine/audit.log')
handler.setFormatter(logging.Formatter(
    '%(asctime)s - %(name)s - %(levelname)s - %(message)s'
))
audit_logger.addHandler(handler)

# Log sensitive operations
def submit_task(tenant_id, task_name, args):
    audit_logger.info(
        f"Task submitted: tenant={tenant_id}, task={task_name}, "
        f"user={get_current_user()}"
    )

    return task.apply_async(args=args, tenant_id=tenant_id)
```

### Structured Audit Logs

```python
import json
import logging

def audit_log(event: str, **context):
    """Structured audit logging."""
    log_entry = {
        "event": event,
        "timestamp": datetime.utcnow().isoformat(),
        "user": context.get("user"),
        "tenant": context.get("tenant"),
        "resource": context.get("resource"),
        "action": context.get("action"),
        "result": context.get("result"),
    }

    audit_logger.info(json.dumps(log_entry))

# Usage
audit_log(
    "task_submitted",
    user="user@example.com",
    tenant="acme-corp",
    resource="task:send_email",
    action="submit",
    result="success"
)
```

## Secrets Management

### Environment Variables

```python
import os

# Load from environment
broker_url = os.environ['TURBINE_BROKER_URL']
api_key = os.environ['API_KEY']

# Never hardcode
# ❌ broker_url = "redis://:password@localhost"
```

### Secret Managers

**AWS Secrets Manager:**

```python
import boto3
import json

def get_secret(secret_name):
    client = boto3.client('secretsmanager', region_name='us-east-1')
    response = client.get_secret_value(SecretId=secret_name)
    return json.loads(response['SecretString'])

# Load configuration
config = get_secret('turbine/config')
broker_url = config['broker_url']
```

**HashiCorp Vault:**

```python
import hvac

client = hvac.Client(url='http://vault:8200')
client.auth.approle.login(
    role_id=os.environ['VAULT_ROLE_ID'],
    secret_id=os.environ['VAULT_SECRET_ID'],
)

secret = client.secrets.kv.v2.read_secret_version(path='turbine/config')
broker_url = secret['data']['data']['broker_url']
```

## Security Checklist

### Deployment Checklist

- [ ] TLS enabled for gRPC
- [ ] mTLS enabled for client authentication
- [ ] Redis password authentication enabled
- [ ] Redis in private network
- [ ] Firewall rules configured
- [ ] Non-root user for services
- [ ] File permissions restricted (600 for certs)
- [ ] Secrets in environment variables or secret manager
- [ ] Audit logging enabled
- [ ] Regular security updates
- [ ] Monitoring and alerting configured
- [ ] Backup and recovery tested

### Code Review Checklist

- [ ] No hardcoded credentials
- [ ] Input validation on all tasks
- [ ] Size limits enforced
- [ ] Tenant validation on multi-tenant tasks
- [ ] Rate limiting implemented
- [ ] Error messages don't leak sensitive info
- [ ] Sensitive data not logged
- [ ] Dependencies up to date (no CVEs)

## Incident Response

### Suspected Compromise

1. **Isolate**: Disconnect compromised components
2. **Rotate**: Rotate all credentials immediately
3. **Audit**: Review audit logs for unauthorized access
4. **Patch**: Update all components
5. **Monitor**: Watch for suspicious activity

### Commands

```bash
# Revoke all active tasks
turbine queues purge-all

# Disable compromised tenant
turbine tenant update compromised-tenant --enabled false

# Clear DLQ
turbine dlq clear --force

# Restart workers with new credentials
systemctl restart turbine-worker
```

## Compliance

### GDPR

```python
# Right to be forgotten
@task
def delete_user_data(user_id):
    # Delete user's task results
    # Clear from DLQ
    # Remove from audit logs
    pass

# Data retention limits
@task(result_ttl=2592000)  # 30 days
def process_user_data(user_id, data):
    pass
```

### SOC 2

- Enable audit logging (all operations)
- Encrypt data in transit (TLS)
- Encrypt data at rest (Redis/PostgreSQL encryption)
- Access controls (multi-tenancy)
- Monitoring and alerting
- Regular security reviews

### HIPAA

- Use mTLS for all connections
- Encrypt Redis and PostgreSQL
- Audit all PHI access
- Short result TTL (delete after use)
- Secure network isolation
- Regular security training

## Common Vulnerabilities

### 1. Command Injection

**Vulnerable:**

```python
@task
def run_command(cmd):
    import os
    os.system(cmd)  # ❌ DANGEROUS
```

**Secure:**

```python
@task
def run_command(action):
    # Whitelist allowed actions
    allowed_actions = {
        'backup': lambda: subprocess.run(['./backup.sh'], check=True),
        'cleanup': lambda: subprocess.run(['./cleanup.sh'], check=True),
    }

    if action not in allowed_actions:
        raise ValueError("Action not allowed")

    allowed_actions[action]()
```

### 2. SQL Injection

**Vulnerable:**

```python
@task
def query_user(username):
    import psycopg2
    conn = psycopg2.connect(DSN)
    cur = conn.cursor()
    cur.execute(f"SELECT * FROM users WHERE username = '{username}'")  # ❌
```

**Secure:**

```python
@task
def query_user(username):
    import psycopg2
    conn = psycopg2.connect(DSN)
    cur = conn.cursor()
    cur.execute("SELECT * FROM users WHERE username = %s", (username,))  # ✅
```

### 3. Path Traversal

**Vulnerable:**

```python
@task
def read_file(filename):
    with open(f"/data/{filename}") as f:  # ❌ filename could be "../../../etc/passwd"
        return f.read()
```

**Secure:**

```python
@task
def read_file(filename):
    import os
    from pathlib import Path

    # Validate filename
    if ".." in filename or filename.startswith("/"):
        raise ValueError("Invalid filename")

    # Use safe path joining
    base_dir = Path("/data")
    file_path = (base_dir / filename).resolve()

    # Ensure it's within base directory
    if not str(file_path).startswith(str(base_dir)):
        raise ValueError("Path traversal detected")

    with open(file_path) as f:
        return f.read()
```

### 4. Denial of Service

**Prevent with:**

- Rate limiting (per-tenant)
- Task size limits
- Timeout configuration
- Queue length limits
- Worker resource limits

```python
from turbine.tenancy import TenantQuotas

# Strict DoS prevention quotas
quotas = TenantQuotas(
    max_tasks_per_hour=1000,
    max_concurrent_tasks=10,
    max_queue_length=100,
    max_task_size_bytes=102400,  # 100KB
)
```

## Secrets in Task Results

### Problem

```python
@task
def call_external_api(api_key, endpoint):
    response = requests.get(endpoint, headers={"Authorization": f"Bearer {api_key}"})
    # ❌ api_key is now in task arguments (stored in Redis)
    return response.json()
```

### Solution

```python
@task(store_result=False)  # Don't store result with secret
def call_external_api(endpoint):
    import os
    api_key = os.environ['API_KEY']  # Load from environment
    response = requests.get(endpoint, headers={"Authorization": f"Bearer {api_key}"})
    # Process and return non-sensitive data only
    return {"status": response.status_code}
```

## Monitoring for Security

### Alert on Suspicious Activity

```yaml
# Prometheus alerts
groups:
  - name: security
    rules:
      - alert: HighFailureRate
        expr: rate(turbine_tasks_total{state="failed"}[5m]) > 100
        annotations:
          summary: "Unusually high task failure rate"

      - alert: UnauthorizedAccess
        expr: rate(turbine_auth_failures_total[5m]) > 10
        annotations:
          summary: "Multiple authentication failures"

      - alert: QuotaViolation
        expr: rate(turbine_quota_exceeded_total[5m]) > 50
        annotations:
          summary: "Frequent quota violations"
```

### Security Metrics

```python
# Track security events
from prometheus_client import Counter

auth_failures = Counter(
    'turbine_auth_failures_total',
    'Total authentication failures',
    ['tenant_id']
)

quota_exceeded = Counter(
    'turbine_quota_exceeded_total',
    'Quota exceeded events',
    ['tenant_id', 'quota_type']
)

# Increment on security events
auth_failures.labels(tenant_id=tenant_id).inc()
quota_exceeded.labels(tenant_id=tenant_id, quota_type='tasks_per_hour').inc()
```

## Security Updates

### Keep Dependencies Updated

```bash
# Check for vulnerabilities
pip list --outdated
cargo audit

# Update regularly
pip install --upgrade turbine-queue
cargo update
```

### Subscribe to Security Advisories

- [GitHub Security Advisories](https://github.com/turbine-queue/turbine/security/advisories)
- [RustSec Advisory Database](https://rustsec.org/)
- [PyPI Security Notifications](https://pypi.org/security/)

## Deployment Security

### Systemd Hardening

```ini
[Service]
# Run as dedicated user
User=turbine
Group=turbine

# Security hardening
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/var/lib/turbine
ProtectKernelTunables=true
ProtectControlGroups=true
RestrictRealtime=true
RestrictNamespaces=true
LockPersonality=true
MemoryDenyWriteExecute=true
RestrictAddressFamilies=AF_UNIX AF_INET AF_INET6
SystemCallFilter=@system-service
SystemCallErrorNumber=EPERM
```

### Docker Security

```dockerfile
# Use non-root user
FROM rust:1.75 as builder
# ... build ...

FROM debian:12-slim
RUN useradd -r -u 1000 turbine
USER turbine

# Read-only filesystem
VOLUME /data
WORKDIR /app
COPY --from=builder /build/turbine-server /app/

# Drop capabilities
ENTRYPOINT ["/app/turbine-server"]
```

```yaml
# docker-compose.yml
services:
  turbine-server:
    security_opt:
      - no-new-privileges:true
    cap_drop:
      - ALL
    cap_add:
      - NET_BIND_SERVICE
    read_only: true
    tmpfs:
      - /tmp
```

## Reporting Security Issues

**DO NOT** open public GitHub issues for security vulnerabilities.

Instead:
- Email: security@turbine-queue.org (coming soon)
- Use GitHub Security Advisories (private)
- Expect response within 48 hours

## References

- [OWASP Top 10](https://owasp.org/www-project-top-ten/)
- [CWE Top 25](https://cwe.mitre.org/top25/)
- [Rust Security Guidelines](https://anssi-fr.github.io/rust-guide/)
- [gRPC Security](https://grpc.io/docs/guides/auth/)
