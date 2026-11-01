import copy
import json
import unittest
from unittest.mock import patch

import check
import database_smoke
import check_mariadb_snapshot_bootstrap_evidence as evidence


def valid_result():
    return {**evidence.FIXED, "anchor_position": 1000, "bootstrap_position": 2000,
            "strict_position": 3000, "process_pid": 1234, "sql_disconnect_resets": {}}


def wire(result):
    return evidence.PREFIX + json.dumps(result) + "\n"


def valid_record():
    return {"format_version": 1, "scope": evidence.SCOPE,
            "generated_at_utc": "2026-10-27T00:00:00+00:00",
            "server_version": "11.8.9-MariaDB-log", "transport": "loopback_plaintext_test_only",
            "process_pid": 1234, "result": wire(valid_result()),
            "tracked_files": {"source.rs": "current"}, "limitations": ["Finite fixture only."]}


class MariaSnapshotBootstrapEvidenceTests(unittest.TestCase):
    def test_exact_observations_and_owned_pid(self):
        self.assertEqual(evidence.parse_result(wire(valid_result()), expected_pid=1234), valid_result())
        for key in evidence.FIXED:
            changed = valid_result()
            changed[key] = 1 if type(changed[key]) is bool else None
            with self.subTest(key=key), self.assertRaises(ValueError):
                evidence.parse_result(wire(changed), expected_pid=1234)
        for actual, expected in ((True, 1), (1234, True), (999, 1234), (0, 0)):
            changed = valid_result(); changed["process_pid"] = actual
            with self.subTest(actual=actual, expected=expected), self.assertRaises(ValueError):
                evidence.parse_result(wire(changed), expected_pid=expected)

    def test_uint_position_order_and_cleanup_are_strict(self):
        for key in ("anchor_position", "bootstrap_position", "strict_position"):
            for value in (True, 4, -1, 2**64, 1000.0, "2000"):
                changed = valid_result(); changed[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    evidence.parse_result(wire(changed), expected_pid=1234)
        for positions in ((1000, 1000, 3000), (2000, 1000, 3000), (1000, 3000, 2000)):
            changed = valid_result()
            changed.update(zip(("anchor_position", "bootstrap_position", "strict_position"), positions))
            with self.assertRaises(ValueError):
                evidence.parse_result(wire(changed), expected_pid=1234)
        for resets in ({"control_final": True}, {"unknown": 1}, {"maria_bootstrap_reader": 2}, []):
            changed = valid_result(); changed["sql_disconnect_resets"] = resets
            with self.assertRaises(ValueError):
                evidence.parse_result(wire(changed), expected_pid=1234)
        changed = valid_result(); changed["sql_disconnect_resets"] = {"control_final": 1, "maria_bootstrap_reader": 1}
        evidence.parse_result(wire(changed), expected_pid=1234)

    def test_bounded_single_line_duplicate_and_unknown_fields(self):
        good = wire(valid_result())
        missing = valid_result(); del missing["shared_pool_released"]
        extra = {**valid_result(), "production_safe": True}
        duplicate = good.replace('"process_pid": 1234', '"process_pid": 1234, "process_pid": 1234')
        nested_duplicate = good.replace('"sql_disconnect_resets": {}', '"sql_disconnect_resets": {"control_final":1,"control_final":1}')
        for candidate in (good.strip(), good + "\n", good.replace("\n", "\r\n"), "prefix" + good,
                          wire(missing), wire(extra), duplicate, nested_duplicate,
                          evidence.PREFIX + "[]\n", evidence.PREFIX + " " * 65536 + "\n"):
            with self.subTest(candidate=candidate[:90]), self.assertRaises(ValueError):
                evidence.parse_result(candidate, expected_pid=1234)

    def test_record_profile_utc_and_native_pid_correlation(self):
        with patch.object(evidence, "tracked_files", return_value={"source.rs"}), patch.object(
                evidence.reference, "digest_file", return_value="current"):
            evidence.validate_record(valid_record())
            for key, value in (("format_version", True), ("scope", "production"),
                               ("server_version", "11.8.9-MariaDB"), ("transport", "verified_CA_TLS"),
                               ("process_pid", 999), ("process_pid", True), ("limitations", []),
                               ("generated_at_utc", True), ("generated_at_utc", "2026-10-27"),
                               ("generated_at_utc", "2026-10-27T09:00:00+09:00")):
                changed = valid_record(); changed[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    evidence.validate_record(changed)
            for changed in ({**valid_record(), "unknown": 1}, {key: value for key, value in valid_record().items() if key != "transport"}):
                with self.assertRaises(ValueError):
                    evidence.validate_record(changed)

    def test_missing_extra_and_stale_sources_refused(self):
        with patch.object(evidence, "tracked_files", return_value={"source.rs"}), patch.object(
                evidence.reference, "digest_file", return_value="current"):
            for hashes in ({}, {"source.rs": "old"}, {"source.rs": "current", "extra.rs": "current"}, {"other.rs": "current"}):
                changed = copy.deepcopy(valid_record()); changed["tracked_files"] = hashes
                with self.subTest(hashes=hashes), self.assertRaises(ValueError):
                    evidence.validate_record(changed)
        paths = evidence.tracked_files()
        self.assertIn("scripts/check_mariadb_snapshot_bootstrap_evidence.py", paths)
        self.assertIn("scripts/database_smoke.py", paths)
        self.assertIn("compatibility/schema/reference.json", paths)
        self.assertIn("crates/recastpress-core/src/resource_budget.rs", paths)
        self.assertIn("experiments/binlog-probe/src/mariadb_snapshot_bootstrap.rs", paths)

    def test_mode_rejection_precedes_any_process_or_fixture_work(self):
        with patch.object(database_smoke.subprocess, "run", side_effect=AssertionError("must not spawn")):
            for backend, cleanup, binlog, gtid in (("mysql", False, False, False),
                                                 ("mariadb", True, False, False),
                                                 ("mariadb", False, True, False),
                                                 ("mariadb", False, False, True),
                                                 ("mysql", False, False, True)):
                with self.subTest(backend=backend, cleanup=cleanup, binlog=binlog, gtid=gtid), self.assertRaises(ValueError):
                    database_smoke.main(False, backend, cleanup, binlog, gtid, True)

    def test_check_flag_runs_four_independent_fixtures_and_isolated_gate(self):
        with patch("sys.argv", ["check.py", "--with-binlog-probes", "--with-mysql-overlap", "--with-mariadb-snapshot-bootstrap"]), patch.object(check, "run") as run:
            check.main()
        commands = [call.args[0] for call in run.call_args_list]
        fixtures = [command[2:] for command in commands if len(command) > 1 and command[1] == "scripts/database_smoke.py"]
        self.assertEqual(fixtures, [["--backend", "mariadb", "--binlog-probe"],
                                    ["--backend", "mysql", "--binlog-probe"],
                                    ["--backend", "mysql", "--gtid-bootstrap-probe"],
                                    ["--backend", "mariadb", "--mariadb-snapshot-bootstrap-probe"]])
        self.assertEqual(sum(command[:4] == ["cargo", "test", "--manifest-path", "experiments/binlog-probe/Cargo.toml"] for command in commands), 1)
        self.assertTrue(any(command[1:] == ["scripts/check_mariadb_snapshot_bootstrap_evidence.py"] for command in commands))
        with patch("sys.argv", ["check.py", "--with-mariadb-snapshot-bootstrap"]), patch.object(check, "run") as run:
            check.main()
        self.assertTrue(any(call.args[0][:4] == ["cargo", "test", "--manifest-path", "experiments/binlog-probe/Cargo.toml"] for call in run.call_args_list))


if __name__ == "__main__":
    unittest.main()
