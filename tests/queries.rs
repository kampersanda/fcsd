use std::iter::FusedIterator;

use fcsd::Set;

#[test]
fn locator_returns_lexicographic_ids_across_bucket_boundaries() {
    let set = Set::with_bucket_size(["deal", "idea", "ideal", "ideas", "tea"], 2).unwrap();
    let mut locator = set.locator();

    assert_eq!(locator.run(b"deal"), Some(0));
    assert_eq!(locator.run(b"idea"), Some(1));
    assert_eq!(locator.run(b"ideal"), Some(2));
    assert_eq!(locator.run(b"ideas"), Some(3));
    assert_eq!(locator.run(b"tea"), Some(4));
}

#[test]
fn locator_returns_none_for_keys_outside_and_between_stored_keys() {
    let set = Set::with_bucket_size(["deal", "idea", "ideal", "ideas", "tea"], 2).unwrap();
    let mut locator = set.locator();

    assert_eq!(locator.run(b"aaa"), None);
    assert_eq!(locator.run(b"ide"), None);
    assert_eq!(locator.run(b"ideals"), None);
    assert_eq!(locator.run(b"tell"), None);
    assert_eq!(locator.run(b"zzz"), None);
}

#[test]
fn locator_returns_none_for_an_empty_query() {
    let set = Set::new(["alpha"]).unwrap();

    assert_eq!(set.locator().run(b""), None);
}

#[test]
fn locator_returns_none_when_the_set_is_empty() {
    let set = Set::new(std::iter::empty::<&str>()).unwrap();

    assert_eq!(set.locator().run(b"alpha"), None);
}

#[test]
fn decoder_returns_each_key_by_its_lexicographic_id() {
    let set = Set::with_bucket_size(["deal", "idea", "ideal", "ideas", "tea"], 2).unwrap();
    let mut decoder = set.decoder();

    assert_eq!(decoder.run(0), b"deal");
    assert_eq!(decoder.run(1), b"idea");
    assert_eq!(decoder.run(2), b"ideal");
    assert_eq!(decoder.run(3), b"ideas");
    assert_eq!(decoder.run(4), b"tea");
}

#[test]
fn indexed_queries_support_non_utf8_keys() {
    let set = Set::new([b"\x01".as_slice(), b"\x80".as_slice(), b"\xff".as_slice()]).unwrap();
    let mut locator = set.locator();
    let mut decoder = set.decoder();

    assert_eq!(locator.run(b"\x80"), Some(1));
    assert_eq!(decoder.run(2), b"\xff");
}

#[test]
#[should_panic]
fn decoder_panics_when_id_equals_the_set_length() {
    let set = Set::new(["alpha"]).unwrap();

    set.decoder().run(1);
}

#[test]
fn iterator_yields_ids_and_keys_in_lexicographic_order() {
    let set = Set::with_bucket_size(["deal", "idea", "ideal", "ideas", "tea"], 2).unwrap();

    let entries = set.iter().collect::<Vec<_>>();

    assert_eq!(
        entries,
        vec![
            (0, b"deal".to_vec()),
            (1, b"idea".to_vec()),
            (2, b"ideal".to_vec()),
            (3, b"ideas".to_vec()),
            (4, b"tea".to_vec()),
        ]
    );
}

#[test]
fn iterator_yields_all_keys_when_each_bucket_contains_one_key() {
    let set = Set::with_bucket_size(["alpha", "beta", "gamma"], 1).unwrap();

    let entries = set.iter().collect::<Vec<_>>();

    assert_eq!(
        entries,
        vec![
            (0, b"alpha".to_vec()),
            (1, b"beta".to_vec()),
            (2, b"gamma".to_vec()),
        ]
    );
}

#[test]
fn iterator_length_tracks_the_remaining_entries() {
    let set = Set::new(["alpha", "beta", "gamma"]).unwrap();
    let mut iter = set.iter();

    assert_eq!(iter.len(), 3);

    iter.next();
    assert_eq!(iter.len(), 2);

    iter.next();
    iter.next();
    assert_eq!(iter.len(), 0);
}

#[test]
fn iterator_stays_exhausted_after_returning_none() {
    let set = Set::new(["alpha"]).unwrap();
    let mut iter = set.iter();
    iter.next();

    assert_eq!(iter.next(), None);
    assert_eq!(iter.next(), None);
    assert_eq!(iter.len(), 0);
}

#[test]
fn iterator_implements_fused_iterator() {
    fn require_fused_iterator<I: FusedIterator>(_iter: I) {}

    let set = Set::new(["alpha"]).unwrap();

    require_fused_iterator(set.iter());
}

#[test]
fn predictive_iterator_yields_only_keys_with_the_prefix() {
    let set =
        Set::with_bucket_size(["deal", "idea", "ideal", "ideas", "ideology", "tea"], 2).unwrap();

    let entries = set.predictive_iter(b"idea").collect::<Vec<_>>();

    assert_eq!(
        entries,
        vec![
            (1, b"idea".to_vec()),
            (2, b"ideal".to_vec()),
            (3, b"ideas".to_vec()),
        ]
    );
}

#[test]
fn predictive_iterator_with_empty_prefix_yields_the_entire_set() {
    let set = Set::new(["alpha", "beta", "gamma"]).unwrap();

    let entries = set.predictive_iter(b"").collect::<Vec<_>>();

    assert_eq!(
        entries,
        vec![
            (0, b"alpha".to_vec()),
            (1, b"beta".to_vec()),
            (2, b"gamma".to_vec()),
        ]
    );
}

#[test]
fn predictive_iterator_returns_no_entries_when_prefix_is_absent() {
    let set = Set::new(["alpha", "beta", "gamma"]).unwrap();

    assert_eq!(set.predictive_iter(b"delta").collect::<Vec<_>>(), vec![]);
}

#[test]
fn predictive_iterator_returns_no_entries_when_the_set_is_empty() {
    let set = Set::new(std::iter::empty::<&str>()).unwrap();

    assert_eq!(set.predictive_iter(b"").collect::<Vec<_>>(), vec![]);
    assert_eq!(set.predictive_iter(b"alpha").collect::<Vec<_>>(), vec![]);
}

#[test]
fn resetting_predictive_iterator_restarts_search_with_the_new_prefix() {
    let set = Set::new(["alpha", "alpine", "beta"]).unwrap();
    let mut iter = set.predictive_iter(b"al");
    assert_eq!(iter.next(), Some((0, b"alpha".to_vec())));

    iter.reset(b"be");
    let entries = iter.collect::<Vec<_>>();

    assert_eq!(entries, vec![(2, b"beta".to_vec())]);
}
