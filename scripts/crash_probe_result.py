"""Strict protocol for one owned, pre-COMMIT client-process kill experiment."""
import json
import re

from binlog_probe_result import exact, parse_restore_result, unique_object

READY_PREFIX = "POSTMETA_CRASH_READY="
RESULT_PREFIX = "POSTMETA_CRASH_RESULT="
PAYLOAD_LIMIT = 12 * 1024


def _uint(value, minimum=0):
    return type(value) is int and minimum <= value < 2**64


def _hex(value, limit):
    if type(value) is not str or len(value) % 2 or len(value) > limit * 2 or not re.fullmatch(r"[0-9a-f]*", value):
        raise ValueError("crash payload is not bounded lowercase hex")
    return bytes.fromhex(value)


def _line(wire, prefix, *, terminated=False, canonical=False):
    if type(wire) is not str or len(wire.encode("utf-8")) > 65536:
        raise ValueError("crash result is not bounded text")
    if terminated and not wire.endswith("\n"):
        raise ValueError("crash READY is not newline terminated")
    body = wire.removesuffix("\n").removesuffix("\r")
    if "\r" in body or "\n" in body or not body.startswith(prefix):
        raise ValueError("crash result must be one prefixed line")
    body = body[len(prefix):]
    value = json.loads(body, object_pairs_hook=unique_object)
    if not isinstance(value, dict):
        raise ValueError("crash result must be an object")
    if canonical and json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False) != body:
        raise ValueError("crash READY is not canonical JSON")
    return value


def _bytes(value):
    return isinstance(value, list) and all(type(byte) is int and 0 <= byte <= 255 for byte in value)


def parse_ready(wire, *, expected_pid, nonce, epoch, producer):
    if not _uint(expected_pid, 1) or type(nonce) is not str or not re.fullmatch(r"[0-9a-f]{32}", nonce) or type(epoch) is not bytes or not 0 < len(epoch) <= 64:
        raise ValueError("expected crash fixture identity is invalid")
    ready = _line(wire, READY_PREFIX, terminated=True, canonical=True)
    fields = {"format_version", "nonce", "process_pid", "source_connection_id", "source_file",
              "start_position", "baseline_position", "target_position", "rows", "epoch_hex", "old_tuple"}
    if set(ready) != fields or not exact(ready["format_version"], 1) or not exact(ready["rows"], 11):
        raise ValueError("crash READY shape/version/row count differs")
    if not _uint(ready["process_pid"], 1) or ready["process_pid"] != expected_pid or not _uint(ready["source_connection_id"], 1):
        raise ValueError("crash READY process/session identity differs")
    if ready["nonce"] != nonce or _hex(ready["epoch_hex"], 64) != epoch:
        raise ValueError("crash READY fixture nonce/epoch differs")
    if type(ready["source_file"]) is not str or not re.fullmatch(r"probe-bin\.[0-9]{6}", ready["source_file"]) or ready["source_file"] != producer["postmeta_source_file"]:
        raise ValueError("crash source file differs from the owned fixture")
    positions = [ready[key] for key in ("start_position", "baseline_position", "target_position")]
    if any(not _uint(position, 5) for position in positions) or not producer["postmeta_saved_position"] < positions[0] < positions[1] < positions[2]:
        raise ValueError("crash source positions are invalid or unordered")
    old = ready["old_tuple"]
    if not isinstance(old, dict) or set(old) != {"epoch_hex", "file_hex", "from_position", "through_position", "receipt_hex", "row_data_hex"}:
        raise ValueError("old crash tuple fields differ")
    if _hex(old["epoch_hex"], 64) != epoch or _hex(old["file_hex"], 255) != ready["source_file"].encode("ascii") or not exact(old["from_position"], positions[0]) or not exact(old["through_position"], positions[1]):
        raise ValueError("old crash tuple context/boundary differs")
    receipt = json.loads(_hex(old["receipt_hex"], PAYLOAD_LIMIT))
    if not isinstance(receipt, list) or len(receipt) != 1 or any(not _bytes(frame) or not frame for frame in receipt):
        raise ValueError("old crash receipt is empty or malformed")
    rows = json.loads(_hex(old["row_data_hex"], PAYLOAD_LIMIT))
    if not isinstance(rows, list) or len(rows) != 11:
        raise ValueError("old crash row count differs")
    previous = None
    for row in rows:
        if not isinstance(row, list) or len(row) != 4 or not _uint(row[0]) or not _uint(row[1]) or any(value is not None and not _bytes(value) for value in row[2:]):
            raise ValueError("old crash row shape differs")
        if previous is not None and row[0] <= previous:
            raise ValueError("old crash row identities are not unique/ordered")
        previous = row[0]
    return ready


def parse_crash_result(wire, ready, *, expected_pid):
    if not _uint(expected_pid, 1) or expected_pid == ready["process_pid"]:
        raise ValueError("crash resumer is not a distinct owned process")
    value = _line(wire, RESULT_PREFIX)
    required = {"format_version": 1, "profile": "fixture_postmeta_precommit_process_kill",
                "nonce": ready["nonce"], "original_pid": ready["process_pid"], "process_pid": expected_pid,
                "source_connection_id": ready["source_connection_id"], "source_file": ready["source_file"],
                "start_position": ready["start_position"], "baseline_position": ready["baseline_position"],
                "target_position": ready["target_position"], "rows": 11,
                "old_pair_preserved": True, "rollback_drained": True, "source_resumed": True,
                "row_data_matches": True, "index_matches": True, "receipt_validated": True,
                "resumed_commits": 1, "resumed_updates": 1}
    if set(value) != set(required) | {"sql_disconnect_resets"} or any(not exact(value[key], expected) for key, expected in required.items()):
        raise ValueError("crash resumer observations differ")
    resets = value["sql_disconnect_resets"]
    if not isinstance(resets, dict) or not set(resets).issubset({"control_final", "crash_resume_journal"}) or any(not exact(count, 1) for count in resets.values()):
        raise ValueError("crash SQL cleanup observations differ")
    return value


def validate_crash_recovery(record, producer, backend):
    fields = {"ready", "process_pid", "exit_code", "was_running_before_kill", "forced_kill",
              "database_stayed_running", "nonce", "epoch_hex", "result", "resume_pid",
              "confirmation_result", "confirmation_pid"}
    if backend not in ("mysql", "mariadb") or not isinstance(record, dict) or set(record) != fields:
        raise ValueError("crash recovery evidence fields differ")
    if any(record[field] is not True for field in ("was_running_before_kill", "forced_kill", "database_stayed_running")):
        raise ValueError("crash recovery process lifetime was not observed")
    if type(record["exit_code"]) is not int or not -(2**31) <= record["exit_code"] < 2**32 or record["exit_code"] == 0:
        raise ValueError("crash child did not record abnormal termination")
    epoch = _hex(record["epoch_hex"], 64)
    if not re.fullmatch(backend.encode("ascii") + rb"-[0-9a-f]{32}", epoch):
        raise ValueError("crash epoch is not an owned fixture UUID")
    ready = parse_ready(record["ready"], expected_pid=record["process_pid"], nonce=record["nonce"], epoch=epoch, producer=producer)
    parse_crash_result(record["result"], ready, expected_pid=record["resume_pid"])
    if not _uint(record["confirmation_pid"], 1):
        raise ValueError("post-crash normal restore PID is missing")
    parse_restore_result(record["confirmation_result"], producer, expected_pid=record["confirmation_pid"])
