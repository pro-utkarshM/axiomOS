# Axiom Kernel Benchmarks

This document contains benchmark results for the Axiom kernel and provides methodology for comparing against Linux.

## Axiom Benchmark Results (QEMU x86_64)

### Test Environment
- **Platform**: QEMU x86_64 emulator
- **Configuration**: Default QEMU settings with 2GB RAM
- **Kernel**: Axiom kernel (dev branch)
- **Measurement Tool**: userspace/benchmark program

### Benchmark Results

| Metric | Result | Target (Proposal) | Status |
|--------|--------|-------------------|--------|
| Boot to init | [TBD - run on QEMU] | <1s (target), <500ms (stretch) | To be measured |
| Kernel heap usage | [TBD - run on QEMU] | <10MB (target), <5MB (stretch) | To be measured |
| BPF load time | [TBD - run on QEMU] | <10ms (target), <1ms (stretch) | To be measured |
| Timer interrupt interval | [TBD - run on QEMU] | <10μs latency (target), <1μs (stretch) | To be measured |

**Note**: These measurements are from QEMU emulation. Hardware measurements on Raspberry Pi 5 will provide more accurate real-world performance data, especially for interrupt latency.

### Detailed Metrics

#### Boot Time
Boot time is measured from kernel entry point (HPET counter start) to init process spawn. This includes:
- Memory subsystem initialization
- BPF subsystem initialization
- Device driver initialization (VirtIO, simulated devices)
- VFS and filesystem mount
- Scheduler and multi-core initialization

**Measurement approach**: HPET counter value at init spawn point.

#### Memory Footprint
Memory footprint is measured as kernel heap usage after all subsystems are initialized. This does not include:
- Kernel code/data segments (statically sized)
- Frame allocator metadata
- Page tables

The heap is used for:
- BPF programs and maps
- Process control blocks
- File system caches
- Device driver state

**Measurement approach**: Heap allocator statistics (used/free/total).

#### BPF Load Time
BPF load time measures the overhead of loading a BPF program via sys_bpf(BPF_PROG_LOAD). This includes:
- Instruction parsing
- Verification (streaming verifier, O(n) memory)
- Program object creation
- JIT compilation (if enabled)

**Test program**: Simple 2-instruction program (r0=42; exit) to measure minimum overhead.
**Measurement approach**: clock_gettime() before/after syscall, averaged over 10 runs.

#### Timer Interrupt Interval
Timer interrupt interval measures the time between consecutive timer ticks, which indicates:
- Timer hardware precision
- Interrupt dispatch overhead
- BPF hook execution overhead

**Measurement approach**: BPF program attached to timer hook that records timestamp via bpf_ktime_get_ns() and writes to ringbuf. Userspace polls ringbuf and calculates intervals between 100 consecutive timestamps.

**Note**: This measures the timer tick rate, not interrupt-to-BPF latency. True interrupt latency requires hardware timestamping at the interrupt entry point, which is deferred to Raspberry Pi 5 hardware testing.

---

## Linux Comparison Methodology

To perform a fair comparison between Axiom and Linux, follow this methodology.

### Hardware Setup
- **Platform**: Raspberry Pi 5 (8GB) for both Axiom and Linux
- **Power supply**: Official Raspberry Pi 5 power supply
- **Storage**: Same SD card for both tests (re-flash between tests)
- **Network**: Disabled during boot time measurement

### Linux Configuration

Use a minimal Buildroot configuration to match Axiom's minimal userspace:

```bash
# Buildroot configuration for minimal Linux comparison
BR2_aarch64=y
BR2_TOOLCHAIN_BUILDROOT_GLIBC=y
BR2_LINUX_KERNEL=y
BR2_LINUX_KERNEL_CUSTOM_VERSION=y
BR2_LINUX_KERNEL_CUSTOM_VERSION_VALUE="6.6"
BR2_LINUX_KERNEL_USE_CUSTOM_CONFIG=y
BR2_LINUX_KERNEL_CUSTOM_CONFIG_FILE="linux-minimal.config"
BR2_TARGET_ROOTFS_EXT2=y
BR2_TARGET_ROOTFS_EXT2_4=y

# Minimal kernel config (linux-minimal.config):
# - CONFIG_EMBEDDED=y
# - Disable all unnecessary drivers
# - Enable only: console, timer, memory management, ext2/ext4
# - Enable eBPF: CONFIG_BPF=y, CONFIG_BPF_SYSCALL=y
# - Disable: networking, USB, audio, graphics (except framebuffer console)
```

### Boot Time Measurement

**Axiom**:
```bash
# Boot Axiom on RPi5, capture serial output
# Look for "Boot to init: X ms" in kernel log
```

**Linux**:
```bash
# Add to kernel command line: initcall_debug
# Boot Linux on RPi5, capture dmesg
# Measure from "Booting Linux" to first userspace process (init)
dmesg | grep "Freeing unused kernel memory"
# Calculate elapsed time from timestamps
```

**Fair comparison criteria**:
- Measure to the same point: first userspace process spawn
- Same hardware, same storage medium
- Cold boot (power cycle, not reboot)
- Average of 5 runs

### Memory Footprint Measurement

**Axiom**:
```bash
# Look for "AXIOM KERNEL METRICS" section in boot log
# Note "Kernel heap usage: X KB"
```

**Linux**:
```bash
# After boot, read memory stats
cat /proc/meminfo | grep "MemTotal\|MemFree"
# Calculate: Kernel memory = MemTotal - MemFree - userspace (minimal init)

# More accurate with CONFIG_SLUB_STATS=y:
cat /sys/kernel/slab/*/total_objects
# Sum all slab allocations
```

**Fair comparison criteria**:
- Measure after boot, before userspace activity
- Include: kernel code, data, heap, slab cache, page tables
- Exclude: userspace memory (both systems have minimal init)

### BPF Load Time Measurement

**Axiom**:
```bash
# Run: /bin/benchmark
# Note "BPF load time: X us (avg of 10)"
```

**Linux**:
```bash
# Write equivalent BPF program test:
cat > test_bpf_load.c <<EOF
#include <stdio.h>
#include <time.h>
#include <linux/bpf.h>
#include <sys/syscall.h>

int main() {
    struct bpf_insn insns[] = {
        { .code = 0xb7, .dst_reg = 0, .imm = 42 },  // r0 = 42
        { .code = 0x95 },                            // exit
    };

    struct timespec start, end;
    for (int i = 0; i < 10; i++) {
        clock_gettime(CLOCK_MONOTONIC, &start);

        union bpf_attr attr = {
            .prog_type = BPF_PROG_TYPE_SOCKET_FILTER,
            .insn_cnt = 2,
            .insns = (uint64_t)insns,
            .license = (uint64_t)"GPL",
        };

        int fd = syscall(__NR_bpf, BPF_PROG_LOAD, &attr, sizeof(attr));

        clock_gettime(CLOCK_MONOTONIC, &end);

        if (fd < 0) {
            perror("bpf");
            return 1;
        }
        close(fd);

        uint64_t elapsed_us = (end.tv_sec - start.tv_sec) * 1000000 +
                               (end.tv_nsec - start.tv_nsec) / 1000;
        printf("Run %d: %lu us\n", i+1, elapsed_us);
    }
    return 0;
}
EOF
gcc -o test_bpf_load test_bpf_load.c
./test_bpf_load
```

**Fair comparison criteria**:
- Same BPF program (2 instructions)
- Same syscall interface
- Measured over multiple runs for consistency
- JIT enabled/disabled consistently

### Interrupt Latency Measurement

**Axiom**:
```bash
# Run: /bin/benchmark
# Note "Timer interval: X us (avg of 99)"
# This measures timer tick rate, not true latency
```

**Linux**:
```bash
# Use cyclictest for interrupt latency
apt-get install rt-tests
cyclictest -p 99 -t 1 -n -m -l 100000
# Reports min/avg/max latency
```

**Note**: Interrupt latency requires hardware timestamping. Axiom measurements from QEMU are not directly comparable to hardware. This benchmark should be deferred until Axiom runs on RPi5 hardware.

**Fair comparison criteria**:
- Same hardware (RPi5)
- Real-time kernel for Linux (PREEMPT_RT patch) vs Axiom (designed for determinism)
- Measure: time from hardware interrupt to first instruction of handler
- Average over 100,000 samples

---

## Comparison Table Template

Once measurements are collected on Raspberry Pi 5 hardware, fill in this table:

| Metric | Axiom (RPi5) | Linux (Minimal, RPi5) | Ratio (Axiom/Linux) | Notes |
|--------|--------------|----------------------|---------------------|-------|
| Boot time | [user measures] ms | [user measures] ms | [calculated] | Cold boot to first userspace process |
| Kernel memory | [user measures] KB | [user measures] KB | [calculated] | After boot, minimal userspace |
| BPF load time (2-insn) | [user measures] μs | [user measures] μs | [calculated] | Averaged over 10 runs |
| BPF load time (100-insn) | [user measures] μs | [user measures] μs | [calculated] | More complex program |
| Interrupt latency (avg) | [user measures] μs | [user measures] μs | [calculated] | cyclictest or equivalent |
| Interrupt latency (max) | [user measures] μs | [user measures] μs | [calculated] | Worst-case latency |

**Expected results** (based on proposal targets):
- Axiom boot time: <1s (vs Linux ~3-5s for minimal config)
- Axiom memory: <10MB (vs Linux ~50-100MB for minimal config)
- Axiom BPF load: <10ms (vs Linux ~10-50ms depending on verifier complexity)
- Axiom interrupt latency: <10μs (vs Linux ~10-100μs, or ~5-10μs with PREEMPT_RT)

---

## QEMU vs Hardware Differences

### QEMU Limitations
1. **Boot time**: QEMU emulation is slower than real hardware. Boot times in QEMU are 10-100x slower.
2. **Interrupt latency**: QEMU's virtualized interrupt handling does not reflect real hardware timing.
3. **Memory performance**: QEMU emulates memory access, so memory-intensive operations (BPF verifier) are slower.
4. **Timer precision**: QEMU's timer emulation has microsecond-level jitter.

### What QEMU is Good For
1. **Functional testing**: Verifying that all subsystems work correctly.
2. **Relative comparisons**: Comparing Axiom changes against each other (e.g., optimization A vs B).
3. **Development iteration**: Fast testing without hardware access.

### Hardware Testing Plan
Once Raspberry Pi 5 hardware is available:
1. Flash Axiom to SD card (same process as QEMU disk image)
2. Boot with serial console capture
3. Run benchmark suite: `/bin/benchmark`
4. Capture results and fill in comparison table
5. Repeat with minimal Linux (Buildroot) for comparison
6. Publish results in this document

---

## Proposal Alignment

### Target Metrics (from docs/proposal.md)

| Metric | Target | Stretch | Axiom (QEMU) | Axiom (RPi5) | Status |
|--------|--------|---------|--------------|--------------|--------|
| Kernel memory footprint | <10MB | <5MB | [TBD] | [TBD] | To be measured |
| Boot to init | <1s | <500ms | [TBD] | [TBD] | To be measured |
| BPF load time | <10ms | <1ms | [TBD] | [TBD] | To be measured |
| Interrupt latency | <10μs | <1μs | N/A (QEMU) | [TBD] | Hardware required |

### Next Steps

1. **Immediate** (Phase 3, Task 2):
   - Build Axiom kernel with benchmark program
   - Run on QEMU x86_64
   - Capture actual benchmark results
   - Fill in QEMU results in this document

2. **Short-term** (Phase 3 completion):
   - Test on Raspberry Pi 5 hardware
   - Capture hardware results
   - Compare against proposal targets

3. **Medium-term** (Phase 4):
   - Build minimal Linux for comparison
   - Run Linux benchmarks on same hardware
   - Fill in comparison table
   - Publish findings in academic paper (AgenticOS2026)

4. **Long-term** (Phase 5+):
   - Add more sophisticated benchmarks (e.g., context switch overhead, syscall latency)
   - Compare against Zephyr, FreeRTOS, seL4
   - Benchmark real robotics workloads (sensor fusion, motor control)

---

## Reproducibility

### Building and Running Benchmarks

**Build**:
```bash
# Clone Axiom repository
git clone https://github.com/your-repo/axiom-ebpf
cd axiom-ebpf

# Build for x86_64
cargo build --release

# Build for aarch64 (RPi5)
cargo build --release --no-default-features --features aarch64_deps
```

**Run on QEMU (x86_64)**:
```bash
# Configure init to run benchmark instead of default init
# Edit userspace/file_structure/src/lib.rs to set /bin/benchmark as init
# OR: Boot to default init and manually run:
cargo run --release -- qemu
# In kernel console, after boot:
# /bin/benchmark
```

**Run on Raspberry Pi 5**:
```bash
# Flash disk image to SD card
sudo dd if=target/disk.img of=/dev/sdX bs=4M status=progress
# Insert SD card into RPi5
# Connect serial console (GPIO 14/15, 115200 baud)
# Power on
# Capture serial output
# After boot, run:
# /bin/benchmark
```

### Automated Testing
```bash
# Future: Add CI pipeline to run benchmarks on every commit
# Store results in git for historical comparison
# Alert on performance regressions
```

---

## References

1. Axiom Proposal: `docs/proposal.md` - Target metrics and rationale
2. BPF Subsystem: `kernel/crates/kernel_bpf/` - Implementation details
3. Benchmark Program: `userspace/benchmark/` - Source code
4. Linux eBPF: https://docs.kernel.org/bpf/
5. Cyclictest: https://wiki.linuxfoundation.org/realtime/documentation/howto/tools/cyclictest
6. Buildroot: https://buildroot.org/ - Minimal Linux builder

---

**Document Status**: Template created. Awaiting QEMU benchmark results.
**Last Updated**: 2026-02-14
**Next Action**: Run Axiom kernel on QEMU and capture benchmark results to fill in [TBD] placeholders.
