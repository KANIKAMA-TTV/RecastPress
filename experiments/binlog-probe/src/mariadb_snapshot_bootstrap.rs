//! Unpublished MariaDB 11.8.9 snapshot-anchor bootstrap in one owned fixture.
//! Same-session RR anchor and bounded raw pages feed the unchanged strict decoder.
//! Scratch rows, driver buffers, expected copies and row decoding are unpooled;
//! only BudgetedPostMetaView rows/index and retained ProbeBuffer payload share
//! logical reservations. No journal, public generation, continuous CDC, source
//! restart identity, automatic allocation, global pause or production protocol.

use crate::{
    ChecksumFault, buffer_adapter::ProbeBuffer, close_fixture_connection,
    normalize_maria_initial_rotate, require_fixture_tls, verify_crc,
};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStreamRequest, Conn, OptsBuilder, Value,
    binlog::{
        events::{BinlogEventHeader, Event, EventData, TableMapEvent},
        row::BinlogRow,
        value::BinlogValue,
    },
    consts::ColumnType,
    prelude::Queryable,
};
use recastpress_core::resource_budget::{BudgetError, BudgetPool, ResourceKind};
use recastpress_db::{
    PostMetaRow,
    budgeted_meta_view::{BudgetedPostMetaView, BudgetedViewError},
    change_buffer::Fault,
    meta_view::{LimitedChangeError, MetaChange, SearchPlan},
};
use serde_json::{Value as Json, json};
use std::{collections::BTreeSet, time::Duration};
type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type EncodedRows = Vec<(u64, u64, Option<Vec<u8>>, Option<Vec<u8>>)>;
type SearchExpectation = (Option<Vec<u8>>, Vec<(u64,)>);
const FILE: &str = "probe-bin.000001";
const LIMIT: usize = 64 * 1024;
const SOURCE: &str = "rp_maria_bootstrap_source";

fn clone_sql_mode(original: &str) -> Option<String> {
    if original
        .split(',')
        .any(|mode| mode.trim().eq_ignore_ascii_case("NO_AUTO_VALUE_ON_ZERO"))
    {
        None
    } else if original.is_empty() {
        Some("NO_AUTO_VALUE_ON_ZERO".into())
    } else {
        Some(format!("{original},NO_AUTO_VALUE_ON_ZERO"))
    }
}

async fn clone_rows_preserving_zero(control: &mut Conn) -> Result<()> {
    let original: String = control
        .query_first("SELECT @@SESSION.sql_mode")
        .await?
        .ok_or("Maria bootstrap clone session SQL mode missing")?;
    let replacement = clone_sql_mode(&original);
    let insert: std::result::Result<(), mysql_async::Error> = async {
        if let Some(replacement) = &replacement {
            control
                .exec_drop("SET SESSION sql_mode = ?", (replacement,))
                .await?;
        }
        control
            .query_drop("INSERT INTO rp_maria_bootstrap_source SELECT * FROM wp_postmeta")
            .await
    }
    .await;
    // Attempt restoration even if SET or INSERT failed. This session-local
    // change never substitutes generated IDs for the source's stored zero ID.
    let restore = if replacement.is_some() {
        control
            .exec_drop("SET SESSION sql_mode = ?", (&original,))
            .await
    } else {
        Ok(())
    };
    match (insert, restore) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(insert), Err(restore)) => Err(format!(
            "Maria bootstrap source clone failed: {insert}; restoring session SQL mode also failed: {restore}"
        )
        .into()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error.into()),
    }
}

async fn schema(connection: &mut Conn) -> Result<()> {
    let columns: Vec<(String,String,String,String,Option<String>)> = connection.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_maria_bootstrap_source' ORDER BY ORDINAL_POSITION").await?;
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
        return Err("Maria bootstrap source SQL schema differs".into());
    }
    let engine: Option<String> = connection.query_first("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_maria_bootstrap_source'").await?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("Maria bootstrap source is not InnoDB".into());
    }
    Ok(())
}

async fn coordinate(connection: &mut Conn, status: &'static str) -> Result<(String, u64)> {
    let mut row: mysql_async::Row = connection
        .query_first(status)
        .await?
        .ok_or("Maria bootstrap source coordinate missing")?;
    let file: String = row
        .take("File")
        .ok_or("Maria bootstrap source file missing")?;
    let position: u64 = row
        .take("Position")
        .ok_or("Maria bootstrap source position missing")?;
    if file != FILE || position <= 4 {
        return Err("Maria bootstrap source coordinate invalid".into());
    }
    Ok((file, position))
}

fn decode(row: &BinlogRow) -> Result<PostMetaRow> {
    if row.len() != 4 {
        return Err("Maria bootstrap row image width differs".into());
    }
    let unsigned = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::UInt(value))) => Ok(*value),
        Some(BinlogValue::Value(Value::Int(value))) => Ok(u64::from_le_bytes(value.to_le_bytes())),
        _ => Err("Maria bootstrap SQL-verified unsigned column image differs"),
    };
    let bytes = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::NULL)) => Ok(None),
        Some(BinlogValue::Value(Value::Bytes(value))) => Ok(Some(value.clone())),
        _ => Err("Maria bootstrap nullable byte column image differs"),
    };
    Ok(PostMetaRow {
        meta_id: unsigned(0)?,
        post_id: unsigned(1)?,
        meta_key: bytes(2)?,
        meta_value: bytes(3)?,
    })
}

struct Expected {
    rows: Vec<PostMetaRow>,
    searches: Vec<SearchExpectation>,
}

fn check_rows(rows: &[PostMetaRow]) -> Result<()> {
    let mut bytes = 0usize;
    for row in rows {
        bytes = bytes
            .checked_add(std::mem::size_of::<PostMetaRow>())
            .and_then(|n| n.checked_add(row.meta_key.as_ref().map_or(0, Vec::len)))
            .and_then(|n| n.checked_add(row.meta_value.as_ref().map_or(0, Vec::len)))
            .ok_or("Maria bootstrap row size overflow")?;
    }
    if rows.len() > 11
        || bytes > LIMIT
        || rows
            .windows(2)
            .any(|pair| pair[0].meta_id >= pair[1].meta_id)
    {
        return Err("Maria bootstrap row count/order/bytes bound differs".into());
    }
    Ok(())
}

async fn raw_rows(connection: &mut Conn, table: &str) -> Result<Vec<PostMetaRow>> {
    if ![SOURCE, "wp_postmeta"].contains(&table) {
        return Err("unknown bootstrap SQL table".into());
    }
    let encoded: EncodedRows=connection.query(format!("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM {table} ORDER BY meta_id LIMIT 12")).await?;
    let rows = materialize(encoded);
    check_rows(&rows)?;
    Ok(rows)
}
async fn rows(connection: &mut Conn) -> Result<Vec<PostMetaRow>> {
    raw_rows(connection, SOURCE).await
}
fn materialize(rows: EncodedRows) -> Vec<PostMetaRow> {
    rows.into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect()
}

async fn page(reader: &mut Conn, after: Option<u64>) -> Result<Vec<PostMetaRow>> {
    let encoded: EncodedRows=match after {
        None=>reader.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_maria_bootstrap_source ORDER BY meta_id LIMIT 3").await?,
        Some(id)=>reader.exec("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_maria_bootstrap_source WHERE meta_id>? ORDER BY meta_id LIMIT 3",(id,)).await?,
    };
    let rows = materialize(encoded);
    check_rows(&rows)?;
    Ok(rows)
}

fn parse_anchor(rows: Vec<(String, String)>) -> Result<(String, u64)> {
    if rows.len() != 2 {
        return Err("Maria bootstrap anchor shape differs".into());
    }
    let mut file = None;
    let mut position = None;
    for (name, value) in rows {
        match name.as_str() {
            "Binlog_snapshot_file" if file.is_none() && value == FILE => file = Some(value),
            "Binlog_snapshot_position"
                if position.is_none()
                    && !value.is_empty()
                    && value.bytes().all(|b| b.is_ascii_digit()) =>
            {
                position = Some(value.parse::<u64>()?)
            }
            _ => return Err("Maria bootstrap anchor status invalid or duplicated".into()),
        }
    }
    match (file, position) {
        (Some(file), Some(position)) if position > 4 => Ok((file, position)),
        _ => Err("Maria bootstrap anchor missing or before file header".into()),
    }
}
async fn anchor(reader: &mut Conn) -> Result<(String, u64)> {
    parse_anchor(reader.query("SHOW SESSION STATUS WHERE Variable_name IN ('Binlog_snapshot_file','Binlog_snapshot_position')").await?)
}

fn check_view(view: &BudgetedPostMetaView, expected: &Expected) -> Result<()> {
    if view.rows() != expected.rows {
        return Err("Maria bootstrap raw rows differ from SQL".into());
    }
    for (key, ids) in &expected.searches {
        let found = view.matching_key(key.as_deref());
        if found.plan() != SearchPlan::ExactKeyIndex
            || found.map(|r| (r.meta_id,)).collect::<Vec<_>>() != *ids
        {
            return Err("Maria bootstrap index differs from binary SQL search".into());
        }
    }
    Ok(())
}
fn build_index(view: &mut BudgetedPostMetaView) -> Result<()> {
    if view
        .try_build_key_index(64, LIMIT)
        .map_err(|_| "Maria bootstrap index pool refused")?
        != SearchPlan::ExactKeyIndex
    {
        return Err("Maria bootstrap index cap refused".into());
    }
    Ok(())
}

fn table_map(table: &TableMapEvent<'_>) -> Result<()> {
    if table.database_name_raw() != b"recastpress_fixture"
        || table.table_name_raw() != SOURCE.as_bytes()
        || table.columns_count() != 4
    {
        return Err("Maria bootstrap table map context/count differs".into());
    }
    let types = (0..4)
        .map(|index| table.get_column_type(index))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let nulls = table.null_bitmask().iter().map(|b| *b).collect::<Vec<_>>();
    validate_layout(&types, &nulls)
}
fn validate_layout(types: &[Option<ColumnType>], nulls: &[bool]) -> Result<()> {
    if types.len() != 4
        || types[0] != Some(ColumnType::MYSQL_TYPE_LONGLONG)
        || types[1] != Some(ColumnType::MYSQL_TYPE_LONGLONG)
        || !matches!(
            types[2],
            Some(ColumnType::MYSQL_TYPE_VARCHAR | ColumnType::MYSQL_TYPE_VAR_STRING)
        )
        || types[3] != Some(ColumnType::MYSQL_TYPE_BLOB)
        || nulls != [false, false, true, true]
    {
        return Err("Maria bootstrap table map types/nulls differ".into());
    }
    Ok(())
}

#[derive(Default)]
struct NegativeObservations {
    shared_commits: usize,
    apply: bool,
    budget: bool,
    crc: bool,
    gap: bool,
}

// No publication or durable journal is installed. Only a successful apply callback
// swaps the unpublished owner; failed callbacks retain its indexed generation.
#[allow(clippy::too_many_arguments)]
async fn consume(
    options: OptsBuilder,
    epoch: &[u8],
    start: u64,
    ends: &[u64],
    expected: &[Expected],
    mut current: BudgetedPostMetaView,
    pool: BudgetPool,
    negatives: bool,
) -> Result<(BudgetedPostMetaView, NegativeObservations)> {
    if ends.len() != expected.len()
        || ends.is_empty()
        || start <= 4
        || ends.windows(2).any(|p| p[0] >= p[1])
        || ends[0] <= start
    {
        return Err("Maria bootstrap window shape differs".into());
    }
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7041)
                .with_filename(FILE.as_bytes())
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?;
    let result = tokio::time::timeout(Duration::from_secs(15), async {
        let original_rows = current.rows().to_vec();
        let mut staged =
            ProbeBuffer::resumed_with_pool(epoch, FILE.as_bytes(), start, true, pool.clone());
        let mut callback =
            negatives.then(|| ProbeBuffer::resumed(epoch, FILE.as_bytes(), start, true));
        let empty_pool = BudgetPool::new(0);
        let mut denied = negatives.then(|| {
            ProbeBuffer::resumed_with_pool(epoch, FILE.as_bytes(), start, true, empty_pool.clone())
        });
        let mut gap = negatives.then(|| ProbeBuffer::resumed(epoch, FILE.as_bytes(), start, true));
        let mut bad_crc =
            negatives.then(|| ProbeBuffer::resumed(epoch, FILE.as_bytes(), start, true));
        let mut skipped = false;
        let mut observed = NegativeObservations::default();
        let mut pending = Vec::new();
        let mut frames = Vec::new();
        let mut commits = 0;
        let mut events = 0;
        while let Some(event) = stream.next().await {
            let event = event?;
            let event = if events == 0 {
                normalize_maria_initial_rotate(event, FILE.as_bytes(), start)?
            } else {
                event
            };
            events += 1;
            if events > 128 || event.data().len() > LIMIT {
                return Err("Maria bootstrap event bound exceeded".into());
            }
            verify_crc(&event)?;
            let data = event
                .read_data()?
                .ok_or("Maria bootstrap event undecoded")?;
            if let EventData::TableMapEvent(table) = &data {
                table_map(table)?;
            }
            let row_event = matches!(data, EventData::RowsEvent(_));
            if let EventData::RowsEvent(rows) = &data {
                let table = stream
                    .get_tme(rows.table_id())
                    .ok_or("Maria bootstrap row map missing")?;
                table_map(table)?;
                frames.push(event.data().to_vec());
                if frames.len() > 4 {
                    return Err("Maria bootstrap frame bound exceeded".into());
                }
                for row in rows.rows(table) {
                    let (before, after) = row?;
                    pending.push(match (before, after) {
                        (None, Some(after)) => MetaChange::Insert(decode(&after)?),
                        (Some(before), Some(after)) => MetaChange::Update {
                            before: decode(&before)?,
                            after: decode(&after)?,
                        },
                        (Some(before), None) => MetaChange::Delete(decode(&before)?),
                        _ => return Err("Maria bootstrap full image missing".into()),
                    });
                    if pending.len() > 4 {
                        return Err("Maria bootstrap change bound exceeded".into());
                    }
                }
            }
            if let Some(negative) = bad_crc.as_mut() {
                if row_event {
                    let mut wire = Vec::new();
                    event.write(mysql_async::binlog::BinlogVersion::Version4, &mut wire)?;
                    *wire
                        .get_mut(BinlogEventHeader::LEN)
                        .ok_or("Maria bootstrap CRC payload missing")? ^= 1;
                    let damaged = Event::read(event.fde(), wire.as_slice())?;
                    if !matches!(verify_crc(&damaged), Err(ChecksumFault::Mismatch))
                        || negative.finish().is_ok()
                    {
                        return Err("Maria bootstrap CRC fault not specifically refused".into());
                    }
                    negative.verify_fault_at_initial_checkpoint()?;
                    observed.crc = true;
                    bad_crc = None;
                } else {
                    negative.accept_projected(&event, &data, |_| {
                        Err("CRC prefix unexpectedly applied".into())
                    })?;
                }
            }
            if let Some(negative) = gap.as_mut() {
                if row_event && !skipped {
                    skipped = true;
                } else if let Err(error) = negative.accept_projected(&event, &data, |_| {
                    Err("gap prefix unexpectedly applied".into())
                }) {
                    if !skipped
                        || !error
                            .to_string()
                            .starts_with("wire event positions are not contiguous")
                    {
                        return Err(error);
                    }
                    negative.verify_fault_at_initial_checkpoint()?;
                    observed.gap = true;
                    gap = None;
                }
            }
            if let Some(negative) = denied.as_mut()
                && let Err(error) = negative.accept_projected(&event, &data, |_| {
                    Err("zero pool unexpectedly applied".into())
                })
            {
                if !row_event
                    || negative.budget_refusal() != Some(BudgetError::Exhausted)
                    || negative.fault() != Some(Fault::PayloadBudgetExceeded)
                {
                    return Err(error);
                }
                negative.verify_fault_at_initial_checkpoint()?;
                if empty_pool
                    .stats()
                    .map_err(|_| "zero pool unavailable")?
                    .claimed
                    != 0
                {
                    return Err("zero pool leaked a reservation".into());
                }
                observed.budget = true;
                denied = None;
            }
            let is_commit = matches!(data, EventData::XidEvent(_));
            if let Some(negative) = callback.as_mut() {
                let result = negative.accept_projected(&event, &data, |_| {
                    match current.prepare_changes(&pending, 64, 0) {
                        Err(error) => Err(error.into()),
                        Ok(_) => Err("zero row allowance applied".into()),
                    }
                });
                if is_commit {
                    if !matches!(
                        result
                            .as_ref()
                            .err()
                            .and_then(|e| e.downcast_ref::<BudgetedViewError>()),
                        Some(BudgetedViewError::Change(
                            LimitedChangeError::ByteLimitExceeded { limit: 0 }
                        ))
                    ) || negative.checkpoint_offset() != start
                        || negative.applied != 0
                    {
                        return Err(
                            "Maria bootstrap typed callback refusal lost its checkpoint".into()
                        );
                    }
                    observed.apply = true;
                    callback = None;
                } else {
                    result?;
                }
            }
            if negatives && commits == 0 {
                // All four negative paths operate against the same untouched
                // indexed owner. Even rejection before the first XID changes none.
                if staged.checkpoint_offset() != start {
                    return Err("negative path advanced positive checkpoint".into());
                }
                if current.rows() != original_rows
                    || current.matching_key(None).plan() != SearchPlan::ExactKeyIndex
                {
                    return Err("negative path modified old rows/index".into());
                }
            }
            let mut candidate = if is_commit {
                let expected_changes = if ends.len() == 3 && commits == 2 {
                    2
                } else {
                    1
                };
                if pending.len() != expected_changes {
                    return Err("Maria bootstrap planned strict change count differs".into());
                }
                let target = expected
                    .get(commits)
                    .ok_or("Maria bootstrap extra commit")?;
                let mut next = current.prepare_changes(&pending, 64, LIMIT)?;
                build_index(&mut next)?;
                check_view(&next, target)?;
                Some(next)
            } else {
                None
            };
            let from = staged.checkpoint_offset();
            staged.accept_projected(&event, &data, |batch| {
                if batch.from().offset() != from
                    || batch.through().offset() != ends[commits]
                    || !batch.rows().eq(frames.iter().map(Vec::as_slice))
                {
                    return Err("Maria bootstrap callback boundary/retained frames differ".into());
                }
                let claims = pool
                    .stats()
                    .map_err(|_| "Maria bootstrap shared pool unavailable")?;
                if [
                    ResourceKind::InternalRows,
                    ResourceKind::Index,
                    ResourceKind::ChangeBuffer,
                ]
                .iter()
                .any(|kind| claims.claimed_for(*kind) == 0)
                {
                    return Err("Maria bootstrap callback did not share all three leases".into());
                }
                current = candidate
                    .take()
                    .ok_or("Maria bootstrap apply without candidate")?;
                observed.shared_commits += 1;
                Ok(())
            })?;
            if is_commit {
                if pending.is_empty() || staged.checkpoint_offset() != ends[commits] {
                    return Err(
                        "Maria bootstrap checkpoint or full changes differ from SQL endpoint"
                            .into(),
                    );
                }
                commits += 1;
                pending.clear();
                frames.clear();
                if commits == ends.len() {
                    break;
                }
            }
        }
        staged.finish()?;
        if commits != ends.len()
            || observed.shared_commits != commits
            || staged.applied != commits
            || staged.checkpoint_offset() != *ends.last().unwrap()
            || !pending.is_empty()
            || !frames.is_empty()
            || (negatives && !(observed.apply && observed.budget && observed.crc && observed.gap))
        {
            return Err("Maria bootstrap finite observations incomplete".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(observed)
    })
    .await;
    let close = stream.close().await;
    let observed = result??;
    close?;
    Ok((current, observed))
}

pub(crate) async fn run(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    version: &str,
) -> Result<Json> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable")
        || std::env::var("RECASTPRESS_DB_EXPECT_VERSION").as_deref() != Ok("11.8.9-MariaDB")
        || !version.starts_with("11.8.9-MariaDB")
    {
        return Err("Maria bootstrap requires the pinned owned MariaDB 11.8.9 fixture".into());
    }
    let profile: Option<(String, String, String)> = control
        .query_first(
            "SELECT @@GLOBAL.binlog_format,@@GLOBAL.binlog_row_image,@@GLOBAL.binlog_checksum",
        )
        .await?;
    if profile != Some(("ROW".into(), "FULL".into(), "CRC32".into())) {
        return Err("Maria bootstrap requires ROW FULL CRC32".into());
    }
    let original = raw_rows(control, "wp_postmeta").await?;
    if original.len() != 11
        || original.first().map(|r| r.meta_id) != Some(0)
        || original.last().map(|r| r.meta_id) != Some(u64::MAX)
        || !original.iter().any(|r| r.meta_id == 3)
        || original.iter().any(|r| r.meta_id == 100)
    {
        return Err("Maria bootstrap source fixture identities differ".into());
    }
    control
        .query_drop("CREATE TABLE rp_maria_bootstrap_source LIKE wp_postmeta")
        .await?;
    clone_rows_preserving_zero(control).await?;
    schema(control).await?;
    if rows(control).await? != original {
        return Err("Maria bootstrap clone lost source bytes/identities".into());
    }
    let initial = capture_expected(control).await?;
    let mut reader = Conn::new(options.clone()).await?;
    require_fixture_tls(&mut reader).await?;
    reader
        .query_drop("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .await?;
    reader
        .query_drop("START TRANSACTION READ ONLY, WITH CONSISTENT SNAPSHOT")
        .await?;
    let (file, a) = anchor(&mut reader).await?;
    let mut expected = Vec::new();
    let mut ends = Vec::new();
    // T1 commits after the anchor and before the reader's first table SELECT.
    control.query_drop("START TRANSACTION").await?;
    control
        .query_drop(
            "UPDATE rp_maria_bootstrap_source SET meta_value=X'6D617269612D6F6E65' WHERE meta_id=3",
        )
        .await?;
    control.query_drop("COMMIT").await?;
    let (next_file, t1) = coordinate(control, "SHOW MASTER STATUS").await?;
    if next_file != file || t1 <= a {
        return Err("Maria bootstrap T1 did not follow anchor".into());
    }
    ends.push(t1);
    expected.push(capture_expected(control).await?);
    let mut snapshot = page(&mut reader, None).await?;
    if snapshot.len() != 3 || snapshot != original[..3] {
        return Err("Maria bootstrap first page is not the old snapshot".into());
    }
    let cursor = snapshot.last().unwrap().meta_id;
    // T2 moves a PK; T3 removes that identity and reuses the old identity (ABA).
    control.query_drop("START TRANSACTION").await?;
    control.query_drop("UPDATE rp_maria_bootstrap_source SET meta_id=100,meta_key='maria-moved' WHERE meta_id=3").await?;
    control.query_drop("COMMIT").await?;
    let (next_file, t2) = coordinate(control, "SHOW MASTER STATUS").await?;
    if next_file != file || t2 <= t1 {
        return Err("Maria bootstrap T2 did not follow T1".into());
    }
    ends.push(t2);
    expected.push(capture_expected(control).await?);
    control.query_drop("START TRANSACTION").await?;
    control
        .query_drop("DELETE FROM rp_maria_bootstrap_source WHERE meta_id=100")
        .await?;
    control.query_drop("INSERT INTO rp_maria_bootstrap_source(meta_id,post_id,meta_key,meta_value) VALUES (3,18446744073709551615,'maria-reused',X'6D617269612D746872656500')").await?;
    control.query_drop("COMMIT").await?;
    let (next_file, c) = coordinate(control, "SHOW MASTER STATUS").await?;
    if next_file != file || c <= t2 {
        return Err("Maria bootstrap T3 did not follow T2".into());
    }
    ends.push(c);
    expected.push(capture_expected(control).await?);
    let mut after = Some(cursor);
    loop {
        let next = page(&mut reader, after).await?;
        if next.is_empty() {
            break;
        }
        if next[0].meta_id <= after.unwrap() {
            return Err("Maria bootstrap snapshot cursor regressed".into());
        }
        after = next.last().map(|r| r.meta_id);
        snapshot.extend(next);
        check_rows(&snapshot)?;
    }
    if snapshot != original || anchor(&mut reader).await? != (file.clone(), a) {
        return Err("Maria bootstrap snapshot rows or session anchor changed".into());
    }
    // Explicitly verify the READ ONLY property; error 1792 must preserve the view.
    match reader
        .query_drop("UPDATE rp_maria_bootstrap_source SET meta_value='forbidden' WHERE meta_id=3")
        .await
    {
        Err(mysql_async::Error::Server(error)) if error.code == 1792 => {}
        Err(error) => return Err(error.into()),
        Ok(()) => return Err("Maria bootstrap read-only reader accepted a write".into()),
    }
    if rows(&mut reader).await? != original || anchor(&mut reader).await? != (file.clone(), a) {
        return Err("Maria bootstrap rejected write changed the snapshot".into());
    }
    let pool = BudgetPool::new(1024 * 1024);
    let mut current = BudgetedPostMetaView::clone_from_rows(&snapshot, pool.clone(), 64, LIMIT)?;
    build_index(&mut current)?;
    check_view(&current, &initial)?;
    let (current, negative) = consume(
        options.clone(),
        epoch,
        a,
        &ends,
        &expected,
        current,
        pool.clone(),
        true,
    )
    .await?;
    // A remains tied to the same open reader even after the CDC catch-up.
    if anchor(&mut reader).await? != (file.clone(), a) || rows(&mut reader).await? != original {
        return Err("Maria bootstrap reader lost its original snapshot after catch-up".into());
    }
    reader.query_drop("ROLLBACK").await?;
    close_fixture_connection(reader, "maria_bootstrap_reader").await?;
    control.query_drop("START TRANSACTION").await?;
    control.query_drop("UPDATE rp_maria_bootstrap_source SET meta_value=X'6D617269612D666F7572' WHERE meta_id=3").await?;
    control.query_drop("COMMIT").await?;
    let (next_file, d) = coordinate(control, "SHOW MASTER STATUS").await?;
    if next_file != file || d <= c {
        return Err("Maria bootstrap strict T4 coordinate differs".into());
    }
    let final_expected = capture_expected(control).await?;
    let (current, strict) = consume(
        options,
        epoch,
        c,
        &[d],
        std::slice::from_ref(&final_expected),
        current,
        pool.clone(),
        false,
    )
    .await?;
    check_view(&current, &final_expected)?;
    let shared_commits = negative
        .shared_commits
        .checked_add(strict.shared_commits)
        .ok_or("Maria bootstrap commit count overflow")?;
    if negative.shared_commits != 3 || strict.shared_commits != 1 || shared_commits != 4 {
        return Err("Maria bootstrap shared callback counts differ".into());
    }
    let final_rows = current.rows().len();
    if current.rows().len() != 11 || raw_rows(control, "wp_postmeta").await? != original {
        return Err("Maria bootstrap changed original WP rows or final count".into());
    }
    drop(current);
    if pool
        .stats()
        .map_err(|_| "Maria bootstrap final pool unavailable")?
        .claimed
        != 0
    {
        return Err("Maria bootstrap retained a shared reservation".into());
    }
    Ok(
        json!({"format_version":1,"profile":"mariadb_11_8_9_unpublished_snapshot_bootstrap","process_pid":std::process::id(),
        "source_file":file,"anchor_position":a,"bootstrap_position":c,"strict_position":d,"snapshot_rows":snapshot.len(),"rows":final_rows,
        "bootstrap_commits":negative.shared_commits,"strict_commits":strict.shared_commits,"shared_pool_commits":shared_commits,"snapshot_anchor_stable":true,"snapshot_rows_stable":true,
        "rows_match":true,"index_matches":true,"apply_refusal_preserved":negative.apply,"budget_refusal_preserved":negative.budget,
        "crc_refusal_preserved":negative.crc,"gap_refusal_preserved":negative.gap,"shared_pool_released":true,"unpublished_only":true}),
    )
}

async fn capture_expected(control: &mut Conn) -> Result<Expected> {
    let rows = rows(control).await?;
    let mut keys: BTreeSet<_> = rows.iter().map(|row| row.meta_key.clone()).collect();
    keys.insert(None);
    keys.insert(Some(b"maria-bootstrap-missing-key".to_vec()));
    let mut searches = Vec::new();
    for key in keys {
        let ids = match &key {
            Some(key) => control.exec("SELECT meta_id FROM rp_maria_bootstrap_source WHERE CAST(meta_key AS BINARY)=CAST(? AS BINARY) ORDER BY meta_id",(key,)).await?,
            None => control.query("SELECT meta_id FROM rp_maria_bootstrap_source WHERE meta_key IS NULL ORDER BY meta_id").await?,
        };
        searches.push((key, ids));
    }
    Ok(Expected { rows, searches })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn same_session_anchor_shape_is_strict() {
        let good = || {
            vec![
                ("Binlog_snapshot_file".into(), FILE.into()),
                ("Binlog_snapshot_position".into(), "123".into()),
            ]
        };
        assert_eq!(parse_anchor(good()).unwrap(), (FILE.into(), 123));
        for value in ["", "4", "-1", "+5", "18446744073709551616"] {
            let mut input = good();
            input[1].1 = value.into();
            assert!(parse_anchor(input).is_err());
        }
        let mut duplicate = good();
        duplicate[1] = duplicate[0].clone();
        assert!(parse_anchor(duplicate).is_err());
        let mut other_file = good();
        other_file[0].1 = "probe-bin.000002".into();
        assert!(parse_anchor(other_file).is_err());
    }
    #[test]
    fn metadata_is_checked_even_without_rows_and_zero_mode_is_not_duplicated() {
        let good = [
            Some(ColumnType::MYSQL_TYPE_LONGLONG),
            Some(ColumnType::MYSQL_TYPE_LONGLONG),
            Some(ColumnType::MYSQL_TYPE_VARCHAR),
            Some(ColumnType::MYSQL_TYPE_BLOB),
        ];
        validate_layout(&good, &[false, false, true, true]).unwrap();
        assert!(validate_layout(&good[..3], &[false, false, true]).is_err());
        assert!(validate_layout(&good, &[true, false, true, true]).is_err());
        let mut wrong = good;
        wrong[0] = Some(ColumnType::MYSQL_TYPE_VARCHAR);
        assert!(validate_layout(&wrong, &[false, false, true, true]).is_err());
        assert_eq!(clone_sql_mode(""), Some("NO_AUTO_VALUE_ON_ZERO".into()));
        assert_eq!(
            clone_sql_mode("STRICT_TRANS_TABLES,NO_AUTO_VALUE_ON_ZERO"),
            None
        );
    }
}
