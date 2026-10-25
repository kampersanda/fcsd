use crate::errors::{FcsdError, Result};
use crate::utils::{self, ByteReader};
use alloc::vec;
use alloc::vec::Vec;
use core::convert::TryInto;

/// Bit-packed integer vector whose words are stored as little-endian `u64`s
/// inside a serialized byte buffer.
///
/// The vector only keeps the position of its words, so every access takes the
/// buffer it was read from.
#[derive(Clone, Copy)]
pub struct IntVector {
    offset: usize,
    len: usize,
    bits: usize,
    mask: u64,
}

impl IntVector {
    /// Packs `input` and appends its serialized form to `out`.
    pub fn write(input: &[u64], out: &mut Vec<u8>) {
        let len = input.len();
        let bits = utils::needed_bits(input.iter().copied().max().unwrap_or(0));
        let mask = (1 << bits) - 1;

        let mut chunks = vec![0_u64; Self::words_for(len * bits)];

        for (i, &x) in input.iter().enumerate() {
            let (q, m) = Self::decompose(i * bits);
            chunks[q] &= !(mask << m);
            chunks[q] |= (x & mask) << m;
            if 64 < m + bits {
                let diff = 64 - m;
                chunks[q + 1] &= !(mask >> diff);
                chunks[q + 1] |= (x & mask) >> diff;
            }
        }

        out.extend_from_slice(&(chunks.len() as u64).to_le_bytes());
        for x in chunks {
            out.extend_from_slice(&x.to_le_bytes());
        }
        out.extend_from_slice(&(len as u64).to_le_bytes());
        out.extend_from_slice(&(bits as u64).to_le_bytes());
        out.extend_from_slice(&mask.to_le_bytes());
    }

    /// Reads a serialized vector at the current position of `reader`.
    pub fn read(reader: &mut ByteReader<'_>) -> Result<Self> {
        let num_words = reader.read_usize("chunk count does not fit in usize")?;
        let offset = reader.pos();
        let num_bytes = num_words
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or(FcsdError::InvalidData("chunk size overflow"))?;
        reader.take(num_bytes)?;

        let len = reader.read_usize("element count does not fit in usize")?;
        let bits = reader.read_usize("bit width does not fit in usize")?;
        let mask = reader.read_u64()?;
        let vector = Self {
            offset,
            len,
            bits,
            mask,
        };
        vector.validate(num_words)?;
        Ok(vector)
    }

    /// Gets the `i`-th integer from `data`, the buffer this vector was read from.
    #[inline(always)]
    pub fn get(&self, data: &[u8], i: usize) -> u64 {
        let word = |q: usize| {
            let start = self.offset + q * 8;
            u64::from_le_bytes(data[start..start + 8].try_into().unwrap())
        };
        let (q, m) = Self::decompose(i * self.bits);
        if m + self.bits <= 64 {
            (word(q) >> m) & self.mask
        } else {
            ((word(q) >> m) | (word(q + 1) << (64 - m))) & self.mask
        }
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    fn validate(&self, num_words: usize) -> Result<()> {
        if self.bits == 0 || self.bits > u64::BITS as usize {
            return Err(FcsdError::InvalidData("bit width must be in 1..=64"));
        }

        let expected_mask = if self.bits == u64::BITS as usize {
            u64::MAX
        } else {
            (1_u64 << self.bits) - 1
        };
        if self.mask != expected_mask {
            return Err(FcsdError::InvalidData("mask does not match bit width"));
        }

        let total_bits = self
            .len
            .checked_mul(self.bits)
            .ok_or(FcsdError::InvalidData("packed bit length overflow"))?;
        if num_words != Self::words_for(total_bits) {
            return Err(FcsdError::InvalidData(
                "chunk count does not match length and bit width",
            ));
        }
        Ok(())
    }

    #[inline(always)]
    const fn words_for(bits: usize) -> usize {
        bits.div_ceil(64)
    }

    #[inline(always)]
    const fn decompose(x: usize) -> (usize, usize) {
        (x / 64, x % 64)
    }
}
