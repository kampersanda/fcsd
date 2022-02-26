use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

fn main() {
    memory("data/words-100000");
    memory("data/wiki-urls-100000");
}

fn memory(filename: &str) {
    println!("== {} ==", filename);
    let keys = load_keyset(filename);

    // +1 is for the terminator.
    let orig_size = keys.iter().fold(0, |acc, k| acc + k.len() + 1);
    {
        let dict = fcsd::FcDict::new(&keys).unwrap();
        print("fcsd", dict.size_in_bytes(), orig_size);
    }
    {
        let map = fst::Map::from_iter(keys.iter().enumerate().map(|(i, k)| (k, i as u64))).unwrap();
        print("fst", map.as_fst().as_bytes().len(), orig_size);
    }
}

fn print(title: &str, dict: usize, orig: usize) {
    println!(
        "{}: {} bytes, {:.3} MiB, ComprRatio={:.3}",
        title,
        dict,
        dict as f64 / (1024. * 1024.),
        dict as f64 / orig as f64
    )
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
