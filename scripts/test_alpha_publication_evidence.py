from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import check_alpha_publication_evidence as evidence


HASH = "a" * 64
ROOT = Path(__file__).resolve().parents[1]


def phase(label):
    return {
        "prepare_pid": 30, "receipt_pid": 31, "payload_bytes": 600, "body_bytes": 400,
        "payload_sha256": ("a" if label == "a" else "b") * 64,
        "body_sha256": ("c" if label == "a" else "d") * 64,
        "public_pid": 50, "etag": '"' + ("e" if label == "a" else "f") * 64 + '"',
        "get_head_conditional": True, "exit_code": 0,
        **{key: True for key in evidence.PHASE_FLAGS[label]},
    }


def record(backend="mysql"):
    return {
        "format_version": 1, "scope": evidence.SCOPE,
        "generated_at_utc": "2026-10-29T01:02:03+00:00", "backend": backend,
        "server_version": "8.4.13" if backend == "mysql" else "11.8.9-MariaDB-log",
        "database_pid": 10, "contract_pid": 20,
        "db_checks": sorted(evidence.REQUIRED_CHECKS), "phases": {"a": phase("a"), "b": phase("b")},
        "rejected_candidate_pid": 60, "rejected_candidate_exit_code": 1,
        **{key: True for key in evidence.TOP_FLAGS},
        "tracked_files": {"source.rs": HASH},
        "limitations": ["Finite owned fixture only; a checksum is not authorization or a live publication protocol."],
    }


def validate(value):
    with patch.object(evidence, "tracked_files", return_value={"source.rs"}), patch.object(
            evidence.reference, "digest_file", return_value=HASH):
        evidence.validate_record(value)


class AlphaPublicationEvidenceTests(unittest.TestCase):
    def test_exact_records_accept_supported_backends_graceful_exits_and_sequential_pid_reuse(self):
        self.assertEqual(len(record()), 18)
        self.assertEqual(len(record()["db_checks"]), 62)
        validate(record())
        validate(record("mariadb"))
        value = record("mariadb")
        value["server_version"] = "11.8.9-MariaDB"
        value["generated_at_utc"] = "2026-10-29T01:02:03Z"
        # Contract/prepare/receipt processes exited before either public process.
        # A exits before B starts; permanent uniqueness must not be assumed.
        value["contract_pid"] = 50
        for label in ("a", "b"):
            value["phases"][label].update(prepare_pid=50, receipt_pid=50, public_pid=50, exit_code=-15)
        validate(value)
        value["phases"]["b"]["exit_code"] = 2**32 - 1
        validate(value)

    def test_top_shape_flags_backend_checks_and_timestamp_are_strict(self):
        for key in evidence.TOP_FIELDS:
            value = record(); del value[key]
            with self.subTest(missing=key), self.assertRaises(ValueError): validate(value)
        for wrong in (None, [], {**record(), "unexpected": True}):
            with self.subTest(wrong=type(wrong)), self.assertRaises(ValueError): validate(wrong)
        for key in evidence.TOP_FLAGS:
            for wrong in (False, 0, 1, None, "true"):
                value = record(); value[key] = wrong
                with self.subTest(flag=key, wrong=wrong), self.assertRaises(ValueError): validate(value)
        for key, wrong in (("format_version", True), ("format_version", 1.0), ("scope", None),
            ("scope", "production"), ("backend", []), ("backend", "MYSQL"),
            ("server_version", "11.8.9-MariaDB-log"), ("server_version", "8.4.13-debug"),
            ("generated_at_utc", None), ("generated_at_utc", "2026-10-29"),
            ("generated_at_utc", "2026-10-29T01:02:03"), ("generated_at_utc", "2026-10-29T01:02:03+09:00")):
            value = record(); value[key] = wrong
            with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError): validate(value)
        for wrong in (None, [], ["other"] * 62, sorted(evidence.REQUIRED_CHECKS)[:-1],
                      [None] * 62, list(reversed(sorted(evidence.REQUIRED_CHECKS)))):
            value = record(); value["db_checks"] = wrong
            with self.subTest(checks=wrong), self.assertRaises(ValueError): validate(value)

    def test_phase_schema_typed_flags_sizes_and_exit_ranges_are_required(self):
        for wrong in (None, [], {"a": phase("a")}, {"a": phase("a"), "b": phase("b"), "c": phase("b")}):
            value = record(); value["phases"] = wrong
            with self.subTest(phases=wrong), self.assertRaises(ValueError): validate(value)
        for label in ("a", "b"):
            for key in phase(label):
                value = record(); del value["phases"][label][key]
                with self.subTest(phase=label, missing=key), self.assertRaises(ValueError): validate(value)
            value = record(); value["phases"][label]["other"] = True
            with self.assertRaises(ValueError): validate(value)
            for key in evidence.PHASE_FLAGS[label] | {"get_head_conditional"}:
                value = record(); value["phases"][label][key] = 1
                with self.subTest(phase=label, flag=key), self.assertRaises(ValueError): validate(value)
            for key, wrong in (("payload_bytes", True), ("payload_bytes", 0),
                ("payload_bytes", evidence.MAX_BUNDLE_BYTES + 1), ("body_bytes", 0),
                ("body_bytes", 600), ("body_bytes", 400.0), ("exit_code", None),
                ("exit_code", True), ("exit_code", -(2**31)-1), ("exit_code", 2**32)):
                value = record(); value["phases"][label][key] = wrong
                with self.subTest(phase=label, key=key, wrong=wrong), self.assertRaises(ValueError): validate(value)

    def test_canonical_digests_strong_tags_and_changed_a_b_are_required(self):
        for label in ("a", "b"):
            for key in ("payload_sha256", "body_sha256"):
                for wrong in (None, "A" * 64, "a" * 63, "g" * 64, 0):
                    value = record(); value["phases"][label][key] = wrong
                    with self.subTest(label=label, key=key, wrong=wrong), self.assertRaises(ValueError): validate(value)
            for wrong in (None, "e" * 64, 'W/"' + "e" * 64 + '"', '"' + "E" * 64 + '"', '"x"'):
                value = record(); value["phases"][label]["etag"] = wrong
                with self.subTest(label=label, tag=wrong), self.assertRaises(ValueError): validate(value)
        for key in ("payload_sha256", "body_sha256", "etag"):
            value = record(); value["phases"]["b"][key] = value["phases"]["a"][key]
            with self.subTest(unchanged=key), self.assertRaises(ValueError): validate(value)

    def test_only_simultaneously_alive_processes_are_required_to_have_distinct_pids(self):
        for key in ("database_pid", "contract_pid", "rejected_candidate_pid"):
            for wrong in (None, True, 0, -1, 2**32):
                value = record(); value[key] = wrong
                with self.subTest(key=key, wrong=wrong), self.assertRaises(ValueError): validate(value)
        value = record(); value["contract_pid"] = value["database_pid"]
        with self.assertRaises(ValueError): validate(value)
        for label in ("a", "b"):
            for key in ("prepare_pid", "receipt_pid", "public_pid"):
                for wrong in (True, 0, value["database_pid"]):
                    value = record(); value["phases"][label][key] = wrong
                    with self.subTest(label=label, key=key, wrong=wrong), self.assertRaises(ValueError): validate(value)
        value = record(); value["rejected_candidate_pid"] = value["phases"]["a"]["public_pid"]
        with self.assertRaises(ValueError): validate(value)
        for wrong in (None, True, 0, -2147483649, 2**32):
            value = record(); value["rejected_candidate_exit_code"] = wrong
            with self.subTest(exit=wrong), self.assertRaises(ValueError): validate(value)

    def test_current_source_identity_and_nonempty_limits_cannot_be_omitted_or_forged(self):
        for wrong in (None, [], [""], ["  "], [None]):
            value = record(); value["limitations"] = wrong
            with self.subTest(limits=wrong), self.assertRaises(ValueError): validate(value)
        for wrong in (None, {}, {"other.rs": HASH}, {"source.rs": None},
                      {"source.rs": "A" * 64}, {"source.rs": "b" * 64}):
            value = record(); value["tracked_files"] = wrong
            with self.subTest(hashes=wrong), self.assertRaises(ValueError): validate(value)
        value = record(); value["tracked_files"] = {"../SECRET": HASH}
        with patch.object(evidence, "tracked_files", return_value={"source.rs"}), patch.object(
                evidence.reference, "digest_file") as digest:
            with self.assertRaises(ValueError): evidence.validate_record(value)
            digest.assert_not_called()
        needed = {"scripts/alpha_publication_path.py", "scripts/smoke.py", "scripts/owned_probe.py",
            "scripts/check.py", "scripts/check_alpha_publication_evidence.py", "scripts/test_alpha_publication_evidence.py",
            "scripts/test_publication_cli.py", "crates/recastpress-server/Cargo.toml",
            "crates/recastpress-server/src/prepared_posts.rs", "crates/recastpress-server/src/bundle_receipt.rs",
            "crates/recastpress-server/src/main.rs", "crates/recastpress-server/tests/prepared_posts_contract.rs"}
        self.assertLessEqual(needed, evidence.tracked_files())

    def test_cli_checks_both_files_and_rejects_duplicate_or_wrong_backend_json(self):
        with tempfile.TemporaryDirectory(prefix="alpha-evidence-", dir=ROOT/"target") as directory:
            temp = Path(directory)
            target = temp/"compatibility/evidence"
            target.mkdir(parents=True)
            for backend in ("mysql", "mariadb"):
                (target/f"alpha-publication-{backend}.json").write_text(json.dumps(record(backend)), encoding="utf-8")
            with patch.object(evidence.reference, "ROOT", temp), patch.object(
                    evidence, "tracked_files", return_value={"source.rs"}), patch.object(
                    evidence.reference, "digest_file", return_value=HASH):
                with redirect_stdout(io.StringIO()) as output:
                    evidence.validate()
                self.assertIn("both DB backends", output.getvalue())
                path = target/"alpha-publication-mariadb.json"
                wrong = record("mysql")
                path.write_text(json.dumps(wrong), encoding="utf-8")
                with self.assertRaises(ValueError): evidence.validate()
                wire = json.dumps(record("mariadb"))
                for invalid in [wire.replace('"database_pid": 10', '"database_pid": 10, "database_pid": 10'),
                    wire.replace('"body_bytes": 400', '"body_bytes": 400, "body_bytes": 400', 1),
                    "null", " " * (evidence.MAX_EVIDENCE_BYTES + 1)]:
                    path.write_text(invalid, encoding="utf-8")
                    with self.subTest(wire=invalid[:45]), self.assertRaises(ValueError): evidence.validate()


if __name__ == "__main__":
    unittest.main()
