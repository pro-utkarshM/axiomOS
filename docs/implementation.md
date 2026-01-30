# Axiom Implementation Roadmap

## Current Priority: BPF Integration Hardening

The kernel boots. The BPF subsystem is complete as a library. **Basic integration exists** - BpfManager and bpf() syscall are implemented. The focus now is **hardening, attach point wiring, and testing**.

```
CURRENT STATE (2026-01-27)
──────────────────────────
Kernel runs → BPF manager initialized → programs loadable via syscall
           → BPF maps demo works → basic program execution

REMAINING WORK
──────────────
→ Fix hardcoded map sizes (security issue)
→ Add pointer validation (security issue)
→ Wire attach points to actual kernel events
→ Implement helpers for kernel interaction
→ Add comprehensive tests
```

---

## Phase 3: BPF Integration

### Step 1: BPF Manager ✅ DONE

**Location:** `kernel/src/bpf/mod.rs`

**Goal:** Kernel component that manages BPF programs.

```rust
// kernel/src/bpf/mod.rs

use kernel_bpf::{
    verifier::Verifier,
    execution::Interpreter,
    maps::{ArrayMap, HashMap, RingBufMap},
    loader::ElfLoader,
};

pub struct BpfManager {
    /// Loaded programs (verified, ready to execute)
    programs: Vec<LoadedProgram>,

    /// Active maps
    maps: Vec<Box<dyn BpfMap>>,

    /// Attached programs (program_id → attach_point)
    attachments: Vec<Attachment>,
}

pub struct LoadedProgram {
    pub id: u32,
    pub name: String,
    pub instructions: Vec<BpfInsn>,
    pub verified: bool,
}

pub struct Attachment {
    pub program_id: u32,
    pub attach_type: AttachType,
    pub enabled: bool,
}

impl BpfManager {
    pub fn new() -> Self { ... }

    /// Load program from ELF bytes, verify, store
    pub fn load_program(&mut self, elf_bytes: &[u8]) -> Result<u32, BpfError> {
        let program = ElfLoader::load(elf_bytes)?;
        Verifier::verify(&program)?;
        let id = self.programs.len() as u32;
        self.programs.push(LoadedProgram {
            id,
            name: program.name,
            instructions: program.instructions,
            verified: true,
        });
        Ok(id)
    }

    /// Attach program to event source
    pub fn attach(&mut self, program_id: u32, attach_type: AttachType) -> Result<(), BpfError> {
        // Validate program exists
        // Register with appropriate subsystem
        // Store attachment
    }

    /// Execute program with context
    pub fn execute(&self, program_id: u32, ctx: &BpfContext) -> Result<u64, BpfError> {
        let program = self.programs.get(program_id as usize)?;
        Interpreter::run(&program.instructions, ctx)
    }
}
```

**Integration point:** Initialize in `kernel/src/lib.rs`:

```rust
// In kernel::init()
pub fn init(boot_info: &BootInfo) {
    // ... existing init ...

    // Initialize BPF subsystem
    log::info!("Initializing BPF subsystem");
    let bpf_manager = BpfManager::new();
    BPF_MANAGER.call_once(|| Mutex::new(bpf_manager));

    // ... continue to load init ...
}

static BPF_MANAGER: OnceCell<Mutex<BpfManager>> = OnceCell::new();
```

---

### Step 2: bpf() Syscall ✅ DONE (needs hardening)

**Location:** `kernel/crates/kernel_abi/src/syscall.rs` - SYS_BPF defined

**Location:** `kernel/src/syscall/bpf.rs` - Handler implemented

**⚠️ Known Issues (from codebase analysis):**

1. **Hardcoded BPF Map Sizes** (lines 67, 101-103)
   - All maps assume 4-byte keys and 8-byte values
   - Fix: Extract key/value sizes from BpfAttr structure

2. **Unsafe Pointer Casts** (lines 22, 54, 88, 123, 150, 177)
   - User pointers cast directly to kernel structures
   - Security risk: Could read/write arbitrary kernel memory
   - Fix: Add address space, alignment, and bounds validation

3. **Basic Null Check Only**
   - Current validation: `if attr_ptr == 0`
   - Needed: Full address space verification

```rust
// BPF syscall commands
pub const BPF_PROG_LOAD: u32 = 0;
pub const BPF_MAP_CREATE: u32 = 1;
pub const BPF_PROG_ATTACH: u32 = 2;
pub const BPF_PROG_DETACH: u32 = 3;
pub const BPF_MAP_LOOKUP_ELEM: u32 = 4;
pub const BPF_MAP_UPDATE_ELEM: u32 = 5;
pub const BPF_MAP_DELETE_ELEM: u32 = 6;

#[repr(C)]
pub struct BpfAttrProgLoad {
    pub prog_type: u32,
    pub insn_cnt: u32,
    pub insns: *const u8,
    pub license: *const u8,
}

#[repr(C)]
pub struct BpfAttrMapCreate {
    pub map_type: u32,
    pub key_size: u32,
    pub value_size: u32,
    pub max_entries: u32,
}

pub fn sys_bpf(cmd: u32, attr: *const u8, size: usize) -> Result<i64, Errno> {
    let bpf_manager = BPF_MANAGER.get().ok_or(Errno::ENODEV)?;
    let mut manager = bpf_manager.lock();

    match cmd {
        BPF_PROG_LOAD => {
            // Copy attr from userspace
            // Load and verify program
            // Return program ID
        }
        BPF_MAP_CREATE => {
            // Create map
            // Return map ID
        }
        BPF_PROG_ATTACH => {
            // Attach program to event source
        }
        _ => Err(Errno::EINVAL),
    }
}
```

**Wire into syscall dispatch:** `kernel/src/syscall/mod.rs`

```rust
match syscall_num {
    // ... existing syscalls ...
    SYS_BPF => sys_bpf(arg0 as u32, arg1 as *const u8, arg2),
    _ => Err(Errno::ENOSYS),
}
```

---

### Step 3: Timer Attach Point ❌ NOT WIRED

**Goal:** Execute BPF program on every timer tick.

**Status:** Timer attach abstraction exists in `kernel_bpf/src/attach/`, but not connected to actual timer interrupts.

**Location:** Modify existing timer interrupt handler.

For x86_64 (`kernel/src/arch/x86_64.rs` or timer code):

```rust
// In timer interrupt handler
fn timer_interrupt_handler() {
    // Existing tick handling...

    // Execute attached BPF programs
    if let Some(bpf_manager) = BPF_MANAGER.get() {
        let manager = bpf_manager.lock();
        for attachment in manager.get_timer_attachments() {
            let ctx = BpfContext {
                timestamp: get_kernel_time_ns(),
                // ... other context
            };
            let _ = manager.execute(attachment.program_id, &ctx);
        }
    }
}
```

For AArch64 (similar pattern in ARM timer handler).

---

### Step 4: Syscall Tracing Attach Point ❌ NOT WIRED

**Goal:** Execute BPF program on syscall entry/exit.

**Status:** Tracepoint attach abstraction exists in `kernel_bpf/src/attach/`, but not connected to syscall dispatcher.

**Location:** `kernel/src/syscall/mod.rs`

```rust
fn handle_syscall(num: usize, args: [usize; 6]) -> Result<i64, Errno> {
    // BPF: syscall entry
    run_bpf_syscall_enter(num, &args);

    // Dispatch to actual handler
    let result = match num {
        SYS_EXIT => sys_exit(args[0] as i32),
        // ... etc
    };

    // BPF: syscall exit
    run_bpf_syscall_exit(num, &result);

    result
}

fn run_bpf_syscall_enter(syscall_num: usize, args: &[usize; 6]) {
    if let Some(bpf_manager) = BPF_MANAGER.get() {
        let manager = bpf_manager.lock();
        for attachment in manager.get_syscall_enter_attachments() {
            let ctx = SyscallContext {
                syscall_num: syscall_num as u64,
                arg0: args[0] as u64,
                arg1: args[1] as u64,
                // ...
            };
            let _ = manager.execute(attachment.program_id, &ctx.as_bpf_context());
        }
    }
}
```

---

### Step 5: Helper Implementation ❌ NOT DONE

**Location:** `kernel/src/bpf/helpers.rs` (needs creation)

```rust
/// Get current kernel time in nanoseconds
pub fn bpf_ktime_get_ns() -> u64 {
    // Read from HPET or ARM timer
    crate::time::get_kernel_time_ns()
}

/// Look up element in map
pub fn bpf_map_lookup_elem(map_id: u32, key: *const u8) -> *const u8 {
    if let Some(bpf_manager) = BPF_MANAGER.get() {
        let manager = bpf_manager.lock();
        if let Some(map) = manager.maps.get(map_id as usize) {
            return map.lookup(key);
        }
    }
    core::ptr::null()
}

/// Output to ring buffer
pub fn bpf_ringbuf_output(map_id: u32, data: *const u8, size: u32) -> i32 {
    if let Some(bpf_manager) = BPF_MANAGER.get() {
        let manager = bpf_manager.lock();
        if let Some(map) = manager.maps.get(map_id as usize) {
            if let Some(ringbuf) = map.as_ringbuf() {
                return ringbuf.output(data, size);
            }
        }
    }
    -1
}

/// Print to serial console (debug)
pub fn bpf_trace_printk(fmt: *const u8, _fmt_size: u32) -> i32 {
    // Safety: validate fmt is in valid memory
    let s = unsafe { core::ffi::CStr::from_ptr(fmt as *const i8) };
    if let Ok(msg) = s.to_str() {
        log::info!("[BPF] {}", msg);
        return 0;
    }
    -1
}
```

**Wire into interpreter:** Modify `kernel_bpf/src/execution/interpreter.rs` to call these helpers:

```rust
fn dispatch_helper(helper_id: u32, args: [u64; 5]) -> u64 {
    match helper_id {
        1 => bpf_ktime_get_ns(),
        2 => bpf_map_lookup_elem(args[0] as u32, args[1] as *const u8) as u64,
        3 => bpf_map_update_elem(...) as u64,
        6 => bpf_ringbuf_output(args[0] as u32, args[1] as *const u8, args[2] as u32) as u64,
        7 => bpf_trace_printk(args[0] as *const u8, args[1] as u32) as u64,
        _ => 0,
    }
}
```

---

### Step 6: Userspace BPF Loader ❌ NOT DONE

**Location:** `userspace/minilib/src/lib.rs`

Add bpf() syscall wrapper (not yet implemented):

```rust
pub fn bpf(cmd: u32, attr: *const u8, size: usize) -> i64 {
    syscall3(SYS_BPF, cmd as usize, attr as usize, size)
}
```

**Location:** `userspace/bpf_loader/` (new, simple test program)

```rust
#![no_std]
#![no_main]

use minilib::{bpf, write, exit};

// Minimal BPF program bytecode (hardcoded for testing)
// This program just returns 0
static PROG: [u8; 16] = [
    0xb7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // mov r0, 0
    0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // exit
];

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let attr = BpfAttrProgLoad {
        prog_type: 0,
        insn_cnt: 2,
        insns: PROG.as_ptr(),
        license: b"GPL\0".as_ptr(),
    };

    let result = bpf(BPF_PROG_LOAD, &attr as *const _ as *const u8, core::mem::size_of_val(&attr));

    if result >= 0 {
        write(1, b"BPF program loaded!\n");
    } else {
        write(1, b"BPF load failed\n");
    }

    exit(0);
}
```

---

## Phase 4: Hardware Attach (RPi5)

### GPIO Driver

**Location:** `kernel/src/driver/gpio/` (new)

```rust
pub struct Rpi5Gpio {
    base: usize, // MMIO base address
}

impl Rpi5Gpio {
    pub fn new(dtb: &DeviceTree) -> Self {
        // Parse GPIO base from DTB
    }

    pub fn configure_input(&self, pin: u32) {
        // Set pin as input
    }

    pub fn configure_output(&self, pin: u32) {
        // Set pin as output
    }

    pub fn set(&self, pin: u32, value: bool) {
        // Write to pin
    }

    pub fn get(&self, pin: u32) -> bool {
        // Read pin
    }

    pub fn enable_edge_interrupt(&self, pin: u32, edge: Edge) {
        // Configure interrupt on edge
    }
}
```

### GPIO Interrupt Handler

```rust
fn gpio_interrupt_handler(pin: u32, edge: Edge) {
    // Execute attached BPF programs
    if let Some(bpf_manager) = BPF_MANAGER.get() {
        let manager = bpf_manager.lock();
        for attachment in manager.get_gpio_attachments(pin) {
            let ctx = GpioContext {
                pin,
                edge,
                timestamp: get_kernel_time_ns(),
            };
            let _ = manager.execute(attachment.program_id, &ctx.as_bpf_context());
        }
    }
}
```

### GPIO Helper

```rust
/// Set GPIO pin value from BPF program
pub fn bpf_gpio_set(chip: u32, line: u32, value: u32) -> i32 {
    if let Some(gpio) = get_gpio_driver() {
        gpio.set(line, value != 0);
        return 0;
    }
    -1
}
```

---

## Testing Strategy

### Unit Tests

```bash
# Test BPF library in isolation
cargo test -p kernel_bpf

# Test with embedded profile
cargo test -p kernel_bpf --features embedded-profile

# Test with cloud profile
cargo test -p kernel_bpf --features cloud-profile
```

### QEMU Integration

```bash
# Build and run kernel
cargo run --release

# Expected output:
# [kernel] Booting Axiom...
# [kernel] Physical memory initialized
# [kernel] Virtual memory initialized
# [kernel] BPF subsystem initialized    <- NEW
# [kernel] Loading /bin/init
# [init] hello from init!
```

### BPF Smoke Test

Once syscall is implemented:

```bash
# Build kernel with bpf_loader in filesystem
cargo run --release

# Expected:
# [init] BPF program loaded!
# or
# [BPF] Hello from BPF!  (if using trace_printk)
```

### RPi5 Hardware Test

```bash
# Build for AArch64
cargo build --target aarch64-unknown-none --release

# Create SD card image
./scripts/make_rpi5_image.sh

# Boot on RPi5
# Connect button to GPIO pin
# Connect LED to another GPIO pin

# Expected:
# Button press → BPF executes → LED toggles
```

---

## Milestones

### Milestone 1: BPF Runs in Kernel ✅ COMPLETE
- [x] BpfManager integrated into kernel
- [x] BPF maps demo executes during init
- [x] Output visible on serial console

### Milestone 2: Syscall Works ⚠️ PARTIAL
- [x] bpf() syscall implemented
- [ ] Security hardening (pointer validation, map sizes)
- [ ] Can load program from userspace (needs testing)
- [ ] Program executes successfully

### Milestone 3: Attach Points Work ⚠️ PARTIAL
- [ ] Timer attach point working
- [ ] Syscall tracing working
- [x] IIO attach point integrated (Simulated)
- [ ] BPF runs on events

### Milestone 4: RPi5 Demo ❌ NOT STARTED
- [x] Kernel boots on RPi5 (AArch64 support complete)
- [ ] GPIO driver working
- [ ] Button → BPF → LED demo

### Milestone 5: Full Demo ❌ NOT STARTED
- [ ] Multiple example programs
- [ ] Safety interlock demo
- [ ] Performance benchmarks
- [ ] Video demo for proposal

---

## File Changes Summary

### Existing Files (from codebase analysis)
```
kernel/src/bpf/
└── mod.rs              # ✅ BpfManager EXISTS

kernel/src/syscall/bpf.rs    # ✅ bpf() syscall handler EXISTS (needs hardening)

kernel/demos/               # ✅ BPF maps demo EXISTS
```

### Files to Create
```
kernel/src/bpf/
├── helpers.rs          # Helper implementations
└── context.rs          # BPF context types

kernel/src/driver/gpio/
├── mod.rs              # GPIO abstraction
└── rpi5.rs             # RPi5 GPIO driver

userspace/bpf_loader/        # Test program
```

### Files to Modify
```
kernel/src/syscall/bpf.rs    # Fix hardcoded map sizes, add pointer validation
kernel/src/lib.rs            # Verify BPF init sequence
kernel/src/syscall/mod.rs    # Verify SYS_BPF dispatch
kernel/crates/kernel_bpf/src/execution/interpreter.rs  # Wire real helpers
```

---

## Dependencies

The kernel_bpf crate needs to be usable from kernel context:

```toml
# kernel/Cargo.toml
[dependencies]
kernel_bpf = { path = "crates/kernel_bpf", default-features = false, features = ["embedded-profile"] }
```

Ensure kernel_bpf is no_std compatible (it should already be).

---

## Technical Debt Summary

*From codebase analysis (2026-01-27). See `.planning/codebase/CONCERNS.md` for details.*

### Critical (Security)

| Issue | Location | Impact |
|-------|----------|--------|
| Hardcoded map sizes | `kernel/src/syscall/bpf.rs:67,101-103` | Buffer overflow risk |
| Unsafe pointer casts | `kernel/src/syscall/bpf.rs:22,54,88,123,150,177` | Arbitrary kernel memory access |
| Basic null check only | `kernel/src/syscall/bpf.rs` | Insufficient validation |

### High Priority

| Issue | Location | Impact |
|-------|----------|--------|
| Missing SAFETY comments | 70+ files in `kernel/src/` | Audit difficulty |
| No BPF syscall tests | `kernel/src/syscall/bpf.rs` | Unverified behavior |
| ARM64 JIT stack hardcoded | `kernel_bpf/src/execution/jit_aarch64.rs:634` | Stack overflow risk |

### Medium Priority

| Issue | Location | Impact |
|-------|----------|--------|
| Edition 2024 in Cargo.toml | Root and kernel Cargo.toml | Build failure on standard toolchains |
| VFS node reuse missing | `kernel_vfs/src/vfs/mod.rs:89` | Performance degradation |
| BTF parsing not implemented | `kernel_bpf/src/loader/mod.rs:152` | No CO-RE support |

### Platform Gaps

| Platform | Status | Missing |
|----------|--------|---------|
| x86_64 | ✅ Complete | - |
| AArch64/RPi5 | ⚠️ Partial | Demand paging |
| RISC-V | ❌ Incomplete | PLIC, paging, most functionality |

### Dependencies at Risk

| Dependency | Version | Risk |
|------------|---------|------|
| zerocopy | 0.9.0-alpha.0 | Alpha - API may change |
| sha3 | 0.11.0-rc.3 | RC - may have bugs |
| mkfs-ext2 | Git | Unversioned - may break |
| mkfs-filesystem | Git | Unversioned - may break |

---

## Next Steps (Recommended Order)

1. **Security Hardening** - Fix pointer validation and map sizes in `bpf.rs`
2. **Add Tests** - Unit tests for BPF syscall handler
3. **Wire Timer Attach** - Connect timer interrupt to BPF execution
4. **Implement Helpers** - `bpf_ktime_get_ns()`, `bpf_trace_printk()`
5. **Userspace Loader** - Create `userspace/bpf_loader/` test program
6. **End-to-End Demo** - Load program from userspace, see output
