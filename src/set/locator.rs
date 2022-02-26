use std::cmp::Ordering;

use crate::utils;
use crate::Set;

/// Locator class to get ids of given string keys.
#[derive(Clone)]
pub struct FcLocator<'a> {
    dict: &'a Set,
    dec: Vec<u8>,
}

impl<'a> FcLocator<'a> {
    /// Makes a [`FcLocator`].
    ///
    /// # Arguments
    ///
    ///  - `dict`: Front-coding dictionay.
    pub fn new(dict: &'a Set) -> Self {
        Self {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
        }
    }

    /// Returns the id of the given key.
    ///
    /// # Arguments
    ///
    ///  - `key`: String key to be searched.
    ///
    /// # Complexity
    ///
    ///  - Logarithmic over the number of keys
    pub fn run<P>(&mut self, key: P) -> Option<usize>
    where
        P: AsRef<[u8]>,
    {
        let key = key.as_ref();
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
