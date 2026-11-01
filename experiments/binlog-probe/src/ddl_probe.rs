//! Negative DDL test in the same already-verified disposable database only.
//! No server-wide lock or global setting/privilege change is performed.

use crate::{
    buffer_adapter::ProbeBuffer, normalize_maria_initial_rotate, require_fixture_tls, verify_crc,
};
use futures_util::StreamExt;
use mysql_async::{
    BinlogStreamRequest, Conn, OptsBuilder, binlog::events::EventData, prelude::Queryable,
};
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const ALTER: &str = "ALTER TABLE rp_cdc_probe ADD COLUMN fixture_ddl INT NULL";

async fn coordinate(control: &mut Conn, status: &'static str) -> Result<(Vec<u8>, u64)> {
    let mut row: mysql_async::Row = control
        .query_first(status)
        .await?
        .ok_or("no DDL log coordinates")?;
    Ok((
        row.take("File").ok_or("no DDL log file")?,
        row.take("Position").ok_or("no DDL log position")?,
    ))
}

pub(crate) async fn verify(
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
                .with_filename(&file)
                .with_pos(start)
                .with_non_blocking(),
        )
        .await?;
    let scan = async {
        let mut buffer = ProbeBuffer::new(epoch, &file, start, maria);
        let mut count = 0;
        let mut prefix_rejected = false;
        while let Some(event) = stream.next().await {
            let event = event?;
            let event = if count == 0 && maria {
                normalize_maria_initial_rotate(event, &file, start)?
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
