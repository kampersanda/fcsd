use fcsd_bench::{build_fst, load_keyset, yada_keyset, BUCKET_SIZES, DATASETS};
use yada::builder::DoubleArrayBuilder;

fn main() {
    println!(
        "dataset,implementation,keys,raw_bytes,dictionary_bytes,bytes_per_key,compression_ratio"
    );
    for dataset in DATASETS {
        let keys = load_keyset(dataset);
        // Newline-delimited UTF-8, one terminator byte per unique key.
        let raw_bytes: usize = keys.iter().map(|k| k.len() + 1).sum();
        for bs in BUCKET_SIZES {
            let dict = fcsd::Set::with_bucket_size(&keys, bs).unwrap();
            print(
                dataset,
                &format!("fcsd-{bs}"),
                keys.len(),
                raw_bytes,
                dict.size_in_bytes(),
            );
        }
        print(
            dataset,
            "fst",
            keys.len(),
            raw_bytes,
            build_fst(&keys).as_fst().as_bytes().len(),
        );
        let bytes = DoubleArrayBuilder::build(&yada_keyset(&keys)).unwrap();
        print(dataset, "yada", keys.len(), raw_bytes, bytes.len());
    }
}

fn print(dataset: &str, implementation: &str, keys: usize, raw: usize, bytes: usize) {
    println!(
        "{dataset},{implementation},{keys},{raw},{bytes},{:.3},{:.6}",
        bytes as f64 / keys as f64,
        bytes as f64 / raw as f64
    );
}
