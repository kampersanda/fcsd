use crate::utils;
use crate::FcDict;

/// Decoder class to get the key string associated with an ID.
#[derive(Clone)]
pub struct FcDecoder<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcDecoder<'a> {
    /// Makes the decoder.
    pub fn new(dict: &'a FcDict) -> Self {
        Self {
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
