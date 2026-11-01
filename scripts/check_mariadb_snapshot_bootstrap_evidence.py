"""Check finite MariaDB snapshot-bootstrap evidence; never opens a database."""

import json
from datetime import datetime, timedelta

import wordpress_reference as reference
from binlog_probe_result import exact, unique_object
from check_database_evidence import required_files

PREFIX = "MARIADB_SNAPSHOT_BOOTSTRAP_RESULT="
FIXED = {
    "format_version": 1,
    "profile": "mariadb_11_8_9_unpublished_snapshot_bootstrap",
    "source_file": "probe-bin.000001",
    "snapshot_rows": 11, "rows": 11, "bootstrap_commits": 3,
    "strict_commits": 1, "shared_pool_commits": 4,
    "snapshot_anchor_stable": True, "snapshot_rows_stable": True,
    "rows_match": True, "index_matches": True,
    "apply_refusal_preserved": True, "budget_refusal_preserved": True,
    "crc_refusal_preserved": True, "gap_refusal_preserved": True,
    "shared_pool_released": True, "unpublished_only": True,
}
VARIABLE = {"anchor_position", "bootstrap_position", "strict_position",
            "process_pid", "sql_disconnect_resets"}
SCOPE = "candidate_mariadb_snapshot_bootstrap_fixture_only"


def parse_result(wire, *, expected_pid):
    if (not isinstance(wire, str) or len(wire.encode("utf-8")) > 65536
            or not wire.startswith(PREFIX) or not wire.endswith("\n")
            or "\n" in wire[:-1] or "\r" in wire):
        raise ValueError("MariaDB bootstrap result must be one bounded protocol line")
    try:
        result = json.loads(wire[len(PREFIX):-1], object_pairs_hook=unique_object)
    except (json.JSONDecodeError, ValueError) as error:
        raise ValueError("invalid MariaDB bootstrap JSON") from error
    if not isinstance(result, dict) or set(result) != set(FIXED) | VARIABLE:
        raise ValueError("MariaDB bootstrap result fields are incomplete or unknown")
    if any(not exact(result[key], value) for key, value in FIXED.items()):
        raise ValueError("required MariaDB bootstrap observations did not pass")
    if (type(expected_pid) is not int or expected_pid <= 0
            or type(result["process_pid"]) is not int or result["process_pid"] != expected_pid):
        raise ValueError("MariaDB bootstrap PID differs from the owned child")
    for key in ("anchor_position", "bootstrap_position", "strict_position"):
        if type(result[key]) is not int or not 0 < result[key] <= 2**64 - 1:
            raise ValueError("invalid MariaDB bootstrap position")
    if not 4 < result["anchor_position"] < result["bootstrap_position"] < result["strict_position"]:
        raise ValueError("MariaDB bootstrap positions do not advance across the seam")
    resets = result["sql_disconnect_resets"]
    if (not isinstance(resets, dict)
            or set(resets) - {"control_final", "maria_bootstrap_reader"}
            or any(type(value) is not int or value != 1 for value in resets.values())):
        raise ValueError("unexpected MariaDB bootstrap cleanup observation")
    return result


def tracked_files():
    return {*required_files(), "scripts/setup_mysql.py", "scripts/database_smoke.py",
            "scripts/check_mysql_rotation_handoff_evidence.py",
            "scripts/mysql_overlap_result.py", "scripts/check_mysql_overlap_evidence.py",
            "scripts/check_mariadb_snapshot_bootstrap_evidence.py", "scripts/owned_probe.py",
            "scripts/binlog_probe_result.py", "scripts/crash_probe_result.py",
            "experiments/binlog-probe/Cargo.toml", "experiments/binlog-probe/Cargo.lock",
            *[path.relative_to(reference.ROOT).as_posix()
              for path in (reference.ROOT / "experiments/binlog-probe/src").glob("*.rs")]}


def validate_record(record):
    expected_fields = {"format_version", "scope", "generated_at_utc", "server_version", "transport",
                       "process_pid", "result", "tracked_files", "limitations"}
    if not isinstance(record, dict) or set(record) != expected_fields:
        raise ValueError("MariaDB bootstrap evidence fields are incomplete or unknown")
    for key, value in {"format_version": 1, "scope": SCOPE,
                       "server_version": "11.8.9-MariaDB-log",
                       "transport": "loopback_plaintext_test_only"}.items():
        if not exact(record[key], value):
            raise ValueError("MariaDB bootstrap evidence profile differs: " + key)
    timestamp = record["generated_at_utc"]
    if not isinstance(timestamp, str):
        raise ValueError("MariaDB bootstrap evidence timestamp is not a UTC datetime")
    try:
        parsed = datetime.fromisoformat(timestamp)
    except ValueError as error:
        raise ValueError("MariaDB bootstrap evidence timestamp is not a UTC datetime") from error
    if "T" not in timestamp or parsed.utcoffset() != timedelta(0):
        raise ValueError("MariaDB bootstrap evidence timestamp is not a UTC datetime")
    if (not isinstance(record["limitations"], list) or not record["limitations"]
            or any(not isinstance(value, str) or not value.strip() for value in record["limitations"])):
        raise ValueError("MariaDB bootstrap limitations are missing")
    parse_result(record["result"], expected_pid=record["process_pid"])
    hashes = record["tracked_files"]
    if not isinstance(hashes, dict) or set(hashes) != tracked_files():
        raise ValueError("MariaDB bootstrap tracked source set differs")
    for path, digest in hashes.items():
        if not exact(digest, reference.digest_file(reference.ROOT / path)):
            raise ValueError("MariaDB bootstrap evidence is stale: " + path)


def validate():
    record = json.loads((reference.ROOT / "compatibility/evidence/mariadb-snapshot-bootstrap.json").read_text(
        encoding="utf-8"), object_pairs_hook=unique_object)
    validate_record(record)
    print("MariaDB snapshot bootstrap evidence matches current experiment; production bootstrap is not verified")


if __name__ == "__main__":
    validate()
