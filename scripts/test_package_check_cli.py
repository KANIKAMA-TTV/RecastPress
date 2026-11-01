"""Actual receipt-aware one-shot CLI against Cargo's current selected artifact."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from owned_probe import reap_owned_probes, run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
CANARY = "PRIVATE_PACKAGE_MARKER_日"
MAX_PAYLOAD = 8 * 1024 * 1024


def encoded(value):
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def receipt(payload):
    return encoded({"format_version": 1, "payload_bytes": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest()}) + b"\n"


def page(**changes):
    return {"target": "/chosen", "status": 200,
            "content_type": "text/html; charset=utf-8", "body": CANARY, **changes}


class PackageCheckCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.addClassCleanup(reap_owned_probes)
        built = subprocess.run(["cargo", "build", "-p", "recastpress-server", "--bin", "recastpress",
            "--offline", "--locked", "--message-format=json"], cwd=ROOT, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        items = [json.loads(line) for line in built.stdout.splitlines()]
        manifest = (ROOT / "crates/recastpress-server/Cargo.toml").resolve()
        main = (manifest.parent / "src/main.rs").resolve()
        paths = [Path(item["executable"]).resolve() for item in items
            if item.get("reason") == "compiler-artifact" and item.get("executable")
            and Path(item.get("manifest_path", "")).resolve() == manifest
            and item.get("target", {}).get("name") == "recastpress"
            and item.get("target", {}).get("kind") == ["bin"]
            and Path(item.get("target", {}).get("src_path", "")).resolve() == main]
        if len(paths) != 1 or not paths[0].is_file():
            raise AssertionError("Cargo did not select exactly one current server artifact")
        cls.binary = paths[0]

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="package-check-cli-", dir=ROOT / "target")
        self.addCleanup(temporary.cleanup)
        self.addCleanup(reap_owned_probes)
        self.work = Path(temporary.name)
        self.payload_path = self.work / (CANARY + "-bundle.json")
        self.receipt_path = self.work / (CANARY + "-receipt.json")

    def run_binary(self, *args):
        _, result = run_captured_probe([str(self.binary), *map(os.fspath, args)],
                                      os.environ.copy(), timeout=10)
        self.assertNotIn(CANARY.encode(), result.stdout)
        self.assertNotIn(CANARY.encode(), result.stderr)
        self.assertNotIn(b"PRIVATE_PACKAGE_MARKER", result.stdout)
        self.assertNotIn(b"PRIVATE_PACKAGE_MARKER", result.stderr)
        self.assertNotIn(str(self.work).encode(), result.stdout)
        self.assertNotIn(str(self.work).encode(), result.stderr)
        return result

    def saved_pair(self, payload, proof=None):
        self.payload_path.write_bytes(payload)
        self.receipt_path.write_bytes(receipt(payload) if proof is None else proof)

    def refused(self, result):
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"package check failed\n")

    def report(self, result):
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b"")
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.assertNotIn(b"\r", result.stdout)
        self.assertEqual(len(result.stdout.splitlines()), 1)
        self.assertLessEqual(len(result.stdout), 1024)
        report = json.loads(result.stdout)
        self.assertEqual(set(report), {"format_version", "accepted_bundle_version", "pages",
                                      "redirects", "body_bytes", "mime_counts"})
        self.assertEqual(report["format_version"], 1)
        return report

    def test_v1_v2_match_existing_admission_report_and_leave_inputs_unchanged(self):
        fixtures = [
            {"format_version": 1, "pages": [page()]},
            {"format_version": 2, "pages": [
                {"target": "/redirect", "status": 308, "location": "/PRIVATE_PACKAGE_MARKER"},
                {"target": "/png", "status": 200, "content_type": "image/png", "body_hex": "89504e470d0a1a0a"},
                {"target": "/text", "status": 410, "content_type": "text/plain; charset=utf-8", "body_hex": "00e697a5"}]}]
        for fixture in fixtures:
            with self.subTest(version=fixture["format_version"]):
                self.saved_pair(encoded(fixture))
                before = {path.name: path.read_bytes() for path in self.work.iterdir()}
                result = self.run_binary("check-package", self.payload_path, self.receipt_path)
                report = self.report(result)
                previous = self.run_binary("check-bundle", self.payload_path)
                self.assertEqual(result.stdout, previous.stdout)
                self.assertEqual(report["accepted_bundle_version"], fixture["format_version"])
                self.assertEqual(report["body_bytes"], len(CANARY.encode()) if fixture["format_version"] == 1 else 12)
                self.assertEqual(before, {path.name: path.read_bytes() for path in self.work.iterdir()})

    def test_changed_length_digest_and_reformatted_bytes_refuse_without_partial_report(self):
        payload = encoded({"format_version": 1, "pages": [page()]})
        proof = receipt(payload)
        for changed in [payload + b" ", payload.replace(b"PRIVATE", b"CHANGED"),
                        json.dumps(json.loads(payload), ensure_ascii=False, indent=2).encode()]:
            with self.subTest(bytes=len(changed)):
                self.saved_pair(changed, proof)
                self.assertEqual(self.run_binary("check-bundle", self.payload_path).returncode, 0)
                self.refused(self.run_binary("check-package", self.payload_path, self.receipt_path))

    def test_matching_receipt_never_bypasses_strict_bundle_admission(self):
        invalid = [b"PRIVATE_PACKAGE_MARKER", b'{"format_version":1,"pages":[],"pages":[]}',
            encoded({"format_version": 1, "pages": []}),
            encoded({"format_version": 1, "pages": [page(), page()]}),
            encoded({"format_version": 1, "pages": [page(target="/health/ready?x=1")]}),
            encoded({"format_version": 1, "pages": [page(status=CANARY)]}),
            encoded({"format_version": 2, "pages": [{"target": "/", "status": 200,
                "content_type": "image/png", "body_hex": "00"}]})]
        for payload in invalid:
            with self.subTest(bytes=len(payload)):
                self.saved_pair(payload)
                self.refused(self.run_binary("check-package", self.payload_path, self.receipt_path))

    def test_strict_receipt_schema_and_sentinel_bounds(self):
        payload = encoded({"format_version": 1, "pages": [page()]})
        valid = json.loads(receipt(payload))
        proofs = [b"PRIVATE_PACKAGE_MARKER", receipt(payload).replace(b'"format_version":1',
                  b'"format_version":1,"format_vers\\u0069on":1')]
        for change in [{"format_version": 2}, {"payload_bytes": True}, {"payload_bytes": -1},
                       {"payload_bytes": float(len(payload))}, {"sha256": "A" * 64},
                       {"sha256": "0" * 63}, {CANARY: True}]:
            proofs.append(encoded({**valid, **change}))
        for proof in proofs:
            self.saved_pair(payload, proof)
            self.refused(self.run_binary("check-package", self.payload_path, self.receipt_path))
        padded_proof = receipt(payload).ljust(1024, b" ")
        self.saved_pair(payload, padded_proof)
        self.report(self.run_binary("check-package", self.payload_path, self.receipt_path))
        self.receipt_path.write_bytes(padded_proof + b" ")
        self.refused(self.run_binary("check-package", self.payload_path, self.receipt_path))
        exact = payload.ljust(MAX_PAYLOAD, b" ")
        self.saved_pair(exact)
        self.report(self.run_binary("check-package", self.payload_path, self.receipt_path))
        self.saved_pair(exact + b" ")
        self.refused(self.run_binary("check-package", self.payload_path, self.receipt_path))

    def test_exact_native_arguments_missing_files_and_help(self):
        self.saved_pair(encoded({"format_version": 1, "pages": [page()]}))
        help_result = self.run_binary("--help")
        self.assertEqual(help_result.returncode, 0)
        self.assertIn(b"recastpress check-package <bundle> <receipt>", help_result.stdout)
        for args in [(), (self.payload_path,), (self.payload_path, self.receipt_path, "extra"),
                     ("", self.receipt_path), (self.payload_path, ""),
                     (self.work / "missing", self.receipt_path),
                     (self.payload_path, self.work / "missing"), (self.work, self.receipt_path)]:
            with self.subTest(arguments=len(args)):
                self.refused(self.run_binary("check-package", *args))

    def test_real_prepared_package_checked_without_server_startup(self):
        input_path = self.work / "input.json"
        input_path.write_bytes(encoded({"format_version": 1, "posts": [
            {"target": "/chosen?b=2&a=1", "title": CANARY, "body_text": "\r\n<script>literal</script> 🦀"}]}))
        output = self.work / "new-package"
        prepared = self.run_binary("prepare-post-package", input_path, output)
        self.assertEqual(prepared.returncode, 0)
        self.assertEqual(prepared.stdout, b"post package prepared\n")
        self.assertEqual(prepared.stderr, b"")
        paths = sorted(output.iterdir())
        self.assertEqual([path.name for path in paths], ["bundle.json", "receipt.json"])
        before = [path.read_bytes() for path in paths]
        report = self.report(self.run_binary("check-package", output / "bundle.json", output / "receipt.json"))
        self.assertEqual((report["pages"], report["redirects"]), (1, 0))
        self.assertEqual([path.read_bytes() for path in paths], before)


if __name__ == "__main__":
    unittest.main()
