//! # Front-coding string dictionary in Rust
//!
//! ![](https://github.com/kampersanda/fcsd/actions/workflows/rust.yml/badge.svg)
//! [![Documentation](https://docs.rs/fcsd/badge.svg)](https://docs.rs/fcsd)
//! [![Crates.io](https://img.shields.io/crates/v/fcsd.svg)](https://crates.io/crates/fcsd)
//! [![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/kampersanda/fcsd/blob/master/LICENSE)
//!
//!
//! This is a Rust library of the (plain) front-coding string dictionary described in [*Martínez-Prieto et al., Practical compressed string dictionaries, INFOSYS 2016*](https://doi.org/10.1016/j.is.2015.08.008).
//!
//! ## Features
//!
//! - **Dictionary encoding.** Fcsd provides a bijective mapping between strings and integer IDs. It is so-called *dictionary encoding* and useful for text compression in many applications.
//! - **Simple and fast compression.** Fcsd maintains a set of strings in a compressed space through *front-coding*, a differential compression technique for strings, allowing for fast decompression operations.
//! - **Random access.** Fcsd maintains strings through a bucketization technique enabling to directly decompress arbitrary strings and perform binary search for strings.
//!
//! ## Note
//!
//! - Input keys must not contain `\0` character because the character is used for the string delimiter.
//! - The bucket size of 8 is recommended in space-time tradeoff by Martínez-Prieto's paper.
mod intvec;
pub mod set;
mod utils;

pub use set::builder::FcBuilder;
pub use set::Set;

/// Special terminator, which must not be contained in stored keys.
pub const END_MARKER: u8 = 0;

/// Default parameter for the number of keys in each bucket.
pub const DEFAULT_BUCKET_SIZE: usize = 8;

/// Serial cookie value for serialization.
const SERIAL_COOKIE: u32 = 114514;

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaChaRng;

    fn gen_random_keys(num: usize, max_len: usize, seed: u64) -> Vec<Vec<u8>> {
        let mut rng = ChaChaRng::seed_from_u64(seed);
        let mut keys = Vec::with_capacity(num);
        for _ in 0..num {
            let len = (rng.gen::<usize>() % (max_len - 1)) + 1;
            keys.push((0..len).map(|_| (rng.gen::<u8>() % 4) + 1).collect());
        }
        keys.sort();
        keys.dedup();
        keys
    }

    #[test]
    fn test_toy() {
        let keys = [
            "deal",
            "idea",
            "ideal",
            "ideas",
            "ideology",
            "tea",
            "techie",
            "technology",
            "tie",
            "trie",
        ];

        assert!(FcBuilder::new(0).is_err());
        assert!(FcBuilder::new(3).is_err());
        let mut builder = FcBuilder::new(4).unwrap();

        for &key in &keys {
            builder.add(key.as_bytes()).unwrap();
        }
        assert!(builder.add("tri".as_bytes()).is_err());
        assert!(builder.add(&[0xFF, 0x00]).is_err());

        let dict = builder.finish();

        let mut locator = dict.locator();
        for i in 0..keys.len() {
            let id = locator.run(keys[i].as_bytes()).unwrap();
            assert_eq!(i, id);
        }
        assert!(locator.run("aaa".as_bytes()).is_none());
        assert!(locator.run("tell".as_bytes()).is_none());
        assert!(locator.run("techno".as_bytes()).is_none());
        assert!(locator.run("zzz".as_bytes()).is_none());

        let mut decoder = dict.decoder();
        for i in 0..keys.len() {
            assert_eq!(keys[i].as_bytes(), &decoder.run(i));
        }

        let mut iterator = dict.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(keys[i].as_bytes(), &dec);
        }
        assert!(iterator.next().is_none());

        let mut iterator = dict.prefix_iter("idea".as_bytes());
        {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(1, id);
            assert_eq!(keys[1].as_bytes(), &dec);
        }
        {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(2, id);
            assert_eq!(keys[2].as_bytes(), &dec);
        }
        {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(3, id);
            assert_eq!(keys[3].as_bytes(), &dec);
        }
        assert!(iterator.next().is_none());

        let mut buffer = vec![];
        dict.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), dict.size_in_bytes());

        let other = Set::deserialize_from(&buffer[..]).unwrap();
        let mut iterator = other.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(keys[i].as_bytes(), &dec);
        }
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_random() {
        let keys = gen_random_keys(10000, 8, 11);
        let mut builder = FcBuilder::new(8).unwrap();

        for key in &keys {
            builder.add(key).unwrap();
        }
        let dict = builder.finish();

        let mut locator = dict.locator();
        for i in 0..keys.len() {
            let id = locator.run(&keys[i]).unwrap();
            assert_eq!(i, id);
        }

        let mut decoder = dict.decoder();
        for i in 0..keys.len() {
            let dec = decoder.run(i);
            assert_eq!(&keys[i], &dec);
        }

        let mut iterator = dict.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(&keys[i], &dec);
        }
        assert!(iterator.next().is_none());

        let mut buffer = vec![];
        dict.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), dict.size_in_bytes());

        let other = Set::deserialize_from(&buffer[..]).unwrap();
        let mut iterator = other.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(&keys[i], &dec);
        }
        assert!(iterator.next().is_none());
    }
}
