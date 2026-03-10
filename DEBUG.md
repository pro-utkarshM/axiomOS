# DEBUG LOG: Raspberry Pi 5 Bring-up (axiom-ebpf)

Last updated: 2026-03-10
Repo: `/home/utkarsh/Work/axiom-ebpf`
Branch: `bench-host-results-qemu-debug`
Base commit at capture time: `b7f7f3c`

This file is a full handoff log of what happened in this debugging thread so a new engineer can continue without re-reading chat history.

## 1. Objective

Boot a custom `axiom-ebpf` kernel image (`kernel8.img`) on Raspberry Pi 5 using SD boot and capture early UART logs via the official Raspberry Pi Debug Probe.

Primary symptom:
- Firmware/BL31 logs appear.
- Custom kernel logs do not appear.
- System appears to panic very early.

## 2. Initial Environment and Storage State

User provided `lsblk -f`:
- SD card appeared as:
  - `/dev/sda1` mounted as `vfat` (FAT32), UUID `A1E9-C253`
- Host root disk is encrypted NVMe and unrelated to SD boot.

Initial direct question was: what disk format should be used for flashing?
- Answer used throughout: FAT32 boot partition is required for Pi firmware boot files.

## 3. Build and Deploy Flow Used

Scripts in repo:
- `scripts/build-rpi5.sh`
- `scripts/deploy-rpi5.sh`

`build-rpi5.sh` behavior:
- Builds `kernel` crate for `aarch64-unknown-none` with `--features embedded-rpi5`.
- Creates raw binary:
  - `target/aarch64-unknown-none/<profile>/kernel8.img`

`deploy-rpi5.sh` behavior:
- Copies `kernel8.img` to SD boot mount.
- Creates `config.txt` if missing.
- Does not automatically install full Raspberry Pi firmware files.

## 4. SD Card Population History

### 4.1 First deploy attempts

User ran deploy script successfully:
- `kernel8.img` and `config.txt` copied to `/mnt/rpi5-boot`.
- However, firmware files were missing on SD:
  - `start4.elf`
  - `fixup4.dat`
  - `bcm2712-rpi-5-b.dtb`

### 4.2 Firmware copy attempts

Actions:
1. Cloned firmware repo:
   - `git clone --depth 1 --branch stable https://github.com/raspberrypi/firmware.git`
2. Tried `rsync` to SD:
   - failed: `rsync: command not found`
3. Switched to `tar | tar` copy.
4. First `tar` failed on ownership changes.
5. Final working command:
   - destination `tar` with `--no-same-owner --no-same-permissions`

After successful copy, file checks confirmed:
- `kernel8.img` present
- `config.txt` present
- `bcm2712-rpi-5-b.dtb` present
- overlays present

## 5. Kernel Image Integrity Checks

User validated image hashes:
- Local build image:
  - `target/aarch64-unknown-none/release/kernel8.img`
  - `sha256: 8ead947e9e85ee991c27573fb0ebb18c5f94ec1dd073a791c6d55a29acc9a9f0`
- SD copy image:
  - `/mnt/rpi5-boot/kernel8.img`
  - same hash as above at that point.

This verified at least one deploy cycle copied exact bytes.

## 6. UART Capture and Boot Evidence

Serial logs repeatedly showed:
- Pi boot ROM + bootloader stages run.
- SD/GPT/FAT32 parsing successful.
- `config.txt` read.
- `bcm2712-rpi-5-b.dtb` read.
- `kernel8.img` read and relocated.
- `Starting OS ...`
- BL31 prints:
  - `NOTICE: BL31: v2.6(release):v2.6-240-gfc45bc492`
  - `NOTICE: BL31: Built : 12:55:13, Dec 4 2024`

But no normal kernel log banner/output appeared.

## 7. Notable Firmware Log Warnings

Warning frequently seen:
- `cmdline.txt not found`
- or `Failed to read command line file 'cmdline.txt'`

Conclusion during debugging:
- This is not the primary blocker for booting bare-metal kernel image.
- It is a warning/noise item, not the root cause of early panic.

## 8. config.txt Iterations

User replaced config with minimal explicit content:

```ini
arm_64bit=1
kernel=kernel8.img
uart_2ndstage=1
enable_uart=1
pciex4_reset=0
gpu_mem=16
disable_splash=1
```

Observed effects:
- Firmware still booted and loaded kernel image.
- Overlay lines changed behavior (at times `disable-bt-pi5` load disappeared when omitted).
- Symptom (no kernel logs) remained.

## 9. Serial Capture Hygiene Issues and Fixes

Problems encountered:
- Corrupted/garbled characters in log due repeated/stacked capture sessions.
- Multiple `screen` sessions and appended logs created noisy diagnostics.
- `sudo tee` created root-owned log file, later causing `Permission denied`.

Improvements applied:
- Use stable by-id serial path:
  - `/dev/serial/by-id/usb-Raspberry_Pi_Debug_Probe__CMSIS-DAP__E6633861A355B838-if01`
- Clear log file before each capture.
- Use one-shot capture (`timeout ... cat`) for deterministic parsing.
- Fix file ownership when needed:
  - `sudo chown "$USER":"$USER" uart.clean.log`

## 10. Marker-Based Instrumentation Strategy

Because text logs from kernel were missing, direct UART character markers were added to isolate crash point.

### 10.1 Existing early markers

From `boot.S` and `boot.rs`/`main.rs`:
- `{|}` from early assembly stages
- `~123456` from Rust early boot path
- `7` on entry to `kernel_main`
- `8` after `kernel::init` returns
- `!` from panic handler

Observed progression over time:
1. `{|}~1234567!`
   - panic occurs inside `kernel::init()`
2. After adding finer markers:
   - `{|}~1234567a!`
   - panic between `a` and `b` (`init_boot_time`)
3. After boot-time workaround:
   - `{|}~1234567ab!`
   - panic between `b` and `c` (around `log::init`)

### 10.2 Character-Marker Debugging: Detailed Playbook

This project used "single-byte breadcrumb tracing" over UART because formatted logs were unavailable in the failing window.

Core idea:
- Emit one known character at each important boundary.
- Capture raw UART stream.
- Read the longest marker subsequence.
- The first missing character identifies the failing transition.

Why this works well in early boot:
- No allocator needed.
- No logger initialization needed.
- No string formatting needed.
- Survives very early panics where normal logging is impossible.

### 10.3 Marker Alphabet and Semantics Used Here

Reserved markers in this thread:
- `{|}`: assembly milestones in `boot.S`.
- `~`: entry into Rust early boot (`_start`).
- `1..6`: `_start` sub-steps in `boot.rs`.
- `7`: entry to `kernel_main`.
- `8`: `kernel::init()` returned to `kernel_main`.
- `!`: panic handler reached.

Extended markers introduced:
- `a..m`: high-level phases in `kernel::init()`.
- `n..q`: internal phases in `Aarch64::init()`.
- `r..x`: memory subsystem sub-steps in `mem::init()`.
- `y,z,Z`: internal `log::init()` checkpoints.

Interpretation rule:
- If sequence ends with `X!`, panic occurred after marker `X` and before next expected marker.
- If marker stream stops without `!`, system likely hung/rebooted before panic handler.

### 10.4 Placement Rules (What To Do / Avoid)

Good placement:
- Place markers immediately before and after suspicious calls.
- Place one marker per boundary, not per line.
- Keep ordering strictly monotonic (never reuse same marker in one path).
- Put a marker at panic handler entry (`!`) to prove panic path.

Avoid:
- Emitting markers from code that depends on the subsystem being debugged.
  - Example: do not rely on logging macros when debugging logger init.
- Putting markers only at function entry.
  - Entry-only markers cannot distinguish call-site failures.
- Reusing markers across concurrent paths (causes ambiguity).

Practical granularity progression:
1. Coarse markers across subsystems.
2. If narrowed, add medium markers inside failing subsystem.
3. If still ambiguous, bracket individual function calls.

### 10.5 Emission Method Used

For Pi 5 debug probe capture path, markers were emitted with direct MMIO writes to UART10 DR register:
- Address used: `0x10_7D00_1000`
- Pattern used:
  - Rust:
    - `unsafe { (0x10_7D00_1000 as *mut u32).write_volatile(0xNN); }`
  - AArch64 assembly:
    - `str w10, [x9]` after loading UART base and byte value.

Reason this method was chosen:
- It bypasses higher-level serial abstractions and locks.
- It avoids deadlocks/recursion with logger/formatting paths.

### 10.6 Capture and Parsing Method

Canonical capture command:

```bash
PORT=/dev/serial/by-id/usb-Raspberry_Pi_Debug_Probe__CMSIS-DAP__E6633861A355B838-if01
: > uart.clean.log
sudo timeout 20s cat "$PORT" | tr -d '\r' | tee uart.clean.log >/dev/null || true
grep -ao '{|}~[0-9A-Za-z!]*' uart.clean.log | tail -n 1
```

Why this exact form:
- by-id port avoids tty index drift (`ttyACM0/1` swaps).
- `timeout` bounds capture to one boot window.
- `tr -d '\r'` removes CR noise.
- `grep -ao` extracts only marker substrings from noisy mixed logs.
- `tail -n 1` selects latest boot attempt.

Important operational details:
- Do not run multiple readers (`screen` + `cat`) simultaneously on same tty.
- Ensure log file is user-writable (avoid `sudo tee` ownership trap).
- Always truncate log before each attempt.

### 10.7 Decoding Procedure

Given expected order:
- `{|}~1234567abcdefgh...`

Decode steps:
1. Find last marker substring in file.
2. Compare against expected sequence from left to right.
3. Identify first missing marker `M`.
4. Root cause lies between previous marker and `M`.
5. Add new markers around that smaller region.
6. Repeat until one function/call remains.

Examples from this session:
- `{|}~1234567!`
  - failure region: inside `kernel::init()`.
- `{|}~1234567a!`
  - failure region: after `init_boot_time` entry, before return.
- `{|}~1234567ab!`
  - failure region: during `log::init()`.

### 10.8 Failure Modes Seen While Using Markers

1. Stale binary deployed
- Symptom: marker pattern does not reflect newest code edits.
- Mitigation:
  - compare SHA256 local vs SD image every cycle.

2. Mixed capture sessions
- Symptom: interleaved/garbled text and false marker interpretation.
- Mitigation:
  - single capture process; stop old `screen` sessions.

3. Root-owned capture file
- Symptom: `tee: Permission denied`.
- Mitigation:
  - `sudo chown "$USER":"$USER" uart.clean.log`.

4. Timeout exit masking pipeline
- Symptom: grep skipped due `&&` chain.
- Mitigation:
  - use `|| true` before grep in alias.

5. Marker collision with firmware output
- Symptom: false positives if too-common chars are chosen.
- Mitigation:
  - parse full structured pattern (`{|}~...`) instead of single-char grep.

### 10.9 Recommended Next Marker Expansion (if needed)

If `ab!` persists even after verified fresh image:
- use `y/z/Z` already added in `log::init()`.
- expected discriminators:
  - `aby!` means panic inside/after first logger line.
  - `abyz!` means panic after `set_logger`.
  - `abyzZ!` means panic after `set_max_level`, later in path.

If no `y` appears despite new build + matching SHA:
- assume wrong boot medium/image or wrong serial source.
- re-validate SD device path and capture port path.

## 11. Code Changes Made During This Session

The branch already had multiple ongoing Pi5 debug edits. Relevant files currently modified:

- `kernel/linker-aarch64.ld`
- `kernel/src/arch/aarch64/boot.S`
- `kernel/src/arch/aarch64/boot.rs`
- `kernel/src/arch/aarch64/mod.rs`
- `kernel/src/arch/aarch64/platform/rpi5/memory_map.rs`
- `kernel/src/arch/aarch64/platform/rpi5/mod.rs`
- `kernel/src/arch/aarch64/platform/rpi5/uart.rs`
- `kernel/src/lib.rs`
- `kernel/src/log.rs`
- `kernel/src/main.rs`
- `kernel/src/mem/mod.rs`
- `kernel/src/time.rs`

Plus untracked:
- `kernel/crates/kernel_bpf/target/`

### 11.1 Changes applied in this chat (high signal)

1. Added fine-grained debug markers in:
   - `kernel/src/lib.rs` (`a..m`)
   - `kernel/src/arch/aarch64/mod.rs` (`n..q`)
   - `kernel/src/mem/mod.rs` (`r..x`)
2. Changed AArch64/non-x86 boot-time handling to avoid early `OnceCell` init panic:
   - `init_boot_time()` non-x86 branch is now no-op.
   - `kernel/src/time.rs` non-x86 `TimestampExt::now()` now uses epoch `0` directly.
3. Changed logger init to avoid panic on `set_logger` failure:
   - `kernel/src/log.rs`: replaced `unwrap()` with ignored result.
4. Added internal logger-stage markers in `log::init()`:
   - `y` at entry
   - `z` after `set_logger`
   - `Z` after `set_max_level`

## 12. Build Validation Status

Repeated checks run successfully:
- `cargo check --target aarch64-unknown-none --features embedded-rpi5 -p kernel`
- No build errors.
- Warnings exist but are non-fatal.

## 13. Current State (latest known)

Latest marker reported by user:
- `{|}~1234567ab!`

Interpretation:
- panic after `init_boot_time` and before marker `c`.
- expected to be around `log::init`.

Important caveat:
- Since `y/z/Z` markers were just added to `log::init`, next boot result must be checked for these markers to disambiguate:
  - if `y` appears: we are definitely executing latest image and panicking inside logger init path.
  - if still only `ab!` without `y`: likely stale image deployed, wrong SD/device, or capture from previous boot file.

## 14. Reliable Repro/Verification Commands

Use this sequence for trustworthy state:

```bash
cd /home/utkarsh/Work/axiom-ebpf

./scripts/build-rpi5.sh release
sha256sum target/aarch64-unknown-none/release/kernel8.img

sudo mount /dev/sda1 /mnt/rpi5-boot
sudo ./scripts/deploy-rpi5.sh /mnt/rpi5-boot release
sha256sum /mnt/rpi5-boot/kernel8.img
sync && sudo umount /mnt/rpi5-boot

PORT=/dev/serial/by-id/usb-Raspberry_Pi_Debug_Probe__CMSIS-DAP__E6633861A355B838-if01
: > uart.clean.log
sudo timeout 20s cat "$PORT" | tr -d '\r' | tee uart.clean.log >/dev/null || true
grep -ao '{|}~[0-9A-Za-z!]*' uart.clean.log | tail -n 1
```

Alias form used:

```bash
alias axiom-log='sudo timeout 20s cat "$PORT" | tr -d "\r" | tee uart.clean.log >/dev/null || true; grep -ao "{|}~[0-9A-Za-z!]*" uart.clean.log | tail -n 1'
```

Notes:
- Do not run `sudo axiom-log` (aliases are shell-expanded, not sudo commands).
- If `uart.clean.log` is root-owned:
  - `sudo chown "$USER":"$USER" uart.clean.log`

## 15. Next Debug Checkpoint

Needed from next run:
1. Marker output with latest binary (must show whether `y/z/Z` appear).
2. Both SHA256 lines (local build and SD copy).

This will determine whether the issue is:
- true runtime panic in logger path, or
- deployment/capture mismatch still using old image.
