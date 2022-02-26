use crate::Set;

/// Iterator to enumerate keys stored in the dictionary.
#[derive(Clone)]
pub struct Iter<'a> {
    set: &'a Set,
    dec: Vec<u8>,
    pos: usize,
    id: usize,
}

impl<'a> Iter<'a> {
    /// Makes an iterator [`Iter`].
    ///
    /// # Arguments
    ///
    ///  - `set`: Front-coding dictionay.
    pub fn new(set: &'a Set) -> Self {
        Self {
            set,
            dec: Vec::with_capacity(set.max_length()),
            pos: 0,
            id: 0,
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = (usize, Vec<u8>);

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos == self.set.serialized.len() {
            return None;
        }
        if self.set.pos_in_bucket(self.id) == 0 {
            self.dec.clear();
        } else {
            let (lcp, next_pos) = self.set.decode_lcp(self.pos);
            self.pos = next_pos;
            self.dec.resize(lcp, 0);
        }
        self.pos = self.set.decode_next(self.pos, &mut self.dec);
        self.id += 1;
        Some((self.id - 1, self.dec.clone()))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.set.num_keys(), Some(self.set.num_keys()))
    }
}
