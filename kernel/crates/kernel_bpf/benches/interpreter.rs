//! Interpreter performance benchmarks.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use kernel_bpf::bytecode::insn::BpfInsn;
use kernel_bpf::bytecode::program::{BpfProgType, ProgramBuilder};
use kernel_bpf::execution::{BpfContext, BpfExecutor, Interpreter};
use kernel_bpf::profile::ActiveProfile;

/// Benchmark simple arithmetic program execution.
fn bench_arithmetic(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter/arithmetic");

    // Build a program that performs arithmetic operations
    let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
        .insn(BpfInsn::mov64_imm(0, 0)) // r0 = 0
        .insn(BpfInsn::mov64_imm(1, 100)) // r1 = 100
        .insn(BpfInsn::add64_reg(0, 1)) // r0 += r1
        .insn(BpfInsn::mul64_imm(0, 2)) // r0 *= 2
        .insn(BpfInsn::sub64_imm(0, 50)) // r0 -= 50
        .insn(BpfInsn::exit()) // return r0
        .build()
        .expect("valid program");

    let interp = Interpreter::<ActiveProfile>::new();
    let ctx = BpfContext::empty();

    group.bench_function("simple_math", |b| {
        b.iter(|| interp.execute(black_box(&program), black_box(&ctx)))
    });

    group.finish();
}

/// Benchmark loop execution.
fn bench_loop(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter/loop");

    for iterations in [10, 100, 1000] {
        // Build a counting loop
        let mut builder = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter);

        builder = builder
            .insn(BpfInsn::mov64_imm(0, 0)) // r0 = 0 (counter)
            .insn(BpfInsn::mov64_imm(1, iterations as i32)); // r1 = iterations

        // Loop body: increment counter, decrement iterations, jump if not zero
        let loop_start = 2;
        builder = builder
            .insn(BpfInsn::add64_imm(0, 1)) // r0++
            .insn(BpfInsn::sub64_imm(1, 1)) // r1--
            .insn(BpfInsn::jne_imm(1, 0, (loop_start as i16) - 4 - 1)) // if r1 != 0, goto loop_start
            .insn(BpfInsn::exit()); // return r0

        let program = builder.build().expect("valid program");
        let interp = Interpreter::<ActiveProfile>::new();
        let ctx = BpfContext::empty();

        group.throughput(Throughput::Elements(iterations as u64));
        group.bench_with_input(
            BenchmarkId::new("iterations", iterations),
            &iterations,
            |b, _| b.iter(|| interp.execute(black_box(&program), black_box(&ctx))),
        );
    }

    group.finish();
}

/// Benchmark conditional jumps.
fn bench_conditionals(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter/conditionals");

    // Build a program with multiple conditional branches using jeq and jne
    let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
        .insn(BpfInsn::mov64_imm(0, 42))
        .insn(BpfInsn::mov64_imm(1, 100))
        .insn(BpfInsn::jeq_reg(1, 0, 2)) // if r1 == r0, skip 2 (won't happen)
        .insn(BpfInsn::mov64_imm(0, 1)) // r0 = 1
        .insn(BpfInsn::ja(1)) // jump over next
        .insn(BpfInsn::mov64_imm(0, 0)) // r0 = 0 (skipped)
        .insn(BpfInsn::jeq_imm(0, 1, 1)) // if r0 == 1, skip 1
        .insn(BpfInsn::add64_imm(0, 10)) // skipped
        .insn(BpfInsn::exit())
        .build()
        .expect("valid program");

    let interp = Interpreter::<ActiveProfile>::new();
    let ctx = BpfContext::empty();

    group.bench_function("branching", |b| {
        b.iter(|| interp.execute(black_box(&program), black_box(&ctx)))
    });

    group.finish();
}

/// Benchmark register operations.
fn bench_registers(c: &mut Criterion) {
    let mut group = c.benchmark_group("interpreter/registers");

    // Build a program that uses many registers
    let program = ProgramBuilder::<ActiveProfile>::new(BpfProgType::SocketFilter)
        .insn(BpfInsn::mov64_imm(0, 1))
        .insn(BpfInsn::mov64_imm(1, 2))
        .insn(BpfInsn::mov64_imm(2, 3))
        .insn(BpfInsn::mov64_imm(3, 4))
        .insn(BpfInsn::mov64_imm(4, 5))
        .insn(BpfInsn::mov64_imm(5, 6))
        .insn(BpfInsn::add64_reg(0, 1))
        .insn(BpfInsn::add64_reg(0, 2))
        .insn(BpfInsn::add64_reg(0, 3))
        .insn(BpfInsn::add64_reg(0, 4))
        .insn(BpfInsn::add64_reg(0, 5))
        .insn(BpfInsn::exit())
        .build()
        .expect("valid program");

    let interp = Interpreter::<ActiveProfile>::new();
    let ctx = BpfContext::empty();

    group.bench_function("multi_register", |b| {
        b.iter(|| interp.execute(black_box(&program), black_box(&ctx)))
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_arithmetic,
    bench_loop,
    bench_conditionals,
    bench_registers,
);

criterion_main!(benches);
