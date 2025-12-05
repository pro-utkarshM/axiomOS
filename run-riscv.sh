#!/bin/bash
set -e

echo "Running Muffin OS RISC-V Demo Kernel in QEMU..."
echo "================================================"
echo ""

KERNEL_PATH="/home/ubuntu/riscv-kernel-demo/target/riscv64gc-unknown-none-elf/debug/riscv-kernel-demo"

# Check if kernel exists
if [ ! -f "$KERNEL_PATH" ]; then
    echo "Error: Kernel not found at $KERNEL_PATH"
    echo "Please run ./build-riscv.sh first"
    exit 1
fi

echo "Kernel: $KERNEL_PATH"
echo ""
echo "Press Ctrl+A then X to exit QEMU"
echo "================================================"
echo ""

qemu-system-riscv64 \
    -machine virt \
    -bios default \
    -kernel "$KERNEL_PATH" \
    -nographic \
    -serial mon:stdio
