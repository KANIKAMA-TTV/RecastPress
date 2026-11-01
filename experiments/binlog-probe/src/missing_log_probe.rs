//! Read-only missing-file request in the already-owned binlog fixture.

use crate::require_fixture_tls;
use futures_util::StreamExt;
use mysql_async::{BinlogStreamRequest, Conn, Error, OptsBuilder, prelude::Queryable};
use std::time::Duration;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

fn missing_file_error(error: Error) -> Result<bool> {
    // Generic SQL server errors are not necessarily marked connection-fatal by
    // the driver. Neither that flag nor a normal EOF establishes binlog health.
    let driver_fatal = error.is_fatal();
    match error {
        Error::Server(error) if error.code == 1236 && error.state == "HY000" => Ok(driver_fatal),
        other => Err(format!(
            "missing-file request did not report the expected server error: {other}"
        )
        .into()),
    }
}

pub(crate) async fn verify(options: OptsBuilder, epoch: &[u8]) -> Result<bool> {
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
        return Err("missing-file request requires the owned disposable fixture".into());
    }
    let mut connection = Conn::new(options).await?;
    require_fixture_tls(&mut connection).await?;
    let missing = format!("missing-{}.000001", std::str::from_utf8(epoch)?).into_bytes();
    let files: Vec<mysql_async::Row> = connection.query("SHOW BINARY LOGS").await?;
    for mut row in files {
        let name: Vec<u8> = row.take("Log_name").ok_or("no log name in fixture index")?;
        if name == missing {
            return Err("negative-test log unexpectedly exists".into());
        }
    }
    let requested = connection
        .get_binlog_stream(
            BinlogStreamRequest::new(7004)
                .with_filename(&missing)
                .with_pos(4)
                .with_non_blocking(),
        )
        .await;
    let mut stream = match requested {
        Err(error) => return missing_file_error(error),
        Ok(stream) => stream,
    };
    let next = tokio::time::timeout(Duration::from_secs(5), stream.next()).await;
    // Validate the server reply before cleanup: after reporting error 1236 the
    // server may reset this replication connection. A close error must neither
    // hide that reply nor substitute for proof that the missing log was refused.
    let verified = match next? {
        Some(Err(error)) => missing_file_error(error),
        None => Err("normal EOF is not proof of a rejected missing log".into()),
        Some(Ok(event)) => Err(format!(
            "missing-file request unexpectedly returned event {}",
            event.header().event_type_raw()
        )
        .into()),
    };
    let _ = stream.close().await;
    verified
}
