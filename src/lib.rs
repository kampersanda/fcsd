mod utils;

const END_MARKER: u8 = 0;

pub struct FcDict {
    pointers: Vec<usize>,
    serialized: Vec<u8>,
    num_keys: usize,
    bucket_size: usize,
    max_length: usize,
}

impl FcDict {
    pub fn from_builder(builder: FcBuilder) -> FcDict {
        FcDict {
            pointers: builder.pointers,
            serialized: builder.serialized,
            num_keys: builder.num_keys,
            bucket_size: builder.bucket_size,
            max_length: builder.max_length,
        }
    }

    pub fn num_keys(&self) -> usize {
        self.num_keys
    }

    pub fn num_buckets(&self) -> usize {
        self.pointers.len()
    }

    pub fn bucket_size(&self) -> usize {
        self.bucket_size
    }

    pub fn max_length(&self) -> usize {
        self.max_length
    }

    fn bucket_id(&self, id: usize) -> usize {
        id / self.bucket_size
    }

    fn pos_in_bucket(&self, id: usize) -> usize {
        id % self.bucket_size
    }

    fn get_header(&self, bi: usize) -> &[u8] {
        let header = &self.serialized[self.pointers[bi]..];
        &header[..utils::get_strlen(&header)]
    }

    fn decode_header(&self, bi: usize, dec: &mut Vec<u8>) -> usize {
        dec.clear();

        let mut pos = self.pointers[bi];
        while self.serialized[pos] != END_MARKER {
            dec.push(self.serialized[pos]);
            pos += 1;
        }
        pos + 1
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
        if cmp < 0 {
            (mi, false)
        } else {
            (mi - 1, false)
        }
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
            let (lcp, num) = utils::vbyte::decode(&dict.serialized[pos..]);
            pos += num;

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

pub struct FcBuilder {
    pointers: Vec<usize>,
    serialized: Vec<u8>,
    last_key: Vec<u8>,
    num_keys: usize,
    bucket_size: usize,
    max_length: usize,
}

impl FcBuilder {
    pub fn new(bucket_size: usize) -> FcBuilder {
        FcBuilder {
            pointers: Vec::new(),
            serialized: Vec::new(),
            last_key: Vec::new(),
            num_keys: 0,
            bucket_size: bucket_size,
            max_length: 0,
        }
    }

    pub fn add(&mut self, key: &[u8]) -> bool {
        let (lcp, cmp) = utils::get_lcp(&self.last_key, key);
        if cmp <= 0 {
            return false;
        }

        if self.num_keys % self.bucket_size == 0 {
            self.pointers.push(self.serialized.len());
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

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str;

    #[test]
    fn test_toy() {
        let keys = [
            "ideal",
            "ideas",
            "ideology",
            "tea",
            "techie",
            "technology",
            "tie",
            "trie",
        ];

        let mut builder = FcBuilder::new(4);
        for key in &keys {
            builder.add(key.as_bytes());
        }

        let dict = FcDict::from_builder(builder);

        let mut locater = FcLocater::new(&dict);
        for i in 0..keys.len() {
            let id = locater.run(keys[i].as_bytes()).unwrap();
            println!("{}: {}", i, id);
        }

        let mut decoder = FcDecoder::new(&dict);
        for i in 0..keys.len() {
            let dec = decoder.run(i).unwrap();
            println!("{}: {}", i, str::from_utf8(&dec).unwrap());
        }
    }
}
