use crate::utils;
use alloc::vec;
use alloc::vec::Vec;
#[cfg(feature = "std")]
use core::convert::TryFrom;
#[cfg(feature = "std")]
use std::io;

#[derive(Clone)]
pub struct IntVector {
    chunks: Vec<u64>,
    len: usize,
    bits: usize,
    mask: u64,
}

impl IntVector {
    pub fn build(input: &[u64]) -> Self {
        let len = input.len();
        let bits = utils::needed_bits(input.iter().copied().max().unwrap_or(0));
        let mask = (1 << bits) - 1;

        let mut chunks = vec![0; Self::words_for(len * bits)];

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

        Self {
            chunks,
            len,
            bits,
            mask,
        }
    }

    #[inline(always)]
    pub fn get(&self, i: usize) -> u64 {
        let (q, m) = Self::decompose(i * self.bits);
        if m + self.bits <= 64 {
            (self.chunks[q] >> m) & self.mask
        } else {
            ((self.chunks[q] >> m) | (self.chunks[q + 1] << (64 - m))) & self.mask
        }
    }

    #[inline(always)]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[cfg(feature = "std")]
    pub fn size_in_bytes(&self) -> usize {
        8 + self.chunks.len() * 8 + 8 * 3
    }

    #[cfg(feature = "std")]
    pub fn serialize_into<W: io::Write>(&self, mut writer: W) -> io::Result<()> {
        utils::write_u64_le(&mut writer, self.chunks.len() as u64)?;
        for &x in &self.chunks {
            utils::write_u64_le(&mut writer, x)?;
        }
        utils::write_u64_le(&mut writer, self.len as u64)?;
        utils::write_u64_le(&mut writer, self.bits as u64)?;
        utils::write_u64_le(&mut writer, self.mask)?;
        Ok(())
    }

    #[cfg(feature = "std")]
    pub fn deserialize_from_with_limit<R: io::Read>(
        mut reader: R,
        max_bytes: usize,
    ) -> io::Result<Self> {
        let chunks_len = Self::read_usize(&mut reader, "chunk count does not fit in usize")?;
        let chunks_bytes = chunks_len
            .checked_mul(core::mem::size_of::<u64>())
            .ok_or_else(|| Self::invalid_data("chunk allocation size overflow"))?;
        if chunks_bytes > max_bytes {
            return Err(Self::invalid_data(
                "chunk allocation exceeds the deserialization limit",
            ));
        }

        let mut chunks = Vec::new();
        for _ in 0..chunks_len {
            chunks.push(utils::read_u64_le(&mut reader)?);
        }

        let len = Self::read_usize(&mut reader, "element count does not fit in usize")?;
        let bits = Self::read_usize(&mut reader, "bit width does not fit in usize")?;
        let mask = utils::read_u64_le(&mut reader)?;
        let vector = Self {
            chunks,
            len,
            bits,
            mask,
        };
        vector.validate()?;
        Ok(vector)
    }

    #[inline(always)]
    #[cfg(feature = "std")]
    pub fn allocated_bytes(&self) -> usize {
        self.chunks.len() * core::mem::size_of::<u64>()
    }

    #[cfg(feature = "std")]
    fn validate(&self) -> io::Result<()> {
        if self.bits == 0 || self.bits > u64::BITS as usize {
            return Err(Self::invalid_data("bit width must be in 1..=64"));
        }

        let expected_mask = if self.bits == u64::BITS as usize {
            u64::MAX
        } else {
            (1_u64 << self.bits) - 1
        };
        if self.mask != expected_mask {
            return Err(Self::invalid_data("mask does not match bit width"));
        }

        let total_bits = self
            .len
            .checked_mul(self.bits)
            .ok_or_else(|| Self::invalid_data("packed bit length overflow"))?;
        let expected_chunks = Self::words_for(total_bits);
        if self.chunks.len() != expected_chunks {
            return Err(Self::invalid_data(
                "chunk count does not match length and bit width",
            ));
        }
        Ok(())
    }

    #[cfg(feature = "std")]
    fn invalid_data(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }

    #[cfg(feature = "std")]
    fn read_usize<R: io::Read>(reader: &mut R, error: &'static str) -> io::Result<usize> {
        usize::try_from(utils::read_u64_le(reader)?).map_err(|_| Self::invalid_data(error))
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
