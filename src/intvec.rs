use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io;

pub struct IntVector {
    chunks: Vec<u64>,
    len: usize,
    bits: usize,
    mask: u64,
}

impl IntVector {
    pub fn build(input: &[u64]) -> IntVector {
        let len = input.len();
        let bits = needed_bits(*input.iter().max().unwrap());
        let mask = (1 << bits) - 1;

        let mut chunks = vec![0; words_for(len * bits)];

        for i in 0..len {
            let (q, m) = decompose(i * bits);
            chunks[q] &= !(mask << m);
            chunks[q] |= (input[i] & mask) << m;
            if 64 < m + bits {
                let diff = 64 - m;
                chunks[q + 1] &= !(mask >> diff);
                chunks[q + 1] |= (input[i] & mask) >> diff;
            }
        }

        IntVector {
            chunks: chunks,
            len: len,
            bits: bits,
            mask: mask,
        }
    }

    pub fn get(&self, i: usize) -> u64 {
        let (q, m) = decompose(i * self.bits);
        if m + self.bits <= 64 {
            (self.chunks[q] >> m) & self.mask
        } else {
            ((self.chunks[q] >> m) | (self.chunks[q + 1] << (64 - m))) & self.mask
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn serialize_into<W: io::Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_u64::<LittleEndian>(self.chunks.len() as u64)?;
        for &x in &self.chunks {
            writer.write_u64::<LittleEndian>(x)?;
        }
        writer.write_u64::<LittleEndian>(self.len as u64)?;
        writer.write_u64::<LittleEndian>(self.bits as u64)?;
        writer.write_u64::<LittleEndian>(self.mask as u64)?;
        Ok(())
    }

    pub fn deserialize_from<R: io::Read>(mut reader: R) -> io::Result<IntVector> {
        let chunks = {
            let len = reader.read_u64::<LittleEndian>()? as usize;
            let mut chunks = vec![0; len];
            for i in 0..len {
                chunks[i] = reader.read_u64::<LittleEndian>()?;
            }
            chunks
        };
        let len = reader.read_u64::<LittleEndian>()? as usize;
        let bits = reader.read_u64::<LittleEndian>()? as usize;
        let mask = reader.read_u64::<LittleEndian>()?;
        Ok(IntVector {
            chunks: chunks,
            len: len,
            bits: bits,
            mask: mask,
        })
    }
}

fn needed_bits(mut x: u64) -> usize {
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

fn words_for(bits: usize) -> usize {
    (bits + 63) / 64
}

fn decompose(x: usize) -> (usize, usize) {
    (x / 64, x % 64)
}
