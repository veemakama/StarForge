use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_basic(c: &mut Criterion) {
    c.bench_function("simulate_ops_10k", |b| {
        b.iter(|| {
            let mut acc: u64 = 0;
            for i in 0..10_000u64 {
                acc = acc.wrapping_add((i & 0xff) as u64);
            }
            black_box(acc);
        })
    });
}

criterion_group!(benches, bench_basic);
criterion_main!(benches);
