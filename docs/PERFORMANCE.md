# Turbine Performance Tuning Guide

Optimize Turbine for maximum throughput, minimum latency, and efficient resource usage.

## Benchmarking Basics

### Measure First

Before optimizing, establish baseline metrics:

```python
import time
from turbine import Turbine, task

app = Turbine(server="localhost:50051")

@task
def benchmark_task(x):
    return x * 2

# Measure throughput
start = time.time()
results = [benchmark_task.delay(i) for i in range(10000)]
submit_time = time.time() - start

print(f"Submission: {10000/submit_time:.0f} tasks/sec")

# Measure latency
start = time.time()
result = benchmark_task.delay(42)
value = result.get(timeout=10)
latency = time.time() - start

print(f"End-to-end latency: {latency*1000:.0f}ms")
```

### Key Metrics

| Metric | Target | Measurement |
|--------|--------|-------------|
| Submission rate | >1000 tasks/sec | Time to submit 10k tasks |
| Task latency | <100ms (p95) | Task submission to completion |
| Worker throughput | >100 tasks/sec/worker | Tasks processed per worker |
| Memory per worker | <100MB | RSS memory usage |
| Queue lag | <5 seconds | Time tasks wait in queue |

## Server Optimization

### Configuration Tuning

```toml
# High-throughput configuration
[broker]
url = "redis://localhost:6379"
pool_size = 50  # Increase for more concurrent connections
prefetch = 20   # Fetch multiple tasks per request
connection_timeout = 10

[server]
host = "0.0.0.0"
grpc_port = 50051
max_request_size = 4194304  # 4MB (reduce if tasks are small)

# Enable HTTP/2 keep-alive for gRPC
# Configured automatically
```

### Redis Optimization

**redis.conf:**

```bash
# Memory
maxmemory 2gb
maxmemory-policy allkeys-lru  # Evict old results

# Network
tcp-backlog 511
timeout 0
tcp-keepalive 300

# Persistence (if needed)
save 900 1
save 300 10
save 60 10000

# Or disable for pure cache
save ""

# Performance
io-threads 4  # Redis 6+
io-threads-do-reads yes
```

### Use Redis Pipelining

Tasks are automatically submitted in batches internally for efficiency.

## Worker Optimization

### Concurrency Tuning

```python
import multiprocessing

# CPU-bound tasks
concurrency = multiprocessing.cpu_count()

# I/O-bound tasks
concurrency = multiprocessing.cpu_count() * 4

# Mixed workload
concurrency = multiprocessing.cpu_count() * 2
```

**Test and measure:**

```bash
# Try different concurrency levels
for c in 2 4 8 16 32; do
    echo "Testing concurrency=$c"
    turbine worker -c $c -Q test &
    WORKER_PID=$!

    # Submit test workload
    python benchmark.py

    # Kill worker
    kill $WORKER_PID
done
```

### Prefetch Configuration

```python
# Low concurrency + high prefetch = good for I/O-bound
config = WorkerConfig(
    concurrency=4,
    prefetch_count=10  # Fetch 10 tasks ahead
)

# High concurrency + low prefetch = good for CPU-bound
config = WorkerConfig(
    concurrency=16,
    prefetch_count=1  # Fetch one at a time
)
```

### Worker Specialization

```bash
# Fast tasks worker
turbine worker -Q fast -c 16 &

# Slow tasks worker
turbine worker -Q slow -c 4 &

# I/O-bound worker
turbine worker -Q io -c 32 &

# CPU-bound worker
turbine worker -Q cpu -c 8 &
```

## Task Optimization

### Minimize Task Size

**❌ Large task (slow):**

```python
@task
def process_data(large_dataset):  # 10MB in task args
    return process(large_dataset)

# Submission is slow (serialize + network + Redis)
```

**✅ Small task (fast):**

```python
@task
def process_data(file_path):  # <100 bytes
    with open(file_path, 'rb') as f:
        large_dataset = f.read()
    return process(large_dataset)

# Submission is fast
```

### Avoid Heavy Imports in Global Scope

**❌ Slow worker startup:**

```python
import tensorflow as tf  # ❌ Loads on import
import pandas as pd      # ❌ Takes 1-2 seconds

@task
def ml_task(data):
    # Use tf, pd
    pass
```

**✅ Lazy imports:**

```python
@task
def ml_task(data):
    import tensorflow as tf  # ✅ Load only when needed
    import pandas as pd
    # Use tf, pd
    pass
```

### Result Size Optimization

```python
# Return minimal data
@task
def fetch_users():
    users = User.objects.all()

    # ❌ Return full objects (large)
    # return list(users)

    # ✅ Return IDs only (small)
    return [u.id for u in users]

    # ✅ Or specific fields
    return [{"id": u.id, "email": u.email} for u in users]
```

## Workflow Optimization

### Parallel > Sequential

```python
# ❌ Sequential (slow)
result1 = task1.delay()
value1 = result1.get()

result2 = task2.delay()
value2 = result2.get()

result3 = task3.delay()
value3 = result3.get()

# ✅ Parallel (fast)
from turbine import group

workflow = group(task1.s(), task2.s(), task3.s())
workflow_id, task_ids = workflow.delay()

# Or use parallel helper
from turbine.dag import parallel
results = parallel(task1.s(), task2.s(), task3.s(), wait=True)
```

### Batch Processing

```python
# ❌ 10,000 individual tasks
for item in large_dataset:
    process_item.delay(item)

# ✅ 100 batch tasks (100 items each)
from turbine.batch import batch_map

results = batch_map(
    process_item,
    large_dataset,
    batch_size=100
)

# 100x less overhead
```

### Chain Optimization

```python
# ❌ Long chains are fragile
chain(task1.s(), task2.s(), task3.s(), ..., task50.s())

# ✅ Group related tasks
@task
def combined_task():
    # Combine task1, task2, task3 logic
    pass

chain(combined_task.s(), other_tasks.s())
```

## Database Optimization

### Connection Pooling

```python
# ❌ Create connection per task
@task
def query_data():
    conn = create_db_connection()  # Slow
    result = conn.query("SELECT ...")
    conn.close()
    return result

# ✅ Use connection pool
from sqlalchemy import create_engine

engine = create_engine(
    DATABASE_URL,
    pool_size=20,
    max_overflow=40,
    pool_pre_ping=True,
)

@task
def query_data():
    with engine.connect() as conn:  # Fast - reuses connection
        result = conn.execute("SELECT ...")
    return result
```

### Django ORM Optimization

```python
@task
def process_users():
    # ❌ N+1 queries
    users = User.objects.all()
    for user in users:
        profile = user.profile  # Query per user
        process(profile)

    # ✅ Eager loading
    users = User.objects.select_related('profile').all()
    for user in users:
        process(user.profile)  # No additional query
```

## Compression Tuning

### When to Use Compression

Use when:
- Results > 10KB
- Text/JSON data (compresses well)
- Network or Redis is bottleneck

Don't use when:
- Results < 1KB (overhead not worth it)
- Already compressed data (images, videos)
- CPU is bottleneck

### Choose Compression Algorithm

| Algorithm | Compression | Speed | Use Case |
|-----------|-------------|-------|----------|
| gzip | Good (60-70%) | Fast | General purpose |
| zlib | Good (60-70%) | Fast | Similar to gzip |
| brotli | Best (70-80%) | Slower | Text/JSON, worth the CPU |
| lz4 | Lower (50-60%) | Fastest | When speed > size |

```python
# Configure worker compression
config = WorkerConfig(
    result_compression=True,
    result_compression_min_size=10240,  # 10KB threshold
    result_compression_type="lz4",  # Fastest for high-throughput
)
```

## Memory Optimization

### Worker Memory Limits

```toml
[worker]
max_memory_mb = 512  # Restart if exceeds
max_tasks_per_worker = 5000  # Periodic restart
```

### Result TTL

```python
# Short TTL = less memory
@task(result_ttl=3600)  # 1 hour
def temporary_task():
    pass

# Or disable completely
@task(store_result=False)
def no_result_task():
    pass
```

### Avoid Memory Leaks

```python
# ❌ Accumulates memory
global_cache = {}  # ❌ Grows forever

@task
def cached_task(key):
    if key not in global_cache:
        global_cache[key] = fetch_data(key)
    return global_cache[key]

# ✅ Use LRU cache
from functools import lru_cache

@lru_cache(maxsize=1000)
def fetch_data(key):
    return expensive_operation(key)

@task
def cached_task(key):
    return fetch_data(key)
```

## Network Optimization

### Reduce Round Trips

```python
# ❌ Multiple round trips
result1 = task1.delay()
result2 = task2.delay()
result3 = task3.delay()

# ✅ Single batch submission
from turbine import group

workflow = group(task1.s(), task2.s(), task3.s())
workflow_id, task_ids = workflow.delay()  # Single RPC call
```

### Use Local Redis

```bash
# ❌ Remote Redis (high latency)
export TURBINE_BROKER_URL=redis://remote-server:6379

# ✅ Local Redis (low latency)
export TURBINE_BROKER_URL=redis://localhost:6379

# Or use Unix socket (lowest latency)
export TURBINE_BROKER_URL=redis://localhost:6379?unix_socket_path=/var/run/redis/redis.sock
```

## Profiling

### Profile Tasks

```python
import cProfile
import pstats

@task
def profiled_task(data):
    profiler = cProfile.Profile()
    profiler.enable()

    result = do_work(data)

    profiler.disable()

    # Print stats
    stats = pstats.Stats(profiler)
    stats.sort_stats('cumulative')
    stats.print_stats(10)  # Top 10

    return result
```

### Profile Worker

```bash
# Run worker with profiling
python -m cProfile -o worker.prof turbine worker -Q default

# Analyze
python -m pstats worker.prof
>>> sort cumulative
>>> stats 20
```

### Memory Profiling

```python
from memory_profiler import profile

@task
@profile
def memory_intensive_task():
    # Will log memory usage line by line
    large_list = [i for i in range(1000000)]
    processed = [x * 2 for x in large_list]
    return sum(processed)
```

## Benchmarking Tools

### Load Testing

```python
# load_test.py
import time
from turbine import Turbine, task

app = Turbine(server="localhost:50051")

@task
def noop():
    pass

def load_test(num_tasks=10000):
    start = time.time()

    # Submit tasks
    results = []
    for i in range(num_tasks):
        result = noop.delay()
        results.append(result)

    submit_time = time.time() - start

    # Wait for completion
    for result in results:
        result.get(timeout=60)

    total_time = time.time() - start

    print(f"Submitted {num_tasks} tasks in {submit_time:.2f}s")
    print(f"Submission rate: {num_tasks/submit_time:.0f} tasks/sec")
    print(f"Total time: {total_time:.2f}s")
    print(f"Throughput: {num_tasks/total_time:.0f} tasks/sec")

if __name__ == "__main__":
    load_test(10000)
```

### Latency Measurement

```python
import time
import statistics

def measure_latency(num_samples=100):
    latencies = []

    for i in range(num_samples):
        start = time.time()
        result = benchmark_task.delay(i)
        value = result.get(timeout=10)
        latency = time.time() - start
        latencies.append(latency * 1000)  # Convert to ms

    print(f"Latency statistics:")
    print(f"  p50: {statistics.median(latencies):.1f}ms")
    print(f"  p95: {statistics.quantiles(latencies, n=20)[18]:.1f}ms")
    print(f"  p99: {statistics.quantiles(latencies, n=100)[98]:.1f}ms")
    print(f"  max: {max(latencies):.1f}ms")

measure_latency()
```

## Performance Comparison: Celery vs Turbine

### Memory Usage

**Celery:**
```bash
# 10 workers × ~450MB = 4.5GB
$ ps aux | grep celery
celery  1234  450MB
celery  1235  448MB
...
```

**Turbine (Rust workers):**
```bash
# 10 workers × ~45MB = 450MB (10x improvement)
$ ps aux | grep turbine-worker
turbine  5678  45MB
turbine  5679  44MB
...
```

**Turbine (Python workers):**
```bash
# 10 workers × ~120MB = 1.2GB (3-4x improvement)
$ ps aux | grep "turbine worker"
turbine  9012  120MB
turbine  9013  118MB
...
```

### Throughput

**Celery:**
- Submission: ~500-1000 tasks/sec
- Processing: ~50-100 tasks/sec/worker

**Turbine:**
- Submission: ~5000-10000 tasks/sec (gRPC is faster)
- Processing (Rust): ~500-1000 tasks/sec/worker
- Processing (Python): ~100-200 tasks/sec/worker

### Latency

**Celery:**
- p50: ~50ms
- p95: ~200ms
- p99: ~500ms

**Turbine:**
- p50: ~20ms (gRPC overhead is lower)
- p95: ~100ms
- p99: ~250ms

## Optimization Checklist

### Quick Wins

1. **Enable Compression** - Saves Redis memory
   ```python
   config.result_compression = True
   ```

2. **Increase Concurrency** - More parallel tasks
   ```python
   config.concurrency = 16
   ```

3. **Batch Similar Tasks** - Reduces overhead
   ```python
   from turbine.batch import batch_map
   batch_map(task, items, batch_size=100)
   ```

4. **Use Local Redis** - Lower latency
   ```bash
   export TURBINE_BROKER_URL=redis://localhost:6379
   ```

5. **Disable Result Storage** - When not needed
   ```python
   @task(store_result=False)
   def fire_and_forget():
       pass
   ```

### Advanced Optimizations

6. **Queue Partitioning** - Horizontal scaling
   ```python
   # 16 partition queues
   queue = f"partition-{user_id % 16}"
   ```

7. **Hybrid Backend** - S3 for large results
   ```python
   backend = HybridBackend(size_threshold=1048576)
   ```

8. **Connection Pooling** - Reuse DB connections
   ```python
   engine = create_engine(url, pool_size=20)
   ```

9. **Result TTL Tuning** - Shorter retention
   ```python
   @task(result_ttl=3600)  # 1 hour
   ```

10. **Lazy Imports** - Faster worker startup
    ```python
    @task
    def ml_task():
        import tensorflow as tf  # Import only when needed
    ```

## Monitoring Performance

### Prometheus Queries

```promql
# Task throughput
rate(turbine_tasks_total[5m])

# Queue lag
turbine_queue_pending_tasks / rate(turbine_tasks_total[5m])

# Worker utilization
turbine_worker_busy_count / turbine_worker_total_count

# p95 latency
histogram_quantile(0.95, rate(turbine_task_duration_seconds_bucket[5m]))
```

### Grafana Dashboards

Use provided dashboard: `grafana/turbine-overview.json`

Key panels:
- Task throughput over time
- Latency percentiles
- Queue depths
- Worker utilization
- Memory usage

## Troubleshooting Performance Issues

### Problem: High Latency

**Symptoms:**
- Tasks take >1s to complete
- p95 latency >500ms

**Solutions:**

1. Check queue depth:
   ```bash
   turbine queues
   ```
   If high, add more workers

2. Check Redis latency:
   ```bash
   redis-cli --latency
   ```
   If high, optimize Redis or use local instance

3. Profile slow tasks:
   ```python
   @task
   def slow_task():
       import time
       start = time.time()
       # ... work ...
       print(f"Duration: {time.time() - start:.3f}s")
   ```

### Problem: Low Throughput

**Symptoms:**
- <100 tasks/sec processing
- Workers mostly idle

**Solutions:**

1. Increase prefetch:
   ```python
   config.prefetch_count = 10
   ```

2. Increase concurrency:
   ```python
   config.concurrency = 16
   ```

3. Use batch processing:
   ```python
   from turbine.batch import batch_map
   ```

### Problem: High Memory Usage

**Symptoms:**
- Workers using >1GB RAM
- Redis using >4GB RAM

**Solutions:**

1. Enable compression:
   ```python
   config.result_compression = True
   ```

2. Shorter TTL:
   ```python
   @task(result_ttl=3600)
   ```

3. Worker restarts:
   ```python
   config.max_memory_mb = 512
   config.max_tasks_per_worker = 5000
   ```

4. Redis maxmemory:
   ```bash
   redis-cli CONFIG SET maxmemory 2gb
   redis-cli CONFIG SET maxmemory-policy allkeys-lru
   ```

### Problem: Queue Backlog

**Symptoms:**
- Pending tasks growing
- Tasks waiting minutes

**Solutions:**

1. Add more workers:
   ```bash
   for i in {1..5}; do
       turbine worker -Q backlogged &
   done
   ```

2. Increase concurrency:
   ```bash
   turbine worker -c 32
   ```

3. Partition queue:
   ```python
   # Split backlogged queue into 4 partitions
   for i in range(4):
       start_worker(queues=[f"backlogged-{i}"])
   ```

## Performance Testing Strategy

### 1. Baseline Test

```python
# Measure current performance
def baseline_test():
    # 10k simple tasks
    # Measure: submission rate, latency, throughput
    pass
```

### 2. Load Test

```python
# Test under heavy load
def load_test():
    # Submit 100k tasks
    # Monitor: queue depth, worker CPU, Redis memory
    pass
```

### 3. Stress Test

```python
# Test failure modes
def stress_test():
    # Submit 1M tasks
    # Kill workers randomly
    # Verify: no data loss, graceful degradation
    pass
```

### 4. Soak Test

```python
# Test long-term stability
def soak_test():
    # Run for 24 hours
    # Submit steady rate (1000 tasks/min)
    # Monitor: memory leaks, connection leaks
    pass
```

## Production Performance Targets

### Recommended Targets

| Metric | Target | Notes |
|--------|--------|-------|
| Task latency (p95) | <100ms | For fast tasks |
| Task latency (p99) | <500ms | Acceptable ceiling |
| Worker throughput | >100 tasks/sec | Per worker |
| Queue lag | <5 seconds | Time waiting in queue |
| Success rate | >99% | Failed tasks to DLQ |
| Worker memory | <200MB | Per Python worker |
| Worker memory | <50MB | Per Rust worker |
| Redis memory | <4GB | Per 1M tasks/day |

### When to Scale

**Scale Out (more workers) when:**
- Queue depth consistently > 100
- Queue lag > 10 seconds
- Worker CPU < 50% (need more workers, not more threads)

**Scale Up (bigger workers) when:**
- Worker CPU consistently > 80%
- Memory pressure
- I/O bottleneck

## See Also

- [Configuration Guide](./CONFIGURATION.md)
- [Best Practices](./BEST_PRACTICES.md)
- [Security Guide](./SECURITY.md)
