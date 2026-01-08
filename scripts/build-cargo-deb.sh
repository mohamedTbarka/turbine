#!/bin/bash
# Build Debian packages using cargo-deb
#
# Usage:
#   ./scripts/build-cargo-deb.sh
#
# This is an alternative to dpkg-buildpackage that builds
# individual .deb packages for each binary.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

cd "$PROJECT_DIR"

echo "==> Checking cargo-deb..."
if ! command -v cargo-deb &> /dev/null; then
    echo "Installing cargo-deb..."
    cargo install cargo-deb
fi

echo "==> Building release binaries..."
cargo build --release \
    --package turbine-server \
    --package turbine-worker \
    --package turbine-dashboard

echo "==> Creating Debian packages..."

mkdir -p dist

# Build turbine-server package
echo "    Building turbine-server..."
cargo deb --package turbine-server --no-build --output dist/

# Build turbine-worker package
echo "    Building turbine-worker..."
cargo deb --package turbine-worker --no-build --output dist/

# Build turbine-dashboard package
echo "    Building turbine-dashboard..."
cargo deb --package turbine-dashboard --no-build --output dist/

echo ""
echo "==> Build complete!"
echo ""
echo "Packages created in dist/:"
ls -la dist/*.deb

echo ""
echo "To install:"
echo "  sudo dpkg -i dist/turbine-server_*.deb"
echo "  sudo dpkg -i dist/turbine-worker_*.deb"
echo "  sudo dpkg -i dist/turbine-dashboard_*.deb"
