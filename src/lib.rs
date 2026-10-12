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
use std::convert::TryFrom;
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

/// Default upper bound for the packed pointers and keys allocated while deserializing a set.
pub const DEFAULT_MAX_DESERIALIZED_BYTES: usize = 256 * 1024 * 1024;

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
    /// An empty iterator creates an empty set.
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
    /// An empty iterator creates an empty set.
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
    /// This method limits allocations to [`DEFAULT_MAX_DESERIALIZED_BYTES`].
    /// Use [`Set::deserialize_from_with_limit`] to choose a different limit.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is truncated, exceeds the allocation
    /// limit, or contains inconsistent metadata or front-coded keys.
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
    pub fn deserialize_from<R>(reader: R) -> Result<Self>
    where
        R: io::Read,
    {
        Self::deserialize_from_with_limit(reader, DEFAULT_MAX_DESERIALIZED_BYTES)
    }

    /// Deserializes the dictionary from a reader with an allocation limit.
    ///
    /// The limit applies to the combined byte size of the packed pointers and
    /// serialized keys. Invalid metadata and structures are rejected before a
    /// [`Set`] is returned.
    ///
    /// # Arguments
    ///
    ///  - `reader`: Readable stream.
    ///  - `max_bytes`: Maximum combined allocation for packed pointers and keys.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is truncated, exceeds `max_bytes`, or
    /// contains inconsistent metadata or front-coded keys.
    pub fn deserialize_from_with_limit<R>(mut reader: R, max_bytes: usize) -> Result<Self>
    where
        R: io::Read,
    {
        let cookie = reader.read_u32::<LittleEndian>()?;
        if cookie != SERIAL_COOKIE {
            return Err(anyhow!("unknown cookie value"));
        }
        let pointers = IntVector::deserialize_from_with_limit(&mut reader, max_bytes)?;
        let remaining_bytes = max_bytes
            .checked_sub(pointers.allocated_bytes())
            .ok_or_else(|| anyhow!("pointer allocation exceeds the deserialization limit"))?;
        let serialized = {
            let len = usize::try_from(reader.read_u64::<LittleEndian>()?)
                .map_err(|_| anyhow!("serialized length does not fit in usize"))?;
            if len > remaining_bytes {
                return Err(anyhow!(
                    "serialized allocation exceeds the deserialization limit"
                ));
            }
            let mut serialized = Vec::new();
            serialized
                .try_reserve_exact(len)
                .map_err(|_| anyhow!("serialized allocation is too large"))?;
            serialized.resize(len, 0);
            reader.read_exact(&mut serialized)?;
            serialized
        };

        let len = Self::read_usize(&mut reader, "key count")?;
        let bucket_bits = Self::read_usize(&mut reader, "bucket bit width")?;
        let bucket_mask = Self::read_usize(&mut reader, "bucket mask")?;
        let max_length = Self::read_usize(&mut reader, "maximum key length")?;

        let set = Self {
            pointers,
            serialized,
            len,
            bucket_bits,
            bucket_mask,
            max_length,
        };
        set.validate_deserialized()?;
        Ok(set)
    }

    fn read_usize<R: io::Read>(reader: &mut R, field: &str) -> Result<usize> {
        usize::try_from(reader.read_u64::<LittleEndian>()?)
            .map_err(|_| anyhow!("{} does not fit in usize", field))
    }

    fn validate_deserialized(&self) -> Result<()> {
        let bucket_size = self
            .bucket_mask
            .checked_add(1)
            .ok_or_else(|| anyhow!("bucket size overflow"))?;
        if !utils::is_power_of_two(bucket_size) {
            return Err(anyhow!("bucket size must be a power of two"));
        }
        if self.bucket_bits != utils::needed_bits(self.bucket_mask as u64) {
            return Err(anyhow!("bucket bit width does not match bucket mask"));
        }

        let expected_buckets = if self.len == 0 {
            0
        } else {
            1 + (self.len - 1) / bucket_size
        };
        if self.pointers.len() != expected_buckets {
            return Err(anyhow!(
                "pointer count does not match key and bucket counts"
            ));
        }

        let mut pos = 0;
        let mut key_length = 0_usize;
        let mut actual_max_length = 0;
        for id in 0..self.len {
            if id % bucket_size == 0 {
                let bucket = id / bucket_size;
                let pointer = usize::try_from(self.pointers.get(bucket))
                    .map_err(|_| anyhow!("bucket pointer does not fit in usize"))?;
                if pointer != pos {
                    return Err(anyhow!(
                        "bucket pointers must match increasing bucket offsets"
                    ));
                }
                key_length = 0;
            } else {
                let lcp = Self::read_vbyte(&self.serialized, &mut pos)?;
                if lcp > key_length {
                    return Err(anyhow!(
                        "front-coded prefix exceeds the previous key length"
                    ));
                }
                key_length = lcp;
            }

            let suffix = self
                .serialized
                .get(pos..)
                .ok_or_else(|| anyhow!("key offset is outside the serialized data"))?;
            let suffix_len = suffix
                .iter()
                .position(|&byte| byte == END_MARKER)
                .ok_or_else(|| anyhow!("key is missing its end marker"))?;
            key_length = key_length
                .checked_add(suffix_len)
                .ok_or_else(|| anyhow!("decoded key length overflow"))?;
            pos = pos
                .checked_add(suffix_len + 1)
                .ok_or_else(|| anyhow!("serialized key offset overflow"))?;
            actual_max_length = actual_max_length.max(key_length);
        }

        if pos != self.serialized.len() {
            return Err(anyhow!(
                "serialized data contains trailing or unparsed bytes"
            ));
        }
        if self.max_length != actual_max_length {
            return Err(anyhow!("maximum key length does not match serialized keys"));
        }
        Ok(())
    }

    fn read_vbyte(bytes: &[u8], pos: &mut usize) -> Result<usize> {
        let mut value = 0_usize;
        let mut shift = 0_u32;
        loop {
            let byte = *bytes
                .get(*pos)
                .ok_or_else(|| anyhow!("unterminated vbyte value"))?;
            *pos += 1;
            let payload = usize::from(byte & 0x7f);
            if shift >= usize::BITS || payload > (usize::MAX >> shift) {
                return Err(anyhow!("vbyte value does not fit in usize"));
            }
            value |= payload << shift;

            if byte & 0x80 == 0 {
                return Ok(value);
            }
            shift += 7;
        }
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
    pub fn locator(&self) -> Locator<'_> {
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
    pub fn decoder(&self) -> Decoder<'_> {
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
    pub fn iter(&self) -> Iter<'_> {
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
    pub fn predictive_iter<P>(&self, prefix: P) -> PredictiveIter<'_>
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
    use byteorder::{LittleEndian, WriteBytesExt};
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

    fn serialize_raw_set(
        pointers: &[u64],
        serialized: &[u8],
        len: usize,
        bucket_bits: usize,
        bucket_mask: usize,
        max_length: usize,
    ) -> Vec<u8> {
        let mut data = Vec::new();
        data.write_u32::<LittleEndian>(SERIAL_COOKIE).unwrap();
        IntVector::build(pointers)
            .serialize_into(&mut data)
            .unwrap();
        data.write_u64::<LittleEndian>(serialized.len() as u64)
            .unwrap();
        data.extend_from_slice(serialized);
        data.write_u64::<LittleEndian>(len as u64).unwrap();
        data.write_u64::<LittleEndian>(bucket_bits as u64).unwrap();
        data.write_u64::<LittleEndian>(bucket_mask as u64).unwrap();
        data.write_u64::<LittleEndian>(max_length as u64).unwrap();
        data
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
        for (i, key) in keys.iter().enumerate() {
            let id = locator.run(key.as_bytes()).unwrap();
            assert_eq!(i, id);
        }
        assert!(locator.run("aaa".as_bytes()).is_none());
        assert!(locator.run("tell".as_bytes()).is_none());
        assert!(locator.run("techno".as_bytes()).is_none());
        assert!(locator.run("zzz".as_bytes()).is_none());

        let mut decoder = set.decoder();
        for (i, key) in keys.iter().enumerate() {
            assert_eq!(key.as_bytes(), &decoder.run(i));
        }

        let mut iterator = set.iter();
        for (i, key) in keys.iter().enumerate() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(key.as_bytes(), &dec);
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
        for (i, key) in keys.iter().enumerate() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(key.as_bytes(), &dec);
        }
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_empty() {
        let set = Set::new(Vec::<Vec<u8>>::new()).unwrap();

        assert!(set.is_empty());
        assert_eq!(set.len(), 0);
        assert_eq!(set.num_buckets(), 0);
        assert_eq!(set.bucket_size(), DEFAULT_BUCKET_SIZE);
        assert!(set.locator().run(b"key").is_none());
        assert!(set.iter().next().is_none());
        assert!(set.predictive_iter(b"").next().is_none());
        assert!(set.predictive_iter(b"prefix").next().is_none());

        let mut buffer = Vec::new();
        set.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), set.size_in_bytes());

        let other = Set::deserialize_from(&buffer[..]).unwrap();
        assert!(other.is_empty());
        assert_eq!(other.len(), 0);
        assert_eq!(other.num_buckets(), 0);
        assert_eq!(other.bucket_size(), DEFAULT_BUCKET_SIZE);
        assert!(other.locator().run(b"key").is_none());
        assert!(other.iter().next().is_none());
        assert!(other.predictive_iter(b"").next().is_none());
        assert!(other.predictive_iter(b"prefix").next().is_none());
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
        for (i, key) in keys.iter().enumerate() {
            let id = locator.run(key).unwrap();
            assert_eq!(i, id);
        }

        let mut decoder = set.decoder();
        for (i, key) in keys.iter().enumerate() {
            let dec = decoder.run(i);
            assert_eq!(key, &dec);
        }

        let mut iterator = set.iter();
        for (i, key) in keys.iter().enumerate() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(key, &dec);
        }
        assert!(iterator.next().is_none());

        let mut buffer = vec![];
        set.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), set.size_in_bytes());

        let other = Set::deserialize_from(&buffer[..]).unwrap();
        let mut iterator = other.iter();
        for (i, key) in keys.iter().enumerate() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(key, &dec);
        }
        assert!(iterator.next().is_none());
    }

    #[test]
    fn test_deserialize_rejects_excessive_allocations() {
        let mut data = Vec::new();
        data.write_u32::<LittleEndian>(SERIAL_COOKIE).unwrap();
        data.write_u64::<LittleEndian>(
            (DEFAULT_MAX_DESERIALIZED_BYTES / std::mem::size_of::<u64>() + 1) as u64,
        )
        .unwrap();
        assert!(Set::deserialize_from(&data[..]).is_err());

        let data = serialize_raw_set(&[0], b"a\0", 1, 3, 7, 1);
        assert!(Set::deserialize_from_with_limit(&data[..], 9).is_err());
        assert!(Set::deserialize_from_with_limit(&data[..], 10).is_ok());
    }

    #[test]
    fn test_deserialize_rejects_invalid_int_vector() {
        let mut data = serialize_raw_set(&[0], b"a\0", 1, 3, 7, 1);
        // cookie + chunk count + one chunk + pointer count
        data[28..36].copy_from_slice(&0_u64.to_le_bytes());
        assert!(Set::deserialize_from(&data[..]).is_err());

        let mut data = serialize_raw_set(&[0], b"a\0", 1, 3, 7, 1);
        // 65 one-bit values require two chunks, but only one is serialized.
        data[20..28].copy_from_slice(&65_u64.to_le_bytes());
        assert!(Set::deserialize_from(&data[..]).is_err());
    }

    #[test]
    fn test_deserialize_rejects_invalid_set_structure() {
        let malformed = [
            serialize_raw_set(&[0], b"a\0", 1, 2, 2, 1),
            serialize_raw_set(&[], b"a\0", 1, 3, 7, 1),
            serialize_raw_set(&[1], b"a\0", 1, 3, 7, 1),
            serialize_raw_set(&[0], b"a", 1, 3, 7, 1),
            serialize_raw_set(&[0], b"a\0\x80", 2, 3, 7, 1),
            serialize_raw_set(&[0], b"a\0\x02b\0", 2, 3, 7, 1),
            serialize_raw_set(
                &[0],
                b"a\0\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\x01\0",
                2,
                3,
                7,
                1,
            ),
            serialize_raw_set(&[0], b"a\0x", 1, 3, 7, 1),
            serialize_raw_set(&[0], b"a\0", 1, 3, 7, 2),
        ];
        for data in malformed {
            assert!(Set::deserialize_from(&data[..]).is_err());
        }
    }

    #[test]
    fn test_deserialize_accepts_noncanonical_vbyte() {
        let data = serialize_raw_set(&[0], b"a\0\x80\x00b\0", 2, 3, 7, 1);
        assert!(Set::deserialize_from(&data[..]).is_ok());
    }

    #[test]
    fn test_deserialize_rejects_every_truncation() {
        let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
        let mut data = Vec::new();
        set.serialize_into(&mut data).unwrap();
        for end in 0..data.len() {
            assert!(Set::deserialize_from(&data[..end]).is_err(), "end={}", end);
        }
        assert!(Set::deserialize_from(&data[..]).is_ok());
    }
}
