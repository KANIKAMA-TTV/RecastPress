"""Strictly decode the disposable probe's JSON result, not a production IPC API."""

import json
import re

EXPECTED = {
    "format_version": 1, "rows": 3, "commits": 3, "mutations": [1, 1, 1],
    "corruption_rejected": True, "buffer_commits": 3, "retry_preserved": True,
    "partial_rejected": True, "gap_rejected": True, "durable_batches": 3,
    "durable_replays": 1, "conflict_rejected": True, "reconnected": True,
    "wrong_context_rejected": True, "missing_log_rejected": True,
    "missing_log_error_code": 1236, "ddl_rejected": True,
    "checkpoint_preserved_after_missing_log": True,
    "typed_rows": 5,
    "postmeta_source_resumed":True,
    "postmeta_rollback_preserved":True, "postmeta_context_rejected":True,
    "postmeta_durable_commits":3, "postmeta_durable_replays":1, "postmeta_ack_loss_preserved":True,
    "postmeta_conflict_rejected":True, "postmeta_projection_reopened":True,
    "postmeta_changes":5, "postmeta_memory_commits":3, "postmeta_projected_matches":True,
    "postmeta_old_view_preserved":True, "postmeta_apply_refusal_preserved":True,
    "postmeta_saved_rows":11,
    "postmeta_logical_limit_preserved":True, "postmeta_limited_commits":3,
    "postmeta_byte_limited_indexes":3,
    "postmeta_shared_pool_commits":3, "postmeta_shared_pool_refusal_preserved":True,
    "postmeta_shared_pool_released":True,
}
VARIABLE = {"filtered_postmeta", "maria_snapshot_anchor", "mysql_snapshot_race", "probe_pid", "postmeta_source_file", "postmeta_saved_position", "sql_disconnect_resets","events", "checksums", "derived_positions", "ddl_prefix_rejected", "missing_log_driver_fatal", "event_types", "typed_unsigned_reinterpreted"}


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate binlog result field: {key}")
        result[key] = value
    return result


def exact(value, expected):
    if type(value) is not type(expected):
        return False
    if isinstance(expected, list):
        return len(value) == len(expected) and all(exact(left, right) for left, right in zip(value, expected))
    if isinstance(expected, dict):
        return set(value) == set(expected) and all(exact(value[key], expected[key]) for key in expected)
    return value == expected


def validate_snapshot_observations(result, backend):
    """Check finite backend experiments, without promoting a production protocol."""
    maria = backend == "mariadb"
    active = "maria_snapshot_anchor" if maria else "mysql_snapshot_race"
    inactive = "mysql_snapshot_race" if maria else "maria_snapshot_anchor"
    if result[inactive] is not None:
        raise ValueError("snapshot observation belongs to the wrong backend")
    observation = result[active]
    fixed = ({"profile": "mariadb_11_8_9", "stable_during_transaction": True,
              "old_row_visible_after_commit": True, "readonly_write_rejected": True,
              "readonly_error_code": 1792} if maria else
             {"profile": "mysql_8_4_13", "old_row_visible_after_commit": True,
              "new_row_visible_to_writer": True, "reported_position_includes_unseen_commit": True,
              "readonly_write_rejected": True, "readonly_error_code": 1792})
    positions = ("position", "global_after") if maria else ("before_position", "reported_position")
    if not isinstance(observation, dict) or set(observation) != set(fixed) | {"source_file", *positions}:
        raise ValueError("snapshot observation fields are incomplete or unknown")
    if any(not exact(observation[key], expected) for key, expected in fixed.items()):
        raise ValueError("snapshot experiment did not record required observations")
    if not isinstance(observation["source_file"], str) or not re.fullmatch(r"probe-bin\.[0-9]{6}", observation["source_file"]):
        raise ValueError("snapshot source file differs from fixture naming")
    if any(type(observation[key]) is not int or not 4 < observation[key] < 2**64 for key in positions):
        raise ValueError("snapshot positions must be bounded u64 integers")
    if observation[positions[0]] >= observation[positions[1]]:
        raise ValueError("snapshot experiment did not observe a later commit position")


def validate_filtered_observation(result, backend):
    value = result["filtered_postmeta"]
    fixed = {"format_version":1,"profile":"fixture_same_db_filtered_postmeta", "rows":11,
             "commits":3,"selected_changes":3,"ignored_changes":2,
             "mixed_commit_verified":True,"ignored_only_commit_verified":True,
             "apply_failure_preserved":True,"ack_loss_replayed":True,"receipt_includes_ignored":True,
             "ignored_crc_rejected":True,"ignored_gap_rejected":True,"ignored_ddl_rejected":True,
             "ignored_ddl_prefix_rejected":backend=="mariadb", "wrong_filter_rejected":True,
             "row_data_matches":True,"index_matches":True,"cross_database_live_verified":False}
    if not isinstance(value, dict) or set(value) != set(fixed) | {"source_file","start_position","end_position"} or any(not exact(value[key], expected) for key, expected in fixed.items()):
        raise ValueError("filtered transaction fixture observations differ")
    if not exact(value["source_file"], result["postmeta_source_file"]) or any(type(value[key]) is not int or not 4 < value[key] < 2**64 for key in ["start_position","end_position"]):
        raise ValueError("filtered transaction source boundary differs")
    if not result["postmeta_saved_position"] < value["start_position"] < value["end_position"]:
        raise ValueError("filtered source did not advance after the original projection")


def parse_result(wire, backend):
    if backend not in ["mysql", "mariadb"]:
        raise ValueError("unknown binlog fixture backend")
    if not isinstance(wire, str):
        raise ValueError("binlog result must be text")
    wire = wire.strip()
    prefix = "BINLOG_PROBE_RESULT="
    if len(wire) > 65536 or not wire.startswith(prefix) or "\n" in wire or "\r" in wire:
        raise ValueError("binlog probe must emit one bounded JSON result line")
    result = json.loads(wire[len(prefix):], object_pairs_hook=unique_object)
    if not isinstance(result, dict) or set(result) != set(EXPECTED) | VARIABLE:
        raise ValueError("binlog result fields are incomplete or unknown")
    if any(not exact(result[key], value) for key, value in EXPECTED.items()):
        raise ValueError("binlog fixture did not record its required observations")
    validate_snapshot_observations(result, backend)
    if type(result["probe_pid"]) is not int or result["probe_pid"] <= 0:
        raise ValueError("producer process identity missing")
    if not isinstance(result["postmeta_source_file"], str) or not re.fullmatch(r"probe-bin\.[0-9]{6}", result["postmeta_source_file"]):
        raise ValueError("saved projection source file differs from fixture naming")
    if type(result["postmeta_saved_position"]) is not int or not 4 < result["postmeta_saved_position"] < 2**64:
        raise ValueError("saved projection position is invalid")
    validate_filtered_observation(result, backend)
    for key in ["events", "checksums", "derived_positions"]:
        if type(result[key]) is not int or result[key] < 0:
            raise ValueError("binlog counters must be nonnegative integers")
    if not 0 < result["checksums"] <= result["events"]:
        raise ValueError("binlog checksum/event counts are inconsistent")
    if not exact(result["ddl_prefix_rejected"], backend == "mariadb") or (result["derived_positions"] > 0) != (backend == "mariadb"):
        raise ValueError("binlog backend profile was not exercised")
    if type(result["missing_log_driver_fatal"]) is not bool:
        raise ValueError("driver error classification must be recorded as a boolean")
    if type(result["typed_unsigned_reinterpreted"]) is not int or not 0 <= result["typed_unsigned_reinterpreted"] <= 2:
        raise ValueError("unsigned representation observations exceed the typed corpus")
    resets = result["sql_disconnect_resets"]
    allowed_resets = {"original_journal_old", "postmeta_journal_old", "postmeta_rollback_observer", "control_final", "filtered_postmeta_journal",
                      "maria_anchor_reader" if backend == "mariadb" else "mysql_race_reader"}
    if not isinstance(resets, dict) or not set(resets).issubset(allowed_resets) or any(type(count) is not int or count <= 0 for count in resets.values()):
        raise ValueError("SQL disconnect observations are malformed")
    kinds = result["event_types"]
    if not isinstance(kinds, dict) or not kinds:
        raise ValueError("binlog event types are missing")
    for key, count in kinds.items():
        if not key.isascii() or not key.isdecimal() or not 0 <= int(key) <= 255 or str(int(key)) != key or type(count) is not int or count <= 0:
            raise ValueError("binlog event type/count is malformed")
    if sum(kinds.values()) != result["events"] or any(kinds.get(key) != count for key, count in {"4": 2, "15": 2, "16": 3}.items()):
        raise ValueError("binlog event totals or reconnect/commit counts differ")
    return result


def parse_restore_result(wire, producer, *, expected_pid=None):
    """Validate a distinct process against the producer's saved boundary."""
    if expected_pid is not None and (type(expected_pid) is not int or expected_pid <= 0):
        raise ValueError("expected restore PID is not a positive integer")
    if not isinstance(wire, str):
        raise ValueError("restore result must be text")
    prefix = "POSTMETA_RESTORE_RESULT="
    wire = wire.strip()
    if len(wire) > 65536 or not wire.startswith(prefix) or "\n" in wire or "\r" in wire:
        raise ValueError("restore must emit one bounded JSON result line")
    restored = json.loads(wire[len(prefix):], object_pairs_hook=unique_object)
    required = {"format_version": 1, "source_file": producer["postmeta_source_file"],
                "position": producer["postmeta_saved_position"], "rows": producer["postmeta_saved_rows"],
                "row_data_matches": True, "index_matches": True, "receipt_validated": True, "store_unchanged": True}
    if not isinstance(restored, dict) or set(restored) != set(required) | {"process_pid", "sql_disconnect_resets"}:
        raise ValueError("restore result fields are incomplete or unknown")
    if any(not exact(restored[key], value) for key, value in required.items()):
        raise ValueError("restored data/boundary observations differ from producer")
    pid = restored["process_pid"]
    if type(pid) is not int or pid <= 0 or pid == producer["probe_pid"] or (expected_pid is not None and pid != expected_pid):
        raise ValueError("restore was not the expected distinct process")
    resets = restored["sql_disconnect_resets"]
    if not isinstance(resets, dict) or not set(resets).issubset({"control_final"}) or any(type(count) is not int or count != 1 for count in resets.values()):
        raise ValueError("restore SQL cleanup observations are invalid")
    return restored


RESTORE_REJECTIONS = {
    "wrong_file": ("fresh-process postmeta restore", "saved postmeta context, row allowance or coordinates differ"),
    "wrong_position": ("fresh-process postmeta restore", "saved postmeta context, row allowance or coordinates differ"),
    "wrong_rows": ("fresh-process postmeta restore", "saved postmeta row count differs"),
    "missing_fixture_flag": ("fixture validation", "requires the owned disposable DB harness"),
    "unknown_argument": ("fixture validation", "unknown or extra probe arguments"),
}


def validate_restore_rejection(record):
    if not isinstance(record, dict) or set(record) != {"case", "process_pid", "exit_code", "stderr"}:
        raise ValueError("restore rejection fields incomplete")
    if not isinstance(record["case"], str) or record["case"] not in RESTORE_REJECTIONS or type(record["process_pid"]) is not int or record["process_pid"] <= 0 or type(record["exit_code"]) is not int or record["exit_code"] != 1:
        raise ValueError("restore rejection identity/exit differs")
    stage, reason = RESTORE_REJECTIONS[record["case"]]
    if record["stderr"] != f"Binlog capability probe failed in {stage}: {reason}":
        raise ValueError("restore refusal did not have its required reason")


def validate_process_restore(record, producer):
    required = {"result", "producer_pid", "restore_pid", "producer_exited_before_restore", "database_stayed_running",
                "rejections", "confirmation_result", "confirmation_pid"}
    if not isinstance(record, dict) or set(record) != required:
        raise ValueError("fresh-process restore evidence identities incomplete")
    if type(record["producer_pid"]) is not int or record["producer_pid"] != producer["probe_pid"] or record["producer_exited_before_restore"] is not True or record["database_stayed_running"] is not True:
        raise ValueError("fresh-process restore lifetime observations differ")
    for field in ("restore_pid", "confirmation_pid"):
        if type(record[field]) is not int or record[field] <= 0:
            raise ValueError("fresh-process restore/confirmation PID missing")
    parse_restore_result(record["result"], producer, expected_pid=record["restore_pid"])
    rejected = record["rejections"]
    if not isinstance(rejected, list) or len(rejected) != len(RESTORE_REJECTIONS):
        raise ValueError("restore refusal observations incomplete")
    for item in rejected:
        validate_restore_rejection(item)
    if {item["case"] for item in rejected} != set(RESTORE_REJECTIONS):
        raise ValueError("restore refusal observations duplicated or substituted")
    parse_restore_result(record["confirmation_result"], producer, expected_pid=record["confirmation_pid"])
