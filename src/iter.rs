use crate::FcDict;

/// Iterator to enumerate keys stored in the dictionary.
#[derive(Clone)]
pub struct FcIterator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
    pos: usize,
    id: usize,
}

impl<'a> FcIterator<'a> {
    /// Makes an iterator [`FcIterator`].
    ///
    /// # Arguments
    ///
    ///  - `dict`: Front-coding dictionay.
    pub fn new(dict: &'a FcDict) -> Self {
        Self {
            dict,
            dec: Vec::with_capacity(dict.max_length()),
            pos: 0,
            id: 0,
        }
    }
}

impl<'a> Iterator for FcIterator<'a> {
    type Item = (usize, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.dict.serialized.len() {
            return None;
        }
        if self.dict.pos_in_bucket(self.id) == 0 {
            self.dec.clear();
        } else {
            let (lcp, next_pos) = self.dict.decode_lcp(self.pos);
            self.pos = next_pos;
            self.dec.resize(lcp, 0);
        }
        self.pos = self.dict.decode_next(self.pos, &mut self.dec);
        self.id += 1;
        Some((self.id - 1, self.dec.clone()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.dict.num_keys(), Some(self.dict.num_keys()))
    }
}
