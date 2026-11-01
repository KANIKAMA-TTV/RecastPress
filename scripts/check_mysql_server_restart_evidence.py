"""Validate one owned MySQL pre-COMMIT server-process restart, without SQL."""
import json
import re
from datetime import datetime, timedelta

import wordpress_reference as reference
from binlog_probe_result import exact, parse_result as parse_producer, parse_restore_result, unique_object
from check_database_evidence import required_files
from crash_probe_result import parse_ready

PREFIX = "MYSQL_SERVER_RESTART_RESULT="
SCOPE = "candidate_mysql_server_restart_fixture_only"
PROFILE = "mysql_8_4_13_unpublished_server_precommit_restart_restore"
FIXED = {"format_version": 1, "profile": PROFILE, "rows": 11,
         **{key: True for key in ("old_tuple_matches", "uncommitted_candidate_absent",
              "rollback_drained", "baseline_row_data_matches", "index_matches",
              "source_commits_preserved", "store_unchanged", "receipt_validated", "no_projection_writes")}}
VARIABLE = {"process_pid", "nonce", "prepared_pid", "source_uuid", "source_file",
            "baseline_position", "candidate_position", "sql_disconnect_resets"}
LIFECYCLE_FIXED = {"initialization_runs": 1, "initial_bootstrap_runs": 1,
    **{key: True for key in ("first_was_running", "first_pid_file_matches", "force_kill_requested",
        "first_exit_observed", "pending_was_running", "server_exited_before_pending_cleanup",
        "restarted_pid_file_matches", "restarted_was_running_after_verification", "same_datadir",
        "same_port", "same_tls_files", "same_settings", "same_uuid", "restarted_without_initialization",
        "restarted_without_init_file", "restarted_without_fixture_writes")}}
LIFECYCLE_VARIABLE = {"first_server_pid", "restarted_server_pid", "first_exit_code", "pending_exit_code"}
RECORD_FIELDS = {"format_version", "scope", "generated_at_utc", "server_version", "transport",
    "nonce", "epoch_hex", "source_uuid", "producer_pid", "producer_result", "before_restore_pid",
    "before_restore_result", "ready_pid", "ready", "lifecycle", "restore_pid", "result",
    "confirmation_pid", "confirmation_result", "tracked_files", "limitations"}


def _uint(value, minimum=1):
    return type(value) is int and minimum <= value < 2**64


def _uuid(value):
    return type(value) is str and re.fullmatch(r"[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}", value) is not None


def parse_result(wire, *, expected_pid, ready, expected_uuid):
    if (type(wire) is not str or len(wire.encode("utf-8")) > 65536
            or not wire.startswith(PREFIX) or not wire.endswith("\n")
            or "\n" in wire[:-1] or "\r" in wire):
        raise ValueError("server restart result must be one bounded protocol line")
    if (not _uint(expected_pid) or not _uuid(expected_uuid) or not isinstance(ready, dict)
            or not _uint(ready.get("process_pid")) or expected_pid == ready["process_pid"]
            or type(ready.get("nonce")) is not str or not re.fullmatch(r"[0-9a-f]{32}", ready["nonce"])
            or type(ready.get("source_file")) is not str or not re.fullmatch(r"probe-bin\.[0-9]{6}", ready["source_file"])
            or not _uint(ready.get("baseline_position"), 5) or not _uint(ready.get("target_position"), 5)
            or ready["baseline_position"] >= ready["target_position"] or not exact(ready.get("rows"), 11)):
        raise ValueError("server restart expected process, UUID or READY context differs")
    try:
        value = json.loads(wire[len(PREFIX):-1], object_pairs_hook=unique_object)
    except (ValueError, json.JSONDecodeError) as error:
        raise ValueError("invalid server restart result JSON") from error
    required = {**FIXED, "process_pid": expected_pid, "nonce": ready["nonce"],
        "prepared_pid": ready["process_pid"], "source_uuid": expected_uuid,
        "source_file": ready["source_file"], "baseline_position": ready["baseline_position"],
        "candidate_position": ready["target_position"]}
    if (not isinstance(value, dict) or set(value) != set(FIXED) | VARIABLE
            or any(not exact(value[key], expected) for key, expected in required.items())):
        raise ValueError("server restart native observations differ")
    resets = value["sql_disconnect_resets"]
    if (not isinstance(resets, dict) or set(resets) - {"control_final"}
            or any(not exact(count, 1) for count in resets.values())):
        raise ValueError("server restart SQL cleanup labels/counts differ")
    return value


def tracked_files():
    return {*required_files(), "scripts/setup_mysql.py", "scripts/database_smoke.py", "scripts/check.py",
        "scripts/owned_probe.py", "scripts/binlog_probe_result.py", "scripts/crash_probe_result.py",
        "scripts/mysql_server_restart.py", "scripts/check_mysql_server_restart_evidence.py",
        "scripts/test_mysql_server_restart_evidence.py", "scripts/test_owned_server_crash.py",
        "scripts/test_mysql_server_restart_lifecycle.py", "scripts/test_binlog_probe_result.py",
        "scripts/test_crash_probe_result.py", "scripts/test_owned_probe.py", "scripts/test_database_process.py",
        "experiments/binlog-probe/Cargo.toml", "experiments/binlog-probe/Cargo.lock",
        *[path.relative_to(reference.ROOT).as_posix()
          for path in (reference.ROOT / "experiments/binlog-probe/src").glob("*.rs")]}


def validate_record(record):
    if not isinstance(record, dict) or set(record) != RECORD_FIELDS:
        raise ValueError("server restart evidence fields are incomplete or unknown")
    for key, expected in {"format_version": 1, "scope": SCOPE, "server_version": "8.4.13",
                          "transport": "ca_and_name_verified_tls"}.items():
        if not exact(record[key], expected):
            raise ValueError("server restart evidence profile differs: " + key)
    if not _uuid(record["source_uuid"]) or type(record["nonce"]) is not str or not re.fullmatch(r"[0-9a-f]{32}", record["nonce"]):
        raise ValueError("server restart evidence UUID/nonce differs")
    timestamp = record["generated_at_utc"]
    if type(timestamp) is not str:
        raise ValueError("server restart timestamp must be a UTC datetime")
    try:
        parsed = datetime.fromisoformat(timestamp)
    except ValueError as error:
        raise ValueError("server restart timestamp must be a UTC datetime") from error
    if "T" not in timestamp or parsed.utcoffset() != timedelta(0):
        raise ValueError("server restart timestamp must be a UTC datetime")
    lifecycle = record["lifecycle"]
    if (not isinstance(lifecycle, dict) or set(lifecycle) != set(LIFECYCLE_FIXED) | LIFECYCLE_VARIABLE
            or any(not exact(lifecycle[key], value) for key, value in LIFECYCLE_FIXED.items())):
        raise ValueError("server restart lifecycle observations differ")
    for key in ("first_exit_code", "pending_exit_code"):
        code = lifecycle[key]
        if type(code) is not int or not -(2**31) <= code < 2**32 or code == 0:
            raise ValueError("server/pending child did not exit abnormally")
    process_ids = [record[key] for key in ("producer_pid", "before_restore_pid", "ready_pid", "restore_pid", "confirmation_pid")]
    process_ids += [lifecycle[key] for key in ("first_server_pid", "restarted_server_pid")]
    if any(not _uint(pid) for pid in process_ids) or len(set(process_ids)) != len(process_ids):
        raise ValueError("server restart owned process identities are invalid or repeated")
    for key, prefix in {"producer_result": "BINLOG_PROBE_RESULT=", "before_restore_result": "POSTMETA_RESTORE_RESULT=",
            "ready": "POSTMETA_CRASH_READY=", "result": PREFIX, "confirmation_result": "POSTMETA_RESTORE_RESULT="}.items():
        wire = record[key]
        if (type(wire) is not str or not wire.startswith(prefix) or len(wire.encode("utf-8")) > 65536 or not wire.endswith("\n")
                or "\n" in wire[:-1] or "\r" in wire):
            raise ValueError("server restart evidence raw protocol line differs: " + key)
    producer = parse_producer(record["producer_result"].strip(), "mysql")
    if not exact(producer["probe_pid"], record["producer_pid"]):
        raise ValueError("server restart producer differs from the owned PID")
    parse_restore_result(record["before_restore_result"], producer, expected_pid=record["before_restore_pid"])
    epoch_hex = record["epoch_hex"]
    if type(epoch_hex) is not str or not re.fullmatch(r"[0-9a-f]+", epoch_hex) or len(epoch_hex) % 2:
        raise ValueError("server restart fixture epoch is not canonical hex")
    epoch = bytes.fromhex(epoch_hex)
    if not re.fullmatch(rb"mysql-[0-9a-f]{32}", epoch):
        raise ValueError("server restart fixture epoch is not an owned MySQL UUID directory")
    ready = parse_ready(record["ready"], expected_pid=record["ready_pid"], nonce=record["nonce"], epoch=epoch, producer=producer)
    baseline = json.loads(bytes.fromhex(ready["old_tuple"]["row_data_hex"]))
    if (baseline[0][0] != 0 or baseline[-1][0] != 2**64 - 1
            or [row[3] for row in baseline if row[0] == 3] != [list(b"crash-baseline-first")]):
        raise ValueError("server restart C1 baseline identities/value differ")
    parse_result(record["result"], expected_pid=record["restore_pid"], ready=ready, expected_uuid=record["source_uuid"])
    parse_restore_result(record["confirmation_result"], producer, expected_pid=record["confirmation_pid"])
    if (not isinstance(record["limitations"], list) or not record["limitations"]
            or any(type(value) is not str or not value.strip() for value in record["limitations"])):
        raise ValueError("server restart limitations are missing")
    hashes = record["tracked_files"]
    if not isinstance(hashes, dict) or set(hashes) != tracked_files():
        raise ValueError("server restart tracked source set differs")
    for path, digest in hashes.items():
        if not exact(digest, reference.digest_file(reference.ROOT / path)):
            raise ValueError("server restart evidence is stale: " + path)


def validate():
    record = json.loads((reference.ROOT / "compatibility/evidence/mysql-server-restart.json").read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    validate_record(record)
    print("Owned MySQL pre-COMMIT server restart evidence matches current sources; general crash recovery is not verified")


if __name__ == "__main__":
    validate()
