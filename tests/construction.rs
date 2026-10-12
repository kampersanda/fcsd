use fcsd::builder::Builder;
use fcsd::{FcsdError, Set, DEFAULT_BUCKET_SIZE};

#[test]
fn new_with_sorted_unique_keys_reports_its_shape() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();

    assert_eq!(set.len(), 3);
    assert!(!set.is_empty());
    assert_eq!(set.bucket_size(), DEFAULT_BUCKET_SIZE);
    assert_eq!(set.num_buckets(), 1);
}

#[test]
fn new_with_no_keys_creates_an_empty_set() {
    let set = Set::new(std::iter::empty::<&str>()).unwrap();

    assert_eq!(set.len(), 0);
    assert!(set.is_empty());
    assert_eq!(set.bucket_size(), DEFAULT_BUCKET_SIZE);
    assert_eq!(set.num_buckets(), 0);
}

#[test]
fn custom_bucket_size_controls_bucket_partitioning() {
    let set = Set::with_bucket_size(["a", "b", "c", "d", "e"], 2).unwrap();

    assert_eq!(set.bucket_size(), 2);
    assert_eq!(set.num_buckets(), 3);
}

#[test]
fn zero_bucket_size_is_rejected() {
    let result = Set::with_bucket_size(["alpha"], 0);

    assert!(matches!(result, Err(FcsdError::BucketSizeZero)));
}

#[test]
fn non_power_of_two_bucket_size_is_rejected() {
    let result = Set::with_bucket_size(["alpha"], 3);

    assert!(matches!(result, Err(FcsdError::BucketSizeNotPowerOfTwo)));
}

#[test]
fn duplicate_keys_are_rejected() {
    let result = Set::new(["alpha", "alpha"]);

    assert!(matches!(result, Err(FcsdError::KeysNotSortedAndUnique)));
}

#[test]
fn descending_keys_are_rejected() {
    let result = Set::new(["beta", "alpha"]);

    assert!(matches!(result, Err(FcsdError::KeysNotSortedAndUnique)));
}

#[test]
fn empty_key_is_rejected() {
    let result = Set::new([""]);

    assert!(matches!(result, Err(FcsdError::KeysNotSortedAndUnique)));
}

#[test]
fn key_containing_end_marker_is_rejected() {
    let result = Set::new([b"alpha\0beta".as_slice()]);

    assert!(matches!(result, Err(FcsdError::KeyContainsEndMarker)));
}

#[test]
fn builder_remains_usable_after_rejecting_a_key() {
    let mut builder = Builder::new(2).unwrap();
    builder.add(b"alpha").unwrap();
    let duplicate = builder.add(b"alpha");

    builder.add(b"beta").unwrap();
    let set = builder.finish();

    assert!(matches!(duplicate, Err(FcsdError::KeysNotSortedAndUnique)));
    assert_eq!(
        set.iter().collect::<Vec<_>>(),
        vec![(0, b"alpha".to_vec()), (1, b"beta".to_vec())]
    );
}
