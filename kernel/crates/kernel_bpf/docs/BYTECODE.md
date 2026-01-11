# BPF Bytecode Reference

This document describes the BPF instruction set supported by kernel_bpf.

## Overview

BPF (Berkeley Packet Filter) uses a RISC-like instruction set with:
- 11 registers (R0-R10)
- 64-bit operations (with 32-bit variants)
- Fixed 8-byte instruction format
- Simple memory model

## Registers

| Register | Purpose | Caller/Callee Saved |
|----------|---------|---------------------|
| R0 | Return value, scratch | Caller |
| R1 | Argument 1, scratch | Caller |
| R2 | Argument 2, scratch | Caller |
| R3 | Argument 3, scratch | Caller |
| R4 | Argument 4, scratch | Caller |
| R5 | Argument 5, scratch | Caller |
| R6 | Callee-saved | Callee |
| R7 | Callee-saved | Callee |
| R8 | Callee-saved | Callee |
| R9 | Callee-saved | Callee |
| R10 | Frame pointer (read-only) | N/A |

### Register Usage

```rust
use kernel_bpf::bytecode::registers::BpfReg;

let r0 = BpfReg::R0;  // Return value
let r1 = BpfReg::R1;  // First argument
let fp = BpfReg::R10; // Frame pointer (read-only)
```

## Instruction Format

Each instruction is 8 bytes:

```
┌─────────┬─────────┬──────────────┬──────────────────────────────┐
│ opcode  │ dst:src │    offset    │             imm              │
│ (8 bit) │ (4:4)   │   (16 bit)   │          (32 bit)            │
└─────────┴─────────┴──────────────┴──────────────────────────────┘
  Byte 0    Byte 1    Bytes 2-3         Bytes 4-7
```

### Opcode Encoding

```
┌─────────┬─────────┬─────────┐
│  class  │  source │   op    │
│ (3 bit) │ (1 bit) │ (4 bit) │
└─────────┴─────────┴─────────┘
  Bits 0-2   Bit 3    Bits 4-7
```

**Classes:**
| Value | Class | Description |
|-------|-------|-------------|
| 0x00 | LD | Load from immediate |
| 0x01 | LDX | Load from memory |
| 0x02 | ST | Store immediate |
| 0x03 | STX | Store from register |
| 0x04 | ALU | 32-bit arithmetic |
| 0x05 | JMP | Jump (64-bit comparison) |
| 0x06 | JMP32 | Jump (32-bit comparison) |
| 0x07 | ALU64 | 64-bit arithmetic |

**Source:**
| Value | Meaning |
|-------|---------|
| 0 | Immediate (imm field) |
| 1 | Register (src field) |

## Instruction Categories

### ALU Instructions

64-bit arithmetic operations on registers.

| Opcode | Mnemonic | Operation |
|--------|----------|-----------|
| 0x07 | add64 imm | dst += imm |
| 0x0f | add64 reg | dst += src |
| 0x17 | sub64 imm | dst -= imm |
| 0x1f | sub64 reg | dst -= src |
| 0x27 | mul64 imm | dst *= imm |
| 0x2f | mul64 reg | dst *= src |
| 0x37 | div64 imm | dst /= imm |
| 0x3f | div64 reg | dst /= src |
| 0x47 | or64 imm | dst \|= imm |
| 0x4f | or64 reg | dst \|= src |
| 0x57 | and64 imm | dst &= imm |
| 0x5f | and64 reg | dst &= src |
| 0x67 | lsh64 imm | dst <<= imm |
| 0x6f | lsh64 reg | dst <<= src |
| 0x77 | rsh64 imm | dst >>= imm (logical) |
| 0x7f | rsh64 reg | dst >>= src (logical) |
| 0x87 | neg64 | dst = -dst |
| 0x97 | mod64 imm | dst %= imm |
| 0x9f | mod64 reg | dst %= src |
| 0xa7 | xor64 imm | dst ^= imm |
| 0xaf | xor64 reg | dst ^= src |
| 0xb7 | mov64 imm | dst = imm |
| 0xbf | mov64 reg | dst = src |
| 0xc7 | arsh64 imm | dst >>= imm (arithmetic) |
| 0xcf | arsh64 reg | dst >>= src (arithmetic) |

**Usage:**

```rust
use kernel_bpf::bytecode::insn::BpfInsn;

// dst = 42
let mov = BpfInsn::mov64_imm(0, 42);

// dst += 10
let add = BpfInsn::add64_imm(0, 10);

// dst -= src
let sub = BpfInsn::sub64_reg(0, 1);

// dst = -dst
let neg = BpfInsn::neg64(0);
```

### Jump Instructions

Control flow operations.

| Opcode | Mnemonic | Condition |
|--------|----------|-----------|
| 0x05 | ja | unconditional |
| 0x15 | jeq imm | dst == imm |
| 0x1d | jeq reg | dst == src |
| 0x25 | jgt imm | dst > imm (unsigned) |
| 0x2d | jgt reg | dst > src (unsigned) |
| 0x35 | jge imm | dst >= imm (unsigned) |
| 0x3d | jge reg | dst >= src (unsigned) |
| 0x45 | jset imm | dst & imm |
| 0x4d | jset reg | dst & src |
| 0x55 | jne imm | dst != imm |
| 0x5d | jne reg | dst != src |
| 0x65 | jsgt imm | dst > imm (signed) |
| 0x6d | jsgt reg | dst > src (signed) |
| 0x75 | jsge imm | dst >= imm (signed) |
| 0x7d | jsge reg | dst >= src (signed) |
| 0x85 | call | call helper function |
| 0x95 | exit | return R0 |
| 0xa5 | jlt imm | dst < imm (unsigned) |
| 0xad | jlt reg | dst < src (unsigned) |
| 0xb5 | jle imm | dst <= imm (unsigned) |
| 0xbd | jle reg | dst <= src (unsigned) |
| 0xc5 | jslt imm | dst < imm (signed) |
| 0xcd | jslt reg | dst < src (signed) |
| 0xd5 | jsle imm | dst <= imm (signed) |
| 0xdd | jsle reg | dst <= src (signed) |

**Usage:**

```rust
// Unconditional jump (offset in instructions)
let ja = BpfInsn::ja(5);  // Jump forward 5 instructions

// Conditional jump
let jeq = BpfInsn::jeq_imm(0, 42, 3);  // if R0 == 42, skip 3

// Compare registers
let jne = BpfInsn::jne_reg(0, 1, 2);  // if R0 != R1, skip 2

// Exit program
let exit = BpfInsn::exit();  // Return value in R0
```

### Memory Instructions

Load and store operations.

| Opcode | Mnemonic | Size | Operation |
|--------|----------|------|-----------|
| 0x18 | lddw | 64-bit | dst = imm64 (wide) |
| 0x61 | ldxw | 32-bit | dst = *(u32*)(src + off) |
| 0x69 | ldxh | 16-bit | dst = *(u16*)(src + off) |
| 0x71 | ldxb | 8-bit | dst = *(u8*)(src + off) |
| 0x79 | ldxdw | 64-bit | dst = *(u64*)(src + off) |
| 0x62 | stw | 32-bit | *(u32*)(dst + off) = imm |
| 0x6a | sth | 16-bit | *(u16*)(dst + off) = imm |
| 0x72 | stb | 8-bit | *(u8*)(dst + off) = imm |
| 0x7a | stdw | 64-bit | *(u64*)(dst + off) = imm |
| 0x63 | stxw | 32-bit | *(u32*)(dst + off) = src |
| 0x6b | stxh | 16-bit | *(u16*)(dst + off) = src |
| 0x73 | stxb | 8-bit | *(u8*)(dst + off) = src |
| 0x7b | stxdw | 64-bit | *(u64*)(dst + off) = src |

**Wide Instructions:**

The `lddw` instruction uses two 8-byte slots to load a 64-bit immediate:

```rust
use kernel_bpf::bytecode::insn::WideInsn;

// Load 64-bit immediate into R0
let wide = WideInsn::ld_dw_imm(0, 0x123456789ABCDEF0);
// Uses slots: [0x18, dst, 0, 0, imm_lo] [0x00, 0, 0, 0, imm_hi]
```

### Atomic Instructions

Atomic memory operations (when supported).

| Opcode | Operation |
|--------|-----------|
| 0xdb + 0x00 | atomic add |
| 0xdb + 0x40 | atomic or |
| 0xdb + 0x50 | atomic and |
| 0xdb + 0xa0 | atomic xor |
| 0xdb + 0xe1 | atomic xchg |
| 0xdb + 0xf1 | atomic cmpxchg |

## Building Programs

### Using ProgramBuilder

```rust
use kernel_bpf::bytecode::insn::BpfInsn;
use kernel_bpf::bytecode::program::{BpfProgType, ProgramBuilder};
use kernel_bpf::profile::ActiveProfile;

let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    // R0 = 10
    .insn(BpfInsn::mov64_imm(0, 10))
    // R0 += 32
    .insn(BpfInsn::add64_imm(0, 32))
    // Return R0
    .insn(BpfInsn::exit())
    .build()
    .expect("valid program");

assert_eq!(program.insn_count(), 3);
```

### Program Types

```rust
pub enum BpfProgType {
    SocketFilter,    // Network packet filtering
    Kprobe,          // Kernel probe
    Tracepoint,      // Tracepoint
    XDP,             // eXpress Data Path
    PerfEvent,       // Performance events
    CgroupSkb,       // Cgroup socket buffer
    LwtIn,           // Lightweight tunnel ingress
    LwtOut,          // Lightweight tunnel egress
    LwtXmit,         // Lightweight tunnel transmit
    SchedCls,        // Traffic classifier
    SchedAct,        // Traffic action
}
```

## Example Programs

### Return Constant

```rust
// Return 42
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 42))
    .insn(BpfInsn::exit())
    .build()?;
```

### Arithmetic

```rust
// Return (10 + 5) * 3 = 45
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 10))
    .insn(BpfInsn::add64_imm(0, 5))
    .insn(BpfInsn::mul64_imm(0, 3))
    .insn(BpfInsn::exit())
    .build()?;
```

### Conditional

```rust
// If R1 == 1, return 100, else return 200
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(1, 1))      // R1 = 1
    .insn(BpfInsn::jeq_imm(1, 1, 2))     // if R1 == 1, skip 2
    .insn(BpfInsn::mov64_imm(0, 200))    // R0 = 200
    .insn(BpfInsn::exit())               // return
    .insn(BpfInsn::mov64_imm(0, 100))    // R0 = 100
    .insn(BpfInsn::exit())               // return
    .build()?;
```

### Loop

```rust
// Count from 0 to 10
let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
    .insn(BpfInsn::mov64_imm(0, 0))      // R0 = 0 (counter)
    .insn(BpfInsn::mov64_imm(1, 10))     // R1 = 10 (limit)
    // loop:
    .insn(BpfInsn::jeq_reg(0, 1, 2))     // if R0 == R1, exit
    .insn(BpfInsn::add64_imm(0, 1))      // R0++
    .insn(BpfInsn::ja(-3))               // goto loop
    // exit:
    .insn(BpfInsn::exit())               // return R0
    .build()?;
```

## Instruction Display

Instructions can be formatted for debugging:

```rust
let insn = BpfInsn::add64_imm(0, 42);
println!("{}", insn);  // "add64 r0, 42"

let insn = BpfInsn::jeq_imm(1, 0, 5);
println!("{}", insn);  // "jeq r1, 0, +5"

let insn = BpfInsn::exit();
println!("{}", insn);  // "exit"
```

## Profile Constraints

### Cloud Profile
- Up to 1,000,000 instructions per program
- 512 KB stack
- JIT compilation available

### Embedded Profile
- Up to 100,000 instructions per program
- 8 KB stack
- Interpreter/AOT only

```rust
// This will fail on embedded profile
let huge_program = ProgramBuilder::<EmbeddedProfile>::new(BpfProgType::SocketFilter)
    .insns(vec![BpfInsn::nop(); 200_000])  // Too many!
    .insn(BpfInsn::exit())
    .build();  // Error: TooManyInstructions
```
