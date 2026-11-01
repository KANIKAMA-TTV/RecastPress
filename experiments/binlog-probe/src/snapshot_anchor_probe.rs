//! MariaDB 11.8.9 snapshot/position observation in the owned fixture only.
//! Uses a session transaction; server internals still synchronize snapshot start.

use crate::{close_fixture_connection, require_fixture_tls};
use mysql_async::{Conn, OptsBuilder, prelude::Queryable};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const FILE: &str = "probe-bin.000001";
const SNAPSHOT_STATUS: &str = "SHOW SESSION STATUS WHERE Variable_name IN ('Binlog_snapshot_file', 'Binlog_snapshot_position')";

fn parse_anchor(rows: Vec<(String, String)>) -> Result<(String, u64)> {
    if rows.len() != 2 {
        return Err("snapshot status must contain exactly two rows".into());
    }
    let mut file = None;
    let mut position = None;
    for (name, value) in rows {
        match name.as_str() {
            "Binlog_snapshot_file" if file.is_none() && value == FILE => file = Some(value),
            "Binlog_snapshot_position"
                if position.is_none()
                    && !value.is_empty()
                    && value.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                position = Some(value.parse::<u64>()?);
            }
            _ => return Err("invalid or duplicate snapshot status".into()),
        }
    }
    let position = position.ok_or("missing snapshot position")?;
    if position <= 4 {
        return Err("snapshot position is not beyond the binlog header".into());
    }
    Ok((file.ok_or("missing snapshot file")?, position))
}

async fn anchor(reader: &mut Conn) -> Result<(String, u64)> {
    parse_anchor(reader.query(SNAPSHOT_STATUS).await?)
}

async fn require_old_row(reader: &mut Conn) -> Result<()> {
    let rows: Vec<(u64, Vec<u8>)> = reader
        .query("SELECT id, value FROM rp_snapshot_anchor_probe ORDER BY id")
        .await?;
    if rows != [(1, b"old".to_vec())] {
        return Err("snapshot reader did not retain the old committed row".into());
    }
    Ok(())
}

pub(crate) async fn verify(
    control: &mut Conn,
    options: OptsBuilder,
    version: &str,
) -> Result<Option<serde_json::Value>> {
    if !version.contains("MariaDB") {
        return Ok(None);
    }
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable")
        || std::env::var("RECASTPRESS_DB_EXPECT_VERSION").as_deref() != Ok("11.8.9-MariaDB")
        || !version.starts_with("11.8.9-MariaDB")
    {
        return Err("snapshot anchor requires the pinned MariaDB 11.8.9 fixture".into());
    }
    // Invoked only after main's ownership, identity, binlog and TLS guards.
    // This dedicated table is outside the WP projection and its source windows.
    control.query_drop("CREATE TABLE recastpress_fixture.rp_snapshot_anchor_probe (id BIGINT UNSIGNED PRIMARY KEY, value VARBINARY(64)) ENGINE=InnoDB").await?;
    control
        .query_drop("INSERT INTO rp_snapshot_anchor_probe VALUES (1, X'6F6C64')")
        .await?;
    let mut reader = Conn::new(options).await?;
    require_fixture_tls(&mut reader).await?;
    reader
        .query_drop("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
        .await?;
    reader
        .query_drop("START TRANSACTION READ ONLY, WITH CONSISTENT SNAPSHOT")
        .await?;
    let initial = anchor(&mut reader).await?;

    // Deliberately commit before the reader's first SELECT from the probe table.
    control.query_drop("START TRANSACTION").await?;
    control
        .query_drop("UPDATE rp_snapshot_anchor_probe SET value=X'6E6577' WHERE id=1")
        .await?;
    control.query_drop("COMMIT").await?;
    let mut global: mysql_async::Row = control
        .query_first("SHOW MASTER STATUS")
        .await?
        .ok_or("missing global binlog coordinates after anchor update")?;
    let global_file: Vec<u8> = global.take("File").ok_or("missing global binlog file")?;
    let global_after: u64 = global
        .take("Position")
        .ok_or("missing global binlog position")?;
    if global_file != initial.0.as_bytes() || global_after <= initial.1 {
        return Err("global binlog did not advance in the anchor file".into());
    }
    require_old_row(&mut reader).await?;
    if anchor(&mut reader).await? != initial {
        return Err("snapshot anchor followed global status instead of the transaction".into());
    }
    match reader
        .query_drop("UPDATE rp_snapshot_anchor_probe SET value=X'72656A656374' WHERE id=1")
        .await
    {
        Err(mysql_async::Error::Server(error)) if error.code == 1792 => {}
        Err(error) => return Err(format!("read-only UPDATE expected error 1792: {error}").into()),
        Ok(()) => return Err("read-only snapshot accepted an UPDATE".into()),
    }
    require_old_row(&mut reader).await?;
    if anchor(&mut reader).await? != initial {
        return Err("snapshot anchor changed after rejected UPDATE".into());
    }
    reader.query_drop("ROLLBACK").await?;
    let rows: Vec<(u64, Vec<u8>)> = control
        .query("SELECT id, value FROM rp_snapshot_anchor_probe ORDER BY id")
        .await?;
    if rows != [(1, b"new".to_vec())] {
        return Err("other connection's committed update was not preserved".into());
    }
    close_fixture_connection(reader, "maria_anchor_reader").await?;
    Ok(Some(serde_json::json!({
        "profile":"mariadb_11_8_9", "source_file":initial.0, "position":initial.1,
        "global_after":global_after, "stable_during_transaction":true,
        "old_row_visible_after_commit":true, "readonly_write_rejected":true,
        "readonly_error_code":1792
    })))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(file: &str, position: &str) -> Vec<(String, String)> {
        vec![
            ("Binlog_snapshot_file".into(), file.into()),
            ("Binlog_snapshot_position".into(), position.into()),
        ]
    }

    #[test]
    fn anchor_status_is_exact_and_bounded() {
        assert_eq!(parse_anchor(rows(FILE, "123")).unwrap(), (FILE.into(), 123));
        for position in ["", "0", "4", "+5", "-1", "18446744073709551616"] {
            assert!(parse_anchor(rows(FILE, position)).is_err());
        }
        for file in ["", "probe-bin.000002", "../probe-bin.000001", "非ASCII"] {
            assert!(parse_anchor(rows(file, "123")).is_err());
        }
        assert!(parse_anchor(Vec::new()).is_err());
        let mut duplicate = rows(FILE, "123");
        duplicate[1] = duplicate[0].clone();
        assert!(parse_anchor(duplicate).is_err());
    }
}
