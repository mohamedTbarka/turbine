"""Task result compression utilities."""

import logging
from typing import Any, Literal
from enum import Enum

logger = logging.getLogger(__name__)


class CompressionType(str, Enum):
    """Supported compression types."""

    NONE = "none"
    GZIP = "gzip"
    ZLIB = "zlib"
    BROTLI = "brotli"
    LZ4 = "lz4"


class Compressor:
    """Handles compression and decompression of task data."""

    def __init__(self, compression_type: CompressionType = CompressionType.GZIP):
        """
        Initialize compressor.

        Args:
            compression_type: Type of compression to use
        """
        self.compression_type = compression_type

    def compress(self, data: bytes) -> tuple[bytes, str]:
        """
        Compress data.

        Args:
            data: Raw bytes to compress

        Returns:
            Tuple of (compressed_data, compression_type)
        """
        if self.compression_type == CompressionType.NONE:
            return data, "none"

        try:
            if self.compression_type == CompressionType.GZIP:
                import gzip
                compressed = gzip.compress(data, compresslevel=6)
                return compressed, "gzip"

            elif self.compression_type == CompressionType.ZLIB:
                import zlib
                compressed = zlib.compress(data, level=6)
                return compressed, "zlib"

            elif self.compression_type == CompressionType.BROTLI:
                try:
                    import brotli
                    compressed = brotli.compress(data, quality=6)
                    return compressed, "brotli"
                except ImportError:
                    logger.warning("brotli not installed, falling back to gzip")
                    import gzip
                    compressed = gzip.compress(data, compresslevel=6)
                    return compressed, "gzip"

            elif self.compression_type == CompressionType.LZ4:
                try:
                    import lz4.frame
                    compressed = lz4.frame.compress(data)
                    return compressed, "lz4"
                except ImportError:
                    logger.warning("lz4 not installed, falling back to gzip")
                    import gzip
                    compressed = gzip.compress(data, compresslevel=6)
                    return compressed, "gzip"

        except Exception as e:
            logger.error(f"Compression failed: {e}, returning uncompressed data")
            return data, "none"

        return data, "none"

    def decompress(self, data: bytes, compression_type: str) -> bytes:
        """
        Decompress data.

        Args:
            data: Compressed bytes
            compression_type: Type of compression used

        Returns:
            Decompressed data
        """
        if compression_type == "none":
            return data

        try:
            if compression_type == "gzip":
                import gzip
                return gzip.decompress(data)

            elif compression_type == "zlib":
                import zlib
                return zlib.decompress(data)

            elif compression_type == "brotli":
                import brotli
                return brotli.decompress(data)

            elif compression_type == "lz4":
                import lz4.frame
                return lz4.frame.decompress(data)

            else:
                logger.warning(f"Unknown compression type: {compression_type}")
                return data

        except Exception as e:
            logger.error(f"Decompression failed: {e}, returning raw data")
            return data

    @staticmethod
    def auto_compress(
        data: bytes,
        min_size: int = 1024,
        compression_type: CompressionType = CompressionType.GZIP,
    ) -> tuple[bytes, str]:
        """
        Automatically compress data only if it's worth it.

        Args:
            data: Data to compress
            min_size: Minimum size in bytes to trigger compression
            compression_type: Type of compression to use

        Returns:
            Tuple of (data, compression_type)
        """
        if len(data) < min_size:
            return data, "none"

        compressor = Compressor(compression_type)
        compressed, comp_type = compressor.compress(data)

        # Only use compression if it saves at least 10%
        if len(compressed) < len(data) * 0.9:
            logger.debug(
                f"Compressed {len(data)} bytes to {len(compressed)} bytes "
                f"({len(compressed)/len(data)*100:.1f}%) using {comp_type}"
            )
            return compressed, comp_type

        return data, "none"

    @staticmethod
    def get_compression_ratio(original_size: int, compressed_size: int) -> float:
        """
        Calculate compression ratio.

        Args:
            original_size: Original data size in bytes
            compressed_size: Compressed data size in bytes

        Returns:
            Compression ratio (e.g., 0.5 means 50% of original size)
        """
        if original_size == 0:
            return 1.0
        return compressed_size / original_size

    @staticmethod
    def supports_compression(compression_type: str) -> bool:
        """
        Check if a compression type is supported.

        Args:
            compression_type: Compression type to check

        Returns:
            True if supported
        """
        if compression_type == "none":
            return True

        try:
            if compression_type == "gzip":
                import gzip
                return True
            elif compression_type == "zlib":
                import zlib
                return True
            elif compression_type == "brotli":
                import brotli
                return True
            elif compression_type == "lz4":
                import lz4.frame
                return True
        except ImportError:
            return False

        return False
