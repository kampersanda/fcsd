//! Shared inputs and builders for the timing and size benchmarks.
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub const DATASETS: [&str; 4] = [
    "words-10000",
    "words-100000",
    "wiki-urls-10000",
    "wiki-urls-100000",
];
pub const BUCKET_SIZES: [usize; 4] = [4, 8, 16, 32];

pub fn load_keyset(dataset: &str) -> Vec<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("data")
        .join(dataset);
    let mut keys: Vec<_> = BufReader::new(File::open(path).unwrap())
        .lines()
        .map(Result::unwrap)
        .collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Fixed-seed Fisher-Yates permutation, independent of external RNG versions.
pub fn query_order(len: usize) -> Vec<usize> {
    let mut order: Vec<_> = (0..len).collect();
    let mut state = 42_u64;
    for i in (1..len).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        order.swap(i, (state % (i as u64 + 1)) as usize);
    }
    order
}

/// Append a non-NUL byte until the candidate is absent from the sorted keyset.
pub fn missing_queries(keys: &[String], order: &[usize]) -> Vec<String> {
    order
        .iter()
        .map(|&i| {
            let mut query = format!("{}~", keys[i]);
            while keys.binary_search(&query).is_ok() {
                query.push('~');
            }
            query
        })
        .collect()
}

pub fn build_fst(keys: &[String]) -> fst::Map<Vec<u8>> {
    fst::Map::from_iter(keys.iter().enumerate().map(|(i, k)| (k, i as u64))).unwrap()
}

pub fn yada_keyset(keys: &[String]) -> Vec<(&[u8], u32)> {
    keys.iter()
        .enumerate()
        .map(|(i, k)| (k.as_bytes(), i as u32))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_datasets_agree_on_ids_and_misses() {
        for dataset in DATASETS {
            let keys = load_keyset(dataset);
            assert!(keys.len() < (1 << 31));
            assert!(keys.iter().all(|k| !k.as_bytes().contains(&0)));
            let order = query_order(keys.len());
            let mut sorted = order.clone();
            sorted.sort_unstable();
            assert_eq!(sorted, (0..keys.len()).collect::<Vec<_>>());
            let misses = missing_queries(&keys, &order);
            let fst = build_fst(&keys);
            let yada = yada::DoubleArray::new(
                yada::builder::DoubleArrayBuilder::build(&yada_keyset(&keys)).unwrap(),
            )
            .unwrap();
            for bs in BUCKET_SIZES {
                let fcsd = fcsd::Set::with_bucket_size(&keys, bs).unwrap();
                let mut locator = fcsd.locator();
                let mut decoder = fcsd.decoder();
                for (&id, miss) in order.iter().zip(&misses) {
                    assert_eq!(locator.run(&keys[id]), Some(id));
                    assert_eq!(fst.get(&keys[id]), Some(id as u64));
                    assert_eq!(yada.exact_match_search(&keys[id]), Some(id as u32));
                    assert_eq!(decoder.run(id), keys[id].as_bytes());
                    assert_eq!(locator.run(miss), None);
                    assert_eq!(fst.get(miss), None);
                    assert_eq!(yada.exact_match_search(miss), None);
                }
                let entries: Vec<_> = fcsd.iter().collect();
                assert_eq!(entries.len(), keys.len());
                for (id, (actual_id, key)) in entries.iter().enumerate() {
                    assert_eq!(*actual_id, id);
                    assert_eq!(key, keys[id].as_bytes());
                }
            }
        }
    }
}
