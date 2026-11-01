//! Exact-byte index comparison within the owned raw-reader fixture only.
use super::*;
use crate::meta_view::{LoadError, MetaChange, PostMetaView, SearchPlan};
use sqlx::mysql::MySqlConnection;

pub(super) async fn contract(
    repository: &EditingRepository,
    writer: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    for (id, key) in [
        (10u64, b"Duplicate".as_slice()),
        (11, b"duplicate "),
        (12, b"duplicate\0"),
        (13, "日本語\0🦀".as_bytes()),
        (u64::MAX, b"duplicate"),
    ] {
        sqlx::query(
            "INSERT INTO wp_postmeta (meta_id,post_id,meta_key,meta_value) VALUES (?,?,?,?)",
        )
        .bind(id)
        .bind(u64::MAX)
        .bind(key)
        .bind(b"private\0value".as_slice())
        .execute(&mut *writer)
        .await?;
    }
    let mut snapshot = repository.snapshot().await?;
    for page_size in [1, 3, 32] {
        for max_rows in [0, 10] {
            assert!(
                matches!(snapshot.load_post_meta_view(max_rows, NonZeroU16::new(page_size).unwrap()).await,
                Err(LoadError::RowLimitExceeded { limit }) if limit == max_rows)
            );
        }
        let complete = snapshot
            .load_post_meta_view(11, NonZeroU16::new(page_size).unwrap())
            .await?;
        assert_eq!(complete.rows().len(), 11);
        assert_eq!(complete.rows().first().unwrap().meta_id, 0);
        assert_eq!(complete.rows().last().unwrap().meta_id, u64::MAX);
        assert_eq!(complete.matching_key(None).plan(), SearchPlan::Scan);
    }
    let mut view = snapshot
        .load_post_meta_view(usize::MAX, NonZeroU16::new(3).unwrap())
        .await?;
    let logical_bytes: usize = view
        .rows()
        .iter()
        .map(|row| {
            std::mem::size_of::<PostMetaRow>()
                + row.meta_key.as_ref().map_or(0, Vec::len)
                + row.meta_value.as_ref().map_or(0, Vec::len)
        })
        .sum();
    assert!(logical_bytes > 0);
    for page_size in [1, 3, 32] {
        let page_size = NonZeroU16::new(page_size).unwrap();
        for allowance in [0, logical_bytes - 1] {
            assert!(
                matches!(snapshot.load_post_meta_view_with_byte_limit(11, allowance, page_size).await,
                Err(LoadError::ByteLimitExceeded { limit }) if limit == allowance)
            );
        }
        for allowance in [logical_bytes, usize::MAX] {
            let bounded = snapshot
                .load_post_meta_view_with_byte_limit(11, allowance, page_size)
                .await?;
            assert_eq!(bounded.rows(), view.rows());
        }
        assert!(matches!(
            snapshot
                .load_post_meta_view_with_byte_limit(0, usize::MAX, page_size)
                .await,
            Err(LoadError::RowLimitExceeded { limit: 0 })
        ));
    }
    let keys: [Option<&[u8]>; 9] = [
        None,
        Some(b""),
        Some(b"duplicate"),
        Some(b"Duplicate"),
        Some(b"duplicate "),
        Some(b"duplicate\0"),
        Some("日本語\0🦀".as_bytes()),
        Some(b"missing"),
        Some(b"orphan"),
    ];
    for allowance in [0, view.rows().len(), 1] {
        let plan = view.build_key_index(allowance);
        assert_eq!(
            plan,
            if allowance == view.rows().len() {
                SearchPlan::ExactKeyIndex
            } else {
                SearchPlan::Scan
            }
        );
        for key in keys {
            let expected: Vec<(u64,)> = match key {
                Some(key) => sqlx::query_as("SELECT meta_id FROM wp_postmeta WHERE CAST(meta_key AS BINARY) = CAST(? AS BINARY) ORDER BY meta_id")
                    .bind(key).fetch_all(&mut *snapshot.transaction).await?,
                None => sqlx::query_as("SELECT meta_id FROM wp_postmeta WHERE meta_key IS NULL ORDER BY meta_id")
                    .fetch_all(&mut *snapshot.transaction).await?,
            };
            let found = view.matching_key(key);
            assert_eq!(found.plan(), plan);
            assert_eq!(
                found.map(|row| (row.meta_id,)).collect::<Vec<_>>(),
                expected
            );
        }
    }
    // The normal text collation is deliberately outside this index contract.
    let collation_ids: Vec<(u64,)> = sqlx::query_as(
        "SELECT meta_id FROM wp_postmeta WHERE meta_key = 'duplicate' ORDER BY meta_id",
    )
    .fetch_all(&mut *snapshot.transaction)
    .await?;
    assert_ne!(
        collation_ids,
        view.matching_key(Some(b"duplicate"))
            .map(|row| (row.meta_id,))
            .collect::<Vec<_>>()
    );
    view.build_key_index(view.rows().len());
    let before = view
        .rows()
        .iter()
        .find(|row| row.meta_id == 3)
        .unwrap()
        .clone();
    let mut after = before.clone();
    after.meta_key = Some(b"changed".to_vec());
    after.meta_value = Some(b"new value".to_vec());
    let mut prepared = view
        .prepare_changes([MetaChange::Update { before, after }])
        .unwrap();
    prepared.build_key_index(prepared.rows().len());
    sqlx::query(
        "UPDATE wp_postmeta SET meta_key='changed', meta_value='new value' WHERE meta_id=3",
    )
    .execute(&mut *writer)
    .await?;
    assert!(
        view.matching_key(Some(b"duplicate"))
            .any(|row| row.meta_id == 3)
    );
    assert_eq!(view.matching_key(Some(b"changed")).count(), 0);
    assert_eq!(
        snapshot
            .post_meta(1)
            .await?
            .iter()
            .find(|row| row.meta_id == 3)
            .unwrap()
            .meta_key
            .as_deref(),
        Some(b"duplicate".as_slice())
    );
    // Both length and data passes must use the old view despite the writer's
    // committed change to key/value lengths.
    assert_eq!(
        snapshot
            .load_post_meta_view_with_byte_limit(11, logical_bytes, NonZeroU16::new(3).unwrap())
            .await?
            .rows(),
        view.rows()
    );
    snapshot.finish().await?;
    let mut newer = repository.snapshot().await?;
    let all_new_rows = newer
        .post_meta_page(None, NonZeroU16::new(32).unwrap())
        .await?;
    assert_eq!(prepared.rows(), all_new_rows.as_slice());
    assert_eq!(
        prepared
            .matching_key(Some(b"changed"))
            .next()
            .unwrap()
            .meta_id,
        3
    );
    let mut new_view = PostMetaView::new(newer.post_meta(1).await?).unwrap();
    new_view.build_key_index(new_view.rows().len());
    assert_eq!(
        new_view
            .matching_key(Some(b"changed"))
            .next()
            .unwrap()
            .meta_value
            .as_deref(),
        Some(b"new value".as_slice())
    );
    assert_eq!(view.matching_key(Some(b"changed")).count(), 0);
    newer.finish().await?;
    // The owned fixture creates a large value on the server, then demonstrates
    // that a refused bounded load transfers only length/ID metadata. Measuring
    // this session's Bytes_sent is a fixture observation, not an RSS bound.
    sqlx::query("UPDATE wp_postmeta SET meta_value=REPEAT('x',1048576) WHERE meta_id=3")
        .execute(&mut *writer)
        .await?;
    let mut oversized = repository.snapshot().await?;
    let (_, before): (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Bytes_sent'")
        .fetch_one(&mut *oversized.transaction)
        .await?;
    assert!(matches!(
        oversized
            .load_post_meta_view_with_byte_limit(11, 0, NonZeroU16::new(32).unwrap())
            .await,
        Err(LoadError::ByteLimitExceeded { limit: 0 })
    ));
    let (_, after): (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Bytes_sent'")
        .fetch_one(&mut *oversized.transaction)
        .await?;
    let transferred = after
        .parse::<u64>()?
        .checked_sub(before.parse()?)
        .ok_or("session transfer counter decreased")?;
    assert!(
        transferred < 64 * 1024,
        "refused preflight transferred {transferred} bytes"
    );
    assert_eq!(
        oversized
            .post_meta_page(None, NonZeroU16::new(1).unwrap())
            .await?
            .first()
            .unwrap()
            .meta_id,
        0
    );
    oversized.finish().await?;
    sqlx::query("UPDATE wp_postmeta SET meta_value='new value' WHERE meta_id=3")
        .execute(&mut *writer)
        .await?;
    Ok(())
}
