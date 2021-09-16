/// Returns (lcp, cmp) such that
///  - lcp: Length of longest commom prefix of two strings.
///  - cmp: if a < b then positive, elif b < a then negative, else zero.
pub fn get_lcp(a: &[u8], b: &[u8]) -> (usize, isize) {
    let min_len = std::cmp::min(a.len(), b.len());
    for i in 0..min_len {
        if a[i] != b[i] {
            return (i, b[i] as isize - a[i] as isize);
        }
    }
    if a.len() < b.len() {
        (min_len, 1)
    } else if b.len() < a.len() {
        (min_len, -1)
    } else {
        (min_len, 0)
    }
}

pub fn get_strlen(a: &[u8]) -> usize {
    use crate::END_MARKER;
    a.iter().position(|&c| c == END_MARKER).unwrap()
}

pub mod vbyte {
    pub fn append(bytes: &mut Vec<u8>, mut val: usize) {
        while 127 < val {
            bytes.push(((val & 127) | 0x80) as u8);
            val >>= 7;
        }
        bytes.push((val & 127) as u8);
    }
    pub fn decode(bytes: &[u8]) -> (usize, usize) {
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
