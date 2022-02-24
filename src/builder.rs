use crate::intvec::IntVector;
use crate::utils;
use crate::FcDict;
use crate::END_MARKER;

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

    pub fn finish(self) -> FcDict {
        FcDict {
            pointers: IntVector::build(&self.pointers),
            serialized: self.serialized,
            num_keys: self.num_keys,
            bucket_bits: self.bucket_bits,
            bucket_mask: self.bucket_mask,
            max_length: self.max_length,
        }
    }
}
