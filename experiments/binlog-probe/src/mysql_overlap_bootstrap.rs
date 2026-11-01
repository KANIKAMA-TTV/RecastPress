//! Unpublished MySQL-only overlap bootstrap in a finite fresh GTID fixture.
//! This is an algorithm experiment, not an official snapshot protocol, durable
//! journal, production source identity or publishing/cache interface. Historical
//! A-owned groups before the replay floor are skipped in a distinct decoder
//! state, never relabelled as harmless metadata in the ordinary strict adapter.

use crate::{
    buffer_adapter::ProbeBuffer, close_fixture_connection, require_fixture_tls, verify_crc,
};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStreamRequest, Conn, OptsBuilder, Value,
    binlog::{
        events::{BinlogEventFooter, BinlogEventHeader, EventData, TableMapEvent},
        row::BinlogRow,
        value::BinlogValue,
    },
    consts::ColumnType,
    prelude::Queryable,
};
use recastpress_db::{
    PostMetaRow,
    meta_view::{MetaChange, PostMetaView, SearchPlan},
};
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type EncodedRows = Vec<(u64, u64, Option<Vec<u8>>, Option<Vec<u8>>)>;
const FILE: &str = "probe-bin.000001";
const TABLE: &[u8] = b"rp_gtid_bootstrap_source";
const LIMIT: usize = 64 * 1024;
const ONE: &[u8] = b"overlap-one";
const TWO: &[u8] = b"overlap-two";
const THREE: &[u8] = b"overlap-three";
const FOUR: &[u8] = b"overlap-four";

fn envelope_size(data_len: usize, is_fde: bool) -> Result<u64> {
    // Event::read removes the FDE's algorithm descriptor in addition to the
    // CRC32 suffix. Other decoded events have only the checksum removed.
    let suffix = BinlogEventFooter::BINLOG_CHECKSUM_LEN
        + if is_fde {
            BinlogEventFooter::BINLOG_CHECKSUM_ALG_DESC_LEN
        } else {
            0
        };
    u64::try_from(data_len)?
        .checked_add(BinlogEventHeader::LEN as u64)
        .and_then(|size| size.checked_add(suffix as u64))
        .ok_or_else(|| "overlap event envelope size overflow".into())
}

fn file_valid(file: &str) -> bool {
    file == FILE
}
fn check_rows(rows: &[PostMetaRow]) -> Result<()> {
    if !(11..=12).contains(&rows.len()) {
        return Err("overlap source row bound differs".into());
    }
    let encoded: EncodedRows = rows
        .iter()
        .map(|r| {
            (
                r.meta_id,
                r.post_id,
                r.meta_key.clone(),
                r.meta_value.clone(),
            )
        })
        .collect();
    if serde_json::to_vec(&encoded)?.len() > LIMIT {
        return Err("overlap row byte bound exceeded".into());
    }
    Ok(())
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
        .ok_or("overlap clone session SQL mode missing")?;
    let replacement = clone_sql_mode(&original);
    let insert: std::result::Result<(), mysql_async::Error> = async {
        if let Some(replacement) = &replacement {
            control
                .exec_drop("SET SESSION sql_mode = ?", (replacement,))
                .await?;
        }
        control
            .query_drop("INSERT INTO rp_gtid_bootstrap_source SELECT * FROM wp_postmeta")
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
            "overlap source clone failed: {insert}; restoring session SQL mode also failed: {restore}"
        )
        .into()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error.into()),
    }
}

async fn rows(connection: &mut Conn) -> Result<Vec<PostMetaRow>> {
    let rows: EncodedRows = connection.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_gtid_bootstrap_source ORDER BY meta_id LIMIT 13").await?;
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect();
    check_rows(&rows)?;
    Ok(rows)
}

async fn schema(connection: &mut Conn) -> Result<()> {
    let columns: Vec<(String,String,String,String,Option<String>)> = connection.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_gtid_bootstrap_source' ORDER BY ORDINAL_POSITION").await?;
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
        return Err("overlap source SQL schema differs".into());
    }
    let engine: Option<String> = connection.query_first("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_gtid_bootstrap_source'").await?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("overlap source is not InnoDB".into());
    }
    Ok(())
}

async fn coordinate(connection: &mut Conn, status: &'static str) -> Result<(String, u64)> {
    let mut row: mysql_async::Row = connection
        .query_first(status)
        .await?
        .ok_or("overlap source coordinate missing")?;
    let file: String = row.take("File").ok_or("overlap source file missing")?;
    let position: u64 = row
        .take("Position")
        .ok_or("overlap source position missing")?;
    if !file_valid(&file) || position <= 4 {
        return Err("overlap source coordinate invalid".into());
    }
    Ok((file, position))
}

fn decode(row: &BinlogRow) -> Result<PostMetaRow> {
    if row.len() != 4 {
        return Err("overlap row image width differs".into());
    }
    let unsigned = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::UInt(value))) => Ok(*value),
        Some(BinlogValue::Value(Value::Int(value))) => Ok(u64::from_le_bytes(value.to_le_bytes())),
        _ => Err("overlap SQL-verified unsigned column image differs"),
    };
    let bytes = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::NULL)) => Ok(None),
        Some(BinlogValue::Value(Value::Bytes(value))) => Ok(Some(value.clone())),
        _ => Err("overlap nullable byte column image differs"),
    };
    Ok(PostMetaRow {
        meta_id: unsigned(0)?,
        post_id: unsigned(1)?,
        meta_key: bytes(2)?,
        meta_value: bytes(3)?,
    })
}

#[derive(Clone)]
struct GtidSet {
    sid: [u8; 16],
    intervals: Vec<(u64, u64)>,
}
fn uuid(text: &str) -> Result<[u8; 16]> {
    if text.len() != 36 || ![8, 13, 18, 23].iter().all(|&i| text.as_bytes()[i] == b'-') {
        return Err("overlap source UUID shape differs".into());
    }
    let digits: Vec<_> = text
        .bytes()
        .enumerate()
        .filter_map(|(i, b)| {
            if [8, 13, 18, 23].contains(&i) {
                None
            } else {
                Some(b)
            }
        })
        .collect();
    let digit = |b| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        _ => Err("overlap UUID requires lowercase hex"),
    };
    let mut sid = [0; 16];
    for (i, pair) in digits.chunks_exact(2).enumerate() {
        sid[i] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Ok(sid)
}
fn positive(text: &str) -> Result<u64> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) || text.starts_with('0') {
        return Err("overlap GTID interval is not canonical positive decimal".into());
    }
    let value = text.parse::<u64>()?;
    if value > i64::MAX as u64 {
        return Err("overlap GTID number exceeds signed GTID range".into());
    }
    Ok(value)
}
impl GtidSet {
    fn parse(text: &str, expected_sid: [u8; 16]) -> Result<Self> {
        if text.len() > 4096 {
            return Err("overlap A GTID text bound exceeded".into());
        }
        let mut words = text.split(':');
        let sid = uuid(words.next().ok_or("overlap A UUID missing")?)?;
        if sid != expected_sid {
            return Err("overlap A contains a foreign source".into());
        }
        let mut intervals = Vec::new();
        let mut previous = 0u64;
        for word in words {
            let (from, through) = if let Some((left, right)) = word.split_once('-') {
                (positive(left)?, positive(right)?)
            } else {
                let value = positive(word)?;
                (value, value)
            };
            if from > through || from <= previous || intervals.len() >= 128 {
                return Err("overlap A intervals overlap or exceed bounds".into());
            }
            previous = through;
            intervals.push((from, through));
        }
        if intervals.is_empty() {
            return Err("overlap A is empty or tagged".into());
        }
        Ok(Self { sid, intervals })
    }
    fn contains(&self, gno: u64) -> bool {
        self.intervals
            .iter()
            .any(|&(from, through)| from <= gno && gno <= through)
    }
}

#[derive(Default)]
struct ReplayFloor {
    position: Option<u64>,
}
impl ReplayFloor {
    fn skip_group(&mut self, a: &GtidSet, gno: u64, start: u64) -> bool {
        if self.position.is_none() && a.contains(gno) {
            return true;
        }
        self.position.get_or_insert(start);
        false
    }
}

fn transaction_end(
    start: u64,
    length: u64,
    event_size: u64,
    maximum: u64,
    fence: u64,
) -> Result<u64> {
    let end = start
        .checked_add(length)
        .ok_or("overlap GTID length overflow")?;
    if event_size == 0 || length < event_size || length > maximum || end > fence {
        return Err("overlap GTID group length is outside its complete fence".into());
    }
    Ok(end)
}

async fn profile(control: &mut Conn, version: &str) -> Result<[u8; 16]> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable")
        || version != "8.4.13"
        || std::env::var("RECASTPRESS_DB_EXPECT_VERSION").as_deref() != Ok("8.4.13")
    {
        return Err("overlap bootstrap requires the owned MySQL 8.4.13 fixture".into());
    }
    let (gtid,enforce,format,image,order,compressed,purged,sid):(String,String,String,String,u8,u8,String,String)=control.query_first("SELECT @@GLOBAL.gtid_mode,@@GLOBAL.enforce_gtid_consistency,@@SESSION.binlog_format,@@SESSION.binlog_row_image,CAST(@@GLOBAL.binlog_order_commits AS UNSIGNED),CAST(@@SESSION.binlog_transaction_compression AS UNSIGNED),@@GLOBAL.gtid_purged,@@GLOBAL.server_uuid").await?.ok_or("overlap source profile missing")?;
    if gtid != "ON"
        || enforce != "ON"
        || format != "ROW"
        || image != "FULL"
        || order != 1
        || compressed != 0
        || !purged.is_empty()
    {
        return Err(
            "overlap source requires GTID ON, ROW FULL, ordered commits, no compression or purge"
                .into(),
        );
    }
    let (global_format,global_image,global_compression,next,logged):(String,String,u8,String,u8)=control.query_first("SELECT @@GLOBAL.binlog_format,@@GLOBAL.binlog_row_image,CAST(@@GLOBAL.binlog_transaction_compression AS UNSIGNED),@@SESSION.gtid_next,CAST(@@SESSION.sql_log_bin AS UNSIGNED)").await?.ok_or("overlap logging profile missing")?;
    if global_format != "ROW"
        || global_image != "FULL"
        || global_compression != 0
        || next != "AUTOMATIC"
        || logged != 1
    {
        return Err("overlap source has unsupported logging overrides".into());
    }
    let files: Vec<(String, u64, Option<String>)> = control.query("SHOW BINARY LOGS").await?;
    if files.len() != 1 || files[0].0 != FILE || files[0].1 > 64 * 1024 * 1024 {
        return Err("overlap source must retain exactly one bounded first log".into());
    }
    let mut status: mysql_async::Row = control
        .query_first("SHOW BINARY LOG STATUS")
        .await?
        .ok_or("overlap log status missing")?;
    let included: String = status
        .take("Binlog_Do_DB")
        .ok_or("overlap log filter status missing")?;
    let excluded: String = status
        .take("Binlog_Ignore_DB")
        .ok_or("overlap log exclusion status missing")?;
    if !included.is_empty() || !excluded.is_empty() {
        return Err("overlap source log filters are unsupported".into());
    }
    uuid(&sid)
}

fn table_map(table: &TableMapEvent<'_>) -> Result<()> {
    if table.database_name_raw() != b"recastpress_fixture"
        || table.table_name_raw() != TABLE
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
        return Err("overlap table map differs from verified clone schema".into());
    }
    Ok(())
}

fn overlay(rows: &mut BTreeMap<u64, PostMetaRow>, change: MetaChange) {
    match change {
        MetaChange::Insert(after) => {
            rows.insert(after.meta_id, after);
        }
        MetaChange::Update { before, after } => {
            if before.meta_id != after.meta_id {
                rows.remove(&before.meta_id);
            }
            rows.insert(after.meta_id, after);
        }
        MetaChange::Delete(before) => {
            rows.remove(&before.meta_id);
        }
    }
}

fn change(before: Option<BinlogRow>, after: Option<BinlogRow>) -> Result<MetaChange> {
    match (before, after) {
        (None, Some(after)) => Ok(MetaChange::Insert(decode(&after)?)),
        (Some(before), Some(after)) => Ok(MetaChange::Update {
            before: decode(&before)?,
            after: decode(&after)?,
        }),
        (Some(before), None) => Ok(MetaChange::Delete(decode(&before)?)),
        _ => Err("overlap row event lacks full images".into()),
    }
}

fn view(rows: &BTreeMap<u64, PostMetaRow>) -> Result<PostMetaView> {
    check_rows(&rows.values().cloned().collect::<Vec<_>>())?;
    PostMetaView::new(rows.values().cloned().collect())
        .map_err(|_| "overlap row identities repeat".into())
}

struct Group {
    end: u64,
    skip: bool,
    began: bool,
    changes: Vec<MetaChange>,
}
struct Bootstrap {
    view: PostMetaView,
    floor: u64,
    skipped: usize,
    last_gtid: u64,
}

#[allow(clippy::too_many_arguments)]
async fn bootstrap(
    options: OptsBuilder,
    a: &GtidSet,
    snapshot: Vec<PostMetaRow>,
    c: u64,
    expected_ends: &[u64],
    expected: &[Vec<PostMetaRow>],
) -> Result<Bootstrap> {
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7031)
                .with_filename(FILE.as_bytes())
                .with_pos(4)
                .with_non_blocking(),
        )
        .await?;
    let result=tokio::time::timeout(Duration::from_secs(20),async {
        let mut cursor=4u64;
        let mut rotated=false;
        let mut described=false;
        let mut previous=false;
        let mut group:Option<Group>=None;
        let mut last_gtid=0;
        let mut floor=ReplayFloor::default();
        let mut skipped=0;
        let mut commits=0;
        let mut events=0;
        let mut map:BTreeMap<_,_>=snapshot.into_iter().map(|row|(row.meta_id,row)).collect();
        while let Some(event)=stream.next().await {
            let event=event?;
            events+=1;
            if events>8192 || event.data().len()>2*1024*1024 {return Err("overlap historical stream bound exceeded".into());}
            let data=event.read_data()?.ok_or("overlap stream event undecoded")?;
            if let EventData::RotateEvent(rotate)=&data {
                if rotated || described || cursor!=4 || rotate.position()!=4 || rotate.name_raw()!=FILE.as_bytes() || event.header().log_pos()!=0 {return Err("overlap rotation or initial Rotate differs".into());}
                rotated=true;
                continue;
            }
            if !rotated {return Err("overlap source precedes initial Rotate".into());}
            if !verify_crc(&event)? {return Err("overlap real frame lacks verified CRC32".into());}
            let size=u64::from(event.header().event_size());
            let end=u64::from(event.header().log_pos());
            let envelope_expected=envelope_size(event.data().len(),matches!(data,EventData::FormatDescriptionEvent(_)))?;
            if size!=envelope_expected || end.checked_sub(size)!=Some(cursor) || end>c {
                return Err(format!("overlap physical frames are not contiguous up to C (type={}, end={}, size={}, data_len={}, cursor={}, c={}, flags={}, checksum_present={}, checksum_algorithm={:?}, envelope_expected={}, implied_start={:?}, described={}, previous_gtids={})",
                    event.header().event_type_raw(),end,size,event.data().len(),cursor,c,event.header().flags_raw(),event.checksum().is_some(),event.footer().get_checksum_alg(),envelope_expected,end.checked_sub(size),described,previous).into());
            }
            let start=cursor;cursor=end;
            if let EventData::FormatDescriptionEvent(fde)=&data {
                if described || previous || start!=4 || fde.binlog_version()!=mysql_async::binlog::BinlogVersion::Version4 || fde.event_header_length()!=19 {return Err("overlap FDE profile differs".into());}
                described=true;continue;
            }
            if !described {return Err("overlap event precedes FDE".into());}
            if let EventData::PreviousGtidsEvent(gtids)=&data {
                if previous || group.is_some() || last_gtid!=0 || !gtids.sids().is_empty() {return Err("overlap requires one empty Previous-GTIDs header".into());}
                previous=true;continue;
            }
            if !previous {return Err("overlap GTID history header missing".into());}
            if let EventData::GtidEvent(gtid)=&data {
                if group.is_some() || gtid.sid()!=a.sid || gtid.is_tagged() || gtid.gno()!=last_gtid+1 || gtid.gno()>i64::MAX as u64 {return Err("overlap GTID origin/order/normal format differs".into());}
                last_gtid=gtid.gno();
                let skip=floor.skip_group(a,gtid.gno(),start);
                let group_end=transaction_end(start,gtid.tx_length(),size,8*1024*1024,c)?;
                group=Some(Group {end:group_end,skip,began:false,changes:Vec::new()});
            } else {
                let active=group.as_mut().ok_or("overlap frame lacks a GTID group")?;
                if end>active.end {return Err("overlap event exceeds its GTID group".into());}
                if active.skip {
                    // A proves this complete historical group was committed
                    // before B. No row decoding or current-schema interpretation.
                    match &data {
                        EventData::QueryEvent(query) if query.error_code()==0=>{},
                        EventData::TableMapEvent(_)|EventData::RowsEvent(_)|EventData::XidEvent(_)=>{},
                        _=>return Err("overlap unsupported event in A-owned prefix".into()),
                    }
                } else {
                    if event.data().len()>LIMIT {return Err("overlap replay event bound exceeded".into());}
                    match &data {
                        EventData::QueryEvent(query) if query.error_code()==0 && query.query_raw()==b"BEGIN" && !active.began && active.changes.is_empty()=>{active.began=true;}
                        EventData::TableMapEvent(table) if active.began=>{table_map(table)?;}
                        EventData::RowsEvent(event_rows) if active.began=>{
                            let table=stream.get_tme(event_rows.table_id()).ok_or("overlap row table map missing")?;
                            table_map(table)?;
                            for row in event_rows.rows(table) {
                                let (before,after)=row?;
                                active.changes.push(change(before,after)?);
                                if active.changes.len()>8 {return Err("overlap replay mutation bound exceeded".into());}
                            }
                        }
                        EventData::XidEvent(_) if active.began && !active.changes.is_empty() && end==active.end=>{},
                        _=>return Err("overlap replay rejects DDL, unknown events and incomplete transactions".into()),
                    }
                }
                if end==active.end {
                    let active=group.take().unwrap();
                    if active.skip {
                        if !matches!(data,EventData::QueryEvent(_)|EventData::XidEvent(_)) {return Err("overlap skipped GTID group lacks a commit boundary".into());}
                        skipped+=1;
                    } else {
                        if !matches!(data,EventData::XidEvent(_)) {return Err("overlap replay group does not end at XID".into());}
                        if expected_ends.get(commits)!=Some(&end) {return Err("overlap source COMMIT differs from planned fence".into());}
                        for change in active.changes {overlay(&mut map,change);}
                        let current=view(&map)?;
                        if expected.get(commits).is_none_or(|rows|current.rows()!=*rows) {return Err("overlap after replay differs from independent committed SQL".into());}
                        commits+=1;
                    }
                }
            }
            if cursor==c {
                if group.is_some() || commits!=3 || skipped==0 || !matches!(data,EventData::XidEvent(_)) {return Err("overlap C is not the third complete replay XID".into());}
                return Ok(Bootstrap {view:view(&map)?,floor:floor.position.ok_or("overlap replay floor missing")?,skipped,last_gtid});
            }
        }
        Err::<Bootstrap,Box<dyn std::error::Error>>("overlap stream ended before C".into())
    }).await;
    stream.close().await?;
    result?
}

async fn index_matches(control: &mut Conn, view: &mut PostMetaView) -> Result<()> {
    if view.rows() != rows(control).await? || view.build_key_index(12) != SearchPlan::ExactKeyIndex
    {
        return Err("overlap rows or index differ from independent SQL".into());
    }
    let mut keys: BTreeSet<_> = view.rows().iter().map(|row| row.meta_key.clone()).collect();
    keys.insert(None);
    keys.insert(Some(b"overlap-absent-key".to_vec()));
    for key in keys {
        let expected: Vec<(u64,)> = if let Some(key) = &key {
            control.exec("SELECT meta_id FROM rp_gtid_bootstrap_source WHERE CAST(meta_key AS BINARY)=CAST(? AS BINARY) ORDER BY meta_id",(key,)).await?
        } else {
            control.query("SELECT meta_id FROM rp_gtid_bootstrap_source WHERE meta_key IS NULL ORDER BY meta_id").await?
        };
        if view
            .matching_key(key.as_deref())
            .map(|row| (row.meta_id,))
            .collect::<Vec<_>>()
            != expected
        {
            return Err("overlap binary index result differs".into());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn strict_seam(
    options: OptsBuilder,
    epoch: &[u8],
    sid: [u8; 16],
    previous_gtid: u64,
    c: u64,
    d: u64,
    current: PostMetaView,
    expected: &[PostMetaRow],
) -> Result<PostMetaView> {
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7032)
                .with_filename(FILE.as_bytes())
                .with_pos(c)
                .with_non_blocking(),
        )
        .await?;
    let result=tokio::time::timeout(Duration::from_secs(10),async {
        let mut staged=ProbeBuffer::resumed(epoch,FILE.as_bytes(),c,false);
        let mut current=current;
        let mut pending=Vec::new();
        let mut events=0;
        let mut gtid_count=0;
        while let Some(event)=stream.next().await {
            let event=event?;events+=1;
            if events>32 || event.data().len()>LIMIT {return Err("overlap strict seam event bound exceeded".into());}
            verify_crc(&event)?;
            let data=event.read_data()?.ok_or("overlap strict seam event undecoded")?;
            if let EventData::GtidEvent(gtid)=&data {
                let size=u64::from(event.header().event_size());
                if gtid.sid()!=sid || gtid.is_tagged() || gtid.gno()!=previous_gtid+1 || gtid.gno()>i64::MAX as u64 || gtid_count!=0
                    || u64::from(event.header().log_pos()).checked_sub(size)!=Some(c)
                    || transaction_end(c,gtid.tx_length(),size,LIMIT as u64,d)?!=d {
                    return Err("overlap strict seam GTID context or transaction length differs".into());
                }
                gtid_count+=1;
            }
            if let EventData::TableMapEvent(table)=&data {table_map(table)?;}
            if let EventData::RowsEvent(event_rows)=&data {
                let table=stream.get_tme(event_rows.table_id()).ok_or("overlap strict row table map missing")?;
                table_map(table)?;
                for row in event_rows.rows(table) {
                    let (before,after)=row?;
                    let change=change(before,after)?;
                    if !matches!(&change,MetaChange::Update {before,after} if before.meta_id==100 && after.meta_id==100 && before.meta_value.as_deref()==Some(THREE) && after.meta_value.as_deref()==Some(FOUR)) || !pending.is_empty() {return Err("overlap strict seam must be the one planned full UPDATE".into());}
                    pending.push(change);
                }
            }
            let is_commit=matches!(data,EventData::XidEvent(_));
            let candidate=if is_commit {
                if pending.len()!=1 {return Err("overlap strict seam UPDATE count differs".into());}
                Some(current.prepare_changes(std::mem::take(&mut pending)).map_err(|_|"overlap strict before image mismatch")?)
            } else {None};
            staged.accept_projected(&event,&data,|batch| {
                if batch.from().offset()!=c || batch.through().offset()!=d || batch.rows().len()!=1 {return Err("overlap strict batch boundary differs".into());}
                Ok(())
            })?;
            if let Some(candidate)=candidate {current=candidate;}
            if staged.checkpoint_offset()==d {
                if !is_commit || gtid_count!=1 || staged.applied!=1 || !pending.is_empty() || current.rows()!=expected {return Err("overlap strict seam endpoint differs from SQL".into());}
                staged.finish()?;
                return Ok(current);
            }
        }
        Err::<PostMetaView,Box<dyn std::error::Error>>("overlap strict stream ended before D".into())
    }).await;
    stream.close().await?;
    result?
}

async fn snapshot_page(reader: &mut Conn, after: Option<u64>) -> Result<Vec<PostMetaRow>> {
    let rows:EncodedRows=match after {
        Some(after)=>reader.exec("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_gtid_bootstrap_source WHERE meta_id>? ORDER BY meta_id LIMIT 3",(after,)).await?,
        None=>reader.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_gtid_bootstrap_source ORDER BY meta_id LIMIT 3").await?,
    };
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

pub(crate) async fn run(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    version: &str,
) -> Result<Json> {
    let sid = profile(control, version).await?;
    if epoch.is_empty() || epoch.len() > 64 {
        return Err("overlap fixture epoch bound differs".into());
    }
    control
        .query_drop("CREATE TABLE rp_gtid_bootstrap_source LIKE wp_postmeta")
        .await?;
    clone_rows_preserving_zero(control).await?;
    schema(control).await?;
    let initial = rows(control).await?;
    if initial.len() != 11
        || initial.first().map(|row| row.meta_id) != Some(0)
        || initial.last().map(|row| row.meta_id) != Some(u64::MAX)
        || initial.iter().any(|row| [100, 101].contains(&row.meta_id))
    {
        return Err("overlap clone must preserve eleven original IDs including zero/MAX".into());
    }
    let a_text: String = control
        .query_first("SELECT @@GLOBAL.gtid_executed")
        .await?
        .ok_or("overlap A GTID set missing")?;
    let a = GtidSet::parse(&a_text, sid)?;
    let mut expected = Vec::new();
    let mut ends = Vec::new();
    control.query_drop("START TRANSACTION").await?;
    control.exec_drop("INSERT INTO rp_gtid_bootstrap_source (meta_id,post_id,meta_key,meta_value) VALUES (100,?,?,?)",(u64::MAX,b"overlap\0key".as_slice(),ONE)).await?;
    if control.affected_rows() != 1 {
        return Err("overlap T1 INSERT count differs".into());
    }
    control.query_drop("COMMIT").await?;
    expected.push(rows(control).await?);
    ends.push(coordinate(control, "SHOW BINARY LOG STATUS").await?.1);
    let mut reader = Conn::new(options.clone()).await?;
    let snapshot_result:Result<Vec<PostMetaRow>>=async {
        require_fixture_tls(&mut reader).await?;
        reader.query_drop("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ").await?;
        reader.query_drop("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY").await?;
        let mut snapshot=Vec::new();
        let mut after=None;
        let mut pages=0;
        loop {
            let page=snapshot_page(&mut reader,after).await?;
            if page.is_empty() {break;}
            for row in &page {
                if after.is_some_and(|after|row.meta_id<=after) {return Err("overlap snapshot cursor failed to advance".into());}
                after=Some(row.meta_id);
            }
            snapshot.extend(page);pages+=1;
            if snapshot.len()>12 || pages>4 {return Err("overlap snapshot row/page bound exceeded".into());}
            if pages==1 {
                control.query_drop("START TRANSACTION").await?;
                control.exec_drop("UPDATE rp_gtid_bootstrap_source SET meta_id=101,meta_value=? WHERE meta_id=100",(TWO,)).await?;
                if control.affected_rows()!=1 {return Err("overlap T2 PK move count differs".into());}
                control.query_drop("COMMIT").await?;
                expected.push(rows(control).await?);
                ends.push(coordinate(control,"SHOW BINARY LOG STATUS").await?.1);
            } else if pages==2 {
                control.query_drop("START TRANSACTION").await?;
                control.query_drop("DELETE FROM rp_gtid_bootstrap_source WHERE meta_id=101").await?;
                if control.affected_rows()!=1 {return Err("overlap T3 DELETE count differs".into());}
                control.exec_drop("INSERT INTO rp_gtid_bootstrap_source (meta_id,post_id,meta_key,meta_value) VALUES (100,?,?,?)",(0u64,b"aba-reused".as_slice(),THREE)).await?;
                if control.affected_rows()!=1 {return Err("overlap T3 ABA INSERT count differs".into());}
                control.query_drop("COMMIT").await?;
                expected.push(rows(control).await?);
                ends.push(coordinate(control,"SHOW BINARY LOG STATUS").await?.1);
            }
        }
        if snapshot.len()!=12 || pages!=4 || snapshot!=expected[0] || snapshot.iter().find(|row|row.meta_id==100).and_then(|row|row.meta_value.as_deref())!=Some(ONE) {return Err("overlap fresh RR snapshot followed later writer commits".into());}
        Ok(snapshot)
    }.await;
    let rollback = reader.query_drop("ROLLBACK").await;
    let close = close_fixture_connection(reader, "overlap_snapshot_reader").await;
    let snapshot = snapshot_result?;
    rollback?;
    close?;
    let (file, c) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if file != FILE
        || ends.len() != 3
        || c != ends[2]
        || ends.windows(2).any(|pair| pair[0] >= pair[1])
        || profile(control, version).await? != sid
    {
        return Err("overlap C or stable fresh profile differs".into());
    }
    let mut boot = bootstrap(options.clone(), &a, snapshot, c, &ends, &expected).await?;
    if !(4 < boot.floor && boot.floor < c) {
        return Err("overlap floor is not before C".into());
    }
    index_matches(control, &mut boot.view).await?;
    control.query_drop("START TRANSACTION").await?;
    control
        .exec_drop(
            "UPDATE rp_gtid_bootstrap_source SET meta_value=? WHERE meta_id=100",
            (FOUR,),
        )
        .await?;
    if control.affected_rows() != 1 {
        return Err("overlap T4 UPDATE count differs".into());
    }
    control.query_drop("COMMIT").await?;
    let (strict_file, d) = coordinate(control, "SHOW BINARY LOG STATUS").await?;
    if strict_file != FILE || d <= c {
        return Err("overlap strict endpoint did not advance".into());
    }
    let expected_d = rows(control).await?;
    let mut final_view = strict_seam(
        options,
        epoch,
        sid,
        boot.last_gtid,
        c,
        d,
        boot.view,
        &expected_d,
    )
    .await?;
    index_matches(control, &mut final_view).await?;
    if final_view.rows().len() != 12 || profile(control, version).await? != sid {
        return Err("overlap final rows or source profile changed".into());
    }
    Ok(
        json!({"format_version":1,"profile":"mysql_8_4_13_unpublished_overlap_bootstrap","process_pid":std::process::id(),
        "source_file":FILE,"start_position":4,"a_gtid_set":a_text,"floor_position":boot.floor,"c_position":c,"strict_position":d,
        "rows":12,"snapshot_rows":12,"bootstrap_commits":3,"strict_commits":1,"prefix_groups_skipped":boot.skipped,
        "full_stream_contiguous":true,"snapshot_overlap_verified":true,"pk_move_aba_verified":true,"rows_match":true,"index_matches":true,
        "strict_seam_verified":true,"unpublished_only":true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn physical_envelopes_restore_only_the_fde_algorithm_descriptor() {
        // The observed first-file FDE is 123 bytes; decoded data omits 19 header
        // bytes, one algorithm descriptor and four CRC32 bytes. Previous-GTIDs
        // has no descriptor: its 31-byte envelope contains eight data bytes.
        assert_eq!(envelope_size(99, true).unwrap(), 123);
        assert_eq!(envelope_size(99, false).unwrap(), 122);
        assert_eq!(envelope_size(8, false).unwrap(), 31);
        if usize::BITS == 64 {
            assert!(envelope_size(usize::MAX, true).is_err());
        }
    }
    fn row(id: u64, value: &[u8]) -> PostMetaRow {
        PostMetaRow {
            meta_id: id,
            post_id: 0,
            meta_key: None,
            meta_value: Some(value.to_vec()),
        }
    }
    #[test]
    fn gtid_set_is_bounded_single_origin_and_normal_not_tagged() {
        let text = "01234567-89ab-cdef-0123-456789abcdef";
        let sid = uuid(text).unwrap();
        let a = GtidSet::parse(&format!("{text}:1-3:8"), sid).unwrap();
        assert!(a.contains(1));
        assert!(a.contains(3));
        assert!(!a.contains(4));
        assert!(a.contains(8));
        for suffix in [
            "",
            ":0",
            ":01",
            ":3-1",
            ":1-3:2",
            ":tag:1",
            ":1,other:2",
            ":9223372036854775808",
        ] {
            assert!(GtidSet::parse(&format!("{text}{suffix}"), sid).is_err());
        }
        assert!(GtidSet::parse(&format!("{text}:1"), [0; 16]).is_err());
        assert!(uuid("01234567-89AB-cdef-0123-456789abcdef").is_err());
    }
    #[test]
    fn replay_keeps_later_a_transactions_after_first_non_a_to_repair_regression() {
        // Pure countermodel: A={T2}, B already sees T2. Replaying T1 alone
        // regresses B; suffix replay T1 then T2 repairs it. The live ON fixture
        // does not claim to manufacture a nonprefix executed set.
        let a = GtidSet {
            sid: [0; 16],
            intervals: vec![(2, 2)],
        };
        let mut floor = ReplayFloor::default();
        let mut map = BTreeMap::from([(1, row(1, b"T2"))]);
        assert!(!floor.skip_group(&a, 1, 100));
        overlay(&mut map, MetaChange::Insert(row(1, b"T1")));
        assert_eq!(map[&1].meta_value.as_deref(), Some(b"T1".as_slice()));
        assert!(!floor.skip_group(&a, 2, 200));
        overlay(&mut map, MetaChange::Insert(row(1, b"T2")));
        assert_eq!(floor.position, Some(100));
        assert_eq!(map[&1], row(1, b"T2"));
    }

    #[test]
    fn runtime_floor_skips_only_the_leading_a_groups_and_never_resets() {
        let a = GtidSet {
            sid: [0; 16],
            intervals: vec![(1, 2), (4, 4)],
        };
        let mut floor = ReplayFloor::default();
        assert!(floor.skip_group(&a, 1, 10));
        assert!(floor.skip_group(&a, 2, 20));
        assert_eq!(floor.position, None);
        assert!(!floor.skip_group(&a, 3, 30));
        assert!(!floor.skip_group(&a, 4, 40));
        assert!(!floor.skip_group(&a, 5, 50));
        assert_eq!(floor.position, Some(30));
    }

    #[test]
    fn shared_gtid_lengths_reject_zero_short_oversized_overflow_and_past_fence() {
        assert_eq!(transaction_end(100, 200, 80, 1000, 300).unwrap(), 300);
        assert_eq!(transaction_end(100, 200, 80, 1000, 400).unwrap(), 300);
        for (start, length, size, maximum, fence) in [
            (100, 0, 80, 1000, 300),
            (100, 79, 80, 1000, 300),
            (100, 200, 0, 1000, 300),
            (100, 200, 80, 199, 300),
            (100, 201, 80, 1000, 300),
            (u64::MAX, 80, 80, 1000, u64::MAX),
        ] {
            assert!(transaction_end(start, length, size, maximum, fence).is_err());
        }
        // A strict caller additionally requires exact D, not merely <= D.
        assert_ne!(transaction_end(100, 199, 80, 1000, 300).unwrap(), 300);
    }
    #[test]
    fn private_after_overlay_handles_pk_moves_aba_and_missing_deletes() {
        let mut map = BTreeMap::from([(100, row(100, b"new ABA"))]);
        overlay(&mut map, MetaChange::Insert(row(100, ONE)));
        overlay(
            &mut map,
            MetaChange::Update {
                before: row(100, ONE),
                after: row(101, TWO),
            },
        );
        overlay(&mut map, MetaChange::Delete(row(101, TWO)));
        overlay(&mut map, MetaChange::Insert(row(100, b"new ABA")));
        overlay(&mut map, MetaChange::Delete(row(101, TWO)));
        assert_eq!(map, BTreeMap::from([(100, row(100, b"new ABA"))]));
    }
}
