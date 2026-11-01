"""Decode the finite unpublished MySQL overlap experiment, not production IPC."""

import json
import re

from binlog_probe_result import exact, unique_object

PREFIX = "MYSQL_OVERLAP_BOOTSTRAP_RESULT="
FIXED = {
    "format_version": 1,
    "profile": "mysql_8_4_13_unpublished_overlap_bootstrap",
    "source_file": "probe-bin.000001", "start_position": 4,
    "rows": 12, "snapshot_rows": 12, "bootstrap_commits": 3, "strict_commits": 1,
    "full_stream_contiguous": True, "snapshot_overlap_verified": True,
    "pk_move_aba_verified": True, "rows_match": True, "index_matches": True,
    "strict_seam_verified": True, "unpublished_only": True,
}
VARIABLE = {"process_pid", "a_gtid_set", "floor_position", "c_position",
            "strict_position", "prefix_groups_skipped", "sql_disconnect_resets"}


def parse_overlap_result(wire, *, expected_pid):
    if (not isinstance(wire, str) or len(wire.encode("utf-8")) > 65536
            or not wire.startswith(PREFIX) or not wire.endswith("\n")
            or "\n" in wire[:-1] or "\r" in wire):
        raise ValueError("overlap result must be one bounded protocol line")
    try:
        result = json.loads(wire[len(PREFIX):-1], object_pairs_hook=unique_object)
    except (json.JSONDecodeError, ValueError) as error:
        raise ValueError("invalid overlap JSON") from error
    if not isinstance(result, dict) or set(result) != set(FIXED) | VARIABLE:
        raise ValueError("overlap result fields are incomplete or unknown")
    if any(not exact(result[key], value) for key, value in FIXED.items()):
        raise ValueError("required overlap observations did not pass")
    if (type(expected_pid) is not int or expected_pid <= 0
            or type(result["process_pid"]) is not int or result["process_pid"] != expected_pid):
        raise ValueError("overlap PID differs from the owned child")
    for key in ("floor_position", "c_position", "strict_position", "prefix_groups_skipped"):
        if type(result[key]) is not int or not 0 < result[key] <= 2**64 - 1:
            raise ValueError("invalid overlap integer observation")
    if not 4 < result["floor_position"] < result["c_position"] < result["strict_position"]:
        raise ValueError("overlap positions do not advance across the seam")
    gtid_set = result["a_gtid_set"]
    if (not isinstance(gtid_set, str) or len(gtid_set) > 8192 or not re.fullmatch(
            r"[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}(?::[1-9][0-9]*(?:-[1-9][0-9]*)?)+", gtid_set)):
        raise ValueError("overlap GTID set is outside the single-origin profile")
    previous = 0
    cardinality = 0
    for interval in gtid_set.split(":")[1:]:
        bounds = [int(value) for value in interval.split("-")]
        lower, upper = bounds[0], bounds[-1]
        if not previous < lower <= upper < 2**63:
            raise ValueError("overlap GTID intervals are invalid or unordered")
        previous = upper
        cardinality += upper - lower + 1
    if result["prefix_groups_skipped"] > cardinality:
        raise ValueError("skipped prefix exceeds the executed GTID set")
    resets = result["sql_disconnect_resets"]
    if (not isinstance(resets, dict)
            or set(resets) - {"control_final", "overlap_snapshot_reader"}
            or any(type(value) is not int or value != 1 for value in resets.values())):
        raise ValueError("unexpected overlap cleanup observation")
    return result
