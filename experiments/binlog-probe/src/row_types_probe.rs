//! Fixed-schema codec capability test. No generic WP row projection or
//! production schema cache is provided by this disposable-table experiment.

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
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const LABEL: &str = "日本語\0 🦀";
const CORPUS: [(u64, i64, Option<&[u8]>); 5] = [
    (0, i64::MIN, None),
    (1, -1, Some(b"")),
    ((1_u64 << 63) - 1, 0, Some(b"a\0b\xff")),
    (1_u64 << 63, 1, Some(b"\0")),
    (u64::MAX, i64::MAX, Some(b"last\xff")),
];

pub(crate) struct Observations {
    pub(crate) rows: usize,
    pub(crate) unsigned_reinterpreted: usize,
}

async fn coordinate(control: &mut Conn, status: &'static str) -> Result<(Vec<u8>, u64)> {
    let mut row: mysql_async::Row = control
        .query_first(status)
        .await?
        .ok_or("no type-probe log position")?;
    Ok((
        row.take("File").ok_or("no type-probe log file")?,
        row.take("Position").ok_or("no type-probe log offset")?,
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
        return Err("row type probe requires the owned fixture".into());
    }
    control.query_drop("CREATE TABLE rp_row_types_probe (id BIGINT UNSIGNED PRIMARY KEY,signed_value BIGINT NOT NULL,optional_value VARBINARY(64) NULL,label VARCHAR(64) CHARACTER SET utf8mb4 NOT NULL) ENGINE=InnoDB").await?;
    let columns: Vec<(String,String,String,String,Option<String>)> = control.query("SELECT COLUMN_NAME,DATA_TYPE,COLUMN_TYPE,IS_NULLABLE,CHARACTER_SET_NAME FROM information_schema.COLUMNS WHERE TABLE_SCHEMA=DATABASE() AND TABLE_NAME='rp_row_types_probe' ORDER BY ORDINAL_POSITION").await?;
    if columns.len() != 4
        || columns[0].0 != "id"
        || columns[0].1 != "bigint"
        || !columns[0].2.ends_with(" unsigned")
        || columns[0].3 != "NO"
        || columns[1].0 != "signed_value"
        || columns[1].1 != "bigint"
        || columns[1].2.contains("unsigned")
        || columns[1].3 != "NO"
        || columns[2].0 != "optional_value"
        || columns[2].1 != "varbinary"
        || columns[2].3 != "YES"
        || columns[2].4.is_some()
        || columns[3].0 != "label"
        || columns[3].1 != "varchar"
        || columns[3].3 != "NO"
        || columns[3].4.as_deref() != Some("utf8mb4")
    {
        return Err("type-probe schema differs from the fixed codec contract".into());
    }
    let (file, start) = coordinate(control, status).await?;
    control.query_drop("BEGIN").await?;
    for (id, signed, optional) in CORPUS {
        control
            .exec_drop(
                "INSERT INTO rp_row_types_probe VALUES (?,?,?,?)",
                (id, signed, optional, LABEL.as_bytes()),
            )
            .await?;
    }
    control.query_drop("COMMIT").await?;
    let (end_file, stop) = coordinate(control, status).await?;
    if file != end_file || stop <= start {
        return Err("row type window rotated or did not advance".into());
    }
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let mut stream = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7005)
                .with_filename(&file)
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?;
    let scan = async {
        let mut staged = ProbeBuffer::new(epoch, &file, start, maria);
        let mut count = 0;
        let mut observations = Observations {
            rows: 0,
            unsigned_reinterpreted: 0,
        };
        while let Some(event) = stream.next().await {
            let event = event?;
            let event = if count == 0 && maria {
                normalize_maria_initial_rotate(event, &file, start)?
            } else {
                event
            };
            count += 1;
            if count > 128 || event.data().len() > 1024 * 1024 {
                return Err("type-probe event bound exceeded".into());
            }
            verify_crc(&event)?;
            let data = event.read_data()?.ok_or("undecoded event in type probe")?;
            if let EventData::RowsEvent(rows) = &data {
                let table = stream
                    .get_tme(rows.table_id())
                    .ok_or("missing type-probe table map")?;
                if table.database_name_raw() != b"recastpress_fixture"
                    || table.table_name_raw() != b"rp_row_types_probe"
                    || table.columns_count() != 4
                    || table.get_column_type(0)? != Some(ColumnType::MYSQL_TYPE_LONGLONG)
                    || table.get_column_type(1)? != Some(ColumnType::MYSQL_TYPE_LONGLONG)
                    || table
                        .null_bitmask()
                        .iter()
                        .map(|bit| *bit)
                        .collect::<Vec<_>>()
                        != [false, false, true, false]
                {
                    return Err("type-probe table map differs from the verified schema".into());
                }
                for row in rows.rows(table) {
                    let (before, after) = row?;
                    if before.is_some() {
                        return Err("type probe did not receive an INSERT image".into());
                    }
                    let row = after.ok_or("type probe has no INSERT image")?;
                    observations.unsigned_reinterpreted += check_row(&row, observations.rows)?;
                    observations.rows += 1;
                }
            }
            staged.accept(&event, &data)?;
            if u64::from(event.header().log_pos()) == stop {
                if !matches!(data, EventData::XidEvent(_)) {
                    return Err("type-probe endpoint is not COMMIT".into());
                }
                break;
            }
        }
        staged.finish()?;
        if observations.rows != CORPUS.len()
            || staged.applied != 1
            || staged.checkpoint_offset() != stop
        {
            return Err("type-probe rows or committed boundary differ".into());
        }
        Ok::<_, Box<dyn std::error::Error>>(observations)
    };
    let result = tokio::time::timeout(Duration::from_secs(10), scan).await;
    stream.close().await?;
    result?
}

fn check_row(row: &BinlogRow, index: usize) -> Result<usize> {
    let &(expected_id, expected_signed, expected_optional) =
        CORPUS.get(index).ok_or("extra type-probe row")?;
    if row.len() != 4 {
        return Err("type-probe row width differs".into());
    }
    let (id, reinterpreted) = match row.as_ref(0) {
        Some(BinlogValue::Value(Value::UInt(value))) => (*value, 0),
        // This conversion is ONLY for the fixed, SQL-verified BIGINT UNSIGNED
        // column. It must never be applied blindly to a signed/unknown column.
        Some(BinlogValue::Value(Value::Int(value))) => (
            u64::from_le_bytes(value.to_le_bytes()),
            usize::from(*value < 0),
        ),
        _ => return Err("unsigned type-probe value has an unexpected representation".into()),
    };
    let signed = match row.as_ref(1) {
        Some(BinlogValue::Value(Value::Int(value))) => *value,
        Some(BinlogValue::Value(Value::UInt(value))) => i64::try_from(*value)?,
        _ => return Err("signed type-probe value has an unexpected representation".into()),
    };
    let optional = match row.as_ref(2) {
        Some(BinlogValue::Value(Value::NULL)) => None,
        Some(BinlogValue::Value(Value::Bytes(value))) => Some(value.as_slice()),
        _ => return Err("nullable type-probe value has an unexpected representation".into()),
    };
    if id != expected_id
        || signed != expected_signed
        || optional != expected_optional
        || !matches!(row.as_ref(3),Some(BinlogValue::Value(Value::Bytes(value))) if value==LABEL.as_bytes())
    {
        return Err("type-probe value differs from its inserted boundary/bytes".into());
    }
    Ok(reinterpreted)
}
