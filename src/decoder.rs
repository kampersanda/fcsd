use alloc::vec::Vec;

use crate::utils;
use crate::{Set, SetRef};

/// Decoder class to get string keys associated with given ids.
#[derive(Clone)]
pub struct Decoder<'a> {
    set: SetRef<'a>,
    dec: Vec<u8>,
}

impl<'a> Decoder<'a> {
    /// Makes a [`Decoder`].
    ///
    /// # Arguments
    ///
    ///  - `set`: Front-coding dictionay.
    pub fn new<S>(set: &'a Set<S>) -> Self
    where
        S: AsRef<[u8]>,
    {
        let set = set.view();
        Self {
            set,
            dec: Vec::with_capacity(set.max_length()),
        }
    }

    /// Returns the string key associated with the given id.
    ///
    /// # Arguments
    ///
    ///  - `id`: Integer id to be decoded.
    ///
    /// # Panics
    ///
    /// If `id` is no less than the number of keys, `panic!` will occur.
    ///
    /// # Complexity
    ///
    ///  - Constant
    pub fn run(&mut self, id: usize) -> Vec<u8> {
        let (set, dec) = (&self.set, &mut self.dec);
        assert!(id < set.len());

        let (bi, bj) = (set.bucket_id(id), set.pos_in_bucket(id));
        let mut pos = set.decode_header(bi, dec);

        for _ in 0..bj {
            let (lcp, num) = utils::vbyte::decode(&set.keys[pos..]);
            pos += num;

            dec.resize(lcp, 0);
            pos = set.decode_next(pos, dec);
        }

        dec.clone()
    }
}
