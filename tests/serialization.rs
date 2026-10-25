use fcsd::{FcsdError, Result, Set};

fn header() -> Vec<u8> {
    let mut data = b"FCSD".to_vec();
    data.extend_from_slice(&fcsd::FORMAT_VERSION.to_le_bytes());
    data
}

fn push_u64(data: &mut Vec<u8>, value: u64) {
    data.extend_from_slice(&value.to_le_bytes());
}

struct RawSet<'a> {
    pointer_chunks: &'a [u64],
    pointer_count: usize,
    pointer_bits: usize,
    pointer_mask: u64,
    keys: &'a [u8],
    key_count: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_key_length: usize,
}

fn raw_set(raw: RawSet<'_>) -> Vec<u8> {
    let mut data = header();
    push_u64(&mut data, raw.pointer_chunks.len() as u64);
    for &chunk in raw.pointer_chunks {
        push_u64(&mut data, chunk);
    }
    push_u64(&mut data, raw.pointer_count as u64);
    push_u64(&mut data, raw.pointer_bits as u64);
    push_u64(&mut data, raw.pointer_mask);
    push_u64(&mut data, raw.keys.len() as u64);
    data.extend_from_slice(raw.keys);
    push_u64(&mut data, raw.key_count as u64);
    push_u64(&mut data, raw.bucket_bits as u64);
    push_u64(&mut data, raw.bucket_mask as u64);
    push_u64(&mut data, raw.max_key_length as u64);
    data
}

fn one_bucket_set(
    keys: &[u8],
    key_count: usize,
    bucket_bits: usize,
    bucket_mask: usize,
    max_key_length: usize,
) -> Vec<u8> {
    raw_set(RawSet {
        pointer_chunks: &[0],
        pointer_count: 1,
        pointer_bits: 1,
        pointer_mask: 1,
        keys,
        key_count,
        bucket_bits,
        bucket_mask,
        max_key_length,
    })
}

fn assert_invalid_data(result: Result<Set>) {
    match result {
        Err(FcsdError::InvalidData(_)) => {}
        Err(error) => panic!("expected InvalidData, got {}", error),
        Ok(_) => panic!("expected malformed data to be rejected"),
    }
}

#[test]
fn serialization_size_matches_the_number_of_written_bytes() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let mut data = Vec::new();

    set.serialize_into(&mut data).unwrap();

    assert_eq!(data.len(), set.size_in_bytes());
}

#[test]
fn deserialization_preserves_the_observable_set() {
    let set = Set::with_bucket_size(["alpha", "alpine", "beta", "gamma"], 2).unwrap();
    let mut data = Vec::new();
    set.serialize_into(&mut data).unwrap();

    let restored = Set::deserialize_from(data.as_slice()).unwrap();

    assert_eq!(restored.len(), 4);
    assert_eq!(restored.bucket_size(), 2);
    assert_eq!(restored.num_buckets(), 2);
    assert_eq!(
        restored.iter().collect::<Vec<_>>(),
        vec![
            (0, b"alpha".to_vec()),
            (1, b"alpine".to_vec()),
            (2, b"beta".to_vec()),
            (3, b"gamma".to_vec()),
        ]
    );
}

#[test]
fn empty_set_survives_serialization_round_trip() {
    let set = Set::new(std::iter::empty::<&str>()).unwrap();
    let mut data = Vec::new();
    set.serialize_into(&mut data).unwrap();

    let restored = Set::deserialize_from(data.as_slice()).unwrap();

    assert!(restored.is_empty());
    assert_eq!(restored.iter().collect::<Vec<_>>(), vec![]);
}

#[test]
fn serialized_data_starts_with_magic_and_format_version() {
    let set = Set::new(["alpha"]).unwrap();
    let mut data = Vec::new();

    set.serialize_into(&mut data).unwrap();

    assert_eq!(data[..8], header()[..]);
}

#[test]
fn deserialization_rejects_unknown_magic() {
    let data = [0_u8; 8];

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_unsupported_format_version() {
    let mut data = b"FCSD".to_vec();
    data.extend_from_slice(&(fcsd::FORMAT_VERSION + 1).to_le_bytes());

    let result = Set::deserialize_from(data.as_slice());

    assert!(matches!(
        result,
        Err(FcsdError::UnsupportedFormatVersion(v)) if v == fcsd::FORMAT_VERSION + 1
    ));
}

#[test]
fn deserialization_rejects_a_declared_allocation_above_the_limit() {
    let mut data = header();
    push_u64(&mut data, 32 * 1024 * 1024);

    assert_invalid_data(Set::deserialize_from_with_limit(data.as_slice(), 1024));
}

#[test]
fn deserialization_accepts_data_at_the_allocation_limit() {
    let data = one_bucket_set(b"a\0", 1, 3, 7, 1);

    let restored = Set::deserialize_from_with_limit(data.as_slice(), 10).unwrap();

    assert_eq!(
        restored.iter().collect::<Vec<_>>(),
        vec![(0, b"a".to_vec())]
    );
}

#[test]
fn deserialization_rejects_data_one_byte_over_the_allocation_limit() {
    let data = one_bucket_set(b"a\0", 1, 3, 7, 1);

    assert_invalid_data(Set::deserialize_from_with_limit(data.as_slice(), 9));
}

#[test]
fn deserialization_rejects_bucket_width_that_disagrees_with_its_mask() {
    let data = one_bucket_set(b"a\0", 1, 2, 7, 1);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_pointer_bit_width_of_zero() {
    let data = raw_set(RawSet {
        pointer_chunks: &[0],
        pointer_count: 1,
        pointer_bits: 0,
        pointer_mask: 0,
        keys: b"a\0",
        key_count: 1,
        bucket_bits: 3,
        bucket_mask: 7,
        max_key_length: 1,
    });

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_too_few_pointer_chunks() {
    let data = raw_set(RawSet {
        pointer_chunks: &[0],
        pointer_count: 65,
        pointer_bits: 1,
        pointer_mask: 1,
        keys: b"a\0",
        key_count: 1,
        bucket_bits: 3,
        bucket_mask: 7,
        max_key_length: 1,
    });

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_missing_bucket_pointer() {
    let data = raw_set(RawSet {
        pointer_chunks: &[],
        pointer_count: 0,
        pointer_bits: 1,
        pointer_mask: 1,
        keys: b"a\0",
        key_count: 1,
        bucket_bits: 3,
        bucket_mask: 7,
        max_key_length: 1,
    });

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_bucket_pointer_to_the_wrong_offset() {
    let data = raw_set(RawSet {
        pointer_chunks: &[1],
        pointer_count: 1,
        pointer_bits: 1,
        pointer_mask: 1,
        keys: b"a\0",
        key_count: 1,
        bucket_bits: 3,
        bucket_mask: 7,
        max_key_length: 1,
    });

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_key_without_an_end_marker() {
    let data = one_bucket_set(b"alpha", 1, 3, 7, 5);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_front_coded_prefix_longer_than_the_previous_key() {
    let data = one_bucket_set(b"a\0\x02b\0", 2, 3, 7, 1);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_empty_key() {
    let data = one_bucket_set(b"\0", 1, 3, 7, 0);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_duplicate_keys() {
    let data = one_bucket_set(b"a\0\x00a\0", 2, 3, 7, 1);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_keys_in_descending_order() {
    let data = one_bucket_set(b"b\0\x00a\0", 2, 3, 7, 1);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_descending_keys_across_bucket_boundaries() {
    let data = raw_set(RawSet {
        pointer_chunks: &[8],
        pointer_count: 2,
        pointer_bits: 2,
        pointer_mask: 3,
        keys: b"b\0a\0",
        key_count: 2,
        bucket_bits: 0,
        bucket_mask: 0,
        max_key_length: 1,
    });

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_overflowing_front_coded_prefix() {
    let data = one_bucket_set(
        b"a\0\xff\xff\xff\xff\xff\xff\xff\xff\xff\xff\x01\0",
        2,
        3,
        7,
        1,
    );

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_accepts_noncanonical_front_coded_prefix() {
    let data = one_bucket_set(b"a\0\x80\x00b\0", 2, 3, 7, 1);

    let restored = Set::deserialize_from(data.as_slice()).unwrap();

    assert_eq!(
        restored.iter().collect::<Vec<_>>(),
        vec![(0, b"a".to_vec()), (1, b"b".to_vec())]
    );
}

#[test]
fn deserialization_rejects_unparsed_trailing_bytes() {
    let data = one_bucket_set(b"a\0x", 1, 3, 7, 1);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_incorrect_maximum_key_length() {
    let data = one_bucket_set(b"a\0", 1, 3, 7, 2);

    assert_invalid_data(Set::deserialize_from(data.as_slice()));
}

#[test]
fn deserialization_rejects_a_truncated_header() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let mut data = Vec::new();
    set.serialize_into(&mut data).unwrap();

    let result = Set::deserialize_from(&data[..3]);

    assert!(matches!(result, Err(FcsdError::Io(_))));
}

#[test]
fn deserialization_rejects_a_truncated_key_payload() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let mut data = Vec::new();
    set.serialize_into(&mut data).unwrap();

    let result = Set::deserialize_from(&data[..data.len() / 2]);

    assert!(result.is_err());
}

#[test]
fn deserialization_rejects_a_truncated_metadata_footer() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let mut data = Vec::new();
    set.serialize_into(&mut data).unwrap();

    let result = Set::deserialize_from(&data[..data.len() - 1]);

    assert!(matches!(result, Err(FcsdError::Io(_))));
}

#[test]
fn from_bytes_queries_serialized_data_in_place() {
    let set = Set::with_bucket_size(["alpha", "alpine", "beta", "gamma"], 2).unwrap();
    let mut data = Vec::new();
    set.serialize_into(&mut data).unwrap();

    let view = Set::from_bytes(data.as_slice()).unwrap();

    assert_eq!(view.as_bytes().as_ptr(), data.as_ptr());
    assert_eq!(view.size_in_bytes(), data.len());
    assert_eq!(view.locator().run("beta"), Some(2));
    assert_eq!(view.decoder().run(3), b"gamma".to_vec());
    assert_eq!(
        view.predictive_iter("al").collect::<Vec<_>>(),
        vec![(0, b"alpha".to_vec()), (1, b"alpine".to_vec())]
    );
}

#[test]
fn from_bytes_accepts_unaligned_data() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let mut buffer = vec![0];
    set.serialize_into(&mut buffer).unwrap();

    let view = Set::from_bytes(&buffer[1..]).unwrap();

    assert_eq!(
        view.iter().collect::<Vec<_>>(),
        set.iter().collect::<Vec<_>>()
    );
}

#[test]
fn from_bytes_rejects_trailing_bytes() {
    let set = Set::new(["alpha"]).unwrap();
    let mut data = set.as_bytes().to_vec();
    data.push(0);

    assert_invalid_data(Set::from_bytes(data));
}

#[test]
fn from_bytes_rejects_truncated_data() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let data = set.as_bytes();

    assert_invalid_data(Set::from_bytes(data[..data.len() - 1].to_vec()));
}

#[test]
fn from_bytes_rejects_invalid_keys() {
    let data = one_bucket_set(b"b\0\x00a\0", 2, 3, 7, 1);

    assert_invalid_data(Set::from_bytes(data));
}

#[test]
fn from_bytes_trusted_skips_key_validation_but_checks_metadata() {
    let unsorted = one_bucket_set(b"b\0\x00a\0", 2, 3, 7, 1);
    let bad_bucket = one_bucket_set(b"a\0", 1, 2, 7, 1);

    assert!(Set::from_bytes_trusted(unsorted).is_ok());
    assert_invalid_data(Set::from_bytes_trusted(bad_bucket));
}
