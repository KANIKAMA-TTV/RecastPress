"""One-shot CLI admission against the freshly Cargo-selected binary, no listeners."""
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from owned_probe import run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
SECRET = "SECRET_BUNDLE_BODY_MARKER_日"


class BundleCheckCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        built = subprocess.run(["cargo", "build", "-p", "recastpress-server", "--bin", "recastpress", "--offline", "--locked", "--message-format=json"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        artifacts = [json.loads(line) for line in built.stdout.splitlines()]
        paths = [item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact"
                 and item.get("target", {}).get("name") == "recastpress"
                 and "bin" in item.get("target", {}).get("kind", []) and item.get("executable")]
        if len(paths) != 1 or not Path(paths[0]).is_file():
            raise AssertionError("Cargo did not identify exactly one current binary")
        cls.binary = Path(paths[0]).resolve()

    def run_binary(self, arguments):
        _, result = run_captured_probe([str(self.binary), *map(str, arguments)], os.environ.copy(), timeout=5)
        return result

    def run_wire(self, wire):
        with tempfile.TemporaryDirectory(prefix="recastpress-bundle-check-", dir=ROOT / "target") as directory:
            path = Path(directory) / (SECRET + ".json")
            path.write_bytes(wire)
            result = self.run_binary(["check-bundle", path])
        for marker in (SECRET.encode(), b"SECRET_BUNDLE_BODY_MARKER"):
            self.assertNotIn(marker, result.stdout)
            self.assertNotIn(marker, result.stderr)
        return result

    def successful(self, value):
        result = self.run_wire(json.dumps(value, ensure_ascii=False).encode())
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stderr, b"")
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.assertEqual(len(result.stdout.splitlines()), 1)
        self.assertLessEqual(len(result.stdout), 1024)
        report = json.loads(result.stdout)
        self.assertEqual(set(report), {"format_version", "accepted_bundle_version", "pages", "redirects", "body_bytes", "mime_counts"})
        for key in ("format_version", "accepted_bundle_version", "pages", "redirects", "body_bytes"):
            self.assertIs(type(report[key]), int)
        return report

    def refused(self, wire):
        result = self.run_wire(wire)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"bundle check failed\n")

    def test_v1_reports_utf8_bytes_and_bounded_counts_without_content_or_path(self):
        report = self.successful({"format_version": 1, "pages": [{"target": "/" + SECRET, "status": 200,
            "content_type": "text/html; charset=utf-8", "body": SECRET}]})
        self.assertEqual((report["format_version"], report["accepted_bundle_version"], report["pages"], report["redirects"]), (1, 1, 1, 0))
        self.assertEqual(report["body_bytes"], len(SECRET.encode()))
        self.assertEqual(report["mime_counts"], {"text/html; charset=utf-8": 1, "text/plain; charset=utf-8": 0,
            "application/rss+xml; charset=utf-8": 0, "application/atom+xml; charset=utf-8": 0,
            "image/png": 0, "image/jpeg": 0, "image/webp": 0})

    def test_v2_reports_decoded_binary_and_redirects_without_location(self):
        report = self.successful({"format_version": 2, "pages": [
            {"target": "/r", "status": 302, "location": "/SECRET_BUNDLE_BODY_MARKER"},
            {"target": "/png", "status": 200, "content_type": "image/png", "body_hex": "89504e470d0a1a0a"},
            {"target": "/text", "status": 410, "content_type": "text/plain; charset=utf-8", "body_hex": "00e697a5"}]})
        self.assertEqual((report["accepted_bundle_version"], report["pages"], report["redirects"], report["body_bytes"]), (2, 3, 1, 12))
        self.assertEqual(report["mime_counts"]["image/png"], 1)
        self.assertEqual(sum(report["mime_counts"].values()), 2)

    def test_strict_rejections_do_not_leak_parser_input_or_partial_json(self):
        page = {"target": "/", "status": 200, "content_type": "text/plain; charset=utf-8", "body": SECRET}
        bad = [
            {"format_version": 1, "pages": [page, page]},
            {"format_version": 1, "pages": []},
            {"format_version": 1, "pages": [{**page, "status": SECRET}]},
            {"format_version": 1, "pages": [{**page, SECRET: True}]},
            {"format_version": 1, "pages": [{**page, "target": "/health/ready?q=1"}]},
            {"format_version": 2, "pages": [{"target": "/", "status": 200, "content_type": "text/plain; charset=utf-8", "body_hex": "0z"}]},
            {"format_version": 2, "pages": [{"target": "/", "status": 200, "content_type": "image/png", "body_hex": "0000"}]},
            {"format_version": 4, "pages": []},
        ]
        for value in bad:
            with self.subTest(value=value):
                self.refused(json.dumps(value, ensure_ascii=False).encode())
        self.refused(b'{"format_version":1,"format_version":1,"pages":[]}')
        self.refused(b" " * (8 * 1024 * 1024 + 1))

    def test_exact_arguments_missing_file_and_serve_role_separation(self):
        help_result = self.run_binary(["--help"])
        self.assertEqual(help_result.returncode, 0)
        self.assertIn(b"recastpress check-bundle <path>", help_result.stdout)
        for args in [["check-bundle"], ["check-bundle", "missing-" + SECRET],
                     ["check-bundle", "missing", "extra"], ["check-bundle", "missing", "--listen", "127.0.0.1:8080"],
                     ["check-bundle", "missing", "management"]]:
            with self.subTest(args=args):
                result = self.run_binary(args)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(result.stdout, b"")
                self.assertEqual(result.stderr, b"bundle check failed\n")
        for args in [["management", "--bundle", "missing"], ["public"]]:
            result = self.run_binary(args)
            self.assertNotEqual(result.returncode, 0)
            self.assertNotEqual(result.stderr, b"bundle check failed\n")


if __name__ == "__main__":
    unittest.main()
