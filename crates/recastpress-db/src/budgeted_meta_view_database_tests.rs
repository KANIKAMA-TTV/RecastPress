//! Owned fixture checks; logical leases do not represent allocator/RSS bounds.
use super::*;
use crate::{budgeted_meta_view::BudgetedViewError, meta_view::SearchPlan};
use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use sqlx::mysql::MySqlConnection;
use std::{collections::BTreeSet, sync::Arc};

pub(super) async fn contract(
    repository: &EditingRepository,
    writer: &mut MySqlConnection,
) -> Result<(), Box<dyn std::error::Error>> {
    let page = NonZeroU16::new(3).unwrap();
    let mut snapshot = repository.snapshot().await?;
    let expected = snapshot
        .post_meta_page(None, NonZeroU16::new(32).unwrap())
        .await?;
    assert_eq!(expected.len(), 11);
    let row_bytes: usize = expected
        .iter()
        .map(|row| {
            std::mem::size_of::<PostMetaRow>()
                + row.meta_key.as_ref().map_or(0, Vec::len)
                + row.meta_value.as_ref().map_or(0, Vec::len)
        })
        .sum();
    let keys: BTreeSet<_> = expected.iter().map(|row| row.meta_key.clone()).collect();
    let index_bytes = expected.len() * std::mem::size_of::<usize>()
        + keys
            .iter()
            .filter_map(Option::as_ref)
            .map(|key| std::mem::size_of::<(Vec<u8>, Vec<usize>)>() + key.len())
            .sum::<usize>();
    let empty_pool = BudgetPool::new(0);
    assert!(matches!(
        snapshot
            .load_budgeted_post_meta_view(empty_pool.clone(), 10, usize::MAX, page)
            .await,
        Err(BudgetedViewError::Load(
            crate::meta_view::LoadError::RowLimitExceeded { limit: 10 }
        ))
    ));
    assert!(matches!(
        snapshot
            .load_budgeted_post_meta_view(empty_pool.clone(), 11, 0, page)
            .await,
        Err(BudgetedViewError::Load(
            crate::meta_view::LoadError::ByteLimitExceeded { limit: 0 }
        ))
    ));
    assert!(matches!(
        snapshot
            .load_budgeted_post_meta_view(empty_pool.clone(), 11, usize::MAX, page)
            .await,
        Err(BudgetedViewError::Budget(BudgetError::Exhausted))
    ));
    assert_eq!(empty_pool.stats().unwrap().claimed, 0);
    assert_eq!(
        snapshot
            .post_meta_page(None, NonZeroU16::new(1).unwrap())
            .await?[0]
            .meta_id,
        0
    );

    let pool = BudgetPool::new(row_bytes);
    let mut owner = snapshot
        .load_budgeted_post_meta_view(pool.clone(), 11, row_bytes, page)
        .await?;
    assert_eq!(owner.rows(), expected);
    assert_eq!(
        pool.stats()
            .unwrap()
            .claimed_for(ResourceKind::InternalRows),
        row_bytes
    );
    assert_eq!(
        owner.try_build_key_index(11, usize::MAX),
        Err(BudgetError::Exhausted)
    );
    assert_eq!(owner.matching_key(None).plan(), SearchPlan::Scan);
    pool.set_limit(row_bytes + index_bytes)
        .map_err(|_| "fixture pool limit unavailable")?;
    assert_eq!(
        owner.try_build_key_index(11, index_bytes),
        Ok(SearchPlan::ExactKeyIndex)
    );
    assert_eq!(
        pool.stats().unwrap().claimed_for(ResourceKind::Index),
        index_bytes
    );
    // Fresh generation/index reservations cannot silently reuse live leases.
    assert!(matches!(
        owner.prepare_changes(&[], 11, row_bytes),
        Err(BudgetedViewError::Budget(BudgetError::Exhausted))
    ));
    assert_eq!(
        owner.try_build_key_index(11, index_bytes),
        Err(BudgetError::Exhausted)
    );
    assert_eq!(owner.matching_key(None).plan(), SearchPlan::ExactKeyIndex);

    for (limit, plan) in [
        (0, SearchPlan::Scan),
        (index_bytes, SearchPlan::ExactKeyIndex),
    ] {
        assert_eq!(owner.try_build_key_index(11, limit), Ok(plan));
        assert_eq!(owner.rows(), expected);
        for key in &keys {
            let ids: Vec<(u64,)> = match key {
                Some(key) => sqlx::query_as("SELECT meta_id FROM wp_postmeta WHERE CAST(meta_key AS BINARY)=CAST(? AS BINARY) ORDER BY meta_id")
                    .bind(key).fetch_all(&mut *snapshot.transaction).await?,
                None => sqlx::query_as("SELECT meta_id FROM wp_postmeta WHERE meta_key IS NULL ORDER BY meta_id")
                    .fetch_all(&mut *snapshot.transaction).await?,
            };
            let found = owner.matching_key(key.as_deref());
            assert_eq!(found.plan(), plan);
            assert_eq!(found.map(|row| (row.meta_id,)).collect::<Vec<_>>(), ids);
        }
    }
    snapshot.finish().await?;
    assert_eq!(pool.stats().unwrap().claimed, row_bytes + index_bytes);
    let owner = Arc::new(owner);
    let retained = Arc::clone(&owner);
    drop(owner);
    assert_eq!(retained.rows(), expected);
    assert_eq!(pool.stats().unwrap().claimed, row_bytes + index_bytes);
    drop(retained);
    assert_eq!(pool.stats().unwrap().claimed, 0);

    // A budget refusal must happen after metadata preflight, before the large
    // body is fetched. The fixture owns this row and restores its original bytes.
    let saved: (Option<Vec<u8>>,) =
        sqlx::query_as("SELECT CAST(meta_value AS BINARY) FROM wp_postmeta WHERE meta_id=3")
            .fetch_one(&mut *writer)
            .await?;
    sqlx::query("UPDATE wp_postmeta SET meta_value=REPEAT('b',1048576) WHERE meta_id=3")
        .execute(&mut *writer)
        .await?;
    let mut large = repository.snapshot().await?;
    let before: (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Bytes_sent'")
        .fetch_one(&mut *large.transaction)
        .await?;
    let refused = large
        .load_budgeted_post_meta_view(empty_pool.clone(), 11, usize::MAX, page)
        .await;
    let after: (String, String) = sqlx::query_as("SHOW SESSION STATUS LIKE 'Bytes_sent'")
        .fetch_one(&mut *large.transaction)
        .await?;
    assert!(matches!(
        refused,
        Err(BudgetedViewError::Budget(BudgetError::Exhausted))
    ));
    let sent = after
        .1
        .parse::<u64>()?
        .checked_sub(before.1.parse::<u64>()?)
        .ok_or("fixture byte counter decreased")?;
    assert!(
        sent < 64 * 1024,
        "budget refusal transferred the large body: {sent}"
    );
    assert_eq!(empty_pool.stats().unwrap().claimed, 0);
    assert_eq!(
        large
            .post_meta_page(None, NonZeroU16::new(1).unwrap())
            .await?[0]
            .meta_id,
        0
    );
    large.finish().await?;
    sqlx::query("UPDATE wp_postmeta SET meta_value=? WHERE meta_id=3")
        .bind(saved.0)
        .execute(&mut *writer)
        .await?;
    Ok(())
}
