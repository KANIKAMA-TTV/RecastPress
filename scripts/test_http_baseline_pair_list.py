"""Explicit offline pair-list contracts; no network, Rust, DB or nested worker."""
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import http_baseline_pair_list as pairs
import owned_probe
from owned_probe import reap_owned_probes, run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "scripts/compare_http_baseline_pairs.py"
BOOLS = {"body_equal", "status_equal", "headers_equal", "http_version_equal", "request_target_equal", "url_equal", "transport_equal", "tls_verification_equal"}


class HttpBaselinePairListTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="baseline-pairs-test-", dir=ROOT / "target")
        self.work = Path(self.directory.name)
        self.manifest = self.work / "manifests" / "PRIVATE_pairs.json"
        self.manifest.parent.mkdir()
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(reap_owned_probes)

    def baseline(self, name, body=b"PRIVATE_A\0\xff", **changes):
        directory = self.work / name
        directory.mkdir()
        value = {"format_version": 1, "scope": "manual_single_url_http_baseline", "url": "http://127.0.0.1:9/PRIVATE_URL?key=PRIVATE_QUERY",
                 "request_target": "/PRIVATE_URL?key=PRIVATE_QUERY", "method": "GET", "status": 200, "http_version": "HTTP/1.1",
                 "recorded_at_utc": "2026-10-29T00:00:00+00:00", "transport": "http", "tls_verification": "not_applicable",
                 "headers": {"content-length": [str(len(body))], "etag": ['"PRIVATE_ETAG"']},
                 "body_file": "body.bin", "body_bytes": len(body), "body_sha256": hashlib.sha256(body).hexdigest()}
        value.update(changes)
        (directory / "body.bin").write_bytes(body)
        (directory / "capture.json").write_bytes((json.dumps(value) + "\n").encode())
        return directory

    def select(self, selected):
        self.manifest.write_bytes((json.dumps({"format_version": 1, "pairs": selected}) + "\n").encode())

    def cli(self, *args):
        _, result = run_captured_probe([sys.executable, str(CLI), *map(os.fspath, args)], os.environ.copy(), timeout=30)
        return result

    def report(self, result, count):
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.assertEqual(len(result.stdout.splitlines()), 1)
        self.assertLessEqual(len(result.stdout), 65536)
        value = json.loads(result.stdout)
        self.assertEqual(set(value), {"format_version", "scope", "pairs"})
        self.assertIs(type(value["format_version"]), int)
        self.assertEqual((value["format_version"], value["scope"]), (1, "manual_http_baseline_pair_list_comparison"))
        self.assertEqual(len(value["pairs"]), count)
        for ordinal, item in enumerate(value["pairs"], 1):
            self.assertEqual(set(item), {"ordinal", "changed_header_names"} | BOOLS)
            self.assertIs(type(item["ordinal"]), int)
            self.assertEqual(item["ordinal"], ordinal)
            self.assertTrue(all(type(item[key]) is bool for key in BOOLS))
            self.assertEqual(item["changed_header_names"], sorted(set(item["changed_header_names"])))
        self.assertNotIn(b"PRIVATE", result.stdout)
        self.assertNotIn(str(self.work).encode(), result.stdout)
        return value

    def refused(self, *args):
        result = self.cli(*(args or (self.manifest,)))
        self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"HTTP baseline pair comparison failed\n"))

    def test_mixed_equal_and_different_pairs_preserve_order_without_a_verdict_or_private_values(self):
        left = self.baseline("PRIVATE_left")
        same = self.baseline("PRIVATE_same", recorded_at_utc="2026-10-29T11:12:13Z")
        different = self.baseline("PRIVATE_different", body=b"PRIVATE_B\0\xff")
        self.select([{"left": str(left), "right": str(same)}, {"left": str(left), "right": str(different)}, {"left": str(different), "right": str(left)}])
        before = {str(path): path.read_bytes() for directory in (left, same, different) for path in directory.iterdir()}
        result = self.cli(self.manifest)
        value = self.report(result, 3)
        self.assertTrue(all(value["pairs"][0][key] for key in BOOLS))
        for item in value["pairs"][1:]:
            self.assertFalse(item["body_equal"])
            self.assertTrue(all(item[key] for key in BOOLS - {"body_equal"}))
            self.assertEqual(item["changed_header_names"], [])
        self.assertEqual({str(path): path.read_bytes() for directory in (left, same, different) for path in directory.iterdir()}, before)
        self.assertNotIn(hashlib.sha256(b"PRIVATE_A\0\xff").hexdigest().encode(), result.stdout)

    def test_relative_paths_anchor_to_manifest_parent_independently_of_current_directory(self):
        self.baseline("left")
        self.baseline("right")
        self.select([{"left": "../left", "right": "../right"}])
        other = self.work / "unrelated-cwd"
        other.mkdir()
        previous = Path.cwd()
        try:
            os.chdir(other)
            value = self.report(self.cli(self.manifest), 1)
        finally:
            os.chdir(previous)
        self.assertTrue(all(value["pairs"][0][key] for key in BOOLS))

    def test_32_pair_and_64k_manifest_boundaries_with_whole_list_refusal(self):
        left = self.baseline("left")
        pair = {"left": str(left), "right": str(left)}
        self.select([pair] * 32)
        self.report(self.cli(self.manifest), 32)
        self.select([pair] * 33)
        self.refused()
        self.select([pair])
        raw = self.manifest.read_bytes()
        self.manifest.write_bytes(raw + b" " * (65536 - len(raw)))
        self.report(self.cli(self.manifest), 1)
        with self.manifest.open("ab") as stream:
            stream.write(b" ")
        self.refused()

    def test_schema_type_path_and_duplicate_json_refusals_are_opaque(self):
        left = self.baseline("left")
        pair = {"left": str(left), "right": str(left)}
        good = {"format_version": 1, "pairs": [pair]}
        invalid = [{**good, "PRIVATE_unknown": True}, {"format_version": 1}, {**good, "format_version": True},
                   {**good, "format_version": 1.0}, {**good, "pairs": []}, {**good, "pairs": False},
                   {**good, "pairs": [{**pair, "PRIVATE_extra": "value"}]}, {**good, "pairs": [{"left": str(left)}]}]
        for value in (None, False, "", "PRIVATE\0path", "\ud800"):
            invalid.append({**good, "pairs": [{**pair, "left": value}]})
        if os.name == "nt":
            for value in ("C:PRIVATE", "\\PRIVATE"):
                invalid.append({**good, "pairs": [{**pair, "left": value}]})
        for index, value in enumerate(invalid):
            with self.subTest(case=index):
                self.manifest.write_bytes(json.dumps(value).encode())
                self.refused()
        wire = json.dumps(good).encode()
        duplicate = wire.replace(b'"format_version": 1', b'"format_version": 1, "format_version": 1')
        for raw in (b"\xff", b"\xef\xbb\xbf" + wire, duplicate, b'{"format_version":NaN}'):
            self.manifest.write_bytes(raw)
            self.refused()

    def test_late_invalid_pair_produces_no_partial_stdout_and_direct_helper_spawns_no_child(self):
        left = self.baseline("left")
        right = self.baseline("right")
        broken = self.baseline("broken")
        (broken / "body.bin").write_bytes(b"PRIVATE_TAMPERED")
        self.select([{"left": str(left), "right": str(right)}, {"left": str(left), "right": str(broken)}])
        with patch.object(pairs, "_compare_worker", wraps=pairs._compare_worker) as compare, patch.object(owned_probe, "run_captured_probe", side_effect=AssertionError("nested process")) as run:
            with self.assertRaises(Exception):
                pairs._pair_list_worker(self.manifest)
            self.assertEqual(compare.call_count, 2)
            run.assert_not_called()
        self.refused()
        request = json.dumps({"manifest": str(self.manifest)}).encode()
        output = io.BytesIO()
        with patch.object(pairs.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(request))), patch.object(pairs.sys, "stdout", SimpleNamespace(buffer=output)):
            self.assertEqual(pairs._worker_main(), 2)
        self.assertEqual(output.getvalue(), b"")

    def test_arguments_missing_and_nonregular_manifest_are_fixed_refusals(self):
        left = self.baseline("left")
        self.select([{"left": str(left), "right": str(left)}])
        for arguments in ([], [self.manifest, "PRIVATE_extra"], [self.work / "PRIVATE_missing"], [self.work]):
            result = self.cli(*arguments)
            self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"HTTP baseline pair comparison failed\n"))

    def test_symlink_manifest_refusal_does_not_follow_or_modify_the_target(self):
        left = self.baseline("left")
        self.select([{"left": str(left), "right": str(left)}])
        before = self.manifest.read_bytes()
        link = self.work / "PRIVATE_link.json"
        try:
            link.symlink_to(self.manifest)
        except (OSError, NotImplementedError) as error:
            self.skipTest("symlink creation unavailable: " + type(error).__name__)
        self.refused(link)
        self.assertEqual(self.manifest.read_bytes(), before)
        self.assertTrue(link.is_symlink())

    def test_pid_report_ordinal_and_timeout_refusals_retry_cleanup_without_process(self):
        row = {"ordinal": 1, **dict.fromkeys(BOOLS, True), "changed_header_names": []}
        report = {"format_version": 1, "scope": "manual_http_baseline_pair_list_comparison", "pairs": [row]}
        pid = 45678
        envelopes = [{"process_pid": pid + 1, "report": report},
                     {"process_pid": pid, "report": {**report, "pairs": [{**row, "ordinal": True}]}},
                     {"process_pid": pid, "report": {**report, "pairs": [{**row, "body_equal": 1}]}},
                     {"process_pid": pid, "report": {**report, "pairs": [{**row, "changed_header_names": ["vary"]}]}},
                     {"process_pid": pid, "report": {**report, "pairs": [row, {**row, "ordinal": 3}]}}]
        for envelope in envelopes:
            result = subprocess.CompletedProcess(["mock-worker"], 0, (json.dumps(envelope) + "\n").encode(), b"")
            with patch.object(owned_probe, "run_captured_probe", return_value=(pid, result)), patch.object(owned_probe, "reap_owned_probes") as reap:
                with self.assertRaises(pairs.HttpBaselinePairListError) as caught:
                    pairs.compare_http_baseline_pairs(self.manifest)
                self.assertEqual(str(caught.exception), "HTTP baseline pair comparison failed")
                self.assertNotIn("PRIVATE", repr(caught.exception))
                reap.assert_called_once()
        with patch.object(owned_probe, "run_captured_probe", side_effect=subprocess.TimeoutExpired(["owned-worker"], 0.2)), patch.object(owned_probe, "reap_owned_probes") as reap:
            with self.assertRaises(pairs.HttpBaselinePairListError):
                pairs.compare_http_baseline_pairs(self.manifest, timeout=0.2)
            reap.assert_called_once()


if __name__ == "__main__":
    unittest.main()
