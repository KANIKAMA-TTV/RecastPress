//! A candidate move-only owner of postmeta rows/indexes and logical leases.
//!
//! Only this owner's row structures/payload lengths and final index logical
//! charges are reserved. Input rows/changes, reference preflight maps, tree-node
//! overhead, Vec/allocator capacity, driver/page buffers, peak memory and RSS
//! are excluded. This is not automatic capacity selection, cross-process
//! coordination, public cache integration or a production CDC protocol.
//!
//! Shared generations can use Arc<BudgetedPostMetaView>: cloning that Arc adds
//! no data/charge, and the last Arc keeps leases alive through data destruction.
//! No owned/mutable inner view escapes. Explicit copies made by callers from
//! borrowed rows are outside this owner's accounting. Existing unbudgeted APIs
//! remain available and their allocations are not retroactively accounted.

use crate::{
    PostMetaRow,
    meta_view::{
        LimitedChangeError, LoadError, MatchingRows, MetaChange, PostMetaView, SearchPlan,
        build_index, checked_row_charge, index_logical_charge, prepare_row_refs,
        validate_preflight,
    },
};
use recastpress_core::resource_budget::{BudgetError, BudgetPool, Reservation, ResourceKind};
use std::{collections::BTreeMap, fmt, num::NonZeroU16};

#[derive(Debug)]
pub enum BudgetedViewError {
    Load(LoadError),
    Change(LimitedChangeError),
    Budget(BudgetError),
}

impl fmt::Display for BudgetedViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Load(error) => error.fmt(f),
            Self::Change(error) => error.fmt(f),
            Self::Budget(BudgetError::Exhausted) => {
                f.write_str("metadata logical budget is exhausted")
            }
            Self::Budget(BudgetError::Unavailable) => {
                f.write_str("metadata logical budget is unavailable")
            }
        }
    }
}

impl std::error::Error for BudgetedViewError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Load(error) => Some(error),
            Self::Change(error) => Some(error),
            Self::Budget(_) => None,
        }
    }
}

/// Owns the data before its leases in field drop order. Not Clone or Copy.
pub struct BudgetedPostMetaView {
    view: PostMetaView,
    rows_lease: Reservation,
    index_lease: Option<Reservation>,
    pool: BudgetPool,
}

impl fmt::Debug for BudgetedPostMetaView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BudgetedPostMetaView")
            .field("row_count", &self.view.rows().len())
            .field("row_logical_bytes", &self.rows_lease.bytes())
            .field(
                "index_logical_bytes",
                &self.index_lease.as_ref().map(Reservation::bytes),
            )
            .finish_non_exhaustive()
    }
}

impl BudgetedPostMetaView {
    /// Validate borrowed input and reserve before cloning any row body.
    /// The separately owned input rows themselves are outside this charge.
    pub fn clone_from_rows(
        rows: &[PostMetaRow],
        pool: BudgetPool,
        max_rows: usize,
        max_logical_bytes: usize,
    ) -> Result<Self, BudgetedViewError> {
        let rows = prepare_row_refs(rows, &[])
            .map_err(|error| BudgetedViewError::Change(LimitedChangeError::Change(error)))?;
        Self::clone_reserved(rows, pool, max_rows, max_logical_bytes)
    }

    fn clone_reserved(
        rows: BTreeMap<u64, &PostMetaRow>,
        pool: BudgetPool,
        max_rows: usize,
        max_logical_bytes: usize,
    ) -> Result<Self, BudgetedViewError> {
        let bytes = checked_row_charge(rows.values().copied(), max_rows, max_logical_bytes)
            .map_err(BudgetedViewError::Change)?;
        let rows_lease = pool
            .try_reserve(ResourceKind::InternalRows, bytes)
            .map_err(BudgetedViewError::Budget)?;
        // This is the first row-body clone, after the successful reservation.
        let view = PostMetaView {
            rows: rows.into_values().cloned().collect(),
            index: None,
        };
        Ok(Self {
            view,
            rows_lease,
            index_lease: None,
            pool,
        })
    }

    /// Reserve the entire distinct candidate while keeping this generation's
    /// rows/index leased. Even smaller candidates cannot reuse an old lease.
    pub fn prepare_changes(
        &self,
        changes: &[MetaChange],
        max_rows: usize,
        max_logical_bytes: usize,
    ) -> Result<Self, BudgetedViewError> {
        let rows = prepare_row_refs(self.view.rows(), changes)
            .map_err(|error| BudgetedViewError::Change(LimitedChangeError::Change(error)))?;
        Self::clone_reserved(rows, self.pool.clone(), max_rows, max_logical_bytes)
    }

    pub fn rows(&self) -> &[PostMetaRow] {
        self.view.rows()
    }

    pub fn matching_key<'a>(&'a self, key: Option<&'a [u8]>) -> MatchingRows<'a> {
        self.view.matching_key(key)
    }

    /// Explicitly free index data before returning its lease. Row data remains.
    pub fn discard_index(&mut self) {
        self.view.discard_index();
        drop(self.index_lease.take());
    }

    /// Explicit entry/byte-cap refusal discards the index and returns Scan.
    /// Budget exhaustion/unavailability is an error and preserves the old
    /// index. Rebuilds reserve both old and new indexes until swapping succeeds.
    pub fn try_build_key_index(
        &mut self,
        max_entries: usize,
        max_logical_bytes: usize,
    ) -> Result<SearchPlan, BudgetError> {
        self.build_with_reserver(max_entries, max_logical_bytes, |pool, bytes| {
            pool.try_reserve(ResourceKind::Index, bytes)
        })
    }

    fn build_with_reserver(
        &mut self,
        max_entries: usize,
        max_logical_bytes: usize,
        reserve: impl FnOnce(&BudgetPool, usize) -> Result<Reservation, BudgetError>,
    ) -> Result<SearchPlan, BudgetError> {
        let Some(bytes) = index_logical_charge(self.view.rows(), max_entries, max_logical_bytes)
        else {
            self.discard_index();
            return Ok(SearchPlan::Scan);
        };
        // The private callback runs with no budget lock held; production only
        // calls try_reserve. It also permits deterministic Unavailable tests.
        let new_lease = reserve(&self.pool, bytes)?;
        let new_index = build_index(self.view.rows());
        let old_index = self.view.index.replace(new_index);
        let old_lease = self.index_lease.replace(new_lease);
        drop(old_index);
        drop(old_lease);
        Ok(SearchPlan::ExactKeyIndex)
    }
}

impl crate::EditingSnapshot {
    /// Lengths-only preflight, then logical reservation, then body load in this
    /// same read-only RR snapshot. Schema stability remains a prerequisite;
    /// rowcount/charge rechecks do not detect arbitrary schema changes. A failed
    /// reservation fetches no new row payload. No index is built automatically.
    pub async fn load_budgeted_post_meta_view(
        &mut self,
        pool: BudgetPool,
        max_rows: usize,
        max_logical_bytes: usize,
        page_size: NonZeroU16,
    ) -> Result<BudgetedPostMetaView, BudgetedViewError> {
        let expected = self
            .post_meta_view_preflight(max_rows, max_logical_bytes, page_size)
            .await
            .map_err(BudgetedViewError::Load)?;
        let rows_lease = pool
            .try_reserve(ResourceKind::InternalRows, expected.logical_bytes)
            .map_err(BudgetedViewError::Budget)?;
        let view = self
            .load_post_meta_view(max_rows, page_size)
            .await
            .map_err(BudgetedViewError::Load)?;
        validate_preflight(&view, &expected, max_logical_bytes).map_err(BudgetedViewError::Load)?;
        Ok(BudgetedPostMetaView {
            view,
            rows_lease,
            index_lease: None,
            pool,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unavailable_reservation_preserves_existing_index_and_all_claims() {
        let pool = BudgetPool::new(usize::MAX);
        let rows = [PostMetaRow {
            meta_id: 0,
            post_id: 0,
            meta_key: None,
            meta_value: None,
        }];
        let mut view =
            BudgetedPostMetaView::clone_from_rows(&rows, pool.clone(), 1, usize::MAX).unwrap();
        view.try_build_key_index(1, usize::MAX).unwrap();
        let before = pool.stats().unwrap();
        assert_eq!(
            view.build_with_reserver(1, usize::MAX, |_, _| Err(BudgetError::Unavailable)),
            Err(BudgetError::Unavailable)
        );
        assert_eq!(pool.stats().unwrap(), before);
        assert_eq!(view.rows(), rows);
        assert_eq!(view.matching_key(None).plan(), SearchPlan::ExactKeyIndex);
        assert_eq!(view.matching_key(None).count(), 1);
    }
}
