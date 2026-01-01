#!/bin/bash
# Build Muffin OS kernel for Raspberry Pi 5
#
# This script builds the kernel for the aarch64-unknown-none target
# with the rpi5 feature enabled, then creates a raw binary suitable
# for the Pi 5 bootloader.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TARGET="aarch64-unknown-none"
PROFILE="${1:-release}"

echo "=== Building Muffin OS for Raspberry Pi 5 ==="
echo "Profile: $PROFILE"
echo ""

cd "$PROJECT_DIR"

# Ensure target is installed
if ! rustup target list | grep -q "$TARGET (installed)"; then
    echo "Installing $TARGET target..."
    rustup target add "$TARGET"
fi

# Build the kernel
echo "Building kernel..."
if [ "$PROFILE" = "release" ]; then
    cargo build --target "$TARGET" --features rpi5 --release -p kernel
    BUILD_DIR="target/$TARGET/release"
else
    cargo build --target "$TARGET" --features rpi5 -p kernel
    BUILD_DIR="target/$TARGET/debug"
fi

# Check if llvm-objcopy is available
if command -v llvm-objcopy &> /dev/null; then
    OBJCOPY="llvm-objcopy"
elif command -v rust-objcopy &> /dev/null; then
    OBJCOPY="rust-objcopy"
else
    echo "Error: Neither llvm-objcopy nor rust-objcopy found."
    echo "Install with: cargo install cargo-binutils && rustup component add llvm-tools-preview"
    exit 1
fi

# Create raw binary
echo "Creating kernel8.img..."
$OBJCOPY -O binary "$BUILD_DIR/kernel" "$BUILD_DIR/kernel8.img"

# Report results
SIZE=$(stat -c%s "$BUILD_DIR/kernel8.img" 2>/dev/null || stat -f%z "$BUILD_DIR/kernel8.img")
echo ""
echo "=== Build Complete ==="
echo "Kernel ELF: $BUILD_DIR/kernel"
echo "Kernel Binary: $BUILD_DIR/kernel8.img"
echo "Size: $SIZE bytes"
echo ""
echo "To deploy to SD card, run:"
echo "  ./scripts/deploy-rpi5.sh /path/to/sdcard/boot"
