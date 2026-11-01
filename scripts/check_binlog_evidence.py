"""Check the isolated candidate experiment's evidence; never connects to a DB."""

import json
import wordpress_reference as reference
from binlog_probe_result import exact, parse_result, validate_process_restore, unique_object
from crash_probe_result import validate_crash_recovery
from check_database_evidence import required_files

FILES = {
    "experiments/binlog-probe/src/mysql_rotation_handoff.rs",
    "scripts/check_mysql_rotation_handoff_evidence.py",
    "experiments/binlog-probe/src/mariadb_snapshot_bootstrap.rs",
    "scripts/check_mariadb_snapshot_bootstrap_evidence.py",
    "experiments/binlog-probe/src/mysql_overlap_bootstrap.rs",
    "scripts/mysql_overlap_result.py", "scripts/check_mysql_overlap_evidence.py",
    "experiments/binlog-probe/src/filtered_postmeta.rs",
    "experiments/binlog-probe/src/postmeta_process_crash.rs",
    "scripts/crash_probe_result.py", "scripts/owned_probe.py",
    "experiments/binlog-probe/src/snapshot_anchor_probe.rs",
    "experiments/binlog-probe/src/mysql_snapshot_race.rs",
    "experiments/binlog-probe/src/fresh_restore.rs",
    "experiments/binlog-probe/src/postmeta_journal.rs",
    "experiments/binlog-probe/src/postmeta_probe.rs",
    "crates/recastpress-db/src/meta_view.rs",
    "experiments/binlog-probe/Cargo.toml", "experiments/binlog-probe/Cargo.lock",
    "experiments/binlog-probe/src/main.rs", "scripts/database_smoke.py",
    "experiments/binlog-probe/src/buffer_adapter.rs", "crates/recastpress-db/src/change_buffer.rs",
    "experiments/binlog-probe/src/durable_journal.rs",
    "experiments/binlog-probe/src/ddl_probe.rs",
    "experiments/binlog-probe/src/missing_log_probe.rs", "scripts/binlog_probe_result.py",
    "experiments/binlog-probe/src/row_types_probe.rs",
    "crates/recastpress-db/src/lib.rs", "crates/recastpress-db/Cargo.toml", "Cargo.toml",
    "crates/recastpress-db/src/taxonomy.rs", "crates/recastpress-db/src/comments.rs", "crates/recastpress-db/src/users.rs", "crates/recastpress-db/src/links.rs",
}


FILES |= required_files()

FIXED_NUMERIC_OBSERVATIONS = {
    "format_version": 1,
    "commits": 3,
    "committed_row_changes": 3,
    "buffer_applied_batches": 3,
    "fixture_durable_batches": 3,
    "fixture_duplicate_receipts": 1,
    "missing_log_error_code": 1236,
    "postmeta_durable_commits": 3,
    "postmeta_durable_replays": 1,
    "postmeta_changes": 5,
    "postmeta_memory_commits": 3,
    "postmeta_limited_commits": 3,
    "postmeta_byte_limited_indexes": 3,
    "postmeta_shared_pool_commits": 3,
    "typed_rows_verified": 5,
}


def validate_server_profile(record, backend):
    """Bind finite observations to the exact server and fixture transport."""
    profiles = {
        "mariadb": {"server_version": "11.8.9-MariaDB-log", "transport": "loopback_plaintext_test_only"},
        "mysql": {"server_version": "8.4.13", "transport": "ca_and_name_verified_tls"},
    }
    if backend not in profiles or not isinstance(record, dict):
        raise ValueError("unknown binlog evidence server profile")
    for field, expected in profiles[backend].items():
        if not exact(record.get(field), expected):
            raise ValueError(f"binlog evidence server profile differs: {field}")


def validate_numeric_observations(record):
    """Validate metadata counters without reading evidence files or references."""
    if not isinstance(record, dict):
        raise ValueError("binlog numeric observations must be a mapping")
    for field, expected in FIXED_NUMERIC_OBSERVATIONS.items():
        if not exact(record.get(field), expected):
            raise ValueError(f"binlog numeric observation is missing or differs in type/value: {field}")
    for field, minimum, maximum in [
        ("event_count", 1, None),
        ("verified_checksums", 1, None),
        ("derived_interior_positions", 0, None),
        ("typed_unsigned_reinterpreted", 0, 2),
    ]:
        value = record.get(field)
        if type(value) is not int or value < minimum or (maximum is not None and value > maximum):
            raise ValueError(f"binlog numeric observation is missing or outside its integer bounds: {field}")
    if record["verified_checksums"] > record["event_count"]:
        raise ValueError("binlog checksum count exceeds its event count")


def validate():
    for backend in ["mariadb", "mysql"]:
        record = json.loads((reference.ROOT / f"compatibility/evidence/binlog-probe-{backend}.json").read_text(encoding="utf-8"), object_pairs_hook=unique_object)
        if type(record["format_version"]) is not int or record["format_version"] != 1 or record["scope"] != "candidate_nonblocking_binlog_fixture_only" or not record["limitations"]:
            raise ValueError("binlog experiment scope/limitations missing")
        validate_server_profile(record, backend)
        validate_numeric_observations(record)
        observed = parse_result(record["result"], backend)
        restore = record["fresh_process_restore"]
        validate_process_restore(restore, observed)
        validate_crash_recovery(record["process_crash_recovery"], observed, backend)
        for field, source in {
            "filtered_postmeta":"filtered_postmeta",
            "maria_snapshot_anchor":"maria_snapshot_anchor",
            "mysql_snapshot_race":"mysql_snapshot_race",
            "sql_disconnect_resets":"sql_disconnect_resets",
            "postmeta_source_resumed":"postmeta_source_resumed",
            "postmeta_rollback_preserved":"postmeta_rollback_preserved", "postmeta_context_rejected":"postmeta_context_rejected",
            "postmeta_durable_commits":"postmeta_durable_commits", "postmeta_durable_replays":"postmeta_durable_replays",
            "postmeta_ack_loss_preserved":"postmeta_ack_loss_preserved", "postmeta_conflict_rejected":"postmeta_conflict_rejected", "postmeta_projection_reopened":"postmeta_projection_reopened",
            "postmeta_changes":"postmeta_changes", "postmeta_memory_commits":"postmeta_memory_commits",
            "postmeta_projected_matches":"postmeta_projected_matches", "postmeta_old_view_preserved":"postmeta_old_view_preserved",
            "postmeta_apply_refusal_preserved":"postmeta_apply_refusal_preserved",
            "postmeta_logical_limit_preserved":"postmeta_logical_limit_preserved",
            "postmeta_limited_commits":"postmeta_limited_commits",
            "postmeta_byte_limited_indexes":"postmeta_byte_limited_indexes",
            "postmeta_shared_pool_commits":"postmeta_shared_pool_commits",
            "postmeta_shared_pool_refusal_preserved":"postmeta_shared_pool_refusal_preserved",
            "postmeta_shared_pool_released":"postmeta_shared_pool_released",
            "event_count": "events", "verified_checksums": "checksums",
            "derived_interior_positions": "derived_positions", "ddl_prefix_rejected": "ddl_prefix_rejected",
            "missing_log_rejected": "missing_log_rejected", "missing_log_error_code": "missing_log_error_code",
            "missing_log_driver_fatal": "missing_log_driver_fatal",
            "checkpoint_preserved_after_missing_log": "checkpoint_preserved_after_missing_log",
            "typed_rows_verified": "typed_rows", "typed_unsigned_reinterpreted": "typed_unsigned_reinterpreted",
        }.items():
            if not exact(record[field], observed[source]):
                raise ValueError(f"recorded binlog observation differs from its result: {field}")
        if record["decoded_values_match"] is not True or record["rollback_row_absent"] is not True:
            raise ValueError("row values/rollback were not verified")
        if record["corrupt_checksum_rejected"] is not True:
            raise ValueError("corrupt event rejection was not verified")
        if record["apply_failure_preserved_batch"] is not True:
            raise ValueError("real-event transaction staging/retry was not verified")
        if record["incomplete_transaction_rejected"] is not True:
            raise ValueError("incomplete real-event transaction rejection was not verified")
        if record["missing_frame_rejected"] is not True:
            raise ValueError("missing real-event frame rejection was not verified")
        if record["ack_loss_after_commit"] is not True:
            raise ValueError("fixture atomic journal/retry after commit was not verified")
        if record["conflicting_replay_rejected"] is not True:
            raise ValueError("conflicting durable replay rejection was not verified")
        if record["actual_ddl_rejected"] is not True or record["ddl_rejection_fault"] != "SchemaChanged":
            raise ValueError("actual DDL was not rejected with a latched schema fault")
        if record["ddl_prefix_rejected"] is not (backend == "mariadb"):
            raise ValueError("DDL rejection point was not distinguished by backend")
        if any(record[field] is not True for field in ["source_reconnected_from_durable_checkpoint", "journal_connection_reopened", "wrong_checkpoint_context_rejected"]):
            raise ValueError("fixture reconnect/durable checkpoint validation was not verified")
        if (backend == "mariadb") != (record["derived_interior_positions"] > 0):
            raise ValueError("backend position profile was not exercised as recorded")
        if set(record["tracked_files"]) != FILES:
            raise ValueError("binlog experiment source identities incomplete")
        for name, expected in record["tracked_files"].items():
            if reference.digest_file(reference.ROOT / name) != expected:
                raise ValueError(f"binlog experiment evidence is stale: {name}; regenerate both probes")
    print("Binlog candidate evidence matches current experiment; production CDC is not verified")


if __name__ == "__main__":
    try:
        validate()
    except (OSError, ValueError, KeyError, TypeError) as error:
        raise SystemExit(f"Binlog evidence check failed: {error}")
