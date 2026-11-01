import json
from pathlib import Path
import unittest
from unittest.mock import patch

from check_mysql_overlap_evidence import validate
from test_mysql_overlap_result import valid, wire


def record():
    return {"format_version": 1, "scope": "candidate_overlapping_mysql_bootstrap_fixture_only",
            "generated_at_utc": "2026-10-26T15:40:00+00:00", "server_version": "8.4.13",
            "transport": "ca_and_name_verified_tls", "process_pid": 1234,
            "result": wire(valid()), "tracked_files": {"fixture.rs": "a" * 64},
            "limitations": ["Finite unpublished fixture only."]}


def check(value):
    with patch.object(Path, "read_text", return_value=json.dumps(value)), \
            patch("check_mysql_overlap_evidence.tracked_files", return_value={"fixture.rs"}), \
            patch("check_mysql_overlap_evidence.reference.digest_file", return_value="a" * 64), \
            patch("builtins.print"):
        validate()


class OverlapEvidenceTests(unittest.TestCase):
    def test_timestamp_profile_and_child_correlation(self):
        check(record())
        for key, value in (("generated_at_utc", False), ("generated_at_utc", "2026-10-27"),
                           ("generated_at_utc", "2026-10-27T00:40:00+09:00"),
                           ("generated_at_utc", "invalid"), ("process_pid", 999),
                           ("format_version", True), ("transport", "plaintext"),
                           ("limitations", [])):
            changed = record(); changed[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                check(changed)

    def test_stale_missing_extra_source_and_unknown_fields_refused(self):
        for hashes in ({}, {"fixture.rs": "b" * 64},
                       {"fixture.rs": "a" * 64, "other.rs": "a" * 64}):
            changed = record(); changed["tracked_files"] = hashes
            with self.assertRaises(ValueError):
                check(changed)
        changed = record(); changed["unreviewed"] = True
        with self.assertRaises(ValueError):
            check(changed)


if __name__ == "__main__":
    unittest.main()
