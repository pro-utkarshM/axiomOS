#!/bin/bash
# Deploy Muffin OS to Raspberry Pi 5 SD card
#
# Usage: ./scripts/deploy-rpi5.sh /path/to/sdcard/boot
#
# This script copies the kernel and configuration to a mounted SD card.
# The SD card should be formatted with a FAT32 boot partition.

set -e

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
TARGET="aarch64-unknown-none"
PROFILE="${2:-release}"

# Check arguments
if [ -z "$1" ]; then
    echo "Usage: $0 /path/to/sdcard/boot [debug|release]"
    echo ""
    echo "The path should be the mounted boot partition of your SD card."
    echo "Example: $0 /media/user/boot"
    exit 1
fi

MOUNT_POINT="$1"

# Verify mount point exists
if [ ! -d "$MOUNT_POINT" ]; then
    echo "Error: Mount point '$MOUNT_POINT' does not exist."
    exit 1
fi

# Determine build directory
if [ "$PROFILE" = "release" ]; then
    BUILD_DIR="$PROJECT_DIR/target/$TARGET/release"
else
    BUILD_DIR="$PROJECT_DIR/target/$TARGET/debug"
fi

# Check if kernel exists
KERNEL_IMG="$BUILD_DIR/kernel8.img"
if [ ! -f "$KERNEL_IMG" ]; then
    echo "Error: Kernel image not found at $KERNEL_IMG"
    echo "Run './scripts/build-rpi5.sh' first."
    exit 1
fi

echo "=== Deploying Muffin OS to Raspberry Pi 5 ==="
echo "Mount point: $MOUNT_POINT"
echo "Kernel: $KERNEL_IMG"
echo ""

# Copy kernel
echo "Copying kernel8.img..."
cp "$KERNEL_IMG" "$MOUNT_POINT/kernel8.img"

# Create config.txt if it doesn't exist
CONFIG_FILE="$MOUNT_POINT/config.txt"
if [ ! -f "$CONFIG_FILE" ]; then
    echo "Creating config.txt..."
    cat > "$CONFIG_FILE" << 'EOF'
# Raspberry Pi 5 configuration for Muffin OS

# Use 64-bit kernel
arm_64bit=1

# Kernel filename
kernel=kernel8.img

# Critical: Disable PCIe reset to keep RP1 peripherals pre-initialized
# Without this, RP1 peripherals won't be accessible without a PCIe driver
pciex4_reset=0

# Critical: Enable RP1 UART for bare metal kernel
# This configures UART0 at 115200 baud through RP1
enable_rp1_uart=1

# Enable UART output (legacy GPIO 14/15)
enable_uart=1

# Early UART output for debugging
uart_2ndstage=1

# Disable rainbow splash screen for faster boot
disable_splash=1

# Minimum GPU memory (we don't use the GPU)
gpu_mem=16

# Disable Bluetooth (uses UART pins by default)
dtoverlay=disable-bt
EOF
else
    echo "config.txt already exists, not overwriting."
fi

# Sync filesystem
echo "Syncing filesystem..."
sync

echo ""
echo "=== Deployment Complete ==="
echo ""
echo "Files on SD card:"
ls -la "$MOUNT_POINT/kernel8.img" "$MOUNT_POINT/config.txt" 2>/dev/null || true
echo ""
echo "Note: You also need the Raspberry Pi firmware files on the SD card:"
echo "  - bootcode.bin (for Pi 4, not needed for Pi 5)"
echo "  - start4.elf"
echo "  - fixup4.dat"
echo ""
echo "Download firmware from:"
echo "  https://github.com/raspberrypi/firmware/tree/master/boot"
echo ""
echo "Safely eject the SD card and boot your Pi 5!"
