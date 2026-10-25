use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use fcsd_bench::{
    build_fst, load_keyset, missing_queries, query_order, yada_keyset, BUCKET_SIZES, DATASETS,
};
use fst::Streamer;
use yada::{builder::DoubleArrayBuilder, DoubleArray};

fn benchmarks(c: &mut Criterion) {
    for dataset in DATASETS {
        let keys = load_keyset(dataset);
        let pairs = yada_keyset(&keys);
        let order = query_order(keys.len());
        let hits: Vec<_> = order.iter().map(|&i| keys[i].as_str()).collect();
        let misses = missing_queries(&keys, &order);

        let mut group = c.benchmark_group(format!("{dataset}/build"));
        group.throughput(Throughput::Elements(keys.len() as u64));
        for bs in BUCKET_SIZES {
            group.bench_function(format!("fcsd-{bs}"), |b| {
                b.iter(|| fcsd::Set::with_bucket_size(black_box(&keys), bs).unwrap());
            });
        }
        group.bench_function("fst", |b| b.iter(|| build_fst(black_box(&keys))));
        group.bench_function("yada", |b| {
            b.iter(|| {
                DoubleArray::new(DoubleArrayBuilder::build(black_box(&pairs)).unwrap()).unwrap()
            });
        });
        group.finish();

        let fst = build_fst(&keys);
        let yada = DoubleArray::new(DoubleArrayBuilder::build(&pairs).unwrap()).unwrap();
        for (operation, queries) in [
            (
                "locate-hit",
                hits.iter().map(|s| s.as_bytes()).collect::<Vec<_>>(),
            ),
            ("locate-miss", misses.iter().map(|s| s.as_bytes()).collect()),
        ] {
            let mut group = c.benchmark_group(format!("{dataset}/{operation}"));
            group.throughput(Throughput::Elements(queries.len() as u64));
            for bs in BUCKET_SIZES {
                let dict = fcsd::Set::with_bucket_size(&keys, bs).unwrap();
                let mut locator = dict.locator();
                group.bench_function(format!("fcsd-{bs}"), |b| {
                    b.iter(|| {
                        for q in &queries {
                            black_box(locator.run(black_box(q)));
                        }
                    })
                });
            }
            group.bench_function("fst", |b| {
                b.iter(|| {
                    for q in &queries {
                        black_box(fst.get(black_box(q)));
                    }
                })
            });
            group.bench_function("yada", |b| {
                b.iter(|| {
                    for q in &queries {
                        black_box(yada.exact_match_search(black_box(q)));
                    }
                })
            });
            group.finish();
        }

        let mut group = c.benchmark_group(format!("{dataset}/decode"));
        group.throughput(Throughput::Elements(keys.len() as u64));
        for bs in BUCKET_SIZES {
            let dict = fcsd::Set::with_bucket_size(&keys, bs).unwrap();
            let mut decoder = dict.decoder();
            group.bench_function(format!("fcsd-{bs}"), |b| {
                b.iter(|| {
                    for &id in &order {
                        black_box(decoder.run(black_box(id)));
                    }
                })
            });
        }
        group.finish();

        let mut group = c.benchmark_group(format!("{dataset}/iterate"));
        group.throughput(Throughput::Elements(keys.len() as u64));
        for bs in BUCKET_SIZES {
            let dict = fcsd::Set::with_bucket_size(&keys, bs).unwrap();
            group.bench_function(format!("fcsd-{bs}"), |b| {
                b.iter(|| {
                    for item in dict.iter() {
                        black_box(item);
                    }
                })
            });
        }
        group.bench_function("fst", |b| {
            b.iter(|| {
                let mut stream = fst.stream();
                while let Some(item) = stream.next() {
                    black_box(item);
                }
            })
        });
        group.finish();
    }
}

criterion_group!(benches, benchmarks);
criterion_main!(benches);
