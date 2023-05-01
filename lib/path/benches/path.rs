use criterion::{criterion_group, criterion_main, Criterion};

criterion_group!(
    name = benches;
    // encapsulates CI noise we saw in
    // https://github.com/vectordotdev/vector/issues/5394
    config = Criterion::default().noise_threshold(0.05);
    targets = benchmark_lookup
);
criterion_main!(benches);

fn benchmark_lookup(c: &mut Criterion) {
    let mut group = c.benchmark_group("lookup");

    let lookup_str = "foo.bar.asdf[7].asdf";
    let lookup_str_escaped = "foo.\"b.ar\".\"asdf\\\"asdf\".asdf[7].asdf";

    group.bench_function("lookup_v2_parse", |b| {
        b.iter(|| path::ValuePath::segment_iter(&lookup_str).count())
    });

    group.bench_function("lookup_v2_parse_escaped", |b| {
        b.iter(|| path::ValuePath::segment_iter(&lookup_str_escaped).count())
    });
}
