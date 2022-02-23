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
mod utils;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use intvec::IntVector;
use std::cmp::Ordering;
use std::io;

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
    /// Builds the dictionary from the given builder.
    pub fn from_builder(builder: FcBuilder) -> Self {
        Self {
            pointers: IntVector::build(&builder.pointers),
            serialized: builder.serialized,
            num_keys: builder.num_keys,
            bucket_bits: builder.bucket_bits,
            bucket_mask: builder.bucket_mask,
            max_length: builder.max_length,
        }
    }

    /// Returns the number of bytes needed to write the dictionary.
    pub fn serialized_size_in_bytes(&self) -> usize {
        let mut bytes = 0;
        bytes += 4; // SERIAL_COOKIE
        bytes += self.pointers.serialized_size_in_bytes(); // pointers
        bytes += 8 + self.serialized.len(); // serialized
        bytes + 8 * 4
    }

    /// Serializes the dictionary.
    pub fn serialize_into<W: io::Write>(&self, mut writer: W) -> io::Result<()> {
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
    pub fn deserialize_from<R: io::Read>(mut reader: R) -> io::Result<Self> {
        let cookie = reader.read_u32::<LittleEndian>()?;
        if cookie != SERIAL_COOKIE {
            return Err(io::Error::new(io::ErrorKind::Other, "unknown cookie value"));
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

/// Builder class of front-coding string dictionary.
#[derive(Clone)]
pub struct FcBuilder {
    pointers: Vec<u64>,
    serialized: Vec<u8>,
    last_key: Vec<u8>,
    num_keys: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
}

impl FcBuilder {
    /// Makes the builder with the given bucket size.
    /// The bucket size needs to be a power of two.
    pub fn new(bucket_size: usize) -> Result<Self, String> {
        if bucket_size == 0 {
            Err("bucket_size is zero.".to_owned())
        } else if !utils::is_power_of_two(bucket_size) {
            Err("bucket_size is not a power of two.".to_owned())
        } else {
            Ok(Self {
                pointers: Vec::new(),
                serialized: Vec::new(),
                last_key: Vec::new(),
                num_keys: 0,
                bucket_bits: utils::needed_bits((bucket_size - 1) as u64),
                bucket_mask: bucket_size - 1,
                max_length: 0,
            })
        }
    }

    /// Adds the given key string to the dictionary.
    /// The keys have to be given in the lex order.
    /// The key must not contain the 0 value.
    pub fn add(&mut self, key: &[u8]) -> Result<(), String> {
        if utils::contains_end_marker(key) {
            return Err("The input key contains END_MARKER.".to_owned());
        }

        let (lcp, cmp) = utils::get_lcp(&self.last_key, key);
        if cmp <= 0 {
            return Err("The input key is less than the previous one.".to_owned());
        }

        if self.num_keys & self.bucket_mask == 0 {
            self.pointers.push(self.serialized.len() as u64);
            self.serialized.extend_from_slice(key);
        } else {
            utils::vbyte::append(&mut self.serialized, lcp);
            self.serialized.extend_from_slice(&key[lcp..]);
        }
        self.serialized.push(END_MARKER);

        self.last_key.resize(key.len(), 0);
        self.last_key.copy_from_slice(key);
        self.num_keys += 1;
        self.max_length = std::cmp::max(self.max_length, key.len());

        Ok(())
    }
}

/// Locator class to get the ID associated with a key string.
#[derive(Clone)]
pub struct FcLocator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcLocator<'a> {
    /// Makes the locator.
    pub fn new(dict: &'a FcDict) -> FcLocator<'a> {
        FcLocator {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
        }
    }

    /// Returns the ID associated with the given key.
    pub fn run(&mut self, key: &[u8]) -> Option<usize> {
        if key.is_empty() {
            return None;
        }

        let (dict, dec) = (&self.dict, &mut self.dec);
        let (bi, found) = dict.search_bucket(key);

        if found {
            return Some(bi * dict.bucket_size());
        }

        let mut pos = dict.decode_header(bi, dec);
        if pos == dict.serialized.len() {
            return None;
        }

        // 1) Process the 1st internal string
        {
            let (dec_lcp, next_pos) = dict.decode_lcp(pos);
            pos = next_pos;
            dec.resize(dec_lcp, 0);
            pos = dict.decode_next(pos, dec);
        }

        let (mut lcp, cmp) = utils::get_lcp(key, dec);
        match cmp.cmp(&0) {
            Ordering::Equal => {
                return Some(bi * dict.bucket_size() + 1);
            }
            Ordering::Greater => return None,
            _ => {}
        }

        // 2) Process the next strings
        for bj in 2..dict.bucket_size() {
            if pos == dict.serialized.len() {
                break;
            }

            let (dec_lcp, next_pos) = dict.decode_lcp(pos);
            pos = next_pos;

            if lcp > dec_lcp {
                break;
            }

            dec.resize(dec_lcp, 0);
            pos = dict.decode_next(pos, dec);

            if lcp == dec_lcp {
                let (next_lcp, cmp) = utils::get_lcp(key, dec);
                match cmp.cmp(&0) {
                    Ordering::Equal => {
                        return Some(bi * dict.bucket_size() + bj);
                    }
                    Ordering::Greater => break,
                    _ => {}
                }
                lcp = next_lcp;
            }
        }

        None
    }
}

/// Decoder class to get the key string associated with an ID.
#[derive(Clone)]
pub struct FcDecoder<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcDecoder<'a> {
    /// Makes the decoder.
    pub fn new(dict: &'a FcDict) -> FcDecoder<'a> {
        FcDecoder {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
        }
    }

    /// Returns the string associated with the given ID.
    pub fn run(&mut self, id: usize) -> Option<Vec<u8>> {
        let (dict, dec) = (&self.dict, &mut self.dec);
        if dict.num_keys() <= id {
            return None;
        }

        let (bi, bj) = (dict.bucket_id(id), dict.pos_in_bucket(id));
        let mut pos = dict.decode_header(bi, dec);

        for _ in 0..bj {
            let (lcp, num) = utils::vbyte::decode(&dict.serialized[pos..]);
            pos += num;

            dec.resize(lcp, 0);
            pos = dict.decode_next(pos, dec);
        }

        Some(dec.clone())
    }
}

/// Iterator class to enumerate the stored keys and IDs in lex order.
#[derive(Clone)]
pub struct FcIterator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
    pos: usize,
    id: usize,
}

impl<'a> FcIterator<'a> {
    /// Makes the iterator.
    pub fn new(dict: &'a FcDict) -> FcIterator<'a> {
        FcIterator {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
            pos: 0,
            id: 0,
        }
    }
}

impl<'a> Iterator for FcIterator<'a> {
    type Item = (usize, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.dict.serialized.len() {
            return None;
        }
        if self.dict.pos_in_bucket(self.id) == 0 {
            self.dec.clear();
        } else {
            let (lcp, next_pos) = self.dict.decode_lcp(self.pos);
            self.pos = next_pos;
            self.dec.resize(lcp, 0);
        }
        self.pos = self.dict.decode_next(self.pos, &mut self.dec);
        self.id += 1;
        Some((self.id - 1, self.dec.clone()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.dict.num_keys(), Some(self.dict.num_keys()))
    }
}

/// Iterator class to enumerate the stored keys and IDs in lex order, starting with a prefix.
#[derive(Clone)]
pub struct FcPrefixIterator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
    key: &'a [u8],
    pos: usize,
    id: usize,
}

impl<'a> FcPrefixIterator<'a> {
    /// Makes the iterator with the prefix key.
    pub fn new(dict: &'a FcDict, key: &'a [u8]) -> FcPrefixIterator<'a> {
        FcPrefixIterator {
            key,
            dict,
            dec: Vec::with_capacity(dict.max_length()),
            pos: 0,
            id: 0,
        }
    }

    /// Inits the prefix key.
    pub fn init_key(&mut self, key: &'a [u8]) {
        self.key = key;
        self.dec.clear();
        self.pos = 0;
        self.id = 0;
    }

    fn search_first(&mut self) -> bool {
        let (dict, dec) = (&self.dict, &mut self.dec);

        if self.key.is_empty() {
            self.pos = dict.decode_header(0, dec);
            self.id = 0;
            return true;
        }

        let (bi, found) = dict.search_bucket(self.key);
        self.pos = dict.decode_header(bi, dec);
        self.id = bi * dict.bucket_size();

        if found || utils::is_prefix(self.key, dec) {
            return true;
        }

        for bj in 1..dict.bucket_size() {
            if self.pos == dict.serialized.len() {
                break;
            }

            let (lcp, next_pos) = dict.decode_lcp(self.pos);
            self.pos = next_pos;
            dec.resize(lcp, 0);
            self.pos = dict.decode_next(self.pos, dec);

            if utils::is_prefix(self.key, dec) {
                self.id += bj;
                return true;
            }
        }

        false
    }
}

impl<'a> Iterator for FcPrefixIterator<'a> {
    type Item = (usize, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.dict.serialized.len() {
            return None;
        }

        if self.dec.is_empty() {
            if !self.search_first() {
                self.dec.clear();
                self.pos = self.dict.serialized.len();
                self.id = 0;
                return None;
            }
        } else {
            self.id += 1;
            if self.dict.pos_in_bucket(self.id) == 0 {
                self.dec.clear();
            } else {
                let (lcp, next_pos) = self.dict.decode_lcp(self.pos);
                self.pos = next_pos;
                self.dec.resize(lcp, 0);
            }
            self.pos = self.dict.decode_next(self.pos, &mut self.dec);
        }

        if utils::is_prefix(self.key, &self.dec) {
            Some((self.id, self.dec.clone()))
        } else {
            self.dec.clear();
            self.pos = self.dict.serialized.len();
            self.id = 0;
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.dict.num_keys()))
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

        let dict = FcDict::from_builder(builder);

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
        let dict = FcDict::from_builder(builder);

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
