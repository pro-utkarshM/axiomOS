#!/bin/bash
set -e

# Generate disk.img by building the root package for aarch64
echo "Building root package for aarch64 to generate disk.img..."
cargo build -p muffinos --target aarch64-unknown-none

# The build.rs generates the image in the target directory.
# We find it and copy it to the root.
DISK_PATH=$(find target/aarch64-unknown-none -name disk.img | head -n 1)
if [ -n "$DISK_PATH" ]; then
    cp "$DISK_PATH" disk.img
else
    echo "Error: disk.img not found in target/aarch64-unknown-none"
    exit 1
fi

# Build the kernel for QEMU virt
cargo build --target aarch64-unknown-none --features virt -p kernel

# Run in QEMU
# Added virtio-blk-device for disk.img
timeout 60s qemu-system-aarch64 \
    -machine virt \
    -m 1G \
    -cpu cortex-a57 \
    -nographic \
    -kernel target/aarch64-unknown-none/debug/kernel \
    -drive if=none,file=disk.img,format=raw,id=hd0 \
    -device virtio-blk-device,drive=hd0 \
    -d guest_errors,unimp \
    -semihosting
