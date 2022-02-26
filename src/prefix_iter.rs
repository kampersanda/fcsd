use crate::utils;
use crate::Set;

/// Iterator to enumerate keys starting from a given string.
#[derive(Clone)]
pub struct PrefixIter<'a> {
    set: &'a Set,
    dec: Vec<u8>,
    key: Vec<u8>,
    pos: usize,
    id: usize,
}

impl<'a> PrefixIter<'a> {
    /// Makes an iterator [`PrefixIter`].
    ///
    /// # Arguments
    ///
    ///  - `set`: Front-coding dictionay.
    ///  - `key`: Prefix key.
    pub fn new<P>(set: &'a Set, key: P) -> Self
    where
        P: AsRef<[u8]>,
    {
        Self {
            key: key.as_ref().to_vec(),
            set,
            dec: Vec::with_capacity(set.max_length()),
            pos: 0,
            id: 0,
        }
    }

    /// Resets the prefix key.
    ///
    /// # Arguments
    ///
    ///  - `key`: Prefix key.
    pub fn reset<P>(&mut self, key: P)
    where
        P: AsRef<[u8]>,
    {
        self.key = key.as_ref().to_vec();
        self.dec.clear();
        self.pos = 0;
        self.id = 0;
    }

    fn search_first(&mut self) -> bool {
        let (set, dec) = (&self.set, &mut self.dec);

        if self.key.is_empty() {
            self.pos = set.decode_header(0, dec);
            self.id = 0;
            return true;
        }

        let (bi, found) = set.search_bucket(&self.key);
        self.pos = set.decode_header(bi, dec);
        self.id = bi * set.bucket_size();

        if found || utils::is_prefix(&self.key, dec) {
            return true;
        }

        for bj in 1..set.bucket_size() {
            if self.pos == set.serialized.len() {
                break;
            }

            let (lcp, next_pos) = set.decode_lcp(self.pos);
            self.pos = next_pos;
            dec.resize(lcp, 0);
            self.pos = set.decode_next(self.pos, dec);

            if utils::is_prefix(&self.key, dec) {
                self.id += bj;
                return true;
            }
        }

        false
    }
}

impl<'a> Iterator for PrefixIter<'a> {
    type Item = (usize, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.set.serialized.len() {
            return None;
        }

        if self.dec.is_empty() {
            if !self.search_first() {
                self.dec.clear();
                self.pos = self.set.serialized.len();
                self.id = 0;
                return None;
            }
        } else {
            self.id += 1;
            if self.set.pos_in_bucket(self.id) == 0 {
                self.dec.clear();
            } else {
                let (lcp, next_pos) = self.set.decode_lcp(self.pos);
                self.pos = next_pos;
                self.dec.resize(lcp, 0);
            }
            self.pos = self.set.decode_next(self.pos, &mut self.dec);
        }

        if utils::is_prefix(&self.key, &self.dec) {
            Some((self.id, self.dec.clone()))
        } else {
            self.dec.clear();
            self.pos = self.set.serialized.len();
            self.id = 0;
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.set.len()))
    }
}
