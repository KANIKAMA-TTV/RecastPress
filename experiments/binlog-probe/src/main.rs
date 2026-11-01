//! Disposable-database capability experiment, never a public process dependency.

use futures_util::StreamExt;
use mysql_async::Value;
use mysql_async::binlog::events::{BinlogEventFooter, BinlogEventHeader, Event, EventData};
use mysql_async::binlog::{row::BinlogRow, value::BinlogValue};
use mysql_async::{BinlogStreamRequest, Conn, OptsBuilder, SslOpts, prelude::Queryable};
use std::{env, path::PathBuf, time::Duration};

mod buffer_adapter;
mod ddl_probe;
mod durable_journal;
mod filtered_postmeta;
mod fresh_restore;
mod mariadb_snapshot_bootstrap;
mod missing_log_probe;
mod mysql_overlap_bootstrap;
mod mysql_rotation_handoff;
mod mysql_snapshot_race;
mod postmeta_journal;
mod postmeta_probe;
mod postmeta_process_crash;
mod row_types_probe;
mod snapshot_anchor_probe;

static SQL_CLOSE_RESETS: std::sync::Mutex<std::collections::BTreeMap<&'static str, usize>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

fn closed_socket_error(error: &mysql_async::Error) -> bool {
    matches!(error, mysql_async::Error::Io(mysql_async::IoError::Io(error))
        if matches!(error.kind(), std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe | std::io::ErrorKind::UnexpectedEof))
}

/// Only for a consumed, fixture-owned SQL connection after its work is complete.
/// The pinned driver sends COM_QUIT, then closes TLS; Windows may report a reset
/// when the server has already closed its end. Never use this for reads, writes,
/// stream errors, commit acknowledgements or connection establishment.
pub(crate) async fn close_fixture_connection(
    connection: Conn,
    label: &'static str,
) -> Result<(), Box<dyn std::error::Error>> {
    match connection.disconnect().await {
        Ok(()) => Ok(()),
        Err(error) if closed_socket_error(&error) => {
            *SQL_CLOSE_RESETS
                .lock()
                .map_err(|_| "cleanup observation lock failed")?
                .entry(label)
                .or_default() += 1;
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

static PROBE_STAGE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
fn stage(value: u8) {
    PROBE_STAGE.store(value, std::sync::atomic::Ordering::SeqCst);
}
fn stage_name() -> &'static str {
    match PROBE_STAGE.load(std::sync::atomic::Ordering::SeqCst) {
        1 => "control connection",
        2 => "fixture schema and source window",
        3 => "original journal initialization",
        4 => "original source open",
        5 => "original committed scan/journal",
        6 => "DDL rejection",
        7 => "fixed row types",
        8 => "postmeta projection/recovery",
        9 => "control disconnect",
        10 => "fresh-process postmeta restore",
        11 => "MariaDB snapshot anchor",
        12 => "MySQL snapshot race",
        13 => "postmeta process crash prepare",
        14 => "postmeta process crash resume",
        15 => "filtered postmeta transactions",
        16 => "MySQL overlapping bootstrap",
        17 => "MariaDB snapshot bootstrap",
        18 => "MySQL rotation handoff",
        19 => "MySQL server restart projection verification",
        _ => "fixture validation",
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Mode {
    Produce,
    RestorePostmeta,
    PreparePostmetaCrash,
    ResumePostmetaCrash,
    VerifyServerRestart,
    MysqlOverlapBootstrap,
    MariaSnapshotBootstrap,
    MysqlRotationHandoff,
}

fn parse_mode(arguments: &[String]) -> Result<Mode, Box<dyn std::error::Error>> {
    match arguments {
        [] => Ok(Mode::Produce),
        [flag] if flag == "--restore-postmeta" => Ok(Mode::RestorePostmeta),
        [flag] if flag == "--prepare-postmeta-crash" => Ok(Mode::PreparePostmetaCrash),
        [flag] if flag == "--resume-postmeta-crash" => Ok(Mode::ResumePostmetaCrash),
        [flag] if flag == "--verify-postmeta-server-restart" => Ok(Mode::VerifyServerRestart),
        [flag] if flag == "--mysql-overlap-bootstrap" => Ok(Mode::MysqlOverlapBootstrap),
        [flag] if flag == "--maria-snapshot-bootstrap" => Ok(Mode::MariaSnapshotBootstrap),
        [flag] if flag == "--mysql-rotation-handoff" => Ok(Mode::MysqlRotationHandoff),
        _ => Err("unknown or extra probe arguments".into()),
    }
}

#[tokio::main]
async fn main() {
    if let Err(error) = probe().await {
        eprintln!(
            "Binlog capability probe failed in {}: {error}",
            stage_name()
        );
        std::process::exit(1);
    }
}

async fn probe() -> Result<(), Box<dyn std::error::Error>> {
    let mode = parse_mode(&env::args().skip(1).collect::<Vec<_>>())?;
    if env::var("RECASTPRESS_DB_FIXTURE").as_deref() != Ok("disposable") {
        return Err("requires the owned disposable DB harness".into());
    }
    let expected = PathBuf::from(env::var("RECASTPRESS_DB_DATADIR")?).canonicalize()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let runs = root.join(".reference/db-runs").canonicalize()?;
    if expected.file_name().is_none_or(|name| name != "data")
        || expected.parent().and_then(|path| path.parent()) != Some(runs.as_path())
    {
        return Err("data directory is outside the owned fixture root".into());
    }
    let port = env::var("RECASTPRESS_DB_PORT")?.parse::<u16>()?;
    let mut options = OptsBuilder::default()
        .ip_or_hostname("127.0.0.1")
        .tcp_port(port)
        .prefer_socket(false)
        .user(Some(env::var("RECASTPRESS_DB_USER")?))
        .pass(Some(env::var("RECASTPRESS_DB_PASSWORD")?))
        .db_name(Some("recastpress_fixture"));
    if let Ok(ca) = env::var("RECASTPRESS_DB_CA") {
        let ca = PathBuf::from(ca).canonicalize()?;
        if ca.parent() != Some(expected.as_path()) {
            return Err("CA is not fixture-owned".into());
        }
        // The harness issues a loopback SAN certificate using its private CA.
        // Both certificate-chain and server-name validation remain enabled.
        options = options.ssl_opts(Some(
            SslOpts::default()
                .with_root_certs(vec![ca.into()])
                .with_disable_built_in_roots(true),
        ));
    }
    stage(1);
    let mut control = Conn::new(options.clone()).await?;
    require_fixture_tls(&mut control).await?;
    let (version, datadir, actual_port): (String, String, u16) = control
        .query_first("SELECT VERSION(), @@datadir, @@port")
        .await?
        .ok_or("no server identity")?;
    if PathBuf::from(datadir).canonicalize()? != expected
        || actual_port != port
        || !version.contains(&env::var("RECASTPRESS_DB_EXPECT_VERSION")?)
    {
        return Err("server identity is not the owned fixture; refusing writes".into());
    }
    let enabled: u8 = control
        .query_first("SELECT @@GLOBAL.log_bin")
        .await?
        .ok_or("no log status")?;
    if enabled != 1 {
        return Err("fixture has no binary log".into());
    }
    let epoch = expected
        .parent()
        .and_then(|path| path.file_name())
        .ok_or("no fixture epoch")?
        .to_string_lossy()
        .as_bytes()
        .to_vec();
    let status = if version.contains("MariaDB") {
        "SHOW MASTER STATUS"
    } else {
        "SHOW BINARY LOG STATUS"
    };
    if mode == Mode::MysqlRotationHandoff {
        stage(18);
        let mut result =
            mysql_rotation_handoff::run(&mut control, options, &epoch, &version).await?;
        close_fixture_connection(control, "control_final").await?;
        result
            .as_object_mut()
            .ok_or("rotation result is not an object")?
            .insert(
                "sql_disconnect_resets".into(),
                serde_json::json!(
                    *SQL_CLOSE_RESETS
                        .lock()
                        .map_err(|_| "cleanup observation lock failed")?
                ),
            );
        println!("MYSQL_ROTATION_HANDOFF_RESULT={result}");
        return Ok(());
    }
    if mode == Mode::MariaSnapshotBootstrap {
        stage(17);
        let mut result =
            mariadb_snapshot_bootstrap::run(&mut control, options, &epoch, &version).await?;
        close_fixture_connection(control, "control_final").await?;
        result
            .as_object_mut()
            .ok_or("Maria bootstrap result is not an object")?
            .insert(
                "sql_disconnect_resets".into(),
                serde_json::json!(
                    *SQL_CLOSE_RESETS
                        .lock()
                        .map_err(|_| "cleanup observation lock failed")?
                ),
            );
        println!("MARIADB_SNAPSHOT_BOOTSTRAP_RESULT={result}");
        return Ok(());
    }
    if mode == Mode::MysqlOverlapBootstrap {
        stage(16);
        let mut result =
            mysql_overlap_bootstrap::run(&mut control, options, &epoch, &version).await?;
        close_fixture_connection(control, "control_final").await?;
        result
            .as_object_mut()
            .ok_or("overlap result is not an object")?
            .insert(
                "sql_disconnect_resets".into(),
                serde_json::json!(
                    *SQL_CLOSE_RESETS
                        .lock()
                        .map_err(|_| "cleanup observation lock failed")?
                ),
            );
        println!("MYSQL_OVERLAP_BOOTSTRAP_RESULT={result}");
        return Ok(());
    }
    if mode == Mode::PreparePostmetaCrash {
        stage(13);
        postmeta_process_crash::prepare(
            &mut control,
            options,
            &epoch,
            status,
            version.contains("MariaDB"),
        )
        .await?;
        return Err("crash preparation unexpectedly returned without termination".into());
    }
    if mode == Mode::ResumePostmetaCrash || mode == Mode::VerifyServerRestart {
        use std::io::Read;
        stage(if mode == Mode::VerifyServerRestart {
            19
        } else {
            14
        });
        let mut wire = Vec::new();
        std::io::stdin().lock().take(65537).read_to_end(&mut wire)?;
        if wire.len() > 65536 {
            return Err("crash marker input exceeds its bound".into());
        }
        let wire = String::from_utf8(wire)?;
        let marker = wire.strip_suffix('\n').unwrap_or(&wire);
        let marker = marker.strip_suffix('\r').unwrap_or(marker);
        if marker.contains(['\r', '\n']) {
            return Err("crash marker input is not one line".into());
        }
        let mut result = if mode == Mode::VerifyServerRestart {
            postmeta_process_crash::verify_restart(&mut control, &epoch, &version, marker).await?
        } else {
            postmeta_process_crash::resume(
                &mut control,
                options,
                &epoch,
                status,
                version.contains("MariaDB"),
                marker,
            )
            .await?
        };
        close_fixture_connection(control, "control_final").await?;
        result
            .as_object_mut()
            .ok_or("crash result is not an object")?
            .insert(
                "sql_disconnect_resets".into(),
                serde_json::json!(
                    *SQL_CLOSE_RESETS
                        .lock()
                        .map_err(|_| "cleanup observation lock failed")?
                ),
            );
        if mode == Mode::VerifyServerRestart {
            println!("MYSQL_SERVER_RESTART_RESULT={result}");
        } else {
            println!("POSTMETA_CRASH_RESULT={result}");
        }
        return Ok(());
    }
    if mode == Mode::RestorePostmeta {
        stage(10);
        let source_file = env::var("RECASTPRESS_RESTORE_FILE")?;
        let position = env::var("RECASTPRESS_RESTORE_POSITION")?.parse::<u64>()?;
        let row_count = env::var("RECASTPRESS_RESTORE_ROWS")?.parse::<usize>()?;
        let restored =
            fresh_restore::verify(&mut control, &epoch, &source_file, position, row_count).await?;
        close_fixture_connection(control, "control_final").await?;
        let result = serde_json::json!({
            "format_version":1,"process_pid":std::process::id(),
            "source_file":restored.source_file,"position":restored.position,"rows":restored.rows,
            "row_data_matches":true,"index_matches":true,"receipt_validated":true,"store_unchanged":true,
            "sql_disconnect_resets":*SQL_CLOSE_RESETS.lock().map_err(|_| "cleanup observation lock failed")?,
        });
        println!("POSTMETA_RESTORE_RESULT={result}");
        return Ok(());
    }
    stage(2);
    control.query_drop("CREATE TABLE recastpress_fixture.rp_cdc_probe (id BIGINT UNSIGNED PRIMARY KEY, value VARBINARY(64)) ENGINE=InnoDB").await?;
    durable_journal::create_tables(&mut control).await?;
    let mut coordinates: mysql_async::Row = control
        .query_first(status)
        .await?
        .ok_or("no log coordinates")?;
    let file: Vec<u8> = coordinates.take("File").ok_or("no log file")?;
    let position: u64 = coordinates.take("Position").ok_or("no log position")?;
    control.query_drop("BEGIN").await?;
    control
        .query_drop("INSERT INTO rp_cdc_probe VALUES (1, X'610062FF')")
        .await?;
    control.query_drop("COMMIT").await?;
    control.query_drop("BEGIN").await?;
    control
        .query_drop("INSERT INTO rp_cdc_probe VALUES (2, X'726F6C6C6261636B')")
        .await?;
    control.query_drop("ROLLBACK").await?;
    control
        .query_drop("UPDATE rp_cdc_probe SET value=X'6368616E676564' WHERE id=1")
        .await?;
    control
        .query_drop("DELETE FROM rp_cdc_probe WHERE id=1")
        .await?;
    let mut final_coordinates: mysql_async::Row = control
        .query_first(status)
        .await?
        .ok_or("no final log coordinates")?;
    let final_file: Vec<u8> = final_coordinates.take("File").ok_or("no final log file")?;
    let stop_position: u64 = final_coordinates
        .take("Position")
        .ok_or("no final log position")?;
    if final_file != file || stop_position <= position {
        return Err("fixture source rotated or did not advance".into());
    }
    // Initialize AFTER capturing the finite source endpoint so journal writes
    // cannot feed back into this experiment. Continuous filtering is unverified.
    stage(3);
    let mut journal =
        durable_journal::AtomicJournal::initialize(&mut control, &epoch, &file, position).await?;
    stage(4);
    let mut connection = Conn::new(options.clone()).await?;
    require_fixture_tls(&mut connection).await?;
    // Separate source/client IDs, entirely within this owned fixture instance.
    let mut stream = Some(
        connection
            .get_binlog_stream(
                BinlogStreamRequest::new(7001)
                    .with_filename(&file)
                    .with_pos(position)
                    .with_non_blocking(),
            )
            .await?,
    );
    stage(5);
    let scan = async {
        let mut staged =
            buffer_adapter::ProbeBuffer::new(&epoch, &file, position, version.contains("MariaDB"));
        let mut event_count = 0usize;
        let mut session_event_count = 0usize;
        let mut session_position = position;
        let mut restart_point = None;
        let mut applied_before_restart = 0;
        let mut retry_before_restart = false;
        let mut derived_before_restart = 0;
        let mut incomplete = Some(buffer_adapter::ProbeBuffer::new(
            &epoch,
            &file,
            position,
            version.contains("MariaDB"),
        ));
        let mut partial_rejected = false;
        let mut missing_frame = Some(buffer_adapter::ProbeBuffer::new(
            &epoch,
            &file,
            position,
            version.contains("MariaDB"),
        ));
        let mut skipped_map = false;
        let mut gap_rejected = false;
        let mut row_count = 0usize;
        let mut commits = 0usize;
        let mut verified_checksums = 0usize;
        let mut corruption_rejected = false;
        let mut mutations = [0usize; 3];
        let mut kinds = std::collections::BTreeMap::<u8, usize>::new();
        while let Some(event) = stream
            .as_mut()
            .ok_or("source connection disappeared")?
            .next()
            .await
        {
            let event = event.map_err(|error| format!("committed stream phase: {error}"))?;
            let event = if session_event_count == 0 && version.contains("MariaDB") {
                normalize_maria_initial_rotate(event, &file, session_position)?
            } else {
                event
            };
            if verify_crc(&event)? {
                verified_checksums += 1;
            }
            event_count += 1;
            session_event_count += 1;
            *kinds.entry(event.header().event_type_raw()).or_default() += 1;
            if event_count > 1000 || event.data().len() > 1024 * 1024 {
                return Err::<_, Box<dyn std::error::Error>>("probe event bound exceeded".into());
            }
            let Some(data) = event.read_data()? else {
                return Err(format!(
                    "unsupported event type {}; cannot treat it as harmless progress",
                    event.header().event_type_raw()
                )
                .into());
            };
            match &data {
                EventData::RowsEvent(rows) => {
                    if !corruption_rejected {
                        let mut wire = Vec::new();
                        event.write(mysql_async::binlog::BinlogVersion::Version4, &mut wire)?;
                        *wire
                            .get_mut(BinlogEventHeader::LEN)
                            .ok_or("no row payload to corrupt")? ^= 1;
                        let damaged = Event::read(event.fde(), wire.as_slice())?;
                        if !matches!(verify_crc(&damaged), Err(ChecksumFault::Mismatch)) {
                            return Err(
                                "corrupted row event was not rejected by CRC before decode".into(),
                            );
                        }
                        corruption_rejected = true;
                    }
                    let table = stream
                        .as_ref()
                        .ok_or("source connection disappeared")?
                        .get_tme(rows.table_id())
                        .ok_or("missing table map")?;
                    for row in rows.rows(table) {
                        let (before, after) = row?;
                        match (before, after) {
                            (None, Some(after)) => {
                                check_row(&after, b"a\0b\xff")?;
                                mutations[0] += 1;
                            }
                            (Some(before), Some(after)) => {
                                check_row(&before, b"a\0b\xff")?;
                                check_row(&after, b"changed")?;
                                mutations[1] += 1;
                            }
                            (Some(before), None) => {
                                check_row(&before, b"changed")?;
                                mutations[2] += 1;
                            }
                            _ => return Err("row event has neither before nor after image".into()),
                        }
                        row_count += 1;
                    }
                }
                EventData::XidEvent(_) => commits += 1,
                _ => {}
            }
            staged
                .accept_durable(&event, &data, &mut journal)
                .await
                .map_err(|error| format!("original journal apply: {error}"))?;
            if matches!(data, EventData::XidEvent(_)) {
                if let Some(prefix) = incomplete.take() {
                    prefix.verify_incomplete_rejection()?;
                    partial_rejected = true;
                }
            } else if let Some(prefix) = &mut incomplete {
                prefix.accept(&event, &data)?;
            }
            if let Some(prefix) = &mut missing_frame {
                if !skipped_map && matches!(data, EventData::TableMapEvent(_)) {
                    skipped_map = true;
                } else if let Err(error) = prefix.accept(&event, &data) {
                    if !skipped_map {
                        return Err(error);
                    }
                    prefix.verify_fault_at_initial_checkpoint()?;
                    gap_rejected = true;
                    missing_frame = None;
                }
            }
            if matches!(data, EventData::XidEvent(_)) && restart_point.is_none() {
                staged.finish()?;
                applied_before_restart = staged.applied;
                retry_before_restart = staged.retry_preserved;
                derived_before_restart = staged.derived_positions;
                let acknowledged = staged.checkpoint_offset();
                if applied_before_restart != 1 || !partial_rejected || !gap_rejected {
                    return Err(
                        "reconnect did not start at the first verified commit boundary".into(),
                    );
                }
                stream
                    .take()
                    .ok_or("source connection disappeared")?
                    .close()
                    .await?;
                let durable = journal
                    .reopen_checkpoint(options.clone())
                    .await
                    .map_err(|error| format!("original journal reopen: {error}"))?;
                if durable != acknowledged || durable <= position || durable >= stop_position {
                    return Err(
                        "reopened durable checkpoint differs from the first source commit".into(),
                    );
                }
                // Discard decoder state and its session identity. Reconstruct
                // ONLY from the reloaded SQL boundary in this same owned fixture.
                staged = buffer_adapter::ProbeBuffer::resumed(
                    &epoch,
                    &file,
                    durable,
                    version.contains("MariaDB"),
                );
                let mut replacement = Conn::new(options.clone())
                    .await
                    .map_err(|error| format!("original resumed source connection: {error}"))?;
                require_fixture_tls(&mut replacement).await?;
                stream = Some(
                    replacement
                        .get_binlog_stream(
                            BinlogStreamRequest::new(7003)
                                .with_filename(&file)
                                .with_pos(durable)
                                .with_non_blocking(),
                        )
                        .await?,
                );
                session_position = durable;
                session_event_count = 0;
                restart_point = Some(durable);
            }
            if u64::from(event.header().log_pos()) == stop_position {
                if !matches!(data, EventData::XidEvent(_)) {
                    return Err("finite source endpoint is not a commit".into());
                }
                break;
            }
        }
        staged.finish()?;
        journal.verify(stop_position).await?;
        let missing_log_driver_fatal = missing_log_probe::verify(options.clone(), &epoch)
            .await
            .map_err(|error| format!("missing-log rejection phase: {error}"))?;
        // The failed source request must not alter the stored data/checkpoint.
        journal.verify(stop_position).await?;
        if applied_before_restart + staged.applied != 3
            || staged.applied != 2
            || !retry_before_restart
            || staged.retry_preserved
            || restart_point.is_none()
            || !partial_rejected
            || !gap_rejected
        {
            return Err(
                "real events did not produce three applied batches with a preserved retry".into(),
            );
        }
        Ok((
            event_count,
            row_count,
            commits,
            mutations,
            kinds,
            verified_checksums,
            corruption_rejected,
            derived_before_restart + staged.derived_positions,
            missing_log_driver_fatal,
        ))
    };
    let result = tokio::time::timeout(Duration::from_secs(10), scan).await;
    if let Some(stream) = stream.take() {
        stream
            .close()
            .await
            .map_err(|error| format!("committed stream close phase: {error}"))?;
    }
    drop(journal);
    let (
        events,
        rows,
        commits,
        mutations,
        kinds,
        checksums,
        corruption_rejected,
        derived_positions,
        missing_log_driver_fatal,
    ) = result??;
    if rows != 3 || commits != 3 || mutations != [1, 1, 1] || checksums == 0 || !corruption_rejected
    {
        return Err(
            format!("unexpected committed stream counts: rows={rows}, commits={commits}").into(),
        );
    }
    stage(6);
    let ddl_prefix_rejected = ddl_probe::verify(
        &mut control,
        options.clone(),
        &epoch,
        status,
        version.contains("MariaDB"),
    )
    .await
    .map_err(|error| format!("DDL rejection phase: {error}"))?;
    stage(7);
    let typed_rows = row_types_probe::verify(
        &mut control,
        options.clone(),
        &epoch,
        status,
        version.contains("MariaDB"),
    )
    .await
    .map_err(|error| format!("typed-row phase: {error}"))?;
    stage(8);
    let postmeta = postmeta_probe::verify(
        &mut control,
        options.clone(),
        &epoch,
        status,
        version.contains("MariaDB"),
    )
    .await
    .map_err(|error| format!("postmeta projection phase: {error}"))?;
    stage(11);
    let maria_snapshot_anchor =
        snapshot_anchor_probe::verify(&mut control, options.clone(), &version)
            .await
            .map_err(|error| format!("MariaDB snapshot anchor phase: {error}"))?;
    stage(12);
    let mysql_snapshot_race =
        mysql_snapshot_race::verify(&mut control, options.clone(), &version, status)
            .await
            .map_err(|error| format!("MySQL snapshot race phase: {error}"))?
            .map(|observation| {
                serde_json::json!({
                    "profile":"mysql_8_4_13", "source_file":observation.source_file,
                    "before_position":observation.before_position,
                    "reported_position":observation.reported_position,
                    "old_row_visible_after_commit":true, "new_row_visible_to_writer":true,
                    "reported_position_includes_unseen_commit":true,
                    "readonly_write_rejected":true, "readonly_error_code":1792
                })
            });
    stage(15);
    let filtered_postmeta = filtered_postmeta::verify(
        &mut control,
        options,
        &epoch,
        status,
        version.contains("MariaDB"),
    )
    .await
    .map_err(|error| format!("filtered postmeta phase: {error}"))?;
    stage(9);
    close_fixture_connection(control, "control_final").await?;
    let mut result = serde_json::json!({
        "sql_disconnect_resets":*SQL_CLOSE_RESETS.lock().map_err(|_| "cleanup observation lock failed")?,
        "format_version":1,"events":events,"rows":rows,"commits":commits,"mutations":mutations,
        "checksums":checksums,"corruption_rejected":corruption_rejected,"buffer_commits":3,
        "retry_preserved":true,"partial_rejected":true,"gap_rejected":true,
        "durable_batches":3,"durable_replays":1,"conflict_rejected":true,
        "derived_positions":derived_positions,"reconnected":true,"wrong_context_rejected":true,
        "missing_log_rejected":true,"missing_log_error_code":1236,"missing_log_driver_fatal":missing_log_driver_fatal,
        "checkpoint_preserved_after_missing_log":true,
        "ddl_rejected":true,"ddl_prefix_rejected":ddl_prefix_rejected,"event_types":kinds,
        "postmeta_source_resumed":postmeta.source_resumed,
        "postmeta_rollback_preserved":postmeta.rollback_preserved,"postmeta_context_rejected":postmeta.context_rejected,
        "postmeta_durable_commits":postmeta.durable_commits,"postmeta_durable_replays":postmeta.durable_replays,
        "postmeta_ack_loss_preserved":postmeta.ack_loss_preserved,"postmeta_conflict_rejected":postmeta.conflict_rejected,"postmeta_projection_reopened":postmeta.reopened,
        "postmeta_changes":postmeta.changes,"postmeta_memory_commits":postmeta.commits,
        "postmeta_projected_matches":true,"postmeta_old_view_preserved":true,"postmeta_apply_refusal_preserved":postmeta.apply_refusal_preserved,
        "typed_rows":typed_rows.rows,"typed_unsigned_reinterpreted":typed_rows.unsigned_reinterpreted,
    });
    let fields = result
        .as_object_mut()
        .ok_or("probe result is not an object")?;
    fields.insert("filtered_postmeta".into(), filtered_postmeta);
    fields.insert(
        "postmeta_shared_pool_commits".into(),
        serde_json::json!(postmeta.shared_pool_commits),
    );
    fields.insert(
        "postmeta_shared_pool_refusal_preserved".into(),
        serde_json::json!(postmeta.shared_pool_refusal_preserved),
    );
    fields.insert(
        "postmeta_shared_pool_released".into(),
        serde_json::json!(postmeta.shared_pool_released),
    );
    fields.insert(
        "postmeta_byte_limited_indexes".into(),
        serde_json::json!(postmeta.byte_limited_indexes),
    );
    fields.insert(
        "postmeta_logical_limit_preserved".into(),
        serde_json::json!(postmeta.logical_limit_preserved),
    );
    fields.insert(
        "postmeta_limited_commits".into(),
        serde_json::json!(postmeta.limited_commits),
    );
    fields.insert(
        "maria_snapshot_anchor".into(),
        serde_json::json!(maria_snapshot_anchor),
    );
    fields.insert(
        "mysql_snapshot_race".into(),
        serde_json::json!(mysql_snapshot_race),
    );
    fields.insert("probe_pid".into(), serde_json::json!(std::process::id()));
    fields.insert(
        "postmeta_source_file".into(),
        serde_json::json!(postmeta.source_file),
    );
    fields.insert(
        "postmeta_saved_position".into(),
        serde_json::json!(postmeta.saved_position),
    );
    fields.insert(
        "postmeta_saved_rows".into(),
        serde_json::json!(postmeta.saved_rows),
    );
    println!("BINLOG_PROBE_RESULT={result}");
    Ok(())
}

#[derive(Debug)]
enum ChecksumFault {
    UnsupportedAlgorithm,
    Mismatch,
}

impl std::fmt::Display for ChecksumFault {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::UnsupportedAlgorithm => "probe requires CRC32 checksums",
            Self::Mismatch => "binlog checksum mismatch",
        })
    }
}
impl std::error::Error for ChecksumFault {}

fn verify_crc(event: &Event) -> Result<bool, ChecksumFault> {
    let Some(expected) = event.checksum() else {
        return Ok(false);
    };
    let algorithm = event
        .footer()
        .get_checksum_alg()
        .map_err(|_| ChecksumFault::UnsupportedAlgorithm)?
        .ok_or(ChecksumFault::UnsupportedAlgorithm)?;
    if algorithm != mysql_async::binlog::BinlogChecksumAlg::BINLOG_CHECKSUM_ALG_CRC32 {
        return Err(ChecksumFault::UnsupportedAlgorithm);
    }
    if event.calc_checksum(algorithm) != u32::from_le_bytes(expected) {
        return Err(ChecksumFault::Mismatch);
    }
    Ok(true)
}

/// mysql_common starts before the FDE supplies checksum context. MariaDB's
/// artificial initial Rotate can already carry CRC32, which otherwise appears
/// as four extra filename bytes. Reparse the COMPLETE wire frame with CRC32
/// context and verify it; never obtain a filename by blindly trimming four bytes.
fn normalize_maria_initial_rotate(
    event: Event,
    file: &[u8],
    position: u64,
) -> Result<Event, Box<dyn std::error::Error>> {
    if event.checksum().is_some() {
        return Ok(event);
    }
    let Some(EventData::RotateEvent(rotate)) = event.read_data()? else {
        return Ok(event);
    };
    if rotate.name_raw().len() != file.len() + 4
        || !rotate.name_raw().starts_with(file)
        || rotate.position() != position
        || event.header().log_pos() != 0
        || !event
            .header()
            .flags()
            .contains(mysql_async::binlog::EventFlags::LOG_EVENT_ARTIFICIAL_F)
    {
        return Ok(event);
    }
    let mut wire = Vec::new();
    event.write(mysql_async::binlog::BinlogVersion::Version4, &mut wire)?;
    let context = event.fde().clone().with_footer(BinlogEventFooter::new(
        mysql_async::binlog::BinlogChecksumAlg::BINLOG_CHECKSUM_ALG_CRC32,
    ));
    let normalized = Event::read(&context, wire.as_slice())?;
    if !verify_crc(&normalized)? {
        return Err("initial Rotate has no verified CRC32".into());
    }
    Ok(normalized)
}

fn check_row(row: &BinlogRow, expected: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    if row.len() != 2
        || !matches!(
            row.as_ref(0),
            Some(BinlogValue::Value(Value::Int(1) | Value::UInt(1)))
        )
    {
        return Err("unexpected row identity; rollback row must not appear".into());
    }
    match row.as_ref(1) {
        Some(BinlogValue::Value(Value::Bytes(bytes))) if bytes == expected => Ok(()),
        _ => Err("row bytes differ from fixture's before/after value".into()),
    }
}

async fn require_fixture_tls(connection: &mut Conn) -> Result<(), Box<dyn std::error::Error>> {
    if env::var_os("RECASTPRESS_DB_CA").is_some() {
        let (_, cipher): (String, String) = connection
            .query_first("SHOW SESSION STATUS LIKE 'Ssl_cipher'")
            .await?
            .ok_or("no TLS session status")?;
        if cipher.is_empty() {
            return Err("CA-verifying fixture connection did not negotiate TLS".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod cleanup_tests {
    use super::{Mode, closed_socket_error, parse_mode};
    use std::io::{Error, ErrorKind};

    #[test]
    fn probe_modes_reject_unknown_and_extra_arguments() {
        assert_eq!(parse_mode(&[]).unwrap(), Mode::Produce);
        assert_eq!(
            parse_mode(&["--restore-postmeta".into()]).unwrap(),
            Mode::RestorePostmeta
        );
        assert!(parse_mode(&["--unknown".into()]).is_err());
        assert_eq!(
            parse_mode(&["--prepare-postmeta-crash".into()]).unwrap(),
            Mode::PreparePostmetaCrash
        );
        assert_eq!(
            parse_mode(&["--resume-postmeta-crash".into()]).unwrap(),
            Mode::ResumePostmetaCrash
        );
        assert_eq!(
            parse_mode(&["--mysql-overlap-bootstrap".into()]).unwrap(),
            Mode::MysqlOverlapBootstrap
        );
        assert_eq!(
            parse_mode(&["--maria-snapshot-bootstrap".into()]).unwrap(),
            Mode::MariaSnapshotBootstrap
        );
        assert_eq!(
            parse_mode(&["--mysql-rotation-handoff".into()]).unwrap(),
            Mode::MysqlRotationHandoff
        );
        assert_eq!(
            parse_mode(&["--verify-postmeta-server-restart".into()]).unwrap(),
            Mode::VerifyServerRestart
        );
        for flag in [
            "--verify-postmeta-server-restart",
            "--prepare-postmeta-crash",
            "--resume-postmeta-crash",
            "--mysql-overlap-bootstrap",
            "--maria-snapshot-bootstrap",
            "--mysql-rotation-handoff",
        ] {
            assert!(parse_mode(&[flag.into(), "--force".into()]).is_err());
        }
        assert!(parse_mode(&["--restore-postmeta".into(), "extra".into()]).is_err());
    }

    #[test]
    fn only_closed_socket_io_errors_are_classified_for_sql_disconnect_cleanup() {
        for kind in [
            ErrorKind::ConnectionReset,
            ErrorKind::BrokenPipe,
            ErrorKind::UnexpectedEof,
        ] {
            assert!(closed_socket_error(&mysql_async::Error::Io(
                mysql_async::IoError::Io(Error::from(kind))
            )));
        }
        assert!(!closed_socket_error(&mysql_async::Error::Server(
            mysql_async::ServerError {
                code: 1236,
                message: "missing fixture log".into(),
                state: "HY000".into(),
            }
        )));
        for kind in [
            ErrorKind::TimedOut,
            ErrorKind::ConnectionRefused,
            ErrorKind::PermissionDenied,
            ErrorKind::InvalidData,
        ] {
            assert!(!closed_socket_error(&mysql_async::Error::Io(
                mysql_async::IoError::Io(Error::from(kind))
            )));
        }
    }
}
