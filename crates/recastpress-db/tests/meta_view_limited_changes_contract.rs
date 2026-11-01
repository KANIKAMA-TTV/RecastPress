use recastpress_db::{
    PostMetaRow,
    meta_view::{ChangeError, LimitedChangeError, MetaChange, PostMetaView, SearchPlan},
};

fn row(id: u64, key: Option<&[u8]>, value: Option<&[u8]>) -> PostMetaRow {
    PostMetaRow {
        meta_id: id,
        post_id: u64::MAX,
        meta_key: key.map(<[u8]>::to_vec),
        meta_value: value.map(<[u8]>::to_vec),
    }
}

fn charge(rows: &[PostMetaRow]) -> usize {
    rows.iter()
        .map(|r| {
            std::mem::size_of::<PostMetaRow>()
                + r.meta_key.as_ref().map_or(0, Vec::len)
                + r.meta_value.as_ref().map_or(0, Vec::len)
        })
        .sum()
}

fn indexed(rows: Vec<PostMetaRow>) -> PostMetaView {
    let mut view = PostMetaView::new(rows).unwrap();
    assert_eq!(view.build_key_index(usize::MAX), SearchPlan::ExactKeyIndex);
    view
}

fn unchanged(view: &PostMetaView, expected: &[PostMetaRow]) {
    assert_eq!(view.rows(), expected);
    for key in [None, Some(b"".as_slice()), Some(b"\0\xff".as_slice())] {
        let matches = view.matching_key(key);
        assert_eq!(matches.plan(), SearchPlan::ExactKeyIndex);
        let actual: Vec<_> = matches.collect();
        let expected: Vec<_> = expected
            .iter()
            .filter(|r| r.meta_key.as_deref() == key)
            .collect();
        assert_eq!(actual, expected);
    }
}

#[test]
fn final_byte_and_row_boundaries_preserve_null_empty_non_utf8_and_old_index() {
    let original = vec![row(0, None, Some(b"")), row(u64::MAX, Some(b""), None)];
    let old = indexed(original.clone());
    let replacement = row(u64::MAX, Some(b"\0\xff"), Some("語".as_bytes()));
    let changes = [MetaChange::Update {
        before: original[1].clone(),
        after: replacement.clone(),
    }];
    let expected = vec![original[0].clone(), replacement];
    let exact = charge(&expected);
    for limit in [exact, usize::MAX] {
        let candidate = old.prepare_changes_with_limits(&changes, 2, limit).unwrap();
        assert_eq!(candidate.rows(), expected);
        assert_eq!(candidate.matching_key(None).plan(), SearchPlan::Scan);
    }
    for limit in [0, exact - 1] {
        assert_eq!(
            old.prepare_changes_with_limits(&changes, usize::MAX, limit)
                .unwrap_err(),
            LimitedChangeError::ByteLimitExceeded { limit }
        );
    }
    for limit in [0, 1] {
        assert_eq!(
            old.prepare_changes_with_limits(&changes, limit, 0)
                .unwrap_err(),
            LimitedChangeError::RowLimitExceeded { limit }
        );
    }
    assert_eq!(
        old.prepare_changes_with_limits(&[], usize::MAX, usize::MAX)
            .unwrap()
            .rows(),
        original
    );
    unchanged(&old, &original);
}

#[test]
fn deletions_can_fit_limits_smaller_than_the_original_and_transient_rows_cost_zero() {
    let original = vec![row(0, None, None), row(u64::MAX, Some(b""), Some(b""))];
    let old = indexed(original.clone());
    let candidate = old
        .prepare_changes_with_limits(
            &[MetaChange::Delete(original[1].clone())],
            1,
            charge(&original[..1]),
        )
        .unwrap();
    assert_eq!(candidate.rows(), &original[..1]);
    let transient = row(7, Some(b"large transient body"), Some(b"\xff\0"));
    let changes = [
        MetaChange::Insert(transient.clone()),
        MetaChange::Delete(original[0].clone()),
        MetaChange::Delete(original[1].clone()),
        MetaChange::Delete(transient),
    ];
    assert!(
        old.prepare_changes_with_limits(&changes, 0, 0)
            .unwrap()
            .rows()
            .is_empty()
    );
    assert!(
        PostMetaView::new(vec![])
            .unwrap()
            .prepare_changes_with_limits(&[], 0, 0)
            .unwrap()
            .rows()
            .is_empty()
    );
    unchanged(&old, &original);
}

#[test]
fn ordered_primary_key_moves_and_aba_use_the_latest_full_image() {
    let original = vec![row(0, None, None), row(u64::MAX, Some(b""), None)];
    let old = indexed(original.clone());
    let moved = row(12, Some(b"\0\xff"), Some(b"first"));
    let returned = row(0, Some(b""), Some(b"second"));
    let replacement = row(12, None, Some(b"reused identity"));
    let changes = [
        MetaChange::Update {
            before: original[0].clone(),
            after: moved.clone(),
        },
        MetaChange::Update {
            before: moved,
            after: returned.clone(),
        },
        MetaChange::Insert(replacement.clone()),
        MetaChange::Delete(returned),
        MetaChange::Insert(original[0].clone()),
    ];
    let expected = vec![original[0].clone(), replacement, original[1].clone()];
    assert_eq!(
        old.prepare_changes_with_limits(&changes, 3, charge(&expected))
            .unwrap()
            .rows(),
        expected
    );
    unchanged(&old, &original);
}

#[test]
fn every_strict_change_error_precedes_limits_and_leaves_old_index_untouched() {
    let original = vec![row(0, None, None), row(u64::MAX, Some(b""), None)];
    let old = indexed(original.clone());
    let different = row(0, Some(b""), None);
    let missing = row(99, None, None);
    let cases = [
        (
            MetaChange::Insert(original[0].clone()),
            ChangeError::DuplicateId(0),
        ),
        (
            MetaChange::Delete(missing.clone()),
            ChangeError::MissingRow(99),
        ),
        (
            MetaChange::Delete(different.clone()),
            ChangeError::BeforeImageMismatch(0),
        ),
        (
            MetaChange::Update {
                before: missing,
                after: original[0].clone(),
            },
            ChangeError::MissingRow(99),
        ),
        (
            MetaChange::Update {
                before: different,
                after: row(8, None, None),
            },
            ChangeError::BeforeImageMismatch(0),
        ),
        (
            MetaChange::Update {
                before: original[0].clone(),
                after: original[1].clone(),
            },
            ChangeError::DuplicateId(u64::MAX),
        ),
    ];
    for (invalid, expected) in cases {
        let changes = [
            MetaChange::Insert(row(5, Some(b"must not leak"), None)),
            invalid,
        ];
        assert_eq!(
            old.prepare_changes_with_limits(&changes, 0, 0).unwrap_err(),
            LimitedChangeError::Change(expected)
        );
        unchanged(&old, &original);
    }
}

#[test]
fn stale_intermediate_images_and_all_before_image_fields_are_validated() {
    let original = vec![row(0, None, None)];
    let old = indexed(original.clone());
    let updated = row(0, Some(b"\0\xff"), Some(b"new"));
    let changes = [
        MetaChange::Update {
            before: original[0].clone(),
            after: updated,
        },
        MetaChange::Delete(original[0].clone()),
    ];
    assert_eq!(
        old.prepare_changes_with_limits(&changes, 0, 0).unwrap_err(),
        LimitedChangeError::Change(ChangeError::BeforeImageMismatch(0))
    );
    let mut different_post = original[0].clone();
    different_post.post_id = 0;
    for before in [
        different_post,
        row(0, Some(b""), None),
        row(0, None, Some(b"")),
    ] {
        assert_eq!(
            old.prepare_changes_with_limits(&[MetaChange::Delete(before)], usize::MAX, usize::MAX)
                .unwrap_err(),
            LimitedChangeError::Change(ChangeError::BeforeImageMismatch(0))
        );
    }
    let error = LimitedChangeError::Change(ChangeError::BeforeImageMismatch(0));
    fn is_error(_: &dyn std::error::Error) {}
    is_error(&error);
    assert!(!error.to_string().is_empty());
    unchanged(&old, &original);
}
