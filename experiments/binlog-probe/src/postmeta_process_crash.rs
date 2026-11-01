//! Owned-fixture client-process kill before SQL COMMIT, followed by finite replay.
//! The source is an eleven-row clone, never wp_postmeta. No server/OS crash,
//! continuous CDC, production storage, source identity or snapshot/log protocol.
//! A separate verify-only entry below inspects recovery after the parent kills
//! and restarts its owned MySQL process; it never reads/replays a binlog stream.

use crate::{
    buffer_adapter::ProbeBuffer, close_fixture_connection, normalize_maria_initial_rotate,
    require_fixture_tls, verify_crc,
};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStreamRequest, Conn, OptsBuilder, TxOpts, Value,
    binlog::{events::EventData, row::BinlogRow, value::BinlogValue},
    consts::ColumnType,
    prelude::Queryable,
};
use recastpress_db::{
    PostMetaRow,
    meta_view::{MetaChange, PostMetaView, SearchPlan},
};
use serde_json::{Map, Value as Json, json};
use std::{collections::BTreeSet, io::Write, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type EncodedRows = Vec<(u64, u64, Option<Vec<u8>>, Option<Vec<u8>>)>;
type Stored = (Vec<u8>, Vec<u8>, u64, u64, Vec<u8>, Vec<u8>);
const READY: &str = "POSTMETA_CRASH_READY=";
const PAYLOAD_LIMIT: usize = 12 * 1024;
const MARKER_LIMIT: usize = 64 * 1024;
const FIRST: &[u8] = b"crash-baseline-first";
const SECOND: &[u8] = b"crash-committed-second";

struct Image {
    from: u64,
    through: u64,
    receipt: Vec<u8>,
    rows: Vec<PostMetaRow>,
}

struct Marker {
    pid: u64,
    connection_id: u64,
    nonce: String,
    file: String,
    start: u64,
    baseline: u64,
    target: u64,
    old: Stored,
}

fn guard() -> Result<String> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
        return Err("process crash probe requires the owned fixture".into());
    }
    let nonce = std::env::var("RECASTPRESS_CRASH_NONCE")?;
    if nonce.len() != 32
        || !nonce
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("process crash nonce must be 32 lowercase hex digits".into());
    }
    Ok(nonce)
}

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
        .ok_or("crash clone session SQL mode missing")?;
    let replacement = clone_sql_mode(&original);
    let insert: std::result::Result<(), mysql_async::Error> = async {
        if let Some(replacement) = &replacement {
            control
                .exec_drop("SET SESSION sql_mode = ?", (replacement,))
                .await?;
        }
        control
            .query_drop("INSERT INTO rp_postmeta_crash_source SELECT * FROM wp_postmeta")
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
            "crash source clone failed: {insert}; restoring session SQL mode also failed: {restore}"
        )
        .into()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error.into()),
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut result = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        result.push(char::from(DIGITS[usize::from(byte >> 4)]));
        result.push(char::from(DIGITS[usize::from(byte & 15)]));
    }
    result
}

fn unhex(value: &Json, max_bytes: usize) -> Result<Vec<u8>> {
    let text = value.as_str().ok_or("crash hex field is not text")?;
    if text.len() % 2 != 0 || text.len() > max_bytes * 2 {
        return Err("crash hex field exceeds its bound".into());
    }
    let digit = |byte| match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err("crash hex field is not lowercase hex"),
    };
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| Ok(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect()
}

fn object<'a>(value: &'a Json, fields: &[&str]) -> Result<&'a Map<String, Json>> {
    let object = value.as_object().ok_or("crash JSON object missing")?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err("crash JSON fields are incomplete or unknown".into());
    }
    Ok(object)
}

fn uint(object: &Map<String, Json>, field: &str) -> Result<u64> {
    object[field]
        .as_u64()
        .ok_or_else(|| "crash field must be an unsigned integer".into())
}

fn file_valid(file: &str) -> bool {
    file.strip_prefix("probe-bin.")
        .is_some_and(|suffix| suffix.len() == 6 && suffix.bytes().all(|byte| byte.is_ascii_digit()))
}

fn tuple_json(tuple: &Stored) -> Json {
    json!({"epoch_hex":hex(&tuple.0),"file_hex":hex(&tuple.1),
        "from_position":tuple.2,"through_position":tuple.3,
        "receipt_hex":hex(&tuple.4),"row_data_hex":hex(&tuple.5)})
}

fn parse_marker(wire: &str, epoch: &[u8], nonce: &str) -> Result<Marker> {
    if wire.len() > MARKER_LIMIT || wire.contains(['\r', '\n']) {
        return Err("crash marker is not one bounded line".into());
    }
    let body = wire
        .strip_prefix(READY)
        .ok_or("crash READY prefix missing")?;
    let value: Json = serde_json::from_str(body)?;
    // READY is emitted as canonical serde_json JSON. Requiring the exact same
    // representation also rejects duplicate keys at every depth without adding
    // a new serde dependency or accepting overwritten duplicate values.
    if serde_json::to_string(&value)? != body {
        return Err("crash marker is noncanonical or contains duplicate fields".into());
    }
    let root = object(
        &value,
        &[
            "format_version",
            "nonce",
            "process_pid",
            "source_connection_id",
            "source_file",
            "start_position",
            "baseline_position",
            "target_position",
            "rows",
            "epoch_hex",
            "old_tuple",
        ],
    )?;
    if uint(root, "format_version")? != 1
        || uint(root, "rows")? != 11
        || root["nonce"].as_str() != Some(nonce)
        || unhex(&root["epoch_hex"], 64)? != epoch
    {
        return Err("crash marker fixture identity differs".into());
    }
    let file = root["source_file"]
        .as_str()
        .ok_or("crash source file is not text")?
        .to_owned();
    let pid = uint(root, "process_pid")?;
    let connection_id = uint(root, "source_connection_id")?;
    let start = uint(root, "start_position")?;
    let baseline = uint(root, "baseline_position")?;
    let target = uint(root, "target_position")?;
    if !file_valid(&file)
        || pid == 0
        || pid == u64::from(std::process::id())
        || connection_id == 0
        || !(4 < start && start < baseline && baseline < target)
    {
        return Err("crash marker PID, file or positions differ".into());
    }
    let old = object(
        &root["old_tuple"],
        &[
            "epoch_hex",
            "file_hex",
            "from_position",
            "through_position",
            "receipt_hex",
            "row_data_hex",
        ],
    )?;
    let old = (
        unhex(&old["epoch_hex"], 64)?,
        unhex(&old["file_hex"], 255)?,
        uint(old, "from_position")?,
        uint(old, "through_position")?,
        unhex(&old["receipt_hex"], PAYLOAD_LIMIT)?,
        unhex(&old["row_data_hex"], PAYLOAD_LIMIT)?,
    );
    if old.0 != epoch || old.1 != file.as_bytes() || old.2 != start || old.3 != baseline {
        return Err("crash marker old tuple context differs".into());
    }
    decode_rows(&old.5)?;
    validate_receipt(&old.4)?;
    Ok(Marker {
        pid,
        connection_id,
        nonce: nonce.to_owned(),
        file,
        start,
        baseline,
        target,
        old,
    })
}

fn encode_rows(rows: &[PostMetaRow]) -> Result<Vec<u8>> {
    if rows.len() != 11 {
        return Err("crash source must contain exactly eleven rows".into());
    }
    let encoded: EncodedRows = rows
        .iter()
        .map(|row| {
            (
                row.meta_id,
                row.post_id,
                row.meta_key.clone(),
                row.meta_value.clone(),
            )
        })
        .collect();
    let bytes = serde_json::to_vec(&encoded)?;
    if bytes.len() > PAYLOAD_LIMIT {
        return Err("crash row payload bound exceeded".into());
    }
    Ok(bytes)
}

fn decode_rows(bytes: &[u8]) -> Result<Vec<PostMetaRow>> {
    if bytes.len() > PAYLOAD_LIMIT {
        return Err("crash saved row payload bound exceeded".into());
    }
    let rows: EncodedRows = serde_json::from_slice(bytes)?;
    if rows.len() != 11 {
        return Err("crash saved row count differs".into());
    }
    let rows = rows
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect();
    Ok(PostMetaView::new(rows)
        .map_err(|_| "crash saved row identity repeats")?
        .rows()
        .to_vec())
}

fn validate_receipt(bytes: &[u8]) -> Result<()> {
    if bytes.is_empty() || bytes.len() > PAYLOAD_LIMIT {
        return Err("crash receipt bound exceeded".into());
    }
    let frames: Vec<Vec<u8>> = serde_json::from_slice(bytes)?;
    if frames.len() != 1 || frames[0].is_empty() {
        return Err("crash receipt must contain one row event".into());
    }
    Ok(())
}

async fn rows(connection: &mut Conn) -> Result<Vec<PostMetaRow>> {
    let rows: EncodedRows = connection.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_postmeta_crash_source ORDER BY meta_id LIMIT 12").await?;
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect();
    encode_rows(&rows)?;
    Ok(rows)
}

async fn schema(connection: &mut Conn) -> Result<()> {
    let columns: Vec<(String,String,String,String,Option<String>)> = connection.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_postmeta_crash_source' ORDER BY ORDINAL_POSITION").await?;
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
        return Err("crash source SQL schema differs".into());
    }
    let engine: Option<String> = connection.query_first("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_postmeta_crash_source'").await?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("crash source is not InnoDB".into());
    }
    Ok(())
}

async fn coordinate(connection: &mut Conn, status: &'static str) -> Result<(String, u64)> {
    let mut row: mysql_async::Row = connection
        .query_first(status)
        .await?
        .ok_or("crash source coordinate missing")?;
    let file: String = row.take("File").ok_or("crash source file missing")?;
    let position: u64 = row
        .take("Position")
        .ok_or("crash source position missing")?;
    if !file_valid(&file) || position <= 4 {
        return Err("crash source coordinate invalid".into());
    }
    Ok((file, position))
}

fn decode(row: &BinlogRow) -> Result<PostMetaRow> {
    if row.len() != 4 {
        return Err("crash row image width differs".into());
    }
    let unsigned = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::UInt(value))) => Ok(*value),
        Some(BinlogValue::Value(Value::Int(value))) => Ok(u64::from_le_bytes(value.to_le_bytes())),
        _ => Err("crash SQL-verified unsigned column image differs"),
    };
    let bytes = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::NULL)) => Ok(None),
        Some(BinlogValue::Value(Value::Bytes(value))) => Ok(Some(value.clone())),
        _ => Err("crash nullable byte column image differs"),
    };
    Ok(PostMetaRow {
        meta_id: unsigned(0)?,
        post_id: unsigned(1)?,
        meta_key: bytes(2)?,
        meta_value: bytes(3)?,
    })
}

// Only these two serialized, full-image UPDATE commits are accepted. Journal
// writes occur after the captured stop, so this is not a general log filter.
#[allow(clippy::too_many_arguments)]
async fn scan(
    options: OptsBuilder,
    epoch: &[u8],
    file: &[u8],
    start: u64,
    stop: u64,
    maria: bool,
    initial: Vec<PostMetaRow>,
    commits: usize,
) -> Result<Vec<Image>> {
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7017)
                .with_filename(file)
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?;
    let result = tokio::time::timeout(Duration::from_secs(10), async {
        let mut staged = ProbeBuffer::new(epoch, file, start, maria);
        let mut current =
            PostMetaView::new(initial).map_err(|_| "crash initial row identity repeats")?;
        let mut pending = Vec::new();
        let mut images = Vec::new();
        let mut events = 0;
        while let Some(event) = stream.next().await {
            let event = event?;
            let event = if events == 0 && maria {
                normalize_maria_initial_rotate(event, file, start)?
            } else {
                event
            };
            events += 1;
            if events > 64 || event.data().len() > PAYLOAD_LIMIT {
                return Err("crash source event bound exceeded".into());
            }
            verify_crc(&event)?;
            let data = event.read_data()?.ok_or("crash source event undecoded")?;
            if let EventData::RowsEvent(event_rows) = &data {
                let table = stream
                    .get_tme(event_rows.table_id())
                    .ok_or("crash table map missing")?;
                if table.database_name_raw() != b"recastpress_fixture"
                    || table.table_name_raw() != b"rp_postmeta_crash_source"
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
                    return Err("crash source table map differs from verified SQL schema".into());
                }
                for row in event_rows.rows(table) {
                    let (before, after) = row?;
                    let before = decode(&before.ok_or("crash source is not an UPDATE")?)?;
                    let after = decode(&after.ok_or("crash source is not an UPDATE")?)?;
                    if before.meta_id != 3
                        || after.meta_id != 3
                        || before.post_id != after.post_id
                        || before.meta_key != after.meta_key
                        || before.meta_value == after.meta_value
                        || !pending.is_empty()
                    {
                        return Err("crash source UPDATE shape differs".into());
                    }
                    pending.push(MetaChange::Update { before, after });
                }
            }
            let is_commit = matches!(data, EventData::XidEvent(_));
            let candidate = if is_commit {
                if pending.len() != 1 {
                    return Err("crash source COMMIT must contain one UPDATE".into());
                }
                Some(
                    current
                        .prepare_changes(std::mem::take(&mut pending))
                        .map_err(|_| "crash source before image differs")?,
                )
            } else {
                None
            };
            staged.accept_projected(&event, &data, |batch| {
                let candidate = candidate
                    .as_ref()
                    .ok_or("crash projection callback precedes COMMIT")?;
                let receipt =
                    serde_json::to_vec(&batch.rows().map(<[u8]>::to_vec).collect::<Vec<_>>())?;
                validate_receipt(&receipt)?;
                encode_rows(candidate.rows())?;
                images.push(Image {
                    from: batch.from().offset(),
                    through: batch.through().offset(),
                    receipt,
                    rows: candidate.rows().to_vec(),
                });
                Ok(())
            })?;
            if let Some(candidate) = candidate {
                current = candidate;
            }
            if images.len() > commits || staged.checkpoint_offset() > stop {
                return Err("crash source exceeded finite endpoint".into());
            }
            if staged.checkpoint_offset() == stop {
                if !is_commit {
                    return Err("crash endpoint is not COMMIT".into());
                }
                break;
            }
        }
        staged.finish()?;
        if images.len() != commits
            || staged.applied != commits
            || staged.checkpoint_offset() != stop
            || !pending.is_empty()
        {
            return Err("crash source finite window incomplete".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(images)
    })
    .await;
    stream.close().await?;
    result?
}

async fn stored(connection: &mut Conn) -> Result<Stored> {
    connection.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_crash_projection WHERE id=1").await?.ok_or_else(|| "crash projection singleton missing".into())
}

async fn check_index(connection: &mut Conn, rows: Vec<PostMetaRow>) -> Result<()> {
    let mut view = PostMetaView::new(rows).map_err(|_| "crash projection identity repeats")?;
    if view.build_key_index(11) != SearchPlan::ExactKeyIndex {
        return Err("crash index not built".into());
    }
    let mut keys: BTreeSet<Option<Vec<u8>>> =
        view.rows().iter().map(|row| row.meta_key.clone()).collect();
    keys.insert(None);
    keys.insert(Some(b"crash-absent-key".to_vec()));
    for key in keys {
        let expected: Vec<(u64,)> = if let Some(key) = &key {
            connection.exec("SELECT meta_id FROM rp_postmeta_crash_source WHERE CAST(meta_key AS BINARY)=CAST(? AS BINARY) ORDER BY meta_id",(key,)).await?
        } else {
            connection.query("SELECT meta_id FROM rp_postmeta_crash_source WHERE meta_key IS NULL ORDER BY meta_id").await?
        };
        if view
            .matching_key(key.as_deref())
            .map(|row| (row.meta_id,))
            .collect::<Vec<_>>()
            != expected
        {
            return Err("crash index differs from independent SQL".into());
        }
    }
    Ok(())
}

/// Emits one READY line and never returns successfully. The named connection ID
/// is the SQL session holding the uncommitted projection transaction, not a PID.
pub(crate) async fn prepare(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    status: &'static str,
    maria: bool,
) -> Result<()> {
    let nonce = guard()?;
    if epoch.is_empty() || epoch.len() > 64 {
        return Err("crash fixture epoch bound differs".into());
    }
    control
        .query_drop("CREATE TABLE rp_postmeta_crash_source LIKE wp_postmeta")
        .await?;
    clone_rows_preserving_zero(control).await?;
    control.query_drop("CREATE TABLE rp_postmeta_crash_projection (id TINYINT UNSIGNED PRIMARY KEY,epoch VARBINARY(64) NOT NULL,log_file VARBINARY(255) NOT NULL,from_position BIGINT UNSIGNED NOT NULL,through_position BIGINT UNSIGNED NOT NULL,receipt LONGBLOB NOT NULL,row_data LONGBLOB NOT NULL) ENGINE=InnoDB").await?;
    schema(control).await?;
    let initial = rows(control).await?;
    if initial.iter().filter(|row| row.meta_id == 3).count() != 1
        || initial.iter().any(|row| {
            row.meta_id == 3 && [Some(FIRST), Some(SECOND)].contains(&row.meta_value.as_deref())
        })
    {
        return Err("crash source target row differs".into());
    }
    let (file, start) = coordinate(control, status).await?;
    let mut expected = Vec::new();
    let mut ends = Vec::new();
    for value in [FIRST, SECOND] {
        control.query_drop("START TRANSACTION").await?;
        control
            .exec_drop(
                "UPDATE rp_postmeta_crash_source SET meta_value=? WHERE meta_id=3",
                (value,),
            )
            .await?;
        if control.affected_rows() != 1 {
            return Err("crash source UPDATE count differs".into());
        }
        control.query_drop("COMMIT").await?;
        let (after_file, position) = coordinate(control, status).await?;
        if after_file != file || position <= *ends.last().unwrap_or(&start) {
            return Err("crash source did not advance in one file".into());
        }
        ends.push(position);
        expected.push(rows(control).await?);
    }
    let images = scan(
        options.clone(),
        epoch,
        file.as_bytes(),
        start,
        ends[1],
        maria,
        initial,
        2,
    )
    .await?;
    if images[0].from != start
        || images[0].through != ends[0]
        || images[1].from != ends[0]
        || images[1].through != ends[1]
        || images
            .iter()
            .zip(&expected)
            .any(|(image, rows)| image.rows != *rows)
    {
        return Err("crash decoded source pairs differ from SQL".into());
    }
    let old: Stored = (
        epoch.to_vec(),
        file.as_bytes().to_vec(),
        start,
        ends[0],
        images[0].receipt.clone(),
        encode_rows(&images[0].rows)?,
    );
    let mut journal = Conn::new(options).await?;
    require_fixture_tls(&mut journal).await?;
    let connection_id: u64 = journal
        .query_first("SELECT CONNECTION_ID()")
        .await?
        .ok_or("crash journal connection ID missing")?;
    let mut baseline = journal.start_transaction(TxOpts::default()).await?;
    baseline
        .exec_drop(
            "INSERT INTO rp_postmeta_crash_projection VALUES (1,?,?,?,?,?,?)",
            (&old.0, &old.1, old.2, old.3, &old.4, &old.5),
        )
        .await?;
    baseline.commit().await?;
    if stored(&mut journal).await? != old {
        return Err("crash durable baseline tuple differs".into());
    }
    let mut tx = journal.start_transaction(TxOpts::default()).await?;
    let new_data = encode_rows(&images[1].rows)?;
    tx.exec_drop("UPDATE rp_postmeta_crash_projection SET from_position=?,through_position=?,receipt=?,row_data=? WHERE id=1",(ends[0],ends[1],&images[1].receipt,&new_data)).await?;
    if tx.affected_rows() != 1 {
        return Err("crash staged UPDATE count differs".into());
    }
    let staged:Stored=tx.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_crash_projection WHERE id=1").await?.ok_or("crash staged tuple missing")?;
    if staged
        != (
            epoch.to_vec(),
            file.as_bytes().to_vec(),
            ends[0],
            ends[1],
            images[1].receipt.clone(),
            new_data,
        )
    {
        return Err("crash own transaction did not see the complete staged pair".into());
    }
    let marker = json!({"format_version":1,"nonce":nonce,"process_pid":std::process::id(),"source_connection_id":connection_id,"source_file":file,"start_position":start,"baseline_position":ends[0],"target_position":ends[1],"rows":11,"epoch_hex":hex(epoch),"old_tuple":tuple_json(&old)});
    let wire = format!("{READY}{}", serde_json::to_string(&marker)?);
    if wire.len() + 1 > MARKER_LIMIT {
        return Err("crash READY marker bound exceeded".into());
    }
    {
        let mut stdout = std::io::stdout().lock();
        writeln!(stdout, "{wire}")?;
        stdout.flush()?;
    }
    // Keep the transaction alive across the await. There is deliberately no
    // COMMIT or normal-success branch after READY: only the owning parent kills.
    std::future::pending::<()>().await;
    let _held_transaction = tx;
    Err("crash failpoint unexpectedly returned".into())
}

pub(crate) async fn resume(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    _status: &'static str,
    maria: bool,
    marker: &str,
) -> Result<Json> {
    let nonce = guard()?;
    let marker = parse_marker(marker, epoch, &nonce)?;
    schema(control).await?;
    let count: u64 = control
        .query_first("SELECT COUNT(*) FROM rp_postmeta_crash_projection")
        .await?
        .ok_or("crash journal count missing")?;
    if count != 1 {
        return Err("crash journal is not a singleton".into());
    }
    let mut found = false;
    for mut row in control
        .query::<mysql_async::Row, _>("SHOW BINARY LOGS")
        .await?
    {
        let file: String = row.take("Log_name").ok_or("crash log index file missing")?;
        let size: u64 = row
            .take("File_size")
            .ok_or("crash log index size missing")?;
        if file == marker.file {
            if found || size < marker.target {
                return Err("crash target outside source log".into());
            }
            found = true;
        }
    }
    if !found {
        return Err("crash source log absent".into());
    }
    let mut journal = Conn::new(options.clone()).await?;
    require_fixture_tls(&mut journal).await?;
    let result: Result<Json> = async {
        journal.query_drop("SET SESSION innodb_lock_wait_timeout=2").await?;
        // A plain consistent read can see the baseline while a dead client's
        // transaction still owns the row. Acquiring its row lock drains rollback.
        tokio::time::timeout(Duration::from_secs(5),async {
            let mut tx=journal.start_transaction(TxOpts::default()).await?;
            let old:Stored=tx.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_crash_projection WHERE id=1 FOR UPDATE").await?.ok_or("crash baseline tuple missing")?;
            let matches=old==marker.old;
            tx.rollback().await?;
            if !matches {return Err("crash killed transaction changed the committed old tuple".into());}
            Ok::<_,Box<dyn std::error::Error>>(())
        }).await??;
        let initial=decode_rows(&marker.old.5)?;
        if initial.iter().find(|row|row.meta_id==3).and_then(|row|row.meta_value.as_deref())!=Some(FIRST) {return Err("crash baseline row value differs".into());}
        let images=scan(options,epoch,marker.file.as_bytes(),marker.baseline,marker.target,maria,initial,1).await?;
        let image=&images[0];
        if image.from!=marker.baseline || image.through!=marker.target || image.rows.iter().find(|row|row.meta_id==3).and_then(|row|row.meta_value.as_deref())!=Some(SECOND) {return Err("crash resumed pair differs".into());}
        let expected:Stored=(epoch.to_vec(),marker.file.as_bytes().to_vec(),marker.baseline,marker.target,image.receipt.clone(),encode_rows(&image.rows)?);
        let mut tx=journal.start_transaction(TxOpts::default()).await?;
        let before:Stored=tx.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_crash_projection WHERE id=1 FOR UPDATE").await?.ok_or("crash replay baseline missing")?;
        if before!=marker.old {return Err("crash baseline changed before replay commit".into());}
        tx.exec_drop("UPDATE rp_postmeta_crash_projection SET from_position=?,through_position=?,receipt=?,row_data=? WHERE id=1",(expected.2,expected.3,&expected.4,&expected.5)).await?;
        if tx.affected_rows()!=1 {return Err("crash replay UPDATE count differs".into());}
        tx.commit().await?;
        if stored(&mut journal).await?!=expected || rows(control).await?!=image.rows {return Err("crash replay rows/checkpoint differ from source".into());}
        check_index(control,image.rows.clone()).await?;
        Ok(json!({"format_version":1,"profile":"fixture_postmeta_precommit_process_kill","nonce":marker.nonce,"original_pid":marker.pid,"process_pid":std::process::id(),"source_connection_id":marker.connection_id,"source_file":marker.file,"start_position":marker.start,"baseline_position":marker.baseline,"target_position":marker.target,"rows":11,"old_pair_preserved":true,"rollback_drained":true,"source_resumed":true,"row_data_matches":true,"index_matches":true,"receipt_validated":true,"resumed_commits":1,"resumed_updates":1}))
    }.await;
    let close = close_fixture_connection(journal, "crash_resume_journal").await;
    let result = result?;
    close?;
    Ok(result)
}

fn valid_restart_uuid(uuid: &str) -> bool {
    uuid.len() == 36
        && uuid.bytes().enumerate().all(|(index, byte)| {
            if [8, 13, 18, 23].contains(&index) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

type RestartProfile = (
    String,
    u8,
    String,
    String,
    String,
    String,
    u8,
    u64,
    u64,
    u64,
);
fn restart_profile_matches(profile: &RestartProfile, expected: &str) -> bool {
    valid_restart_uuid(expected)
        && profile.0 == expected
        && profile.1 == 1
        && profile.2 == "OFF"
        && profile.3 == "ROW"
        && profile.4 == "FULL"
        && profile.5 == "CRC32"
        && profile.6 == 0
        && profile.7 == 1
        && profile.8 == 1
        && profile.9 == 0
}

fn restart_expected_source(baseline: &[PostMetaRow]) -> Result<Vec<PostMetaRow>> {
    if baseline.len() != 11
        || baseline.first().map(|row| row.meta_id) != Some(0)
        || baseline.last().map(|row| row.meta_id) != Some(u64::MAX)
        || baseline
            .windows(2)
            .any(|pair| pair[0].meta_id >= pair[1].meta_id)
        || baseline.iter().filter(|row| row.meta_id == 3).count() != 1
    {
        return Err("restart baseline row identities differ".into());
    }
    let mut expected = baseline.to_vec();
    let row = expected
        .iter_mut()
        .find(|row| row.meta_id == 3)
        .ok_or("restart baseline target missing")?;
    if row.meta_value.as_deref() != Some(FIRST) {
        return Err("restart baseline value differs".into());
    }
    // T2 committed in the source BEFORE the journal's pending transaction.
    // Restoring C1 is intentionally not equality to current full source rows.
    row.meta_value = Some(SECOND.to_vec());
    Ok(expected)
}

/// Verify recovery only: no projection DML/COMMIT, DDL, or binlog resumption.
/// The host must have killed/reaped its owned DB, then restarted the same exact
/// disposable datadir with no initialization/init-file/schema replay. That OS
/// lifecycle is observed by the parent, not inferred from this SQL connection.
pub(crate) async fn verify_restart(
    control: &mut Conn,
    epoch: &[u8],
    version: &str,
    wire: &str,
) -> Result<Json> {
    let nonce = guard()?;
    if version != "8.4.13" {
        return Err("server restart verifier requires pinned MySQL 8.4.13".into());
    }
    let expected_uuid = std::env::var("RECASTPRESS_SERVER_RESTART_UUID")?;
    let profile:RestartProfile=control.query_first("SELECT @@server_uuid,@@GLOBAL.log_bin,@@GLOBAL.gtid_mode,@@GLOBAL.binlog_format,@@GLOBAL.binlog_row_image,@@GLOBAL.binlog_checksum,@@GLOBAL.binlog_transaction_compression,@@GLOBAL.sync_binlog,@@GLOBAL.innodb_flush_log_at_trx_commit,@@GLOBAL.innodb_force_recovery").await?.ok_or("restart profile missing")?;
    if !restart_profile_matches(&profile, &expected_uuid) {
        return Err("server restart UUID or durable fixture profile differs".into());
    }
    let marker = parse_marker(wire, epoch, &nonce)?;
    schema(control).await?;
    let engine:Option<String>=control.query_first("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_postmeta_crash_projection'").await?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("restart projection is not InnoDB".into());
    }
    let count: u64 = control
        .query_first("SELECT COUNT(*) FROM rp_postmeta_crash_projection")
        .await?
        .ok_or("restart singleton count missing")?;
    if count != 1 {
        return Err("restart projection is not a singleton".into());
    }
    let mut found = false;
    for mut row in control
        .query::<mysql_async::Row, _>("SHOW BINARY LOGS")
        .await?
    {
        let file: Vec<u8> = row
            .take("Log_name")
            .ok_or("restart retained file name missing")?;
        let size: u64 = row
            .take("File_size")
            .ok_or("restart retained file size missing")?;
        if file == marker.file.as_bytes() {
            if found || size < marker.target {
                return Err("restart old source coordinates are not retained".into());
            }
            found = true;
        }
    }
    if !found {
        return Err("restart old source file missing".into());
    }
    control
        .query_drop("SET SESSION innodb_lock_wait_timeout=2")
        .await?;
    let recovered=tokio::time::timeout(Duration::from_secs(5),async {
        // InnoDB may accept connections before background undo completes.
        // Lock acquisition drains the pending transaction's row lock; a plain
        // consistent read alone would not establish that recovery has drained.
        let mut tx=control.start_transaction(TxOpts::default()).await?;
        let saved:Stored=tx.query_first("SELECT epoch,log_file,from_position,through_position,receipt,row_data FROM rp_postmeta_crash_projection WHERE id=1 FOR UPDATE").await?.ok_or("restart committed baseline missing")?;
        tx.rollback().await?;
        Ok::<_,Box<dyn std::error::Error>>(saved)
    }).await??;
    if recovered != marker.old {
        return Err("restart recovered tuple differs from the committed C1 pair".into());
    }
    validate_receipt(&recovered.4)?;
    let baseline = decode_rows(&recovered.5)?;
    let expected_source = restart_expected_source(&baseline)?;
    if rows(control).await? != expected_source {
        return Err("restart committed source T2 differs from the C1-derived expectation".into());
    }
    // Only values changed between T1/T2. Binary key/NULL lookup results remain
    // independently comparable to SQL while the historical C1 body stays old.
    check_index(control, baseline).await?;
    if stored(control).await? != recovered {
        return Err("restart verification changed its committed tuple".into());
    }
    Ok(
        json!({"format_version":1,"profile":"mysql_8_4_13_unpublished_server_precommit_restart_restore",
        "process_pid":std::process::id(),"nonce":marker.nonce,"prepared_pid":marker.pid,
        "source_uuid":expected_uuid,"source_file":marker.file,"baseline_position":marker.baseline,
        "candidate_position":marker.target,"rows":11,"old_tuple_matches":true,
        "uncommitted_candidate_absent":true,"rollback_drained":true,"baseline_row_data_matches":true,
        "index_matches":true,"source_commits_preserved":true,"store_unchanged":true,
        "receipt_validated":true,"no_projection_writes":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn restart_profile_is_exact_and_uuid_is_canonical_lowercase() {
        let uuid = "12345678-1234-1234-1234-123456789abc";
        let valid: RestartProfile = (
            uuid.into(),
            1,
            "OFF".into(),
            "ROW".into(),
            "FULL".into(),
            "CRC32".into(),
            0,
            1,
            1,
            0,
        );
        assert!(restart_profile_matches(&valid, uuid));
        for bad in [
            "",
            "12345678123412341234123456789abc",
            "12345678-1234-1234-1234-123456789abC",
            "12345678/1234-1234-1234-123456789abc",
        ] {
            assert!(!valid_restart_uuid(bad));
        }
        let mut changed = valid.clone();
        changed.0 = "00000000-0000-0000-0000-000000000000".into();
        assert!(!restart_profile_matches(&changed, uuid));
        let mut changed = valid.clone();
        changed.2 = "ON".into();
        assert!(!restart_profile_matches(&changed, uuid));
        for index in 0..5 {
            let mut changed = valid.clone();
            match index {
                0 => changed.1 = 0,
                1 => changed.6 = 1,
                2 => changed.7 = 0,
                3 => changed.8 = 2,
                _ => changed.9 = 1,
            };
            assert!(!restart_profile_matches(&changed, uuid));
        }
    }

    #[test]
    fn restart_source_expectation_changes_only_the_already_committed_target_value() {
        let baseline: Vec<_> = (0..10)
            .chain([u64::MAX])
            .map(|meta_id| PostMetaRow {
                meta_id,
                post_id: u64::MAX,
                meta_key: if meta_id == 0 {
                    None
                } else {
                    Some(vec![0, 255])
                },
                meta_value: if meta_id == 3 {
                    Some(FIRST.to_vec())
                } else {
                    Some(Vec::new())
                },
            })
            .collect();
        let before = baseline.clone();
        let expected = restart_expected_source(&baseline).unwrap();
        assert_eq!(baseline, before);
        for (old, new) in baseline.iter().zip(&expected) {
            if old.meta_id == 3 {
                let mut target = old.clone();
                target.meta_value = Some(SECOND.to_vec());
                assert_eq!(&target, new);
            } else {
                assert_eq!(old, new);
            }
        }
        for index in [0, 3, 10] {
            let mut invalid = baseline.clone();
            invalid.remove(index);
            assert!(restart_expected_source(&invalid).is_err());
        }
        let mut invalid = baseline.clone();
        invalid[3].meta_value = Some(SECOND.to_vec());
        assert!(restart_expected_source(&invalid).is_err());
        let mut invalid = baseline.clone();
        invalid[1].meta_id = 0;
        assert!(restart_expected_source(&invalid).is_err());
    }

    #[test]
    fn clone_mode_preserves_existing_modes_and_adds_zero_id_protection_once() {
        assert_eq!(clone_sql_mode(""), Some("NO_AUTO_VALUE_ON_ZERO".into()));
        assert_eq!(
            clone_sql_mode("STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION"),
            Some("STRICT_TRANS_TABLES,NO_ENGINE_SUBSTITUTION,NO_AUTO_VALUE_ON_ZERO".into())
        );
        for mode in [
            "NO_AUTO_VALUE_ON_ZERO",
            "STRICT_TRANS_TABLES,NO_AUTO_VALUE_ON_ZERO,NO_ENGINE_SUBSTITUTION",
            "no_auto_value_on_zero",
        ] {
            assert_eq!(clone_sql_mode(mode), None);
        }
    }

    fn marker() -> (Json, Vec<u8>, String) {
        let epoch = b"fixture-epoch".to_vec();
        let nonce = "a".repeat(32);
        let rows: Vec<_> = (0..11)
            .map(|meta_id| PostMetaRow {
                meta_id,
                post_id: u64::MAX,
                meta_key: if meta_id == 0 {
                    None
                } else {
                    Some(b"k\0".to_vec())
                },
                meta_value: if meta_id == 3 {
                    Some(FIRST.to_vec())
                } else {
                    Some(b"\xff".to_vec())
                },
            })
            .collect();
        let old: Stored = (
            epoch.clone(),
            b"probe-bin.000001".to_vec(),
            10,
            20,
            serde_json::to_vec(&vec![vec![1u8]]).unwrap(),
            encode_rows(&rows).unwrap(),
        );
        let value = json!({"format_version":1,"nonce":nonce,"process_pid":u64::from(std::process::id())+1,
            "source_connection_id":7,"source_file":"probe-bin.000001","start_position":10,
            "baseline_position":20,"target_position":30,"rows":11,"epoch_hex":hex(&epoch),"old_tuple":tuple_json(&old)});
        (value, epoch, nonce)
    }

    fn wire(value: &Json) -> String {
        format!("{READY}{}", serde_json::to_string(value).unwrap())
    }

    #[test]
    fn ready_roundtrip_preserves_all_old_bytes_and_rejects_duplicate_or_unknown_keys() {
        let (value, epoch, nonce) = marker();
        let encoded = wire(&value);
        let decoded = parse_marker(&encoded, &epoch, &nonce).unwrap();
        assert_eq!(tuple_json(&decoded.old), value["old_tuple"]);
        let duplicate = format!("{},\"rows\":11}}", &encoded[..encoded.len() - 1]);
        assert!(parse_marker(&duplicate, &epoch, &nonce).is_err());
        let nested = encoded.replace(
            "\"old_tuple\":{",
            "\"old_tuple\":{\"epoch_hex\":\"666978747572652d65706f6368\",",
        );
        assert!(parse_marker(&nested, &epoch, &nonce).is_err());
        for nested in [false, true] {
            let mut invalid = value.clone();
            let object = if nested {
                invalid["old_tuple"].as_object_mut().unwrap()
            } else {
                invalid.as_object_mut().unwrap()
            };
            object.insert("unknown".into(), json!(true));
            assert!(parse_marker(&wire(&invalid), &epoch, &nonce).is_err());
        }
        assert!(parse_marker(&(encoded.clone() + "\n"), &epoch, &nonce).is_err());
        assert!(
            parse_marker(
                &format!("{READY}{}", " ".repeat(MARKER_LIMIT)),
                &epoch,
                &nonce
            )
            .is_err()
        );
    }

    #[test]
    fn ready_identity_types_boundaries_and_complete_tuple_are_required() {
        let (value, epoch, nonce) = marker();
        for (field, replacement) in [
            ("process_pid", json!(std::process::id())),
            ("source_connection_id", json!(0)),
            ("start_position", json!(4)),
            ("baseline_position", json!(30)),
            ("target_position", json!(20)),
            ("rows", json!(true)),
            ("start_position", json!(10.0)),
            ("source_file", json!("../probe-bin.000001")),
            ("nonce", json!("b".repeat(32))),
            ("epoch_hex", json!("00")),
        ] {
            let mut invalid = value.clone();
            invalid[field] = replacement;
            assert!(
                parse_marker(&wire(&invalid), &epoch, &nonce).is_err(),
                "{field}"
            );
        }
        for field in [
            "epoch_hex",
            "file_hex",
            "from_position",
            "through_position",
            "receipt_hex",
            "row_data_hex",
        ] {
            let mut invalid = value.clone();
            invalid["old_tuple"].as_object_mut().unwrap().remove(field);
            assert!(parse_marker(&wire(&invalid), &epoch, &nonce).is_err());
        }
        let mut invalid = value.clone();
        invalid["old_tuple"]["through_position"] = json!(21);
        assert!(parse_marker(&wire(&invalid), &epoch, &nonce).is_err());
    }

    #[test]
    fn lower_hex_receipt_and_eleven_row_payload_bounds_fail_closed() {
        assert_eq!(unhex(&json!("00ff"), 2).unwrap(), [0, 255]);
        for value in [json!("0"), json!("FF"), json!("00ff00"), json!(true)] {
            assert!(unhex(&value, 2).is_err());
        }
        for receipt in [b"[]".as_slice(), b"[[]]", b"[[1],[2]]", b"null"] {
            assert!(validate_receipt(receipt).is_err());
        }
        assert!(validate_receipt(&vec![0; PAYLOAD_LIMIT + 1]).is_err());
        assert!(decode_rows(b"[]").is_err());
        assert!(decode_rows(&vec![0; PAYLOAD_LIMIT + 1]).is_err());
        let (mut value, epoch, nonce) = marker();
        value["old_tuple"]["receipt_hex"] = json!(hex(b"[[]]"));
        assert!(parse_marker(&wire(&value), &epoch, &nonce).is_err());
    }
}
