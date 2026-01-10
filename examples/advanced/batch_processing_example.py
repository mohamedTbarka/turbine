"""
Advanced example: Batch processing with Turbine

This example demonstrates:
- Processing large datasets efficiently
- Using BatchProcessor for parallel processing
- Progress tracking and error handling
- Map-reduce pattern
"""

from turbine import Turbine, task
from turbine.batch import BatchProcessor, Batcher, batch_map


# Initialize Turbine
app = Turbine(server="localhost:50051")


@task(queue="processing", timeout=30)
def process_image(image_id: int) -> dict:
    """Process a single image."""
    import time

    # Simulate image processing
    time.sleep(0.1)

    return {
        "image_id": image_id,
        "processed": True,
        "size": image_id * 100,
    }


@task(queue="processing")
def aggregate_sizes(results: list[dict]) -> dict:
    """Aggregate results from image processing."""
    total_size = sum(r["size"] for r in results)
    processed_count = sum(1 for r in results if r["processed"])

    return {
        "total_images": len(results),
        "processed": processed_count,
        "total_size": total_size,
    }


def example_simple_batch():
    """Simple batch processing."""
    print("=" * 60)
    print("Example 1: Simple Batch Processing")
    print("=" * 60)

    # Process 100 images in batches
    image_ids = list(range(1, 101))

    print(f"Processing {len(image_ids)} images...")

    # Simple batch map
    results = batch_map(process_image, image_ids, batch_size=20)

    print(f"Submitted {len(results)} tasks")
    print("Waiting for completion...")

    # Wait and collect results
    values = []
    for i, result in enumerate(results, 1):
        try:
            value = result.get(timeout=60)
            values.append(value)

            if i % 10 == 0:
                print(f"  Completed {i}/{len(results)} tasks")

        except Exception as e:
            print(f"  Task {i} failed: {e}")

    print(f"\n✓ Processed {len(values)} images successfully")
    total_size = sum(v["size"] for v in values)
    print(f"  Total size: {total_size:,} bytes")


def example_batch_processor_with_progress():
    """Batch processing with progress tracking."""
    print("\n" + "=" * 60)
    print("Example 2: Batch Processing with Progress Tracking")
    print("=" * 60)

    image_ids = list(range(1, 201))

    # Progress callback
    def on_progress(done: int, total: int):
        percent = (done / total) * 100
        print(f"  Progress: {done}/{total} ({percent:.1f}%)")

    # Error callback
    def on_error(error: Exception, item: Any):
        print(f"  Error processing {item}: {error}")

    # Create batch processor
    processor = BatchProcessor(
        task=process_image,
        chunk_size=25,
        max_concurrent=10,
        on_progress=on_progress,
        on_error=on_error,
    )

    print(f"Processing {len(image_ids)} images with progress tracking...")

    results = processor.map(image_ids)

    print(f"\n✓ Submitted {len(results)} tasks")


def example_map_reduce():
    """Map-reduce pattern."""
    print("\n" + "=" * 60)
    print("Example 3: Map-Reduce Pattern")
    print("=" * 60)

    image_ids = list(range(1, 51))

    print(f"Processing {len(image_ids)} images and aggregating results...")

    processor = BatchProcessor(task=process_image, chunk_size=10)

    # Map-reduce: process images and aggregate sizes
    result = processor.map_reduce(
        items=image_ids,
        reducer=aggregate_sizes,
    )

    print("Waiting for aggregation...")
    final = result.get(timeout=120)

    print(f"\n✓ Results:")
    print(f"  Total images: {final['total_images']}")
    print(f"  Processed: {final['processed']}")
    print(f"  Total size: {final['total_size']:,} bytes")


def example_batcher_accumulator():
    """Batcher for accumulating tasks."""
    print("\n" + "=" * 60)
    print("Example 4: Batcher Accumulator")
    print("=" * 60)

    print("Accumulating tasks and auto-submitting in batches of 20...")

    # Context manager auto-flushes on exit
    with Batcher(process_image, batch_size=20) as batcher:
        for i in range(1, 101):
            batcher.add(i)

            # Auto-submits when batch is full (20 items)
            if i % 20 == 0:
                print(f"  Auto-submitted batch at {i} items")

    print(f"\n✓ All tasks submitted")
    print(f"  Total batches: {batcher.results()}")


def example_starmap():
    """Process tuples of arguments."""
    print("\n" + "=" * 60)
    print("Example 5: Starmap (Multiple Arguments)")
    print("=" * 60)

    @task(queue="processing")
    def resize_image(image_id: int, width: int, height: int) -> dict:
        """Resize image to specified dimensions."""
        return {
            "image_id": image_id,
            "dimensions": f"{width}x{height}",
        }

    # List of (image_id, width, height) tuples
    resize_jobs = [
        (1, 800, 600),
        (2, 1024, 768),
        (3, 1920, 1080),
        (4, 640, 480),
    ]

    print(f"Resizing {len(resize_jobs)} images...")

    processor = BatchProcessor(resize_image, chunk_size=2)
    results = processor.starmap(resize_jobs)

    print("Results:")
    for result in results:
        value = result.get(timeout=30)
        print(f"  Image {value['image_id']}: {value['dimensions']}")


if __name__ == "__main__":
    import sys

    examples = {
        "1": ("Simple batch", example_simple_batch),
        "2": ("With progress", example_batch_processor_with_progress),
        "3": ("Map-reduce", example_map_reduce),
        "4": ("Batcher", example_batcher_accumulator),
        "5": ("Starmap", example_starmap),
        "all": ("Run all", None),
    }

    if len(sys.argv) > 1 and sys.argv[1] in examples:
        choice = sys.argv[1]
    else:
        print("Batch Processing Examples\n")
        print("Available examples:")
        for key, (name, _) in examples.items():
            print(f"  {key}. {name}")
        print()
        choice = input("Select example (or 'all'): ").strip()

    if choice == "all":
        for key, (name, func) in examples.items():
            if func:
                func()
    elif choice in examples and examples[choice][1]:
        examples[choice][1]()
    else:
        print("Invalid choice")
        sys.exit(1)
