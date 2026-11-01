import copy
import json
import unittest
from unittest.mock import patch

import check
import database_smoke
import check_binlog_evidence
import check_mysql_rotation_handoff_evidence as evidence

NONCE = "0123456789abcdef0123456789abcdef"


def valid_result():
    return {**evidence.FIXED, "start_position": 1000, "first_commit_position": 2000,
            "old_file_size": 2040, "new_header_position": 155,
            "second_commit_position": 300, "strict_position": 450,
            "process_pid": 1234, "nonce": NONCE, "sql_disconnect_resets": {}}


def wire(result):
    return evidence.PREFIX + json.dumps(result) + "\n"


def parse(result):
    return evidence.parse_result(wire(result), expected_pid=1234, expected_nonce=NONCE)


def valid_record():
    return {"format_version": 1, "scope": evidence.SCOPE,
            "generated_at_utc": "2026-10-27T00:00:00+00:00", "server_version": "8.4.13",
            "transport": "ca_and_name_verified_tls", "process_pid": 1234, "nonce": NONCE,
            "result": wire(valid_result()), "tracked_files": {"source.rs": "current"},
            "limitations": ["Finite RAM handoff only."]}


class RotationHandoffEvidenceTests(unittest.TestCase):
    def test_exact_observations_and_owned_nonce_pid_are_required(self):
        self.assertEqual(parse(valid_result()), valid_result())
        self.assertEqual(len(valid_result()), 39)
        for key in ("artificial_rotate_events", "bridge_reconnects"):
            for count in (0, 2, True, 1.0):
                changed = valid_result(); changed[key] = count
                with self.subTest(key=key, count=count), self.assertRaises(ValueError):
                    parse(changed)
        for scope in ("all_attempts", True, None):
            changed = valid_result(); changed["rotation_event_count_scope"] = scope
            with self.subTest(scope=scope), self.assertRaises(ValueError):
                parse(changed)
        for key in evidence.FIXED:
            changed = valid_result()
            changed[key] = 1 if type(changed[key]) is bool else None
            with self.subTest(key=key), self.assertRaises(ValueError):
                parse(changed)
        for pid in (True, 1234.0, 0, 1235):
            changed = valid_result(); changed["process_pid"] = pid
            with self.subTest(pid=pid), self.assertRaises(ValueError):
                parse(changed)
        for nonce in (None, True, NONCE.upper(), NONCE[:-1], "f" * 32):
            changed = valid_result(); changed["nonce"] = nonce
            with self.subTest(nonce=nonce), self.assertRaises(ValueError):
                parse(changed)
        for expected_pid, expected_nonce in ((True, NONCE), (1234, None), (1234, "x" * 32)):
            with self.assertRaises(ValueError):
                evidence.parse_result(wire(valid_result()), expected_pid=expected_pid, expected_nonce=expected_nonce)

    def test_positions_are_unsigned_strictly_ordered_within_each_file_only(self):
        # The newer file's offsets may be numerically smaller than the old file.
        parse(valid_result())
        positions = ("start_position", "first_commit_position", "old_file_size",
                     "new_header_position", "second_commit_position", "strict_position")
        for key in positions:
            for value in (True, -1, 0, 2**64, 300.0, "300"):
                changed = valid_result(); changed[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    parse(changed)
        for key, value in (("start_position", 4), ("first_commit_position", 1000),
                           ("old_file_size", 2000), ("new_header_position", 4),
                           ("second_commit_position", 155), ("strict_position", 300)):
            changed = valid_result(); changed[key] = value
            with self.assertRaises(ValueError):
                parse(changed)

    def test_reset_labels_cannot_mask_other_sql_failures(self):
        changed = valid_result(); changed["sql_disconnect_resets"] = {"control_final": 1}
        parse(changed)
        for value in ({"control_final": True}, {"control_final": 1.0}, {"control_final": 2},
                      {"maria_bootstrap_reader": 1}, {"rotation_stream": 1}, []):
            changed = valid_result(); changed["sql_disconnect_resets"] = value
            with self.assertRaises(ValueError):
                parse(changed)

    def test_protocol_refuses_duplicates_extra_missing_fields_and_multiple_lines(self):
        good = wire(valid_result())
        missing = valid_result(); del missing["driver_table_map_cleared"]
        duplicate = good.replace('"process_pid": 1234', '"process_pid": 1234, "process_pid": 1234')
        nested = good.replace('"sql_disconnect_resets": {}', '"sql_disconnect_resets": {"control_final":1,"control_final":1}')
        for candidate in (good.strip(), good + "\n", good.replace("\n", "\r\n"), "x" + good,
                          duplicate, nested, wire(missing), wire({**valid_result(), "production_safe": True}),
                          evidence.PREFIX + "[]\n", evidence.PREFIX + " " * 65536 + "\n"):
            with self.assertRaises(ValueError):
                evidence.parse_result(candidate, expected_pid=1234, expected_nonce=NONCE)

    def test_record_profile_pid_nonce_and_current_exact_source_set_are_correlated(self):
        with patch.object(evidence, "tracked_files", return_value={"source.rs"}), patch.object(
                evidence.reference, "digest_file", return_value="current"):
            evidence.validate_record(valid_record())
            for key, value in (("format_version", True), ("scope", "production"),
                               ("server_version", "8.4.12"), ("transport", "plaintext"),
                               ("process_pid", 1235), ("nonce", "f" * 32), ("limitations", []),
                               ("generated_at_utc", "2026-10-27"), ("generated_at_utc", "2026-10-27T09:00:00+09:00")):
                changed = valid_record(); changed[key] = value
                with self.subTest(key=key), self.assertRaises(ValueError):
                    evidence.validate_record(changed)
            for hashes in ({}, {"source.rs": "old"}, {"source.rs": "current", "extra.rs": "current"}):
                changed = copy.deepcopy(valid_record()); changed["tracked_files"] = hashes
                with self.assertRaises(ValueError):
                    evidence.validate_record(changed)
            changed = valid_record(); del changed["nonce"]
            with self.assertRaises(ValueError):
                evidence.validate_record(changed)

    def test_wrong_backend_or_conflicting_mode_is_refused_before_any_process(self):
        with patch.object(database_smoke.subprocess, "run", side_effect=AssertionError("must not spawn")), patch.object(
                database_smoke.subprocess, "Popen", side_effect=AssertionError("must not start server")):
            for backend, cleanup, normal, gtid, maria in (("mariadb", False, False, False, False),
                    ("mysql", True, False, False, False), ("mysql", False, True, False, False),
                    ("mysql", False, False, True, False), ("mysql", False, False, False, True)):
                with self.assertRaises(ValueError):
                    database_smoke.main(False, backend, cleanup, normal, gtid, maria, True)

    def test_combined_gate_runs_five_independent_fixtures_and_checks_new_provenance(self):
        with patch("sys.argv", ["check.py", "--with-binlog-probes", "--with-mysql-overlap",
                                "--with-mariadb-snapshot-bootstrap", "--with-mysql-rotation-handoff"]), patch.object(check, "run") as run:
            check.main()
        commands = [call.args[0] for call in run.call_args_list]
        fixtures = [command[2:] for command in commands if len(command) > 1 and command[1] == "scripts/database_smoke.py"]
        self.assertEqual(fixtures, [["--backend", "mariadb", "--binlog-probe"],
            ["--backend", "mysql", "--binlog-probe"], ["--backend", "mysql", "--gtid-bootstrap-probe"],
            ["--backend", "mariadb", "--mariadb-snapshot-bootstrap-probe"],
            ["--backend", "mysql", "--mysql-rotation-handoff-probe"]])
        self.assertTrue(any(command[1:] == ["scripts/check_mysql_rotation_handoff_evidence.py"] for command in commands))
        self.assertEqual(sum(command[:4] == ["cargo", "test", "--manifest-path", "experiments/binlog-probe/Cargo.toml"] for command in commands), 1)
        with patch("sys.argv", ["check.py", "--with-mysql-rotation-handoff"]), patch.object(check, "run") as run:
            check.main()
        self.assertTrue(any(call.args[0][1:] == ["scripts/database_smoke.py", "--backend", "mysql", "--mysql-rotation-handoff-probe"] for call in run.call_args_list))

    def test_new_native_and_checker_are_tracked_by_new_and_existing_binlog_provenance(self):
        for path in ("experiments/binlog-probe/src/mysql_rotation_handoff.rs",
                     "scripts/check_mysql_rotation_handoff_evidence.py"):
            self.assertIn(path, evidence.tracked_files())
            self.assertIn(path, check_binlog_evidence.FILES)
        self.assertIn("scripts/check.py", evidence.tracked_files())


if __name__ == "__main__":
    unittest.main()
