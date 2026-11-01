//! Fixture-only classifier driving the real editing-side ChangeBuffer.
//! It deliberately rejects rotation, XA, statement mutation and other profiles.
//! The positive fixture uses an atomic SQL journal; negative prefixes use memory.
//! This is not the production data projection, source identity or schema design.

use crate::durable_journal::AtomicJournal;
use mysql_async::binlog::{
    BinlogVersion, EventFlags,
    events::{Event, EventData},
};
use recastpress_core::resource_budget::{BudgetError, BudgetPool};
use recastpress_db::{
    budgeted_change_buffer::{BudgetedChangeBuffer, BudgetedPushError},
    change_buffer::{Event as Change, Fault, Outcome},
};
use std::sync::Arc;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Clone)]
pub(crate) struct Position {
    // A fresh, nonserializable session identity. Persistent source identity and
    // reconnection validation remain the responsibility of a future coordinator.
    source: Arc<()>,
    offset: u64,
    epoch: Arc<[u8]>,
    file: Arc<[u8]>,
}
impl PartialEq for Position {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source) && self.offset == other.offset
    }
}
impl Eq for Position {}

impl Position {
    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }
    pub(crate) fn matches_context(&self, epoch: &[u8], file: &[u8]) -> bool {
        &*self.epoch == epoch && &*self.file == file
    }
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.source, &other.source)
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Phase {
    Idle,
    GtidBeforeBegin,
    Transaction,
}

pub(crate) struct ProbeBuffer {
    source: Arc<()>,
    file: Vec<u8>,
    epoch: Arc<[u8]>,
    initial: u64,
    cursor: u64,
    last_applied: u64,
    rotated: bool,
    described: bool,
    maria: bool,
    phase: Phase,
    buffer: BudgetedChangeBuffer<Position>,
    budget_refusal: Option<BudgetError>,
    pub(crate) applied: usize,
    pub(crate) retry_preserved: bool,
    inject_ack_loss: bool,
    pub(crate) derived_positions: usize,
}

impl ProbeBuffer {
    pub(crate) fn new(epoch: &[u8], file: &[u8], offset: u64, maria: bool) -> Self {
        Self::with_pool(epoch, file, offset, maria, BudgetPool::new(1024 * 1024))
    }

    pub(crate) fn with_pool(
        epoch: &[u8],
        file: &[u8],
        offset: u64,
        maria: bool,
        pool: BudgetPool,
    ) -> Self {
        let source = Arc::new(());
        Self {
            source: Arc::clone(&source),
            file: file.to_vec(),
            epoch: epoch.into(),
            initial: offset,
            cursor: offset,
            last_applied: offset,
            rotated: false,
            described: false,
            maria,
            phase: Phase::Idle,
            buffer: BudgetedChangeBuffer::new(
                Position {
                    source,
                    offset,
                    epoch: epoch.into(),
                    file: file.into(),
                },
                1024 * 1024,
                pool,
            ),
            budget_refusal: None,
            applied: 0,
            retry_preserved: false,
            inject_ack_loss: true,
            derived_positions: 0,
        }
    }

    /// A new decoder/session after the fixture host reloads a durable boundary.
    /// The initial consumer already exercised acknowledgement loss once.
    pub(crate) fn resumed(epoch: &[u8], file: &[u8], offset: u64, maria: bool) -> Self {
        Self::resumed_with_pool(epoch, file, offset, maria, BudgetPool::new(1024 * 1024))
    }

    pub(crate) fn resumed_with_pool(
        epoch: &[u8],
        file: &[u8],
        offset: u64,
        maria: bool,
        pool: BudgetPool,
    ) -> Self {
        let mut resumed = Self::with_pool(epoch, file, offset, maria, pool);
        resumed.inject_ack_loss = false;
        resumed
    }

    pub(crate) fn budget_refusal(&self) -> Option<BudgetError> {
        self.budget_refusal
    }

    pub(crate) fn fault(&self) -> Option<Fault> {
        self.buffer.fault()
    }

    pub(crate) fn checkpoint_offset(&self) -> u64 {
        self.buffer.checkpoint().offset
    }

    fn stage(&mut self, event: &Event, data: &EventData<'_>) -> Result<Option<u64>> {
        if self.buffer.fault().is_some() {
            return Err("adapter requires source reconciliation after a prior fault".into());
        }
        let result = self.accept_inner(event, data);
        if result.is_err() {
            self.buffer.disconnect();
        }
        result
    }

    fn point(&self, offset: u64) -> Position {
        Position {
            source: Arc::clone(&self.source),
            offset,
            epoch: Arc::clone(&self.epoch),
            file: self.file.clone().into(),
        }
    }

    fn accept_inner(&mut self, event: &Event, data: &EventData<'_>) -> Result<Option<u64>> {
        if self.buffer.checkpoint().offset != self.last_applied {
            return Err("checkpoint advanced without application".into());
        }
        if let EventData::RotateEvent(rotate) = data {
            if !self.rotated
                && !self.described
                && rotate.name_raw() == self.file
                && rotate.position() == self.initial
                && event.header().log_pos() == 0
            {
                self.rotated = true;
                return Ok(None);
            }
            return Err(format!("rotation requires coordinator reconciliation (initial={}, described={}, file_match={}, position={}, requested={}, header_end={}, flags={}, name_len={}, requested_name_len={}, name_prefix={})",
                !self.rotated, self.described, rotate.name_raw() == self.file,
                rotate.position(), self.initial, event.header().log_pos(), event.header().flags_raw(), rotate.name_raw().len(), self.file.len(), rotate.name_raw().starts_with(&self.file)).into());
        }
        if let EventData::FormatDescriptionEvent(description) = data {
            if self.rotated
                && !self.described
                && self.cursor == self.initial
                && description.binlog_version() == BinlogVersion::Version4
                && description.event_header_length() == 19
                && u64::from(event.header().log_pos()) <= self.initial
            {
                self.described = true;
                return Ok(None);
            }
            return Err("unexpected format description".into());
        }
        if !self.described {
            return Err("source events precede stream format".into());
        }
        let size = u64::from(event.header().event_size());
        if event.checksum().is_none() || size != 19 + event.data().len() as u64 + 4 {
            return Err("source event size/checksum envelope is inconsistent".into());
        }
        let reported_end = u64::from(event.header().log_pos());
        // MariaDB 11.4+ may leave end_log_pos zero inside a transaction to
        // precompute checksums. Derive physical offsets only for classified
        // interior frames, then require an explicit matching XID position before
        // applying anything. Never publish a derived position as a checkpoint.
        let end = if self.maria
            && self.phase == Phase::Transaction
            && reported_end == 0
            && matches!(
                data,
                EventData::QueryEvent(_) | EventData::TableMapEvent(_) | EventData::RowsEvent(_)
            ) {
            self.derived_positions += 1;
            self.cursor
                .checked_add(size)
                .ok_or("source offset overflow")?
        } else {
            reported_end
        };
        if end <= self.cursor
            || end.checked_sub(u64::from(event.header().event_size())) != Some(self.cursor)
        {
            return Err(format!("wire event positions are not contiguous (type={}, expected_start={}, end={}, size={})", event.header().event_type_raw(), self.cursor, end, event.header().event_size()).into());
        }
        let start = self.point(self.cursor);
        let next = self.point(end);
        let change = match data {
            EventData::GtidEvent(_) | EventData::AnonymousGtidEvent(_)
                if !self.maria && self.phase == Phase::Idle =>
            {
                self.phase = Phase::GtidBeforeBegin;
                Change::Begin { start, end: next }
            }
            EventData::QueryEvent(query)
                if query.error_code() == 0 && query.query_raw() == b"BEGIN" =>
            {
                match self.phase {
                    Phase::Idle if self.maria => {
                        self.phase = Phase::Transaction;
                        Change::Begin { start, end: next }
                    }
                    Phase::GtidBeforeBegin => {
                        self.phase = Phase::Transaction;
                        Change::Advance { start, end: next }
                    }
                    _ => return Err("unexpected or nested BEGIN".into()),
                }
            }
            // MariaDB's legacy-compatible stream may replace annotation events
            // with truncated single-line SQL comments. Only accept this synthetic
            // shape inside an existing transaction, never as an idle checkpoint.
            // This is not native MariaDB GTID support or a general SQL classifier.
            EventData::QueryEvent(query)
                if self.maria
                    && self.phase == Phase::Transaction
                    && query.thread_id() == 0
                    && query.error_code() == 0
                    && query.schema_raw().is_empty()
                    && query.status_vars_raw().is_empty()
                    && query.query_raw().starts_with(b"# Dummy event")
                    && !query
                        .query_raw()
                        .iter()
                        .any(|byte| matches!(byte, b'\r' | b'\n'))
                    && event
                        .header()
                        .flags()
                        .contains(EventFlags::LOG_EVENT_SUPPRESS_USE_F) =>
            {
                Change::Advance { start, end: next }
            }
            EventData::TableMapEvent(_) if self.phase == Phase::Transaction => {
                Change::Advance { start, end: next }
            }
            EventData::RowsEvent(_) if self.phase == Phase::Transaction => Change::Row {
                start,
                end: next,
                bytes: event.data().to_vec(),
            },
            EventData::XidEvent(_) if self.phase == Phase::Transaction => {
                Change::Commit { start, end: next }
            }
            EventData::QueryEvent(_) => {
                let _ = self.buffer.push(Change::Ddl);
                return Err("DDL or unclassified query requires reconciliation".into());
            }
            _ => return Err("unsupported transaction event".into()),
        };
        let outcome = match self.buffer.push(change) {
            Ok(outcome) => outcome,
            Err(error) => {
                if let BudgetedPushError::Budget(reason) = error {
                    self.budget_refusal = Some(reason);
                }
                return Err(format!("change buffer rejected event: {error:?}").into());
            }
        };
        self.cursor = end;
        if outcome == Outcome::ApplyRequired {
            return Ok(Some(end));
        }
        if self.buffer.checkpoint().offset != self.last_applied {
            return Err("partial transaction advanced checkpoint".into());
        }
        Ok(None)
    }

    /// Non-durable fixture consumers use an in-memory acknowledgement. If a gap is
    /// accidentally accepted, this advances and the negative test MUST fail.
    pub(crate) fn accept(&mut self, event: &Event, data: &EventData<'_>) -> Result<()> {
        if let Some(end) = self.stage(event, data)? {
            self.buffer
                .apply_pending::<std::convert::Infallible>(|_| Ok(()))?;
            self.last_applied = end;
            self.applied += 1;
            self.phase = Phase::Idle;
        }
        Ok(())
    }

    /// Fixture-only in-memory projection. This is NOT the durable apply contract
    /// used by accept_durable; all row data and this checkpoint disappear on exit.
    pub(crate) fn accept_projected(
        &mut self,
        event: &Event,
        data: &EventData<'_>,
        apply: impl FnOnce(&recastpress_db::change_buffer::CommittedBatch<Position>) -> Result<()>,
    ) -> Result<()> {
        if let Some(end) = self.stage(event, data)? {
            self.buffer.apply_pending(apply)?;
            self.last_applied = end;
            self.applied += 1;
            self.phase = Phase::Idle;
        }
        Ok(())
    }

    pub(crate) async fn accept_projected_async(
        &mut self,
        event: &Event,
        data: &EventData<'_>,
        apply: impl std::ops::AsyncFnOnce(
            &recastpress_db::change_buffer::CommittedBatch<Position>,
        ) -> Result<()>,
    ) -> Result<()> {
        if self.stage(event, data)?.is_some() {
            self.retry_projected_async(apply).await?;
        }
        Ok(())
    }

    /// Retry the SAME pending batch after an ambiguous fixture acknowledgement;
    /// the durable host callback must validate its stored receipt, not restage it.
    pub(crate) async fn retry_projected_async(
        &mut self,
        apply: impl std::ops::AsyncFnOnce(
            &recastpress_db::change_buffer::CommittedBatch<Position>,
        ) -> Result<()>,
    ) -> Result<()> {
        let end = self
            .buffer
            .pending()
            .ok_or("no pending projected batch")?
            .through()
            .offset;
        self.buffer.apply_pending_async(apply).await?;
        self.last_applied = end;
        self.applied += 1;
        self.phase = Phase::Idle;
        Ok(())
    }

    pub(crate) async fn accept_durable(
        &mut self,
        event: &Event,
        data: &EventData<'_>,
        journal: &mut AtomicJournal<'_>,
    ) -> Result<()> {
        let Some(end) = self.stage(event, data)? else {
            return Ok(());
        };
        let before = self.buffer.checkpoint().offset;
        if self.inject_ack_loss && !self.retry_preserved {
            let mut committed = false;
            let failed: Result<bool> = self
                .buffer
                .apply_pending_async(async |batch| {
                    journal.apply(batch).await?;
                    journal.verify_conflicting_replay(batch).await?;
                    committed = true;
                    Err("injected loss of acknowledgement AFTER durable commit".into())
                })
                .await;
            if !committed {
                return Err(failed
                    .err()
                    .unwrap_or_else(|| "apply callback was not called".into()));
            }
            if failed.is_ok()
                || self.buffer.checkpoint().offset != before
                || self.buffer.pending().is_none()
            {
                return Err("ambiguous commit lost its batch or advanced memory checkpoint".into());
            }
            self.retry_preserved = true;
        }
        let last = self.last_applied;
        let applied = self
            .buffer
            .apply_pending_async(async |batch| {
                if batch.from().offset != last
                    || batch.through().offset != end
                    || batch.rows().len() != 1
                {
                    return Err(
                        "committed fixture batch has an unexpected boundary or row count".into(),
                    );
                }
                journal.apply(batch).await
            })
            .await?;
        if !applied {
            return Err("committed batch vanished before durable application".into());
        }
        self.last_applied = end;
        self.applied += 1;
        self.phase = Phase::Idle;
        Ok(())
    }

    pub(crate) fn finish(&mut self) -> Result<()> {
        if self.buffer.fault().is_some()
            || !self.rotated
            || !self.described
            || self.phase != Phase::Idle
            || self.cursor != self.last_applied
            || self.buffer.pending().is_some()
        {
            self.buffer.disconnect();
            return Err("stream ended before a complete applied transaction boundary".into());
        }
        Ok(())
    }

    /// Negative test using the real stream prefix, ending immediately before
    /// its first XID. Requires a retained row to avoid a vacuous empty-stream test.
    pub(crate) fn verify_incomplete_rejection(mut self) -> Result<()> {
        if self.buffer.charged_bytes() == 0
            || self.applied != 0
            || self.buffer.checkpoint().offset != self.initial
        {
            return Err("incomplete-transaction fixture did not stage an unapplied row".into());
        }
        if self.finish().is_ok()
            || self.buffer.charged_bytes() != 0
            || self.buffer.checkpoint().offset != self.initial
            || self.buffer.pending().is_some()
        {
            return Err(
                "incomplete real transaction was not discarded at its prior checkpoint".into(),
            );
        }
        Ok(())
    }

    pub(crate) fn verify_fault_at_initial_checkpoint(&self) -> Result<()> {
        if self.buffer.fault().is_none()
            || self.buffer.charged_bytes() != 0
            || self.buffer.pending().is_some()
            || self.applied != 0
            || self.buffer.checkpoint().offset != self.initial
        {
            return Err("missing-frame prefix was not invalidated at its old checkpoint".into());
        }
        Ok(())
    }

    pub(crate) fn verify_schema_fault_at_initial_checkpoint(&mut self) -> Result<()> {
        self.verify_fault_at_initial_checkpoint()?;
        if self.buffer.fault() != Some(Fault::SchemaChanged) || self.finish().is_ok() {
            return Err(
                "actual DDL did not latch schema reconciliation at the old checkpoint".into(),
            );
        }
        self.verify_fault_at_initial_checkpoint()
    }
}
