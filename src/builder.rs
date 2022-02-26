use anyhow::{anyhow, Result};

use crate::intvec::IntVector;
use crate::utils;
use crate::Set;
use crate::END_MARKER;

/// Builder class for [`Set`].
#[derive(Clone)]
pub struct Builder {
    pointers: Vec<u64>,
    serialized: Vec<u8>,
    last_key: Vec<u8>,
    num_keys: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
}

impl Builder {
    /// Creates a [`Builder`] with the given bucket size.
    ///
    /// # Arguments
    ///
    ///  - `bucket_size`: The number of strings in each bucket, which must be a power of two.
    ///
    /// # Errors
    ///
    /// [`anyhow::Result`] will be returned when
    ///
    ///  - `bucket_size` is zero, or
    ///  - `bucket_size` is not a power of two.
    pub fn new(bucket_size: usize) -> Result<Self> {
        if bucket_size == 0 {
            Err(anyhow!("bucket_size must not be zero."))
        } else if !utils::is_power_of_two(bucket_size) {
            Err(anyhow!("bucket_size must be a power of two."))
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

    /// Pushes a key back to the dictionary.
    ///
    /// # Arguments
    ///
    ///  - `key`: String key to be added.
    ///
    /// # Errors
    ///
    /// [`anyhow::Result`] will be returned when
    ///
    ///  - `key` is no more than the last one, or
    ///  - `key` contains [`END_MARKER`].
    pub fn add(&mut self, key: &[u8]) -> Result<()> {
        if utils::contains_end_marker(key) {
            return Err(anyhow!(
                "The input key must not contain END_MARKER (={}).",
                END_MARKER
            ));
        }

        let (lcp, cmp) = utils::get_lcp(&self.last_key, key);
        if cmp <= 0 {
            return Err(anyhow!("The input key must be more than the last one.",));
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

    /// Builds and returns the dictionary.
    pub fn finish(self) -> Set {
        Set {
            pointers: IntVector::build(&self.pointers),
            serialized: self.serialized,
            num_keys: self.num_keys,
            bucket_bits: self.bucket_bits,
            bucket_mask: self.bucket_mask,
            max_length: self.max_length,
        }
    }
}
