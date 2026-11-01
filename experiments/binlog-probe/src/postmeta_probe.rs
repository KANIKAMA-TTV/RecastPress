//! Finite, serialized WP postmeta fixture with a toy atomic SQL projection store.
//! No production snapshot/log pairing, storage-format contract, durable index or CDC API.

use crate::postmeta_journal::{Journal, Receipt};
use crate::{
    buffer_adapter::ProbeBuffer, normalize_maria_initial_rotate, require_fixture_tls, verify_crc,
};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStreamRequest, Conn, OptsBuilder, Value,
    binlog::{events::EventData, row::BinlogRow, value::BinlogValue},
    consts::ColumnType,
    prelude::Queryable,
};
use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use recastpress_db::change_buffer::Fault;
use recastpress_db::{
    PostMetaRow,
    budgeted_meta_view::{BudgetedPostMetaView, BudgetedViewError},
    meta_view::{LimitedChangeError, MetaChange, SearchPlan},
};
use std::{sync::Arc, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type SqlMeta = (u64, u64, Option<Vec<u8>>, Option<Vec<u8>>);

pub(crate) struct Observations {
    pub(crate) changes: usize,
    pub(crate) commits: usize,
    pub(crate) apply_refusal_preserved: bool,
    pub(crate) logical_limit_preserved: bool,
    pub(crate) limited_commits: usize,
    pub(crate) byte_limited_indexes: usize,
    pub(crate) shared_pool_commits: usize,
    pub(crate) shared_pool_refusal_preserved: bool,
    pub(crate) shared_pool_released: bool,
    pub(crate) durable_commits: usize,
    pub(crate) durable_replays: usize,
    pub(crate) ack_loss_preserved: bool,
    pub(crate) conflict_rejected: bool,
    pub(crate) reopened: bool,
    pub(crate) rollback_preserved: bool,
    pub(crate) context_rejected: bool,
    pub(crate) source_resumed: bool,
    pub(crate) source_file: String,
    pub(crate) saved_position: u64,
    pub(crate) saved_rows: usize,
}

async fn rows(control: &mut Conn) -> Result<Vec<PostMetaRow>> {
    let rows: Vec<SqlMeta> = control.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM wp_postmeta ORDER BY meta_id").await?;
    Ok(rows
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect())
}

async fn coordinate(control: &mut Conn, status: &'static str) -> Result<(Vec<u8>, u64)> {
    let mut row: mysql_async::Row = control
        .query_first(status)
        .await?
        .ok_or("no postmeta-probe coordinate")?;
    Ok((
        row.take("File").ok_or("no postmeta log file")?,
        row.take("Position").ok_or("no postmeta log position")?,
    ))
}

pub(crate) async fn verify(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    status: &'static str,
    maria: bool,
) -> Result<Observations> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
        return Err("postmeta probe requires the owned fixture".into());
    }
    let columns: Vec<(String,String,String,String,Option<String>)> = control.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='wp_postmeta' ORDER BY ORDINAL_POSITION").await?;
    if columns.len() != 4
        || columns[0].0 != "meta_id"
        || columns[1].0 != "post_id"
        || columns[..2].iter().any(|column| {
            column.1 != "bigint" || !column.2.ends_with(" unsigned") || column.3 != "NO"
        })
        || columns[2].0 != "meta_key"
        || columns[2].1 != "varchar"
        || columns[2].3 != "YES"
        || columns[2].4.as_deref() != Some("utf8mb4")
        || columns[3].0 != "meta_value"
        || columns[3].1 != "longtext"
        || columns[3].3 != "YES"
        || columns[3].4.as_deref() != Some("utf8mb4")
    {
        return Err("postmeta schema differs from the fixed fixture codec".into());
    }
    let initial_rows = rows(control).await?;
    if initial_rows
        .iter()
        .any(|row| matches!(row.meta_id, 100..=102))
        || initial_rows.len() > 32
    {
        return Err("postmeta probe identities or initial row bound are unavailable".into());
    }
    let pool = BudgetPool::new(1024 * 1024);
    let empty_pool = BudgetPool::new(0);
    let mut initial =
        BudgetedPostMetaView::clone_from_rows(&initial_rows, pool.clone(), 64, 64 * 1024)?;
    if initial
        .try_build_key_index(64, 64 * 1024)
        .map_err(|_| "initial shared index refused")?
        != SearchPlan::ExactKeyIndex
    {
        return Err("initial shared index was not built".into());
    }
    let initial = Arc::new(initial);
    // Only this fixture writes during this serialized setup. This sequencing is
    // not a general concurrent snapshot/binlog anchoring procedure.
    let (file, start) = coordinate(control, status).await?;
    let mut expected = Vec::new();
    control.query_drop("BEGIN").await?;
    control
        .exec_drop(
            "INSERT INTO wp_postmeta (meta_id,post_id,meta_key,meta_value) VALUES (100,?,?,?)",
            (
                u64::MAX,
                b"probe-key".as_slice(),
                b"first\0value".as_slice(),
            ),
        )
        .await?;
    control.query_drop("INSERT INTO wp_postmeta (meta_id,post_id,meta_key,meta_value) VALUES (101,0,NULL,NULL)").await?;
    control.query_drop("COMMIT").await?;
    expected.push(rows(control).await?);
    control.query_drop("BEGIN").await?;
    control
        .query_drop("UPDATE wp_postmeta SET meta_key='must-not-appear' WHERE meta_id=100")
        .await?;
    control.query_drop("ROLLBACK").await?;
    control.query_drop("BEGIN").await?;
    control.exec_drop("UPDATE wp_postmeta SET meta_id=102,post_id=0,meta_key=?,meta_value=? WHERE meta_id=100", ("変更\0🦀".as_bytes(), b"a:1:{s:3:\"key\";s:3:\"a\0b\";}".as_slice())).await?;
    control
        .query_drop("DELETE FROM wp_postmeta WHERE meta_id=101")
        .await?;
    control.query_drop("COMMIT").await?;
    expected.push(rows(control).await?);
    control.query_drop("BEGIN").await?;
    control
        .query_drop("DELETE FROM wp_postmeta WHERE meta_id=102")
        .await?;
    control.query_drop("COMMIT").await?;
    expected.push(rows(control).await?);
    let (end_file, stop) = coordinate(control, status).await?;
    if file != end_file || stop <= start {
        return Err("postmeta window rotated or did not advance".into());
    }
    let mut journal =
        Journal::initialize(options.clone(), epoch, &file, start, &initial_rows).await?;
    let mut connection = Conn::new(options.clone()).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = Some(
        connection
            .get_binlog_stream(
                BinlogStreamRequest::new(7006)
                    .with_filename(&file)
                    .with_pos(start)
                    .with_non_blocking(),
            )
            .await?,
    );
    let scan = async {
        let mut staged = ProbeBuffer::with_pool(epoch, &file, start, maria, pool.clone());
        let mut rejected = Some(ProbeBuffer::new(epoch, &file, start, maria));
        let mut pool_rejected = Some(ProbeBuffer::with_pool(
            epoch,
            &file,
            start,
            maria,
            empty_pool.clone(),
        ));
        let mut observed = Observations {
            changes: 0,
            commits: 0,
            apply_refusal_preserved: false,
            logical_limit_preserved: false,
            limited_commits: 0,
            byte_limited_indexes: 0,
            shared_pool_commits: 0,
            shared_pool_refusal_preserved: false,
            shared_pool_released: false,
            durable_commits: 0,
            durable_replays: 0,
            ack_loss_preserved: false,
            conflict_rejected: false,
            reopened: false,
            rollback_preserved: false,
            context_rejected: false,
            source_resumed: false,
            source_file: String::from_utf8(file.clone())?,
            saved_position: stop,
            saved_rows: initial_rows.len(),
        };
        let mut current = Arc::clone(&initial);
        let mut pending = Vec::new();
        let mut frames = Vec::new();
        let mut count = 0;
        let mut session_count = 0;
        let mut session_start = start;
        let mut applied_before_resume = 0;
        while let Some(event) = stream
            .as_mut()
            .ok_or("postmeta source disappeared")?
            .next()
            .await
        {
            let event = event?;
            let event = if session_count == 0 && maria {
                normalize_maria_initial_rotate(event, &file, session_start)?
            } else {
                event
            };
            count += 1;
            session_count += 1;
            if count > 128 || event.data().len() > 1024 * 1024 {
                return Err("postmeta event bound exceeded".into());
            }
            verify_crc(&event)?;
            let data = event.read_data()?.ok_or("undecoded postmeta event")?;
            if let EventData::RowsEvent(event_rows) = &data {
                let table = stream
                    .as_ref()
                    .ok_or("postmeta source disappeared")?
                    .get_tme(event_rows.table_id())
                    .ok_or("missing postmeta table map")?;
                if table.database_name_raw() != b"recastpress_fixture"
                    || table.table_name_raw() != b"wp_postmeta"
                    || table.columns_count() != 4
                    || table.get_column_type(0)? != Some(ColumnType::MYSQL_TYPE_LONGLONG)
                    || table.get_column_type(1)? != Some(ColumnType::MYSQL_TYPE_LONGLONG)
                    || !matches!(
                        table.get_column_type(2)?,
                        Some(ColumnType::MYSQL_TYPE_VARCHAR | ColumnType::MYSQL_TYPE_VAR_STRING)
                    )
                    || table.get_column_type(3)? != Some(ColumnType::MYSQL_TYPE_BLOB)
                    || table
                        .null_bitmask()
                        .iter()
                        .map(|bit| *bit)
                        .collect::<Vec<_>>()
                        != [false, false, true, true]
                {
                    return Err("postmeta table map differs from verified schema".into());
                }
                frames.push(event.data().to_vec());
                for row in event_rows.rows(table) {
                    let (before, after) = row?;
                    let change = match (before, after) {
                        (None, Some(after)) => MetaChange::Insert(decode(&after)?),
                        (Some(before), Some(after)) => MetaChange::Update {
                            before: decode(&before)?,
                            after: decode(&after)?,
                        },
                        (Some(before), None) => MetaChange::Delete(decode(&before)?),
                        _ => return Err("postmeta event has no full row image".into()),
                    };
                    pending.push(change);
                    observed.changes += 1;
                    if pending.len() > 8 {
                        return Err("postmeta change bound exceeded".into());
                    }
                }
            }
            if let Some(negative) = pool_rejected.as_mut() {
                let denied = negative.accept_projected(&event, &data, |_| {
                    Err("zero-pool transaction unexpectedly committed".into())
                });
                if denied.is_err() {
                    if !matches!(data, EventData::RowsEvent(_))
                        || negative.budget_refusal() != Some(BudgetError::Exhausted)
                        || negative.fault() != Some(Fault::PayloadBudgetExceeded)
                        || negative.checkpoint_offset() != start
                        || empty_pool
                            .stats()
                            .map_err(|_| "empty pool unavailable")?
                            .claimed
                            != 0
                        || current.rows() != initial_rows
                    {
                        return Err("shared-pool refusal did not preserve its exact context".into());
                    }
                    observed.shared_pool_refusal_preserved = true;
                    pool_rejected = None;
                } else if matches!(data, EventData::XidEvent(_)) {
                    return Err("zero-pool source transaction was accepted".into());
                }
            }
            if let Some(negative) = rejected.as_mut() {
                let result = negative.accept_projected(&event, &data, |_| {
                    match current.prepare_changes(&pending, 64, 0) {
                        Err(error) => Err(error.into()),
                        Ok(_) => Err("zero-byte projection allowance was accepted".into()),
                    }
                });
                if matches!(data, EventData::XidEvent(_)) {
                    if !matches!(
                        result
                            .as_ref()
                            .err()
                            .and_then(|error| error.downcast_ref::<BudgetedViewError>()),
                        Some(BudgetedViewError::Change(
                            LimitedChangeError::ByteLimitExceeded { limit: 0 }
                        ))
                    ) || negative.checkpoint_offset() != start
                        || current.rows() != initial_rows
                        || current.matching_key(None).plan() != SearchPlan::ExactKeyIndex
                    {
                        return Err("projection refusal advanced data or checkpoint".into());
                    }
                    observed.apply_refusal_preserved = true;
                    observed.logical_limit_preserved = true;
                    rejected = None;
                } else {
                    result?;
                }
            }
            let is_commit = matches!(data, EventData::XidEvent(_));
            let first_commit = is_commit && observed.commits == 0;
            let mut candidate = if is_commit {
                if pending.is_empty() {
                    return Err("postmeta commit has no pending full images".into());
                }
                let mut prepared = current
                    .prepare_changes(&pending, 64, 64 * 1024)
                    .map_err(|_| "postmeta full image conflict")?;
                pending.clear();
                observed.limited_commits += 1;
                let expected = expected
                    .get(observed.commits)
                    .ok_or("extra postmeta commit")?;
                if prepared.rows() != expected {
                    return Err("postmeta projected rows differ from committed SQL".into());
                }
                for (limit, plan) in [
                    (0, SearchPlan::Scan),
                    (64 * 1024, SearchPlan::ExactKeyIndex),
                ] {
                    if prepared
                        .try_build_key_index(64, limit)
                        .map_err(|_| "shared index budget refused")?
                        != plan
                        || prepared.rows() != expected
                    {
                        return Err("postmeta index logical allowance or raw rows differ".into());
                    }
                    for key in [
                        None,
                        Some(b"probe-key".as_slice()),
                        Some("変更\0🦀".as_bytes()),
                        Some(b"must-not-appear".as_slice()),
                    ] {
                        let found = prepared.matching_key(key);
                        if found.plan() != plan
                            || !found
                                .eq(expected.iter().filter(|row| row.meta_key.as_deref() == key))
                        {
                            return Err("postmeta scan or byte-limited index differs from committed SQL rows".into());
                        }
                    }
                }
                observed.byte_limited_indexes += 1;
                Some(prepared)
            } else {
                None
            };
            let applied = staged
                .accept_projected_async(&event, &data, async |batch| {
                    if !batch.rows().eq(frames.iter().map(Vec::as_slice)) {
                        return Err("projected frames differ from staged transaction".into());
                    }
                    let prepared = candidate
                        .as_ref()
                        .ok_or("projection callback precedes COMMIT")?;
                    let claims = pool
                        .stats()
                        .map_err(|_| "shared fixture ledger unavailable")?;
                    if claims.claimed_for(ResourceKind::InternalRows) == 0
                        || claims.claimed_for(ResourceKind::Index) == 0
                        || claims.claimed_for(ResourceKind::ChangeBuffer) == 0
                    {
                        return Err(
                            "rows/index/raw frames did not share active reservations".into()
                        );
                    }
                    observed.shared_pool_commits += 1;
                    if first_commit {
                        journal
                            .verify_rollback(batch, prepared.rows(), options.clone())
                            .await?;
                        observed.rollback_preserved = true;
                    }
                    if journal.apply(batch, prepared.rows()).await? != Receipt::Applied {
                        return Err("unexpected first application replay".into());
                    }
                    observed.durable_commits += 1;
                    if first_commit {
                        journal
                            .verify_conflicting_replay(batch, prepared.rows())
                            .await?;
                        observed.conflict_rejected = true;
                        return Err(
                            "injected loss of acknowledgement after projection COMMIT".into()
                        );
                    }
                    current = Arc::new(candidate.take().ok_or("no prepared projection")?);
                    Ok(())
                })
                .await;
            if first_commit {
                if applied.is_ok()
                    || staged.checkpoint_offset() != start
                    || current.rows() != initial_rows
                {
                    return Err("projection acknowledgement loss advanced memory state".into());
                }
                staged
                    .retry_projected_async(async |batch| {
                        if !batch.rows().eq(frames.iter().map(Vec::as_slice)) {
                            return Err("retry projected frames differ".into());
                        }
                        let prepared =
                            candidate.as_ref().ok_or("retry lost prepared projection")?;
                        journal
                            .verify(batch.through().offset(), prepared.rows())
                            .await?;
                        if journal.apply(batch, prepared.rows()).await? != Receipt::Replay {
                            return Err(
                                "projection acknowledgement retry was not idempotent".into()
                            );
                        }
                        observed.durable_replays += 1;
                        observed.ack_loss_preserved = true;
                        current = Arc::new(candidate.take().ok_or("retry lost candidate")?);
                        Ok(())
                    })
                    .await?;
            } else {
                applied?;
            }
            if is_commit {
                frames.clear();
                observed.commits += 1;
                journal
                    .verify(staged.checkpoint_offset(), current.rows())
                    .await?;
                if first_commit {
                    let restored = journal.reopen(options.clone()).await?;
                    if restored.position != staged.checkpoint_offset()
                        || restored.rows != current.rows()
                    {
                        return Err(
                            "reopened projection is not the committed data/position pair".into(),
                        );
                    }
                    let mut rebuilt = BudgetedPostMetaView::clone_from_rows(
                        &restored.rows,
                        pool.clone(),
                        64,
                        64 * 1024,
                    )?;
                    if rebuilt
                        .try_build_key_index(64, 64 * 1024)
                        .map_err(|_| "reopened shared index refused")?
                        != SearchPlan::ExactKeyIndex
                    {
                        return Err("reopened metadata index was not rebuilt".into());
                    }
                    current = Arc::new(rebuilt);
                    observed.reopened = true;
                    observed.context_rejected = true;
                    if staged.applied != 1 || observed.commits != 1 {
                        return Err(
                            "postmeta source did not resume after exactly one applied commit"
                                .into(),
                        );
                    }
                    let source = stream
                        .take()
                        .ok_or("postmeta source disappeared before resume")?;
                    source.close().await?;
                    applied_before_resume = staged.applied;
                    session_start = restored.position;
                    session_count = 0;
                    staged = ProbeBuffer::resumed_with_pool(
                        epoch,
                        &file,
                        session_start,
                        maria,
                        pool.clone(),
                    );
                    let mut replacement = Conn::new(options.clone()).await?;
                    require_fixture_tls(&mut replacement).await?;
                    stream = Some(
                        replacement
                            .get_binlog_stream(
                                BinlogStreamRequest::new(7007)
                                    .with_filename(&file)
                                    .with_pos(session_start)
                                    .with_non_blocking(),
                            )
                            .await?,
                    );
                    observed.source_resumed = true;
                }
            }
            if initial.rows() != initial_rows {
                return Err("old postmeta view was changed".into());
            }
            if !matches!(data, EventData::XidEvent(_)) {
                let before = if observed.commits == 0 {
                    &initial_rows
                } else {
                    &expected[observed.commits - 1]
                };
                if current.rows() != before {
                    return Err("uncommitted row image became visible".into());
                }
            }
            if u64::from(event.header().log_pos()) == stop {
                if !matches!(data, EventData::XidEvent(_)) {
                    return Err("postmeta endpoint is not COMMIT".into());
                }
                break;
            }
        }
        staged.finish()?;
        if observed.changes != 5
            || observed.commits != 3
            || observed.durable_commits != 3
            || observed.durable_replays != 1
            || !observed.ack_loss_preserved
            || !observed.conflict_rejected
            || !observed.rollback_preserved
            || !observed.context_rejected
            || !observed.reopened
            || !observed.apply_refusal_preserved
            || !observed.source_resumed
            || !observed.shared_pool_refusal_preserved
            || observed.shared_pool_commits != 3
            || applied_before_resume != 1
            || staged.applied != 2
            || applied_before_resume + staged.applied != 3
            || staged.checkpoint_offset() != stop
            || !pending.is_empty()
            || !frames.is_empty()
            || current.rows() != initial_rows
        {
            return Err("postmeta projection did not finish the expected committed window".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(observed)
    };
    let result = tokio::time::timeout(Duration::from_secs(10), scan).await;
    if let Some(stream) = stream {
        stream.close().await?;
    }
    let mut observed = result??;
    drop(initial);
    if pool
        .stats()
        .map_err(|_| "final shared ledger unavailable")?
        .claimed
        != 0
        || empty_pool
            .stats()
            .map_err(|_| "final empty ledger unavailable")?
            .claimed
            != 0
    {
        return Err("shared fixture ledger retained charges after owner drop".into());
    }
    observed.shared_pool_released = true;
    Ok(observed)
}

fn decode(row: &BinlogRow) -> Result<PostMetaRow> {
    if row.len() != 4 {
        return Err("postmeta row width differs".into());
    }
    // Bit reinterpretation is restricted to these SQL-verified UNSIGNED columns.
    let unsigned = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::UInt(value))) => Ok(*value),
        Some(BinlogValue::Value(Value::Int(value))) => Ok(u64::from_le_bytes(value.to_le_bytes())),
        _ => Err("postmeta unsigned image differs"),
    };
    let bytes = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::NULL)) => Ok(None),
        Some(BinlogValue::Value(Value::Bytes(bytes))) => Ok(Some(bytes.clone())),
        _ => Err("postmeta nullable byte image differs"),
    };
    Ok(PostMetaRow {
        meta_id: unsigned(0)?,
        post_id: unsigned(1)?,
        meta_key: bytes(2)?,
        meta_value: bytes(3)?,
    })
}
