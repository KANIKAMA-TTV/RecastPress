import copy
import json
import unittest
from unittest.mock import patch

import check_mysql_server_restart_evidence as evidence
from test_binlog_probe_result import result as producer_value, wire as producer_wire
from test_crash_probe_result import ready_value, wire as ready_wire, payload, NONCE, EPOCH

UUID = "12345678-1234-1234-1234-123456789abc"


def ready():
    value = ready_value()
    rows = json.loads(bytes.fromhex(value["old_tuple"]["row_data_hex"]))
    rows[3][3] = list(b"crash-baseline-first")
    value["old_tuple"]["row_data_hex"] = payload(rows)
    return value


def result():
    value = ready()
    return {**evidence.FIXED, "process_pid": 300, "nonce": NONCE, "prepared_pid": 200,
        "source_uuid": UUID, "source_file": value["source_file"], "baseline_position": value["baseline_position"],
        "candidate_position": value["target_position"], "sql_disconnect_resets": {}}


def wire(value):
    return evidence.PREFIX + json.dumps(value, separators=(",", ":")) + "\n"


def parse(value):
    return evidence.parse_result(wire(value), expected_pid=300, ready=ready(), expected_uuid=UUID)


def restore(pid):
    return "POSTMETA_RESTORE_RESULT=" + json.dumps({"format_version": 1, "process_pid": pid,
        "source_file": "probe-bin.000001", "position": 9000, "rows": 11, "row_data_matches": True,
        "index_matches": True, "receipt_validated": True, "store_unchanged": True, "sql_disconnect_resets": {}}) + "\n"


def record():
    return {"format_version": 1, "scope": evidence.SCOPE, "generated_at_utc": "2026-10-27T00:00:00+00:00",
        "server_version": "8.4.13", "transport": "ca_and_name_verified_tls", "nonce": NONCE,
        "epoch_hex": EPOCH.hex(), "source_uuid": UUID, "producer_pid": 100,
        "producer_result": producer_wire(producer_value()) + "\n", "before_restore_pid": 150,
        "before_restore_result": restore(150), "ready_pid": 200, "ready": ready_wire(ready()),
        "lifecycle": {**evidence.LIFECYCLE_FIXED, "first_server_pid": 500, "restarted_server_pid": 600,
            "first_exit_code": 1, "pending_exit_code": 1}, "restore_pid": 300, "result": wire(result()),
        "confirmation_pid": 400, "confirmation_result": restore(400), "tracked_files": {"source.rs": "current"},
        "limitations": ["One owned pre-COMMIT server-process restart, no binlog replay or OS/power-crash guarantee."]}


def validate(value):
    with patch.object(evidence, "tracked_files", return_value={"source.rs"}), patch.object(
            evidence.reference, "digest_file", return_value="current"):
        evidence.validate_record(value)


class ServerRestartEvidenceTests(unittest.TestCase):
    def test_exact_result_and_complete_owned_lifecycle_correlations_are_accepted(self):
        self.assertEqual(len(result()), 20)
        self.assertEqual(len(record()["lifecycle"]), 22)
        self.assertEqual(parse(result()), result())
        value = result(); value["sql_disconnect_resets"] = {"control_final": 1}; parse(value)
        validate(record())

    def test_native_flags_coordinates_uuid_and_actual_child_pid_are_strict(self):
        for key in evidence.FIXED:
            value = result(); value[key] = 1 if type(value[key]) is bool else None
            with self.subTest(key=key), self.assertRaises(ValueError): parse(value)
        for key, wrong in [("process_pid", True), ("process_pid", 301), ("prepared_pid", 201),
            ("baseline_position", True), ("baseline_position", 11000.0), ("candidate_position", 11000),
            ("candidate_position", 2**64), ("source_uuid", UUID.upper()),
            ("source_file", "probe-bin.000002"), ("nonce", "00" * 16), ("rows", 11.0)]:
            value = result(); value[key] = wrong
            with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError): parse(value)
        for pid, uuid in [(True, UUID), (200, UUID), (300, UUID.replace("-", "")), (300, UUID.upper())]:
            with self.assertRaises(ValueError): evidence.parse_result(wire(result()), expected_pid=pid, ready=ready(), expected_uuid=uuid)

    def test_result_wire_rejects_duplicates_unknown_fields_multiline_and_unscoped_cleanup(self):
        good = wire(result()); missing = result(); del missing["old_tuple_matches"]
        duplicate = good.replace('"process_pid":300', '"process_pid":300,"process_pid":300')
        for candidate in [good.rstrip("\n"), good + "\n", good.replace("\n", "\r\n"), "x" + good,
            duplicate, wire(missing), wire({**result(), "source_resumed": True}), evidence.PREFIX + "[]\n",
            evidence.PREFIX + " " * 65536 + "\n"]:
            with self.subTest(candidate=candidate[:60]), self.assertRaises(ValueError):
                evidence.parse_result(candidate, expected_pid=300, ready=ready(), expected_uuid=UUID)
        for cleanup in [{"control_final": True}, {"control_final": 1.0}, {"control_final": 2},
                        {"crash_resume_journal": 1}, {"server_shutdown": 1}, []]:
            value = result(); value["sql_disconnect_resets"] = cleanup
            with self.assertRaises(ValueError): parse(value)

    def test_every_lifecycle_flag_count_and_abrupt_exit_is_required(self):
        for key in evidence.LIFECYCLE_FIXED:
            for wrong in [None, False, 1.0]:
                value = record(); value["lifecycle"][key] = wrong
                with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError): validate(value)
        for key in ["first_exit_code", "pending_exit_code"]:
            for wrong in [0, True, 1.0, -(2**31)-1, 2**32, None]:
                value = record(); value["lifecycle"][key] = wrong
                with self.assertRaises(ValueError): validate(value)
        for key, wrong in [("first_server_pid", 100), ("restarted_server_pid", 500), ("first_server_pid", True)]:
            value = record(); value["lifecycle"][key] = wrong
            with self.assertRaises(ValueError): validate(value)
        for mutation in ["missing", "extra"]:
            value = record()
            if mutation == "missing": del value["lifecycle"]["first_exit_observed"]
            else: value["lifecycle"]["power_crash_verified"] = True
            with self.assertRaises(ValueError): validate(value)

    def test_record_profiles_raw_process_identity_and_normal_restore_cannot_be_substituted(self):
        for key, wrong in [("format_version", True), ("server_version", "11.8.9-MariaDB-log"),
            ("transport", "loopback_plaintext_test_only"), ("scope", "production"),
            ("generated_at_utc", "2026-10-27"), ("generated_at_utc", "2026-10-27T00:00:00+09:00"),
            ("epoch_hex", b"mysql-not-owned".hex()), ("nonce", "ab" * 15), ("source_uuid", UUID.upper()),
            ("producer_pid", 101), ("ready_pid", 201), ("before_restore_pid", 200),
            ("restore_pid", 301), ("confirmation_pid", 401), ("limitations", []), ("producer_result", None)]:
            value = record(); value[key] = wrong
            with self.subTest(key=key), self.assertRaises(ValueError): validate(value)
        for key in ["producer_result", "before_restore_result", "ready", "result", "confirmation_result"]:
            for mutation in ["missing_newline", "extra_blank", "extra_prefix", "crlf"]:
                value = record()
                if mutation == "missing_newline": value[key] = value[key].rstrip("\n")
                elif mutation == "extra_blank": value[key] += "\n"
                elif mutation == "extra_prefix": value[key] = "\n" + value[key]
                else: value[key] = value[key].replace("\n", "\r\n")
                with self.subTest(key=key, mutation=mutation), self.assertRaises(ValueError): validate(value)
        for key in ["before_restore_result", "confirmation_result"]:
            value = record(); native = json.loads(value[key].split("=", 1)[1]); native["position"] += 1
            value[key] = "POSTMETA_RESTORE_RESULT=" + json.dumps(native) + "\n"
            with self.assertRaises(ValueError): validate(value)

    def test_ready_old_tuple_and_c1_source_split_remain_bound_to_the_validated_marker(self):
        for mutate in ["receipt", "baseline", "target", "first_id", "last_id", "old_value", "duplicate"]:
            value = record(); marker = ready()
            if mutate == "receipt": marker["old_tuple"]["receipt_hex"] = payload([])
            elif mutate == "baseline": marker["baseline_position"] += 1
            elif mutate == "target": marker["target_position"] = marker["baseline_position"]
            elif mutate == "duplicate":
                value["ready"] = value["ready"].replace('"rows":11', '"rows":11,"rows":11')
            else:
                rows = json.loads(bytes.fromhex(marker["old_tuple"]["row_data_hex"]))
                if mutate == "first_id": rows[0][0] = 1
                elif mutate == "last_id": rows[-1][0] -= 1
                else: rows[3][3] = list(b"crash-committed-second")
                marker["old_tuple"]["row_data_hex"] = payload(rows)
            if mutate != "duplicate": value["ready"] = ready_wire(marker)
            with self.subTest(mutate=mutate), self.assertRaises(ValueError): validate(value)

    def test_source_hash_set_is_exact_and_current_without_a_cached_reference_tree(self):
        value = record(); value["tracked_files"] = {"other.rs": "current"}
        with self.assertRaises(ValueError): validate(value)
        value = record(); value["tracked_files"]["source.rs"] = "stale"
        with self.assertRaises(ValueError): validate(value)
        needed = {"scripts/mysql_server_restart.py", "scripts/check_mysql_server_restart_evidence.py",
            "scripts/test_mysql_server_restart_evidence.py", "scripts/test_owned_server_crash.py",
            "scripts/test_mysql_server_restart_lifecycle.py", "experiments/binlog-probe/src/postmeta_process_crash.rs",
            "experiments/binlog-probe/src/main.rs", "scripts/owned_probe.py", "scripts/database_smoke.py", "scripts/check.py"}
        self.assertLessEqual(needed, evidence.tracked_files())


if __name__ == "__main__": unittest.main()
