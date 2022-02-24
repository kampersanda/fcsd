use std::cmp::Ordering;

use crate::utils;
use crate::FcDict;

/// Locator class to get the ID associated with a key string.
#[derive(Clone)]
pub struct FcLocator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcLocator<'a> {
    /// Makes the locator.
    pub fn new(dict: &'a FcDict) -> Self {
        Self {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
        }
    }

    /// Returns the ID associated with the given key.
    pub fn run(&mut self, key: &[u8]) -> Option<usize> {
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
