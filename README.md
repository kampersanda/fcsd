# Front-coding string dictionary in Rust

![](https://github.com/kampersanda/fcsd/actions/workflows/rust.yml/badge.svg)
[![Documentation](https://docs.rs/fcsd/badge.svg)](https://docs.rs/fcsd)
[![Crates.io](https://img.shields.io/crates/v/fcsd.svg)](https://crates.io/crates/fcsd)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/kampersanda/fcsd/blob/master/LICENSE)


This is a Rust library of the (plain) front-coding string dictionary described in [*Martínez-Prieto et al., Practical compressed string dictionaries, INFOSYS 2016*](https://doi.org/10.1016/j.is.2015.08.008).

[Japanese description](https://kampersanda.hatenablog.jp/entry/2021/09/29/123644)

## Features

- **Dictionary encoding.** Fcsd provides a bijective mapping between strings and integer IDs. It is so-called *dictionary encoding* and useful for text compression in many applications.
- **Simple and fast compression.** Fcsd maintains a set of strings in a compressed space through *front-coding*, a differential compression technique for strings, allowing for fast decompression operations.
- **Random access.** Fcsd maintains strings through a bucketization technique enabling to directly decompress arbitrary strings and perform binary search for strings.

## Example

```rust
use fcsd::Set;

// Input string keys should be sorted and unique.
let keys = ["ICDM", "ICML", "SIGIR", "SIGKDD", "SIGMOD"];

// Builds the dictionary.
let set = Set::new(keys).unwrap();
assert_eq!(set.len(), keys.len());

// Locates IDs associated with given keys.
let mut locator = set.locator();
assert_eq!(locator.run(b"ICML"), Some(1));
assert_eq!(locator.run(b"SIGMOD"), Some(4));
assert_eq!(locator.run(b"SIGSPATIAL"), None);

// Decodes string keys associated with given IDs.
let mut decoder = set.decoder();
assert_eq!(decoder.run(0), b"ICDM".to_vec());
assert_eq!(decoder.run(3), b"SIGKDD".to_vec());

// Enumerates string keys starting with a prefix.
let mut iter = set.prefix_iter(b"SIG");
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

## Note

- Input keys must not contain `\0` character because the character is used for the string delimiter.
- The bucket size of 8 is recommended in space-time tradeoff by Martínez-Prieto's paper.

## Todo

- Add benchmarking codes.
- Add RePair compressed veriants.

## Licensing

This library is free software provided under MIT.

