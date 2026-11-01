//! Experimental immutable editing-side row subset with an optional exact-byte index.
//!
//! This is not WP_Meta_Query, SQL collation, a complete snapshot loader, CDC, or
//! a public projection. The caller supplies rows from one coherent snapshot.
//! The row limit controls index entries, not process memory or automatic budgets.

use crate::PostMetaRow;
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    num::NonZeroU16,
    slice,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DuplicateMetaId(pub u64);

#[derive(Debug)]
pub enum LoadError {
    RowLimitExceeded { limit: usize },
    ByteLimitExceeded { limit: usize },
    SnapshotShapeChanged,
    Database(sqlx::Error),
    DuplicateId(DuplicateMetaId),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RowLimitExceeded { limit } => {
                write!(f, "metadata view exceeds its {limit}-row allowance")
            }
            Self::ByteLimitExceeded { limit } => {
                write!(
                    f,
                    "metadata view exceeds its {limit}-byte logical allowance"
                )
            }
            Self::SnapshotShapeChanged => {
                f.write_str("metadata snapshot shape differs from its preflight")
            }
            Self::Database(_) => f.write_str("metadata snapshot read failed"),
            Self::DuplicateId(_) => {
                f.write_str("metadata snapshot contains a repeated row identity")
            }
        }
    }
}

impl std::error::Error for LoadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Database(error) => Some(error),
            _ => None,
        }
    }
}

fn add_logical_row(
    current: usize,
    key_bytes: Option<u64>,
    value_bytes: Option<u64>,
    limit: usize,
) -> Result<usize, LoadError> {
    let next = || {
        current
            .checked_add(std::mem::size_of::<PostMetaRow>())?
            .checked_add(usize::try_from(key_bytes.unwrap_or(0)).ok()?)?
            .checked_add(usize::try_from(value_bytes.unwrap_or(0)).ok()?)
    };
    match next() {
        Some(bytes) if bytes <= limit => Ok(bytes),
        _ => Err(LoadError::ByteLimitExceeded { limit }),
    }
}

pub(super) struct PostMetaPreflight {
    pub(super) row_count: usize,
    pub(super) logical_bytes: usize,
}

pub(super) fn validate_preflight(
    view: &PostMetaView,
    expected: &PostMetaPreflight,
    max_logical_bytes: usize,
) -> Result<(), LoadError> {
    if view.rows().len() != expected.row_count {
        return Err(LoadError::SnapshotShapeChanged);
    }
    let mut actual_bytes = 0;
    for row in view.rows() {
        let key_bytes = row
            .meta_key
            .as_ref()
            .map(|bytes| u64::try_from(bytes.len()))
            .transpose()
            .map_err(|_| LoadError::SnapshotShapeChanged)?;
        let value_bytes = row
            .meta_value
            .as_ref()
            .map(|bytes| u64::try_from(bytes.len()))
            .transpose()
            .map_err(|_| LoadError::SnapshotShapeChanged)?;
        actual_bytes = add_logical_row(actual_bytes, key_bytes, value_bytes, max_logical_bytes)
            .map_err(|_| LoadError::SnapshotShapeChanged)?;
    }
    if actual_bytes != expected.logical_bytes {
        return Err(LoadError::SnapshotShapeChanged);
    }
    Ok(())
}

impl crate::EditingSnapshot {
    /// Load a complete view only after a full, lengths-only preflight fits both
    /// host allowances. Both passes use this same read-only RR snapshot under
    /// the existing stable-schema assumption. A failed preflight fetches no new
    /// key/value body into the client and returns no partial view.
    ///
    /// Logical bytes charge size_of::<PostMetaRow>() per row plus key/value byte
    /// lengths (NULL adds no payload). This excludes driver/page overhead,
    /// allocator capacity and indexes; it is not an RSS bound, automatic budget,
    /// public manual setting, or snapshot/binlog anchoring protocol.
    pub async fn load_post_meta_view_with_byte_limit(
        &mut self,
        max_rows: usize,
        max_logical_bytes: usize,
        page_size: NonZeroU16,
    ) -> Result<PostMetaView, LoadError> {
        let expected = self
            .post_meta_view_preflight(max_rows, max_logical_bytes, page_size)
            .await?;
        let view = self.load_post_meta_view(max_rows, page_size).await?;
        validate_preflight(&view, &expected, max_logical_bytes)?;
        Ok(view)
    }

    pub(super) async fn post_meta_view_preflight(
        &mut self,
        max_rows: usize,
        max_logical_bytes: usize,
        page_size: NonZeroU16,
    ) -> Result<PostMetaPreflight, LoadError> {
        let mut count = 0usize;
        let mut logical_bytes = 0;
        let mut cursor = None;
        loop {
            let remaining = max_rows - count;
            if remaining == 0 {
                if self
                    .post_meta_exists_after(cursor)
                    .await
                    .map_err(LoadError::Database)?
                {
                    return Err(LoadError::RowLimitExceeded { limit: max_rows });
                }
                break;
            }
            let requested = remaining.min(usize::from(page_size.get()));
            let limit = NonZeroU16::new(requested as u16).expect("positive capped page size");
            let lengths = self
                .post_meta_lengths_page(cursor, limit)
                .await
                .map_err(LoadError::Database)?;
            if lengths.len() > requested {
                return Err(LoadError::SnapshotShapeChanged);
            }
            if lengths.is_empty() {
                break;
            }
            for (id, key_bytes, value_bytes) in lengths {
                if cursor.is_some_and(|previous| id <= previous) {
                    return Err(LoadError::SnapshotShapeChanged);
                }
                logical_bytes =
                    add_logical_row(logical_bytes, key_bytes, value_bytes, max_logical_bytes)?;
                count = count
                    .checked_add(1)
                    .ok_or(LoadError::SnapshotShapeChanged)?;
                cursor = Some(id);
            }
        }
        Ok(PostMetaPreflight {
            row_count: count,
            logical_bytes,
        })
    }

    /// Load this snapshot's complete postmeta table, or return no view. A limit
    /// failure leaves the snapshot usable for ordinary paged reads. No index is
    /// built automatically. The host must separately budget row bytes, page and
    /// index allocations; row_count alone is not a memory bound. This neither
    /// anchors the snapshot to a binlog position nor publishes any row data.
    pub async fn load_post_meta_view(
        &mut self,
        max_rows: usize,
        page_size: NonZeroU16,
    ) -> Result<PostMetaView, LoadError> {
        let mut rows = Vec::new();
        let mut cursor = None;
        loop {
            let remaining = max_rows - rows.len();
            if remaining == 0 {
                if self
                    .post_meta_exists_after(cursor)
                    .await
                    .map_err(LoadError::Database)?
                {
                    return Err(LoadError::RowLimitExceeded { limit: max_rows });
                }
                return PostMetaView::new(rows).map_err(LoadError::DuplicateId);
            }
            // Never request a row body outside the allowance, even at zero.
            let requested = remaining.min(usize::from(page_size.get()));
            let limit = NonZeroU16::new(requested as u16).expect("positive capped page size");
            let page = self
                .post_meta_page(cursor, limit)
                .await
                .map_err(LoadError::Database)?;
            if page.len() > remaining {
                return Err(LoadError::RowLimitExceeded { limit: max_rows });
            }
            if page.is_empty() {
                return PostMetaView::new(rows).map_err(LoadError::DuplicateId);
            }
            cursor = page.last().map(|row| row.meta_id);
            rows.extend(page);
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchPlan {
    Scan,
    ExactKeyIndex,
}

/// Already decoded full row images. This type does not establish a source
/// position or COMMIT boundary; a trusted caller must validate those separately.
pub enum MetaChange {
    Insert(PostMetaRow),
    Update {
        before: PostMetaRow,
        after: PostMetaRow,
    },
    Delete(PostMetaRow),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChangeError {
    DuplicateId(u64),
    MissingRow(u64),
    BeforeImageMismatch(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitedChangeError {
    Change(ChangeError),
    RowLimitExceeded { limit: usize },
    ByteLimitExceeded { limit: usize },
}

impl fmt::Display for LimitedChangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Change(ChangeError::DuplicateId(_)) => {
                f.write_str("metadata change repeats a row identity")
            }
            Self::Change(ChangeError::MissingRow(_)) => {
                f.write_str("metadata change references a missing row")
            }
            Self::Change(ChangeError::BeforeImageMismatch(_)) => {
                f.write_str("metadata change before image differs from its row")
            }
            Self::RowLimitExceeded { limit } => {
                write!(f, "metadata candidate exceeds its {limit}-row allowance")
            }
            Self::ByteLimitExceeded { limit } => {
                write!(
                    f,
                    "metadata candidate exceeds its {limit}-byte logical allowance"
                )
            }
        }
    }
}

impl std::error::Error for LimitedChangeError {}

pub(super) fn prepare_row_refs<'a>(
    original: &'a [PostMetaRow],
    changes: &'a [MetaChange],
) -> Result<BTreeMap<u64, &'a PostMetaRow>, ChangeError> {
    let mut rows = BTreeMap::new();
    for row in original {
        if rows.insert(row.meta_id, row).is_some() {
            return Err(ChangeError::DuplicateId(row.meta_id));
        }
    }
    for change in changes {
        let (before, after) = match change {
            MetaChange::Insert(after) => (None, Some(after)),
            MetaChange::Delete(before) => (Some(before), None),
            MetaChange::Update { before, after } => (Some(before), Some(after)),
        };
        if let Some(before) = before {
            match rows.get(&before.meta_id) {
                None => return Err(ChangeError::MissingRow(before.meta_id)),
                Some(row) if *row != before => {
                    return Err(ChangeError::BeforeImageMismatch(before.meta_id));
                }
                Some(_) => {}
            }
            rows.remove(&before.meta_id);
        }
        if let Some(after) = after {
            if rows.contains_key(&after.meta_id) {
                return Err(ChangeError::DuplicateId(after.meta_id));
            }
            rows.insert(after.meta_id, after);
        }
    }
    Ok(rows)
}

pub(super) fn checked_row_charge<'a>(
    rows: impl Iterator<Item = &'a PostMetaRow> + Clone,
    max_rows: usize,
    max_logical_bytes: usize,
) -> Result<usize, LimitedChangeError> {
    let mut count = 0usize;
    for _ in rows.clone() {
        count = count
            .checked_add(1)
            .filter(|count| *count <= max_rows)
            .ok_or(LimitedChangeError::RowLimitExceeded { limit: max_rows })?;
    }
    let mut bytes = 0usize;
    for row in rows {
        bytes = bytes
            .checked_add(std::mem::size_of::<PostMetaRow>())
            .and_then(|bytes| bytes.checked_add(row.meta_key.as_ref().map_or(0, Vec::len)))
            .and_then(|bytes| bytes.checked_add(row.meta_value.as_ref().map_or(0, Vec::len)))
            .filter(|bytes| *bytes <= max_logical_bytes)
            .ok_or(LimitedChangeError::ByteLimitExceeded {
                limit: max_logical_bytes,
            })?;
    }
    Ok(bytes)
}

pub(super) fn index_logical_charge(
    rows: &[PostMetaRow],
    max_entries: usize,
    max_logical_bytes: usize,
) -> Option<usize> {
    if rows.len() > max_entries {
        return None;
    }
    let mut bytes = rows
        .len()
        .checked_mul(std::mem::size_of::<usize>())
        .filter(|bytes| *bytes <= max_logical_bytes)?;
    let mut keys: BTreeSet<&[u8]> = BTreeSet::new();
    for row in rows {
        if let Some(key) = row.meta_key.as_deref()
            && keys.insert(key)
        {
            bytes = bytes
                .checked_add(std::mem::size_of::<(Vec<u8>, Vec<usize>)>())
                .and_then(|bytes| bytes.checked_add(key.len()))
                .filter(|bytes| *bytes <= max_logical_bytes)?;
        }
    }
    Some(bytes)
}

pub(super) fn build_index(rows: &[PostMetaRow]) -> KeyIndex {
    let mut index = KeyIndex::default();
    for (offset, row) in rows.iter().enumerate() {
        match &row.meta_key {
            Some(key) => index.keys.entry(key.clone()).or_default().push(offset),
            None => index.nulls.push(offset),
        }
    }
    index
}

#[derive(Default)]
pub(super) struct KeyIndex {
    keys: BTreeMap<Vec<u8>, Vec<usize>>,
    nulls: Vec<usize>,
}

/// All indexes belong to these immutable rows; offsets cannot be used with a
/// different generation. Neither row values nor key bytes are exposed by Debug.
pub struct PostMetaView {
    pub(super) rows: Vec<PostMetaRow>,
    pub(super) index: Option<KeyIndex>,
}

impl fmt::Debug for PostMetaView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PostMetaView")
            .field("row_count", &self.rows.len())
            .field("indexed", &self.index.is_some())
            .finish_non_exhaustive()
    }
}

impl PostMetaView {
    /// Input may be a subset, not necessarily all metadata in the database.
    /// Results are ordered by meta_id; repeated key/value rows remain distinct.
    pub fn new(mut rows: Vec<PostMetaRow>) -> Result<Self, DuplicateMetaId> {
        rows.sort_unstable_by_key(|row| row.meta_id);
        if let Some(pair) = rows
            .windows(2)
            .find(|pair| pair[0].meta_id == pair[1].meta_id)
        {
            return Err(DuplicateMetaId(pair[0].meta_id));
        }
        Ok(Self { rows, index: None })
    }

    pub fn rows(&self) -> &[PostMetaRow] {
        &self.rows
    }

    /// Prepare a separate, unindexed view, keeping this view and its index
    /// unchanged even if a later change fails. Full before images are required.
    /// This clones the supplied row subset; shared memory budgets, durable
    /// checkpointing, replay identity, and automatic publication are pending.
    pub fn prepare_changes(
        &self,
        changes: impl IntoIterator<Item = MetaChange>,
    ) -> Result<Self, ChangeError> {
        let mut rows: BTreeMap<_, _> = self
            .rows
            .iter()
            .cloned()
            .map(|row| (row.meta_id, row))
            .collect();
        for change in changes {
            match change {
                MetaChange::Insert(row) => insert_row(&mut rows, row)?,
                MetaChange::Delete(before) => remove_row(&mut rows, &before)?,
                MetaChange::Update { before, after } => {
                    remove_row(&mut rows, &before)?;
                    insert_row(&mut rows, after)?;
                }
            }
        }
        Ok(Self {
            rows: rows.into_values().collect(),
            index: None,
        })
    }

    /// Validate ordered full-image changes using borrowed rows, then clone only
    /// the final candidate after it fits both allowances. Strict change errors
    /// precede limit errors. This view and its optional index remain unchanged.
    ///
    /// Logical bytes charge size_of::<PostMetaRow>() plus non-NULL key/value
    /// lengths for final rows only. The old view, supplied changes, reference
    /// map, indexes, Vec capacity and peak allocations are excluded. This is
    /// neither an RSS bound nor shared-pool accounting or an automatic formula.
    pub fn prepare_changes_with_limits(
        &self,
        changes: &[MetaChange],
        max_rows: usize,
        max_logical_bytes: usize,
    ) -> Result<Self, LimitedChangeError> {
        let rows = prepare_row_refs(&self.rows, changes).map_err(LimitedChangeError::Change)?;
        // These checks intentionally follow ALL strict validation, and precede
        // every payload clone. Transient rows do not consume final-row limits.
        checked_row_charge(rows.values().copied(), max_rows, max_logical_bytes)?;
        Ok(Self {
            rows: rows.into_values().cloned().collect(),
            index: None,
        })
    }

    /// Build the entire optional index or scan instead. A smaller allowance
    /// also evicts an existing index, and no partially built index is visible.
    /// Additional key-byte and allocator memory accounting is not implemented.
    pub fn build_key_index(&mut self, max_entries: usize) -> SearchPlan {
        self.discard_index();
        if self.rows.len() > max_entries {
            return SearchPlan::Scan;
        }
        self.index = Some(build_index(&self.rows));
        SearchPlan::ExactKeyIndex
    }

    /// Discard the old index and build only when the complete final logical
    /// index fits both allowances. Refusal leaves the same rows using a scan.
    /// The borrowed-key preflight clones no keys and constructs no owned index.
    ///
    /// Charge every row's usize offset (including NULL), plus one
    /// size_of::<(Vec<u8>, Vec<usize>)>() and key length per distinct non-NULL
    /// key. The inline KeyIndex root already in this view is excluded. So are
    /// preflight references, tree-node overhead, Vec capacity, row bodies, peak
    /// allocations, RSS and shared-pool accounting. This is no automatic formula.
    pub fn build_key_index_with_byte_limit(
        &mut self,
        max_entries: usize,
        max_logical_bytes: usize,
    ) -> SearchPlan {
        self.discard_index();
        if index_logical_charge(&self.rows, max_entries, max_logical_bytes).is_none() {
            return SearchPlan::Scan;
        }
        // The borrowed-key set has been dropped with the completed preflight.
        self.build_key_index(max_entries)
    }

    pub fn discard_index(&mut self) {
        self.index = None;
    }

    /// None selects SQL IS NULL. Some selects exact bytes, corresponding only
    /// to an explicit binary comparison, never a database's default collation.
    pub fn matching_key<'a>(&'a self, key: Option<&'a [u8]>) -> MatchingRows<'a> {
        let cursor = match &self.index {
            Some(index) => {
                let offsets = match key {
                    Some(key) => index.keys.get(key).map(Vec::as_slice).unwrap_or(&[]),
                    None => &index.nulls,
                };
                Cursor::Indexed(offsets.iter())
            }
            None => Cursor::Scan {
                rows: self.rows.iter(),
                key,
            },
        };
        MatchingRows {
            rows: &self.rows,
            cursor,
        }
    }
}

fn insert_row(rows: &mut BTreeMap<u64, PostMetaRow>, row: PostMetaRow) -> Result<(), ChangeError> {
    if rows.contains_key(&row.meta_id) {
        return Err(ChangeError::DuplicateId(row.meta_id));
    }
    rows.insert(row.meta_id, row);
    Ok(())
}

fn remove_row(
    rows: &mut BTreeMap<u64, PostMetaRow>,
    before: &PostMetaRow,
) -> Result<(), ChangeError> {
    match rows.get(&before.meta_id) {
        None => return Err(ChangeError::MissingRow(before.meta_id)),
        Some(row) if row != before => return Err(ChangeError::BeforeImageMismatch(before.meta_id)),
        Some(_) => {}
    }
    rows.remove(&before.meta_id);
    Ok(())
}

enum Cursor<'a> {
    Scan {
        rows: slice::Iter<'a, PostMetaRow>,
        key: Option<&'a [u8]>,
    },
    Indexed(slice::Iter<'a, usize>),
}

pub struct MatchingRows<'a> {
    rows: &'a [PostMetaRow],
    cursor: Cursor<'a>,
}

impl MatchingRows<'_> {
    pub fn plan(&self) -> SearchPlan {
        match self.cursor {
            Cursor::Scan { .. } => SearchPlan::Scan,
            Cursor::Indexed(_) => SearchPlan::ExactKeyIndex,
        }
    }
}

impl<'a> Iterator for MatchingRows<'a> {
    type Item = &'a PostMetaRow;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.cursor {
            Cursor::Scan { rows, key } => rows.find(|row| row.meta_key.as_deref() == *key),
            Cursor::Indexed(offsets) => offsets.next().map(|offset| &self.rows[*offset]),
        }
    }
}

#[cfg(test)]
mod byte_allowance_tests {
    use super::*;

    #[test]
    fn null_and_empty_payloads_charge_the_fixed_row_and_obey_exact_boundaries() {
        let fixed = std::mem::size_of::<PostMetaRow>();
        for lengths in [(None, None), (Some(0), Some(0)), (None, Some(0))] {
            assert_eq!(
                add_logical_row(0, lengths.0, lengths.1, fixed).unwrap(),
                fixed
            );
            assert!(matches!(
                add_logical_row(0, lengths.0, lengths.1, 0),
                Err(LoadError::ByteLimitExceeded { limit: 0 })
            ));
            assert!(add_logical_row(0, lengths.0, lengths.1, fixed - 1).is_err());
        }
    }

    #[test]
    fn payload_charge_counts_bytes_and_accumulates_rows_without_mutating_inputs() {
        let row = PostMetaRow {
            meta_id: u64::MAX,
            post_id: 0,
            meta_key: Some("日本語".as_bytes().to_vec()),
            meta_value: Some(b"\0\xff".to_vec()),
        };
        let original = row.clone();
        let fixed = std::mem::size_of::<PostMetaRow>();
        let key_bytes = row.meta_key.as_ref().map(|bytes| bytes.len() as u64);
        let value_bytes = row.meta_value.as_ref().map(|bytes| bytes.len() as u64);
        let first = add_logical_row(0, key_bytes, value_bytes, usize::MAX).unwrap();
        assert_eq!(first, fixed + 11);
        assert_eq!(
            add_logical_row(first, None, Some(3), usize::MAX).unwrap(),
            2 * fixed + 14
        );
        assert!(add_logical_row(0, Some(9), Some(2), fixed + 10).is_err());
        assert_eq!(row, original);
    }

    #[test]
    fn checked_charge_rejects_overflow_even_with_maximum_allowance() {
        let fixed = std::mem::size_of::<PostMetaRow>();
        assert_eq!(
            add_logical_row(usize::MAX - fixed, None, None, usize::MAX).unwrap(),
            usize::MAX
        );
        for (current, key, value) in [
            (usize::MAX, None, None),
            (0, Some(u64::MAX), None),
            (0, None, Some(u64::MAX)),
            (usize::MAX - fixed, Some(1), None),
            (0, Some(u64::MAX), Some(u64::MAX)),
        ] {
            assert!(
                matches!(add_logical_row(current, key, value, usize::MAX), Err(LoadError::ByteLimitExceeded { limit }) if limit == usize::MAX)
            );
        }
    }
}
