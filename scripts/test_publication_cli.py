"""Finite real-CLI preparation/receipt checks, no successful public socket startup."""
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from owned_probe import run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
SECRET = "SECRET_PUBLICATION_BODY_日"


class SampleTextParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.current = None
        self.values = {"title": "", "heading": "", "body": ""}
        self.tags = []

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        if tag == "title":
            self.current = "title"
        elif tag == "h1":
            self.current = "heading"
        elif tag == "div" and dict(attrs).get("class") == "post-body":
            self.current = "body"

    def handle_endtag(self, tag):
        if tag in ("title", "h1", "div"):
            self.current = None

    def handle_data(self, data):
        if self.current is not None:
            self.values[self.current] += data


class PublicationCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(["cargo", "build", "-p", "recastpress-server", "--bin", "recastpress", "--offline", "--locked", "--message-format=json"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        items = [json.loads(line) for line in result.stdout.splitlines()]
        paths = [item["executable"] for item in items if item.get("reason") == "compiler-artifact"
                 and item.get("target", {}).get("name") == "recastpress"
                 and "bin" in item.get("target", {}).get("kind", []) and item.get("executable")]
        if len(paths) != 1 or not Path(paths[0]).is_file():
            raise AssertionError("Cargo did not identify exactly one current binary")
        cls.binary = Path(paths[0]).resolve()

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="publication-cli-", dir=ROOT / "target")
        self.addCleanup(self.directory.cleanup)
        self.work = Path(self.directory.name)

    def run_binary(self, *args):
        _, result = run_captured_probe([str(self.binary), *map(os.fspath, args)], os.environ.copy(), timeout=10)
        return result

    def saved(self, label, wire):
        path = self.work / (label + ".json")
        path.write_bytes(wire)
        return path

    def one_shot_refused(self, command, wire, diagnostic):
        path = self.saved("SECRET_INPUT", wire)
        result = self.run_binary(command, path)
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, diagnostic + b"\n")
        self.assertNotIn(b"SECRET", result.stderr)

    def test_two_saved_text_generations_export_complete_html_then_independent_receipt(self):
        prior_receipt = None
        for phase in ("A", "B"):
            title = '<First> & "quote" 日本語'
            body = "\n公開本文 <script>& 'one'\r\nnext\t日本語😀 " + phase
            posts = [{"target": "/article?x=1&y=2", "title": title, "body_text": body},
                     {"target": "/empty", "title": "", "body_text": ""}]
            source = self.saved("posts", json.dumps({"format_version": 1, "posts": posts}, ensure_ascii=False).encode())
            prepared = self.run_binary("prepare-posts", source)
            self.assertEqual((prepared.returncode, prepared.stderr), (0, b""))
            self.assertTrue(prepared.stdout.endswith(b"\n"))
            self.assertEqual(len(prepared.stdout.splitlines()), 1)
            document = json.loads(prepared.stdout)
            self.assertEqual(set(document), {"format_version", "pages"})
            self.assertEqual(document["format_version"], 1)
            self.assertEqual(len(document["pages"]), 2)
            for post, page in zip(posts, document["pages"]):
                self.assertEqual(set(page), {"target", "status", "content_type", "body"})
                self.assertEqual((page["target"], page["status"], page["content_type"]), (post["target"], 200, "text/html; charset=utf-8"))
                parser = SampleTextParser()
                parser.feed(page["body"])
                self.assertEqual(parser.values, {"title": post["title"], "heading": post["title"], "body": post["body_text"]})
                self.assertNotIn("script", parser.tags)
            self.assertIn("&lt;script&gt;", document["pages"][0]["body"])
            self.assertIn("&#13;", document["pages"][0]["body"])
            payload = self.saved("prepared", prepared.stdout)
            checked = self.run_binary("check-bundle", payload)
            self.assertEqual((checked.returncode, checked.stderr), (0, b""))
            summary = json.loads(checked.stdout)
            self.assertEqual(summary["accepted_bundle_version"], 1)
            self.assertEqual(summary["pages"], 2)
            self.assertEqual(summary["body_bytes"], sum(len(page["body"].encode()) for page in document["pages"]))
            receipt = self.run_binary("bundle-receipt", payload)
            self.assertEqual((receipt.returncode, receipt.stderr), (0, b""))
            self.assertTrue(receipt.stdout.endswith(b"\n"))
            self.assertLessEqual(len(receipt.stdout), 1024)
            identity = json.loads(receipt.stdout)
            self.assertEqual(identity, {"format_version": 1, "payload_bytes": len(prepared.stdout), "sha256": hashlib.sha256(prepared.stdout).hexdigest()})
            if prior_receipt is not None:
                self.assertNotEqual(identity["sha256"], prior_receipt["sha256"])
            prior_receipt = identity

    def test_preparation_refuses_malformed_private_unknown_fields_bounds_and_no_partial_stdout(self):
        good = {"target": "/", "title": "Public", "body_text": SECRET}
        bad = [
            {"format_version": 1, "posts": [{**good, "private": True}]},
            {"format_version": 1, "posts": [good, {**good, "target": "/later", "password": SECRET}]},
            {"format_version": 1, "posts": [{**good, "body_text": None}]},
            {"format_version": 1, "posts": [{**good, "body_text": "\0" + SECRET}]},
            {"format_version": 1, "posts": [good, {**good, "target": "/health/ready?q=1"}]},
            {"format_version": 1, "posts": [good, good]},
            {"format_version": 2, "posts": [good]},
            {"format_version": 1.0, "posts": [good]},
            {"format_version": 1, "posts": []},
        ]
        for value in bad:
            with self.subTest(value=value):
                self.one_shot_refused("prepare-posts", json.dumps(value, ensure_ascii=False).encode(), b"post preparation failed")
        for wire in (b'{"format_version":1,"format_version":1,"posts":[]}', b'not JSON SECRET', b" " * (8 * 1024 * 1024 + 1)):
            self.one_shot_refused("prepare-posts", wire, b"post preparation failed")
        # Small enough input; expansion itself exceeds the existing HTML output cap.
        expanded = {"format_version": 1, "posts": [{**good, "body_text": "<" * (2 * 1024 * 1024 + 1)}]}
        self.one_shot_refused("prepare-posts", json.dumps(expanded).encode(), b"post preparation failed")

    def test_receipt_refuses_invalid_payload_or_arguments_without_body_or_path_diagnostics(self):
        for wire in (b"SECRET_NOT_A_BUNDLE", b'{"format_version":1,"pages":[],"SECRET_UNKNOWN":true}', b" " * (8 * 1024 * 1024 + 1)):
            self.one_shot_refused("bundle-receipt", wire, b"bundle receipt failed")
        for command, diagnostic in (("prepare-posts", b"post preparation failed"), ("bundle-receipt", b"bundle receipt failed")):
            for args in ([command], [command, "SECRET_MISSING"], [command, "SECRET_MISSING", "extra"], [command, "missing", "--listen", "127.0.0.1:0"]):
                with self.subTest(args=args):
                    result = self.run_binary(*args)
                    self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", diagnostic + b"\n"))

    def test_receipt_mismatch_rejects_public_startup_before_listening_without_exposing_data(self):
        payload_wire = json.dumps({"format_version": 1, "pages": [{"target": "/SECRET_TARGET", "status": 200, "content_type": "text/plain; charset=utf-8", "body": SECRET}]}).encode()
        payload = self.saved("SECRET_PAYLOAD_PATH", payload_wire)
        issued = self.run_binary("bundle-receipt", payload)
        self.assertEqual((issued.returncode, issued.stderr), (0, b""))
        identity = json.loads(issued.stdout)
        identity["sha256"] = "0" * 64
        receipt = self.saved("SECRET_RECEIPT_PATH", json.dumps(identity).encode())
        result = self.run_binary("public", "--host", "example.test", "--listen", "127.0.0.1:0", "--bundle", payload, "--receipt", receipt)
        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(result.stdout, b"")
        self.assertNotIn(b"listening", result.stderr)
        self.assertNotIn(b"SECRET", result.stderr)
        self.assertTrue(b"PayloadDigestMismatch" in result.stderr or b"bundle payload digest does not match receipt" in result.stderr, result.stderr)
        for args in (["public", "--host", "example.test", "--receipt", "SECRET_RECEIPT"],
                     ["management", "--receipt", "SECRET_RECEIPT"],
                     ["public", "--host", "example.test", "--bundle", "SECRET_BUNDLE", "--receipt", "one", "--receipt", "SECRET_TWO"]):
            result = self.run_binary(*args)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b"")
            self.assertNotIn(b"SECRET", result.stderr)
            self.assertNotIn(b"listening", result.stderr)


if __name__ == "__main__":
    unittest.main()
