mod intvec;
mod utils;

use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use intvec::IntVector;
use std::io;

const END_MARKER: u8 = 0;
const SERIAL_COOKIE: u32 = 114514;

pub struct FcDict {
    pointers: IntVector,
    serialized: Vec<u8>,
    num_keys: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
}

impl FcDict {
    pub fn from_builder(builder: FcBuilder) -> FcDict {
        FcDict {
            pointers: IntVector::build(&builder.pointers),
            serialized: builder.serialized,
            num_keys: builder.num_keys,
            bucket_bits: builder.bucket_bits,
            bucket_mask: builder.bucket_mask,
            max_length: builder.max_length,
        }
    }

    pub fn serialized_size_in_bytes(&self) -> usize {
        let mut bytes = 0;
        bytes += 4; // SERIAL_COOKIE
        bytes += self.pointers.serialized_size_in_bytes(); // pointers
        bytes += 8 + self.serialized.len(); // serialized
        bytes + 8 * 4
    }

    pub fn serialize_into<W: io::Write>(&self, mut writer: W) -> io::Result<()> {
        writer.write_u32::<LittleEndian>(SERIAL_COOKIE)?;
        self.pointers.serialize_into(&mut writer)?;
        writer.write_u64::<LittleEndian>(self.serialized.len() as u64)?;
        for &x in &self.serialized {
            writer.write_u8(x)?;
        }
        writer.write_u64::<LittleEndian>(self.num_keys as u64)?;
        writer.write_u64::<LittleEndian>(self.bucket_bits as u64)?;
        writer.write_u64::<LittleEndian>(self.bucket_mask as u64)?;
        writer.write_u64::<LittleEndian>(self.max_length as u64)?;
        Ok(())
    }

    pub fn deserialize_from<R: io::Read>(mut reader: R) -> io::Result<FcDict> {
        let cookie = reader.read_u32::<LittleEndian>()?;
        if cookie != SERIAL_COOKIE {
            return Err(io::Error::new(io::ErrorKind::Other, "unknown cookie value"));
        }
        let pointers = IntVector::deserialize_from(&mut reader)?;
        let serialized = {
            let len = reader.read_u64::<LittleEndian>()? as usize;
            let mut serialized = vec![0; len];
            for i in 0..len {
                serialized[i] = reader.read_u8()?;
            }
            serialized
        };

        let num_keys = reader.read_u64::<LittleEndian>()? as usize;
        let bucket_bits = reader.read_u64::<LittleEndian>()? as usize;
        let bucket_mask = reader.read_u64::<LittleEndian>()? as usize;
        let max_length = reader.read_u64::<LittleEndian>()? as usize;

        Ok(FcDict {
            pointers: pointers,
            serialized: serialized,
            num_keys: num_keys,
            bucket_bits: bucket_bits,
            bucket_mask: bucket_mask,
            max_length: max_length,
        })
    }

    pub fn num_keys(&self) -> usize {
        self.num_keys
    }

    pub fn num_buckets(&self) -> usize {
        self.pointers.len()
    }

    pub fn bucket_size(&self) -> usize {
        self.bucket_mask + 1
    }

    pub fn max_length(&self) -> usize {
        self.max_length
    }

    fn bucket_id(&self, id: usize) -> usize {
        id >> self.bucket_bits
    }

    fn pos_in_bucket(&self, id: usize) -> usize {
        id & self.bucket_mask
    }

    fn get_header(&self, bi: usize) -> &[u8] {
        let header = &self.serialized[self.pointers.get(bi) as usize..];
        &header[..utils::get_strlen(&header)]
    }

    fn decode_header(&self, bi: usize, dec: &mut Vec<u8>) -> usize {
        dec.clear();
        let mut pos = self.pointers.get(bi) as usize;
        while self.serialized[pos] != END_MARKER {
            dec.push(self.serialized[pos]);
            pos += 1;
        }
        pos + 1
    }

    fn decode_lcp(&self, pos: usize) -> (usize, usize) {
        let (lcp, num) = utils::vbyte::decode(&self.serialized[pos..]);
        (lcp, pos + num)
    }

    fn decode_next(&self, mut pos: usize, dec: &mut Vec<u8>) -> usize {
        while self.serialized[pos] != END_MARKER {
            dec.push(self.serialized[pos]);
            pos += 1;
        }
        pos + 1
    }

    fn search_bucket(&self, key: &[u8]) -> (usize, bool) {
        let mut cmp = 0;
        let (mut lo, mut hi, mut mi) = (0, self.num_buckets(), 0);
        while lo < hi {
            mi = (lo + hi) / 2;
            cmp = utils::get_lcp(key, self.get_header(mi)).1;
            if cmp < 0 {
                lo = mi + 1;
            } else if cmp > 0 {
                hi = mi;
            } else {
                return (mi, true);
            }
        }
        if cmp < 0 || mi == 0 {
            (mi, false)
        } else {
            (mi - 1, false)
        }
    }
}

pub struct FcBuilder {
    pointers: Vec<u64>,
    serialized: Vec<u8>,
    last_key: Vec<u8>,
    num_keys: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_length: usize,
}

impl FcBuilder {
    pub fn new(bucket_size: usize) -> Result<FcBuilder, String> {
        if bucket_size == 0 {
            Err("bucket_size is zero.".to_owned())
        } else if !utils::is_power_of_two(bucket_size) {
            Err("bucket_size is not a power of two.".to_owned())
        } else {
            Ok(FcBuilder {
                pointers: Vec::new(),
                serialized: Vec::new(),
                last_key: Vec::new(),
                num_keys: 0,
                bucket_bits: utils::needed_bits((bucket_size - 1) as u64),
                bucket_mask: bucket_size - 1,
                max_length: 0,
            })
        }
    }

    pub fn add(&mut self, key: &[u8]) -> Result<(), String> {
        if utils::contains_end_marker(key) {
            return Err("The input key contains END_MARKER.".to_owned());
        }

        let (lcp, cmp) = utils::get_lcp(&self.last_key, key);
        if cmp <= 0 {
            return Err("The input key is less than the previous one.".to_owned());
        }

        if self.num_keys & self.bucket_mask == 0 {
            self.pointers.push(self.serialized.len() as u64);
            self.serialized.extend_from_slice(&key);
            self.serialized.push(END_MARKER);
        } else {
            utils::vbyte::append(&mut self.serialized, lcp);
            self.serialized.extend_from_slice(&key[lcp..]);
            self.serialized.push(END_MARKER);
        }

        self.last_key.resize(key.len(), 0);
        self.last_key.copy_from_slice(key);
        self.num_keys += 1;
        self.max_length = std::cmp::max(self.max_length, key.len());

        Ok(())
    }
}

pub struct FcLocater<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcLocater<'a> {
    pub fn new(dict: &'a FcDict) -> FcLocater<'a> {
        FcLocater {
            dict: dict,
            dec: Vec::with_capacity(dict.max_length()),
        }
    }

    pub fn run(&mut self, key: &[u8]) -> Option<usize> {
        if key.is_empty() {
            return None;
        }

        let (dict, dec) = (&self.dict, &mut self.dec);
        let (bi, found) = dict.search_bucket(key);

        if found {
            return Some(bi * dict.bucket_size());
        }

        let mut pos = dict.decode_header(bi, dec);

        for bj in 1..dict.bucket_size() {
            if pos == dict.serialized.len() {
                break;
            }
            let (lcp, next_pos) = dict.decode_lcp(pos);
            pos = next_pos;

            dec.resize(lcp, 0);
            pos = dict.decode_next(pos, dec);

            let cmp = utils::get_lcp(key, &dec).1;
            if cmp == 0 {
                return Some(bi * dict.bucket_size() + bj);
            } else if cmp > 0 {
                break;
            }
        }

        None
    }
}

pub struct FcDecoder<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
}

impl<'a> FcDecoder<'a> {
    pub fn new(dict: &'a FcDict) -> FcDecoder<'a> {
        FcDecoder {
            dict: dict,
            dec: Vec::with_capacity(dict.max_length()),
        }
    }

    pub fn run(&mut self, id: usize) -> Option<&[u8]> {
        let (dict, dec) = (&self.dict, &mut self.dec);
        if dict.num_keys() <= id {
            return None;
        }

        let (bi, bj) = (dict.bucket_id(id), dict.pos_in_bucket(id));
        let mut pos = dict.decode_header(bi, dec);

        for _ in 0..bj {
            let (lcp, num) = utils::vbyte::decode(&dict.serialized[pos..]);
            pos += num;

            dec.resize(lcp, 0);
            pos = dict.decode_next(pos, dec);
        }

        Some(dec)
    }
}

pub struct FcIterator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
    pos: usize,
    id: usize,
}

impl<'a> FcIterator<'a> {
    pub fn new(dict: &'a FcDict) -> FcIterator<'a> {
        FcIterator {
            dict: dict,
            dec: Vec::with_capacity(dict.max_length()),
            pos: 0,
            id: 0,
        }
    }

    pub fn next(&mut self) -> Option<(usize, &[u8])> {
        let (dict, dec) = (&self.dict, &mut self.dec);
        if self.pos == dict.serialized.len() {
            return None;
        }
        if dict.pos_in_bucket(self.id) == 0 {
            dec.clear();
            self.pos = dict.decode_next(self.pos, dec);
        } else {
            let (lcp, next_pos) = dict.decode_lcp(self.pos);
            self.pos = next_pos;
            dec.resize(lcp, 0);
            self.pos = dict.decode_next(self.pos, dec);
        }
        self.id += 1;
        Some((self.id - 1, dec))
    }
}

pub struct FcPrefixIterator<'a> {
    dict: &'a FcDict,
    dec: Vec<u8>,
    key: &'a [u8],
    pos: usize,
    id: usize,
}

impl<'a> FcPrefixIterator<'a> {
    pub fn new(dict: &'a FcDict) -> FcPrefixIterator<'a> {
        FcPrefixIterator {
            key: &[],
            dict: dict,
            dec: Vec::with_capacity(dict.max_length()),
            pos: 0,
            id: 0,
        }
    }

    pub fn set_key(&mut self, key: &'a [u8]) {
        self.key = key;
        self.dec.clear();
        self.pos = 0;
        self.id = 0;
    }

    pub fn next(&mut self) -> Option<(usize, &[u8])> {
        if self.pos == self.dict.serialized.len() {
            return None;
        }

        if self.dec.is_empty() {
            if !self.search_first() {
                self.dec.clear();
                self.pos = self.dict.serialized.len();
                self.id = 0;
                return None;
            }
        } else {
            self.id += 1;
            if self.dict.pos_in_bucket(self.id) == 0 {
                self.dec.clear();
                self.pos = self.dict.decode_next(self.pos, &mut self.dec);
            } else {
                let (lcp, next_pos) = self.dict.decode_lcp(self.pos);
                self.pos = next_pos;
                self.dec.resize(lcp, 0);
                self.pos = self.dict.decode_next(self.pos, &mut self.dec);
            }
        }

        if utils::is_prefix(self.key, &self.dec) {
            Some((self.id, &self.dec))
        } else {
            self.dec.clear();
            self.pos = self.dict.serialized.len();
            self.id = 0;
            None
        }
    }

    fn search_first(&mut self) -> bool {
        let (dict, dec) = (&self.dict, &mut self.dec);

        if self.key.is_empty() {
            self.pos = dict.decode_header(0, dec);
            self.id = 0;
            return true;
        }

        let (bi, found) = dict.search_bucket(self.key);
        self.pos = dict.decode_header(bi, dec);
        self.id = bi * dict.bucket_size();

        if found || utils::is_prefix(self.key, &dec) {
            return true;
        }

        for bj in 1..dict.bucket_size() {
            if self.pos == dict.serialized.len() {
                break;
            }

            let (lcp, next_pos) = dict.decode_lcp(self.pos);
            self.pos = next_pos;
            dec.resize(lcp, 0);
            self.pos = dict.decode_next(self.pos, dec);

            if utils::is_prefix(self.key, &dec) {
                self.id += bj;
                return true;
            }
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str;

    #[test]
    fn test_toy() {
        let keys = [
            "deal",
            "idea",
            "ideal",
            "ideas",
            "ideology",
            "tea",
            "techie",
            "technology",
            "tie",
            "trie",
        ];

        assert!(FcBuilder::new(0).is_err());
        assert!(FcBuilder::new(3).is_err());
        let mut builder = FcBuilder::new(4).unwrap();

        for key in &keys {
            builder.add(key.as_bytes()).unwrap();
        }
        assert!(builder.add("tri".as_bytes()).is_err());
        assert!(builder.add(&[0xFF, 0x00]).is_err());

        let dict = FcDict::from_builder(builder);

        let mut locater = FcLocater::new(&dict);
        for i in 0..keys.len() {
            let id = locater.run(keys[i].as_bytes()).unwrap();
            assert_eq!(i, id);
        }

        assert!(locater.run("aaa".as_bytes()).is_none());
        assert!(locater.run("tell".as_bytes()).is_none());
        assert!(locater.run("techno".as_bytes()).is_none());
        assert!(locater.run("zzz".as_bytes()).is_none());

        let mut decoder = FcDecoder::new(&dict);
        for i in 0..keys.len() {
            let dec = decoder.run(i).unwrap();
            assert_eq!(keys[i], str::from_utf8(&dec).unwrap());
        }

        let mut iterator = FcIterator::new(&dict);
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(keys[i], str::from_utf8(&dec).unwrap());
        }
        assert!(iterator.next().is_none());

        let mut buffer = vec![];
        dict.serialize_into(&mut buffer).unwrap();
        assert_eq!(buffer.len(), dict.serialized_size_in_bytes());

        let other = FcDict::deserialize_from(&buffer[..]).unwrap();
        let mut iterator = FcIterator::new(&other);
        for i in 0..keys.len() {
            let (id, dec) = iterator.next().unwrap();
            assert_eq!(i, id);
            assert_eq!(keys[i], str::from_utf8(&dec).unwrap());
        }
        assert!(iterator.next().is_none());
    }
}
