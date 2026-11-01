"""Finite offline package CLI contracts; no HTTP or database startup."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

from owned_probe import reap_owned_probes, run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
LIMIT = 8 * 1024 * 1024
SECRET = "SECRET_PACKAGE_BODY_日本語"


class PostPackageCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        result = subprocess.run(
            ["cargo", "build", "-p", "recastpress-server", "--bin", "recastpress",
             "--offline", "--locked", "--message-format=json"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120,
            check=True, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
        )
        items = [json.loads(line) for line in result.stdout.splitlines()]
        paths = [item["executable"] for item in items
                 if item.get("reason") == "compiler-artifact"
                 and item.get("target", {}).get("name") == "recastpress"
                 and "bin" in item.get("target", {}).get("kind", [])
                 and item.get("executable")]
        if len(paths) != 1 or not Path(paths[0]).is_file():
            raise AssertionError("Cargo did not identify exactly one current binary")
        cls.binary = Path(paths[0]).resolve()

    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="post-package-cli-", dir=ROOT / "target")
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(reap_owned_probes)
        self.work = Path(self.directory.name)

    def run_binary(self, *args):
        _, result = run_captured_probe(
            [str(self.binary), *map(os.fspath, args)], os.environ.copy(), timeout=10)
        return result

    def source(self, wire=None, name="SECRET_INPUT.json"):
        path = self.work / name
        if wire is None:
            wire = json.dumps({"format_version": 1, "posts": [{
                "target": "/package?x=1&y=2", "title": '<Title> & "日本語"',
                "body_text": "\n" + SECRET + " <script> & 'text'\r\nnext\t😀",
            }]}, ensure_ascii=False).encode()
        path.write_bytes(wire)
        return path

    def refused(self, *args):
        result = self.run_binary("prepare-post-package", *args)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (2, b"", b"post package preparation failed\n"))
        self.assertNotIn(b"SECRET", result.stderr)

    def test_success_exact_bundle_receipt_and_distinct_generations(self):
        previous = None
        for phase in ("A", "A", "B"):
            source = self.source(name="input-" + phase + ".json")
            value = json.loads(source.read_bytes())
            value["posts"][0]["body_text"] += phase
            source.write_bytes(json.dumps(value, ensure_ascii=False).encode())
            destination = self.work / ("package-" + str(len(list(self.work.glob("package-*")))))
            result = self.run_binary("prepare-post-package", source, destination)
            self.assertEqual((result.returncode, result.stdout, result.stderr),
                             (0, b"post package prepared\n", b""))
            self.assertEqual({path.name for path in destination.iterdir()}, {"bundle.json", "receipt.json"})
            payload = (destination / "bundle.json").read_bytes()
            receipt_wire = (destination / "receipt.json").read_bytes()
            prepared = self.run_binary("prepare-posts", source)
            self.assertEqual((prepared.returncode, prepared.stderr), (0, b""))
            self.assertEqual(payload, prepared.stdout)
            receipt = json.loads(receipt_wire)
            self.assertEqual(receipt, {"format_version": 1, "payload_bytes": len(payload),
                                       "sha256": hashlib.sha256(payload).hexdigest()})
            independent = self.run_binary("bundle-receipt", destination / "bundle.json")
            self.assertEqual((independent.returncode, independent.stdout, independent.stderr),
                             (0, receipt_wire, b""))
            checked = self.run_binary("check-bundle", destination / "bundle.json")
            self.assertEqual((checked.returncode, checked.stderr), (0, b""))
            self.assertEqual(json.loads(checked.stdout)["pages"], 1)
            if previous is not None:
                if phase == "A":
                    self.assertEqual((payload, receipt_wire), previous)
                else:
                    self.assertNotEqual(payload, previous[0])
                    self.assertNotEqual(receipt["sha256"], json.loads(previous[1])["sha256"])
            previous = (payload, receipt_wire)

    def test_invalid_input_and_caps_are_refused_before_directory_creation(self):
        good = {"target": "/", "title": "", "body_text": SECRET}
        bad = [b"SECRET not JSON", b'{"format_version":1,"format_version":1,"posts":[]}',
               b" " * (LIMIT + 1)]
        values = [{"format_version": 1, "posts": [{**good, key: SECRET}]}
                  for key in ("private", "password", "status")]
        values += [
            {"format_version": 1, "posts": [good, good]},
            {"format_version": 1, "posts": [{**good, "target": "/health/ready?q=1"}]},
            {"format_version": 1, "posts": [{**good, "body_text": None}]},
            {"format_version": 1, "posts": [{**good, "body_text": "\0" + SECRET}]},
            {"format_version": 1, "posts": []},
            {"format_version": 2, "posts": [good]},
            {"format_version": 1, "posts": [{**good, "body_text": "<" * (2 * 1024 * 1024 + 1)}]},
        ]
        bad += [json.dumps(value).encode() for value in values]
        for index, wire in enumerate(bad):
            with self.subTest(case=index):
                source = self.source(wire)
                destination = self.work / ("SECRET_rejected_" + str(index))
                self.refused(source, destination)
                self.assertFalse(destination.exists())
                self.assertEqual(source.read_bytes(), wire)

    def test_arguments_and_input_or_parent_io_failures_are_opaque(self):
        source = self.source()
        destination = self.work / "SECRET_new"
        for args in ((), (source,), (source, destination, "extra"),
                     (self.work / "SECRET_missing", destination), (self.work, destination)):
            with self.subTest(args=args):
                self.refused(*args)
                self.assertFalse(destination.exists())
        self.refused(source, self.work / "SECRET_missing_parent" / "new")
        parent_file = self.work / "SECRET_parent_file"
        parent_file.write_bytes(b"parent canary")
        self.refused(source, parent_file / "new")
        self.assertEqual(parent_file.read_bytes(), b"parent canary")

    def test_existing_directory_file_and_input_paths_are_never_overwritten(self):
        source = self.source()
        original = source.read_bytes()
        empty = self.work / "empty"
        empty.mkdir()
        populated = self.work / "populated"
        populated.mkdir()
        (populated / "bundle.json").write_bytes(b"bundle canary")
        (populated / "receipt.json").write_bytes(b"receipt canary")
        existing = self.work / "file"
        existing.write_bytes(b"file canary")
        for destination in (empty, populated, existing, source):
            self.refused(source, destination)
        self.assertEqual(list(empty.iterdir()), [])
        self.assertEqual({p.name: p.read_bytes() for p in populated.iterdir()},
                         {"bundle.json": b"bundle canary", "receipt.json": b"receipt canary"})
        self.assertEqual(existing.read_bytes(), b"file canary")
        self.assertEqual(source.read_bytes(), original)

    def test_existing_symlinks_are_refused_without_touching_their_targets(self):
        source = self.source()
        target = self.work / "target-directory"
        target.mkdir()
        canary = target / "canary"
        canary.write_bytes(b"symlink target canary")
        link = self.work / "SECRET_directory_link"
        try:
            link.symlink_to(target, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest("symlink creation unavailable: " + type(error).__name__)
        file_link = self.work / "SECRET_file_link"
        file_link.symlink_to(canary)
        dangling = self.work / "SECRET_dangling_link"
        dangling.symlink_to(self.work / "missing-target")
        for destination in (link, file_link, dangling):
            self.refused(source, destination)
            self.assertTrue(destination.is_symlink())
        self.assertEqual({p.name for p in target.iterdir()}, {"canary"})
        self.assertEqual(canary.read_bytes(), b"symlink target canary")
        self.assertFalse((self.work / "missing-target").exists())

    def test_repeated_command_refuses_and_preserves_the_completed_pair(self):
        source = self.source()
        destination = self.work / "completed"
        result = self.run_binary("prepare-post-package", source, destination)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, b"post package prepared\n", b""))
        before = {p.name: p.read_bytes() for p in destination.iterdir()}
        self.source(b"SECRET changed malformed input")
        self.refused(source, destination)
        self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)
        valid = self.source()
        self.refused(valid, destination)
        self.assertEqual({p.name: p.read_bytes() for p in destination.iterdir()}, before)


if __name__ == "__main__":
    unittest.main()
