#!/bin/bash
# Install Turbine from source
#
# Usage:
#   curl -sSL https://raw.githubusercontent.com/turbine-queue/turbine/main/scripts/install.sh | bash
#
# Or locally:
#   ./scripts/install.sh [--prefix=/usr/local]

set -e

# Default installation prefix
PREFIX="${PREFIX:-/usr/local}"
SYSTEMD_DIR="/etc/systemd/system"
CONFIG_DIR="/etc/turbine"
LOG_DIR="/var/log/turbine"
DATA_DIR="/var/lib/turbine"

# Parse arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        --prefix=*)
            PREFIX="${1#*=}"
            shift
            ;;
        --prefix)
            PREFIX="$2"
            shift 2
            ;;
        --help|-h)
            echo "Usage: $0 [OPTIONS]"
            echo ""
            echo "Options:"
            echo "  --prefix=PATH    Installation prefix (default: /usr/local)"
            echo "  --help           Show this help message"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

# Detect script directory or download source
if [ -f "Cargo.toml" ] && grep -q 'turbine-server' Cargo.toml 2>/dev/null; then
    PROJECT_DIR="$(pwd)"
elif [ -d "$(dirname "$0")/../crates" ]; then
    PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
else
    echo "==> Downloading Turbine source..."
    TEMP_DIR=$(mktemp -d)
    trap "rm -rf $TEMP_DIR" EXIT
    git clone --depth 1 https://github.com/turbine-queue/turbine.git "$TEMP_DIR/turbine"
    PROJECT_DIR="$TEMP_DIR/turbine"
fi

cd "$PROJECT_DIR"

echo "==> Installing Turbine to $PREFIX"
echo ""

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust is required but not installed."
    echo ""
    echo "Install Rust with:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

# Build release binaries
echo "==> Building release binaries..."
cargo build --release \
    --package turbine-server \
    --package turbine-worker \
    --package turbine-dashboard

# Install binaries
echo "==> Installing binaries..."
install -d "$PREFIX/bin"
install -m 755 target/release/turbine-server "$PREFIX/bin/"
install -m 755 target/release/turbine-worker "$PREFIX/bin/"
install -m 755 target/release/turbine-dashboard "$PREFIX/bin/"

# Install configuration
echo "==> Installing configuration..."
if [ ! -f "$CONFIG_DIR/turbine.toml" ]; then
    install -d "$CONFIG_DIR"
    install -m 640 debian/turbine.toml "$CONFIG_DIR/"
    echo "    Created $CONFIG_DIR/turbine.toml"
else
    echo "    Configuration already exists, skipping"
fi

# Install systemd services (if systemd is available)
if [ -d "$SYSTEMD_DIR" ] && command -v systemctl &> /dev/null; then
    echo "==> Installing systemd services..."
    install -m 644 debian/turbine-server.service "$SYSTEMD_DIR/"
    install -m 644 debian/turbine-worker.service "$SYSTEMD_DIR/"
    install -m 644 debian/turbine-worker@.service "$SYSTEMD_DIR/"
    install -m 644 debian/turbine-dashboard.service "$SYSTEMD_DIR/"
    systemctl daemon-reload
fi

# Create turbine user if it doesn't exist
if ! id turbine &>/dev/null 2>&1; then
    echo "==> Creating turbine user..."
    if command -v useradd &> /dev/null; then
        useradd --system --user-group --home-dir "$DATA_DIR" --shell /usr/sbin/nologin turbine || true
    fi
fi

# Create directories
echo "==> Creating directories..."
install -d -m 750 "$LOG_DIR"
install -d -m 750 "$DATA_DIR"
if id turbine &>/dev/null 2>&1; then
    chown -R turbine:turbine "$LOG_DIR" "$DATA_DIR" 2>/dev/null || true
    chown root:turbine "$CONFIG_DIR/turbine.toml" 2>/dev/null || true
fi

echo ""
echo "==> Installation complete!"
echo ""
echo "Binaries installed to:"
echo "  $PREFIX/bin/turbine-server"
echo "  $PREFIX/bin/turbine-worker"
echo "  $PREFIX/bin/turbine-dashboard"
echo ""
echo "Configuration file:"
echo "  $CONFIG_DIR/turbine.toml"
echo ""
echo "To start Turbine:"
echo "  # Start server"
echo "  sudo systemctl start turbine-server"
echo ""
echo "  # Start worker(s)"
echo "  sudo systemctl start turbine-worker"
echo "  # Or multiple workers:"
echo "  sudo systemctl start turbine-worker@1"
echo "  sudo systemctl start turbine-worker@2"
echo ""
echo "  # Start dashboard"
echo "  sudo systemctl start turbine-dashboard"
echo ""
echo "Or run manually:"
echo "  turbine-server --config $CONFIG_DIR/turbine.toml"
echo "  turbine-worker --config $CONFIG_DIR/turbine.toml"
echo "  turbine-dashboard --config $CONFIG_DIR/turbine.toml"
