# Axiom RPi5 Implementation Plan

Last updated: 2026-03-11
Owner: kernel bring-up track
Scope: move from stable Pi5 boot-to-idle to publishable Pi5 benchmark results

## 1. Current Baseline

- Current stable trace: `{|}~1234567abyzZcdenrRAJKLTVWXUMVWXNOPBCDEFGHIsuvwxopqfghijklRm89ABCDESsjZ01TUuqrst0QX`
- Interpretation:
  - Early boot and MMU were stable before scheduler handoff.
  - `SsjZ01` proves PID 1 is selected and the switch lands on the init process.
  - `TUuqrst0QX` proves task/process trampolines run, TTBR0 is switched, and control reaches pre-`eret` userspace handoff.
  - Block devices are wired (`n` disappears) so the kernel is no longer stuck waiting for `BlockDevices::by_id(0)`.
  - The remaining gap is post-`eret` EL0/syscall visibility (`w`/`p`) and benchmark banner capture.

## 2. Milestones

## M1: Pi5 boot stable (Done)

- Status: complete
- Evidence: consistent `...nF` marker and no panic marker (`!`) in stable runs.

## M2: Block device registration + rootfs mount (Complete)

- Status: **complete**
- Evidence: UART now shows `{|}~...ESsjZ01` so PID 1 is scheduled and the rootfs path is available; the `n` marker is gone.
- Implementation: disk image already wires the simulated block device, and the kernel now keeps UART mapped during the forced switch for reliable logging.

## M3: Userspace benchmark binary on Pi5 (In Progress)

- Goal: execute `/bin/benchmark` reliably on Pi5.
- Exit criteria:
  - System reaches userspace init on Pi5.
  - `/bin/benchmark` runs end-to-end without manual recovery steps.
  - 5 repeated runs succeed.

Work items:
1. ~~Image contents validation~~ *(done)*
   - Benchmark binary present in rootfs image.
   - Init updated to spawn `/bin/benchmark` instead of `/bin/iio_demo`.
2. ~~Fix `get_kernel_time_ns()` for aarch64~~ *(done)*
   - Wired ARM generic timer (`cntvct_el0`/`cntfrq_el0`) into `Timestamp::now()`.
   - Unblocks `clock_gettime`, `nanosleep`, `msleep`, and `bpf_ktime_get_ns`.
3. Runtime validation
   - Capture UART after forced scheduler probe and look for `...SsjZ01TUu`.
   - Confirm progression reaches `...SsjZ01TUuqrst0QX`.
   - Grep for `AXIOM BENCHMARK RESULTS`, `BPF Load Time Summary`, `Timer Interrupt Interval`, and the `w`/`p` markers emitted by `kernel/src/syscall/mod.rs`.
   - If still stuck at `...0QX`, use vector markers (`6789A`) from `exception_vectors.S` to classify whether lower-EL sync entry is reached.
   - Collect the benchmark summary printed by `userspace/benchmark`.
4. Consistency checks
   - Run 5 cold boots with identical image hash and capture setup; ensure each trace includes the same marker sequence plus benchmark outputs.

## M4: Publish matched Axiom vs Linux benchmark table

- Goal: fill `docs/benchmarks.md` with matched Pi5 measurements and ratios.
- Exit criteria:
  - Axiom Pi5 metrics captured with same methodology/repetition class as Linux baseline.
  - Table updated with mean, sample count, and notes.
  - Repro steps documented.

Work items:
1. Measurement harness
   - Standardize commands for capture and parsing.
   - Require local vs SD image hash check before each run set.
2. Data collection
   - Collect at least 5 cold-boot runs for each metric.
3. Reporting
   - Update `docs/benchmarks.md` with raw summary and ratios.
   - Link run artifacts/log snippets.

## 3. Execution Sequence

1. Complete M2 before any new performance claims.
2. After M2, run M3 until benchmark execution is stable.
3. After M3, perform matched benchmark campaign for M4.
4. Only then update proposal-level performance claims.

## 4. Verification Protocol (Per Run)

1. Build image.
2. Verify local kernel hash.
3. Deploy to SD.
4. Verify SD kernel hash matches local hash.
5. Capture UART (debug probe) ensuring the marker sequence `...SsjZ01TUu`.
6. Confirm `AXIOM BENCHMARK RESULTS` and `BPF Load Time Summary` appear alongside `w/p` syscall markers.
7. Store run metadata (timestamp, hash, boot result, notes).

Reference commands:

```bash
./scripts/build-rpi5.sh release
sha256sum target/aarch64-unknown-none/release/kernel8.img

sudo mount /dev/sda1 /mnt/rpi5-boot
sudo cp -v target/aarch64-unknown-none/release/kernel8.img /mnt/rpi5-boot/kernel8.img
sync
sha256sum /mnt/rpi5-boot/kernel8.img
sudo umount /mnt/rpi5-boot

PORT=/dev/serial/by-id/usb-Raspberry_Pi_Debug_Probe__CMSIS-DAP__E6633861A355B838-if01
: > uart.clean.log
sudo timeout 20s cat "$PORT" | tr -d '\r' | tee uart.clean.log >/dev/null || true
grep -ao '{|}~[0-9A-Za-z!]*' uart.clean.log | tail -n 1
```

## 5. Risks and Mitigations

1. Stale image deployed
   - Mitigation: mandatory hash match check (local vs SD) before boot.
2. Corrupted serial capture due multiple readers
   - Mitigation: single capture process and by-id serial port path.
3. Debug logging reintroduces panic recursion
   - Mitigation: keep marker-first strategy until storage/init path is stable.
4. Partial storage bring-up gives intermittent behavior
   - Mitigation: require repeated-run pass criteria (5/5) before advancing milestone.

## 6. Deliverables

- `implementation.md` (this plan)
- Code changes for M2/M3
- Updated `docs/benchmarks.md` with Pi5 Axiom measurements (M4)
- Updated `docs/proposal.md` milestone status once M2+M3 are completed

## 7. Definition of Done

- M2 done: storage registered and rootfs mounted on Pi5.
- M3 done: `/bin/benchmark` runs reliably on Pi5 across repeated cold boots.
- M4 done: matched Axiom vs Linux table published with reproducible methodology.
