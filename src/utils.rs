use std::cmp::Ordering;

use crate::END_MARKER;

/// Returns (lcp, cmp) such that
///  - lcp: Length of longest commom prefix of two strings.
///  - cmp: if a < b then positive, elif b < a then negative, else zero.
#[inline(always)]
pub fn get_lcp(a: &[u8], b: &[u8]) -> (usize, isize) {
    let min_len = std::cmp::min(a.len(), b.len());
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
    a.iter().any(|&c| c == END_MARKER)
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
    #[inline(always)]
    pub fn append(bytes: &mut Vec<u8>, mut val: usize) {
        while 127 < val {
            bytes.push(((val & 127) | 0x80) as u8);
            val >>= 7;
        }
        bytes.push((val & 127) as u8);
    }
    #[inline(always)]
    pub const fn decode(bytes: &[u8]) -> (usize, usize) {
        let mut val = 0;
        let (mut i, mut j) = (0, 0);
        while (bytes[i] & 0x80) != 0 {
            val |= ((bytes[i] & 127) as usize) << j;
            i += 1;
            j += 7;
        }
        val |= ((bytes[i] & 127) as usize) << j;
        (val, i + 1)
    }
}
