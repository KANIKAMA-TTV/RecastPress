use recastpress_db::{
    PostMetaRow,
    meta_view::{PostMetaView, SearchPlan},
};

fn row(id: u64, key: Option<&[u8]>) -> PostMetaRow {
    PostMetaRow {
        meta_id: id,
        post_id: u64::MAX,
        meta_key: key.map(<[u8]>::to_vec),
        // Values are not index-key charges.
        meta_value: Some(b"\0\xffvalue".to_vec()),
    }
}

fn offsets(count: usize) -> usize {
    count * std::mem::size_of::<usize>()
}

fn key_entry(length: usize) -> usize {
    std::mem::size_of::<(Vec<u8>, Vec<usize>)>() + length
}

fn assert_matches_scan(view: &PostMetaView, plan: SearchPlan) {
    for key in [
        None,
        Some(b"".as_slice()),
        Some(b"A".as_slice()),
        Some(b"a".as_slice()),
        Some(b"\0\xff".as_slice()),
        Some(b"absent".as_slice()),
    ] {
        let result = view.matching_key(key);
        assert_eq!(result.plan(), plan);
        let result: Vec<_> = result.collect();
        let expected: Vec<_> = view
            .rows()
            .iter()
            .filter(|r| r.meta_key.as_deref() == key)
            .collect();
        assert_eq!(result, expected);
    }
}

#[test]
fn exact_minus_one_zero_and_maximum_allowances_obey_both_limits() {
    let rows = vec![row(0, Some(b"A")), row(u64::MAX, None)];
    let mut view = PostMetaView::new(rows.clone()).unwrap();
    let exact = offsets(2) + key_entry(1);
    for limit in [exact, usize::MAX] {
        assert_eq!(
            view.build_key_index_with_byte_limit(2, limit),
            SearchPlan::ExactKeyIndex
        );
        assert_matches_scan(&view, SearchPlan::ExactKeyIndex);
    }
    for limit in [0, exact - 1] {
        assert_eq!(
            view.build_key_index_with_byte_limit(usize::MAX, limit),
            SearchPlan::Scan
        );
        assert_matches_scan(&view, SearchPlan::Scan);
    }
    for entries in [0, 1] {
        assert_eq!(
            view.build_key_index_with_byte_limit(entries, usize::MAX),
            SearchPlan::Scan
        );
    }
    assert_eq!(view.rows(), rows);
}

#[test]
fn null_offsets_and_empty_key_entries_are_distinct() {
    let mut nulls = PostMetaView::new(vec![row(0, None), row(1, None)]).unwrap();
    assert_eq!(
        nulls.build_key_index_with_byte_limit(2, offsets(2)),
        SearchPlan::ExactKeyIndex
    );
    assert_eq!(nulls.matching_key(None).count(), 2);
    assert_eq!(nulls.matching_key(Some(b"")).count(), 0);
    assert_eq!(
        nulls.build_key_index_with_byte_limit(2, offsets(2) - 1),
        SearchPlan::Scan
    );

    let mut mixed = PostMetaView::new(vec![row(0, None), row(1, Some(b""))]).unwrap();
    let exact = offsets(2) + key_entry(0);
    assert_eq!(
        mixed.build_key_index_with_byte_limit(2, exact - 1),
        SearchPlan::Scan
    );
    assert_eq!(
        mixed.build_key_index_with_byte_limit(2, exact),
        SearchPlan::ExactKeyIndex
    );
    assert_eq!(
        mixed
            .matching_key(None)
            .map(|r| r.meta_id)
            .collect::<Vec<_>>(),
        [0]
    );
    assert_eq!(
        mixed
            .matching_key(Some(b""))
            .map(|r| r.meta_id)
            .collect::<Vec<_>>(),
        [1]
    );
}

#[test]
fn repeated_key_bytes_are_charged_once_but_every_offset_is_charged() {
    let mut view = PostMetaView::new(vec![
        row(0, Some(b"\0\xff")),
        row(1, Some(b"\0\xff")),
        row(2, Some(b"\0\xff")),
        row(u64::MAX, None),
    ])
    .unwrap();
    let exact = offsets(4) + key_entry(2);
    assert_eq!(
        view.build_key_index_with_byte_limit(4, exact),
        SearchPlan::ExactKeyIndex
    );
    assert_matches_scan(&view, SearchPlan::ExactKeyIndex);
    // Omitting a single repeated-row or NULL offset is insufficient.
    assert_eq!(
        view.build_key_index_with_byte_limit(4, exact - std::mem::size_of::<usize>()),
        SearchPlan::Scan
    );
}

#[test]
fn binary_case_sensitive_keys_preserve_id_order_through_refusal_and_rebuild() {
    let mut view = PostMetaView::new(vec![
        row(u64::MAX, Some(b"\0\xff")),
        row(3, Some(b"A")),
        row(2, Some(b"a")),
        row(0, Some(b"\0\xff")),
    ])
    .unwrap();
    let original = view.rows().to_vec();
    let exact = offsets(4) + key_entry(2) + 2 * key_entry(1);
    assert_eq!(
        view.build_key_index_with_byte_limit(4, exact),
        SearchPlan::ExactKeyIndex
    );
    assert_eq!(
        view.matching_key(Some(b"\0\xff"))
            .map(|r| r.meta_id)
            .collect::<Vec<_>>(),
        [0, u64::MAX]
    );
    assert_matches_scan(&view, SearchPlan::ExactKeyIndex);
    assert_eq!(
        view.build_key_index_with_byte_limit(3, usize::MAX),
        SearchPlan::Scan
    );
    assert_matches_scan(&view, SearchPlan::Scan);
    assert_eq!(
        view.build_key_index_with_byte_limit(4, exact),
        SearchPlan::ExactKeyIndex
    );
    assert_eq!(
        view.build_key_index_with_byte_limit(4, exact - 1),
        SearchPlan::Scan
    );
    assert_matches_scan(&view, SearchPlan::Scan);
    assert_eq!(view.rows(), original);
    // The original unbounded-byte API remains usable after refusal.
    assert_eq!(view.build_key_index(4), SearchPlan::ExactKeyIndex);
    assert_matches_scan(&view, SearchPlan::ExactKeyIndex);
    assert_eq!(view.rows(), original);
}

#[test]
fn empty_view_builds_with_zero_allowances_and_no_inline_root_charge() {
    let mut view = PostMetaView::new(vec![]).unwrap();
    for _ in 0..2 {
        assert_eq!(
            view.build_key_index_with_byte_limit(0, 0),
            SearchPlan::ExactKeyIndex
        );
        assert_matches_scan(&view, SearchPlan::ExactKeyIndex);
        view.discard_index();
        assert_matches_scan(&view, SearchPlan::Scan);
    }
    assert!(view.rows().is_empty());
}
