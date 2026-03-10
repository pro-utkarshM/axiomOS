# Axiom RPi5 Implementation Plan

Last updated: 2026-03-10
Owner: kernel bring-up track
Scope: move from stable Pi5 boot-to-idle to publishable Pi5 benchmark results

## 1. Current Baseline

- Current stable marker: `{|}~1234567abyzZcdenrRAJKLTVWXUMVWXNOPBCDEFGHIsuvwxopqfghijklm89ABnF`
- Interpretation:
  - Early boot, MMU, and `kernel::init()` complete.
  - Post-init path executes.
  - `n` indicates missing `BlockDevices::by_id(0)`.
  - `F` indicates intentional idle fallback (no panic).
- Root blocker: Pi5 storage registration/mount path is not complete, so `/bin/init` is not active.

## 2. Milestones

## M1: Pi5 boot stable (Done)

- Status: complete
- Evidence: consistent `...nF` marker and no panic marker (`!`) in stable runs.

## M2: Block device registration + rootfs mount (Next)

- Goal: make block device 0 available and mount root filesystem on Pi5.
- Exit criteria:
  - `BlockDevices::by_id(0)` returns a valid block device.
  - Root filesystem mounts successfully on Pi5 hardware.
  - Kernel transitions past current idle fallback path.

Work items:
1. Audit registration path
   - Trace where block devices are registered in x86_64/QEMU path.
   - Compare with AArch64 RPi5 path and identify missing init/driver hookup.
2. Implement or connect Pi5 storage backend
   - Preferred: register SD card backend first.
   - Alternative: register a currently working backend (USB/NVMe) if SD path is not ready.
3. Add guarded debug output
   - Add minimal non-recursive markers around registration and mount path.
   - Keep verbose logging disabled until path is stable.
4. Validate mount path
   - Confirm root mount success and no fallback to idle.

## M3: Userspace benchmark binary on Pi5

- Goal: execute `/bin/benchmark` reliably on Pi5.
- Exit criteria:
  - System reaches userspace init on Pi5.
  - `/bin/benchmark` runs end-to-end without manual recovery steps.
  - 5 repeated runs succeed.

Work items:
1. Image contents validation
   - Verify benchmark binary is present in rootfs image.
   - Verify init flow can launch benchmark path.
2. Runtime validation
   - Collect serial evidence for init spawn and benchmark start/stop.
3. Consistency checks
   - Run 5 cold boots with identical image hash and capture setup.

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
5. Capture UART using single-reader by-id port setup.
6. Parse marker/log output.
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
