//! Finite, same-database filtering of verified row events in an owned fixture.
//! All raw row frames, including excluded rows, remain staged and receipted.
//! This saves no retained-payload budget and is not production CDC, source
//! identity, snapshot/log coordination or cross-database live verification.

use crate::{
    ChecksumFault,
    buffer_adapter::{Position, ProbeBuffer},
    close_fixture_connection, normalize_maria_initial_rotate, require_fixture_tls, verify_crc,
};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStreamRequest, Conn, OptsBuilder, TxOpts, Value,
    binlog::{
        events::{BinlogEventHeader, Event, EventData, TableMapEvent},
        row::BinlogRow,
        value::BinlogValue,
    },
    consts::ColumnType,
    prelude::Queryable,
};
use recastpress_db::{
    PostMetaRow,
    change_buffer::CommittedBatch,
    meta_view::{MetaChange, PostMetaView, SearchPlan},
};
use serde_json::{Value as Json, json};
use std::{collections::BTreeSet, time::Duration};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
type EncodedRows = Vec<(u64, u64, Option<Vec<u8>>, Option<Vec<u8>>)>;
type Stored = (Vec<u8>, Vec<u8>, String, u64, u64, Vec<u8>, Vec<u8>);
type SearchExpectation = (Option<Vec<u8>>, Vec<(u64,)>);
const PROFILE: &str = "fixture_same_db_filtered_postmeta";
const PAYLOAD_LIMIT: usize = 64 * 1024;
const ALTER: &str = "ALTER TABLE rp_filter_other ADD COLUMN fixture_ddl INT NULL";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Selection {
    Target,
    Other,
}
fn selection(database: &[u8], table: &[u8]) -> Result<Selection> {
    match (database, table) {
        (b"recastpress_fixture", b"rp_filter_target") => Ok(Selection::Target),
        (b"recastpress_fixture", b"rp_filter_other") => Ok(Selection::Other),
        _ => Err("filtered fixture encountered an unknown database/table".into()),
    }
}

fn validate_layout(which: Selection, types: &[Option<ColumnType>], nulls: &[bool]) -> Result<()> {
    let valid = match which {
        Selection::Target => {
            types.len() == 4
                && types[0] == Some(ColumnType::MYSQL_TYPE_LONGLONG)
                && types[1] == Some(ColumnType::MYSQL_TYPE_LONGLONG)
                && matches!(
                    types[2],
                    Some(ColumnType::MYSQL_TYPE_VARCHAR | ColumnType::MYSQL_TYPE_VAR_STRING)
                )
                && types[3] == Some(ColumnType::MYSQL_TYPE_BLOB)
                && nulls == [false, false, true, true]
        }
        Selection::Other => {
            types.len() == 2
                && types[0] == Some(ColumnType::MYSQL_TYPE_LONGLONG)
                && matches!(
                    types[1],
                    Some(
                        ColumnType::MYSQL_TYPE_VARCHAR
                            | ColumnType::MYSQL_TYPE_VAR_STRING
                            | ColumnType::MYSQL_TYPE_STRING
                    )
                )
                && nulls == [false, false]
        }
    };
    if !valid {
        return Err("filtered known table column types or null bitmap differ".into());
    }
    Ok(())
}

fn validate_table_map(table: &TableMapEvent<'_>) -> Result<Selection> {
    let which = selection(table.database_name_raw(), table.table_name_raw())?;
    let count = match which {
        Selection::Target => 4,
        Selection::Other => 2,
    };
    if table.columns_count() != count {
        return Err("filtered known table column count differs".into());
    }
    let types = (0..count as usize)
        .map(|index| table.get_column_type(index))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let nulls = table
        .null_bitmask()
        .iter()
        .map(|bit| *bit)
        .collect::<Vec<_>>();
    validate_layout(which, &types, &nulls)?;
    Ok(which)
}
fn file_valid(file: &str) -> bool {
    file.strip_prefix("probe-bin.")
        .is_some_and(|suffix| suffix.len() == 6 && suffix.bytes().all(|b| b.is_ascii_digit()))
}
fn encode(rows: &[PostMetaRow]) -> Result<Vec<u8>> {
    if !(11..=12).contains(&rows.len()) {
        return Err("filtered target row count bound differs".into());
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
    let data = serde_json::to_vec(&encoded)?;
    if data.len() > PAYLOAD_LIMIT {
        return Err("filtered row payload bound exceeded".into());
    }
    Ok(data)
}
fn receipt(frames: &[Vec<u8>]) -> Result<Vec<u8>> {
    if frames.is_empty() || frames.len() > 8 || frames.iter().any(Vec::is_empty) {
        return Err("filtered receipt frame shape differs".into());
    }
    let bytes = serde_json::to_vec(frames)?;
    if bytes.len() > PAYLOAD_LIMIT {
        return Err("filtered receipt bound exceeded".into());
    }
    Ok(bytes)
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
        .ok_or("filtered clone session SQL mode missing")?;
    let replacement = clone_sql_mode(&original);
    let insert: std::result::Result<(), mysql_async::Error> = async {
        if let Some(replacement) = &replacement {
            control
                .exec_drop("SET SESSION sql_mode = ?", (replacement,))
                .await?;
        }
        control
            .query_drop("INSERT INTO rp_filter_target SELECT * FROM wp_postmeta")
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
            "filtered source clone failed: {insert}; restoring session SQL mode also failed: {restore}"
        )
        .into()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error.into()),
    }
}

async fn rows(connection: &mut Conn) -> Result<Vec<PostMetaRow>> {
    let rows: EncodedRows = connection.query("SELECT meta_id,post_id,CAST(meta_key AS BINARY),CAST(meta_value AS BINARY) FROM rp_filter_target ORDER BY meta_id LIMIT 13").await?;
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(meta_id, post_id, meta_key, meta_value)| PostMetaRow {
            meta_id,
            post_id,
            meta_key,
            meta_value,
        })
        .collect();
    encode(&rows)?;
    Ok(rows)
}

async fn schema(connection: &mut Conn) -> Result<()> {
    let columns: Vec<(String,String,String,String,Option<String>)> = connection.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_filter_target' ORDER BY ORDINAL_POSITION").await?;
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
        return Err("filtered source SQL schema differs".into());
    }
    let engine: Option<String> = connection.query_first("SELECT ENGINE FROM information_schema.TABLES WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_filter_target'").await?;
    if engine.as_deref() != Some("InnoDB") {
        return Err("filtered source is not InnoDB".into());
    }
    Ok(())
}

async fn coordinate(connection: &mut Conn, status: &'static str) -> Result<(String, u64)> {
    let mut row: mysql_async::Row = connection
        .query_first(status)
        .await?
        .ok_or("filtered source coordinate missing")?;
    let file: String = row.take("File").ok_or("filtered source file missing")?;
    let position: u64 = row
        .take("Position")
        .ok_or("filtered source position missing")?;
    if !file_valid(&file) || position <= 4 {
        return Err("filtered source coordinate invalid".into());
    }
    Ok((file, position))
}

fn decode(row: &BinlogRow) -> Result<PostMetaRow> {
    if row.len() != 4 {
        return Err("filtered row image width differs".into());
    }
    let unsigned = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::UInt(value))) => Ok(*value),
        Some(BinlogValue::Value(Value::Int(value))) => Ok(u64::from_le_bytes(value.to_le_bytes())),
        _ => Err("filtered SQL-verified unsigned column image differs"),
    };
    let bytes = |index| match row.as_ref(index) {
        Some(BinlogValue::Value(Value::NULL)) => Ok(None),
        Some(BinlogValue::Value(Value::Bytes(value))) => Ok(Some(value.clone())),
        _ => Err("filtered nullable byte column image differs"),
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

async fn capture_expected(control: &mut Conn) -> Result<Expected> {
    let rows = rows(control).await?;
    let mut keys: BTreeSet<_> = rows.iter().map(|row| row.meta_key.clone()).collect();
    keys.insert(None);
    keys.insert(Some(b"filtered-missing-key".to_vec()));
    let mut searches = Vec::new();
    for key in keys {
        let ids = match &key {
            Some(key) => control.exec("SELECT meta_id FROM rp_filter_target WHERE CAST(meta_key AS BINARY)=CAST(? AS BINARY) ORDER BY meta_id",(key,)).await?,
            None => control.query("SELECT meta_id FROM rp_filter_target WHERE meta_key IS NULL ORDER BY meta_id").await?,
        };
        searches.push((key, ids));
    }
    Ok(Expected { rows, searches })
}

fn check_view(view: &mut PostMetaView, expected: &Expected) -> Result<()> {
    if view.rows() != expected.rows || view.build_key_index(12) != SearchPlan::ExactKeyIndex {
        return Err("filtered view rows or index differ from committed SQL".into());
    }
    for (key, ids) in &expected.searches {
        if view
            .matching_key(key.as_deref())
            .map(|row| (row.meta_id,))
            .collect::<Vec<_>>()
            != *ids
        {
            return Err("filtered index differs from captured binary SQL search".into());
        }
    }
    Ok(())
}

async fn other_schema(control: &mut Conn) -> Result<()> {
    let columns:Vec<(String,String,String,String,Option<String>)>=control.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_filter_other' ORDER BY ORDINAL_POSITION").await?;
    if columns.len() != 2
        || columns[0].0 != "id"
        || columns[0].1 != "bigint"
        || !columns[0].2.ends_with(" unsigned")
        || columns[0].3 != "NO"
        || columns[1].0 != "value"
        || columns[1].1 != "varbinary"
        || columns[1].3 != "NO"
        || columns[1].4.is_some()
    {
        return Err("filtered unrelated SQL schema differs".into());
    }
    Ok(())
}

fn check_other_image(row: &BinlogRow) -> Result<()> {
    if row.len() != 2
        || !matches!(
            row.as_ref(0),
            Some(BinlogValue::Value(Value::UInt(1) | Value::Int(1)))
        )
        || !matches!(row.as_ref(1),Some(BinlogValue::Value(Value::Bytes(value))) if value==b"other-one" || value==b"other-two")
    {
        return Err("filtered unrelated row image differs".into());
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Receipt {
    Applied,
    Replay,
}

struct Journal {
    connection: Conn,
    epoch: Vec<u8>,
    file: Vec<u8>,
}
impl Journal {
    async fn initialize(
        options: OptsBuilder,
        epoch: &[u8],
        file: &[u8],
        start: u64,
        rows: &[PostMetaRow],
    ) -> Result<Self> {
        let mut connection = Conn::new(options).await?;
        require_fixture_tls(&mut connection).await?;
        connection.query_drop("CREATE TABLE rp_filter_journal (id TINYINT UNSIGNED PRIMARY KEY,epoch VARBINARY(64) NOT NULL,log_file VARBINARY(255) NOT NULL,filter_profile VARBINARY(64) NOT NULL,from_position BIGINT UNSIGNED NOT NULL,through_position BIGINT UNSIGNED NOT NULL,receipt LONGBLOB NOT NULL,row_data LONGBLOB NOT NULL) ENGINE=InnoDB").await?;
        connection
            .exec_drop(
                "INSERT INTO rp_filter_journal VALUES (1,?,?,?,?,?,?,?)",
                (
                    epoch,
                    file,
                    PROFILE,
                    start,
                    start,
                    b"".as_slice(),
                    encode(rows)?,
                ),
            )
            .await?;
        Ok(Self {
            connection,
            epoch: epoch.to_vec(),
            file: file.to_vec(),
        })
    }
    async fn read(&mut self) -> Result<Stored> {
        self.connection.query_first("SELECT epoch,log_file,filter_profile,from_position,through_position,receipt,row_data FROM rp_filter_journal WHERE id=1").await?.ok_or_else(||"filtered journal singleton missing".into())
    }
    async fn apply(
        &mut self,
        batch: &CommittedBatch<Position>,
        rows: &[PostMetaRow],
        profile: &str,
        rollback: bool,
    ) -> Result<Receipt> {
        if !batch.from().matches_context(&self.epoch, &self.file)
            || !batch.through().matches_context(&self.epoch, &self.file)
            || !batch.from().same_session(batch.through())
        {
            return Err("filtered batch source context differs".into());
        }
        let frames: Vec<_> = batch.rows().map(<[u8]>::to_vec).collect();
        let receipt = receipt(&frames)?;
        let data = encode(rows)?;
        let mut tx = self.connection.start_transaction(TxOpts::default()).await?;
        let old:Stored=tx.query_first("SELECT epoch,log_file,filter_profile,from_position,through_position,receipt,row_data FROM rp_filter_journal WHERE id=1 FOR UPDATE").await?.ok_or("filtered journal baseline missing")?;
        if old.0 != self.epoch || old.1 != self.file || old.2 != profile || profile != PROFILE {
            tx.rollback().await?;
            return Err("filtered journal filter/source context differs".into());
        }
        if old.4 == batch.through().offset() {
            let same = old.3 == batch.from().offset() && old.5 == receipt && old.6 == data;
            tx.rollback().await?;
            return if same {
                Ok(Receipt::Replay)
            } else {
                Err("filtered replay receipt/data conflict".into())
            };
        }
        if old.4 != batch.from().offset() {
            tx.rollback().await?;
            return Err("filtered journal position gap".into());
        }
        tx.exec_drop("UPDATE rp_filter_journal SET from_position=?,through_position=?,receipt=?,row_data=? WHERE id=1",(batch.from().offset(),batch.through().offset(),&receipt,&data)).await?;
        if tx.affected_rows() != 1 {
            return Err("filtered journal UPDATE count differs".into());
        }
        if rollback {
            let staged: (u64, Vec<u8>, Vec<u8>) = tx
                .query_first(
                    "SELECT through_position,receipt,row_data FROM rp_filter_journal WHERE id=1",
                )
                .await?
                .ok_or("filtered uncommitted pair missing")?;
            if staged != (batch.through().offset(), receipt, data) {
                return Err("filtered own SQL transaction pair differs".into());
            }
            tx.rollback().await?;
            return Err("injected filtered SQL apply rollback".into());
        }
        tx.commit().await?;
        Ok(Receipt::Applied)
    }
    async fn verify(
        &mut self,
        from: u64,
        through: u64,
        frames: &[Vec<u8>],
        rows: &[PostMetaRow],
    ) -> Result<()> {
        let stored = self.read().await?;
        let expected = (
            self.epoch.clone(),
            self.file.clone(),
            PROFILE.to_owned(),
            from,
            through,
            receipt(frames)?,
            encode(rows)?,
        );
        if stored != expected {
            return Err("filtered durable view/checkpoint differs".into());
        }
        Ok(())
    }
}

struct Observations {
    selected: usize,
    ignored: usize,
    commits: usize,
    crc: bool,
    gap: bool,
}

#[allow(clippy::too_many_arguments)]
async fn consume(
    options: OptsBuilder,
    epoch: &[u8],
    file: &[u8],
    start: u64,
    ends: &[u64],
    maria: bool,
    initial: Vec<PostMetaRow>,
    expected: &[Expected],
    journal: &mut Journal,
) -> Result<Observations> {
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7021)
                .with_filename(file)
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?;
    let result=tokio::time::timeout(Duration::from_secs(15),async {
        let mut staged=ProbeBuffer::resumed(epoch,file,start,maria);
        let mut missing=Some(ProbeBuffer::resumed(epoch,file,start,maria));
        let mut skipped=false;
        let mut observed=Observations {selected:0,ignored:0,commits:0,crc:false,gap:false};
        let mut current=PostMetaView::new(initial).map_err(|_|"filtered initial identities repeat")?;
        let mut selected=Vec::new();
        let mut frames=Vec::new();
        let mut selected_in_commit=0;
        let mut ignored_in_commit=0;
        let mut events=0;
        while let Some(event)=stream.next().await {
            let event=event?;
            let event=if events==0 && maria {normalize_maria_initial_rotate(event,file,start)?} else {event};
            events+=1;
            if events>128 || event.data().len()>PAYLOAD_LIMIT {return Err("filtered event bound exceeded".into());}
            verify_crc(&event)?;
            let data=event.read_data()?.ok_or("filtered event undecoded")?;
            // Explicitly validate every TableMap, even one not followed by rows.
            if let EventData::TableMapEvent(table)=&data {validate_table_map(table)?;}
            let mut ignored_event=false;
            if let EventData::RowsEvent(event_rows)=&data {
                let table=stream.get_tme(event_rows.table_id()).ok_or("filtered table map missing")?;
                let which=validate_table_map(table)?;
                if which==Selection::Other {
                        ignored_event=true;
                        if !observed.crc {
                            let mut wire=Vec::new(); event.write(mysql_async::binlog::BinlogVersion::Version4,&mut wire)?;
                            *wire.get_mut(BinlogEventHeader::LEN).ok_or("filtered ignored payload missing")?^=1;
                            let damaged=Event::read(event.fde(),wire.as_slice())?;
                            if !matches!(verify_crc(&damaged),Err(ChecksumFault::Mismatch)) {return Err("filtered ignored CRC corruption accepted".into());}
                            observed.crc=true;
                        }
                    }
                frames.push(event.data().to_vec());
                if frames.len()>8 {return Err("filtered row frame count bound exceeded".into());}
                for row in event_rows.rows(table) {
                    let (before,after)=row?;
                    match which {
                        Selection::Target=>{
                            let change=match (before,after) {
                                (None,Some(after))=>MetaChange::Insert(decode(&after)?),
                                (Some(before),Some(after))=>MetaChange::Update {before:decode(&before)?,after:decode(&after)?},
                                (Some(before),None)=>MetaChange::Delete(decode(&before)?),
                                _=>return Err("filtered target image missing".into()),
                            };
                            selected.push(change); selected_in_commit+=1;observed.selected+=1;
                        }
                        Selection::Other=>{
                            if before.is_none() && after.is_none() {return Err("filtered unrelated image missing".into());}
                            if let Some(before)=before {check_other_image(&before)?;}
                            if let Some(after)=after {check_other_image(&after)?;}
                            ignored_in_commit+=1;observed.ignored+=1;
                        }
                    }
                    if selected_in_commit+ignored_in_commit>8 {return Err("filtered mutation bound exceeded".into());}
                }
            }
            if let Some(negative)=missing.as_mut() {
                if ignored_event && !skipped {skipped=true;}
                else if let Err(error)=negative.accept_projected(&event,&data,|_|Err("missing ignored row unexpectedly reached apply".into())) {
                    if !skipped || !error.to_string().starts_with("wire event positions are not contiguous") {return Err(error);}
                    negative.verify_fault_at_initial_checkpoint()?;
                    observed.gap=true;missing=None;
                }
            }
            let is_commit=matches!(data,EventData::XidEvent(_));
            let index=observed.commits;
            let mut candidate=if is_commit {
                let expected_counts=[(2,1),(0,1),(1,0)].get(index).ok_or("filtered extra COMMIT")?;
                if (selected_in_commit,ignored_in_commit)!=*expected_counts {return Err("filtered selected/ignored COMMIT shape differs".into());}
                let mut next=current.prepare_changes(std::mem::take(&mut selected)).map_err(|_|"filtered before image conflict")?;
                check_view(&mut next,&expected[index])?;
                Some(next)
            } else {None};
            let before_position=staged.checkpoint_offset();
            let prior=if is_commit {Some(journal.read().await?)} else {None};
            let result=staged.accept_projected_async(&event,&data,async |batch| {
                if !batch.rows().eq(frames.iter().map(Vec::as_slice)) {return Err("filtered receipt lost a selected or ignored frame".into());}
                let prepared=candidate.as_ref().ok_or("filtered apply callback before COMMIT")?;
                if batch.through().offset()!=ends[index] {return Err("filtered COMMIT endpoint differs from SQL".into());}
                if index==1 {journal.apply(batch,prepared.rows(),PROFILE,true).await?;return Err("filtered injected rollback unexpectedly succeeded".into());}
                if journal.apply(batch,prepared.rows(),PROFILE,false).await?!=Receipt::Applied {return Err("filtered first application replayed".into());}
                current=candidate.take().ok_or("filtered candidate missing")?;
                Ok(())
            }).await;
            if is_commit && index==1 {
                if !matches!(&result,Err(error) if error.to_string()=="injected filtered SQL apply rollback") || staged.checkpoint_offset()!=before_position || journal.read().await?!=*prior.as_ref().unwrap() || current.rows()!=expected[0].rows {
                    return Err("filtered SQL rollback changed pending data or checkpoint".into());
                }
                let pending_frames=frames.clone();
                let ack=staged.retry_projected_async(async |batch| {
                    if !batch.rows().eq(pending_frames.iter().map(Vec::as_slice)) {return Err("filtered rollback lost pending receipt".into());}
                    if journal.apply(batch,candidate.as_ref().ok_or("filtered pending candidate missing")?.rows(),PROFILE,false).await?!=Receipt::Applied {return Err("filtered retry did not COMMIT once".into());}
                    Err("injected filtered acknowledgement loss after COMMIT".into())
                }).await;
                if !matches!(&ack,Err(error) if error.to_string()=="injected filtered acknowledgement loss after COMMIT") || staged.checkpoint_offset()!=before_position || current.rows()!=expected[0].rows {return Err("filtered ACK loss advanced memory state".into());}
                journal.verify(before_position,ends[index],&frames,candidate.as_ref().unwrap().rows()).await?;
                let committed=journal.read().await?;
                staged.retry_projected_async(async |batch| {
                    if !batch.rows().eq(frames.iter().map(Vec::as_slice)) {return Err("filtered ACK retry receipt differs".into());}
                    let prepared=candidate.as_ref().ok_or("filtered ACK candidate missing")?;
                    let refused=journal.apply(batch,prepared.rows(),"different-filter-profile",false).await;
                    if !matches!(&refused,Err(error) if error.to_string()=="filtered journal filter/source context differs") || journal.read().await?!=committed {return Err("filtered wrong profile was not specifically refused or changed storage".into());}
                    if journal.apply(batch,prepared.rows(),PROFILE,false).await?!=Receipt::Replay {return Err("filtered ACK retry was not an exact replay".into());}
                    current=candidate.take().ok_or("filtered ACK candidate disappeared")?;
                    Ok(())
                }).await?;
                if journal.read().await?!=committed {return Err("filtered receipt replay modified storage".into());}
            } else {result?;}
            if is_commit {
                journal.verify(before_position,ends[index],&frames,current.rows()).await?;
                observed.commits+=1; frames.clear(); selected_in_commit=0; ignored_in_commit=0;
                if observed.commits==3 {break;}
            }
        }
        staged.finish()?;
        if observed.commits!=3 || observed.selected!=3 || observed.ignored!=2 || staged.applied!=3 || staged.checkpoint_offset()!=ends[2] || !frames.is_empty() || !selected.is_empty() || !observed.crc || !observed.gap {
            return Err("filtered finite stream observations incomplete".into());
        }
        Ok::<_,Box<dyn std::error::Error>>(observed)
    }).await;
    stream.close().await?;
    result?
}

async fn verify_ddl(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    status: &'static str,
    maria: bool,
) -> Result<bool> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
        return Err("DDL check requires the owned disposable harness".into());
    }
    let (file, start) = coordinate(control, status).await?;
    // control/options originate only from main's VERSION/datadir/port-verified
    // loopback fixture. This alters its toy probe table, never a WP/user table.
    control.query_drop(ALTER).await?;
    let (end_file, stop) = coordinate(control, status).await?;
    if end_file != file || stop <= start {
        return Err("DDL window rotated or did not advance".into());
    }
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7002)
                .with_filename(file.as_bytes())
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?;
    let scan = async {
        let mut buffer = ProbeBuffer::new(epoch, file.as_bytes(), start, maria);
        let mut count = 0;
        let mut prefix_rejected = false;
        while let Some(event) = stream.next().await {
            let event = event?;
            let event = if count == 0 && maria {
                normalize_maria_initial_rotate(event, file.as_bytes(), start)?
            } else {
                event
            };
            count += 1;
            if count > 64 || event.data().len() > 1024 * 1024 {
                return Err("DDL event bound exceeded".into());
            }
            let checksum_verified = verify_crc(&event)?;
            let data = event
                .read_data()?
                .ok_or("DDL window contains undecoded data")?;
            let actual_ddl = matches!(&data,EventData::QueryEvent(query) if query.error_code()==0 && query.schema_raw()==b"recastpress_fixture" && query.query_raw()==ALTER.as_bytes());
            if actual_ddl {
                if !checksum_verified || u64::from(event.header().log_pos()) != stop {
                    return Err("actual DDL checksum or end coordinate differs".into());
                }
                if buffer.accept(&event, &data).is_ok() {
                    return Err("actual DDL was incorrectly accepted".into());
                }
                buffer.verify_schema_fault_at_initial_checkpoint()?;
                // A repeated frame cannot heal the schema fault or apply data.
                if buffer.accept(&event, &data).is_ok() {
                    return Err("schema fault recovered without reconciliation".into());
                }
                buffer.verify_schema_fault_at_initial_checkpoint()?;
                return Ok(prefix_rejected);
            }
            if let Err(error) = buffer.accept(&event, &data) {
                if let EventData::QueryEvent(query) = &data {
                    // MariaDB's standalone GTID compatibility notification is
                    // truncated here. Keep the positive classifier rejecting it.
                    // Only this negative observer continues reading after the
                    // fault to confirm the actual ALTER and persistent refusal.
                    if maria
                        && !prefix_rejected
                        && checksum_verified
                        && query.query_raw() == b"# Dum"
                        && query.thread_id() == 0
                        && query.error_code() == 0
                        && query.schema_raw().is_empty()
                        && query.status_vars_raw().is_empty()
                        && event
                            .header()
                            .flags()
                            .contains(mysql_async::binlog::EventFlags::LOG_EVENT_SUPPRESS_USE_F)
                    {
                        buffer.verify_schema_fault_at_initial_checkpoint()?;
                        prefix_rejected = true;
                        continue;
                    }
                    return Err(format!("DDL fixture query differed before expected ALTER (schema={:?}, prefix={:?}, length={}): {error}",
                        query.schema_raw(), &query.query_raw()[..query.query_raw().len().min(64)], query.query_raw().len()).into());
                }
                return Err(error);
            }
        }
        Err::<bool, Box<dyn std::error::Error>>("DDL query was not observed and rejected".into())
    };
    let result = tokio::time::timeout(Duration::from_secs(10), scan).await;
    stream.close().await?;
    result?
}

pub(crate) async fn verify(
    control: &mut Conn,
    options: OptsBuilder,
    epoch: &[u8],
    status: &'static str,
    maria: bool,
) -> Result<Json> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable")
        || epoch.is_empty()
        || epoch.len() > 64
    {
        return Err("filtered probe requires the owned disposable fixture context".into());
    }
    // All operations below are private fixture tables after main's identity/TLS
    // guards. In particular, no changes are made to wp_postmeta or its journal.
    control
        .query_drop("CREATE TABLE rp_filter_target LIKE wp_postmeta")
        .await?;
    clone_rows_preserving_zero(control).await?;
    control.query_drop("CREATE TABLE rp_filter_other (id BIGINT UNSIGNED PRIMARY KEY,value VARBINARY(64) NOT NULL) ENGINE=InnoDB").await?;
    schema(control).await?;
    other_schema(control).await?;
    let initial = rows(control).await?;
    if initial.len() != 11
        || initial.first().map(|row| row.meta_id) != Some(0)
        || initial.last().map(|row| row.meta_id) != Some(u64::MAX)
        || initial.iter().any(|row| row.meta_id == 100)
    {
        return Err("filtered target clone identities or eleven-row count differ".into());
    }
    let (file, start) = coordinate(control, status).await?;
    let mut expected = Vec::new();
    let mut ends = Vec::new();
    for index in 0..3 {
        control.query_drop("START TRANSACTION").await?;
        match index {
            0 => {
                control.exec_drop("INSERT INTO rp_filter_target (meta_id,post_id,meta_key,meta_value) VALUES (100,?,?,?)",(u64::MAX,b"filtered\0key".as_slice(),b"target-one".as_slice())).await?;
                if control.affected_rows() != 1 {
                    return Err("filtered target INSERT count differs".into());
                }
                control
                    .query_drop("INSERT INTO rp_filter_other VALUES (1,X'6F746865722D6F6E65')")
                    .await?;
                if control.affected_rows() != 1 {
                    return Err("filtered other INSERT count differs".into());
                }
                control
                    .exec_drop(
                        "UPDATE rp_filter_target SET meta_value=? WHERE meta_id=100",
                        (b"target-two".as_slice(),),
                    )
                    .await?;
            }
            1 => {
                control
                    .exec_drop(
                        "UPDATE rp_filter_other SET value=? WHERE id=1",
                        (b"other-two".as_slice(),),
                    )
                    .await?
            }
            2 => {
                control
                    .query_drop("DELETE FROM rp_filter_target WHERE meta_id=100")
                    .await?
            }
            _ => unreachable!(),
        }
        if control.affected_rows() != 1 {
            return Err("filtered last source mutation count differs".into());
        }
        control.query_drop("COMMIT").await?;
        let (end_file, position) = coordinate(control, status).await?;
        if end_file != file || position <= *ends.last().unwrap_or(&start) {
            return Err("filtered source rotated or failed to advance".into());
        }
        ends.push(position);
        expected.push(capture_expected(control).await?);
    }
    if expected[0].rows.len() != 12
        || expected[1].rows != expected[0].rows
        || expected[2].rows != initial
    {
        return Err("filtered expected mixed/ignored/final SQL rows differ".into());
    }
    // Journal creation/writes occur after the finite stop. This avoids reading
    // the consumer's own writes without claiming a general production filter.
    let mut journal =
        Journal::initialize(options.clone(), epoch, file.as_bytes(), start, &initial).await?;
    let result:Result<Json>=async {
        let observed=consume(options.clone(),epoch,file.as_bytes(),start,&ends,maria,initial,&expected,&mut journal).await?;
        if rows(control).await?!=expected[2].rows {return Err("filtered final source rows changed".into());}
        let mut final_view=PostMetaView::new(expected[2].rows.clone()).map_err(|_|"filtered final identities repeat")?;
        check_view(&mut final_view,&capture_expected(control).await?)?;
        let before=journal.read().await?;
        let prefix=verify_ddl(control,options,epoch,status,maria).await?;
        if prefix!=maria || journal.read().await?!=before {return Err("filtered excluded-table DDL changed the durable pair or profile".into());}
        Ok(json!({"format_version":1,"profile":PROFILE,"source_file":file,"start_position":start,"end_position":ends[2],"rows":11,
            "commits":observed.commits,"selected_changes":observed.selected,"ignored_changes":observed.ignored,
            "mixed_commit_verified":true,"ignored_only_commit_verified":true,"apply_failure_preserved":true,"ack_loss_replayed":true,
            "receipt_includes_ignored":true,"ignored_crc_rejected":observed.crc,"ignored_gap_rejected":observed.gap,"ignored_ddl_rejected":true,
            "ignored_ddl_prefix_rejected":prefix,"wrong_filter_rejected":true,"row_data_matches":true,"index_matches":true,"cross_database_live_verified":false}))
    }.await;
    let close = close_fixture_connection(journal.connection, "filtered_postmeta_journal").await;
    let result = result?;
    close?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_table_metadata_is_rejected_without_waiting_for_a_row_event() {
        let target = selection(b"recastpress_fixture", b"rp_filter_target").unwrap();
        let other = selection(b"recastpress_fixture", b"rp_filter_other").unwrap();
        let types = [
            Some(ColumnType::MYSQL_TYPE_LONGLONG),
            Some(ColumnType::MYSQL_TYPE_LONGLONG),
            Some(ColumnType::MYSQL_TYPE_VARCHAR),
            Some(ColumnType::MYSQL_TYPE_BLOB),
        ];
        let nulls = [false, false, true, true];
        validate_layout(target, &types, &nulls).unwrap();
        assert!(validate_layout(target, &types[..3], &nulls).is_err());
        assert!(validate_layout(target, &types, &[false, false, false, true]).is_err());
        let mut malformed = types;
        malformed[0] = Some(ColumnType::MYSQL_TYPE_VARCHAR);
        assert!(validate_layout(target, &malformed, &nulls).is_err());
        malformed = types;
        malformed[3] = None;
        assert!(validate_layout(target, &malformed, &nulls).is_err());
        let other_types = [
            Some(ColumnType::MYSQL_TYPE_LONGLONG),
            Some(ColumnType::MYSQL_TYPE_VARCHAR),
        ];
        validate_layout(other, &other_types, &[false, false]).unwrap();
        assert!(validate_layout(other, &other_types, &[false, true]).is_err());
        assert!(validate_layout(other, &types, &nulls).is_err());
    }

    #[test]
    fn target_selection_uses_database_and_table_bytes_without_cross_db_fallback() {
        assert_eq!(
            selection(b"recastpress_fixture", b"rp_filter_target").unwrap(),
            Selection::Target
        );
        assert_eq!(
            selection(b"recastpress_fixture", b"rp_filter_other").unwrap(),
            Selection::Other
        );
        for (database, table) in [
            (b"different_db".as_slice(), b"rp_filter_target".as_slice()),
            (b"recastpress_fixture", b"unknown"),
            (b"RECASTPRESS_FIXTURE", b"rp_filter_target"),
        ] {
            assert!(selection(database, table).is_err());
        }
    }

    #[test]
    fn receipt_retains_ignored_frames_and_bounds_empty_or_oversized_payloads() {
        let frames = vec![
            b"selected-one".to_vec(),
            b"ignored".to_vec(),
            b"selected-two".to_vec(),
        ];
        let encoded = receipt(&frames).unwrap();
        assert_eq!(
            serde_json::from_slice::<Vec<Vec<u8>>>(&encoded).unwrap(),
            frames
        );
        let ignored_only = vec![b"ignored-only".to_vec()];
        assert_eq!(
            serde_json::from_slice::<Vec<Vec<u8>>>(&receipt(&ignored_only).unwrap()).unwrap(),
            ignored_only
        );
        assert!(receipt(&[]).is_err());
        assert!(receipt(&[Vec::new()]).is_err());
        assert!(receipt(&vec![vec![1]; 9]).is_err());
        assert!(receipt(&[vec![255; PAYLOAD_LIMIT]]).is_err());
    }

    #[test]
    fn clone_session_mode_preserves_explicit_zero_ids_without_duplicate_modes() {
        assert_eq!(clone_sql_mode(""), Some("NO_AUTO_VALUE_ON_ZERO".into()));
        assert_eq!(
            clone_sql_mode("STRICT_TRANS_TABLES"),
            Some("STRICT_TRANS_TABLES,NO_AUTO_VALUE_ON_ZERO".into())
        );
        assert_eq!(
            clone_sql_mode("STRICT_TRANS_TABLES,no_auto_value_on_zero"),
            None
        );
    }
}
