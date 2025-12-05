#!/bin/bash
set -e

echo "Building Muffin OS RISC-V Demo Kernel..."
echo "=========================================="
echo ""
echo "Note: This builds the standalone demo kernel"
echo "Location: /home/ubuntu/riscv-kernel-demo/"
echo "The full kernel port is still in progress."
echo ""

# Add RISC-V target if not already installed
rustup target add riscv64gc-unknown-none-elf

# Build the standalone RISC-V demo kernel
cd /home/ubuntu/riscv-kernel-demo
cargo build

echo ""
echo "✅ Build complete!"
echo ""
echo "Kernel binary: /home/ubuntu/riscv-kernel-demo/target/riscv64gc-unknown-none-elf/debug/riscv-kernel-demo"
echo ""
echo "To run in QEMU:"
echo "  qemu-system-riscv64 \\"
echo "    -machine virt \\"
echo "    -bios default \\"
echo "    -kernel /home/ubuntu/riscv-kernel-demo/target/riscv64gc-unknown-none-elf/debug/riscv-kernel-demo \\"
echo "    -nographic"
echo ""
echo "Or use the run script:"
echo "  ./run-riscv.sh"
echo ""
