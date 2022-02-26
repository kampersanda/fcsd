use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

use criterion::{
    criterion_group, criterion_main, measurement::WallTime, BenchmarkGroup, Criterion, SamplingMode,
};

const SAMPLE_SIZE: usize = 10;
const WARM_UP_TIME: Duration = Duration::from_secs(5);
const MEASURE_TIME: Duration = Duration::from_secs(10);

const BUCKET_SIZES: [usize; 4] = [4, 8, 16, 32];

fn criterion_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("build");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);
    group.sampling_mode(SamplingMode::Flat);

    let keys = load_keyset("data/words-100000");
    build(&mut group, &keys);
}

fn criterion_locate(c: &mut Criterion) {
    let mut group = c.benchmark_group("locate");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);
    group.sampling_mode(SamplingMode::Flat);

    let keys = load_keyset("data/words-100000");
    locate(&mut group, &keys, &keys);
}

fn build(group: &mut BenchmarkGroup<WallTime>, keys: &[String]) {
    for &bs in &BUCKET_SIZES {
        group.bench_function(format!("fcsd<{}>", bs), |b| {
            b.iter(|| {
                fcsd::Set::with_bucket_size(keys, bs).unwrap();
            });
        });
    }

    group.bench_function("fst", |b| {
        b.iter(|| {
            fst::Map::from_iter(keys.iter().enumerate().map(|(i, k)| (k, i as u64))).unwrap();
        });
    });
}

fn locate(group: &mut BenchmarkGroup<WallTime>, keys: &[String], queries: &[String]) {
    for &bs in &BUCKET_SIZES {
        group.bench_function(format!("fcsd<{}>", bs), |b| {
            let dict = fcsd::Set::with_bucket_size(keys, bs).unwrap();
            let mut locator = dict.locator();
            b.iter(|| {
                let mut sum = 0;
                for q in queries {
                    sum += locator.run(q).unwrap();
                }
                if sum == 0 {
                    panic!();
                }
            });
        });
    }

    group.bench_function("fst", |b| {
        let dict =
            fst::Map::from_iter(keys.iter().enumerate().map(|(i, k)| (k, i as u64))).unwrap();
        b.iter(|| {
            let mut sum = 0;
            for q in queries {
                sum += dict.get(q).unwrap();
            }
            if sum == 0 {
                panic!();
            }
        });
    });
}

fn load_keyset<P>(path: P) -> Vec<String>
where
    P: AsRef<Path>,
{
    let file = File::open(path).unwrap();
    let buf = BufReader::new(file);
    let mut keys: Vec<_> = buf.lines().map(|line| line.unwrap()).collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

criterion_group!(benches, criterion_build, criterion_locate);

criterion_main!(benches);
