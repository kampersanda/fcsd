use std::cmp::Ordering;

use crate::utils;
use crate::Set;

/// Locator class to get ids of given string keys.
#[derive(Clone)]
pub struct Locator<'a> {
    set: &'a Set,
    dec: Vec<u8>,
}

impl<'a> Locator<'a> {
    /// Makes a [`Locator`].
    ///
    /// # Arguments
    ///
    ///  - `set`: Front-coding dictionay.
    pub fn new(set: &'a Set) -> Self {
        Self {
            set,
            dec: Vec::with_capacity(set.max_length()),
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

        let (set, dec) = (&self.set, &mut self.dec);
        let (bi, found) = set.search_bucket(key);

        if found {
            return Some(bi * set.bucket_size());
        }

        let mut pos = set.decode_header(bi, dec);
        if pos == set.serialized.len() {
            return None;
        }

        // 1) Process the 1st internal string
        {
            let (dec_lcp, next_pos) = set.decode_lcp(pos);
            pos = next_pos;
            dec.resize(dec_lcp, 0);
            pos = set.decode_next(pos, dec);
        }

        let (mut lcp, cmp) = utils::get_lcp(key, dec);
        match cmp.cmp(&0) {
            Ordering::Equal => {
                return Some(bi * set.bucket_size() + 1);
            }
            Ordering::Greater => return None,
            _ => {}
        }

        // 2) Process the next strings
        for bj in 2..set.bucket_size() {
            if pos == set.serialized.len() {
                break;
            }

            let (dec_lcp, next_pos) = set.decode_lcp(pos);
            pos = next_pos;

            if lcp > dec_lcp {
                break;
            }

            dec.resize(dec_lcp, 0);
            pos = set.decode_next(pos, dec);

            if lcp == dec_lcp {
                let (next_lcp, cmp) = utils::get_lcp(key, dec);
                match cmp.cmp(&0) {
                    Ordering::Equal => {
                        return Some(bi * set.bucket_size() + bj);
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
