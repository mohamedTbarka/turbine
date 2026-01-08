#!/bin/bash
# Build Debian packages locally
#
# Usage:
#   ./scripts/build-deb.sh
#
# Requirements:
#   - Rust toolchain
#   - debhelper, devscripts packages

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "==> Checking dependencies..."

# Check for required tools
for cmd in cargo dpkg-buildpackage; do
    if ! command -v "$cmd" &> /dev/null; then
        echo "Error: $cmd is required but not installed."
        echo ""
        if [ "$cmd" = "cargo" ]; then
            echo "Install Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        else
            echo "Install debhelper: sudo apt-get install debhelper devscripts"
        fi
        exit 1
    fi
done

echo "==> Building Debian packages..."

# Build packages
dpkg-buildpackage -us -uc -b

echo "==> Build complete!"
echo ""
echo "Packages created in parent directory:"
ls -la ../*.deb 2>/dev/null || echo "No .deb files found"

echo ""
echo "To install:"
echo "  sudo dpkg -i ../turbine_*.deb"
echo "  sudo dpkg -i ../turbine-server_*.deb"
echo "  sudo dpkg -i ../turbine-worker_*.deb"
echo "  sudo dpkg -i ../turbine-dashboard_*.deb"
