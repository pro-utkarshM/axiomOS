# rkBPF How-To Guide

A practical guide to using the rkBPF library and tools.

## Table of Contents

1. [Building the Project](#building-the-project)
2. [Using the Library](#using-the-library)
3. [Using rk-cli](#using-rk-cli)
4. [Using rk-bridge](#using-rk-bridge)
5. [Writing BPF Programs](#writing-bpf-programs)
6. [Profile Selection](#profile-selection)
7. [Benchmarking](#benchmarking)

---

## Building the Project

### Prerequisites

- Rust 1.75+ (2024 edition)
- Clang/LLVM (for compiling BPF programs)

### Build Commands

```bash
# Build with cloud profile (default for development)
cargo build --no-default-features --features cloud-profile -p kernel_bpf

# Build with embedded profile (for RPi4, Jetson)
cargo build --no-default-features --features embedded-profile -p kernel_bpf

# Build all userspace tools
cargo build -p rk_cli -p rk_bridge

# Run tests
cargo test --no-default-features --features cloud-profile -p kernel_bpf
cargo test --no-default-features --features embedded-profile -p kernel_bpf

# Run clippy
cargo clippy --no-default-features --features cloud-profile -p kernel_bpf -- -D warnings
```

---

## Using the Library

### Verifying a BPF Program

```rust
use kernel_bpf::verifier::StreamingVerifier;
use kernel_bpf::bytecode::Instruction;

// Create instructions (example: mov r0, 0; exit)
let instructions = vec![
    Instruction::mov64_imm(0, 0),  // r0 = 0
    Instruction::exit(),           // exit
];

// Verify the program
match StreamingVerifier::verify(&instructions) {
    Ok(()) => println!("Program verified successfully"),
    Err(e) => println!("Verification failed: {:?}", e),
}
```

### Loading a BPF Program from ELF

```rust
use kernel_bpf::loader::{BpfLoader, LoaderConfig};

let elf_bytes = std::fs::read("program.bpf.o")?;

let config = LoaderConfig::default();
let loader = BpfLoader::new(config);

match loader.load(&elf_bytes) {
    Ok(program) => {
        println!("Loaded {} instructions", program.instructions().len());
    }
    Err(e) => println!("Load failed: {:?}", e),
}
```

### Using Maps

```rust
use kernel_bpf::maps::{ArrayMap, HashMap, RingBuf, MapDef, MapType};

// Array Map
let def = MapDef::new(MapType::Array, 4, 8, 256);  // key=4, val=8, max=256
let mut array = ArrayMap::new(def)?;

let key: u32 = 0;
let value: u64 = 42;
array.update(&key.to_ne_bytes(), &value.to_ne_bytes(), 0)?;

// Hash Map
let def = MapDef::new(MapType::Hash, 8, 8, 1024);
let mut hash = HashMap::new(def)?;
hash.update(&key.to_ne_bytes(), &value.to_ne_bytes(), 0)?;

if let Some(val) = hash.lookup(&key.to_ne_bytes()) {
    println!("Found: {:?}", val);
}

// Ring Buffer
let ringbuf = RingBuf::new(4096)?;  // 4KB buffer
ringbuf.output(b"event data", 0)?;
```

### Using the Time-Series Map

```rust
use kernel_bpf::maps::{TimeSeriesMap, MapDef, MapType};

let def = MapDef::new(MapType::Array, 4, 8, 1000);  // 1000 entries
let mut ts = TimeSeriesMap::new(def)?;

// Push timestamped values
ts.push(1000, 100)?;  // timestamp=1000, value=100
ts.push(2000, 150)?;
ts.push(3000, 120)?;

// Query last N values
let recent = ts.get_last_n(2);

// Query time window
let window = ts.get_in_window(1500, 2500);

// Get statistics
let stats = ts.stats();
println!("Count: {}, Avg: {}", stats.count, stats.avg);
```

### Signing Programs

```rust
use kernel_bpf::signing::{ProgramHash, SignedProgramHeader, SignatureVerifier, TrustedKey};

// Hash a program
let program_bytes = &[/* ELF bytes */];
let hash = ProgramHash::compute(program_bytes);

// Create a verifier with trusted keys
let mut verifier = SignatureVerifier::new();

// Add a trusted public key
let public_key = [/* 32 bytes */];
let key = TrustedKey::new(public_key, "my-key")?;
verifier.add_key(key)?;

// Verify a signed program
let signed_bytes = &[/* signed program bytes */];
match verifier.verify_program(signed_bytes) {
    Ok(program_data) => println!("Signature valid"),
    Err(e) => println!("Verification failed: {:?}", e),
}
```

---

## Using rk-cli

The `rk` command-line tool provides deployment and management capabilities.

### Key Management

```bash
# Generate a new signing key pair
rk key generate --name mykey

# List all keys
rk key list

# Export public key (for distribution)
rk key export mykey --output mykey.pub

# Import a public key
rk key import --name partner --file partner.pub
```

### Building Programs

```bash
# Build a BPF program from C source
rk build program.bpf.c --output program.bpf.o

# Build with specific target
rk build program.bpf.c --target bpf --output program.bpf.o
```

### Signing Programs

```bash
# Sign a program with your key
rk sign program.bpf.o --key mykey --output program.signed.bpf

# Verify a signed program
rk verify program.signed.bpf
```

### Deployment

```bash
# Deploy to local system
rk deploy program.signed.bpf --attach kprobe:sys_write

# Deploy to remote system
rk deploy program.signed.bpf --remote user@robot.local --attach iio:accel

# List loaded programs
rk list

# Unload a program
rk unload <program-id>
```

### Project Scaffolding

```bash
# Initialize a new rkBPF project
rk init my-project

# This creates:
# my-project/
# ├── src/
# │   └── main.bpf.c
# ├── include/
# │   └── vmlinux.h
# ├── Makefile
# └── rk.toml
```

---

## Using rk-bridge

The ROS2 bridge consumes kernel events and publishes them to ROS2 topics.

### Running the Bridge

```bash
# Start the bridge daemon
rk-bridge --config /etc/rk-bridge.toml

# Run in demo mode (generates test events)
rk-bridge --demo

# Specify output format
rk-bridge --format json
rk-bridge --format ros2
```

### Configuration

```toml
# /etc/rk-bridge.toml

[ringbuf]
path = "/sys/fs/bpf/events"
size = 65536

[publishers.imu]
type = "ros2"
topic = "/rk/imu"

[publishers.motor]
type = "ros2"
topic = "/rk/motor"

[publishers.safety]
type = "stdout"
format = "json"
```

### Event Types

The bridge handles these event types:

| Event | Description | Fields |
|-------|-------------|--------|
| IMU | Accelerometer/gyro data | timestamp, accel_x/y/z, gyro_x/y/z |
| Motor | Motor command traces | timestamp, channel, duty_cycle, frequency |
| Safety | Safety interlock events | timestamp, source, triggered, action |
| GPIO | GPIO edge events | timestamp, chip, line, edge, value |
| TimeSeries | Generic time-series data | timestamp, series_id, value |
| Trace | Debug trace events | timestamp, message |

---

## Writing BPF Programs

### Basic Structure

```c
// program.bpf.c
#include <linux/bpf.h>
#include <bpf/bpf_helpers.h>

// Define a map
struct {
    __uint(type, BPF_MAP_TYPE_RINGBUF);
    __uint(max_entries, 4096);
} events SEC(".maps");

// Define event structure
struct event {
    __u64 timestamp;
    __u32 value;
};

// Program section determines attach point
SEC("kprobe/sys_write")
int trace_write(struct pt_regs *ctx)
{
    struct event *e;

    e = bpf_ringbuf_reserve(&events, sizeof(*e), 0);
    if (!e)
        return 0;

    e->timestamp = bpf_ktime_get_ns();
    e->value = 42;

    bpf_ringbuf_submit(e, 0);
    return 0;
}

char LICENSE[] SEC("license") = "GPL";
```

### Robotics-Specific Programs

```c
// iio_filter.bpf.c - Filter accelerometer readings
SEC("iio/device0/in_accel_x")
int filter_accel(struct iio_event *evt)
{
    // Drop readings outside valid range
    if (evt->value < -32768 || evt->value > 32767)
        return 0;  // Drop

    return 1;  // Forward to userspace
}

// gpio_safety.bpf.c - Safety interlock
SEC("gpio/chip0/line17/rising")
int limit_switch(struct gpio_event *evt)
{
    // Emergency stop when limit switch triggered
    bpf_motor_emergency_stop(MOTOR_ALL);

    struct safety_event e = {
        .timestamp = bpf_ktime_get_ns(),
        .source = GPIO_LIMIT_SWITCH,
        .triggered = 1,
    };
    bpf_ringbuf_output(&safety_events, &e, sizeof(e), 0);

    return 0;
}

// pwm_trace.bpf.c - Trace motor commands
SEC("pwm/chip0/channel0")
int trace_motor(struct pwm_state *state)
{
    struct motor_event e = {
        .timestamp = bpf_ktime_get_ns(),
        .duty_cycle = state->duty_cycle,
        .period = state->period,
    };
    bpf_ringbuf_output(&motor_events, &e, sizeof(e), 0);

    return 0;
}
```

### Available Helper Functions

```c
// Core helpers
void *bpf_map_lookup_elem(map, key);
int bpf_map_update_elem(map, key, value, flags);
int bpf_map_delete_elem(map, key);
int bpf_ringbuf_output(ringbuf, data, size, flags);
u64 bpf_ktime_get_ns(void);
int bpf_trace_printk(fmt, fmt_size, ...);

// Robotics helpers (rkBPF extensions)
int bpf_motor_emergency_stop(motor_mask);
int bpf_timeseries_push(map, key, value);
u64 bpf_sensor_last_timestamp(sensor_id);
int bpf_gpio_read(chip, line);
int bpf_gpio_write(chip, line, value);
int bpf_pwm_get_duty(chip, channel);
int bpf_iio_read_channel(device, channel);
int bpf_can_send(interface, id, data, len);
```

---

## Profile Selection

rkBPF supports two profiles for different deployment targets:

### Cloud Profile

For development machines and servers with ample resources.

```bash
cargo build --no-default-features --features cloud-profile -p kernel_bpf
```

| Limit | Value |
|-------|-------|
| Max instructions | 1,000,000 |
| Max maps | 256 |
| Max map entries | 1,000,000 |
| Stack size | 512 bytes |
| JIT | Enabled |
| Trusted keys | 32 |

### Embedded Profile

For resource-constrained devices (RPi4, Jetson Nano).

```bash
cargo build --no-default-features --features embedded-profile -p kernel_bpf
```

| Limit | Value |
|-------|-------|
| Max instructions | 4,096 |
| Max maps | 16 |
| Max map entries | 4,096 |
| Stack size | 256 bytes |
| JIT | Disabled (interpreter only) |
| Trusted keys | 4 |
| Memory pool | 64KB static |

### Checking Active Profile

```rust
use kernel_bpf::profile::{ActiveProfile, PhysicalProfile};

println!("Max instructions: {}", ActiveProfile::MAX_INSTRUCTIONS);
println!("JIT enabled: {}", ActiveProfile::JIT_ENABLED);
```

---

## Benchmarking

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench -p kernel_bpf

# Run specific benchmark
cargo bench -p kernel_bpf -- interpreter
cargo bench -p kernel_bpf -- verifier
cargo bench -p kernel_bpf -- maps
```

### Benchmark Suites

**Interpreter Benchmarks** (`benches/interpreter.rs`)
- Arithmetic operations (ADD, SUB, MUL, DIV)
- Loop execution (bounded iterations)
- Conditional branches
- Register operations

**Verifier Benchmarks** (`benches/verifier.rs`)
- Small program verification
- Scaling with program size
- Control flow complexity

**Map Benchmarks** (`benches/maps.rs`)
- Array lookup/update
- Hash map operations
- Ring buffer throughput

### Example Output

```
interpreter/arithmetic   time:   [45.2 ns 45.8 ns 46.5 ns]
interpreter/loop_100     time:   [892 ns 901 ns 912 ns]
verifier/small_program   time:   [12.3 us 12.5 us 12.8 us]
maps/array_lookup        time:   [8.2 ns 8.4 ns 8.6 ns]
maps/hash_lookup         time:   [23.1 ns 23.5 ns 24.0 ns]
```

---

## Troubleshooting

### Build Errors

**"no profile selected"**
```bash
# Must specify exactly one profile
cargo build --no-default-features --features cloud-profile -p kernel_bpf
```

**Clippy warnings as errors**
```bash
# CI uses -D warnings, fix all warnings
cargo clippy --no-default-features --features cloud-profile -p kernel_bpf -- -D warnings
```

### Verification Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `UnreachableInstruction` | Dead code after exit | Remove unreachable code |
| `UninitializedRegister` | Using register before setting | Initialize registers |
| `InvalidJumpTarget` | Jump outside program bounds | Check jump offsets |
| `BackwardJumpNotBounded` | Unbounded loop | Add loop bound or refactor |
| `StackOutOfBounds` | Stack access beyond limit | Reduce stack usage |

### Map Errors

| Error | Cause | Fix |
|-------|-------|-----|
| `MapFull` | Exceeded max_entries | Increase limit or delete old entries |
| `KeyNotFound` | Lookup on missing key | Check existence first |
| `InvalidKeySize` | Key size mismatch | Use correct size for map type |

---

## Next Steps

See [tasks.md](tasks.md) for current implementation status and [proposal.md](proposal.md) for the full project vision.
