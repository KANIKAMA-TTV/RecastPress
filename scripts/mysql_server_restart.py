"""Finite crash of one captured disposable MySQL process, then read-only checks.

No service lookup, arbitrary PID termination, snapshot anchor, or binlog replay.
The initial schema/producer belongs to database_smoke's fresh owned fixture.
"""
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import secrets
import shutil
import subprocess
import time

import wordpress_reference as reference
from binlog_probe_result import parse_result as parse_producer, parse_restore_result
from crash_probe_result import parse_ready
from owned_probe import require_success_output, run_until_server_killed


PROFILE_SQL = ("SELECT VERSION(),@@datadir,@@port,@@server_uuid,@@gtid_mode,"
    "@@binlog_format,@@binlog_row_image,@@binlog_checksum,@@binlog_transaction_compression,"
    "@@sync_binlog,@@innodb_flush_log_at_trx_commit,@@innodb_force_recovery,@@log_bin")


def validate_restart_command(command, work):
    if (len(command) != 4 or Path(command[0]).name.lower() != "mysqld.exe"
            or command[1] != f"--defaults-file={work/'server.ini'}"
            or command[2:] != ["--no-monitor", "--console"]):
        raise ValueError("restart command is not the exact owned common command")
    text = (work/"server.ini").read_text(encoding="utf-8")
    if any(line.strip().lower().replace("_", "-").startswith("init-file") for line in text.splitlines()):
        raise ValueError("restart config would execute bootstrap SQL again")


def reap_owned_servers(servers):
    errors = []
    for process in reversed(servers):
        try:
            if process.poll() is None:
                process.kill()
        except BaseException as error:
            errors.append(f"owned PID {process.pid}: {type(error).__name__}: {error}")
        try:
            process.wait(timeout=10)
        except BaseException as error:
            errors.append(f"owned PID {process.pid} wait: {type(error).__name__}: {error}")
    if errors:
        raise RuntimeError("owned server cleanup failed; retain fixture: " + "; ".join(errors))


def run_experiment(*, root, work, server, servers, restart_command, log, client,
                   environment, run_hidden, run_probe):
    from check_mysql_server_restart_evidence import SCOPE, tracked_files, validate_record, parse_result
    validate_restart_command(restart_command, work)
    config_hash = reference.digest_file(work/"server.ini")
    certificates = {name: reference.digest_file(work/"data"/name)
        for name in ("ca.pem", "probe-server-cert.pem", "probe-server-key.pem")}

    def guard(process, *, wait=False):
        deadline = time.monotonic()+30
        while True:
            if process.poll() is not None:
                raise ValueError("owned server exited during restart identity guard")
            query = run_hidden([*client, "--execute="+PROFILE_SQL],
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=5)
            if query.returncode == 0:
                break
            if not wait or time.monotonic() >= deadline:
                raise ValueError("owned restart server did not accept profile query")
            time.sleep(0.1)
        fields = query.stdout.decode("utf-8", errors="strict").strip().split("\t")
        if (len(fields) != 13 or fields[0] != "8.4.13"
                or Path(fields[1]).resolve() != (work/"data").resolve()
                or fields[2] != environment["RECASTPRESS_DB_PORT"]
                or fields[4:] != ["OFF", "ROW", "FULL", "CRC32", "0", "1", "1", "0", "1"]):
            raise ValueError("owned restart server profile differs")
        if int((work/"server.pid").read_text(encoding="ascii").strip()) != process.pid:
            raise ValueError("owned restart server PID file differs")
        if reference.digest_file(work/"server.ini") != config_hash:
            raise ValueError("owned restart server configuration changed")
        if any(reference.digest_file(work/"data"/name) != digest for name, digest in certificates.items()):
            raise ValueError("owned restart TLS identity files changed")
        return fields

    original_profile = guard(server)
    source_uuid = original_profile[3]
    probe = str(root/"target/binlog-probe/debug/recastpress-binlog-probe.exe")
    probe_env = environment.copy()
    probe_env["RECASTPRESS_DB_DATADIR"] = str((work/"data").resolve())
    producer_pid, producer = run_probe([probe], probe_env)
    producer_wire = require_success_output(producer, "Restart baseline producer")
    observed = parse_producer(producer_wire.strip(), "mysql")
    if observed["probe_pid"] != producer_pid:
        raise ValueError("restart producer PID differs")
    restore_env = probe_env.copy()
    restore_env.update(RECASTPRESS_RESTORE_FILE=observed["postmeta_source_file"],
        RECASTPRESS_RESTORE_POSITION=str(observed["postmeta_saved_position"]),
        RECASTPRESS_RESTORE_ROWS=str(observed["postmeta_saved_rows"]))

    def restore():
        pid, completed = run_probe([probe, "--restore-postmeta"], restore_env)
        wire = require_success_output(completed, "Restart normal projection restore")
        parse_restore_result(wire.strip(), observed, expected_pid=pid)
        return pid, wire

    before_pid, before_wire = restore()
    nonce = secrets.token_hex(16)
    crash_env = probe_env.copy()
    crash_env.update(RECASTPRESS_CRASH_NONCE=nonce, RECASTPRESS_SERVER_RESTART_UUID=source_uuid)

    def before_kill(ready):
        if guard(server) != original_profile:
            raise ValueError("owned profile changed before kill")

    killed = run_until_server_killed([probe, "--prepare-postmeta-crash"], crash_env,
        lambda wire, expected_pid: parse_ready(wire, expected_pid=expected_pid,
            nonce=nonce, epoch=work.name.encode("ascii"), producer=observed),
        server, before_kill)
    # The old handle is reaped before a new handle is created. This path performs
    # no initialization, fixture DDL, producer replay, or bootstrap SQL.
    validate_restart_command(restart_command, work)
    restarted = subprocess.Popen(restart_command, cwd=work, stdout=log,
        stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    servers.append(restarted)
    if guard(restarted, wait=True) != original_profile:
        raise ValueError("restarted owned server identity/settings differ")
    source_log = work/killed.child.ready["source_file"]
    if not source_log.is_file() or source_log.stat().st_size < killed.child.ready["target_position"]:
        raise ValueError("pre-crash source file no longer covers committed source transactions")
    verifier_pid, verified = run_probe([probe, "--verify-postmeta-server-restart"], crash_env,
        input_bytes=killed.child.ready_line.encode("utf-8"))
    result_wire = require_success_output(verified, "Restart journal verification")
    parse_result(result_wire, expected_pid=verifier_pid, ready=killed.child.ready, expected_uuid=source_uuid)
    confirmation_pid, confirmation_wire = restore()
    if guard(restarted) != original_profile:
        raise ValueError("owned server changed during post-restart verification")
    lifecycle = {key: True for key in ("first_was_running", "first_pid_file_matches",
        "force_kill_requested", "first_exit_observed", "pending_was_running",
        "server_exited_before_pending_cleanup", "restarted_pid_file_matches",
        "restarted_was_running_after_verification", "same_datadir", "same_port",
        "same_tls_files", "same_settings", "same_uuid", "restarted_without_initialization",
        "restarted_without_init_file", "restarted_without_fixture_writes")}
    lifecycle.update(first_server_pid=server.pid, restarted_server_pid=restarted.pid,
        first_exit_code=killed.server_exit_code, pending_exit_code=killed.child.exit_code,
        initialization_runs=1, initial_bootstrap_runs=1)
    record = dict(format_version=1, scope=SCOPE, generated_at_utc=datetime.now(timezone.utc).isoformat(),
        server_version="8.4.13", transport="ca_and_name_verified_tls", nonce=nonce,
        epoch_hex=work.name.encode("ascii").hex(), source_uuid=source_uuid,
        producer_pid=producer_pid, producer_result=producer_wire,
        before_restore_pid=before_pid, before_restore_result=before_wire,
        ready_pid=killed.child.process_pid, ready=killed.child.ready_line, lifecycle=lifecycle,
        restore_pid=verifier_pid, result=result_wire, confirmation_pid=confirmation_pid,
        confirmation_result=confirmation_wire,
        tracked_files={path: reference.digest_file(root/path) for path in sorted(tracked_files())},
        limitations=["Finite owned MySQL process kill before candidate COMMIT; no existing service or data is used.",
            "One same-datadir restart checks prior journal durability and pending rollback; no binlog replay across restart.",
            "No OS/power-loss, COMMIT-time ambiguity, replacement-resistant source identity, production CDC, publication, backup or snapshot-anchor protocol is established."])
    validate_record(record)
    # Reap every captured handle before recording success or removing its data.
    reap_owned_servers(servers)
    log.flush()
    log.close()
    logs = root/"target/development-logs"
    logs.mkdir(exist_ok=True)
    shutil.copyfile(work/"server.log", logs/f"mysql-server-restart-{nonce}.log")
    (root/"compatibility/evidence/mysql-server-restart.json").write_text(
        json.dumps(record, indent=2)+"\n", encoding="utf-8")
    print(result_wire, end="")
