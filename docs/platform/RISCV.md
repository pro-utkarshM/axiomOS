# Building and Running RISC-V Kernel

## Quick Start

### Build
```bash
./scripts/build-riscv.sh
```

### Run
```bash
./scripts/run-riscv.sh
```

Press `Ctrl+A` then `X` to exit QEMU.

## What Gets Built

The build script compiles the **RISC-V demo kernel** located at:
```
kernel/demos/riscv/
```

**Note:** This is NOT the main Muffin kernel. It's a minimal demonstration kernel that proves OpenSBI integration works.

## Why a Separate Demo Kernel?

The main Muffin kernel (`kernel/`) currently has deep x86_64 dependencies and cannot build for RISC-V yet. This is **normal** during OS porting.

The demo kernel:
- ✅ Proves OpenSBI bootloader integration works
- ✅ Shows boot assembly, SBI calls, and console output
- ✅ Serves as reference for full kernel integration
- ✅ Follows industry best practices (Linux does this too)

See `docs/MULTI_ARCH_STRATEGY.md` for the full porting plan.

## Expected Output

```
OpenSBI v0.9
...
axiom-ebpf RISC-V Kernel
=======================
Hart ID: 0
DTB Address: 0x87000000

OpenSBI integration successful!
Kernel booted via OpenSBI firmware

This is a minimal demonstration kernel showing:
  - Boot assembly entry point
  - OpenSBI SBI calls (console output)
  - Device tree blob address capture
  - Proper linker script usage

Halting...
```

## Build Details

### Prerequisites
- Rust toolchain (installed via rustup)
- RISC-V target: `riscv64gc-unknown-none-elf`
- RISC-V GCC: `gcc-riscv64-unknown-elf` (for assembly)
- QEMU: `qemu-system-riscv64`

### Build Process
1. Adds RISC-V target to rustup (if needed)
2. Compiles boot assembly (`boot.S`) via `cc` crate
3. Links with linker script (`linker.ld`)
4. Produces ELF binary at physical address `0x80200000`

### Binary Location
```
kernel/demos/riscv/target/riscv64gc-unknown-none-elf/debug/riscv-kernel-demo
```

## Manual Build (Alternative)

```bash
cd kernel/demos/riscv
rustup target add riscv64gc-unknown-none-elf
cargo build
```

## Manual Run (Alternative)

```bash
qemu-system-riscv64 \
  -machine virt \
  -bios default \
  -kernel kernel/demos/riscv/target/riscv64gc-unknown-none-elf/debug/riscv-kernel-demo \
  -nographic
```

## Troubleshooting

### "Kernel not found"
Run `./scripts/build-riscv.sh` first to build the kernel.

### "qemu-system-riscv64: command not found"
Install QEMU:
```bash
sudo apt install qemu-system-misc
```

### "error: failed to find tool 'riscv64-unknown-elf-gcc'"
Install RISC-V GCC:
```bash
sudo apt install gcc-riscv64-unknown-elf
```

### Build fails with x86_64 errors
You're trying to build the main kernel (`kernel/`) which doesn't support RISC-V yet. Use the demo kernel instead via `./scripts/build-riscv.sh`.

### Build fails with workspace errors
The demo kernel should be excluded from the workspace. Check that `Cargo.toml` has:
```toml
[workspace]
exclude = [
  "kernel/demos/riscv",
]
```

And `kernel/demos/riscv/Cargo.toml` has:
```toml
[workspace]
# This is a standalone project
```

## Next Steps

For full RISC-V support in the main kernel, see:
- `docs/MULTI_ARCH_STRATEGY.md` - Porting strategy
- `docs/ARCHITECTURE_SUPPORT.md` - Architecture abstraction
- `docs/PORTING_DESIGN.md` - Design decisions

The demo kernel proves OpenSBI works. Full porting is ongoing.
