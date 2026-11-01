use recastpress_db::{
    PostMetaRow,
    meta_view::{DuplicateMetaId, PostMetaView, SearchPlan},
};

fn row(id: u64, key: Option<&[u8]>) -> PostMetaRow {
    PostMetaRow {
        meta_id: id,
        post_id: u64::MAX,
        meta_key: key.map(<[u8]>::to_vec),
        meta_value: Some(b"secret\0\xff".to_vec()),
    }
}

#[test]
fn optional_index_matches_scan_preserving_raw_rows_and_order() {
    let rows = vec![
        row(u64::MAX, Some(b"duplicate")),
        row(2, Some(b"")),
        row(1, None),
        row(0, Some(b"duplicate")),
        row(9, Some(b"Duplicate")),
        row(8, Some(b"duplicate ")),
        row(7, Some(b"\0\xff")),
        row(6, Some(b"duplicate")),
    ];
    let mut view = PostMetaView::new(rows).unwrap();
    let keys: [Option<&[u8]>; 8] = [
        None,
        Some(b""),
        Some(b"duplicate"),
        Some(b"Duplicate"),
        Some(b"duplicate "),
        Some(b"\0\xff"),
        Some(b"missing"),
        Some(b"dup"),
    ];
    let expected: Vec<Vec<PostMetaRow>> = keys
        .iter()
        .map(|key| {
            let found = view.matching_key(*key);
            assert_eq!(found.plan(), SearchPlan::Scan);
            found.cloned().collect()
        })
        .collect();
    assert_eq!(
        expected[2]
            .iter()
            .map(|row| row.meta_id)
            .collect::<Vec<_>>(),
        [0, 6, u64::MAX]
    );
    assert_eq!(view.build_key_index(8), SearchPlan::ExactKeyIndex);
    for (key, expected) in keys.iter().zip(&expected) {
        let found = view.matching_key(*key);
        assert_eq!(found.plan(), SearchPlan::ExactKeyIndex);
        assert_eq!(found.cloned().collect::<Vec<_>>(), *expected);
    }
    // A smaller allowance discards the whole index instead of serving a prefix.
    assert_eq!(view.build_key_index(7), SearchPlan::Scan);
    for (key, expected) in keys.iter().zip(&expected) {
        assert_eq!(
            view.matching_key(*key).cloned().collect::<Vec<_>>(),
            *expected
        );
    }
    assert_eq!(view.build_key_index(8), SearchPlan::ExactKeyIndex);
    view.discard_index();
    assert_eq!(view.matching_key(None).plan(), SearchPlan::Scan);
    assert_eq!(view.rows().len(), 8);
    assert!(!format!("{view:?}").contains("secret"));
    assert!(!format!("{view:?}").contains("duplicate"));
}

#[test]
fn duplicate_row_identity_is_rejected_but_duplicate_metadata_is_retained() {
    assert!(matches!(
        PostMetaView::new(vec![row(1, None), row(1, Some(b"different"))]),
        Err(DuplicateMetaId(1))
    ));
    assert_eq!(
        PostMetaView::new(vec![row(1, None), row(2, None)])
            .unwrap()
            .rows()
            .len(),
        2
    );
}

#[test]
fn views_do_not_share_indexes_or_replace_older_rows() {
    let mut old = PostMetaView::new(vec![row(1, Some(b"old"))]).unwrap();
    old.build_key_index(1);
    let mut newer = PostMetaView::new(vec![row(1, Some(b"new"))]).unwrap();
    newer.build_key_index(1);
    assert_eq!(old.matching_key(Some(b"old")).count(), 1);
    assert_eq!(newer.matching_key(Some(b"old")).count(), 0);
    assert_eq!(old.matching_key(Some(b"new")).count(), 0);
    assert_eq!(newer.matching_key(Some(b"new")).count(), 1);
    let mut empty = PostMetaView::new(vec![]).unwrap();
    assert_eq!(empty.build_key_index(0), SearchPlan::ExactKeyIndex);
    assert_eq!(empty.matching_key(None).count(), 0);
}

#[test]
fn changes_prepare_a_complete_new_view_without_old_index_offsets() {
    use recastpress_db::meta_view::MetaChange;
    let before = row(1, Some(b"old"));
    let deleted = row(2, None);
    let mut after = row(u64::MAX, Some(b"new"));
    after.post_id = 0;
    after.meta_value = None;
    let mut old = PostMetaView::new(vec![before.clone(), deleted.clone()]).unwrap();
    old.build_key_index(2);
    let mut new = old
        .prepare_changes([
            MetaChange::Update {
                before,
                after: after.clone(),
            },
            MetaChange::Delete(deleted),
            MetaChange::Insert(row(0, Some(b"new"))),
        ])
        .unwrap();
    assert_eq!(new.matching_key(Some(b"new")).plan(), SearchPlan::Scan);
    assert_eq!(new.rows(), &[row(0, Some(b"new")), after]);
    assert_eq!(new.build_key_index(2), SearchPlan::ExactKeyIndex);
    assert_eq!(new.matching_key(Some(b"new")).count(), 2);
    assert_eq!(old.matching_key(Some(b"old")).count(), 1);
    assert_eq!(old.matching_key(None).count(), 1);
    assert_eq!(old.matching_key(None).plan(), SearchPlan::ExactKeyIndex);
}

#[test]
fn late_conflicts_and_wrong_before_images_leave_existing_rows_unchanged() {
    use recastpress_db::meta_view::{ChangeError, MetaChange};
    let original = row(1, Some(b"original"));
    let mut old = PostMetaView::new(vec![original.clone(), row(2, None)]).unwrap();
    old.build_key_index(2);
    let cases = [
        (
            MetaChange::Delete(row(1, Some(b"wrong"))),
            ChangeError::BeforeImageMismatch(1),
        ),
        (
            MetaChange::Delete(row(99, None)),
            ChangeError::MissingRow(99),
        ),
        (
            MetaChange::Insert(row(2, Some(b"duplicate id"))),
            ChangeError::DuplicateId(2),
        ),
        (
            MetaChange::Update {
                before: row(2, None),
                after: row(1, Some(b"occupied")),
            },
            ChangeError::DuplicateId(1),
        ),
    ];
    for (invalid, expected) in cases {
        let result =
            old.prepare_changes([MetaChange::Insert(row(3, Some(b"must not leak"))), invalid]);
        assert_eq!(result.unwrap_err(), expected);
        assert_eq!(old.rows(), &[original.clone(), row(2, None)]);
        assert_eq!(old.matching_key(Some(b"must not leak")).count(), 0);
        assert_eq!(
            old.matching_key(Some(b"original")).plan(),
            SearchPlan::ExactKeyIndex
        );
    }
}

#[test]
fn multiple_changes_to_the_same_row_use_the_preceding_full_image() {
    use recastpress_db::meta_view::{ChangeError, MetaChange};
    let old = PostMetaView::new(vec![row(0, None)]).unwrap();
    let prepared = old
        .prepare_changes([
            MetaChange::Update {
                before: row(0, None),
                after: row(0, Some(b"next")),
            },
            MetaChange::Delete(row(0, Some(b"next"))),
            MetaChange::Insert(row(0, Some(b"final"))),
        ])
        .unwrap();
    assert_eq!(prepared.rows(), &[row(0, Some(b"final"))]);
    assert_eq!(
        prepared
            .prepare_changes([MetaChange::Delete(row(0, None))])
            .unwrap_err(),
        ChangeError::BeforeImageMismatch(0)
    );
}

#[test]
fn generated_byte_keys_and_raw_values_match_independent_filtering() {
    let mut rows = Vec::new();
    for index in 0..4096u64 {
        let key = match index % 19 {
            0 => None,
            1 => Some(vec![]),
            _ => Some(vec![(index % 256) as u8, 0, ((index / 256) % 4) as u8]),
        };
        rows.push(PostMetaRow {
            meta_id: if index == 4095 { u64::MAX } else { index },
            post_id: index % 7,
            meta_key: key,
            meta_value: match index % 3 {
                0 => None,
                1 => Some(vec![]),
                _ => Some(vec![0, 255, (index % 256) as u8]),
            },
        });
    }
    let mut keys = vec![None, Some(vec![]), Some(b"not present".to_vec())];
    keys.extend((0u8..=255).flat_map(|byte| (0..4).map(move |tail| Some(vec![byte, 0, tail]))));
    let expected: Vec<Vec<PostMetaRow>> = keys
        .iter()
        .map(|key| {
            rows.iter()
                .filter(|row| row.meta_key == *key)
                .cloned()
                .collect()
        })
        .collect();
    rows.reverse();
    let mut view = PostMetaView::new(rows).unwrap();
    for allowance in [0, 4096, 4095, 4096] {
        view.build_key_index(allowance);
        for (key, expected) in keys.iter().zip(&expected) {
            assert_eq!(
                view.matching_key(key.as_deref())
                    .cloned()
                    .collect::<Vec<_>>(),
                *expected
            );
        }
    }
}
