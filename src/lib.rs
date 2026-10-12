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
pub mod errors;
mod intvec;
pub mod iter;
pub mod locator;
pub mod predictive_iter;
mod utils;

use std::cmp::Ordering;
use std::convert::TryFrom;
use std::io::{self, Read};

use builder::Builder;
use decoder::Decoder;
pub use errors::{FcsdError, Result};
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
        utils::write_u32_le(&mut writer, SERIAL_COOKIE)?;
        self.pointers.serialize_into(&mut writer)?;
        utils::write_u64_le(&mut writer, self.serialized.len() as u64)?;
        writer.write_all(&self.serialized)?;
        utils::write_u64_le(&mut writer, self.len as u64)?;
        utils::write_u64_le(&mut writer, self.bucket_bits as u64)?;
        utils::write_u64_le(&mut writer, self.bucket_mask as u64)?;
        utils::write_u64_le(&mut writer, self.max_length as u64)?;
        Ok(())
    }

    /// Deserializes the dictionary from a reader.
    ///
    /// # Arguments
    ///
    ///  - `reader`: Readable stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is truncated or contains inconsistent
    /// metadata or front-coded keys.
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
        Self::deserialize_from_with_limit(reader, usize::MAX)
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
        let cookie = utils::read_u32_le(&mut reader)?;
        if cookie != SERIAL_COOKIE {
            return Err(FcsdError::InvalidData("unknown cookie value"));
        }
        let pointers = IntVector::deserialize_from_with_limit(&mut reader, max_bytes)?;
        let remaining_bytes = max_bytes - pointers.allocated_bytes();
        let serialized = {
            let len = Self::read_usize(&mut reader, "serialized length does not fit in usize")?;
            if len > remaining_bytes {
                return Err(FcsdError::InvalidData(
                    "serialized allocation exceeds the deserialization limit",
                ));
            }
            let mut serialized = Vec::new();
            reader
                .by_ref()
                .take(len as u64)
                .read_to_end(&mut serialized)?;
            if serialized.len() != len {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
            serialized
        };

        let len = Self::read_usize(&mut reader, "key count does not fit in usize")?;
        let bucket_bits = Self::read_usize(&mut reader, "bucket bit width does not fit in usize")?;
        let bucket_mask = Self::read_usize(&mut reader, "bucket mask does not fit in usize")?;
        let max_length = Self::read_usize(&mut reader, "maximum key length does not fit in usize")?;

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

    fn read_usize<R: io::Read>(reader: &mut R, error: &'static str) -> Result<usize> {
        usize::try_from(utils::read_u64_le(reader)?).map_err(|_| FcsdError::InvalidData(error))
    }

    fn validate_deserialized(&self) -> Result<()> {
        let bucket_size = self
            .bucket_mask
            .checked_add(1)
            .ok_or(FcsdError::InvalidData("bucket size overflow"))?;
        if !utils::is_power_of_two(bucket_size) {
            return Err(FcsdError::InvalidData("bucket size must be a power of two"));
        }
        if self.bucket_bits != self.bucket_mask.count_ones() as usize {
            return Err(FcsdError::InvalidData(
                "bucket bit width does not match bucket mask",
            ));
        }

        let expected_buckets = if self.len == 0 {
            0
        } else {
            1 + (self.len - 1) / bucket_size
        };
        if self.pointers.len() != expected_buckets {
            return Err(FcsdError::InvalidData(
                "pointer count does not match key and bucket counts",
            ));
        }

        let mut pos = 0;
        let mut previous_key = Vec::new();
        let mut key = Vec::new();
        let mut actual_max_length = 0;
        for id in 0..self.len {
            let lcp = if id % bucket_size == 0 {
                let bucket = id / bucket_size;
                let pointer = usize::try_from(self.pointers.get(bucket))
                    .map_err(|_| FcsdError::InvalidData("bucket pointer does not fit in usize"))?;
                if pointer != pos {
                    return Err(FcsdError::InvalidData(
                        "bucket pointers must match increasing bucket offsets",
                    ));
                }
                0
            } else {
                let (lcp, bytes) = utils::vbyte::try_decode(&self.serialized[pos..])
                    .ok_or(FcsdError::InvalidData("invalid vbyte value"))?;
                pos += bytes;
                if lcp > previous_key.len() {
                    return Err(FcsdError::InvalidData(
                        "front-coded prefix exceeds the previous key length",
                    ));
                }
                lcp
            };

            let suffix = self.serialized.get(pos..).ok_or(FcsdError::InvalidData(
                "key offset is outside the serialized data",
            ))?;
            let suffix_len = suffix
                .iter()
                .position(|&byte| byte == END_MARKER)
                .ok_or(FcsdError::InvalidData("key is missing its end marker"))?;
            if suffix_len == 0 {
                return Err(FcsdError::InvalidData("keys must not be empty"));
            }

            key.clear();
            key.extend_from_slice(&previous_key[..lcp]);
            key.extend_from_slice(&suffix[..suffix_len]);
            if id != 0 && key <= previous_key {
                return Err(FcsdError::InvalidData("keys must be sorted and unique"));
            }

            pos += suffix_len + 1;
            actual_max_length = actual_max_length.max(key.len());
            std::mem::swap(&mut previous_key, &mut key);
        }

        if pos != self.serialized.len() {
            return Err(FcsdError::InvalidData(
                "serialized data contains trailing or unparsed bytes",
            ));
        }
        if self.max_length != actual_max_length {
            return Err(FcsdError::InvalidData(
                "maximum key length does not match serialized keys",
            ));
        }
        Ok(())
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
