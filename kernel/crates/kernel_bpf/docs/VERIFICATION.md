# BPF Verifier Guide

This document explains how the BPF verifier ensures program safety.

## Overview

The verifier performs static analysis on BPF programs before execution to guarantee:
- No out-of-bounds memory access
- No use of uninitialized data
- No infinite loops (bounded iteration)
- No division by zero
- No stack overflow
- Profile-specific constraints are met

## Verification Pipeline

```
┌────────────┐    ┌─────────────┐    ┌──────────────┐    ┌────────────┐
│   Parse    │───▶│  Build CFG  │───▶│   Analyze    │───▶│  Verified  │
│  Program   │    │             │    │   States     │    │  Program   │
└────────────┘    └─────────────┘    └──────────────┘    └────────────┘
      │                  │                  │
      ▼                  ▼                  ▼
   Invalid           Unreachable        Safety
   Opcodes           Code Found         Violations
```

## Using the Verifier

### Basic Usage

```rust
use kernel_bpf::verifier::Verifier;
use kernel_bpf::profile::ActiveProfile;

let verifier = Verifier::<ActiveProfile>::new();

match verifier.verify(&program) {
    Ok(()) => println!("Program verified successfully"),
    Err(e) => println!("Verification failed: {}", e),
}
```

### With Options

```rust
use kernel_bpf::verifier::{Verifier, VerifyOptions};

let options = VerifyOptions {
    max_iterations: 100_000,      // Max verification iterations
    allow_loops: true,            // Allow bounded loops
    strict_alignment: true,       // Enforce aligned access
};

let verifier = Verifier::<ActiveProfile>::with_options(options);
verifier.verify(&program)?;
```

## Verification Checks

### 1. Opcode Validation

Every instruction must have a valid opcode:

```rust
// Valid
BpfInsn::mov64_imm(0, 42)  // 0xb7 - known opcode

// Invalid - will fail verification
BpfInsn::new(0xFF, 0, 0, 0, 0)  // 0xFF - invalid opcode
```

**Error:** `VerifyError::InvalidOpcode { pc: usize, opcode: u8 }`

### 2. Register Initialization

Registers must be initialized before use:

```rust
// BAD: R1 is used but never initialized
let bad_program = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_reg(0, 1))  // R0 = R1, but R1 is undefined!
    .insn(BpfInsn::exit())
    .build()?;

// GOOD: R1 is initialized first
let good_program = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(1, 42)) // R1 = 42
    .insn(BpfInsn::mov64_reg(0, 1))  // R0 = R1
    .insn(BpfInsn::exit())
    .build()?;
```

**Error:** `VerifyError::UninitializedRegister { pc: usize, reg: u8 }`

### 3. Frame Pointer Protection

R10 (frame pointer) is read-only:

```rust
// BAD: Writing to R10
let bad = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(10, 0))  // R10 = 0, FORBIDDEN!
    .insn(BpfInsn::exit())
    .build()?;
```

**Error:** `VerifyError::WriteToR10 { pc: usize }`

### 4. Division by Zero

Division/modulo by zero is detected:

```rust
// BAD: Division by zero
let bad = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 100))
    .insn(BpfInsn::div64_imm(0, 0))  // R0 /= 0, FORBIDDEN!
    .insn(BpfInsn::exit())
    .build()?;
```

**Error:** `VerifyError::DivisionByZero { pc: usize }`

### 5. Exit Requirement

Every program must have at least one reachable exit:

```rust
// BAD: No exit instruction
let bad = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 42))
    // Missing exit!
    .build()?;

// BAD: Infinite loop, no reachable exit
let bad = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::ja(0))  // Jump to self forever
    .insn(BpfInsn::exit()) // Never reached
    .build()?;
```

**Error:** `VerifyError::NoExit`

### 6. Bounded Iteration

Loops must be provably bounded:

```rust
// GOOD: Loop with clear termination
let good = ProgramBuilder::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 0))      // counter = 0
    .insn(BpfInsn::mov64_imm(1, 10))     // limit = 10
    .insn(BpfInsn::jeq_reg(0, 1, 2))     // if counter == limit, exit
    .insn(BpfInsn::add64_imm(0, 1))      // counter++
    .insn(BpfInsn::ja(-3))               // goto loop
    .insn(BpfInsn::exit())
    .build()?;
```

The verifier tracks loop iterations and fails if the bound cannot be determined.

**Error:** `VerifyError::UnboundedLoop { pc: usize }`

### 7. Stack Bounds

Stack access must be within bounds:

```rust
// Stack grows downward from R10
// Valid range: [R10 - MAX_STACK_SIZE, R10)

// GOOD: Valid stack access
let offset = -8;  // 8 bytes below frame pointer
// store to stack: *(R10 + offset) = value

// BAD: Stack overflow
let offset = -600_000;  // Way below stack limit
// Error: StackOutOfBounds
```

**Error:** `VerifyError::StackOutOfBounds { pc: usize, offset: i32 }`

### 8. Memory Access

Pointer arithmetic and memory access are validated:

```rust
// Pointer must be valid before dereference
// Offset must be within object bounds
// Access size must match instruction

// BAD: Null pointer dereference
// BAD: Out-of-bounds access
// BAD: Misaligned access (if strict_alignment enabled)
```

**Error:** `VerifyError::InvalidMemoryAccess { pc: usize, reason: String }`

## Control Flow Graph

The verifier builds a CFG to analyze all possible execution paths:

```rust
use kernel_bpf::verifier::cfg::ControlFlowGraph;

let cfg = ControlFlowGraph::build(&program)?;

// Analyze basic blocks
for block in cfg.blocks() {
    println!("Block {}: instructions {}-{}",
        block.id, block.start, block.end);
    println!("  Successors: {:?}", block.successors);
    println!("  Predecessors: {:?}", block.predecessors);
}

// Check reachability
for unreachable in cfg.unreachable_blocks() {
    println!("Warning: unreachable code at {}", unreachable);
}
```

### CFG Example

```
Program:
  0: mov64 r0, 0
  1: jeq r1, 0, +2
  2: mov64 r0, 1
  3: ja +1
  4: mov64 r0, 2
  5: exit

CFG:
  Block 0 (insn 0-1): successors=[1, 2]
  Block 1 (insn 2-3): predecessors=[0], successors=[3]
  Block 2 (insn 4):   predecessors=[0], successors=[3]
  Block 3 (insn 5):   predecessors=[1, 2]
```

## State Tracking

The verifier tracks the state of registers and stack:

### Register States

```rust
pub enum RegState {
    /// Register contains garbage (uninitialized)
    Uninitialized,

    /// Register contains a known scalar value
    Scalar {
        value: Option<u64>,  // Known value, if determinable
        min: u64,            // Minimum possible value
        max: u64,            // Maximum possible value
    },

    /// Register contains a pointer
    Pointer {
        base: PointerBase,   // What it points to
        offset: Range<i64>,  // Offset range
    },
}

pub enum PointerBase {
    Stack,      // Points into stack
    Map,        // Points into map
    Context,    // Points into context
    Packet,     // Points into packet data
}
```

### Value Tracking

The verifier tracks value ranges through operations:

```rust
// Initial state
R0: Scalar { value: Some(10), min: 10, max: 10 }

// After: add64 r0, r1 (where R1 is 0..100)
R0: Scalar { value: None, min: 10, max: 110 }

// After: if r0 < 50, goto ...
// True branch: R0: Scalar { min: 10, max: 49 }
// False branch: R0: Scalar { min: 50, max: 110 }
```

## Profile-Specific Verification

### Cloud Profile

Standard verification with relaxed limits:

```rust
// Cloud allows more instructions
const MAX_INSN: usize = 1_000_000;

// Cloud allows more stack
const MAX_STACK: usize = 512 * 1024;
```

### Embedded Profile

Additional checks for real-time safety:

```rust
// Embedded has stricter limits
const MAX_INSN: usize = 100_000;
const MAX_STACK: usize = 8 * 1024;

// Additional checks:
// - WCET budget verification
// - No dynamic allocation
// - Bounded iteration proof
```

**Embedded-only errors:**
- `VerifyError::WCETExceeded { estimated: u64, budget: u64 }`
- `VerifyError::DynamicAllocation { pc: usize }`

## Error Reference

| Error | Description | Fix |
|-------|-------------|-----|
| `InvalidOpcode` | Unknown instruction opcode | Use valid BPF opcodes |
| `UninitializedRegister` | Reading uninitialized register | Initialize before use |
| `WriteToR10` | Attempting to modify frame pointer | Don't write to R10 |
| `DivisionByZero` | Division/modulo by zero | Check divisor first |
| `NoExit` | No reachable exit instruction | Add exit instruction |
| `UnboundedLoop` | Loop without provable bound | Add loop counter check |
| `StackOutOfBounds` | Stack access outside valid range | Check stack offset |
| `InvalidMemoryAccess` | Bad pointer dereference | Validate pointer first |
| `TooManyInstructions` | Program exceeds limit | Reduce program size |
| `StackOverflow` | Stack usage exceeds limit | Reduce stack usage |
| `OutOfBoundsJump` | Jump target outside program | Fix jump offset |
| `UnreachableCode` | Dead code detected | Remove or fix branches |

## Best Practices

1. **Initialize all registers** before use
2. **Check pointers** before dereferencing
3. **Bound all loops** with explicit counters
4. **Avoid R10 modification** - it's read-only
5. **Check divisors** before division
6. **Keep programs small** - stay under limits
7. **Use structured control flow** - avoid complex jumps
8. **Test edge cases** - empty input, max values

## Debugging Verification Failures

```rust
// Enable verbose verification
let options = VerifyOptions {
    verbose: true,
    ..Default::default()
};

let verifier = Verifier::with_options(options);
match verifier.verify(&program) {
    Ok(()) => println!("OK"),
    Err(e) => {
        println!("Error at PC {}: {}", e.pc(), e);
        println!("Register state: {:?}", e.state());
        println!("Instruction: {}", program.insn_at(e.pc()));
    }
}
```
