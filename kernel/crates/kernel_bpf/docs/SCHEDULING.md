# BPF Scheduler Guide

This document describes the BPF program scheduler and how programs are scheduled for execution.

## Overview

The BPF scheduler manages pending program executions and determines the order in which programs run based on the active profile's policy.

## Scheduler Architecture

```
┌────────────────────────────────────────────────────────────────┐
│                        BpfScheduler                             │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    BpfQueue<P>                            │  │
│  │  ┌─────────┐  ┌─────────┐  ┌─────────┐  ┌─────────┐     │  │
│  │  │  Prog1  │  │  Prog2  │  │  Prog3  │  │  Prog4  │     │  │
│  │  │ Pri:Hi  │  │ Pri:Nor │  │ Pri:Low │  │ Pri:Cri │     │  │
│  │  │ t=100   │  │ t=200   │  │ t=150   │  │ t=50    │     │  │
│  │  └─────────┘  └─────────┘  └─────────┘  └─────────┘     │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                    Policy::select()                       │  │
│  │  ┌─────────────────────┐  ┌────────────────────────────┐ │  │
│  │  │  ThroughputPolicy   │  │     DeadlinePolicy         │ │  │
│  │  │  (cloud-profile)    │  │   (embedded-profile)       │ │  │
│  │  └─────────────────────┘  └────────────────────────────┘ │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│                       Next Program                              │
└────────────────────────────────────────────────────────────────┘
```

## Basic Usage

### Creating a Scheduler

```rust
use kernel_bpf::scheduler::BpfScheduler;

let mut scheduler = BpfScheduler::new();

// Check status
assert!(!scheduler.has_pending());
assert_eq!(scheduler.pending_count(), 0);
```

### Submitting Programs

```rust
use kernel_bpf::scheduler::{BpfExecRequest, ProgId, ExecPriority};
use kernel_bpf::execution::BpfContext;
use std::sync::Arc;

// Create execution request
let request = BpfExecRequest::new(
    ProgId(1),                    // Unique program ID
    Arc::new(program),            // The BPF program
    BpfContext::empty(),          // Execution context
);

// Submit for execution
scheduler.submit(request)?;

// With priority
let request = BpfExecRequest::new(ProgId(2), prog, ctx)
    .with_priority(ExecPriority::High);
scheduler.submit(request)?;
```

### Getting Next Program

```rust
// Get next program to execute
while let Some(queued) = scheduler.next() {
    println!("Executing program {}", queued.id.0);

    // Execute the program
    let result = executor.execute(&queued.program, &queued.context);

    match result {
        Ok(ret) => println!("Program returned: {}", ret),
        Err(e) => println!("Execution error: {}", e),
    }
}
```

### Canceling Programs

```rust
// Cancel a pending program
if scheduler.cancel(ProgId(42)) {
    println!("Program 42 cancelled");
} else {
    println!("Program 42 not found");
}
```

## Priority Levels

```rust
pub enum ExecPriority {
    /// Lowest priority - background execution
    Low = 0,

    /// Default priority
    Normal = 1,

    /// Elevated priority
    High = 2,

    /// Highest priority - critical execution
    Critical = 3,
}
```

### Priority Selection

```rust
let request = BpfExecRequest::new(ProgId(1), prog, ctx)
    .with_priority(ExecPriority::Critical);

// Critical priority programs run before all others
```

### Priority Ordering

Within the same priority level, FIFO ordering is used:

```
Submit order: P1(Normal), P2(Normal), P3(High), P4(Normal)

Execution order:
  1. P3 (High priority)
  2. P1 (Normal, first submitted)
  3. P2 (Normal, second submitted)
  4. P4 (Normal, last submitted)
```

## Scheduling Policies

### ThroughputPolicy (Cloud)

Optimizes for maximum throughput with fair scheduling.

```rust
#[cfg(feature = "cloud-profile")]
use kernel_bpf::scheduler::ThroughputPolicy;

// Automatically used when scheduler is created
let scheduler = BpfScheduler::new();  // Uses ThroughputPolicy

// Access execution stats
println!("Programs executed: {}", scheduler.exec_count());
```

**Algorithm:**
1. Find highest priority program in queue
2. Among same priority, select earliest submission (FIFO)
3. Remove from queue and return
4. No preemption - programs run to completion

**Characteristics:**
- Fair: same-priority programs get equal treatment
- High throughput: minimal scheduling overhead
- No starvation: FIFO within priority prevents indefinite waiting
- Cooperative: no preemption needed

### DeadlinePolicy (Embedded)

Implements Earliest Deadline First (EDF) for real-time scheduling.

```rust
#[cfg(feature = "embedded-profile")]
use kernel_bpf::scheduler::{DeadlinePolicy, Deadline};

// Automatically used when scheduler is created
let mut scheduler = BpfScheduler::new();  // Uses DeadlinePolicy

// Update current time (call from timer interrupt)
scheduler.update_time(current_time_ns);

// Create request with deadline
let deadline = Deadline::from_now(current_time_ns, 1_000_000); // 1ms
let request = BpfExecRequest::new(ProgId(1), prog, ctx)
    .with_deadline(deadline);
scheduler.submit(request)?;

// Check deadline misses
println!("Deadline misses: {}", scheduler.deadline_misses());
```

**Algorithm:**
1. Find program with earliest absolute deadline
2. If no deadlines, fall back to priority ordering
3. Track deadline misses for monitoring
4. Support preemption at safe points (not yet implemented)

**Characteristics:**
- Deadline-aware: meets real-time requirements
- Predictable: bounded latency for deadline programs
- Fallback: uses priority when no deadlines set
- Observable: tracks deadline miss statistics

## Deadlines (Embedded Only)

```rust
#[cfg(feature = "embedded-profile")]
{
    use kernel_bpf::scheduler::Deadline;

    // Absolute deadline
    let deadline = Deadline::new(
        absolute_ns: 1_000_000_000,  // 1 second from boot
        relative_ns: 500_000,        // Was 500us from submission
    );

    // Relative deadline (more common)
    let now = get_monotonic_time_ns();
    let deadline = Deadline::from_now(now, 1_000_000);  // 1ms from now

    // Check deadline status
    if deadline.is_expired(now) {
        println!("Deadline missed!");
    }

    let remaining = deadline.time_remaining(now);
    println!("{}ns until deadline", remaining);
}
```

## Queue Management

### Queue Limits

```rust
// Queue sizes are profile-dependent
#[cfg(feature = "cloud-profile")]
const MAX_QUEUE_SIZE: usize = 1024;

#[cfg(feature = "embedded-profile")]
const MAX_QUEUE_SIZE: usize = 32;
```

### Queue Full Handling

```rust
match scheduler.submit(request) {
    Ok(()) => println!("Submitted"),
    Err(SchedError::QueueFull) => {
        println!("Queue full! Dropping program or waiting...");
        // Options:
        // 1. Wait and retry
        // 2. Drop lowest priority
        // 3. Return error to caller
    }
    Err(e) => println!("Error: {}", e),
}
```

### Checking Queue Status

```rust
// Check if there are pending programs
if scheduler.has_pending() {
    println!("{} programs waiting", scheduler.pending_count());
}

// Check if queue can accept more
let can_submit = scheduler.pending_count() < MAX_QUEUE_SIZE;
```

## Program Identification

```rust
/// Unique identifier for a scheduled BPF program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProgId(pub u32);

// Generate unique IDs
static NEXT_ID: AtomicU32 = AtomicU32::new(1);

fn next_prog_id() -> ProgId {
    ProgId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
}
```

## Execution Context

```rust
use kernel_bpf::execution::BpfContext;

// Empty context
let ctx = BpfContext::empty();

// Context with data
let packet_data = vec![0u8; 1500];
let ctx = BpfContext::from_slice(&packet_data);

// Include in request
let request = BpfExecRequest::new(ProgId(1), prog, ctx);
```

## Statistics and Monitoring

### Execution Count

```rust
// Get total programs executed
let count = scheduler.exec_count();
println!("Executed {} programs", count);
```

### Deadline Misses (Embedded)

```rust
#[cfg(feature = "embedded-profile")]
{
    let misses = scheduler.deadline_misses();
    if misses > 0 {
        println!("WARNING: {} deadline misses!", misses);
    }
}
```

### Custom Metrics

```rust
struct SchedulerMetrics {
    submitted: u64,
    completed: u64,
    cancelled: u64,
    queue_high_water: usize,
}

impl SchedulerMetrics {
    fn record_submit(&mut self, scheduler: &BpfScheduler) {
        self.submitted += 1;
        self.queue_high_water = self.queue_high_water
            .max(scheduler.pending_count());
    }
}
```

## Error Handling

```rust
pub enum SchedError {
    /// Queue is full
    QueueFull,

    /// Program not found
    NotFound,

    /// Invalid deadline (embedded only)
    #[cfg(feature = "embedded-profile")]
    InvalidDeadline,

    /// Deadline miss detected (embedded only)
    #[cfg(feature = "embedded-profile")]
    DeadlineMiss,
}

// Handle errors
match scheduler.submit(request) {
    Ok(()) => {}
    Err(SchedError::QueueFull) => {
        // Handle full queue
    }
    #[cfg(feature = "embedded-profile")]
    Err(SchedError::InvalidDeadline) => {
        // Deadline already passed
    }
    Err(e) => {
        println!("Scheduling error: {}", e);
    }
}
```

## Integration Example

### Basic Executor Loop

```rust
use kernel_bpf::scheduler::{BpfScheduler, BpfExecRequest, ProgId};
use kernel_bpf::execution::{BpfContext, BpfExecutor, Interpreter};
use kernel_bpf::profile::ActiveProfile;

fn run_scheduler() {
    let mut scheduler = BpfScheduler::new();
    let executor = Interpreter::<ActiveProfile>::new();

    loop {
        // Wait for work
        while !scheduler.has_pending() {
            wait_for_event();
        }

        // Process all pending programs
        while let Some(queued) = scheduler.next() {
            let result = executor.execute(&queued.program, &queued.context);

            match result {
                Ok(ret) => {
                    handle_result(queued.id, ret);
                }
                Err(e) => {
                    handle_error(queued.id, e);
                }
            }
        }
    }
}
```

### With Deadline Handling (Embedded)

```rust
#[cfg(feature = "embedded-profile")]
fn run_realtime_scheduler() {
    let mut scheduler = BpfScheduler::new();
    let executor = Interpreter::<ActiveProfile>::new();

    loop {
        // Update time from hardware timer
        let now = read_timer_ns();
        scheduler.update_time(now);

        // Process programs
        while let Some(queued) = scheduler.next() {
            // Check if we have time
            if let Some(deadline) = queued.deadline {
                if deadline.is_expired(now) {
                    // Log deadline miss but still execute
                    log_deadline_miss(queued.id);
                }
            }

            let result = executor.execute(&queued.program, &queued.context);
            handle_result(queued.id, result);
        }

        // Report deadline statistics periodically
        if should_report() {
            println!("Deadline misses: {}", scheduler.deadline_misses());
        }
    }
}
```

## Best Practices

1. **Use appropriate priorities**: Reserve Critical for truly critical work
2. **Set realistic deadlines**: Account for worst-case execution time
3. **Monitor queue depth**: High queue depth indicates backpressure
4. **Handle queue full**: Decide policy (wait, drop, error)
5. **Track deadline misses**: Alert on excessive misses
6. **Use unique IDs**: Ensure ProgId uniqueness for tracking
7. **Update time regularly**: Call `update_time()` from timer interrupt
