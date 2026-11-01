//! Negative observation for the owned MySQL 8.4.13 fixture only.
//! An SQL read view and ordinary global binlog status can describe different
//! commits. This finite observation is not a proof that every MySQL anchoring
//! method is impossible, or a production snapshot/CDC protocol.

use crate::{close_fixture_connection, require_fixture_tls};
use mysql_async::{Conn, OptsBuilder, prelude::Queryable};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(crate) struct Observations {
    pub(crate) source_file: String,
    pub(crate) before_position: u64,
    pub(crate) reported_position: u64,
}

async fn coordinate(connection: &mut Conn, status: &'static str) -> Result<(String, u64)> {
    let mut row: mysql_async::Row = connection
        .query_first(status)
        .await?
        .ok_or("missing MySQL race binlog status")?;
    let file: String = row.take("File").ok_or("missing MySQL race binlog file")?;
    let position: u64 = row
        .take("Position")
        .ok_or("missing MySQL race binlog position")?;
    if !file.starts_with("probe-bin.")
        || file.len() != "probe-bin.".len() + 6
        || !file["probe-bin.".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit())
        || position <= 4
    {
        return Err("invalid MySQL race binlog coordinate".into());
    }
    Ok((file, position))
}

async fn require_row(connection: &mut Conn, expected: &[u8]) -> Result<()> {
    let rows: Vec<(u64, Vec<u8>)> = connection
        .query("SELECT id, value FROM rp_mysql_snapshot_race ORDER BY id")
        .await?;
    if rows != [(1, expected.to_vec())] {
        return Err("MySQL race row differs from the required committed view".into());
    }
    Ok(())
}

pub(crate) async fn verify(
    control: &mut Conn,
    options: OptsBuilder,
    version: &str,
    status: &'static str,
) -> Result<Option<Observations>> {
    if version.contains("MariaDB") {
        return Ok(None);
    }
    if std::env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable")
        || std::env::var("RECASTPRESS_DB_EXPECT_VERSION").as_deref() != Ok("8.4.13")
        || version != "8.4.13"
        || status != "SHOW BINARY LOG STATUS"
    {
        return Err("snapshot race requires the pinned MySQL 8.4.13 fixture".into());
    }
    // Main has already verified fixture ownership, server identity and TLS.
    // This table does not belong to the WP source or projection schema.
    control.query_drop("CREATE TABLE recastpress_fixture.rp_mysql_snapshot_race (id BIGINT UNSIGNED PRIMARY KEY, value VARBINARY(64)) ENGINE=InnoDB").await?;
    control
        .query_drop("INSERT INTO rp_mysql_snapshot_race VALUES (1, X'6F6C64')")
        .await?;
    let mut reader = Conn::new(options).await?;
    let observation: Result<Observations> = async {
        require_fixture_tls(&mut reader).await?;
        reader
            .query_drop("SET SESSION TRANSACTION ISOLATION LEVEL REPEATABLE READ")
            .await?;
        reader
            .query_drop("START TRANSACTION WITH CONSISTENT SNAPSHOT, READ ONLY")
            .await?;
        let before = coordinate(control, status).await?;

        // Commit before the reader's first SELECT from the dedicated table:
        // WITH CONSISTENT SNAPSHOT must already have fixed its read view.
        control.query_drop("START TRANSACTION").await?;
        control
            .query_drop("UPDATE rp_mysql_snapshot_race SET value=X'6E6577' WHERE id=1")
            .await?;
        control.query_drop("COMMIT").await?;
        let writer_after = coordinate(control, status).await?;
        let reported = coordinate(&mut reader, status).await?;
        if before.0 != writer_after.0 || reported != writer_after || before.1 >= reported.1 {
            return Err("MySQL race status did not include the writer's later commit".into());
        }
        require_row(&mut reader, b"old").await?;
        require_row(control, b"new").await?;
        match reader
            .query_drop("UPDATE rp_mysql_snapshot_race SET value=X'72656A656374' WHERE id=1")
            .await
        {
            Err(mysql_async::Error::Server(error)) if error.code == 1792 => {}
            Err(error) => {
                return Err(format!("MySQL read-only UPDATE expected error 1792: {error}").into());
            }
            Ok(()) => return Err("MySQL read-only snapshot accepted an UPDATE".into()),
        }
        require_row(&mut reader, b"old").await?;
        require_row(control, b"new").await?;
        if coordinate(&mut reader, status).await? != reported {
            return Err("MySQL race status changed after rejected UPDATE".into());
        }
        Ok(Observations {
            source_file: reported.0,
            before_position: before.1,
            reported_position: reported.1,
        })
    }
    .await;
    // Always attempt both cleanup operations, including on a failed assertion.
    let rollback = reader.query_drop("ROLLBACK").await;
    let close = close_fixture_connection(reader, "mysql_race_reader").await;
    let observation = observation?;
    rollback?;
    close?;
    Ok(Some(observation))
}
