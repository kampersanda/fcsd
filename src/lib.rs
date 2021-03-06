//! # Fast and compact indexed string set using front coding
//!
//! This crate provides an indexed set of strings in a compressed format based on front coding.
//! `n` strings in the set are indexed with integers from `[0..n-1]` and assigned in the lexicographical order.
//!
//! ## Supported queries
//!
//!  - `Locate` gets the index of a string key.
//!  - `Decode` gets the string with an index.
//!  - `Predict` enumerates the strings starting from a prefix.
//!
//! ## References
//!
//!  - Martínez-Prieto et al., [Practical compressed string dictionaries](https://doi.org/10.1016/j.is.2015.08.008), INFOSYS 2016
pub mod builder;
pub mod decoder;
mod intvec;
pub mod iter;
pub mod locator;
pub mod predictive_iter;
mod utils;

use std::cmp::Ordering;
use std::io;

use anyhow::{anyhow, Result};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};

use builder::Builder;
use decoder::Decoder;
use intvec::IntVector;
use iter::Iter;
use locator::Locator;
use predictive_iter::PredictiveIter;

/// Special terminator, which must not be contained in stored keys.
pub const END_MARKER: u8 = 0;

/// Default parameter for the number of keys in each bucket.
pub const DEFAULT_BUCKET_SIZE: usize = 8;

/// Serial cookie value for serialization.
const SERIAL_COOKIE: u32 = 114514;

/// Fast and compact indexed string set using front coding.
///
/// This implements an indexed set of strings in a compressed format based on front coding.
/// `n` strings in the set are indexed with integers from `[0..n-1]` and assigned in the lexicographical order.
///
/// ## Supported queries
///
///  - `Locate` gets the index of a string key.
///  - `Decode` gets the string with an index.
///  - `Predict` enumerates the strings starting from a prefix.
///
/// ## Limitations
///
/// Input keys must not contain `\0` character because the character is used for the terminator.
///
/// # Example
///
/// ```
/// use fcsd::Set;
///
/// // Input string keys should be sorted and unique.
/// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
///
/// // Builds an indexed set.
/// let set = Set::new(keys).unwrap();
/// assert_eq!(set.len(), keys.len());
///
/// // Gets indexes associated with given keys.
/// let mut locator = set.locator();
/// assert_eq!(locator.run(b"ICML"), Some(1));
/// assert_eq!(locator.run(b"SIGMOD"), Some(4));
/// assert_eq!(locator.run(b"SIGSPATIAL"), None);
///
/// // Decodes string keys from given indexes.
/// let mut decoder = set.decoder();
/// assert_eq!(decoder.run(0), b"ICDM".to_vec());
/// assert_eq!(decoder.run(3), b"SIGKDD".to_vec());
///
/// // Enumerates indexes and keys stored in the set.
/// let mut iter = set.iter();
/// assert_eq!(iter.next(), Some((0, b"ICDM".to_vec())));
/// assert_eq!(iter.next(), Some((1, b"ICML".to_vec())));
/// assert_eq!(iter.next(), Some((2, b"SIGIR".to_vec())));
/// assert_eq!(iter.next(), Some((3, b"SIGKDD".to_vec())));
/// assert_eq!(iter.next(), Some((4, b"SIGMOD".to_vec())));
/// assert_eq!(iter.next(), None);
///
/// // Enumerates indexes and keys starting with a prefix.
/// let mut iter = set.predictive_iter(b"SIG");
/// assert_eq!(iter.next(), Some((2, b"SIGIR".to_vec())));
/// assert_eq!(iter.next(), Some((3, b"SIGKDD".to_vec())));
/// assert_eq!(iter.next(), Some((4, b"SIGMOD".to_vec())));
/// assert_eq!(iter.next(), None);
///
/// // Serialization / Deserialization
/// let mut data = Vec::<u8>::new();
/// set.serialize_into(&mut data).unwrap();
/// assert_eq!(data.len(), set.size_in_bytes());
/// let other = Set::deserialize_from(&data[..]).unwrap();
/// assert_eq!(data.len(), other.size_in_bytes());
/// ```
#[derive(Clone)]
pub struct Set {
    pointers: IntVector,
    serialized: Vec<u8>,
    len: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
}

impl Set {
    /// Builds a new [`Set`] from string keys.
    ///
    /// # Arguments
    ///
    ///  - `keys`: string keys that are unique and sorted.
    ///
    /// # Notes
    ///
    /// It will set the bucket size to [`DEFAULT_BUCKET_SIZE`].
    /// If you want to optionally set the parameter, use [`Set::with_bucket_size`] instead.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    /// assert_eq!(set.len(), keys.len());
    /// ```
    pub fn new<I, P>(keys: I) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        Self::with_bucket_size(keys, DEFAULT_BUCKET_SIZE)
    }

    /// Builds a new [`Set`] from string keys with a specified bucket size.
    ///
    /// # Arguments
    ///
    ///  - `keys`: string keys that are unique and sorted.
    ///  - `bucket_size`: The number of strings in each bucket, which must be a power of two.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::with_bucket_size(keys, 4).unwrap();
    /// assert_eq!(set.len(), keys.len());
    /// ```
    pub fn with_bucket_size<I, P>(keys: I, bucket_size: usize) -> Result<Self>
    where
        I: IntoIterator<Item = P>,
        P: AsRef<[u8]>,
    {
        let mut builder = Builder::new(bucket_size)?;
        for key in keys {
            builder.add(key.as_ref())?;
        }
        Ok(builder.finish())
    }

    /// Returns the number of bytes needed to write the dictionary.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    /// assert_eq!(set.size_in_bytes(), 110);
    /// ```
    pub fn size_in_bytes(&self) -> usize {
        let mut bytes = 0;
        bytes += 4; // SERIAL_COOKIE
        bytes += self.pointers.size_in_bytes(); // pointers
        bytes += 8 + self.serialized.len(); // serialized
        bytes + 8 * 4
    }

    /// Serializes the dictionary into a writer.
    ///
    /// # Arguments
    ///
    ///  - `writer`: Writable stream.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    ///
    /// let mut data = Vec::<u8>::new();
    /// set.serialize_into(&mut data).unwrap();
    /// assert_eq!(data.len(), 110);
    /// ```
    pub fn serialize_into<W>(&self, mut writer: W) -> Result<()>
    where
        W: io::Write,
    {
        writer.write_u32::<LittleEndian>(SERIAL_COOKIE)?;
        self.pointers.serialize_into(&mut writer)?;
        writer.write_u64::<LittleEndian>(self.serialized.len() as u64)?;
        for &x in &self.serialized {
            writer.write_u8(x)?;
        }
        writer.write_u64::<LittleEndian>(self.len as u64)?;
        writer.write_u64::<LittleEndian>(self.bucket_bits as u64)?;
        writer.write_u64::<LittleEndian>(self.bucket_mask as u64)?;
        writer.write_u64::<LittleEndian>(self.max_length as u64)?;
        Ok(())
    }

    /// Deserializes the dictionary from a reader.
    ///
    /// # Arguments
    ///
    ///  - `reader`: Readable stream.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    ///
    /// let mut data = Vec::<u8>::new();
    /// set.serialize_into(&mut data).unwrap();
    /// let other = Set::deserialize_from(&data[..]).unwrap();
    /// assert_eq!(set.size_in_bytes(), other.size_in_bytes());
    /// ```
    pub fn deserialize_from<R>(mut reader: R) -> Result<Self>
    where
        R: io::Read,
    {
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

        let len = reader.read_u64::<LittleEndian>()? as usize;
        let bucket_bits = reader.read_u64::<LittleEndian>()? as usize;
        let bucket_mask = reader.read_u64::<LittleEndian>()? as usize;
        let max_length = reader.read_u64::<LittleEndian>()? as usize;

        Ok(Self {
            pointers,
            serialized,
            len,
            bucket_bits,
            bucket_mask,
            max_length,
        })
    }

    /// Makes a class to get ids of given string keys.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    ///
    /// let mut locator = set.locator();
    /// assert_eq!(locator.run(b"ICML"), Some(1));
    /// assert_eq!(locator.run(b"SIGMOD"), Some(4));
    /// assert_eq!(locator.run(b"SIGSPATIAL"), None);
    /// ```
    pub fn locator(&self) -> Locator {
        Locator::new(self)
    }

    /// Makes a class to decode stored keys associated with given ids.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    ///
    /// let mut decoder = set.decoder();
    /// assert_eq!(decoder.run(0), b"ICDM".to_vec());
    /// assert_eq!(decoder.run(3), b"SIGKDD".to_vec());
    /// ```
    pub fn decoder(&self) -> Decoder {
        Decoder::new(self)
    }

    /// Makes an iterator to enumerate keys stored in the dictionary.
    ///
    /// The keys will be reported in the lexicographical order.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR"];
    /// let set = Set::new(keys).unwrap();
    ///
    /// let mut iter = set.iter();
    /// assert_eq!(iter.next(), Some((0, b"ICDM".to_vec())));
    /// assert_eq!(iter.next(), Some((1, b"ICML".to_vec())));
    /// assert_eq!(iter.next(), Some((2, b"SIGIR".to_vec())));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn iter(&self) -> Iter {
        Iter::new(self)
    }

    /// Makes a predictive iterator to enumerate keys starting from a given string.
    ///
    /// The keys will be reported in the lexicographical order.
    ///
    /// # Arguments
    ///
    ///  - `prefix`: Prefix of keys to be predicted.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    ///
    /// let mut iter = set.predictive_iter(b"SIG");
    /// assert_eq!(iter.next(), Some((2, b"SIGIR".to_vec())));
    /// assert_eq!(iter.next(), Some((3, b"SIGKDD".to_vec())));
    /// assert_eq!(iter.next(), Some((4, b"SIGMOD".to_vec())));
    /// assert_eq!(iter.next(), None);
    /// ```
    pub fn predictive_iter<P>(&self, prefix: P) -> PredictiveIter
    where
        P: AsRef<[u8]>,
    {
        PredictiveIter::new(self, prefix)
    }

    /// Gets the number of stored keys.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    /// assert_eq!(set.len(), keys.len());
    /// ```
    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Checks if the set is empty.
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Gets the number of defined buckets.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::with_bucket_size(keys, 4).unwrap();
    /// assert_eq!(set.num_buckets(), 2);
    /// ```
    #[inline(always)]
    pub const fn num_buckets(&self) -> usize {
        self.pointers.len()
    }

    /// Gets the bucket size.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::with_bucket_size(keys, 4).unwrap();
    /// assert_eq!(set.bucket_size(), 4);
    /// ```
    #[inline(always)]
    pub const fn bucket_size(&self) -> usize {
        self.bucket_mask + 1
    }

    #[inline(always)]
    const fn max_length(&self) -> usize {
        self.max_length
    }

    #[inline(always)]
    const fn bucket_id(&self, id: usize) -> usize {
        id >> self.bucket_bits
    }

    #[inline(always)]
    const fn pos_in_bucket(&self, id: usize) -> usize {
        id & self.bucket_mask
    }

    #[inline(always)]
    fn get_header(&self, bi: usize) -> &[u8] {
        let header = &self.serialized[self.pointers.get(bi) as usize..];
        &header[..utils::get_strlen(header)]
    }

    #[inline(always)]
    fn decode_header(&self, bi: usize, dec: &mut Vec<u8>) -> usize {
        dec.clear();
        let mut pos = self.pointers.get(bi) as usize;
        while self.serialized[pos] != END_MARKER {
            dec.push(self.serialized[pos]);
            pos += 1;
        }
        pos + 1
    }

    #[inline(always)]
    fn decode_lcp(&self, pos: usize) -> (usize, usize) {
        let (lcp, num) = utils::vbyte::decode(&self.serialized[pos..]);
        (lcp, pos + num)
    }

    #[inline(always)]
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

        assert!(Builder::new(0).is_err());
        assert!(Builder::new(3).is_err());
        let mut builder = Builder::new(4).unwrap();

        for &key in &keys {
            builder.add(key.as_bytes()).unwrap();
        }
        assert!(builder.add("tri".as_bytes()).is_err());
        assert!(builder.add(&[0xFF, 0x00]).is_err());

        let set = builder.finish();

        let mut locator = set.locator();
        for i in 0..keys.len() {
            let id = locator.run(keys[i].as_bytes()).unwrap();
            assert_eq!(i, id);
        }
        assert!(locator.run("aaa".as_bytes()).is_none());
        assert!(locator.run("tell".as_bytes()).is_none());
        assert!(locator.run("techno".as_bytes()).is_none());
        assert!(locator.run("zzz".as_bytes()).is_none());

        let mut decoder = set.decoder();
        for i in 0..keys.len() {
            assert_eq!(keys[i].as_bytes(), &decoder.run(i));
        }

        let mut iterator = set.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(keys[i].as_bytes(), &dec);
        }
        assert!(iterator.next().is_none());

        let mut iterator = set.predictive_iter("idea".as_bytes());
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
        set.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), set.size_in_bytes());

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
        let mut builder = Builder::new(8).unwrap();

        for key in &keys {
            builder.add(key).unwrap();
        }
        let set = builder.finish();

        let mut locator = set.locator();
        for i in 0..keys.len() {
            let id = locator.run(&keys[i]).unwrap();
            assert_eq!(i, id);
        }

        let mut decoder = set.decoder();
        for i in 0..keys.len() {
            let dec = decoder.run(i);
            assert_eq!(&keys[i], &dec);
        }

        let mut iterator = set.iter();
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(&keys[i], &dec);
        }
        assert!(iterator.next().is_none());

        let mut buffer = vec![];
        set.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), set.size_in_bytes());

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
