"""Alternative result backend implementations."""

import logging
from abc import ABC, abstractmethod
from typing import Any
import json

logger = logging.getLogger(__name__)


class ResultBackend(ABC):
    """Abstract base class for result backends."""

    @abstractmethod
    def store(self, task_id: str, data: dict[str, Any], ttl: int) -> bool:
        """
        Store task result.

        Args:
            task_id: Task identifier
            data: Result data
            ttl: Time-to-live in seconds

        Returns:
            True if stored successfully
        """
        pass

    @abstractmethod
    def get(self, task_id: str) -> dict[str, Any] | None:
        """
        Retrieve task result.

        Args:
            task_id: Task identifier

        Returns:
            Result data or None if not found
        """
        pass

    @abstractmethod
    def delete(self, task_id: str) -> bool:
        """
        Delete task result.

        Args:
            task_id: Task identifier

        Returns:
            True if deleted
        """
        pass


class RedisBackend(ResultBackend):
    """Redis result backend (default)."""

    def __init__(self, url: str = "redis://localhost:6379"):
        """Initialize Redis backend."""
        import redis
        self.conn = redis.from_url(url, decode_responses=False)

    def store(self, task_id: str, data: dict[str, Any], ttl: int) -> bool:
        """Store result in Redis."""
        try:
            import msgpack
            key = f"turbine:result:{task_id}"
            packed = msgpack.packb(data, use_bin_type=True)
            self.conn.set(key, packed, ex=ttl)
            return True
        except Exception as e:
            logger.error(f"Failed to store result: {e}")
            return False

    def get(self, task_id: str) -> dict[str, Any] | None:
        """Get result from Redis."""
        try:
            import msgpack
            key = f"turbine:result:{task_id}"
            data = self.conn.get(key)
            if data:
                return msgpack.unpackb(data, raw=False)
            return None
        except Exception as e:
            logger.error(f"Failed to get result: {e}")
            return None

    def delete(self, task_id: str) -> bool:
        """Delete result from Redis."""
        try:
            key = f"turbine:result:{task_id}"
            return self.conn.delete(key) > 0
        except Exception as e:
            logger.error(f"Failed to delete result: {e}")
            return False


class S3Backend(ResultBackend):
    """
    S3 result backend for large payloads.

    Useful when results exceed Redis practical limits (>10MB).
    """

    def __init__(
        self,
        bucket: str,
        region: str = "us-east-1",
        prefix: str = "turbine/results/",
        aws_access_key_id: str | None = None,
        aws_secret_access_key: str | None = None,
    ):
        """
        Initialize S3 backend.

        Args:
            bucket: S3 bucket name
            region: AWS region
            prefix: Key prefix for results
            aws_access_key_id: AWS access key (optional, uses IAM role if not provided)
            aws_secret_access_key: AWS secret key
        """
        try:
            import boto3
        except ImportError:
            raise ImportError(
                "boto3 is required for S3 backend. Install with: pip install boto3"
            )

        self.bucket = bucket
        self.prefix = prefix

        # Initialize S3 client
        if aws_access_key_id and aws_secret_access_key:
            self.s3 = boto3.client(
                's3',
                region_name=region,
                aws_access_key_id=aws_access_key_id,
                aws_secret_access_key=aws_secret_access_key,
            )
        else:
            # Use IAM role or environment credentials
            self.s3 = boto3.client('s3', region_name=region)

        logger.info(f"Initialized S3 backend: s3://{bucket}/{prefix}")

    def _get_key(self, task_id: str) -> str:
        """Get S3 key for task result."""
        return f"{self.prefix}{task_id}.json"

    def store(self, task_id: str, data: dict[str, Any], ttl: int) -> bool:
        """Store result in S3."""
        try:
            key = self._get_key(task_id)
            body = json.dumps(data).encode('utf-8')

            # Store with metadata
            self.s3.put_object(
                Bucket=self.bucket,
                Key=key,
                Body=body,
                ContentType='application/json',
                Metadata={
                    'task_id': task_id,
                    'ttl': str(ttl),
                }
            )

            logger.debug(f"Stored result to s3://{self.bucket}/{key}")
            return True

        except Exception as e:
            logger.error(f"Failed to store result to S3: {e}")
            return False

    def get(self, task_id: str) -> dict[str, Any] | None:
        """Get result from S3."""
        try:
            key = self._get_key(task_id)

            response = self.s3.get_object(
                Bucket=self.bucket,
                Key=key
            )

            body = response['Body'].read()
            data = json.loads(body.decode('utf-8'))

            logger.debug(f"Retrieved result from s3://{self.bucket}/{key}")
            return data

        except self.s3.exceptions.NoSuchKey:
            logger.debug(f"Result not found: {task_id}")
            return None
        except Exception as e:
            logger.error(f"Failed to get result from S3: {e}")
            return None

    def delete(self, task_id: str) -> bool:
        """Delete result from S3."""
        try:
            key = self._get_key(task_id)

            self.s3.delete_object(
                Bucket=self.bucket,
                Key=key
            )

            logger.debug(f"Deleted result from s3://{self.bucket}/{key}")
            return True

        except Exception as e:
            logger.error(f"Failed to delete result from S3: {e}")
            return False

    def list_results(self, max_keys: int = 1000) -> list[str]:
        """
        List all task IDs in the backend.

        Args:
            max_keys: Maximum number of results to return

        Returns:
            List of task IDs
        """
        try:
            response = self.s3.list_objects_v2(
                Bucket=self.bucket,
                Prefix=self.prefix,
                MaxKeys=max_keys
            )

            if 'Contents' not in response:
                return []

            task_ids = []
            for obj in response['Contents']:
                key = obj['Key']
                # Extract task_id from key
                task_id = key.replace(self.prefix, '').replace('.json', '')
                task_ids.append(task_id)

            return task_ids

        except Exception as e:
            logger.error(f"Failed to list results from S3: {e}")
            return []


class HybridBackend(ResultBackend):
    """
    Hybrid backend that uses Redis for small results and S3 for large ones.

    Automatically routes results based on size threshold.
    """

    def __init__(
        self,
        redis_url: str = "redis://localhost:6379",
        s3_bucket: str | None = None,
        s3_region: str = "us-east-1",
        size_threshold: int = 1048576,  # 1MB
    ):
        """
        Initialize hybrid backend.

        Args:
            redis_url: Redis connection URL
            s3_bucket: S3 bucket name (required for S3 offload)
            s3_region: AWS region
            size_threshold: Size threshold for S3 offload (bytes)
        """
        self.size_threshold = size_threshold
        self.redis = RedisBackend(redis_url)

        if s3_bucket:
            self.s3 = S3Backend(s3_bucket, region=s3_region)
        else:
            self.s3 = None

        logger.info(
            f"Initialized hybrid backend (threshold: {size_threshold} bytes)"
        )

    def store(self, task_id: str, data: dict[str, Any], ttl: int) -> bool:
        """Store result, routing to Redis or S3 based on size."""
        try:
            import msgpack

            # Serialize to check size
            serialized = msgpack.packb(data, use_bin_type=True)
            size = len(serialized)

            if size > self.size_threshold and self.s3:
                # Large result -> S3
                logger.info(
                    f"Result size {size} bytes exceeds threshold, storing to S3"
                )
                success = self.s3.store(task_id, data, ttl)

                # Store pointer in Redis
                if success:
                    self.redis.conn.set(
                        f"turbine:result:{task_id}:s3",
                        "true",
                        ex=ttl
                    )

                return success
            else:
                # Small result -> Redis
                return self.redis.store(task_id, data, ttl)

        except Exception as e:
            logger.error(f"Failed to store result: {e}")
            return False

    def get(self, task_id: str) -> dict[str, Any] | None:
        """Get result, checking both Redis and S3."""
        try:
            # Check if result is in S3
            s3_marker = self.redis.conn.get(f"turbine:result:{task_id}:s3")

            if s3_marker and self.s3:
                # Result is in S3
                return self.s3.get(task_id)
            else:
                # Result is in Redis
                return self.redis.get(task_id)

        except Exception as e:
            logger.error(f"Failed to get result: {e}")
            return None

    def delete(self, task_id: str) -> bool:
        """Delete result from both backends."""
        redis_deleted = self.redis.delete(task_id)
        s3_deleted = False

        # Check if result was in S3
        s3_marker = self.redis.conn.get(f"turbine:result:{task_id}:s3")
        if s3_marker and self.s3:
            s3_deleted = self.s3.delete(task_id)
            self.redis.conn.delete(f"turbine:result:{task_id}:s3")

        return redis_deleted or s3_deleted


class LocalFileBackend(ResultBackend):
    """
    Local filesystem backend for development/testing.

    NOT recommended for production use.
    """

    def __init__(self, directory: str = "/tmp/turbine-results"):
        """
        Initialize file backend.

        Args:
            directory: Directory to store results
        """
        import os

        self.directory = directory
        os.makedirs(directory, exist_ok=True)
        logger.info(f"Initialized file backend: {directory}")

    def _get_path(self, task_id: str) -> str:
        """Get file path for task result."""
        import os
        return os.path.join(self.directory, f"{task_id}.json")

    def store(self, task_id: str, data: dict[str, Any], ttl: int) -> bool:
        """Store result to file."""
        try:
            path = self._get_path(task_id)
            with open(path, 'w') as f:
                json.dump(data, f, indent=2)
            return True
        except Exception as e:
            logger.error(f"Failed to store result to file: {e}")
            return False

    def get(self, task_id: str) -> dict[str, Any] | None:
        """Get result from file."""
        try:
            import os
            path = self._get_path(task_id)

            if not os.path.exists(path):
                return None

            with open(path, 'r') as f:
                return json.load(f)

        except Exception as e:
            logger.error(f"Failed to get result from file: {e}")
            return None

    def delete(self, task_id: str) -> bool:
        """Delete result file."""
        try:
            import os
            path = self._get_path(task_id)

            if os.path.exists(path):
                os.remove(path)
                return True

            return False

        except Exception as e:
            logger.error(f"Failed to delete result file: {e}")
            return False


def get_backend(backend_type: str, **kwargs) -> ResultBackend:
    """
    Factory function to get result backend.

    Args:
        backend_type: Type of backend ('redis', 's3', 'hybrid', 'file')
        **kwargs: Backend-specific configuration

    Returns:
        ResultBackend instance

    Example:
        # Redis
        backend = get_backend('redis', url='redis://localhost:6379')

        # S3
        backend = get_backend('s3', bucket='my-bucket', region='us-west-2')

        # Hybrid
        backend = get_backend(
            'hybrid',
            redis_url='redis://localhost:6379',
            s3_bucket='my-bucket',
            size_threshold=5*1024*1024  # 5MB
        )
    """
    if backend_type == "redis":
        return RedisBackend(**kwargs)
    elif backend_type == "s3":
        return S3Backend(**kwargs)
    elif backend_type == "hybrid":
        return HybridBackend(**kwargs)
    elif backend_type == "file":
        return LocalFileBackend(**kwargs)
    else:
        raise ValueError(f"Unknown backend type: {backend_type}")
