# Front-coded string dictionary: Fast and compact indexed string set

![](https://github.com/kampersanda/fcsd/actions/workflows/rust.yml/badge.svg)
[![Documentation](https://docs.rs/fcsd/badge.svg)](https://docs.rs/fcsd)
[![Crates.io](https://img.shields.io/crates/v/fcsd.svg)](https://crates.io/crates/fcsd)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/kampersanda/fcsd/blob/master/LICENSE)

This is a Rust library to store an indexed set of strings and support fast queires.
The data structure is a plain front-coded string dictionary described in [*Martínez-Prieto et al., Practical compressed string dictionaries, INFOSYS 2016*](https://doi.org/10.1016/j.is.2015.08.008).

[Japanese description](https://kampersanda.hatenablog.jp/entry/2021/09/29/123644)

## Features

- **Indexed set.** Fcsd implements an indexed set of strings in a compressed format. `n` strings in the set are indexed with integers from `[0..n-1]` and assigned in the lexicographical order.
- **Simple and fast compression/decompression.** Fcsd maintains a set of strings in a compressed space through *front coding*, a differential compression technique for strings, allowing for fast decompression operations.
- **Random access.** Fcsd maintains strings through a bucketization technique enabling to directly decompress arbitrary strings and perform binary search for strings.

## Example

```rust
use fcsd::Set;

// Input string keys should be sorted and unique.
let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];

// Builds an indexed set.
let set = Set::new(keys).unwrap();
assert_eq!(set.len(), keys.len());

// Gets indexes associated with given keys.
let mut locator = set.locator();
assert_eq!(locator.run(b"ICML"), Some(1));
assert_eq!(locator.run(b"SIGMOD"), Some(4));
assert_eq!(locator.run(b"SIGSPATIAL"), None);

// Decodes string keys from given indexes.
let mut decoder = set.decoder();
assert_eq!(decoder.run(0), b"ICDM".to_vec());
assert_eq!(decoder.run(3), b"SIGKDD".to_vec());

// Enumerates indexes and keys stored in the set.
let mut iter = set.iter();
assert_eq!(iter.next(), Some((0, b"ICDM".to_vec())));
assert_eq!(iter.next(), Some((1, b"ICML".to_vec())));
assert_eq!(iter.next(), Some((2, b"SIGIR".to_vec())));
assert_eq!(iter.next(), Some((3, b"SIGKDD".to_vec())));
assert_eq!(iter.next(), Some((4, b"SIGMOD".to_vec())));
assert_eq!(iter.next(), None);

// Enumerates indexes and keys starting with a prefix.
let mut iter = set.predictive_iter(b"SIG");
assert_eq!(iter.next(), Some((2, b"SIGIR".to_vec())));
assert_eq!(iter.next(), Some((3, b"SIGKDD".to_vec())));
assert_eq!(iter.next(), Some((4, b"SIGMOD".to_vec())));
assert_eq!(iter.next(), None);

// Serialization / Deserialization
let mut data = Vec::<u8>::new();
set.serialize_into(&mut data).unwrap();
assert_eq!(data.len(), set.size_in_bytes());
let other = Set::deserialize_from(&data[..]).unwrap();
assert_eq!(data.len(), other.size_in_bytes());
```

## Todo

- Add benchmarking codes.
- Add RePair compressed veriants.

## Licensing

This library is free software provided under MIT.

