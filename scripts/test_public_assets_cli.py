"""Fixed V3 HTML/CSS/JavaScript and existing sealed HTTP relations.

Only explicitly prepared public fixtures, with owned finite local processes.
No browser, external fetch, font discovery, DB or theme engine adoption.
"""
import hashlib
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import owned_probe
import smoke
from test_prepared_workbench_package import select_server

PUBLIC_TARGET = ROOT / "target/wasm-publication-probe/public-server"
BUILD_STREAM = ROOT / "target/public-assets-current-server-build.jsonl"
OBSERVATION = ROOT / "target/sealed-public-assets-native-observation.json"
HTML_TARGET = "/asset-demo?view=1"
CSS_TARGET = "/assets/demo.css?v=1"
JS_TARGET = "/assets/demo.js?v=1"
HTML = b'''<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>Sealed assets fixture</title>
<link rel="icon" href="data:,"><link rel="stylesheet" href="/assets/demo.css?v=1">
<script src="/assets/demo.js?v=1" defer></script></head>
<body><main><h1>Fixed sealed assets</h1><section id="status-card">
<p>CSS: this card should have a green background and border.</p>
<p id="script-status">JavaScript has not run.</p></section></main></body></html>
'''
CSS = b'''body { font-family: system-ui, sans-serif; margin: 2rem; }
#status-card { background: #e8f7ee; border: 3px solid #087b78; padding: 1rem; border-radius: .5rem; }
'''
JS = b'''"use strict";
document.getElementById("script-status").textContent = "JavaScript loaded.";
document.getElementById("status-card").dataset.script = "ready";
'''
ASSETS = ((HTML_TARGET, "text/html; charset=utf-8", HTML),
          (CSS_TARGET, "text/css; charset=utf-8", CSS),
          (JS_TARGET, "text/javascript; charset=utf-8", JS))
OLD_MIMES = ("text/html; charset=utf-8", "text/plain; charset=utf-8",
             "application/rss+xml; charset=utf-8", "application/atom+xml; charset=utf-8",
             "image/png", "image/jpeg", "image/webp")
NEW_MIMES = (*OLD_MIMES, "text/css; charset=utf-8", "text/javascript; charset=utf-8")


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")


def fixture_payload():
    return encoded({"format_version": 3, "pages": [
        {"target": target, "status": 200, "content_type": mime, "body_hex": raw.hex()}
        for target, mime, raw in ASSETS]})


def fixture_receipt(payload):
    return encoded({"format_version": 1, "payload_bytes": len(payload), "sha256": sha(payload)})


def write_fixture(directory):
    """Optional public browser fixture, no build/admission/listener operation.

    Caller uses a new target-owned directory. Browser authority/server startup
    remains the root agent's separate owned operation after V3 adoption.
    """
    directory = Path(directory).resolve()
    if not directory.is_relative_to((ROOT / "target").resolve()):
        raise ValueError("Fixture directory must be target-owned")
    payload = fixture_payload()
    directory.mkdir()
    (directory / "bundle.json").write_bytes(payload)
    (directory / "receipt.json").write_bytes(fixture_receipt(payload))
    return {"bundle_bytes": len(payload), "bundle_sha256": sha(payload), "html_target": HTML_TARGET}


def source_hashes():
    paths = ["Cargo.toml", "Cargo.lock", "crates/recastpress-server/Cargo.toml",
             "crates/recastpress-core/src/publication.rs", "crates/recastpress-core/src/publication_redirect.rs",
             "crates/recastpress-server/src/bundle.rs", "crates/recastpress-server/src/bundle_receipt.rs",
             "crates/recastpress-server/src/http_cache.rs", "crates/recastpress-server/src/main.rs",
             "crates/recastpress-server/src/cli.rs"]
    return {name: sha((ROOT / name).read_bytes()) for name in paths}


class PublicAssetsCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.addClassCleanup(owned_probe.reap_owned_probes)
        before = source_hashes()
        cls.build_pid, result = owned_probe.run_captured_probe(
            ["cargo", "build", "--manifest-path", str(ROOT / "Cargo.toml"), "-p", "recastpress-server",
             "--bin", "recastpress", "--target-dir", str(PUBLIC_TARGET), "--offline", "--locked",
             "--message-format=json"], os.environ.copy(), timeout=240)
        if result.returncode != 0 or len(result.stdout) > 16 * 1024 * 1024:
            raise AssertionError("Current isolated server build failed")
        cls.binary = select_server(result.stdout)
        if not cls.binary.is_relative_to(PUBLIC_TARGET.resolve()) or source_hashes() != before:
            raise AssertionError("Isolated artifact or selected sources differ")
        BUILD_STREAM.write_bytes(result.stdout)
        cls.sources = before
        cls.binary_sha = sha(cls.binary.read_bytes())

    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="sealed-assets-cli-", dir=ROOT / "target")).resolve()
        self.addCleanup(self.cleanup_owned)
        self.pids = []
        self.bundle = self.work / "bundle.json"
        self.receipt = self.work / "receipt.json"

    def cleanup_owned(self):
        failures = []
        for reap in (smoke.reap_servers, owned_probe.reap_owned_probes):
            try:
                reap()
            except BaseException as error:
                failures.append(error)
        if failures:
            raise RuntimeError("Owned fixture cleanup incomplete") from None
        if self.work.parent != (ROOT / "target").resolve() or not self.work.name.startswith("sealed-assets-cli-"):
            raise AssertionError("Owned cleanup containment failed")
        if self.work.exists():
            shutil.rmtree(self.work)
        self.assertEqual(source_hashes(), self.sources)
        self.assertEqual(sha(self.binary.read_bytes()), self.binary_sha)

    def cli(self, *args):
        pid, result = owned_probe.run_captured_probe([str(self.binary), *map(os.fspath, args)],
                                                    os.environ.copy(), timeout=20)
        self.pids.append(pid)
        self.assertLessEqual(len(result.stdout), 65536)
        self.assertLessEqual(len(result.stderr), 65536)
        return result

    def saved(self, payload):
        self.bundle.write_bytes(payload)
        self.receipt.write_bytes(fixture_receipt(payload))

    def report(self, result, version, pages, body_bytes, counts, redirects=0):
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        self.assertLessEqual(len(result.stdout), 1024)
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.assertEqual(len(result.stdout.splitlines()), 1)
        self.assertNotIn(b"\r", result.stdout)
        expected = {"format_version": 2 if version == 3 else 1, "accepted_bundle_version": version,
                    "pages": pages, "redirects": redirects, "body_bytes": body_bytes,
                    "mime_counts": {mime: counts.get(mime, 0) for mime in (NEW_MIMES if version == 3 else OLD_MIMES)}}
        self.assertEqual(json.loads(result.stdout), expected)
        self.assertNotIn(str(self.work).encode(), result.stdout)
        return result.stdout

    def test_v3_has_report2_exact_nine_mimes_and_schema1_byte_receipt(self):
        payload = fixture_payload()
        self.saved(payload)
        counts = {mime: 1 for _, mime, _ in ASSETS}
        wire = self.report(self.cli("check-bundle", self.bundle), 3, 3,
                           sum(len(raw) for _, _, raw in ASSETS), counts)
        receipt = self.cli("bundle-receipt", self.bundle)
        self.assertEqual((receipt.returncode, receipt.stderr, receipt.stdout),
                         (0, b"", fixture_receipt(payload)))
        self.assertLessEqual(len(receipt.stdout), 1024)
        checked = self.cli("check-package", self.bundle, self.receipt)
        self.assertEqual((checked.returncode, checked.stderr, checked.stdout), (0, b"", wire))

    def test_old_v1_v2_reports_keep_schema1_and_exact_seven_mimes(self):
        for version in (1, 2):
            body = {"body": "old text"} if version == 1 else {"body_hex": b"old text".hex()}
            payload = encoded({"format_version": version, "pages": [
                {"target": "/old", "status": 200, "content_type": "text/plain; charset=utf-8", **body}]})
            self.saved(payload)
            self.report(self.cli("check-bundle", self.bundle), version, 1, 8,
                        {"text/plain; charset=utf-8": 1})

    def test_css_js_keep_legacy_domain_closed_and_v3_refuses_invalid_asset_admission(self):
        invalid = []
        for mime in NEW_MIMES[-2:]:
            invalid.append({"format_version": 1, "pages": [{"target": "/asset", "status": 200,
                            "content_type": mime, "body": "public"}]})
            invalid.append({"format_version": 2, "pages": [{"target": "/asset", "status": 200,
                            "content_type": mime, "body_hex": b"public".hex()}]})
            for change in ({"status": 201}, {"body_hex": "ff"}, {"content_type": mime.upper()},
                           {"target": "/health/live?x=1"}, {"file": "PRIVATE_ASSET_PATH"}):
                invalid.append({"format_version": 3, "pages": [{"target": "/asset", "status": 200,
                                "content_type": mime, "body_hex": b"public".hex(), **change}]})
        repeated = json.loads(fixture_payload())
        repeated["pages"].append(repeated["pages"][0].copy())
        invalid.append(repeated)
        for value in invalid:
            with self.subTest(version=value["format_version"]):
                self.saved(encoded(value))
                result = self.cli("check-bundle", self.bundle)
                self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"bundle check failed\n"))
                checked = self.cli("check-package", self.bundle, self.receipt)
                self.assertEqual((checked.returncode, checked.stdout, checked.stderr), (2, b"", b"package check failed\n"))

    def test_valid_utf8_assets_are_not_claimed_to_pass_language_syntax_validation(self):
        # Valid UTF8 BOM/NUL and nonsensical syntax stay raw; no parser executes.
        payload = encoded({"format_version": 3, "pages": [
            {"target": "/bad.css", "status": 200, "content_type": NEW_MIMES[-2], "body_hex": b"\xef\xbb\xbf\0{???".hex()},
            {"target": "/bad.js", "status": 200, "content_type": NEW_MIMES[-1], "body_hex": b"\0not valid javascript )".hex()}]})
        self.saved(payload)
        size = sum(len(bytes.fromhex(page["body_hex"])) for page in json.loads(payload)["pages"])
        self.report(self.cli("check-bundle", self.bundle), 3, 2, size,
                    {NEW_MIMES[-2]: 1, NEW_MIMES[-1]: 1})

    def test_fixed_three_assets_serve_exact_bytes_and_keep_sealed_file_snapshot(self):
        payload = fixture_payload()
        self.saved(payload)
        expected_receipt = self.receipt.read_bytes()
        checked = self.cli("check-package", self.bundle, self.receipt)
        self.assertEqual((checked.returncode, checked.stderr), (0, b""))
        server = smoke.start("public", "--host", "example.test", "--bundle", str(self.bundle),
                             "--receipt", str(self.receipt), binary=self.binary)
        checks, etags = [], {}
        try:
            for index, (target, mime, body) in enumerate(ASSETS):
                status, headers, raw = smoke.request(server.port, target)
                self.assertEqual((status, raw, headers["content-type"], int(headers["content-length"])),
                                 (200, body, mime, len(body)))
                etags[target] = headers["etag"]
                checks.append(f"get_{index}")
                status, headers, raw = smoke.request(server.port, target, method="HEAD")
                self.assertEqual((status, raw, int(headers["content-length"]), headers["etag"]),
                                 (200, b"", len(body), etags[target]))
                checks.append(f"head_{index}")
                status, _, raw = smoke.request(server.port, target, extra_headers={"If-None-Match": etags[target]})
                self.assertEqual((status, raw), (304, b""))
                checks.append(f"conditional_{index}")
                self.assertEqual(smoke.request(server.port, target, host="wrong.test")[0], 421)
                self.assertEqual(smoke.request(server.port, target + "&other=1")[0], 404)
                checks.append(f"host_query_{index}")
            for index, (target, _, body) in enumerate(ASSETS[1:]):
                status, headers, raw = smoke.request(server.port, target, extra_headers={"Range": "bytes=0-15"})
                self.assertEqual((status, raw, int(headers["content-length"])), (206, body[:16], 16))
                self.assertEqual(headers["content-range"], f"bytes 0-15/{len(body)}")
                self.assertEqual(headers["etag"], etags[target])
                checks.append(f"range_{index}")
            self.bundle.write_bytes(b"changed after startup\n")
            self.receipt.write_bytes(b"changed after startup\n")
            for index, (target, mime, body) in enumerate(ASSETS):
                status, headers, raw = smoke.request(server.port, target)
                self.assertEqual((status, raw, headers["etag"], headers["content-type"]),
                                 (200, body, etags[target], mime))
                checks.append(f"sealed_{index}")
        finally:
            smoke.stop(server, original=sys.exc_info()[1])
        smoke.reap_servers()
        owned_probe.reap_owned_probes()
        self.assertIsNotNone(server.process.returncode)
        self.assertFalse(server.reader.is_alive())
        self.assertEqual(source_hashes(), self.sources)
        self.assertEqual(sha(self.binary.read_bytes()), self.binary_sha)
        record = {"format_version": 1, "scope": "fixed_v3_sealed_html_css_javascript",
                  "build_pid": self.build_pid, "probe_pids": self.pids, "server_pid": server.process.pid,
                  "server_sha256": self.binary_sha, "bundle_sha256": sha(payload),
                  "receipt_sha256": sha(expected_receipt), "asset_sha256": [sha(raw) for _, _, raw in ASSETS],
                  "checks": checks, "selected_source_hashes": self.sources,
                  "selected_sources_unchanged": True, "server_exit_code": server.process.returncode,
                  "reader_joined": True, "all_owned_children_reaped": True,
                  "limitations": "No browser effect, URL discovery, general language validation, theme/auth/DB/Linux or cross-request atomic snapshot claim."}
        self.cleanup_owned()
        self.assertFalse(self.work.exists())
        record["owned_directory_removed"] = True
        OBSERVATION.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
