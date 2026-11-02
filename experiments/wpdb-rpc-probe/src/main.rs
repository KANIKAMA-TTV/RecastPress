#![forbid(unsafe_code)]

use mysql_async::{Conn, Error, Opts, OptsBuilder, SslOpts, Value as Cell, prelude::Queryable};
use serde_json::{Value, json};
use std::io::{self, Read};
use std::time::Duration;

const INPUT_SCHEMA: &str = "recastpress.wpdb-wire-input.v0.5";
const OUTPUT_SCHEMA: &str = "recastpress.wpdb-wire-observation.v0.5";
type ProbeResult<T> = Result<T, &'static str>;

struct Input {
    port: u16,
    password: String,
    tls: bool,
    ca: Option<Vec<u8>>,
}

fn check(ok: bool, reason: &'static str) -> ProbeResult<()> {
    if ok { Ok(()) } else { Err(reason) }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        output.push(DIGITS[(b >> 4) as usize] as char);
        output.push(DIGITS[(b & 15) as usize] as char);
    }
    output
}

fn parse_input(wire: &[u8]) -> ProbeResult<Input> {
    check(wire.len() <= 20 * 1024, "input_quota")?;
    let value: Value = serde_json::from_slice(wire).map_err(|_| "invalid_input_json")?;
    let object = value.as_object().ok_or("input_object_required")?;
    let mut keys: Vec<&str> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    check(
        keys == [
            "database",
            "host",
            "password",
            "port",
            "schema",
            "tls",
            "tls_ca_pem",
            "user",
        ],
        "input_exact_keys",
    )?;
    check(
        value["schema"] == INPUT_SCHEMA
            && value["host"] == "127.0.0.1"
            && value["database"] == "rp_wpdb_probe"
            && value["user"] == "rp_wpdb_probe",
        "input_closed_destination",
    )?;
    let port = value["port"]
        .as_u64()
        .filter(|p| (1..=65535).contains(p))
        .ok_or("input_port")? as u16;
    let password = value["password"].as_str().ok_or("input_secret_type")?;
    check(
        password.len() == 48
            && password
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "input_secret_shape",
    )?;
    let tls = match value["tls"].as_str() {
        Some("disabled") => false,
        Some("required") => true,
        _ => return Err("input_tls_profile"),
    };
    let ca = if tls {
        let pem = value["tls_ca_pem"].as_str().ok_or("input_ca_required")?;
        check(
            !pem.is_empty()
                && pem.len() <= 16 * 1024
                && pem.starts_with("-----BEGIN CERTIFICATE-----"),
            "input_ca_pem_quota",
        )?;
        Some(pem.as_bytes().to_vec())
    } else {
        check(
            value["tls_ca_pem"].is_null(),
            "disabled_tls_ca_must_be_null",
        )?;
        None
    };
    Ok(Input {
        port,
        password: password.to_owned(),
        tls,
        ca,
    })
}

fn options(input: &Input) -> Opts {
    let tls = input.ca.as_ref().map(|pem| {
        SslOpts::default()
            .with_root_certs(vec![pem.clone().into()])
            .with_disable_built_in_roots(true)
            .with_danger_accept_invalid_certs(false)
            .with_danger_skip_domain_validation(false)
    });
    OptsBuilder::default()
        .ip_or_hostname("127.0.0.1")
        .tcp_port(input.port)
        .user(Some("rp_wpdb_probe"))
        .pass(Some(input.password.clone()))
        .db_name(Some("rp_wpdb_probe"))
        .prefer_socket(false)
        .client_found_rows(false)
        .multi_statements(false)
        .local_files(false)
        .ssl_opts(tls)
        .into()
}

fn column_value(column: &mysql_async::Column) -> Value {
    json!({"schema_hex":hex(column.schema_ref()), "table_hex":hex(column.table_ref()),
        "original_table_hex":hex(column.org_table_ref()), "name_hex":hex(column.name_ref()),
        "original_name_hex":hex(column.org_name_ref()), "type":column.column_type() as u8,
        "flags":column.flags().bits(), "character_set":column.character_set(),
        "declared_length":column.column_length(), "decimals":column.decimals()})
}

async fn query(
    conn: &mut Conn,
    name: &str,
    sql: &[u8],
    observations: &mut Vec<Value>,
) -> ProbeResult<Value> {
    let mut observation = json!({"case":name,"sql_hex":hex(sql),"protocol":"COM_QUERY/TextProtocol",
        "complete":false,"columns":null,"rows":null});
    let mut result = match conn.query_iter(sql).await {
        Ok(result) => result,
        Err(_) => {
            observation["error_class"] = json!("unexpected_driver_or_server_error");
            observations.push(observation);
            return Err("query_failed");
        }
    };
    let columns: Vec<Value> = result.columns_ref().iter().map(column_value).collect();
    observation["columns"] = json!(columns);
    let mut rows: Vec<Value> = Vec::new();
    let row_result = async {
        while let Some(row) = result.next().await.map_err(|_| "row_receive_failed")? {
            check(
                rows.len() < 16 && row.len() == columns.len() && row.len() <= 16,
                "row_or_column_quota",
            )?;
            let mut values = Vec::with_capacity(row.len());
            for index in 0..row.len() {
                // A missing/taken slot is an error, never SQL NULL.
                values.push(
                    match row.as_ref(index).ok_or("missing_or_taken_row_slot")? {
                        Cell::NULL => Value::Null,
                        Cell::Bytes(bytes) => {
                            check(bytes.len() <= 4096, "cell_quota")?;
                            json!(hex(bytes))
                        }
                        _ => return Err("non_text_protocol_cell"),
                    },
                );
            }
            rows.push(json!(values));
        }
        check(result.is_empty(), "pending_result_set")?;
        Ok(())
    }
    .await;
    observation["rows"] = json!(rows);
    observation["affected_rows"] = json!(result.affected_rows().to_string());
    observation["last_insert_id"] = json!(result.last_insert_id().map(|n| n.to_string()));
    observation["warnings"] = json!(result.warnings());
    if let Err(reason) = row_result {
        observations.push(observation);
        return Err(reason);
    }
    result
        .drop_result()
        .await
        .map_err(|_| "result_drain_failed")?;
    observation["complete"] = json!(true);
    observations.push(observation.clone());
    Ok(observation)
}

fn expected_rows(observation: &Value, expected: Value) -> ProbeResult<()> {
    check(observation["rows"] == expected, "row_bytes_mismatch")
}

async fn syntax_rejection(
    conn: &mut Conn,
    name: &str,
    sql: &[u8],
    observations: &mut Vec<Value>,
) -> ProbeResult<()> {
    let failure = match conn.query_iter(sql).await {
        Ok(result) => {
            result
                .drop_result()
                .await
                .map_err(|_| "unexpected_success_drain_failed")?;
            observations.push(json!({"case":name,"sql_hex":hex(sql),"server_rejected":false}));
            return Err("server_did_not_reject_sql");
        }
        Err(error) => error,
    };
    match failure {
        Error::Server(error) => {
            // Only the fixed ASCII syntax-error corpus is covered. This is not
            // raw ERR packet preservation; the driver's String boundary remains.
            let accepted = error.code == 1064 && error.state == "42000" && error.message.is_ascii();
            observations.push(json!({"case":name,"sql_hex":hex(sql),"server_rejected":true,
                "code":error.code,"state":error.state,"ascii_message_hex":hex(error.message.as_bytes()),
                "message_is_ascii":error.message.is_ascii(),"raw_err_packet_preserved":false}));
            check(accepted, "unexpected_syntax_error_contract")
        }
        _ => {
            observations.push(json!({"case":name,"sql_hex":hex(sql),"server_rejected":false,"transport_or_driver_error":true}));
            Err("syntax_rejection_was_not_server_error")
        }
    }
}

fn handshake(conn: &Conn, label: &str, tls: bool, report: &mut Value) -> ProbeResult<()> {
    let mask = conn.negotiated_capabilities();
    use mysql_async::consts::CapabilityFlags as C;
    let off =
        !mask.intersects(C::CLIENT_FOUND_ROWS | C::CLIENT_MULTI_STATEMENTS | C::CLIENT_LOCAL_FILES);
    let observation = json!({"connection":label,"connection_id":conn.id(),"serialized_capabilities":mask.bits(),
        "found_rows":mask.contains(C::CLIENT_FOUND_ROWS),"multi_statements":mask.contains(C::CLIENT_MULTI_STATEMENTS),
        "local_files":mask.contains(C::CLIENT_LOCAL_FILES),"multi_results":mask.contains(C::CLIENT_MULTI_RESULTS),
        "ssl_capability":mask.contains(C::CLIENT_SSL),"raw_server_version_hex":hex(conn.raw_server_version()),
        "three_forbidden_capabilities_off":off});
    report["handshakes"]
        .as_array_mut()
        .unwrap()
        .push(observation);
    check(
        off && mask.contains(C::CLIENT_SSL) == tls
            && !conn.raw_server_version().is_empty()
            && conn.raw_server_version().len() <= 256,
        "handshake_contract",
    )
}

async fn check_tls(
    conn: &mut Conn,
    tls: bool,
    name: &str,
    observations: &mut Vec<Value>,
) -> ProbeResult<()> {
    let result = query(
        conn,
        name,
        b"SHOW SESSION STATUS LIKE 'Ssl_cipher'",
        observations,
    )
    .await?;
    let rows = result["rows"].as_array().ok_or("tls_status_rows")?;
    check(
        rows.len() == 1
            && rows[0].as_array().is_some_and(|row| row.len() == 2)
            && rows[0][0] == hex(b"Ssl_cipher"),
        "tls_status_shape",
    )?;
    let cipher = rows[0][1].as_str().ok_or("tls_status_cipher")?;
    check(
        if tls {
            !cipher.is_empty()
        } else {
            cipher.is_empty()
        },
        "actual_tls_profile_mismatch",
    )
}

async fn initial_cases(
    conn: &mut Conn,
    tls: bool,
    observations: &mut Vec<Value>,
) -> ProbeResult<Value> {
    check_tls(conn, tls, "initial_tls", observations).await?;
    let id = query(
        conn,
        "initial_connection_id",
        b"SELECT CONNECTION_ID()",
        observations,
    )
    .await?;
    expected_rows(&id, json!([[hex(conn.id().to_string().as_bytes())]]))?;
    let count = query(
        conn,
        "fixture_empty",
        b"SELECT COUNT(*) FROM rp_wire_fixture",
        observations,
    )
    .await?;
    expected_rows(&count, json!([[hex(b"0")]]))?;
    query(conn, "set_latin1", b"SET NAMES latin1", observations).await?;
    let raw = query(
        conn,
        "non_utf8_sql_bytes",
        b"SELECT _binary'\xff' AS raw_query_byte",
        observations,
    )
    .await?;
    expected_rows(&raw, json!([["ff"]]))?;
    let raw_name = query(
        conn,
        "non_utf8_column_name",
        b"SELECT 7 AS `\xff`",
        observations,
    )
    .await?;
    expected_rows(&raw_name, json!([[hex(b"7")]]))?;
    check(
        raw_name["columns"].as_array().is_some_and(|c| c.len() == 1)
            && raw_name["columns"][0]["name_hex"] == "ff",
        "raw_column_name_mismatch",
    )?;
    let inserted = query(conn,"insert",b"INSERT INTO rp_wire_fixture(marker,payload,dec_value,big_value) VALUES('rp-wire',X'00ff41',12345678901234567890.1234567890,18446744073709551615)",observations).await?;
    check(
        inserted["affected_rows"] == "1"
            && inserted["last_insert_id"]
                .as_str()
                .is_some_and(|s| s.parse::<u64>().is_ok_and(|n| n > 0)),
        "insert_id_or_affected",
    )?;
    let select_sql = b"SELECT payload AS duplicate,NULL AS duplicate,X'' AS empty_value,dec_value,big_value FROM rp_wire_fixture WHERE marker='rp-wire'";
    let values = query(conn, "raw_ordered_rows", select_sql, observations).await?;
    expected_rows(
        &values,
        json!([[
            "00ff41",
            null,
            "",
            hex(b"12345678901234567890.1234567890"),
            hex(b"18446744073709551615")
        ]]),
    )?;
    check(
        values["columns"][0]["name_hex"] == hex(b"duplicate")
            && values["columns"][1]["name_hex"] == hex(b"duplicate"),
        "duplicate_column_names_lost",
    )?;
    let zero = query(conn,"zero_rows_with_columns",b"SELECT payload AS duplicate,NULL AS duplicate,X'' AS empty_value,dec_value,big_value FROM rp_wire_fixture WHERE 1=0",observations).await?;
    expected_rows(&zero, json!([]))?;
    check(
        zero["columns"] == values["columns"]
            && zero["columns"].as_array().is_some_and(|c| c.len() == 5),
        "zero_row_metadata_lost",
    )?;
    let same = query(
        conn,
        "same_value_update",
        b"UPDATE rp_wire_fixture SET payload=X'00ff41' WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    check(same["affected_rows"] == "0", "found_rows_behavior")?;
    let changed = query(
        conn,
        "changed_update",
        b"UPDATE rp_wire_fixture SET payload=X'00ff42' WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    check(changed["affected_rows"] == "1", "changed_update_count")?;
    syntax_rejection(
        conn,
        "ascii_syntax_error",
        b"SELEC rp_fixed_ascii_error",
        observations,
    )
    .await?;
    let recovery = query(conn, "syntax_error_recovery", b"SELECT 17", observations).await?;
    expected_rows(&recovery, json!([[hex(b"17")]]))?;
    syntax_rejection(
        conn,
        "multi_statement_rejection",
        b"SELECT 1; SELECT 2",
        observations,
    )
    .await?;
    let recovery = query(conn, "multi_rejection_recovery", b"SELECT 23", observations).await?;
    expected_rows(&recovery, json!([[hex(b"23")]]))?;
    query(conn, "begin_rollback", b"BEGIN", observations).await?;
    query(
        conn,
        "transaction_update",
        b"UPDATE rp_wire_fixture SET payload=X'00ff43' WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    query(conn, "rollback", b"ROLLBACK", observations).await?;
    let restored = query(
        conn,
        "rollback_read",
        b"SELECT payload FROM rp_wire_fixture WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    expected_rows(&restored, json!([["00ff42"]]))?;
    query(
        conn,
        "session_marker",
        b"SET @rp_wire_marker=91",
        observations,
    )
    .await?;
    query(conn, "begin_before_disconnect", b"BEGIN", observations).await?;
    query(
        conn,
        "uncommitted_before_disconnect",
        b"UPDATE rp_wire_fixture SET payload=X'00ff44' WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    Ok(id["rows"].clone())
}

async fn fresh_cases(
    conn: &mut Conn,
    tls: bool,
    previous_id: &Value,
    observations: &mut Vec<Value>,
) -> ProbeResult<()> {
    check_tls(conn, tls, "fresh_tls", observations).await?;
    let id = query(
        conn,
        "fresh_connection_id",
        b"SELECT CONNECTION_ID()",
        observations,
    )
    .await?;
    expected_rows(&id, json!([[hex(conn.id().to_string().as_bytes())]]))?;
    check(id["rows"] != *previous_id, "connection_id_was_reused")?;
    let marker = query(
        conn,
        "fresh_session_variable",
        b"SELECT @rp_wire_marker",
        observations,
    )
    .await?;
    expected_rows(&marker, json!([[null]]))?;
    query(
        conn,
        "bounded_lock_wait",
        b"SET SESSION innodb_lock_wait_timeout=2",
        observations,
    )
    .await?;
    // A same-value UPDATE must obtain the row lock; this also checks that the
    // prior uncommitted transaction no longer holds it after disconnect.
    let no_lock = query(
        conn,
        "fresh_acquires_old_row_lock",
        b"UPDATE rp_wire_fixture SET payload=X'00ff42' WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    check(
        no_lock["affected_rows"] == "0",
        "disconnect_rollback_or_lock_release",
    )?;
    let restored = query(
        conn,
        "disconnect_rollback_read",
        b"SELECT payload FROM rp_wire_fixture WHERE marker='rp-wire'",
        observations,
    )
    .await?;
    expected_rows(&restored, json!([["00ff42"]]))
}

fn disconnect_error_value(error: &Error) -> Value {
    use mysql_async::{IoError, TlsError};
    use std::io::ErrorKind;

    // Public disconnect() combines COM_QUIT writing and stream shutdown. This
    // classification cannot identify which internal stage returned the error.
    let mut value = json!({"stage":"public_conn_disconnect","lower_stage_observed":false,
        "fatal":error.is_fatal()});
    match error {
        Error::Io(IoError::Io(error)) => {
            let kind = match error.kind() {
                ErrorKind::ConnectionReset => "connection_reset",
                ErrorKind::ConnectionAborted => "connection_aborted",
                ErrorKind::ConnectionRefused => "connection_refused",
                ErrorKind::BrokenPipe => "broken_pipe",
                ErrorKind::NotConnected => "not_connected",
                ErrorKind::UnexpectedEof => "unexpected_eof",
                ErrorKind::TimedOut => "timed_out",
                ErrorKind::WriteZero => "write_zero",
                ErrorKind::WouldBlock => "would_block",
                ErrorKind::Interrupted => "interrupted",
                ErrorKind::InvalidData => "invalid_data",
                ErrorKind::InvalidInput => "invalid_input",
                ErrorKind::PermissionDenied => "permission_denied",
                ErrorKind::Unsupported => "unsupported",
                ErrorKind::Other => "other",
                _ => "other_kind",
            };
            value["class"] = json!("io");
            value["io_kind"] = json!(kind);
            value["raw_os_error"] = json!(error.raw_os_error());
            value["inner_error_present"] = json!(error.get_ref().is_some());
        }
        Error::Io(IoError::Tls(error)) => {
            value["class"] = json!("tls");
            value["tls_variant"] = json!(match error {
                TlsError::Tls(_) => "rustls_error",
                TlsError::InvalidDnsName(_) => "invalid_dns_name",
                TlsError::Pem(_) => "pem_error",
                TlsError::VerifierBuilderError(_) => "verifier_builder_error",
            });
        }
        Error::Driver(_) => value["class"] = json!("driver"),
        Error::Server(error) => {
            value["class"] = json!("server");
            value["server_code"] = json!(error.code);
        }
        Error::Url(_) => value["class"] = json!("url"),
        Error::Other(_) => value["class"] = json!("other"),
    }
    value
}

async fn execute(input: &Input, report: &mut Value) -> ProbeResult<()> {
    let mut observations = Vec::new();
    let result = async {
        let mut conn = Conn::new(options(input))
            .await
            .map_err(|_| "initial_connect_failed")?;
        let first = match handshake(&conn, "initial", input.tls, report) {
            Ok(()) => initial_cases(&mut conn, input.tls, &mut observations).await,
            Err(reason) => Err(reason),
        };
        let disconnect_result = conn.disconnect().await;
        report["initial_disconnect_error"] = disconnect_result
            .as_ref()
            .err()
            .map(disconnect_error_value)
            .unwrap_or(Value::Null);
        let disconnected = disconnect_result.is_ok();
        report["initial_disconnect_completed"] = json!(disconnected);
        check(disconnected, "initial_disconnect_failed")?;
        let previous_id = first?;
        let mut fresh = Conn::new(options(input))
            .await
            .map_err(|_| "fresh_connect_failed")?;
        let second = match handshake(&fresh, "fresh", input.tls, report) {
            Ok(()) => fresh_cases(&mut fresh, input.tls, &previous_id, &mut observations).await,
            Err(reason) => Err(reason),
        };
        let disconnect_result = fresh.disconnect().await;
        report["fresh_disconnect_error"] = disconnect_result
            .as_ref()
            .err()
            .map(disconnect_error_value)
            .unwrap_or(Value::Null);
        let disconnected = disconnect_result.is_ok();
        report["fresh_disconnect_completed"] = json!(disconnected);
        check(disconnected, "fresh_disconnect_failed")?;
        second
    }
    .await;
    report["observations"] = json!(observations);
    result
}

fn main() {
    let mut report = json!({"schema":OUTPUT_SCHEMA,"outcome":"not_run","passed":false,
        "handshakes":[],"observations":[],"initial_disconnect_completed":false,"fresh_disconnect_completed":false,
        "initial_disconnect_error":null,"fresh_disconnect_error":null,
        "raw_non_utf8_err_verified":false,"native_wpdb_comparison_passed":false,
        "standard_wpdb_compatibility_verified":false,"product_driver_selected":false,
        "database_started_by_probe":false,"ddl_executed_by_probe":false,"physical_session_only":true});
    let result = (|| {
        check(std::env::args_os().len() == 1, "no_command_line_options")?;
        let mut wire = Vec::new();
        io::stdin()
            .take(20 * 1024 + 1)
            .read_to_end(&mut wire)
            .map_err(|_| "stdin_read_failed")?;
        let input = parse_input(&wire)?;
        report["tls_profile"] = json!(if input.tls { "required" } else { "disabled" });
        report["whole_probe_deadline_seconds"] = json!(60);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "runtime_creation_failed")?;
        runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(60), execute(&input, &mut report))
                .await
                .map_err(|_| "whole_probe_timeout")?
        })
    })();
    match result {
        Ok(()) => {
            report["passed"] = json!(true);
            report["outcome"] = json!("fixed_wire_cases_passed");
        }
        Err(reason) => {
            report["outcome"] = json!("fixed_wire_cases_failed");
            report["failure"] = json!(reason);
        }
    }
    // Errors are fixed identifiers. Never format options, input, credentials,
    // arbitrary driver errors or local certificate paths into public output.
    println!("{}", report);
    if result.is_err() {
        std::process::exit(1);
    }
}
