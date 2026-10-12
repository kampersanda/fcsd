use crate::utils;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
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

    pub fn deserialize_from<R: io::Read>(mut reader: R) -> io::Result<Self> {
        let chunks = {
            let len = reader.read_u64::<LittleEndian>()? as usize;
            let mut chunks = vec![0; len];
            for x in chunks.iter_mut() {
                *x = reader.read_u64::<LittleEndian>()?;
            }
            chunks
        };
        let len = reader.read_u64::<LittleEndian>()? as usize;
        let bits = reader.read_u64::<LittleEndian>()? as usize;
        let mask = reader.read_u64::<LittleEndian>()?;
        Ok(Self {
            chunks,
            len,
            bits,
            mask,
        })
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
