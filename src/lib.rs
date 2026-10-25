#![cfg_attr(not(feature = "std"), no_std)]
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
//! ## Zero-copy loading
//!
//! A [`Set`] is stored in its serialized form, so it can be queried directly
//! from any byte buffer, such as a memory-mapped file, with [`Set::from_bytes`].
//!
//! ## References
//!
//!  - Martínez-Prieto et al., [Practical compressed string dictionaries](https://doi.org/10.1016/j.is.2015.08.008), INFOSYS 2016

extern crate alloc;

#[cfg(test)]
extern crate std;

pub mod builder;
pub mod decoder;
pub mod errors;
mod intvec;
pub mod iter;
pub mod locator;
pub mod predictive_iter;
mod utils;

use alloc::vec::Vec;
use core::cmp::Ordering;
use core::convert::TryFrom;
#[cfg(feature = "std")]
use std::io::{self, Read};

use builder::Builder;
use decoder::Decoder;
pub use errors::{FcsdError, Result};
use intvec::IntVector;
use iter::Iter;
use locator::Locator;
use predictive_iter::PredictiveIter;
use utils::ByteReader;

/// Special terminator, which must not be contained in stored keys.
pub const END_MARKER: u8 = 0;

/// Default parameter for the number of keys in each bucket.
pub const DEFAULT_BUCKET_SIZE: usize = 8;

/// Magic bytes at the beginning of serialized data.
const MAGIC: [u8; 4] = *b"FCSD";

/// Version of the serialization format.
///
/// This is independent of the crate version and is incremented only when the
/// serialized layout changes.
pub const FORMAT_VERSION: u32 = 1;

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
/// ## Storage
///
/// The set is kept in its serialized form inside the byte container `S`.
/// It is [`Vec<u8>`] for a set built with [`Set::new`] or read with
/// [`Set::deserialize_from`]. [`Set::from_bytes`] accepts any container that
/// implements `AsRef<[u8]>`, such as a borrowed slice or a memory-mapped file,
/// and queries it without copying.
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
///
/// // Zero-copy loading from bytes
/// let view = Set::from_bytes(data.as_slice()).unwrap();
/// assert_eq!(view.locator().run(b"SIGKDD"), Some(3));
/// ```
#[derive(Clone)]
pub struct Set<S = Vec<u8>> {
    data: S,
    layout: Layout,
}

/// Positions and parameters parsed from serialized data.
#[derive(Clone, Copy)]
struct Layout {
    pointers: IntVector,
    keys_start: usize,
    keys_end: usize,
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

    /// Deserializes the dictionary from a reader.
    ///
    /// # Arguments
    ///
    ///  - `reader`: Readable stream.
    ///
    /// # Errors
    ///
    /// Returns an error if the input is truncated, was written in an
    /// unsupported format version, or contains inconsistent metadata or
    /// front-coded keys.
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
    #[cfg(feature = "std")]
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
    /// Returns an error if the input is truncated, was written in an
    /// unsupported format version, exceeds `max_bytes`, or contains
    /// inconsistent metadata or front-coded keys.
    #[cfg(feature = "std")]
    pub fn deserialize_from_with_limit<R>(mut reader: R, max_bytes: usize) -> Result<Self>
    where
        R: io::Read,
    {
        let mut data = Vec::new();
        read_into(&mut reader, MAGIC.len() + 4, &mut data)?;
        read_header(&mut ByteReader::new(&data))?;

        let pointers_bytes = read_usize_into(&mut reader, &mut data)?
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(FcsdError::InvalidData("chunk allocation size overflow"))?;
        if pointers_bytes > max_bytes {
            return Err(FcsdError::InvalidData(
                "chunk allocation exceeds the deserialization limit",
            ));
        }
        read_into(&mut reader, pointers_bytes, &mut data)?;
        read_into(&mut reader, 8 * 3, &mut data)?;

        let keys_bytes = read_usize_into(&mut reader, &mut data)?;
        if keys_bytes > max_bytes - pointers_bytes {
            return Err(FcsdError::InvalidData(
                "serialized allocation exceeds the deserialization limit",
            ));
        }
        read_into(&mut reader, keys_bytes, &mut data)?;
        read_into(&mut reader, 8 * 4, &mut data)?;

        Self::from_bytes(data)
    }
}

impl<S> Set<S>
where
    S: AsRef<[u8]>,
{
    /// Loads the dictionary from its serialized bytes without copying them.
    ///
    /// `data` must hold exactly the bytes written by [`Set::serialize_into`].
    /// Any byte container can be used, such as `&[u8]`, [`Vec<u8>`], or a
    /// memory-mapped file. No alignment is required.
    ///
    /// This validates every key, which reads the whole buffer. Use
    /// [`Set::from_bytes_trusted`] to skip it for data you produced yourself.
    ///
    /// # Arguments
    ///
    ///  - `data`: Serialized dictionary.
    ///
    /// # Errors
    ///
    /// Returns an error if `data` is truncated, has trailing bytes, was
    /// written in an unsupported format version, or contains inconsistent
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
    /// let view = Set::from_bytes(set.as_bytes()).unwrap();
    /// assert_eq!(view.locator().run(b"SIGIR"), Some(2));
    /// ```
    ///
    /// With a memory-mapped file, e.g., using the `memmap2` crate:
    ///
    /// ```ignore
    /// let file = std::fs::File::open("keys.fcsd")?;
    /// let mmap = unsafe { memmap2::Mmap::map(&file)? };
    /// let set = fcsd::Set::from_bytes(mmap)?;
    /// ```
    pub fn from_bytes(data: S) -> Result<Self> {
        let set = Self::from_bytes_trusted(data)?;
        set.view().validate_keys()?;
        Ok(set)
    }

    /// Loads the dictionary from its serialized bytes without copying them or
    /// validating the keys.
    ///
    /// This is the same as [`Set::from_bytes`] except that it only checks the
    /// header and metadata, so it runs in constant time and does not touch
    /// the key data. This is useful for memory-mapped files, whose pages are
    /// then loaded only when queries access them.
    ///
    /// If the key data is corrupted, queries may panic or return wrong
    /// results, but never cause undefined behavior.
    ///
    /// # Arguments
    ///
    ///  - `data`: Serialized dictionary.
    ///
    /// # Errors
    ///
    /// Returns an error if `data` is truncated, has trailing bytes, was
    /// written in an unsupported format version, or contains inconsistent
    /// metadata.
    pub fn from_bytes_trusted(data: S) -> Result<Self> {
        let layout = Layout::parse(data.as_ref())?;
        Ok(Self { data, layout })
    }

    /// Returns the serialized bytes of the dictionary.
    ///
    /// # Example
    ///
    /// ```
    /// use fcsd::Set;
    ///
    /// let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];
    /// let set = Set::new(keys).unwrap();
    /// assert_eq!(set.as_bytes().len(), set.size_in_bytes());
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        self.data.as_ref()
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
    /// assert_eq!(set.size_in_bytes(), 114);
    /// ```
    pub fn size_in_bytes(&self) -> usize {
        self.as_bytes().len()
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
    /// assert_eq!(data.len(), 114);
    /// ```
    #[cfg(feature = "std")]
    pub fn serialize_into<W>(&self, mut writer: W) -> Result<()>
    where
        W: io::Write,
    {
        writer.write_all(self.as_bytes())?;
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
        self.layout.len
    }

    /// Checks if the set is empty.
    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        self.layout.len == 0
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
        self.layout.pointers.len()
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
        self.layout.bucket_mask + 1
    }

    pub(crate) fn view(&self) -> SetRef<'_> {
        let data = self.as_bytes();
        SetRef {
            data,
            keys: &data[self.layout.keys_start..self.layout.keys_end],
            layout: self.layout,
        }
    }
}

/// Reads exactly `len` bytes from `reader` and appends them to `data`.
#[cfg(feature = "std")]
fn read_into<R: io::Read>(reader: &mut R, len: usize, data: &mut Vec<u8>) -> Result<()> {
    let start = data.len();
    reader.by_ref().take(len as u64).read_to_end(data)?;
    if data.len() - start != len {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    Ok(())
}

/// Reads a `u64` value from `reader`, appends its bytes to `data`, and returns it.
#[cfg(feature = "std")]
fn read_usize_into<R: io::Read>(reader: &mut R, data: &mut Vec<u8>) -> Result<usize> {
    read_into(reader, 8, data)?;
    ByteReader::new(&data[data.len() - 8..]).read_usize("length does not fit in usize")
}

fn read_header(reader: &mut ByteReader<'_>) -> Result<()> {
    if reader.take(MAGIC.len())? != MAGIC {
        return Err(FcsdError::InvalidData("unknown magic bytes"));
    }
    let version = reader.read_u32()?;
    if version != FORMAT_VERSION {
        return Err(FcsdError::UnsupportedFormatVersion(version));
    }
    Ok(())
}

/// Appends the serialized form of a dictionary to `data`.
pub(crate) fn write_set(
    data: &mut Vec<u8>,
    pointers: &[u64],
    keys: &[u8],
    len: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
) {
    data.extend_from_slice(&MAGIC);
    data.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    IntVector::write(pointers, data);
    data.extend_from_slice(&(keys.len() as u64).to_le_bytes());
    data.extend_from_slice(keys);
    for x in [len, bucket_bits, bucket_mask, max_length] {
        data.extend_from_slice(&(x as u64).to_le_bytes());
    }
}

impl Layout {
    fn parse(data: &[u8]) -> Result<Self> {
        let mut reader = ByteReader::new(data);
        read_header(&mut reader)?;
        let pointers = IntVector::read(&mut reader)?;
        let keys_len = reader.read_usize("serialized length does not fit in usize")?;
        let keys_start = reader.pos();
        reader.take(keys_len)?;
        let keys_end = reader.pos();
        let len = reader.read_usize("key count does not fit in usize")?;
        let bucket_bits = reader.read_usize("bucket bit width does not fit in usize")?;
        let bucket_mask = reader.read_usize("bucket mask does not fit in usize")?;
        let max_length = reader.read_usize("maximum key length does not fit in usize")?;
        if !reader.is_empty() {
            return Err(FcsdError::InvalidData(
                "unexpected bytes after the serialized dictionary",
            ));
        }

        let layout = Self {
            pointers,
            keys_start,
            keys_end,
            len,
            bucket_bits,
            bucket_mask,
            max_length,
        };
        layout.validate()?;
        Ok(layout)
    }

    fn validate(&self) -> Result<()> {
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
        Ok(())
    }
}

/// Borrowed form of a [`Set`] on which queries run.
#[derive(Clone, Copy)]
pub(crate) struct SetRef<'a> {
    data: &'a [u8],
    pub(crate) keys: &'a [u8],
    layout: Layout,
}

impl SetRef<'_> {
    fn validate_keys(&self) -> Result<()> {
        let bucket_size = self.bucket_size();
        let mut pos = 0;
        let mut previous_key = Vec::new();
        let mut key = Vec::new();
        let mut actual_max_length = 0;
        for id in 0..self.len() {
            let lcp = if id % bucket_size == 0 {
                let bucket = id / bucket_size;
                let pointer = usize::try_from(self.layout.pointers.get(self.data, bucket))
                    .map_err(|_| FcsdError::InvalidData("bucket pointer does not fit in usize"))?;
                if pointer != pos {
                    return Err(FcsdError::InvalidData(
                        "bucket pointers must match increasing bucket offsets",
                    ));
                }
                0
            } else {
                let (lcp, bytes) = utils::vbyte::try_decode(&self.keys[pos..])
                    .ok_or(FcsdError::InvalidData("invalid vbyte value"))?;
                pos += bytes;
                if lcp > previous_key.len() {
                    return Err(FcsdError::InvalidData(
                        "front-coded prefix exceeds the previous key length",
                    ));
                }
                lcp
            };

            let suffix = self.keys.get(pos..).ok_or(FcsdError::InvalidData(
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
            core::mem::swap(&mut previous_key, &mut key);
        }

        if pos != self.keys.len() {
            return Err(FcsdError::InvalidData(
                "serialized data contains trailing or unparsed bytes",
            ));
        }
        if self.layout.max_length != actual_max_length {
            return Err(FcsdError::InvalidData(
                "maximum key length does not match serialized keys",
            ));
        }
        Ok(())
    }

    #[inline(always)]
    pub(crate) const fn len(&self) -> usize {
        self.layout.len
    }

    #[inline(always)]
    pub(crate) const fn is_empty(&self) -> bool {
        self.layout.len == 0
    }

    #[inline(always)]
    pub(crate) const fn bucket_size(&self) -> usize {
        self.layout.bucket_mask + 1
    }

    #[inline(always)]
    const fn num_buckets(&self) -> usize {
        self.layout.pointers.len()
    }

    #[inline(always)]
    pub(crate) const fn max_length(&self) -> usize {
        self.layout.max_length
    }

    #[inline(always)]
    pub(crate) const fn bucket_id(&self, id: usize) -> usize {
        id >> self.layout.bucket_bits
    }

    #[inline(always)]
    pub(crate) const fn pos_in_bucket(&self, id: usize) -> usize {
        id & self.layout.bucket_mask
    }

    #[inline(always)]
    fn pointer(&self, bi: usize) -> usize {
        self.layout.pointers.get(self.data, bi) as usize
    }

    #[inline(always)]
    fn get_header(&self, bi: usize) -> &[u8] {
        let header = &self.keys[self.pointer(bi)..];
        &header[..utils::get_strlen(header)]
    }

    #[inline(always)]
    pub(crate) fn decode_header(&self, bi: usize, dec: &mut Vec<u8>) -> usize {
        dec.clear();
        self.decode_next(self.pointer(bi), dec)
    }

    #[inline(always)]
    pub(crate) fn decode_lcp(&self, pos: usize) -> (usize, usize) {
        let (lcp, num) = utils::vbyte::decode(&self.keys[pos..]);
        (lcp, pos + num)
    }

    #[inline(always)]
    pub(crate) fn decode_next(&self, mut pos: usize, dec: &mut Vec<u8>) -> usize {
        while self.keys[pos] != END_MARKER {
            dec.push(self.keys[pos]);
            pos += 1;
        }
        pos + 1
    }

    pub(crate) fn search_bucket(&self, key: &[u8]) -> (usize, bool) {
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
