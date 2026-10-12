use crate::Set;
use std::iter::FusedIterator;

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
        let remaining = self.set.len().saturating_sub(self.id);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for Iter<'_> {}

impl FusedIterator for Iter<'_> {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn size_hint_tracks_remaining_items() {
        let set = Set::new(["alpha", "beta", "gamma"]).unwrap();
        let mut iter = set.iter();

        assert_eq!(iter.size_hint(), (3, Some(3)));
        assert_eq!(iter.len(), 3);

        assert_eq!(iter.next(), Some((0, b"alpha".to_vec())));
        assert_eq!(iter.size_hint(), (2, Some(2)));
        assert_eq!(iter.len(), 2);

        assert_eq!(iter.next(), Some((1, b"beta".to_vec())));
        assert_eq!(iter.size_hint(), (1, Some(1)));

        assert_eq!(iter.next(), Some((2, b"gamma".to_vec())));
        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.len(), 0);

        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.size_hint(), (0, Some(0)));
    }

    #[test]
    fn empty_iter_has_no_remaining_items() {
        let set = Set::new(std::iter::empty::<&str>()).unwrap();
        let mut iter = set.iter();

        assert_eq!(iter.size_hint(), (0, Some(0)));
        assert_eq!(iter.len(), 0);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn iter_is_fused() {
        fn assert_fused<I: FusedIterator>(_iter: I) {}

        let set = Set::new(["alpha"]).unwrap();
        assert_fused(set.iter());
    }
}
