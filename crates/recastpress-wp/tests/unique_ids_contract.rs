use recastpress_wp::UniqueIds;

#[test]
fn explicit_unique_id_contexts_start_independently_and_have_no_shared_global_counter() {
    let mut first = UniqueIds::new();
    let mut second = UniqueIds::default();
    assert_eq!(first.wp_unique_id(b"p-").unwrap(), b"p-1");
    assert_eq!(first.wp_unique_id(b"p-").unwrap(), b"p-2");
    assert_eq!(second.wp_unique_id(b"p-").unwrap(), b"p-1");
    assert_eq!(first.wp_unique_prefixed_id(b"p-").unwrap(), b"p-1");
    assert_eq!(first.wp_unique_prefixed_id(b"p-").unwrap(), b"p-2");
    assert_eq!(second.wp_unique_prefixed_id(b"p-").unwrap(), b"p-1");
}

#[test]
fn global_and_per_prefix_counters_are_independent_under_interleaved_calls() {
    let mut ids = UniqueIds::new();
    assert_eq!(ids.wp_unique_id(b"a").unwrap(), b"a1");
    assert_eq!(ids.wp_unique_prefixed_id(b"a").unwrap(), b"a1");
    assert_eq!(ids.wp_unique_id(b"b").unwrap(), b"b2");
    assert_eq!(ids.wp_unique_prefixed_id(b"b").unwrap(), b"b1");
    assert_eq!(ids.wp_unique_prefixed_id(b"a").unwrap(), b"a2");
    assert_eq!(ids.wp_unique_id(b"").unwrap(), b"3");
    assert_eq!(ids.wp_unique_prefixed_id(b"").unwrap(), b"1");
    assert_eq!(ids.wp_unique_id(b"a").unwrap(), b"a4");
}

#[test]
fn per_prefix_map_preserves_numeric_case_empty_and_binary_prefix_identity() {
    let mut ids = UniqueIds::new();
    let prefixes: &[&[u8]] = &[
        b"", b"0", b"00", b"1", b"-0", b"01", b"A", b"a", b"\0", b"\xff", b"x\0\xff",
    ];
    for prefix in prefixes {
        let before = prefix.to_vec();
        assert_eq!(
            ids.wp_unique_prefixed_id(prefix).unwrap(),
            [*prefix, b"1"].concat()
        );
        assert_eq!(*prefix, before);
    }
    for prefix in prefixes.iter().rev() {
        assert_eq!(
            ids.wp_unique_prefixed_id(prefix).unwrap(),
            [*prefix, b"2"].concat()
        );
    }
    for byte in 0..=255u8 {
        let prefix = [b'p', byte, 0xff];
        assert_eq!(
            ids.wp_unique_prefixed_id(&prefix).unwrap(),
            [&prefix[..], b"1"].concat()
        );
        assert_eq!(
            ids.wp_unique_prefixed_id(&prefix).unwrap(),
            [&prefix[..], b"2"].concat()
        );
    }
}

#[test]
fn concatenated_prefix_ids_can_collide_and_are_not_universal_identifiers() {
    let mut ids = UniqueIds::new();
    let prefixed_first = ids.wp_unique_prefixed_id(b"a1").unwrap();
    assert_eq!(prefixed_first, b"a11");
    for n in 1..=10 {
        assert_eq!(
            ids.wp_unique_prefixed_id(b"a").unwrap(),
            format!("a{n}").as_bytes()
        );
    }
    assert_eq!(ids.wp_unique_prefixed_id(b"a").unwrap(), prefixed_first);
    let global_first = ids.wp_unique_id(b"a1").unwrap();
    for _ in 2..=10 {
        ids.wp_unique_id(b"unused").unwrap();
    }
    assert_eq!(ids.wp_unique_id(b"a").unwrap(), global_first);
}
