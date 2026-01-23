//! Map performance benchmarks.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use kernel_bpf::maps::{ArrayMap, BpfMap, HashMap, MapDef, MapType, RingBufMap};
use kernel_bpf::profile::ActiveProfile;

/// Update flags constant (ANY = 0).
const MAP_UPDATE_ANY: u64 = 0;

/// Benchmark array map operations.
fn bench_array_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("maps/array");

    let def = MapDef::new(MapType::Array, 4, 8, 1024);
    let map: ArrayMap<ActiveProfile> = ArrayMap::new(def).expect("failed to create array map");

    // Benchmark lookups
    let key: u32 = 512;
    let key_bytes = key.to_ne_bytes();

    // Pre-populate
    let value = [0xABu8; 8];
    map.update(&key_bytes, &value, MAP_UPDATE_ANY).unwrap();

    group.bench_function("lookup", |b| b.iter(|| map.lookup(black_box(&key_bytes))));

    group.bench_function("update", |b| {
        b.iter(|| map.update(black_box(&key_bytes), black_box(&value), MAP_UPDATE_ANY))
    });

    // Benchmark sequential access
    group.throughput(Throughput::Elements(100));
    group.bench_function("sequential_100", |b| {
        b.iter(|| {
            for i in 0u32..100 {
                let k = i.to_ne_bytes();
                let _ = map.lookup(black_box(&k));
            }
        })
    });

    group.finish();
}

/// Benchmark hash map operations.
fn bench_hash_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("maps/hash");

    let def = MapDef::new(MapType::Hash, 8, 8, 1024);
    let map: HashMap<ActiveProfile> = HashMap::new(def).expect("failed to create hash map");

    // Pre-populate with some entries
    for i in 0u64..512 {
        let key = i.to_ne_bytes();
        let value = (i * 2).to_ne_bytes();
        map.update(&key, &value, MAP_UPDATE_ANY).unwrap();
    }

    // Benchmark lookup of existing key
    let existing_key = 256u64.to_ne_bytes();
    group.bench_function("lookup_hit", |b| {
        b.iter(|| map.lookup(black_box(&existing_key)))
    });

    // Benchmark lookup of non-existing key
    let missing_key = 9999u64.to_ne_bytes();
    group.bench_function("lookup_miss", |b| {
        b.iter(|| map.lookup(black_box(&missing_key)))
    });

    // Benchmark update
    let update_key = 100u64.to_ne_bytes();
    let update_value = 12345u64.to_ne_bytes();
    group.bench_function("update", |b| {
        b.iter(|| {
            map.update(
                black_box(&update_key),
                black_box(&update_value),
                MAP_UPDATE_ANY,
            )
        })
    });

    group.finish();
}

/// Benchmark hash map scaling.
fn bench_hash_scaling(c: &mut Criterion) {
    let mut group = c.benchmark_group("maps/hash_scaling");

    for size in [64, 256, 1024] {
        let def = MapDef::new(MapType::Hash, 8, 8, size);
        let map: HashMap<ActiveProfile> = HashMap::new(def).expect("failed to create hash map");

        // Fill to 50% capacity
        let fill = size / 2;
        for i in 0u64..(fill as u64) {
            let key = i.to_ne_bytes();
            let value = i.to_ne_bytes();
            map.update(&key, &value, MAP_UPDATE_ANY).unwrap();
        }

        // Lookup in filled map
        let key = (fill as u64 / 2).to_ne_bytes();

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::new("lookup_50pct", size), &size, |b, _| {
            b.iter(|| map.lookup(black_box(&key)))
        });
    }

    group.finish();
}

/// Benchmark ring buffer operations.
fn bench_ringbuf(c: &mut Criterion) {
    let mut group = c.benchmark_group("maps/ringbuf");

    let ringbuf: RingBufMap<ActiveProfile> =
        RingBufMap::new(4096).expect("failed to create ring buffer");

    // Benchmark output (reserve + write + commit)
    let data = [0xAAu8; 64];
    group.bench_function("output", |b| {
        b.iter(|| {
            let _ = ringbuf.output(black_box(&data), 0);
        })
    });

    // Throughput test
    group.throughput(Throughput::Bytes(64));
    group.bench_function("throughput_64b", |b| {
        b.iter(|| {
            let _ = ringbuf.output(black_box(&data), 0);
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_array_map,
    bench_hash_map,
    bench_hash_scaling,
    bench_ringbuf,
);

criterion_main!(benches);
