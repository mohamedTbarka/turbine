"""Generate Python gRPC stubs from proto files."""

import subprocess
import sys
from pathlib import Path


def main() -> int:
    """Generate Python protobuf and gRPC stubs."""
    # Find the proto file
    package_root = Path(__file__).parent.parent.parent.parent
    proto_dir = package_root / "proto"
    proto_file = proto_dir / "turbine.proto"

    if not proto_file.exists():
        print(f"Error: Proto file not found at {proto_file}")
        return 1

    # Output directory for generated files
    output_dir = Path(__file__).parent / "_proto"
    output_dir.mkdir(exist_ok=True)

    # Create __init__.py
    init_file = output_dir / "__init__.py"
    init_file.write_text(
        '"""Generated protobuf and gRPC stubs."""\n'
        "from turbine._proto.turbine_pb2 import *  # noqa: F401, F403\n"
        "from turbine._proto.turbine_pb2_grpc import *  # noqa: F401, F403\n"
    )

    # Run grpc_tools.protoc
    try:
        from grpc_tools import protoc

        result = protoc.main(
            [
                "grpc_tools.protoc",
                f"-I{proto_dir}",
                f"--python_out={output_dir}",
                f"--grpc_python_out={output_dir}",
                f"--pyi_out={output_dir}",
                str(proto_file),
            ]
        )

        if result != 0:
            print(f"Error: protoc returned {result}")
            return result

        # Fix imports in generated files (grpc_tools generates relative imports incorrectly)
        pb2_grpc_file = output_dir / "turbine_pb2_grpc.py"
        if pb2_grpc_file.exists():
            content = pb2_grpc_file.read_text()
            content = content.replace(
                "import turbine_pb2 as turbine__pb2",
                "from turbine._proto import turbine_pb2 as turbine__pb2",
            )
            pb2_grpc_file.write_text(content)

        print(f"Successfully generated gRPC stubs in {output_dir}")
        return 0

    except ImportError:
        print("Error: grpc_tools not installed. Install with: pip install grpcio-tools")
        return 1
    except Exception as e:
        print(f"Error generating stubs: {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
