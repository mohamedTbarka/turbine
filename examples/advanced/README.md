# Advanced Turbine Examples

This directory contains advanced examples demonstrating Turbine's powerful features.

## Prerequisites

```bash
# Install Turbine with all optional dependencies
pip install turbine-queue[worker]

# Start Redis
redis-server

# Start Turbine server
./target/release/turbine-server

# Start Python worker (in another terminal)
cd examples/advanced
turbine worker -I batch_processing_example -I dag_example -I routing_example
```

## Examples

### 1. Batch Processing (`batch_processing_example.py`)

Demonstrates efficient processing of large datasets:

- **Simple Batch**: Process 100 items in batches of 20
- **Progress Tracking**: Monitor batch processing progress
- **Map-Reduce**: Parallel processing with aggregation
- **Batcher Accumulator**: Auto-submit when batch is full
- **Starmap**: Process tuples of multiple arguments

**Run:**
```bash
python batch_processing_example.py 1    # Simple batch
python batch_processing_example.py 2    # With progress
python batch_processing_example.py 3    # Map-reduce
python batch_processing_example.py all  # Run all examples
```

**Use Cases:**
- Processing uploaded files in bulk
- Data migrations
- Report generation for many users
- Bulk email sending
- ETL pipelines

### 2. Task Dependencies - DAG (`dag_example.py`)

Demonstrates complex task dependency graphs:

- **Linear DAG**: Sequential task pipeline
- **Parallel Branches**: Tasks that run in parallel then merge
- **Diamond DAG**: Fan-out and fan-in pattern
- **Conditional DAG**: Execute based on previous results
- **Parallel Helper**: Simple parallel execution

**Run:**
```bash
python dag_example.py 1    # Linear pipeline
python dag_example.py 2    # Parallel branches
python dag_example.py 3    # Diamond DAG
python dag_example.py all  # Run all examples
```

**Use Cases:**
- ETL pipelines with stages
- Machine learning workflows
- Report generation with multiple data sources
- Multi-step data validation
- Complex business logic flows

### 3. Routing & Load Balancing (`routing_example.py`)

Demonstrates task routing strategies:

- **Round-Robin**: Distribute evenly across queues
- **Hash Routing**: Consistent routing by key (user_id, tenant_id)
- **Load Balancing**: Route to least loaded queue
- **Consistent Hashing**: Partition work consistently
- **Rate Limiting**: Limit submission rate
- **Tenant Routing**: Route with quota enforcement

**Run:**
```bash
python routing_example.py 1    # Round-robin
python routing_example.py 2    # Hash routing
python routing_example.py 3    # Load balancing
python routing_example.py all  # Run all examples
```

**Use Cases:**
- Multi-tenant SaaS applications
- User-based partitioning
- Geographic routing
- Priority-based routing
- Rate-limited APIs
- Resource quota enforcement

## Architecture Patterns

### Pattern 1: ETL Pipeline with DAG

```python
from turbine import task
from turbine.dag import DAG

@task
def extract(source):
    # Extract data from source
    return raw_data

@task
def transform(data):
    # Transform data
    return transformed_data

@task
def load(data):
    # Load into destination
    return success

# Build pipeline
dag = DAG("etl-pipeline")
extract_id = dag.add_task(extract, args=["source.csv"])
transform_id = dag.add_task(transform, dependencies=[extract_id])
load_id = dag.add_task(load, dependencies=[transform_id])

# Execute
results = dag.execute(wait=True)
```

### Pattern 2: Batch Processing with Compression

```python
from turbine import task
from turbine.batch import BatchProcessor

@task(queue="processing")
def process_large_file(file_id):
    # Process file and return large result
    return large_data

# Process 1000 files with compression
processor = BatchProcessor(
    process_large_file,
    chunk_size=50,
    max_concurrent=10
)

# Worker will auto-compress results > 1KB
results = processor.map(file_ids)
```

### Pattern 3: Multi-Tenant with Load Balancing

```python
from turbine import task
from turbine.routing import LoadBalancer
from turbine.tenancy import TenantManager

@task
def tenant_task(data):
    return processed_data

balancer = LoadBalancer(app, queues=["tenant-q1", "tenant-q2"])
tenant_mgr = TenantManager()

def submit_tenant_task(tenant_id, data):
    # Check quota
    allowed, error = tenant_mgr.check_quota(tenant_id, "tasks_per_hour")
    if not allowed:
        raise Exception(error)

    # Increment usage
    tenant_mgr.increment_usage(tenant_id, "tasks_hour", ttl=3600)

    # Load balance submission
    return balancer.route_task(
        tenant_task,
        args=[data],
        strategy="least_loaded"
    )
```

### Pattern 4: Fan-Out/Fan-In with Error Handling

```python
from turbine import task, chord

@task
def split_work(data):
    return [chunk1, chunk2, chunk3]

@task
def process_chunk(chunk):
    # May fail for some chunks
    if chunk.is_invalid():
        raise ValueError("Invalid chunk")
    return process(chunk)

@task
def aggregate(results):
    # Filter out None (failed tasks)
    valid_results = [r for r in results if r is not None]
    return combine(valid_results)

# Execute with error tolerance
split_result = split_work.delay(large_data)
chunks = split_result.get()

# Process chunks with chord
workflow = chord(
    [process_chunk.s(chunk) for chunk in chunks],
    aggregate.s()
)

final_result = workflow.delay()
```

## Performance Tips

### 1. Batch Size Selection

- **Small batches (10-50)**: Better parallelism, more overhead
- **Medium batches (50-200)**: Good balance
- **Large batches (200+)**: Less overhead, less parallelism

Choose based on task duration:
- Fast tasks (<100ms): Use larger batches
- Slow tasks (>1s): Use smaller batches

### 2. Queue Strategy

- **Single queue**: Simple, but can bottleneck
- **Per-type queues**: Better isolation (emails, processing, etc.)
- **Partition queues**: Scale horizontally (user-0, user-1, etc.)
- **Priority queues**: Different SLAs (high, medium, low)

### 3. Compression

Enable compression for:
- Large result objects (>10KB)
- JSON/text data (compresses well)
- Repeated processing of similar data

Disable for:
- Small results (<1KB)
- Already compressed data (images, videos)
- Low-latency requirements

### 4. DAG vs Workflow Primitives

Use DAG when:
- Complex dependencies between tasks
- Need visualization of task graph
- Conditional execution paths
- Dynamic task creation

Use chain/group/chord when:
- Simple sequential or parallel patterns
- Fixed workflow structure
- Better performance (less overhead)

## Troubleshooting

### DAG Cycle Detection

```
ValueError: Adding task would create a cycle in DAG
```

**Solution**: Check task dependencies, ensure no circular references

### Rate Limit Exceeded

```
Rate limit exceeded (100)
```

**Solution**: Increase rate limit or wait:

```python
limiter = RateLimiter(max_rate=1000, period=60)
limiter.wait_if_needed(key)
```

### Queue Not Found

```
Queue 'xyz' not found
```

**Solution**: Create queue or use existing queue name

## Next Steps

- Explore [Multi-Tenancy Guide](../../docs/MULTI_TENANCY.md)
- Read [Migration from Celery](../../docs/MIGRATION_FROM_CELERY.md)
- Check [Dashboard API](../../docs/DASHBOARD_API.md)

## Contributing

Have a useful pattern? Submit a PR with a new example!
