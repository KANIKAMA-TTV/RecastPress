"""Verify finite overlap evidence provenance without starting a database."""

import json
from datetime import datetime, timedelta

import wordpress_reference as reference
from binlog_probe_result import exact, unique_object
from check_database_evidence import required_files
from mysql_overlap_result import parse_overlap_result


def tracked_files():
    return {*required_files(), "scripts/setup_mysql.py", "scripts/database_smoke.py",
            "scripts/check_mysql_rotation_handoff_evidence.py",
            "scripts/mysql_overlap_result.py", "scripts/check_mysql_overlap_evidence.py", "scripts/owned_probe.py",
            "scripts/check_mariadb_snapshot_bootstrap_evidence.py",
            "scripts/binlog_probe_result.py", "scripts/crash_probe_result.py",
            "experiments/binlog-probe/Cargo.toml", "experiments/binlog-probe/Cargo.lock",
            *[path.relative_to(reference.ROOT).as_posix()
              for path in (reference.ROOT / "experiments/binlog-probe/src").glob("*.rs")]}


def validate():
    record = json.loads((reference.ROOT / "compatibility/evidence/mysql-overlap-bootstrap.json").read_text(
        encoding="utf-8"), object_pairs_hook=unique_object)
    expected_fields = {"format_version", "scope", "generated_at_utc", "server_version", "transport",
                       "process_pid", "result", "tracked_files", "limitations"}
    if not isinstance(record, dict) or set(record) != expected_fields:
        raise ValueError("overlap evidence fields are incomplete or unknown")
    for key, value in {"format_version": 1, "scope": "candidate_overlapping_mysql_bootstrap_fixture_only",
                       "server_version": "8.4.13", "transport": "ca_and_name_verified_tls"}.items():
        if not exact(record[key], value):
            raise ValueError("overlap evidence profile differs: " + key)
    timestamp = record["generated_at_utc"]
    if not isinstance(timestamp, str):
        raise ValueError("overlap evidence timestamp is not a UTC datetime")
    try:
        parsed = datetime.fromisoformat(timestamp)
    except ValueError as error:
        raise ValueError("overlap evidence timestamp is not a UTC datetime") from error
    if "T" not in timestamp or parsed.utcoffset() != timedelta(0):
        raise ValueError("overlap evidence timestamp is not a UTC datetime")
    if (not isinstance(record["limitations"], list) or not record["limitations"]
            or any(not isinstance(value, str) or not value.strip() for value in record["limitations"])):
        raise ValueError("overlap experiment limitations are missing")
    parse_overlap_result(record["result"], expected_pid=record["process_pid"])
    hashes = record["tracked_files"]
    if not isinstance(hashes, dict) or set(hashes) != tracked_files():
        raise ValueError("overlap evidence tracked source set differs")
    for path, digest in hashes.items():
        if not exact(digest, reference.digest_file(reference.ROOT / path)):
            raise ValueError("overlap evidence is stale: " + path)
    print("MySQL overlap evidence matches current experiment; production bootstrap is not verified")


if __name__ == "__main__":
    validate()
