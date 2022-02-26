use crate::utils;
use crate::FcDict;

/// Decoder class to get string keys associated with given ids.
#[derive(Clone)]
pub struct FcDecoder<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcDecoder<'a> {
    /// Makes a [`FcDecoder`].
    ///
    /// # Arguments
    ///
    ///  - `dict`: Front-coding dictionay.
    pub fn new(dict: &'a FcDict) -> Self {
        Self {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
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
        let (dict, dec) = (&self.dict, &mut self.dec);
        assert!(id < dict.num_keys());

        let (bi, bj) = (dict.bucket_id(id), dict.pos_in_bucket(id));
        let mut pos = dict.decode_header(bi, dec);

        for _ in 0..bj {
            let (lcp, num) = utils::vbyte::decode(&dict.serialized[pos..]);
            pos += num;

            dec.resize(lcp, 0);
            pos = dict.decode_next(pos, dec);
        }

        dec.clone()
    }
}
