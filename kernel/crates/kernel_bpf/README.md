# kernel_bpf - Single-Source eBPF Kernel with Build-Time Physical Profiles

A profile-constrained eBPF subsystem for Muffin OS where the same source code serves both cloud and embedded deployments. The difference between profiles is expressed through build-time selection and compile-time erasure, not code forks.

## Guiding Principle

> "Cloud and embedded share one source code; the difference is not features, but the physical assumptions the kernel is allowed to make at build time."

## Features

- **Two Physical Profiles**: CLOUD and EMBEDDED, mutually exclusive at compile time
- **Zero Semantic Drift**: Same bytecode produces identical results on both profiles
- **Compile-Time Erasure**: Profile-inappropriate code is physically absent from builds
- **Full eBPF Stack**: Bytecode, verifier, interpreter, JIT (cloud), maps, scheduler

## Quick Start

### Building

Select exactly one profile at build time:

```bash
# Cloud profile - elastic resources, JIT compilation, soft latency bounds
cargo build --features cloud-profile

# Embedded profile - static resources, interpreter/AOT only, hard deadlines
cargo build --features embedded-profile
```

### Running Tests

```bash
# Test cloud profile
cargo test --features cloud-profile

# Test embedded profile
cargo test --features embedded-profile
```

## Profile Comparison

| Property | Cloud | Embedded |
|----------|-------|----------|
| Memory Strategy | Elastic (heap) | Static (pool) |
| Stack Size | 512 KB | 8 KB |
| Instruction Limit | 1,000,000 | 100,000 |
| JIT Compilation | Yes (default) | No (erased) |
| Interpreter | Fallback | Primary |
| AOT Compilation | Optional | Encouraged |
| WCET Enforcement | No | Yes |
| Deadline Scheduling | No | Yes (EDF) |
| Map Resize | Yes | No (erased) |
| Restart Acceptable | Yes | No |

## Architecture

```
kernel_bpf/
├── bytecode/       # eBPF instruction set and program representation
│   ├── registers   # R0-R10 register file
│   ├── opcode      # Opcode classes, ALU ops, jump conditions
│   ├── insn        # BpfInsn struct (8 bytes per instruction)
│   └── program     # BpfProgram<P> with profile-bounded limits
│
├── verifier/       # Safety verification with profile constraints
│   ├── state       # Register and stack state tracking
│   ├── cfg         # Control flow graph construction
│   └── core        # Core verifier with profile hooks
│
├── execution/      # Program execution engines
│   ├── interpreter # Bytecode interpreter (both profiles)
│   └── jit/        # JIT compiler (cloud-only, x86_64)
│
├── maps/           # BPF map implementations
│   ├── array       # Array map with profile-aware storage
│   └── static_pool # Static memory pool (embedded-only)
│
├── scheduler/      # Profile-aware program scheduling
│   ├── queue       # Ready queue implementation
│   ├── throughput  # Fairness-first policy (cloud-only)
│   └── deadline    # EDF + priority ceiling (embedded-only)
│
└── profile/        # Physical profile definitions
    ├── memory      # MemoryStrategy trait
    ├── scheduler   # SchedulerPolicy trait
    └── failure     # FailureSemantic trait
```

## Usage Examples

### Creating and Running a BPF Program

```rust
use kernel_bpf::bytecode::insn::BpfInsn;
use kernel_bpf::bytecode::program::{BpfProgType, ProgramBuilder};
use kernel_bpf::execution::{BpfContext, BpfExecutor, Interpreter};
use kernel_bpf::profile::ActiveProfile;

// Build a simple program that returns 42
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 42))  // r0 = 42
    .insn(BpfInsn::exit())             // return r0
    .build()
    .expect("valid program");

// Execute with interpreter
let interp = Interpreter::<ActiveProfile>::new();
let result = interp.execute(&program, &BpfContext::empty());
assert_eq!(result, Ok(42));
```

### Using BPF Maps

```rust
use kernel_bpf::maps::{ArrayMap, BpfMap};
use kernel_bpf::profile::ActiveProfile;

// Create an array map with 4-byte values and 100 entries
let map = ArrayMap::<ActiveProfile>::with_entries(4, 100)
    .expect("create map");

// Write to index 5
let key = 5u32.to_ne_bytes();
let value = 42u32.to_ne_bytes();
map.update(&key, &value, 0).expect("update");

// Read back
let result = map.lookup(&key).expect("lookup");
assert_eq!(result, value);
```

### Scheduling BPF Programs

```rust
use kernel_bpf::scheduler::{BpfScheduler, BpfExecRequest, ExecPriority, ProgId};
use kernel_bpf::execution::BpfContext;
use std::sync::Arc;

let mut scheduler = BpfScheduler::new();

// Submit a program for execution
let request = BpfExecRequest::new(
    ProgId(1),
    Arc::new(program),
    BpfContext::empty(),
).with_priority(ExecPriority::High);

scheduler.submit(request).expect("submit");

// Get next program to execute
if let Some(queued) = scheduler.next() {
    // Execute the program...
}
```

### Embedded Profile: Deadline Scheduling

```rust
#[cfg(feature = "embedded-profile")]
{
    use kernel_bpf::scheduler::{Deadline, BpfExecRequest};

    // Create request with a deadline
    let deadline = Deadline::from_now(current_time_ns, 1_000_000); // 1ms deadline
    let request = BpfExecRequest::new(ProgId(1), program, context)
        .with_deadline(deadline);

    scheduler.submit(request).expect("submit");
}
```

## Compile-Time Erasure

The following features are completely absent from their non-applicable profiles:

### Cloud-Only (Erased from Embedded)
- `JitExecutor` and JIT compilation
- `ThroughputPolicy` scheduler
- `BpfMap::resize()` method
- LRU hash maps, LPM trie maps

### Embedded-Only (Erased from Cloud)
- `StaticPool` memory allocator
- `DeadlinePolicy` scheduler with EDF
- `Deadline` and `EnergyBudget` types
- WCET verification errors

## Verifier

The verifier ensures BPF programs are safe before execution:

```rust
use kernel_bpf::verifier::Verifier;

let verifier = Verifier::<ActiveProfile>::new();
match verifier.verify(&program) {
    Ok(()) => println!("Program is safe"),
    Err(e) => println!("Verification failed: {}", e),
}
```

### Verification Checks

**Both Profiles:**
- Invalid opcodes
- Uninitialized register access
- Out-of-bounds memory access
- Division by zero (compile-time detection)
- Infinite loops (bounded iteration)
- Stack overflow

**Embedded Profile Only:**
- WCET budget exceeded
- Interrupt-unsafe operations
- Dynamic allocation attempts

## Testing

### Profile Contract Tests

Verify that each profile maintains its documented constraints:

```bash
cargo test --features cloud-profile --test profile_contracts
cargo test --features embedded-profile --test profile_contracts
```

### Semantic Consistency Tests

Verify that identical bytecode produces identical results:

```bash
cargo test --features cloud-profile --test semantic_consistency
cargo test --features embedded-profile --test semantic_consistency
```

### Mutual Exclusion

Verify that both profiles cannot be enabled together:

```bash
# This should fail to compile
cargo build --features cloud-profile,embedded-profile
```

## CI/CD

The GitHub Actions workflow (`.github/workflows/bpf-profiles.yml`) runs:

1. **cloud-profile**: Build + test + clippy
2. **embedded-profile**: Build + test + clippy
3. **mutual-exclusion**: Verify compile-time constraints
4. **semantic-consistency**: Run same tests under both profiles
5. **format-check**: Verify code formatting

## Design Decisions

### Why Build-Time Profiles?

Runtime profile switching would require:
- Larger binary size (both code paths present)
- Runtime overhead for profile checks
- Risk of accidentally using wrong profile code

Build-time selection ensures:
- Minimal binary size (only relevant code)
- Zero runtime overhead
- Compile-time guarantee of correct profile usage

### Why Sealed Traits?

The `PhysicalProfile` trait is sealed to prevent external implementations. This ensures:
- Only `CloudProfile` and `EmbeddedProfile` exist
- Profile contracts cannot be violated by third-party code
- Compiler can optimize based on known implementations

### Why PhantomData<fn() -> P>?

Using `PhantomData<fn() -> P>` instead of `PhantomData<P>` ensures the profile marker is:
- `Send` and `Sync` regardless of `P`
- Zero-sized (no runtime cost)
- Covariant over `P`

## License

This crate is part of Muffin OS and follows its licensing terms.
