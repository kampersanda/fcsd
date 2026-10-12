use crate::utils;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::convert::TryFrom;
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

    pub fn size_in_bytes(&self) -> usize {
        8 + self.chunks.len() * 8 + 8 * 3
    }

    pub fn serialize_into<W: io::Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(self.chunks.len() as u64)?;
        for &x in &self.chunks {
            writer.write_u64::<LittleEndian>(x)?;
        }
        writer.write_u64::<LittleEndian>(self.len as u64)?;
        writer.write_u64::<LittleEndian>(self.bits as u64)?;
        writer.write_u64::<LittleEndian>(self.mask)?;
        Ok(())
    }

    pub fn deserialize_from_with_limit<R: io::Read>(
        mut reader: R,
        max_bytes: usize,
    ) -> io::Result<Self> {
        let chunks_len = usize::try_from(reader.read_u64::<LittleEndian>()?)
            .map_err(|_| Self::invalid_data("chunk count does not fit in usize"))?;
        let chunks_bytes = chunks_len
            .checked_mul(std::mem::size_of::<u64>())
            .ok_or_else(|| Self::invalid_data("chunk allocation size overflow"))?;
        if chunks_bytes > max_bytes {
            return Err(Self::invalid_data(
                "chunk allocation exceeds the deserialization limit",
            ));
        }

        let mut chunks = Vec::new();
        chunks
            .try_reserve_exact(chunks_len)
            .map_err(|_| Self::invalid_data("chunk allocation is too large"))?;
        chunks.resize(chunks_len, 0);
        for chunk in &mut chunks {
            *chunk = reader.read_u64::<LittleEndian>()?;
        }

        let len = usize::try_from(reader.read_u64::<LittleEndian>()?)
            .map_err(|_| Self::invalid_data("element count does not fit in usize"))?;
        let bits = usize::try_from(reader.read_u64::<LittleEndian>()?)
            .map_err(|_| Self::invalid_data("bit width does not fit in usize"))?;
        let mask = reader.read_u64::<LittleEndian>()?;
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
    pub fn allocated_bytes(&self) -> usize {
        self.chunks.len() * std::mem::size_of::<u64>()
    }

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
        let expected_chunks = total_bits
            .checked_add(63)
            .ok_or_else(|| Self::invalid_data("packed chunk count overflow"))?
            / 64;
        if self.chunks.len() != expected_chunks {
            return Err(Self::invalid_data(
                "chunk count does not match length and bit width",
            ));
        }

        let used_in_last_chunk = total_bits % 64;
        if used_in_last_chunk != 0 {
            if let Some(chunk) = self.chunks.last() {
                if chunk >> used_in_last_chunk != 0 {
                    return Err(Self::invalid_data(
                        "unused bits in the final chunk are not zero",
                    ));
                }
            }
        }
        Ok(())
    }

    fn invalid_data(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
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
