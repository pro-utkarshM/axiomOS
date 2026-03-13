# Axiom Kernel Benchmarks

This document records benchmark results for the Axiom kernel and provides a reproducible methodology for comparing Axiom against Linux on identical hardware.

The goal is to measure:

* Kernel boot performance
* Memory footprint
* eBPF subsystem overhead
* Timer interrupt behavior

All results are reproducible and tied to specific environments.

---

# 1. Axiom Benchmark Results (QEMU x86_64)

## Test Environment

* **Platform:** QEMU x86_64 emulator
* **Memory:** 2 GB
* **Kernel:** Axiom kernel (dev branch)
* **Measurement Tool:** `userspace/benchmark` program
* **Date:** 2026-03-06

## Benchmark Results

| Metric                   | Result              | Target (Proposal)                | Status         |
| ------------------------ | ------------------- | -------------------------------- | -------------- |
| Boot to init             | 45 ms               | <1 s (target), <500 ms (stretch) | Measured       |
| Kernel heap usage        | 2231 KB             | <10 MB (target), <5 MB (stretch) | Measured       |
| BPF load time            | 3787 µs (avg of 10) | <10 ms (target), <1 ms (stretch) | Measured       |
| Timer interrupt interval | 495 µs              | <10 µs latency (target)          | Emulated timer |

### Notes

These results come from a **virtualized environment**.
QEMU emulation introduces timing distortions for interrupts and memory access.

Therefore:

* Boot time is slower than hardware
* Interrupt timing is not accurate
* Results are mainly useful for **development iteration**

Hardware measurements on Raspberry Pi 5 provide the authoritative numbers.

---

# 2. Axiom Benchmark Results (Raspberry Pi 5)

## Test Environment

* **Platform:** Raspberry Pi 5 Model B Rev 1.0 (8GB)
* **Kernel:** `axiom-ebpf`
* **Build Command**

```bash
./scripts/build-rpi5.sh release --features embedded-rpi5
```

* **Storage:** FAT32 boot partition with deployed `kernel8.img`
* **Capture:** Raspberry Pi Debug Probe UART
* **Console:** 115200 baud serial

UART capture device:

```
/dev/serial/by-id/usb-Raspberry_Pi_Debug_Probe__CMSIS-DAP__E6633861A355B838-if01
```

Instrumentation:

* kernel markers
* userspace `/bin/benchmark` program
* BPF timer probe

---

## Benchmark Results (Hardware)

| Metric                   | Result         | Notes                           |
| ------------------------ | -------------- | ------------------------------- |
| Boot to init             | 99 ms          | Measured via kernel timer       |
| Kernel heap usage        | 12290 KB       | Current allocation at init      |
| Kernel image size        | 10 MB          | Total binary footprint          |
| BPF load time            | 0 µs avg       | Min: 0 µs, Max: 2 µs            |
| Timer interrupt interval | 9999 µs avg    | Min: 9999 µs, Max: 10000 µs     |
| Timer samples            | 100            | collected via BPF               |

---

## Raw Benchmark Output

```
AXIOM KERNEL METRICS
Boot to init: 99 ms
Kernel heap: 12290 KB
Kernel image: 10 MB

========================================
  AXIOM BENCHMARK RESULTS
========================================

[Benchmark 1] BPF Program Load Time

Loading test program 10 times...

Run 1: 2 us
Run 2: 0 us
...
Run 10: 0 us

BPF Load Time Summary

Min: 0 us  
Max: 2 us  
Avg: 0 us  

[Benchmark 2] Timer Interrupt Interval

Collecting 100 timer samples...

Timer Interrupt Interval Summary

Samples: 100  
Min: 9999 us  
Max: 10000 us  
Avg: 9999 us
```

---

## Observations

Hardware measurements confirm the correct operation of multiple kernel subsystems:

* ARM Generic Timer
* GIC interrupt controller
* eBPF runtime
* userspace scheduling
* syscall path
* timer-driven BPF execution

Timer frequency is approximately:

```
100 Hz
```

The results show extremely stable timing with **1 µs jitter**.

BPF program load overhead is effectively **negligible** in interpreter mode.

---

# 3. Linux Baseline Results (RPi5)

## Test Environment

* **Platform:** Raspberry Pi 5 Model B Rev 1.0 (8GB)
* **OS:** Raspberry Pi OS 64-bit
* **Kernel:** Linux 6.12.62+rpt-rpi-2712
* **Tools:** `dmesg`, `cyclictest`, `gcc`
* **Runs:** 5 cold-boot measurements
* **Date:** 2026-03-09

---

## Benchmark Results

| Metric                      | Result     | Notes                 |
| --------------------------- | ---------- | --------------------- |
| Boot to init                | 573.124 ms | dmesg timestamp delta |
| MemTotal                    | 8256464 KB | `/proc/meminfo`       |
| MemFree                     | 7089104 KB | `/proc/meminfo`       |
| Used (rough)                | 1167360 KB | MemTotal − MemFree    |
| Slab                        | 71136 KB   | `/proc/meminfo`       |
| BPF load time (2 insn)      | 24.80 µs   | average of 10 loads   |
| BPF load time (2 insn warm) | 19.78 µs   | runs 2-10             |
| BPF load time (100 insn)    | 56.60 µs   | average of 10 loads   |
| Interrupt latency avg       | 2 µs       | cyclictest            |
| Interrupt latency max       | 7 µs       | cyclictest            |

---

# 4. Comparison Snapshot

| Metric            | Axiom (RPi5) | Linux (RPi5)         | Notes                          |
| ----------------- | ------------ | -------------------- | ------------------------------ |
| Boot time         | 99 ms        | 573 ms               | measured to init process spawn |
| Kernel memory     | ~22 MB       | ~1.1 GB system usage | image + heap (Axiom)           |
| BPF load time     | ~0 µs        | 24.8 µs              | interpreter vs full verifier   |
| Timer interval    | 9999 µs      | configurable         | kernel tick                    |
| Interrupt latency | TBD          | avg 2 µs             | cyclictest                     |

---

# 5. Host Microbenchmarks (Verifier)

Host-side Criterion benchmarks measure verifier scaling.

## Test Environment

* **Host:** x86_64 Linux
* **Command**

```
cargo bench -p kernel_bpf --bench verifier --features embedded-profile
```

* **Tool:** Criterion.rs

---

## Results

| Benchmark                           | Time (95% CI) |
| ----------------------------------- | ------------- |
| verifier/small/minimal              | 218-220 ns    |
| verifier/small/arithmetic           | 265-268 ns    |
| verifier/instructions/10            | 273-274 ns    |
| verifier/instructions/50            | 474-476 ns    |
| verifier/instructions/100           | 747-753 ns    |
| verifier/instructions/500           | 2.69-2.73 µs  |
| verifier/instructions/1000          | 5.11-5.12 µs  |
| verifier/control_flow/linear        | 232-233 ns    |
| verifier/control_flow/single_branch | 906-909 ns    |
| verifier/control_flow/multi_branch  | 974-976 ns    |

---

# 6. Measurement Methodology

## Boot Time

Measured from kernel entry point to first userspace process spawn.

Includes:

* memory initialization
* scheduler setup
* BPF subsystem init
* driver initialization

---

## Memory Footprint

Measured from kernel heap allocator statistics.

Includes:

* BPF programs
* kernel objects
* process control blocks

Excludes:

* static kernel image
* frame allocator metadata

---

## BPF Load Time

Measures the overhead of:

```
sys_bpf(BPF_PROG_LOAD)
```

Includes:

* instruction parsing
* verifier execution
* program object creation
* JIT compilation (if enabled)

Test program:

```
r0 = 42
exit
```

---

## Timer Interrupt Interval

Measured using a BPF program attached to the timer hook.

Procedure:

1. BPF program records timestamps using `bpf_ktime_get_ns()`
2. Writes events to ring buffer
3. Userspace computes interval between samples

---

# 7. QEMU vs Hardware

## QEMU Limitations

* virtualized interrupts
* slower memory access
* synthetic timers

Therefore QEMU is used only for:

* functional testing
* development iteration
* regression detection

Hardware measurements are authoritative.

---

# 8. Reproducibility

## Build

```
git clone https://github.com/axiom/axiom-ebpf
cd axiom-ebpf
cargo build --release
```

---

## Run Host Benchmarks

```
cargo bench -p kernel_bpf --bench verifier --features embedded-profile
```

---

## Run on QEMU

```
cargo run --release -- qemu
/bin/benchmark
```

---

## Run on Raspberry Pi 5

Flash kernel:

```
sudo dd if=target/disk.img of=/dev/sdX bs=4M status=progress
```

Capture UART:

```
sudo timeout 70s cat $PORT | tr -d "\r" | tee uart.clean.log
```

Look for:

```
AXIOM BENCHMARK RESULTS
```

---

# 9. Proposal Targets

| Metric            | Target | Stretch |
| ----------------- | ------ | ------- |
| Kernel memory     | <10 MB | <5 MB   |
| Boot to init      | <1 s   | <500 ms |
| BPF load          | <10 ms | <1 ms   |
| Interrupt latency | <10 µs | <1 µs   |

---

# 10. Future Benchmarks

Planned additional measurements:

* syscall latency
* context switch overhead
* IPC performance
* scheduler fairness
* robotics control loop latency

Long-term comparisons planned against:

* Linux
* Zephyr
* FreeRTOS
* seL4

---

# References

* Axiom Proposal (`docs/proposal.md`)
* Linux eBPF documentation
* Cyclictest realtime benchmarks
* Criterion.rs benchmarking framework

---

**Document Status:** Hardware benchmark validated on Raspberry Pi 5

**Last Updated:** 2026-03-13

**Next Action:** Add boot timing and interrupt latency measurements.
