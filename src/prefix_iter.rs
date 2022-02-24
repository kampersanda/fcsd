use crate::utils;
use crate::FcDict;

/// Iterator class to enumerate the stored keys and IDs in lex order, starting with a prefix.
#[derive(Clone)]
pub struct FcPrefixIterator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
    key: &'a [u8],
    pos: usize,
    id: usize,
}

impl<'a> FcPrefixIterator<'a> {
    /// Makes the iterator with the prefix key.
    pub fn new(dict: &'a FcDict, key: &'a [u8]) -> FcPrefixIterator<'a> {
        FcPrefixIterator {
            key,
            dict,
            dec: Vec::with_capacity(dict.max_length()),
            pos: 0,
            id: 0,
        }
    }

    /// Inits the prefix key.
    pub fn init_key(&mut self, key: &'a [u8]) {
        self.key = key;
        self.dec.clear();
        self.pos = 0;
        self.id = 0;
    }

    fn search_first(&mut self) -> bool {
        let (dict, dec) = (&self.dict, &mut self.dec);

        if self.key.is_empty() {
            self.pos = dict.decode_header(0, dec);
            self.id = 0;
            return true;
        }

        let (bi, found) = dict.search_bucket(self.key);
        self.pos = dict.decode_header(bi, dec);
        self.id = bi * dict.bucket_size();

        if found || utils::is_prefix(self.key, dec) {
            return true;
        }

        for bj in 1..dict.bucket_size() {
            if self.pos == dict.serialized.len() {
                break;
            }

            let (lcp, next_pos) = dict.decode_lcp(self.pos);
            self.pos = next_pos;
            dec.resize(lcp, 0);
            self.pos = dict.decode_next(self.pos, dec);

            if utils::is_prefix(self.key, dec) {
                self.id += bj;
                return true;
            }
        }

        false
    }
}

impl<'a> Iterator for FcPrefixIterator<'a> {
    type Item = (usize, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.dict.serialized.len() {
            return None;
        }

        if self.dec.is_empty() {
            if !self.search_first() {
                self.dec.clear();
                self.pos = self.dict.serialized.len();
                self.id = 0;
                return None;
            }
        } else {
            self.id += 1;
            if self.dict.pos_in_bucket(self.id) == 0 {
                self.dec.clear();
            } else {
                let (lcp, next_pos) = self.dict.decode_lcp(self.pos);
                self.pos = next_pos;
                self.dec.resize(lcp, 0);
            }
            self.pos = self.dict.decode_next(self.pos, &mut self.dec);
        }

        if utils::is_prefix(self.key, &self.dec) {
            Some((self.id, self.dec.clone()))
        } else {
            self.dec.clear();
            self.pos = self.dict.serialized.len();
            self.id = 0;
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.dict.num_keys()))
    }
}
