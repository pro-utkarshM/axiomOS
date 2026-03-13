# DEBUG LOG: Raspberry Pi 5 Bring-up (axiom-ebpf)

Last updated: 2026-03-11
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

## 11. Latest Observations (March 11, 2026)

- The forced scheduler probe (`S`/`Y`) now runs just after rootfs mount and the UART stream reached `...SsjZ01TUu`. That proves PID=1 was scheduled, the task-entry trampoline executed, and control reached the `/bin/init` trampoline in userspace.
- Added root-vs-user marker logging inside `kernel/src/mcore/mtask/scheduler/mod.rs` (`k/j` + `Zhh`) so we can tell whether the first switched task is the kernel worker or init process. The first `SsjZ01` run confirmed non-root (PID 1) was chosen, so the scheduler is not starving init.
- Added `T`/`U` instrumentation at the start of `task_entry_trampoline` (`kernel/src/arch/aarch64/context.rs`) along with `u` from the trampoline itself so we know whether we ever reach the process trampoline.
- Discovered that Pi5 kernel lives at physical/virtual low addresses (`0x0008_0000`) while each user process maps TTBR0 to a fresh high-identity mapping. Switching TTBR0 before trampoline meant UART/MMIO (debug probe) was no longer memory-mapped, so further markers disappeared. Short-term mitigation: for `feature="rpi5"` the scheduler now leaves `cr3_value` at zero (no TTBR0 switch) so the kernel still sees UART while we observe confirmation markers.
- IIO simulation work queue was running on Pi5 before init, so initial scheduling always picked that task. We now skip spawning the simulated IIO worker on Pi5 builds (`kernel/src/driver/iio.rs` gating imports/task spawn). Cleanup worker only runs when real cleanup work exists, and `TaskCleanup::enqueue` now lazily schedules it.
- `kernel/src/time.rs` got a working `Timestamp::now()` for AArch64 via `cntvct_el0/cntfrq_el0`, unblocking nanosleep/bpf_ktime_get_ns. `userspace/init` now spawns `/bin/benchmark` so we can exercise userspace signal paths once scheduling is healthy.
- Next immediate validation steps: capture UART again and confirm the `AXIOM BENCHMARK RESULTS` banner is present, `BPF Load Time Summary`/`Timer Interrupt Interval` lines print, and the `w`/`p` markers follow `...SsjZ01TUu`. If any portion is still missing (e.g., `w`/`p` don't appear), instrument the syscall path and benchmark loader to see whether the userspace payload is stalling or losing MMIO access.
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

### 10.10 Current Long-Marker Decode (authoritative)

Latest long marker seen in stable runs:
- `{|}~1234567abyzZcdenrRAJKLTVWXUMVWXNOPBCDEFGHIsuvwxopqfghijklm89ABnF`

Decode by segment:
- `{|}~1234567`
- assembly to Rust handoff through `kernel_main`.
- `abyzZ`
- entry through logger-init checkpoints; logger registration path no longer panics.
- `cdenrRAJKLTVWXUMVWXNOPBCDEFGHI`
- platform + memory + MMU bring-up path now passes through the previously failing windows.
- `suvwxopqfghijklm8`
- late `kernel::init()` phases complete and return to `kernel_main`.
- `9ABnF`
- post-init path executes; storage device `id=0` missing (`n`), then intentional idle (`F`).

Regression cues:
- Ends at `...ab!`: logger-init regression.
- Ends near `...TV!`/`...WX!`: phys/mm reserved-region regression.
- Ends at `...H!`/`...HI!`: MMU mapping regression.
- Reaches `...nF` without `!`: expected behavior until storage registration is implemented.

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

Latest stable marker reported:
- `{|}~1234567abyzZcdenrRAJKLTVWXUMVWXNOPBCDEFGHIsuvwxopqfghijklm89ABnF`

Interpretation of latest marker:
- Bootloader + handoff works.
- MMU enable now succeeds (`...H I ...`).
- Full `kernel::init()` completes and returns (`...lm8`).
- AArch64 post-init path executes (`9AB`).
- No block device `id=0` exists on current Pi5 runtime path, so marker `n` is emitted.
- Kernel then intentionally idles (`F` + `turn_idle()`).

Current high-level status:
- Kernel no longer dies in early bring-up path.
- Boot now reaches steady-state idle loop.
- Remaining bring-up gap is storage stack/device registration on Pi5 (so rootfs/init process path can run).

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

Needed for next functional milestone (booting `/bin/init`):
1. Ensure a Pi5 block device driver registers `BlockDevices::by_id(0)`, or change rootfs strategy.
2. Re-enable selective logging safely (currently disabled for early stability on Pi5).
3. Remove/trim temporary marker instrumentation once storage path works.

This will determine whether kernel can proceed from idle bring-up into full userspace init.

## 16. Extended Timeline After Initial Snapshot

The earlier snapshot ended around `...ab!`. The following happened afterwards:

1. Verified stale image mismatch:
- Built image: `358136`, hash `d7f52af3b347c11aac88fea1e2e4220d3296cceb55fa704a6572d3e906db9ce1`
- SD image initially: `358248` with different hash
- Root cause for repeated old markers was stale `kernel8.img` on SD
- Manual `cp` + hash verification fixed deployment mismatch

2. Marker progression after image mismatch fix:
- `{|}~1234567aby!`
  - panic inside `log::init` before `z`
- after switching to `set_logger_racy` / `set_max_level_racy`:
  - `{|}~1234567abyzZc!!!!!!!!!!!!!!!!!!!!!!!!`
  - logger init completed, but first formatted `info!` call caused panic recursion
- set Pi5 runtime log level to `Off` for bring-up:
  - `{|}~1234567abyzZcdenr!`
  - advanced to memory init window
- deeper mm/phys markers:
  - `{|}~...rRA!`
  - then `{|}~...rRAJKL!`
  - then `{|}~...rRAJKLTV!`
  - narrowed failure to reserved-region lock path
- replaced early-lock/oncecell in phys init path with single-core static approach:
  - advanced through `...TVWX...NOP...`
- then marker stalled at `...H` (pre/post MMU boundary)
  - identified first-1GB mapping issue for Pi5 kernel load at `0x0008_0000`
  - fixed rpi5 first-1GB mapping to normal executable RAM (while keeping virt behavior)
- finally reached:
  - `{|}~...HIsuvwxopqfghijklm8!` and then
  - `{|}~...89ABnF` (no panic, idle by design when no block device present)

3. Root causes identified and addressed:
- stale SD image deployment
- early atomic logger/oncecell paths before MMU stability
- panic recursion via formatted logging
- reserved-region lock path in early memory setup
- incorrect first-1GB device/XN mapping for Pi5 boot address
- missing post-init storage path now surfaced as next real blocker

4. Serial-data quality observations:
- Full `uart.clean.log` may still contain random binary/noise fragments due bootloader/probe traffic.
- Marker extraction remains reliable when parsed with:
  - `grep -ao '{|}~[0-9A-Za-z!]*' uart.clean.log | tail -n 1`
- Debug decisions in this session were made from extracted marker strings, not from raw noisy lines.

## 17. Additional Code Changes Since First Draft

Additional key changes made after initial `DEBUG.md` creation:

- `kernel/src/log.rs`
  - Pi5-specific early logger setup switched to `set_logger_racy` and `set_max_level_racy`
  - runtime log level temporarily forced `Off` for early bring-up stability
  - added `y/z/Z` markers

- `kernel/src/mem/phys.rs`
  - replaced early `OnceCell<Mutex<...>>` style init path with static single-core early-boot path
  - removed early mutex lock from reserved-region registration path
  - added `V/W/X` markers

- `kernel/src/arch/aarch64/mm.rs`
  - added fine-grained `A..I`, `J` markers
  - added Pi5 high MMIO block mappings for post-MMU debug UART visibility
  - fixed Pi5 first-1GB mapping to Normal WB (executable), while keeping `virt` behavior separate

- `kernel/src/main.rs`
  - added post-init markers (`9`, `A`, `B`, `C`, `D`, `E`, `F`, `n`, `e..i`)
  - converted hard `expect/unwrap` points in AArch64 rootfs/init path to non-panicking fallbacks
  - on missing block device, enter idle instead of panicking

- `.gitignore`
  - added `kernel/crates/kernel_bpf/target/`

## 18. Commit History for This Bring-up Work

Commits made during this debugging effort:
- `6cd0549` - `rpi5 bring-up: stabilize early boot and add UART marker tracing`
- `3db2af2` - `gitignore: exclude kernel_bpf build artifacts`

## 19. Practical Next Steps for New Engineer

1. Implement/enable a real Pi5 block device provider (SD/NVMe/USB) that registers `BlockDevices::by_id(0)`.
2. Once storage works, verify root mount and `/bin/init` creation path.
3. Gradually re-enable runtime logging on Pi5 (start with minimal, non-recursive paths).
4. Remove temporary marker instrumentation after stable textual logs are confirmed.

## 20. Handoff Delta (Most Recent Session End)

1. User asked to commit current bring-up state first:
- current relevant commits are recorded in Section 18.

2. User flagged untracked build artifacts:
- `kernel/crates/kernel_bpf/target/` was confirmed as build output and added to `.gitignore`.

3. Final confidence state before handoff:
- deployment mismatch issue is resolved (local and SD image hashes now intentionally checked each cycle).
- marker stream reaches post-init idle path (`...nF`) without panic.
- active blocker is functional storage registration, not early boot stability.

## 21. March 11 Continuation (Authoritative Current State)

This section supersedes the older `...nF` stopping point for current bring-up status.

### 21.1 What changed after the `...nF` phase

1. Storage/rootfs path was wired, and marker progress moved beyond idle into userspace handoff.
2. Marker stream advanced through:
   - `...SsjZ01TUu`
   - then `...SsjZ01TUuqrst0QX`
3. Syscall markers (`w` for first `SYS_WRITE`, `p` for first `SYS_BPF`) were added in `kernel/src/syscall/mod.rs`.
4. Additional sync-exception markers were added:
   - `V` (SVC), `I` (instruction abort), `D` (data abort), `Yxx` (unhandled EC), `Nks` (invalid vector path).
5. Before-TTBR0 and before-ERET instrumentation was added:
   - in process trampoline: `...t0Q`
   - in userspace entry: `X` immediately before `eret`.

### 21.2 Latest repeated marker (current baseline)

Observed repeatedly on fresh captures:

- `{|}~1234567abyzZcdenrRAJKLTVWXUMVWXNOPBCDEFGHIsuvwxopqfghijklRm89ABCDESsjZ01TUuqrst0QX`

Interpretation:
- Kernel init, scheduler, PID1 selection, task trampoline, process trampoline, TTBR0 switch, and pre-ERET all execute.
- Failure/stop occurs at or immediately after EL0 entry (`eret`) before first observed userspace syscall marker (`w`).
- No post-`X` `V/I/D/Y/N/w/p` has been consistently seen in this path yet.

### 21.3 Important exception during investigation

At one point the TTBR0 switch was temporarily skipped for Pi5 and marker reached:
- `...0QXD!`

That confirmed exception/panic visibility can return when page-table state changes, and informed the current working hypothesis:
- the EL0 entry/return boundary is still fragile, and vector/exception routing needs tighter tracing exactly at lower-EL sync entry.

### 21.4 Deployment/hash lessons (critical)

A recurring blocker was stale SD payload. Multiple times:
- local `kernel8.img` hash did not match `/mnt/rpi5-boot/kernel8.img`
- marker output remained old until manual `cp` + `sync` + hash verification.

Current mandatory rule:
1. `sha256sum target/.../kernel8.img`
2. copy to SD
3. `sync`
4. `sha256sum /mnt/rpi5-boot/kernel8.img`
5. only then boot

### 21.5 UART capture lessons (critical)

Corrupted captures were repeatedly caused by:
- multiple concurrent readers (`screen` + `cat`)
- stale/append logs
- ownership mismatch (`sudo tee` root-owned file)

Current canonical one-reader flow:

```bash
PORT=/dev/serial/by-id/usb-Raspberry_Pi_Debug_Probe__CMSIS-DAP__E6633861A355B838-if01
: > uart.clean.log
sudo timeout 70s cat "$PORT" | tr -d '\r' | tee uart.clean.log >/dev/null || true
grep -ao '{|}~[0-9A-Za-z!]*' uart.clean.log | tail -n 1
```

### 21.6 Current code-side diagnostic plan (next run)

To isolate whether the first EL0 `svc`/abort reaches the lower-EL sync vector at all, assembly markers were added in:
- `kernel/src/arch/aarch64/exception_vectors.S`

Lower-EL sync path markers now emit:
- `6` after `save_context`
- `7` before `handle_sync_exception`
- `8` after `handle_sync_exception`
- `9` after `check_preemption`
- `A` before `eret` back out of the exception path

How to read with current suffix:
- If marker stays at `...0QX` with no `6789A`, sync exceptions are not reaching this path (or fail before/inside `save_context`).
- If `...0QX67...` appears, vector entry is alive and failure is deeper in Rust sync handling or return path.
- If `...0QX6789A` appears repeatedly, syscalls are occurring and we should then see `w`/`p`.

## 22. Current Reality Snapshot (March 11, 2026)

- Pi5 now reaches post-init userspace handoff markers.
- Rootfs/bin-init path is active up to EL0 transition preparation.
- Remaining gap is EL0 execution + first syscall visibility after `eret`.
- Benchmark publication (M4) is blocked until `w/p` and benchmark banner are stable in repeated runs.

## 23. Current Files Most Relevant To Continue

- `kernel/src/mcore/mtask/process/mod.rs` (trampoline, TTBR0, `qrst0Q`)
- `kernel/src/arch/aarch64/context.rs` (`X` before `eret`)
- `kernel/src/arch/aarch64/exception_vectors.S` (new `6789A` assembly markers)
- `kernel/src/arch/aarch64/exceptions.rs` (`V/I/D/Yxx/Nks`)
- `kernel/src/syscall/mod.rs` (`w`, `p`)
- `userspace/init/src/main.rs` (`/bin/benchmark` spawn)
- `userspace/benchmark/src/main.rs` (expected UART benchmark banner)

## 24. Definition Of Progress For The Next Session

The next session should produce at least one of:

1. `...0QX6` (proves lower-EL sync vector entry after EL0 transition), or
2. `...0QXw` / benchmark text (proves userspace syscall path is alive), or
3. `...0QXV/I/D/Y..` (proves exception class is now observable post-ERET).

If none of these appear and sequence remains hard-stuck at `...0QX`, prioritize:
- vector-entry pre-stack instrumentation and/or EL0 entry-state verification (SPSR/ELR/SP + VBAR assumptions).

## 25. Build Artifact Note

`disk.img` appears as an untracked artifact in this workspace and should not be committed.

## 26. Late Session Update (March 11, 2026, evening)

This section captures the most recent progression after the previous `...0QX`/`Y00` baseline.

### 26.1 Marker outcomes observed in order

1. A regression build produced a tight sync loop before `s`:
   - `...TUuqrjkjkjkjk...`
   - Interpretation: EL1 synchronous exceptions are firing repeatedly during trampoline read/load stage, before ELF-load-complete marker `s`.

2. Reverted back to pre-regression trampoline path (restored `process/mod.rs` toward known stable behavior).

3. Stable baseline was re-confirmed:
   - `{|}~1234567abyzZcdenrRAJKLTVWXUMVWXNOPBCDEFGHIsuvwxopqfghijklRm89ABCDESsjZ01TUuqrst0QX67hjkY00E00211464R02000000P00000000F00000000IFFFFFFFFJFFAFFFAFS00000001001A6000!`

4. Decoding of the stable packet (important):
   - `Y00` = unknown synchronous exception class (EC=0x00)
   - `E00211464` = ELR low 32 bits `0x00211464` (matches `/bin/init` entry)
   - `R02000000` = ESR low 32 bits `0x02000000`
   - `P00000000` = SPSR low 32 bits `0x00000000`
   - `F00000000` = FAR low 32 bits `0x00000000`
   - `IFFFFFFFF` and `JFFAFFFAF` = instruction words fetched via telemetry path at `ELR` and `ELR+4`
   - `S00000001001A6000` = `SP_EL0` snapshot

Conclusion remains: kernel reaches EL0 handoff path, but first userspace instruction stream at entry still appears invalid/corrupted in runtime context.

### 26.2 Latest code change prepared after re-confirming baseline

File changed:
- `kernel/src/mcore/mtask/process/mod.rs`

Intent:
- keep ELF file bytes in a kernel-owned `Vec<u8>` (avoid relying on user allocation as parse source)
- run `ElfLoader::load(...)` under active process TTBR0 on AArch64, with IRQ-masked wrapper:
  - helper `with_process_address_space_active(...)`
- avoid inserting `executable_file_allocation` into `executable_file_data` in this path (startup simplification for this experiment)

### 26.3 Build result for current unvalidated attempt

Built successfully via:
- `./scripts/build-rpi5.sh release`

Produced image:
- `target/aarch64-unknown-none/release/kernel8.img`
- size: `10860216`
- sha256: `46ca8fcee7b85b0740205b343c6cd6b6487c19a35ba6ffa044fb0daa1b1cf8eb`

Status of this specific image at the moment of writing:
- build is complete
- needs hardware flash + UART verification
- expected success signal: progression beyond `...Y00...` into syscall/user markers (`w`, `p`) or benchmark text.

### 26.4 Immediate next hardware check command set

```bash
sudo mount /dev/sda1 /mnt/rpi5-boot
sudo cp -v target/aarch64-unknown-none/release/kernel8.img /mnt/rpi5-boot/kernel8.img
sync
sha256sum /mnt/rpi5-boot/kernel8.img
sudo umount /mnt/rpi5-boot

: > uart.clean.log
sudo timeout 70s cat "$PORT" | tr -d '\r' | tee uart.clean.log >/dev/null || true
grep -ao '{|}~[0-9A-Za-z!]*' uart.clean.log | tail -n 1
```

