"""Independent offline baseline fixtures and private structural report contracts."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import http_baseline_compare
import owned_probe
from owned_probe import reap_owned_probes, run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "scripts/compare_http_baselines.py"
LIMIT = 8 * 1024 * 1024
BOOLS = {"body_equal", "status_equal", "headers_equal", "http_version_equal", "request_target_equal", "url_equal", "transport_equal", "tls_verification_equal"}
HEADERS = {"content-type", "content-encoding", "content-length", "cache-control", "etag", "last-modified", "location", "vary"}


class HttpBaselineCompareTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="http-baseline-compare-test-", dir=ROOT / "target")
        self.work = Path(self.directory.name)
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(reap_owned_probes)

    def fixture(self, name, body=b"\0SECRET_BODY\xff", **changes):
        directory = self.work / name
        directory.mkdir()
        value = {"format_version": 1, "scope": "manual_single_url_http_baseline",
                 "url": "http://127.0.0.1:9/SECRET_TARGET?key=SECRET_QUERY", "request_target": "/SECRET_TARGET?key=SECRET_QUERY",
                 "method": "GET", "status": 200, "http_version": "HTTP/1.1", "recorded_at_utc": "2026-10-29T00:00:00+00:00",
                 "transport": "http", "tls_verification": "not_applicable",
                 "headers": {"content-type": ["application/octet-stream"], "content-length": [str(len(body))], "vary": ["Accept", "X-SECRET"]},
                 "body_file": "body.bin", "body_bytes": len(body), "body_sha256": hashlib.sha256(body).hexdigest()}
        value.update(changes)
        (directory / "body.bin").write_bytes(body)
        self.metadata(directory, value)
        return directory

    def metadata(self, directory, value):
        (directory / "capture.json").write_bytes((json.dumps(value, ensure_ascii=True) + "\n").encode())

    def read_metadata(self, directory):
        return json.loads((directory / "capture.json").read_bytes())

    def cli(self, *args):
        _, result = run_captured_probe([sys.executable, str(CLI), *map(os.fspath, args)], os.environ.copy(), timeout=30)
        return result

    def compared(self, left, right, unequal=(), changed=()):
        result = self.cli(left, right)
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.assertEqual(len(result.stdout.splitlines()), 1)
        value = json.loads(result.stdout)
        self.assertEqual(set(value), {"format_version", "scope", "changed_header_names"} | BOOLS)
        self.assertIs(type(value["format_version"]), int)
        self.assertEqual((value["format_version"], value["scope"]), (1, "manual_http_baseline_structural_comparison"))
        for key in BOOLS:
            self.assertIs(type(value[key]), bool)
            self.assertEqual(value[key], key not in unequal)
        self.assertEqual(value["changed_header_names"], list(changed))
        for canary in (b"SECRET", str(left).encode(), str(right).encode(), self.read_metadata(left)["body_sha256"].encode()):
            self.assertNotIn(canary, result.stdout)
        return value

    def refused(self, *args):
        result = self.cli(*args)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"HTTP baseline comparison failed\n"))

    def test_identical_binary_records_ignore_valid_utc_observation_time_and_are_read_only(self):
        left = self.fixture("left")
        right = self.fixture("right", recorded_at_utc="2026-10-29T12:34:56.123456Z")
        before = {str(path): path.read_bytes() for directory in (left, right) for path in directory.iterdir()}
        self.compared(left, right)
        self.compared(left, left)
        self.assertEqual({str(path): path.read_bytes() for directory in (left, right) for path in directory.iterdir()}, before)

    def test_body_difference_is_exit_zero_with_no_aggregate_pass_or_private_values(self):
        left = self.fixture("left", body=b"SECRET_A\0\xff")
        right = self.fixture("right", body=b"SECRET_B\0\xff")
        self.compared(left, right, unequal={"body_equal"})

    def test_headers_preserve_duplicate_order_empty_values_and_sorted_changed_names(self):
        left = self.fixture("left")
        headers = self.read_metadata(left)["headers"]
        headers["vary"] = ["", "Accept\r\n X-SECRET", "\xff"]
        self.metadata(left, {**self.read_metadata(left), "headers": headers})
        right = self.fixture("right", headers=dict(reversed(list(headers.items()))))
        self.compared(left, right)
        changed = {**headers, "vary": list(reversed(headers["vary"])), "content-type": ["application/SECRET"], "cache-control": ["private"]}
        self.metadata(right, {**self.read_metadata(right), "headers": changed})
        self.compared(left, right, unequal={"headers_equal"}, changed=("cache-control", "content-type", "vary"))

    def test_context_differences_are_independent_observations_not_a_url_or_tls_verdict(self):
        left = self.fixture("left")
        right = self.fixture("right", status=503, http_version="HTTP/1.0")
        self.compared(left, right, unequal={"status_equal", "http_version_equal"})
        self.metadata(right, {**self.read_metadata(left), "url": "http://localhost:9/SECRET_TARGET?key=SECRET_QUERY"})
        self.compared(left, right, unequal={"url_equal"})
        self.metadata(right, {**self.read_metadata(left), "url": "https://localhost:9/SECRET_OTHER?", "request_target": "/SECRET_OTHER?", "transport": "https", "tls_verification": "default_verified"})
        self.compared(left, right, unequal={"url_equal", "request_target_equal", "transport_equal", "tls_verification_equal"})

    def test_metadata_shape_types_crossfields_and_body_identity_are_admission_not_differences(self):
        left = self.fixture("left")
        right = self.fixture("right")
        original = self.read_metadata(right)
        bad = [{**original, "SECRET_unknown": True}, {key: value for key, value in original.items() if key != "body_sha256"}]
        for key, value in [("format_version", True), ("format_version", 2), ("status", False), ("status", 199), ("status", 600),
                           ("body_bytes", True), ("body_bytes", 0), ("body_sha256", "0" * 64), ("body_sha256", original["body_sha256"].upper()),
                           ("scope", "other"), ("method", "POST"), ("body_file", "../SECRET"), ("http_version", "HTTP/2"),
                           ("url", "file:///SECRET"), ("request_target", "/OTHER"), ("transport", "https"), ("tls_verification", "default_verified"),
                           ("recorded_at_utc", "2026-02-30T00:00:00Z"), ("recorded_at_utc", "2026-10-29T00:00:00+01:00"),
                           ("headers", {"Set-Cookie": ["SECRET"]}), ("headers", {"vary": []}), ("headers", {"vary": [False]}),
                           ("headers", {"content-length": ["1", "1"]}), ("headers", {"content-length": ["+12"]}), ("headers", {"content-length": ["999"]})]:
            bad.append({**original, key: value})
        bad.append({**original, "status": 204})  # A nonempty body cannot be a bodyless status.
        for index, value in enumerate(bad):
            with self.subTest(case=index):
                self.metadata(right, value)
                before = (right / "capture.json").read_bytes()
                self.refused(left, right)
                self.assertEqual((right / "capture.json").read_bytes(), before)
        valid = (json.dumps(original) + "\n").encode()
        for raw in (b"\xef\xbb\xbf" + valid, b"\xff", valid.replace(b'"format_version": 1', b'"format_version": 1.0'),
                    valid.replace(b'"format_version": 1', b'"format_version": 1, "format_version": 1'), b'{"x":NaN}'):
            (right / "capture.json").write_bytes(raw)
            self.refused(left, right)
        self.metadata(right, original)
        (right / "body.bin").write_bytes(b"SECRET_TAMPERED")
        self.refused(left, right)

    def test_exact_and_excess_file_bounds_and_bodyless_content_length_semantics(self):
        body = b"\0\xffAB" * (LIMIT // 4)
        left = self.fixture("left", body=body)
        right = self.fixture("right", body=body)
        self.compared(left, right)
        (right / "body.bin").write_bytes(body + b"X")
        self.refused(left, right)
        (right / "body.bin").write_bytes(body)
        raw = (right / "capture.json").read_bytes()
        (right / "capture.json").write_bytes(raw + b" " * (65536 - len(raw)))
        self.compared(left, right)
        with (right / "capture.json").open("ab") as stream:
            stream.write(b" ")
        self.refused(left, right)
        empty_left = self.fixture("empty-left", body=b"", status=304, headers={"content-length": ["999"]})
        empty_right = self.fixture("empty-right", body=b"", status=304, headers={"content-length": ["999"]})
        self.compared(empty_left, empty_right)

    def test_missing_extra_and_nonregular_files_and_arguments_are_fixed_refusals(self):
        left = self.fixture("left")
        right = self.fixture("right")
        for args in ((), (left,), (left, right, "SECRET_extra"), (left, self.work / "SECRET_missing")):
            self.refused(*args)
        extra = right / "SECRET_extra"
        extra.write_bytes(b"PRIVATE_CANARY")
        self.refused(left, right)
        self.assertEqual(extra.read_bytes(), b"PRIVATE_CANARY")
        extra.unlink()
        body = (right / "body.bin").read_bytes()
        (right / "body.bin").unlink()
        self.refused(left, right)
        (right / "body.bin").mkdir()
        self.refused(left, right)
        (right / "body.bin").rmdir()
        (right / "body.bin").write_bytes(body)
        metadata = (right / "capture.json").read_bytes()
        (right / "capture.json").unlink()
        (right / "capture.json").mkdir()
        self.refused(left, right)
        (right / "capture.json").rmdir()
        (right / "capture.json").write_bytes(metadata)
        self.compared(left, right)

    def test_symlink_directory_and_leaf_files_are_refused_without_following_them(self):
        left = self.fixture("left")
        right = self.fixture("right")
        link = self.work / "SECRET_link"
        try:
            link.symlink_to(right, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest("symlink creation unavailable: " + type(error).__name__)
        self.refused(left, link)
        body = (right / "body.bin").read_bytes()
        saved = self.work / "SECRET_body"
        (right / "body.bin").rename(saved)
        (right / "body.bin").symlink_to(saved)
        self.refused(left, right)
        self.assertEqual(saved.read_bytes(), body)
        self.assertTrue((right / "body.bin").is_symlink())

    def test_worker_timeout_and_tampered_envelopes_are_closed_with_cleanup_retry(self):
        left = self.fixture("left")
        right = self.fixture("right")
        valid_report = {"format_version": 1, "scope": "manual_http_baseline_structural_comparison", **dict.fromkeys(BOOLS, True), "changed_header_names": []}
        pid = 5678
        envelope = {"process_pid": pid, "report": valid_report}
        malformed = [{**envelope, "process_pid": pid + 1},
                     {**envelope, "report": {**valid_report, "body_equal": 1}},
                     {**envelope, "report": {**valid_report, "changed_header_names": ["vary"]}},
                     {**envelope, "report": {**valid_report, "headers_equal": False, "changed_header_names": ["vary", "etag"]}}]
        for value in malformed:
            result = subprocess.CompletedProcess(["mock-worker"], 0, (json.dumps(value) + "\n").encode(), b"")
            with patch.object(owned_probe, "run_captured_probe", return_value=(pid, result)), patch.object(owned_probe, "reap_owned_probes") as reap:
                with self.assertRaises(http_baseline_compare.HttpBaselineCompareError) as caught:
                    http_baseline_compare.compare_http_baselines(left, right)
                self.assertEqual(str(caught.exception), "HTTP baseline comparison failed")
                reap.assert_called_once()
        with patch.object(owned_probe, "run_captured_probe", side_effect=subprocess.TimeoutExpired(["owned-worker"], 0.2)), patch.object(owned_probe, "reap_owned_probes") as reap:
            with self.assertRaises(http_baseline_compare.HttpBaselineCompareError) as caught:
                http_baseline_compare.compare_http_baselines(left, right, timeout=0.2)
            self.assertEqual(str(caught.exception), "HTTP baseline comparison failed")
            reap.assert_called_once()


if __name__ == "__main__":
    unittest.main()
