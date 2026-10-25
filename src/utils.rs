use core::cmp::Ordering;
use core::convert::{TryFrom, TryInto};

use crate::errors::{FcsdError, Result};
use crate::END_MARKER;

/// Cursor that reads little-endian values from serialized bytes.
pub struct ByteReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> ByteReader<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    pub const fn pos(&self) -> usize {
        self.pos
    }

    pub const fn is_empty(&self) -> bool {
        self.pos == self.data.len()
    }

    pub fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let bytes = self
            .data
            .get(self.pos..)
            .and_then(|rest| rest.get(..len))
            .ok_or(FcsdError::InvalidData("unexpected end of data"))?;
        self.pos += len;
        Ok(bytes)
    }

    pub fn read_u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    pub fn read_u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    pub fn read_usize(&mut self, error: &'static str) -> Result<usize> {
        usize::try_from(self.read_u64()?).map_err(|_| FcsdError::InvalidData(error))
    }
}

/// Returns (lcp, cmp) such that
///  - lcp: Length of longest commom prefix of two strings.
///  - cmp: if a < b then positive, elif b < a then negative, else zero.
#[inline(always)]
pub fn get_lcp(a: &[u8], b: &[u8]) -> (usize, isize) {
    let min_len = core::cmp::min(a.len(), b.len());
    for i in 0..min_len {
        if a[i] != b[i] {
            return (i, b[i] as isize - a[i] as isize);
        }
    }
    match a.len().cmp(&b.len()) {
        Ordering::Less => (min_len, 1),
        Ordering::Greater => (min_len, -1),
        Ordering::Equal => (min_len, 0),
    }
}

#[inline(always)]
pub fn get_strlen(a: &[u8]) -> usize {
    a.iter().position(|&c| c == END_MARKER).unwrap()
}

/// Checks if a is a prefix of b.
#[inline(always)]
pub fn is_prefix(a: &[u8], b: &[u8]) -> bool {
    if a.len() > b.len() {
        return false;
    }
    for i in 0..a.len() {
        if a[i] != b[i] {
            return false;
        }
    }
    true
}

/// Checks if END_MARKER is contained.
#[inline(always)]
pub fn contains_end_marker(a: &[u8]) -> bool {
    a.contains(&END_MARKER)
}

#[inline(always)]
pub fn is_power_of_two(x: usize) -> bool {
    debug_assert_ne!(x, 0);
    (x & (x - 1)) == 0
}

#[inline(always)]
pub const fn needed_bits(mut x: u64) -> usize {
    if x == 0 {
        return 1;
    }
    let mut n = 0;
    while x != 0 {
        x >>= 1;
        n += 1;
    }
    n
}

pub mod vbyte {
    use alloc::vec::Vec;

    #[inline(always)]
    pub fn append(bytes: &mut Vec<u8>, mut val: usize) {
        while 127 < val {
            bytes.push(((val & 127) | 0x80) as u8);
            val >>= 7;
        }
        bytes.push((val & 127) as u8);
    }
    pub fn try_decode(bytes: &[u8]) -> Option<(usize, usize)> {
        let mut val = 0;
        let (mut i, mut shift) = (0, 0);
        loop {
            let byte = *bytes.get(i)?;
            let payload = (byte & 127) as usize;
            if shift >= usize::BITS || payload > (usize::MAX >> shift) {
                return None;
            }
            val |= payload << shift;
            i += 1;
            if byte & 0x80 == 0 {
                return Some((val, i));
            }
            shift += 7;
        }
    }
    #[inline(always)]
    pub fn decode(bytes: &[u8]) -> (usize, usize) {
        try_decode(bytes).unwrap()
    }
}
