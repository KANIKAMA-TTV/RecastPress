//! Finite MySQL 8.4.13 GTID-OFF real-rotation RAM handoff, never publication.
//! The ordinary strict decoder and its one-file Position are unchanged. A separate
//! file-aware outer checkpoint stays at file1's XID while a bounded metadata
//! bridge is verified. Only a real file2 COMMIT callback installs its new pair.
//! No journal/restart/source-replacement/retention/continuous CDC contract exists.
//! Driver/input/expected copies and metadata/positions are unpooled. Only positive
//! view rows/index and retained row frames share the fixture's logical pool.
//! FLUSH BINARY LOGS is a one-time test-controller action after COMMIT, never
//! FTWRL, a write-pause strategy, or a production CDC-reader privilege requirement.
//! The observed next-file artificial Rotate is separately CRC/shape-validated
//! exactly once after the physical Rotate; it is not a physical cursor advance.
//! One owned blocking dump is killed after physical Rotate verification. Buffered
//! bytes are never read after the cut. A fresh attempt starts at unchanged C1;
//! its initial transport Rotate is TLS-protected, not a CRC-verified log event.
//! This requires the same alive owned source and both retained files; it does not
//! establish a source epoch or a restart/retention recovery protocol.

use crate::{ChecksumFault, buffer_adapter::ProbeBuffer, require_fixture_tls, verify_crc};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStream, BinlogStreamRequest, Conn, OptsBuilder, Value,
    binlog::{
        events::{BinlogEventFooter, BinlogEventHeader, Event, EventData, TableMapEvent},
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
const FIRST: &str = "probe-bin.000001";
const SECOND: &str = "probe-bin.000002";
const LIMIT: usize = 64 * 1024;
const SOURCE: &str = "rp_rotation_source";

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
        .ok_or("rotation handoff clone session SQL mode missing")?;
    let replacement = clone_sql_mode(&original);
    let insert: std::result::Result<(), mysql_async::Error> = async {
        if let Some(replacement) = &replacement {
            control
                .exec_drop("SET SESSION sql_mode = ?", (replacement,))
                .await?;
        }
        control
            .query_drop("INSERT INTO rp_rotation_source SELECT * FROM wp_postmeta")
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
            "rotation handoff source clone failed: {insert}; restoring session SQL mode also failed: {restore}"
        )
        .into()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error.into()),
    }
}

async fn schema(connection: &mut Conn) -> Result<()> {
    let columns: Vec<(String,String,String,String,Option<String>)> = connection.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_rotation_source' ORDER BY ORDINAL_POSITION").await?;
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
        return Err("rotation handoff source SQL schema differs".into());
    }
    let engine: Option<String> = connection.query_first("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_rotation_source'").await?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("rotation handoff source is not InnoDB".into());
    }
    Ok(())
}

async fn coordinate(connection: &mut Conn, status: &'static str) -> Result<(String, u64)> {
    let mut row: mysql_async::Row = connection
        .query_first(status)
        .await?
        .ok_or("rotation handoff source coordinate missing")?;
    let file: String = row
        .take("File")
        .ok_or("rotation handoff source file missing")?;
    let position: u64 = row
        .take("Position")
        .ok_or("rotation handoff source position missing")?;
    if ![FIRST, SECOND].contains(&file.as_str()) || position <= 4 {
        return Err("rotation handoff source coordinate invalid".into());
    }
    Ok((file, position))
}

fn decode(row: &BinlogRow) -> Result<PostMetaRow> {
    if row.len() != 4 {
        return Err("rotation handoff row image width differs".into());
    }
    let unsigned = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::UInt(value))) => Ok(*value),
        Some(BinlogValue::Value(Value::Int(value))) => Ok(u64::from_le_bytes(value.to_le_bytes())),
        _ => Err("rotation handoff SQL-verified unsigned column image differs"),
    };
    let bytes = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::NULL)) => Ok(None),
        Some(BinlogValue::Value(Value::Bytes(value))) => Ok(Some(value.clone())),
        _ => Err("rotation handoff nullable byte column image differs"),
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
            .ok_or("rotation handoff row size overflow")?;
    }
    if rows.len() > 11
        || bytes > LIMIT
        || rows
            .windows(2)
            .any(|pair| pair[0].meta_id >= pair[1].meta_id)
    {
        return Err("rotation handoff row count/order/bytes bound differs".into());
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

fn check_view(view: &BudgetedPostMetaView, expected: &Expected) -> Result<()> {
    if view.rows() != expected.rows {
        return Err("rotation handoff raw rows differ from SQL".into());
    }
    for (key, ids) in &expected.searches {
        let found = view.matching_key(key.as_deref());
        if found.plan() != SearchPlan::ExactKeyIndex
            || found.map(|r| (r.meta_id,)).collect::<Vec<_>>() != *ids
        {
            return Err("rotation handoff index differs from binary SQL search".into());
        }
    }
    Ok(())
}
fn build_index(view: &mut BudgetedPostMetaView) -> Result<()> {
    if view
        .try_build_key_index(64, LIMIT)
        .map_err(|_| "rotation handoff index pool refused")?
        != SearchPlan::ExactKeyIndex
    {
        return Err("rotation handoff index cap refused".into());
    }
    Ok(())
}

fn table_map(table: &TableMapEvent<'_>) -> Result<()> {
    if table.database_name_raw() != b"recastpress_fixture"
        || table.table_name_raw() != SOURCE.as_bytes()
        || table.columns_count() != 4
    {
        return Err("rotation handoff table map context/count differs".into());
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
        return Err("rotation handoff table map types/nulls differ".into());
    }
    Ok(())
}

async fn capture_expected(control: &mut Conn) -> Result<Expected> {
    let rows = rows(control).await?;
    let mut keys: BTreeSet<_> = rows.iter().map(|row| row.meta_key.clone()).collect();
    keys.insert(None);
    keys.insert(Some(b"maria-bootstrap-missing-key".to_vec()));
    let mut searches = Vec::new();
    for key in keys {
        let ids = match &key {
            Some(key) => control.exec("SELECT meta_id FROM rp_rotation_source WHERE CAST(meta_key AS BINARY)=CAST(? AS BINARY) ORDER BY meta_id",(key,)).await?,
            None => control.query("SELECT meta_id FROM rp_rotation_source WHERE meta_key IS NULL ORDER BY meta_id").await?,
        };
        searches.push((key, ids));
    }
    Ok(Expected { rows, searches })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum File {
    First,
    Second,
}
impl File {
    fn name(self) -> &'static str {
        match self {
            Self::First => FIRST,
            Self::Second => SECOND,
        }
    }
}
#[derive(Clone, Eq, PartialEq)]
struct Checkpoint {
    epoch: Vec<u8>,
    file: File,
    offset: u64,
}
impl Checkpoint {
    fn new(epoch: &[u8], file: File, offset: u64) -> Self {
        Self {
            epoch: epoch.to_vec(),
            file,
            offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BridgeFault {
    OpenTransaction,
    UnknownFile,
    MissingHeader,
    MissingAnnouncement,
    Position,
    Profile,
    Unexpected,
}
impl std::fmt::Display for BridgeFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rotation bridge fault: {self:?}")
    }
}
impl std::error::Error for BridgeFault {}
#[derive(Clone)]
enum BridgeFrame {
    Rotate {
        name: Vec<u8>,
        position: u64,
        size: u64,
        end: u64,
        artificial: bool,
        flags: u16,
    },
    Fde {
        size: u64,
        end: u64,
        valid: bool,
        artificial: bool,
    },
    Previous {
        size: u64,
        end: u64,
        empty: bool,
    },
}
#[derive(Clone)]
struct Bridge {
    phase: u8,
    old_commit: u64,
    old_size: u64,
    new_cursor: u64,
    new_header: u64,
    announced: bool,
}
impl Bridge {
    fn new(old_commit: u64, old_size: u64, new_header: u64) -> Self {
        Self {
            phase: 0,
            old_commit,
            old_size,
            new_cursor: 4,
            new_header,
            announced: false,
        }
    }
    fn accept(&mut self, frame: &BridgeFrame, idle: bool) -> std::result::Result<(), BridgeFault> {
        if !idle {
            return Err(BridgeFault::OpenTransaction);
        }
        match (self.phase, frame) {
            (
                0,
                BridgeFrame::Rotate {
                    name,
                    position,
                    size,
                    end,
                    artificial,
                    flags,
                },
            ) => {
                if name != SECOND.as_bytes() {
                    return Err(BridgeFault::UnknownFile);
                }
                if *position != 4 || *artificial || *flags != 0 {
                    return Err(BridgeFault::Profile);
                }
                // Some Rotate headers carry zero rather than an old-file end.
                // Only this classified, CRC-verified final frame may use zero:
                // its physical length MUST exactly reach SHOW BINARY LOGS EOF.
                if self.old_commit.checked_add(*size) != Some(self.old_size)
                    || ![0, self.old_size].contains(end)
                {
                    return Err(BridgeFault::Position);
                }
                self.phase = 1;
            }
            (
                1,
                BridgeFrame::Rotate {
                    name,
                    position,
                    size,
                    end,
                    artificial,
                    flags,
                },
            ) => {
                if self.announced {
                    return Err(BridgeFault::Unexpected);
                }
                if name != SECOND.as_bytes() {
                    return Err(BridgeFault::UnknownFile);
                }
                if !artificial
                    || *flags != mysql_async::binlog::EventFlags::LOG_EVENT_ARTIFICIAL_F.bits()
                {
                    return Err(BridgeFault::Profile);
                }
                if *position != 4
                    || *end != 0
                    || *size
                        != (BinlogEventHeader::LEN
                            + 8
                            + SECOND.len()
                            + BinlogEventFooter::BINLOG_CHECKSUM_LEN)
                            as u64
                {
                    return Err(BridgeFault::Position);
                }
                // This server-generated announcement is not a physical file
                // frame. It neither advances new_cursor=4 nor acknowledges data.
                self.announced = true;
            }
            (
                1,
                BridgeFrame::Fde {
                    size,
                    end,
                    valid,
                    artificial,
                },
            ) => {
                if !self.announced {
                    return Err(BridgeFault::MissingAnnouncement);
                }
                if !valid || *artificial {
                    return Err(BridgeFault::Profile);
                }
                if self.new_cursor.checked_add(*size) != Some(*end) || *end >= self.new_header {
                    return Err(BridgeFault::Position);
                }
                self.new_cursor = *end;
                self.phase = 2;
            }
            (2, BridgeFrame::Previous { size, end, empty }) => {
                if !empty {
                    return Err(BridgeFault::Profile);
                }
                if self.new_cursor.checked_add(*size) != Some(*end) || *end != self.new_header {
                    return Err(BridgeFault::Position);
                }
                self.new_cursor = *end;
                self.phase = 3;
            }
            (1, _) => return Err(BridgeFault::MissingHeader),
            _ => return Err(BridgeFault::Unexpected),
        }
        Ok(())
    }
}
fn envelope(event: &Event, fde: bool) -> Result<u64> {
    let expected = (BinlogEventHeader::LEN as u64)
        .checked_add(u64::try_from(event.data().len())?)
        .and_then(|n| {
            n.checked_add(
                (BinlogEventFooter::BINLOG_CHECKSUM_LEN
                    + if fde {
                        BinlogEventFooter::BINLOG_CHECKSUM_ALG_DESC_LEN
                    } else {
                        0
                    }) as u64,
            )
        })
        .ok_or("rotation envelope overflow")?;
    if u64::from(event.header().event_size()) != expected || !verify_crc(event)? {
        return Err("rotation physical frame envelope/CRC absent".into());
    }
    Ok(expected)
}
fn bridge_frame(event: &Event, data: &EventData<'_>) -> Result<BridgeFrame> {
    let artificial = event
        .header()
        .flags()
        .contains(mysql_async::binlog::EventFlags::LOG_EVENT_ARTIFICIAL_F);
    let end = u64::from(event.header().log_pos());
    Ok(match data {
        EventData::RotateEvent(rotate) => BridgeFrame::Rotate {
            name: rotate.name_raw().to_vec(),
            position: rotate.position(),
            size: envelope(event, false)?,
            end,
            artificial,
            flags: event.header().flags_raw(),
        },
        EventData::FormatDescriptionEvent(fde) => BridgeFrame::Fde {
            size: envelope(event, true)?,
            end,
            valid: fde.binlog_version() == mysql_async::binlog::BinlogVersion::Version4
                && fde.event_header_length() == 19,
            artificial,
        },
        EventData::PreviousGtidsEvent(previous) => BridgeFrame::Previous {
            size: envelope(event, false)?,
            end,
            empty: previous.sids().is_empty(),
        },
        _ => return Err(BridgeFault::Unexpected.into()),
    })
}
fn corrupted_crc_is_rejected(event: &Event) -> Result<()> {
    let mut wire = Vec::new();
    event.write(mysql_async::binlog::BinlogVersion::Version4, &mut wire)?;
    *wire.last_mut().ok_or("rotation checksum suffix absent")? ^= 1;
    let damaged = Event::read(event.fde(), wire.as_slice())?;
    if !matches!(verify_crc(&damaged), Err(ChecksumFault::Mismatch)) {
        return Err("rotation corrupted checksum not specifically rejected".into());
    }
    Ok(())
}
fn require_bridge_fault(
    result: std::result::Result<(), BridgeFault>,
    expected: BridgeFault,
) -> Result<()> {
    if result != Err(expected) {
        return Err("rotation injected metadata fault reason differs".into());
    }
    Ok(())
}
fn nonce_valid(nonce: &str) -> bool {
    nonce.len() == 32
        && nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
async fn open_stream(options: OptsBuilder, file: File, start: u64) -> Result<BinlogStream> {
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    Ok(connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7051)
                .with_filename(file.name().as_bytes())
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?)
}

#[derive(Eq, PartialEq)]
struct SourceIdentity {
    account: String,
    version: String,
    datadir: std::path::PathBuf,
    port: u16,
    uuid: String,
    gtid: String,
    format: String,
    image: String,
    checksum: String,
    compression: u8,
}
type IdentityRow = (
    u64,
    String,
    String,
    String,
    u16,
    String,
    String,
    String,
    String,
    String,
    u8,
);
async fn identity(connection: &mut Conn) -> Result<(u32, SourceIdentity)> {
    let row:Option<IdentityRow>=connection.query_first("SELECT CONNECTION_ID(),CURRENT_USER(),VERSION(),@@datadir,@@port,@@server_uuid,@@GLOBAL.gtid_mode,@@GLOBAL.binlog_format,@@GLOBAL.binlog_row_image,@@GLOBAL.binlog_checksum,@@GLOBAL.binlog_transaction_compression").await?;
    let (id, account, version, datadir, port, uuid, gtid, format, image, checksum, compression) =
        row.ok_or("rotation source identity missing")?;
    if id != u64::from(connection.id()) || id == 0 {
        return Err("rotation handshake and SQL connection IDs differ".into());
    }
    Ok((
        connection.id(),
        SourceIdentity {
            account,
            version,
            datadir: std::path::PathBuf::from(datadir).canonicalize()?,
            port,
            uuid,
            gtid,
            format,
            image,
            checksum,
            compression,
        },
    ))
}
async fn open_owned_dump(
    options: OptsBuilder,
    start: u64,
    blocking: bool,
    source: &SourceIdentity,
) -> Result<(u32, BinlogStream)> {
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let (id, actual) = identity(&mut connection).await?;
    if actual != *source {
        return Err("rotation reader belongs to another source/account/profile".into());
    }
    let request = BinlogStreamRequest::new(7051)
        .with_filename(FIRST.as_bytes())
        .with_pos(start);
    let request = if blocking {
        request
    } else {
        request.with_non_blocking()
    };
    Ok((id, connection.get_binlog_stream(request).await?))
}
type ProcessRow = (u32, String, String);
async fn owned_processes(
    control: &mut Conn,
    control_id: u32,
    account: &str,
) -> Result<Vec<ProcessRow>> {
    let rows: Vec<mysql_async::Row> = control.query("SHOW PROCESSLIST").await?;
    if rows.len() > 16 {
        return Err("rotation processlist bound exceeded".into());
    }
    let mut processes = Vec::new();
    for mut row in rows {
        let id: u32 = row.take("Id").ok_or("rotation processlist ID missing")?;
        let user: String = row
            .take("User")
            .ok_or("rotation processlist user missing")?;
        let command: String = row
            .take("Command")
            .ok_or("rotation processlist command missing")?;
        if user.len() > 64 || command.len() > 32 {
            return Err("rotation processlist fields exceed bounds".into());
        }
        processes.push((id, user, command));
    }
    let user = account
        .split('@')
        .next()
        .ok_or("rotation account missing")?;
    if processes
        .iter()
        .filter(|row| row.0 == control_id && row.1 == user)
        .count()
        != 1
    {
        return Err("rotation processlist cannot prove its own control is visible".into());
    }
    Ok(processes)
}
fn require_owned_dump(
    processes: &[ProcessRow],
    control_id: u32,
    dump_id: u32,
    account: &str,
) -> Result<()> {
    if dump_id == control_id || dump_id == 0 {
        return Err("rotation refuses to terminate control or unknown ID".into());
    }
    let user = account
        .split('@')
        .next()
        .ok_or("rotation account missing")?;
    if processes
        .iter()
        .filter(|row| row.0 == dump_id && row.1 == user && row.2 == "Binlog Dump")
        .count()
        != 1
    {
        return Err("rotation owned blocking dump is not live under the same account".into());
    }
    Ok(())
}
async fn terminate_owned_dump(
    control: &mut Conn,
    control_id: u32,
    dump_id: u32,
    source: &SourceIdentity,
) -> Result<()> {
    let (actual_id, actual) = identity(control).await?;
    if actual_id != control_id || actual != *source {
        return Err("rotation control source changed before owned cut".into());
    }
    let processes = owned_processes(control, control_id, &source.account).await?;
    require_owned_dump(&processes, control_id, dump_id, &source.account)?;
    // Only this captured u32 ID is interpolated, never an external SQL fragment.
    // No new privilege is requested: both sessions use the same fixture account.
    control
        .query_drop(format!("KILL CONNECTION {dump_id}"))
        .await?;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let processes = owned_processes(control, control_id, &source.account).await?;
            if !processes.iter().any(|row| row.0 == dump_id) {
                return Ok::<_, Box<dyn std::error::Error>>(());
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await??;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PreambleFault {
    Rotate,
    Fde,
}
impl std::fmt::Display for PreambleFault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "rotation retry preamble fault: {self:?}")
    }
}
impl std::error::Error for PreambleFault {}
fn validate_initial_rotate(
    name: &[u8],
    position: u64,
    expected: u64,
    end: u64,
    size: u64,
    flags: u16,
    checksum: bool,
) -> std::result::Result<(), PreambleFault> {
    if name != FIRST.as_bytes()
        || position != expected
        || end != 0
        || size != (BinlogEventHeader::LEN + 8 + FIRST.len()) as u64
        || flags != 32
        || checksum
    {
        return Err(PreambleFault::Rotate);
    }
    Ok(())
}
fn validate_initial_fde(
    end: u64,
    size: u64,
    flags: u16,
    valid: bool,
    crc: bool,
) -> std::result::Result<(), PreambleFault> {
    // The observed initial FDE is a transport descriptor with end zero,
    // distinct from the physical file-header FDE ending at 127.
    if end != 0 || size != 123 || flags != 0 || !valid || !crc {
        return Err(PreambleFault::Fde);
    }
    Ok(())
}
async fn retry_preamble(stream: &mut BinlogStream, epoch: &[u8], c1: u64) -> Result<ProbeBuffer> {
    let mut ordinary = ProbeBuffer::resumed(epoch, FIRST.as_bytes(), c1, false);
    let event = stream
        .next()
        .await
        .ok_or("rotation retry ended before initial Rotate")??;
    let data = event
        .read_data()?
        .ok_or("rotation retry initial Rotate undecoded")?;
    if let EventData::RotateEvent(rotate) = &data {
        validate_initial_rotate(rotate.name_raw(),rotate.position(),c1,u64::from(event.header().log_pos()),u64::from(event.header().event_size()),event.header().flags_raw(),event.checksum().is_some())
            .map_err(|error|format!("{error} (file_match={}, position={}, requested={c1}, end={}, size={}, flags={}, checksum={})",rotate.name_raw()==FIRST.as_bytes(),rotate.position(),event.header().log_pos(),event.header().event_size(),event.header().flags_raw(),event.checksum().is_some()))?;
    } else {
        return Err(PreambleFault::Rotate.into());
    }
    // This size43 artificial transport notification is protected by TLS, not
    // a CRC assertion. It is never counted as a physical/CRC-verified Rotate.
    ordinary.accept(&event, &data)?;
    let event = stream
        .next()
        .await
        .ok_or("rotation retry ended before initial FDE")??;
    let data = event
        .read_data()?
        .ok_or("rotation retry initial FDE undecoded")?;
    if let EventData::FormatDescriptionEvent(fde) = &data {
        let valid = fde.binlog_version() == mysql_async::binlog::BinlogVersion::Version4
            && fde.event_header_length() == 19;
        validate_initial_fde(
            u64::from(event.header().log_pos()),
            envelope(&event, true)?,
            event.header().flags_raw(),
            valid,
            verify_crc(&event)?,
        )
        .map_err(|error| {
            format!(
                "{error} (end={}, size={}, flags={}, checksum={})",
                event.header().log_pos(),
                event.header().event_size(),
                event.header().flags_raw(),
                event.checksum().is_some()
            )
        })?;
    } else {
        return Err(PreambleFault::Fde.into());
    }
    ordinary.accept(&event, &data)?;
    ordinary.finish()?;
    if ordinary.checkpoint_offset() != c1 || ordinary.applied != 0 {
        return Err("rotation retry preamble advanced its checkpoint".into());
    }
    Ok(ordinary)
}

struct Scan {
    ordinary: ProbeBuffer,
    table_ids: BTreeSet<u64>,
    refused: bool,
    budget_refused: bool,
}
#[allow(clippy::too_many_arguments)]
async fn scan_commit(
    stream: &mut BinlogStream,
    epoch: &[u8],
    file: File,
    start: u64,
    end: u64,
    current: &mut BudgetedPostMetaView,
    outer: &mut Checkpoint,
    prior: &Expected,
    expected: &Expected,
    pool: &BudgetPool,
    negative: bool,
) -> Result<Scan> {
    let before = outer.clone();
    let mut staged =
        ProbeBuffer::resumed_with_pool(epoch, file.name().as_bytes(), start, false, pool.clone());
    let mut ordinary = ProbeBuffer::resumed(epoch, file.name().as_bytes(), start, false);
    let mut rejected =
        negative.then(|| ProbeBuffer::resumed(epoch, file.name().as_bytes(), start, false));
    let empty = BudgetPool::new(0);
    let mut denied = negative.then(|| {
        ProbeBuffer::resumed_with_pool(epoch, file.name().as_bytes(), start, false, empty.clone())
    });
    let mut pending = Vec::new();
    let mut frames = Vec::new();
    let mut table_ids = BTreeSet::new();
    let mut refused = false;
    let mut budget_refused = false;
    let mut count = 0;
    while let Some(event) = stream.next().await {
        let event = event?;
        count += 1;
        if count > 32 || event.data().len() > LIMIT {
            return Err("rotation transaction event bound exceeded".into());
        }
        verify_crc(&event)?;
        let data = event
            .read_data()?
            .ok_or("rotation transaction event undecoded")?;
        if let EventData::TableMapEvent(map) = &data {
            table_map(map)?;
        }
        if let EventData::RowsEvent(rows) = &data {
            let map = stream
                .get_tme(rows.table_id())
                .ok_or("rotation row table map missing")?;
            table_map(map)?;
            table_ids.insert(rows.table_id());
            frames.push(event.data().to_vec());
            for row in rows.rows(map) {
                let (old, new) = row?;
                pending.push(match (old, new) {
                    (Some(old), Some(new)) => MetaChange::Update {
                        before: decode(&old)?,
                        after: decode(&new)?,
                    },
                    _ => return Err("rotation fixture requires one full UPDATE".into()),
                });
                if pending.len() > 1 {
                    return Err("rotation fixture UPDATE count exceeded".into());
                }
            }
        }
        if let Some(buffer) = denied.as_mut()
            && let Err(error) =
                buffer.accept_projected(&event, &data, |_| Err("zero-pool rotation applied".into()))
        {
            if !matches!(data, EventData::RowsEvent(_))
                || buffer.budget_refusal() != Some(BudgetError::Exhausted)
                || buffer.fault() != Some(Fault::PayloadBudgetExceeded)
            {
                return Err(error);
            }
            buffer.verify_fault_at_initial_checkpoint()?;
            if empty
                .stats()
                .map_err(|_| "rotation empty pool unavailable")?
                .claimed
                != 0
            {
                return Err("rotation empty pool leaked credit".into());
            }
            check_view(current, prior)?;
            if *outer != before {
                return Err("rotation pool refusal advanced outer pair".into());
            }
            budget_refused = true;
            denied = None;
        }
        let commit = matches!(data, EventData::XidEvent(_));
        if let Some(buffer) = rejected.as_mut() {
            let result = buffer.accept_projected(&event, &data, |_| {
                match current.prepare_changes(&pending, 64, 0) {
                    Err(error) => Err(error.into()),
                    Ok(_) => Err("rotation zero logical limit accepted".into()),
                }
            });
            if commit {
                if !matches!(
                    result
                        .as_ref()
                        .err()
                        .and_then(|e| e.downcast_ref::<BudgetedViewError>()),
                    Some(BudgetedViewError::Change(
                        LimitedChangeError::ByteLimitExceeded { limit: 0 }
                    ))
                ) || buffer.checkpoint_offset() != start
                    || buffer.applied != 0
                {
                    return Err("rotation typed callback refusal differs".into());
                }
                check_view(current, prior)?;
                if *outer != before {
                    return Err("rotation callback refusal advanced outer pair".into());
                }
                refused = true;
                rejected = None;
            } else {
                result?;
            }
        }
        let mut candidate = if commit {
            if pending.len() != 1 || frames.len() != 1 {
                return Err("rotation full-image batch shape differs".into());
            }
            let mut next = current.prepare_changes(&pending, 64, LIMIT)?;
            build_index(&mut next)?;
            check_view(&next, expected)?;
            Some(next)
        } else {
            None
        };
        ordinary.accept(&event, &data)?;
        staged.accept_projected(&event, &data, |batch| {
            if batch.from().offset() != start
                || batch.through().offset() != end
                || !batch.rows().eq(frames.iter().map(Vec::as_slice))
                || *outer != before
            {
                return Err("rotation batch or outer handoff context differs".into());
            }
            let claims = pool
                .stats()
                .map_err(|_| "rotation shared pool unavailable")?;
            if [
                ResourceKind::InternalRows,
                ResourceKind::Index,
                ResourceKind::ChangeBuffer,
            ]
            .iter()
            .any(|kind| claims.claimed_for(*kind) == 0)
            {
                return Err("rotation did not share all three logical lease kinds".into());
            }
            *current = candidate.take().ok_or("rotation apply has no candidate")?;
            *outer = Checkpoint::new(epoch, file, end);
            Ok(())
        })?;
        if commit {
            staged.finish()?;
            ordinary.finish()?;
            if staged.checkpoint_offset() != end
                || staged.applied != 1
                || *outer != Checkpoint::new(epoch, file, end)
                || (negative && !(refused && budget_refused))
            {
                return Err("rotation transaction did not finish at its expected pair".into());
            }
            return Ok(Scan {
                ordinary,
                table_ids,
                refused,
                budget_refused,
            });
        }
    }
    Err("rotation source ended before its expected COMMIT".into())
}

async fn update(control: &mut Conn, value: &[u8]) -> Result<()> {
    control.query_drop("START TRANSACTION").await?;
    control
        .exec_drop(
            "UPDATE rp_rotation_source SET meta_value=? WHERE meta_id=3",
            (value,),
        )
        .await?;
    control.query_drop("COMMIT").await?;
    Ok(())
}
async fn file_sizes(control: &mut Conn) -> Result<(u64, u64)> {
    let logs: Vec<mysql_async::Row> = control.query("SHOW BINARY LOGS").await?;
    if logs.len() != 2 {
        return Err("rotation fixture must retain exactly two logs".into());
    }
    let mut sizes = Vec::new();
    for (mut row, name) in logs.into_iter().zip([FIRST, SECOND]) {
        let actual: String = row.take("Log_name").ok_or("rotation log name missing")?;
        let size: u64 = row.take("File_size").ok_or("rotation log size missing")?;
        if actual != name || size <= 4 {
            return Err("rotation retained log identity differs".into());
        }
        sizes.push(size);
    }
    Ok((sizes[0], sizes[1]))
}

pub(crate) async fn run(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    version: &str,
) -> Result<Json> {
    let nonce = std::env::var("RECASTPRESS_ROTATION_NONCE")?;
    if version != "8.4.13"
        || std::env::var("RECASTPRESS_DB_EXPECT_VERSION").as_deref() != Ok("8.4.13")
        || std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable")
        || !nonce_valid(&nonce)
    {
        return Err("rotation requires its owned pinned MySQL fixture and nonce".into());
    }
    let profile:Option<(String,String,String,String,u8)>=control.query_first("SELECT @@GLOBAL.gtid_mode,@@GLOBAL.binlog_format,@@GLOBAL.binlog_row_image,@@GLOBAL.binlog_checksum,@@GLOBAL.binlog_transaction_compression").await?;
    if profile != Some(("OFF".into(), "ROW".into(), "FULL".into(), "CRC32".into(), 0)) {
        return Err("rotation fixture profile must be GTID OFF ROW FULL CRC32 uncompressed".into());
    }
    let (control_id, source_identity) = identity(control).await?;
    let original = raw_rows(control, "wp_postmeta").await?;
    if original.len() != 11
        || original.first().map(|r| r.meta_id) != Some(0)
        || original.last().map(|r| r.meta_id) != Some(u64::MAX)
        || !original.iter().any(|r| r.meta_id == 3)
    {
        return Err("rotation initial eleven-row identities differ".into());
    }
    control
        .query_drop("CREATE TABLE rp_rotation_source LIKE wp_postmeta")
        .await?;
    clone_rows_preserving_zero(control).await?;
    schema(control).await?;
    if rows(control).await? != original {
        return Err("rotation clone lost source bytes or zero IDs".into());
    }
    let initial = capture_expected(control).await?;
    let (file, a) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if file != FIRST {
        return Err("rotation must start in first file".into());
    }
    update(control, b"rotation-one").await?;
    let (file, c1) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if file != FIRST || c1 <= a {
        return Err("rotation T1 did not advance first file".into());
    }
    let one = capture_expected(control).await?;
    // Exactly one controller-only rotation, after the source COMMIT. No read
    // lock or table/global write pause is requested. Internal log synchronization
    // is not claimed absent. The harness grants RELOAD only in this new fixture.
    control.query_drop("FLUSH BINARY LOGS").await?;
    let (file, h2) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if file != SECOND {
        return Err("rotation did not create exact second file".into());
    }
    let (old_size, new_size) = file_sizes(control).await?;
    if old_size <= c1 || new_size != h2 {
        return Err("rotation file boundaries differ before T2".into());
    }
    update(control, b"rotation-two").await?;
    let (file, c2) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if file != SECOND || c2 <= h2 {
        return Err("rotation T2 did not follow the new header".into());
    }
    let two = capture_expected(control).await?;
    let pool = BudgetPool::new(1024 * 1024);
    let mut current = BudgetedPostMetaView::clone_from_rows(&original, pool.clone(), 64, LIMIT)?;
    build_index(&mut current)?;
    check_view(&current, &initial)?;
    let mut outer = Checkpoint::new(epoch, File::First, a);
    let (dump_id, mut cut_stream) =
        open_owned_dump(options.clone(), a, true, &source_identity).await?;
    if dump_id == control_id {
        return Err("rotation dump reused its control ID".into());
    }
    let cut = tokio::time::timeout(Duration::from_secs(15), async {
        let first = scan_commit(
            &mut cut_stream,
            epoch,
            File::First,
            a,
            c1,
            &mut current,
            &mut outer,
            &initial,
            &one,
            &pool,
            false,
        )
        .await?;
        let event = cut_stream
            .next()
            .await
            .ok_or("rotation cut ended before physical Rotate")??;
        if event.data().len() > LIMIT {
            return Err("rotation cut frame bound exceeded".into());
        }
        let data = event.read_data()?.ok_or("rotation cut frame undecoded")?;
        let frame = bridge_frame(&event, &data)?;
        // Phase zero accepts only the CRC-verified real Rotate, never a transport
        // notification. No decoder/raw/map state from this attempt is reused.
        let mut bridge = Bridge::new(c1, old_size, h2);
        bridge.accept(&frame, true)?;
        if bridge.phase != 1
            || first.table_ids.is_empty()
            || first
                .table_ids
                .iter()
                .any(|id| cut_stream.get_tme(*id).is_some())
            || first.ordinary.checkpoint_offset() != c1
            || outer != Checkpoint::new(epoch, File::First, c1)
        {
            return Err(
                "rotation cut did not retain its complete C1 pair or clear driver maps".into(),
            );
        }
        check_view(&current, &one)?;
        let reservations = pool.stats().map_err(|_| "rotation cut pool unavailable")?;
        terminate_owned_dump(control, control_id, dump_id, &source_identity).await?;
        // Do not poll this stream again, even if its userspace receive buffer
        // still contains more frames. Processlist disappearance proves the cut.
        check_view(&current, &one)?;
        if outer != Checkpoint::new(epoch, File::First, c1)
            || pool.stats().map_err(|_| "rotation cut pool unavailable")? != reservations
        {
            return Err("rotation owned cut changed its C1 pair or leases".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(())
    })
    .await;
    drop(cut_stream);
    cut??;
    let (actual_id, actual_source) = identity(control).await?;
    if actual_id != control_id
        || actual_source != source_identity
        || file_sizes(control).await? != (old_size, c2)
    {
        return Err(
            "rotation retry requires the same alive source with both unchanged retained files"
                .into(),
        );
    }
    let (retry_id, mut first_stream) =
        open_owned_dump(options.clone(), c1, false, &source_identity).await?;
    if retry_id == dump_id || retry_id == control_id {
        return Err("rotation retry ID was not fresh".into());
    }
    let scan = tokio::time::timeout(Duration::from_secs(15), async {
        let mut ordinary=retry_preamble(&mut first_stream,epoch,c1).await?;
        let before = outer.clone();
        let mut bridge = Bridge::new(c1, old_size, h2);
        let mut after_rotate: Option<Bridge> = None;
        let mut count = 0;
        let mut rotate = false;
        let mut fde = false;
        let mut missing = false;

        let mut ordinary_rejected = false;
        while let Some(event) = first_stream.next().await {
            let event = event?;
            count += 1;
            if count > 4 || event.data().len() > LIMIT {
                return Err("rotation bridge event bound exceeded".into());
            }
            let data = event
                .read_data()?
                .ok_or("rotation bridge event undecoded")?;
            let frame = bridge_frame(&event, &data)?;
            match &frame {
                BridgeFrame::Rotate {
                    position,
                    size,
                    end,
                    artificial,
                    flags,
                    ..
                } => {
                  if !ordinary_rejected {
                    corrupted_crc_is_rejected(&event)?;
                    let mut unknown = Bridge::new(c1, old_size, h2);
                    let altered = BridgeFrame::Rotate {
                        name: b"probe-bin.000003".to_vec(),
                        position: *position,
                        size: *size,
                        end: *end,
                        artificial: *artificial,
                        flags: *flags,
                    };
                    require_bridge_fault(unknown.accept(&altered, true), BridgeFault::UnknownFile)?;
                    require_bridge_fault(
                        Bridge::new(c1, old_size, h2).accept(&frame, false),
                        BridgeFault::OpenTransaction,
                    )?;
                    let error = ordinary.accept(&event, &data)
                        .err()
                        .ok_or("ordinary strict decoder accepted real rotation")?;
                    if !error
                        .to_string()
                        .starts_with("rotation requires coordinator reconciliation")
                        || ordinary.checkpoint_offset() != c1
                    {
                        return Err(format!("ordinary rotation refusal reason/checkpoint differs (error={error}, cp={}, c1={c1}, fault={:?}, event_index={count}, bridge_phase={}, prior_rotation_rejected={}, type={}, end={}, size={}, flags={})",ordinary.checkpoint_offset(),ordinary.fault(),bridge.phase,ordinary_rejected,event.header().event_type_raw(),event.header().log_pos(),event.header().event_size(),event.header().flags_raw()).into());
                    }
                    ordinary_rejected = true;
                    rotate = true;
                  }
                }
                BridgeFrame::Fde { .. } => {
                    corrupted_crc_is_rejected(&event)?;
                    fde = true;
                }
                BridgeFrame::Previous { .. } => {
                    let mut rejected = after_rotate
                        .clone()
                        .ok_or("rotation proof before Rotate missing")?;
                    require_bridge_fault(
                        rejected.accept(&frame, true),
                        BridgeFault::MissingHeader,
                    )?;
                    missing = true;
                }
            }
            let rotate_target=match &frame {BridgeFrame::Rotate{name,position,..}=>Some((name==SECOND.as_bytes(),*position)),_=>None};
            bridge.accept(&frame, true).map_err(|error| format!(
                "{error} (phase={}, type={}, end={}, size={}, data_len={}, flags={}, old_commit={}, old_size={}, new_cursor={}, new_header={}, rotate_file_match_and_position={rotate_target:?})",
                bridge.phase,event.header().event_type_raw(),event.header().log_pos(),event.header().event_size(),event.data().len(),event.header().flags_raw(),c1,old_size,bridge.new_cursor,h2))?;
            if matches!(frame, BridgeFrame::Rotate { .. }) {
                after_rotate = Some(bridge.clone());
            }
            if outer != before {
                return Err("rotation metadata advanced the outer checkpoint".into());
            }
            check_view(&current, &one)?;
            if bridge.phase == 3 {
                break;
            }
        }
        if bridge.phase != 3
            || !(rotate && fde && missing && ordinary_rejected)
        {
            return Err("rotation real bridge observations incomplete".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(usize::from(bridge.announced))
    })
    .await;
    let close = first_stream.close().await;
    let artificial_rotates = scan??;
    close?;
    if outer != Checkpoint::new(epoch, File::First, c1) {
        return Err("rotation metadata published a new-file checkpoint".into());
    }
    let mut second_stream = open_stream(options.clone(), File::Second, h2).await?;
    let second = tokio::time::timeout(
        Duration::from_secs(15),
        scan_commit(
            &mut second_stream,
            epoch,
            File::Second,
            h2,
            c2,
            &mut current,
            &mut outer,
            &one,
            &two,
            &pool,
            true,
        ),
    )
    .await;
    let close = second_stream.close().await;
    let second = second??;
    close?;
    if !second.refused
        || !second.budget_refused
        || outer != Checkpoint::new(epoch, File::Second, c2)
    {
        return Err("rotation first new-file commit did not atomically hand off".into());
    }
    drop(second);
    update(control, b"rotation-three").await?;
    let (file, c3) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if file != SECOND || c3 <= c2 {
        return Err("rotation T3 strict seam coordinate differs".into());
    }
    let three = capture_expected(control).await?;
    let mut third_stream = open_stream(options, File::Second, c2).await?;
    let third = tokio::time::timeout(
        Duration::from_secs(15),
        scan_commit(
            &mut third_stream,
            epoch,
            File::Second,
            c2,
            c3,
            &mut current,
            &mut outer,
            &two,
            &three,
            &pool,
            false,
        ),
    )
    .await;
    let close = third_stream.close().await;
    drop(third??);
    close?;
    check_view(&current, &three)?;
    let (final_old, final_new) = file_sizes(control).await?;
    if outer != Checkpoint::new(epoch, File::Second, c3)
        || final_old != old_size
        || final_new != c3
        || raw_rows(control, "wp_postmeta").await? != original
    {
        return Err("rotation final source/pair/original WP bytes differ".into());
    }
    drop(current);
    if pool
        .stats()
        .map_err(|_| "rotation pool unavailable at final drop")?
        .claimed
        != 0
    {
        return Err("rotation leaked shared reservations".into());
    }
    let mut result = json!({"format_version":1,"profile":"mysql_8_4_13_unpublished_rotation_handoff","process_pid":std::process::id(),"nonce":nonce,"source_files":[FIRST,SECOND],
        "start_position":a,"first_commit_position":c1,"old_file_size":old_size,"new_header_position":h2,"second_commit_position":c2,"strict_position":c3,
        "rows":11,"commits":3,"handoffs":1,"real_rotate_events":1,"artificial_rotate_events":artificial_rotates,"real_fde_events":1,"shared_pool_commits":3,"row_data_matches":true,"index_matches":true,
        "outer_checkpoint_preserved":true,"corrupt_rotate_rejected":true,"corrupt_fde_rejected":true,"unknown_file_rejected":true,"missing_header_rejected":true,"open_transaction_rejected":true,
        "apply_refusal_preserved":true,"budget_refusal_preserved":true,"driver_table_map_cleared":true,"shared_pool_released":true,"ordinary_rotation_rejected":true,"unpublished_only":true});
    let fields = result
        .as_object_mut()
        .ok_or("rotation result was not an object")?;
    fields.insert("owned_dump_terminated".into(), json!(true));
    fields.insert("bridge_reconnects".into(), json!(1));
    fields.insert("retry_checkpoint_preserved".into(), json!(true));
    fields.insert("retry_preamble_verified".into(), json!(true));
    fields.insert("same_owned_server_verified".into(), json!(true));
    fields.insert(
        "rotation_event_count_scope".into(),
        json!("successful_bridge_only"),
    );
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn owned_cut_requires_a_distinct_live_same_account_dump() {
        let live = vec![
            (7, "fixture".into(), "Query".into()),
            (8, "fixture".into(), "Binlog Dump".into()),
        ];
        require_owned_dump(&live, 7, 8, "fixture@localhost").unwrap();
        for id in [0, 7, 9] {
            assert!(require_owned_dump(&live, 7, id, "fixture@localhost").is_err());
        }
        for process in [
            (8, "other".into(), "Binlog Dump".into()),
            (8, "fixture".into(), "Sleep".into()),
            (8, "fixture".into(), "Binlog Dump GTID".into()),
        ] {
            assert!(require_owned_dump(&[process], 7, 8, "fixture@localhost").is_err());
        }
        let mut duplicate = live.clone();
        duplicate.push(live[1].clone());
        assert!(require_owned_dump(&duplicate, 7, 8, "fixture@localhost").is_err());
    }

    #[test]
    fn retry_preamble_is_not_a_physical_rotate_and_requires_exact_fde() {
        // This validator is shared with the runtime. A checksummed next-file
        // announcement cannot substitute for the checksum-less initial notice.
        validate_initial_rotate(FIRST.as_bytes(), 100, 100, 0, 43, 32, false).unwrap();
        for (name, position, end, size, flags, checksum) in [
            (SECOND.as_bytes(), 100, 0, 43, 32, false),
            (b"../probe-bin.000001".as_slice(), 100, 0, 43, 32, false),
            (FIRST.as_bytes(), 4, 0, 43, 32, false),
            (FIRST.as_bytes(), 100, 100, 43, 32, false),
            (FIRST.as_bytes(), 100, 0, 47, 32, true),
            (FIRST.as_bytes(), 100, 0, 43, 0, false),
            (FIRST.as_bytes(), 100, 0, 43, 160, false),
            (FIRST.as_bytes(), 100, 0, 43, 32, true),
        ] {
            assert_eq!(
                validate_initial_rotate(name, position, 100, end, size, flags, checksum),
                Err(PreambleFault::Rotate)
            );
        }
        validate_initial_fde(0, 123, 0, true, true).unwrap();
        for (end, size, flags, valid, crc) in [
            (127, 123, 0, true, true),
            (0, 122, 0, true, true),
            (0, 123, 32, true, true),
            (0, 123, 0, false, true),
            (0, 123, 0, true, false),
        ] {
            assert_eq!(
                validate_initial_fde(end, size, flags, valid, crc),
                Err(PreambleFault::Fde)
            );
        }
    }

    #[test]
    fn a_second_rotate_reports_the_latched_fault_without_advancing_checkpoint() {
        // Synthetic initialization frames exercise only classifier state here;
        // they do not stand in for the live CRC/physical bridge verification.
        fn rotate(name: &[u8], position: u64) -> Event {
            let mut wire = vec![0; BinlogEventHeader::LEN];
            wire[4] = 4;
            let size = (BinlogEventHeader::LEN + 8 + name.len()) as u32;
            wire[9..13].copy_from_slice(&size.to_le_bytes());
            wire.extend_from_slice(&position.to_le_bytes());
            wire.extend_from_slice(name);
            Event::read(
                &mysql_async::binlog::events::FormatDescriptionEvent::new(
                    mysql_async::binlog::BinlogVersion::Version4,
                ),
                wire.as_slice(),
            )
            .unwrap()
        }
        let mut ordinary = ProbeBuffer::resumed(b"synthetic", FIRST.as_bytes(), 100, false);
        let initial = rotate(FIRST.as_bytes(), 100);
        ordinary
            .accept(&initial, &initial.read_data().unwrap().unwrap())
            .unwrap();
        let next = rotate(SECOND.as_bytes(), 4);
        let first = ordinary
            .accept(&next, &next.read_data().unwrap().unwrap())
            .unwrap_err();
        assert!(
            first
                .to_string()
                .starts_with("rotation requires coordinator reconciliation")
        );
        assert_eq!(ordinary.checkpoint_offset(), 100);
        assert_eq!(ordinary.fault(), Some(Fault::Disconnected));
        let second = ordinary
            .accept(&next, &next.read_data().unwrap().unwrap())
            .unwrap_err();
        assert_eq!(
            second.to_string(),
            "adapter requires source reconciliation after a prior fault"
        );
        assert_eq!(ordinary.checkpoint_offset(), 100);
    }
    #[test]
    fn outer_positions_include_file_and_epoch_even_when_offsets_collide() {
        let old = Checkpoint::new(b"owned", File::First, 127);
        assert!(old != Checkpoint::new(b"owned", File::Second, 127));
        assert!(old != Checkpoint::new(b"other", File::First, 127));
        assert!(nonce_valid("0123456789abcdef0123456789abcdef"));
        assert!(!nonce_valid("0123456789ABCDEF0123456789abcdef"));
    }
    #[test]
    fn metadata_bridge_is_ordered_and_faults_do_not_advance_its_cursor() {
        let rotate = BridgeFrame::Rotate {
            name: SECOND.as_bytes().to_vec(),
            position: 4,
            size: 40,
            end: 140,
            artificial: false,
            flags: 0,
        };
        let fde = BridgeFrame::Fde {
            size: 120,
            end: 124,
            valid: true,
            artificial: false,
        };
        let previous = BridgeFrame::Previous {
            size: 31,
            end: 155,
            empty: true,
        };
        let mut bridge = Bridge::new(100, 140, 155);
        assert_eq!(
            bridge.accept(&rotate, false),
            Err(BridgeFault::OpenTransaction)
        );
        assert_eq!(bridge.phase, 0);
        bridge.accept(&rotate, true).unwrap();
        assert_eq!(
            bridge.accept(&previous, true),
            Err(BridgeFault::MissingHeader)
        );
        assert_eq!(bridge.phase, 1);
        let announcement = BridgeFrame::Rotate {
            name: SECOND.as_bytes().to_vec(),
            position: 4,
            size: 47,
            end: 0,
            artificial: true,
            flags: 32,
        };
        bridge.accept(&announcement, true).unwrap();
        bridge.accept(&fde, true).unwrap();
        bridge.accept(&previous, true).unwrap();
        assert_eq!(bridge.phase, 3);
        assert_eq!(bridge.accept(&rotate, true), Err(BridgeFault::Unexpected));
        let bad = BridgeFrame::Rotate {
            name: b"../probe-bin.000002".to_vec(),
            position: 4,
            size: 40,
            end: 140,
            artificial: false,
            flags: 0,
        };
        assert_eq!(
            Bridge::new(100, 140, 155).accept(&bad, true),
            Err(BridgeFault::UnknownFile)
        );
        assert_eq!(
            Bridge::new(101, 140, 155).accept(&rotate, true),
            Err(BridgeFault::Position)
        );
    }

    #[test]
    fn artificial_announcement_requires_exact_phase_flags_name_position_and_size() {
        let real = BridgeFrame::Rotate {
            name: SECOND.as_bytes().to_vec(),
            position: 4,
            size: 47,
            end: 147,
            artificial: false,
            flags: 0,
        };
        let announcement = BridgeFrame::Rotate {
            name: SECOND.as_bytes().to_vec(),
            position: 4,
            size: 47,
            end: 0,
            artificial: true,
            flags: 32,
        };
        let fde = BridgeFrame::Fde {
            size: 123,
            end: 127,
            valid: true,
            artificial: false,
        };
        let previous = BridgeFrame::Previous {
            size: 31,
            end: 158,
            empty: true,
        };
        let mut bridge = Bridge::new(100, 147, 158);
        assert_eq!(
            bridge.accept(&announcement, true),
            Err(BridgeFault::Profile)
        );
        bridge.accept(&real, true).unwrap();
        assert_eq!(
            bridge.accept(&fde, true),
            Err(BridgeFault::MissingAnnouncement)
        );
        let awaiting = bridge.clone();
        bridge.accept(&announcement, true).unwrap();
        assert!(bridge.announced);
        assert_eq!(bridge.phase, 1);
        assert_eq!(bridge.new_cursor, 4);
        assert_eq!(bridge.old_commit, 100);
        assert_eq!(
            bridge.accept(&announcement, true),
            Err(BridgeFault::Unexpected)
        );
        bridge.accept(&fde, true).unwrap();
        assert_eq!(
            bridge.accept(&announcement, true),
            Err(BridgeFault::Unexpected)
        );
        bridge.accept(&previous, true).unwrap();
        assert_eq!(bridge.phase, 3);
        for (name, position, size, end, artificial, flags, expected) in [
            (SECOND.as_bytes(), 4, 47, 0, true, 0, BridgeFault::Profile),
            (SECOND.as_bytes(), 4, 47, 0, true, 160, BridgeFault::Profile),
            (SECOND.as_bytes(), 4, 47, 0, false, 32, BridgeFault::Profile),
            (
                FIRST.as_bytes(),
                4,
                47,
                0,
                true,
                32,
                BridgeFault::UnknownFile,
            ),
            (
                b"../probe-bin.000002".as_slice(),
                4,
                47,
                0,
                true,
                32,
                BridgeFault::UnknownFile,
            ),
            (SECOND.as_bytes(), 0, 47, 0, true, 32, BridgeFault::Position),
            (
                SECOND.as_bytes(),
                158,
                47,
                0,
                true,
                32,
                BridgeFault::Position,
            ),
            (SECOND.as_bytes(), 4, 46, 0, true, 32, BridgeFault::Position),
            (SECOND.as_bytes(), 4, 48, 0, true, 32, BridgeFault::Position),
            (
                SECOND.as_bytes(),
                4,
                47,
                158,
                true,
                32,
                BridgeFault::Position,
            ),
        ] {
            let mut rejected = awaiting.clone();
            let frame = BridgeFrame::Rotate {
                name: name.to_vec(),
                position,
                size,
                end,
                artificial,
                flags,
            };
            assert_eq!(rejected.accept(&frame, true), Err(expected));
            assert_eq!(rejected.phase, 1);
            assert_eq!(rejected.new_cursor, 4);
            assert!(!rejected.announced);
        }
    }
}
