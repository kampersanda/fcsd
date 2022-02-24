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
pub mod builder;
pub mod decoder;
mod intvec;
pub mod iter;
pub mod locator;
pub mod prefix_iter;
mod utils;

use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use intvec::IntVector;
use std::cmp::Ordering;
use std::io;

pub use builder::FcBuilder;
pub use decoder::FcDecoder;
pub use iter::FcIterator;
pub use locator::FcLocator;
pub use prefix_iter::FcPrefixIterator;

const END_MARKER: u8 = 0;
const SERIAL_COOKIE: u32 = 114514;

/// Front-coding string dictionary that provides a bijection between strings and interger IDs.
/// Let n be the number of strings, the integer IDs are in [0..n-1] and assigned in the lex order.
#[derive(Clone)]
pub struct FcDict {
    pointers: IntVector,
    serialized: Vec<u8>,
    num_keys: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
}

impl FcDict {
    /// Returns the number of bytes needed to write the dictionary.
    pub fn serialized_size_in_bytes(&self) -> usize {
        let mut bytes = 0;
        bytes += 4; // SERIAL_COOKIE
        bytes += self.pointers.serialized_size_in_bytes(); // pointers
        bytes += 8 + self.serialized.len(); // serialized
        bytes + 8 * 4
    }

    /// Serializes the dictionary.
    pub fn serialize_into<W: io::Write>(&self, mut writer: W) -> Result<()> {
        writer.write_u32::<LittleEndian>(SERIAL_COOKIE)?;
        self.pointers.serialize_into(&mut writer)?;
        writer.write_u64::<LittleEndian>(self.serialized.len() as u64)?;
        for &x in &self.serialized {
            writer.write_u8(x)?;
        }
        writer.write_u64::<LittleEndian>(self.num_keys as u64)?;
        writer.write_u64::<LittleEndian>(self.bucket_bits as u64)?;
        writer.write_u64::<LittleEndian>(self.bucket_mask as u64)?;
        writer.write_u64::<LittleEndian>(self.max_length as u64)?;
        Ok(())
    }

    /// Deserializes the dictionary.
    pub fn deserialize_from<R: io::Read>(mut reader: R) -> Result<Self> {
        let cookie = reader.read_u32::<LittleEndian>()?;
        if cookie != SERIAL_COOKIE {
            return Err(anyhow!("unknown cookie value"));
        }
        let pointers = IntVector::deserialize_from(&mut reader)?;
        let serialized = {
            let len = reader.read_u64::<LittleEndian>()? as usize;
            let mut serialized = vec![0; len];
            for x in serialized.iter_mut() {
                *x = reader.read_u8()?;
            }
            serialized
        };

        let num_keys = reader.read_u64::<LittleEndian>()? as usize;
        let bucket_bits = reader.read_u64::<LittleEndian>()? as usize;
        let bucket_mask = reader.read_u64::<LittleEndian>()? as usize;
        let max_length = reader.read_u64::<LittleEndian>()? as usize;

        Ok(Self {
            pointers,
            serialized,
            num_keys,
            bucket_bits,
            bucket_mask,
            max_length,
        })
    }

    /// Makes the locator.
    pub fn locator(&self) -> FcLocator {
        FcLocator::new(self)
    }

    /// Makes the decoder.
    pub fn decoder(&self) -> FcDecoder {
        FcDecoder::new(self)
    }

    /// Makes the iterator.
    pub fn iter(&self) -> FcIterator {
        FcIterator::new(self)
    }

    /// Makes the prefix iterator.
    pub fn prefix_iter<'a>(&'a self, key: &'a [u8]) -> FcPrefixIterator {
        FcPrefixIterator::new(self, key)
    }

    /// Gets the number of stored keys.
    pub const fn num_keys(&self) -> usize {
        self.num_keys
    }

    /// Gets the number of defined buckets.
    pub const fn num_buckets(&self) -> usize {
        self.pointers.len()
    }

    /// Gets the bucket size.
    pub const fn bucket_size(&self) -> usize {
        self.bucket_mask + 1
    }

    const fn max_length(&self) -> usize {
        self.max_length
    }

    const fn bucket_id(&self, id: usize) -> usize {
        id >> self.bucket_bits
    }

    const fn pos_in_bucket(&self, id: usize) -> usize {
        id & self.bucket_mask
    }

    fn get_header(&self, bi: usize) -> &[u8] {
        let header = &self.serialized[self.pointers.get(bi) as usize..];
        &header[..utils::get_strlen(header)]
    }

    fn decode_header(&self, bi: usize, dec: &mut Vec<u8>) -> usize {
        dec.clear();
        let mut pos = self.pointers.get(bi) as usize;
        while self.serialized[pos] != END_MARKER {
            dec.push(self.serialized[pos]);
            pos += 1;
        }
        pos + 1
    }

    fn decode_lcp(&self, pos: usize) -> (usize, usize) {
        let (lcp, num) = utils::vbyte::decode(&self.serialized[pos..]);
        (lcp, pos + num)
    }

    fn decode_next(&self, mut pos: usize, dec: &mut Vec<u8>) -> usize {
        while self.serialized[pos] != END_MARKER {
            dec.push(self.serialized[pos]);
            pos += 1;
        }
        pos + 1
    }

    fn search_bucket(&self, key: &[u8]) -> (usize, bool) {
        let mut cmp = 0;
        let (mut lo, mut hi, mut mi) = (0, self.num_buckets(), 0);
        while lo < hi {
            mi = (lo + hi) / 2;
            cmp = utils::get_lcp(key, self.get_header(mi)).1;
            match cmp.cmp(&0) {
                Ordering::Less => lo = mi + 1,
                Ordering::Greater => hi = mi,
                Ordering::Equal => return (mi, true),
            }
        }
        if cmp < 0 || mi == 0 {
            (mi, false)
        } else {
            (mi - 1, false)
        }
    }
}

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
            assert_eq!(keys[i].as_bytes(), &decoder.run(i).unwrap());
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
        assert_eq!(buffer.len(), dict.serialized_size_in_bytes());

        let other = FcDict::deserialize_from(&buffer[..]).unwrap();
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
            let dec = decoder.run(i).unwrap();
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
        assert_eq!(buffer.len(), dict.serialized_size_in_bytes());

        let other = FcDict::deserialize_from(&buffer[..]).unwrap();
        let mut iterator = other.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(&keys[i], &dec);
        }
        assert!(iterator.next().is_none());
    }
}
