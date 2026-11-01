"""Check finite MySQL rotation-handoff evidence; never opens a database."""

import json
from datetime import datetime, timedelta

import wordpress_reference as reference
from binlog_probe_result import exact, unique_object
from check_database_evidence import required_files

PREFIX = "MYSQL_ROTATION_HANDOFF_RESULT="
FIXED = {
    "format_version": 1, "profile": "mysql_8_4_13_unpublished_rotation_handoff",
    "source_files": ["probe-bin.000001", "probe-bin.000002"],
    "rows": 11, "commits": 3, "handoffs": 1, "real_rotate_events": 1,
    "artificial_rotate_events": 1,
    "real_fde_events": 1, "shared_pool_commits": 3, "bridge_reconnects": 1,
    "rotation_event_count_scope": "successful_bridge_only",
    **{key: True for key in ("row_data_matches", "index_matches", "outer_checkpoint_preserved",
        "corrupt_rotate_rejected", "corrupt_fde_rejected", "unknown_file_rejected",
        "missing_header_rejected", "open_transaction_rejected", "apply_refusal_preserved",
        "budget_refusal_preserved", "driver_table_map_cleared", "shared_pool_released",
        "ordinary_rotation_rejected", "unpublished_only", "owned_dump_terminated",
        "retry_checkpoint_preserved", "retry_preamble_verified", "same_owned_server_verified")},
}
VARIABLE = {"start_position", "first_commit_position", "old_file_size", "new_header_position",
            "second_commit_position", "strict_position", "process_pid", "nonce", "sql_disconnect_resets"}

SCOPE = "candidate_mysql_rotation_handoff_fixture_only"


def parse_result(wire, *, expected_pid, expected_nonce):
    if (not isinstance(wire, str) or len(wire.encode("utf-8")) > 65536
            or not wire.startswith(PREFIX) or not wire.endswith("\n")
            or "\n" in wire[:-1] or "\r" in wire):
        raise ValueError("MySQL rotation handoff result must be one bounded protocol line")
    try:
        result = json.loads(wire[len(PREFIX):-1], object_pairs_hook=unique_object)
    except (json.JSONDecodeError, ValueError) as error:
        raise ValueError("invalid MySQL rotation handoff JSON") from error
    if not isinstance(result, dict) or set(result) != set(FIXED) | VARIABLE:
        raise ValueError("MySQL rotation handoff result fields are incomplete or unknown")
    if any(not exact(result[key], value) for key, value in FIXED.items()):
        raise ValueError("required MySQL rotation handoff observations did not pass")
    if (type(expected_pid) is not int or expected_pid <= 0
            or type(result["process_pid"]) is not int or result["process_pid"] != expected_pid):
        raise ValueError("MySQL rotation handoff PID differs from the owned child")
    if (type(expected_nonce) is not str or len(expected_nonce) != 32
            or any(char not in "0123456789abcdef" for char in expected_nonce)
            or not exact(result["nonce"], expected_nonce)):
        raise ValueError("rotation handoff nonce differs from the owned attempt")
    for key in ("start_position", "first_commit_position", "old_file_size", "new_header_position",
                "second_commit_position", "strict_position"):
        if type(result[key]) is not int or not 0 < result[key] <= 2**64 - 1:
            raise ValueError("invalid rotation handoff position")
    if not (4 < result["start_position"] < result["first_commit_position"] < result["old_file_size"]
            and 4 < result["new_header_position"] < result["second_commit_position"] < result["strict_position"]):
        raise ValueError("rotation positions do not advance within each file")
    resets = result["sql_disconnect_resets"]
    if (not isinstance(resets, dict)
            or set(resets) - {"control_final"}
            or any(type(value) is not int or value != 1 for value in resets.values())):
        raise ValueError("unexpected MySQL rotation handoff cleanup observation")
    return result


def tracked_files():
    return {*required_files(), "scripts/setup_mysql.py", "scripts/database_smoke.py",
            "scripts/mysql_overlap_result.py", "scripts/check_mysql_overlap_evidence.py",
            "scripts/check_mysql_rotation_handoff_evidence.py", "scripts/owned_probe.py",
            "scripts/binlog_probe_result.py", "scripts/crash_probe_result.py", "scripts/check.py",
            "scripts/test_mysql_rotation_handoff_evidence.py", "scripts/check_mariadb_snapshot_bootstrap_evidence.py",
            "experiments/binlog-probe/Cargo.toml", "experiments/binlog-probe/Cargo.lock",
            *[path.relative_to(reference.ROOT).as_posix()
              for path in (reference.ROOT / "experiments/binlog-probe/src").glob("*.rs")]}


def validate_record(record):
    expected_fields = {"format_version", "scope", "generated_at_utc", "server_version", "transport",
                       "process_pid", "nonce", "result", "tracked_files", "limitations"}
    if not isinstance(record, dict) or set(record) != expected_fields:
        raise ValueError("MySQL rotation handoff evidence fields are incomplete or unknown")
    for key, value in {"format_version": 1, "scope": SCOPE,
                       "server_version": "8.4.13",
                       "transport": "ca_and_name_verified_tls"}.items():
        if not exact(record[key], value):
            raise ValueError("MySQL rotation handoff evidence profile differs: " + key)
    timestamp = record["generated_at_utc"]
    if not isinstance(timestamp, str):
        raise ValueError("MySQL rotation handoff evidence timestamp is not a UTC datetime")
    try:
        parsed = datetime.fromisoformat(timestamp)
    except ValueError as error:
        raise ValueError("MySQL rotation handoff evidence timestamp is not a UTC datetime") from error
    if "T" not in timestamp or parsed.utcoffset() != timedelta(0):
        raise ValueError("MySQL rotation handoff evidence timestamp is not a UTC datetime")
    if (not isinstance(record["limitations"], list) or not record["limitations"]
            or any(not isinstance(value, str) or not value.strip() for value in record["limitations"])):
        raise ValueError("MySQL rotation handoff limitations are missing")
    parse_result(record["result"], expected_pid=record["process_pid"], expected_nonce=record["nonce"])
    hashes = record["tracked_files"]
    if not isinstance(hashes, dict) or set(hashes) != tracked_files():
        raise ValueError("MySQL rotation handoff tracked source set differs")
    for path, digest in hashes.items():
        if not exact(digest, reference.digest_file(reference.ROOT / path)):
            raise ValueError("MySQL rotation handoff evidence is stale: " + path)


def validate():
    record = json.loads((reference.ROOT / "compatibility/evidence/mysql-rotation-handoff.json").read_text(
        encoding="utf-8"), object_pairs_hook=unique_object)
    validate_record(record)
    print("MySQL rotation handoff evidence matches current experiment; production rotation is not verified")


if __name__ == "__main__":
    validate()
