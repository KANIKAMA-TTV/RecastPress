"""Explicit owned source selections; no PHP execution, network, AI or DB."""
from datetime import datetime, timedelta
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

import migration_source_inventory as inventory
import owned_probe
from owned_probe import reap_owned_probes, run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "scripts/prepare_migration_source_inventory.py"
FILE_LIMIT = 8 * 1024 * 1024


class MigrationSourceInventoryTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="migration-inventory-test-", dir=ROOT / "target")
        self.work = Path(self.directory.name)
        self.root = self.work / "PRIVATE_SOURCE_ROOT"
        self.root.mkdir()
        self.selection = self.work / "PRIVATE_selection.json"
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(reap_owned_probes)

    def source(self, name="entry.php", body=b"\xff\0<?php CANARY_DO_NOT_EXECUTE ?>"):
        path = self.root.joinpath(*name.split("/"))
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)
        return path

    def select(self, files, unconfirmed=()):
        self.selection.write_bytes((json.dumps({"format_version": 1, "files": files, "unconfirmed": list(unconfirmed)}, ensure_ascii=False) + "\n").encode())

    def cli(self, *args):
        _, result = run_captured_probe([sys.executable, str(CLI), *map(os.fspath, args)], os.environ.copy(), timeout=30)
        return result

    def refused(self, destination, root=None, selection=None):
        result = self.cli(root or self.root, selection or self.selection, destination)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"Migration source inventory failed\n"))

    def check_result(self, summary, destination, files, notes=()):
        self.assertEqual(set(summary), {"format_version", "file_count", "total_bytes", "manifest_sha256"})
        for key in ("format_version", "file_count", "total_bytes"):
            self.assertIs(type(summary[key]), int)
        self.assertEqual((summary["format_version"], summary["file_count"]), (1, len(files)))
        self.assertEqual({path.name for path in destination.iterdir()}, {"inventory.json"})
        wire = (destination / "inventory.json").read_bytes()
        self.assertLessEqual(len(wire), 65536)
        self.assertEqual(summary["manifest_sha256"], hashlib.sha256(wire).hexdigest())
        manifest = json.loads(wire)
        self.assertEqual(set(manifest), {"format_version", "scope", "recorded_at_utc", "files", "unconfirmed"})
        self.assertEqual((manifest["format_version"], manifest["scope"]), (1, "manual_selected_migration_source_inventory"))
        self.assertEqual(datetime.fromisoformat(manifest["recorded_at_utc"].replace("Z", "+00:00")).utcoffset(), timedelta(0))
        expected = [{"path": name, "bytes": len(self.root.joinpath(*name.split("/")).read_bytes()),
                     "sha256": hashlib.sha256(self.root.joinpath(*name.split("/")).read_bytes()).hexdigest()} for name in files]
        self.assertEqual(manifest["files"], expected)
        self.assertEqual(manifest["unconfirmed"], list(notes))
        self.assertEqual(summary["total_bytes"], sum(item["bytes"] for item in expected))
        return manifest

    def test_cli_preserves_explicit_selection_order_notes_opaque_source_and_read_only_inputs(self):
        bodies = {"PRIVATE_PLUGIN.php": b"\xff\0<?php CANARY_DO_NOT_EXECUTE ?>", "テーマ/main.php": b"<?php echo 'PRIVATE_CODE'; ?>"}
        for name, body in bodies.items():
            self.source(name, body)
        self.source("unselected.py", b"deliberately invalid executable syntax PRIVATE_UNSELECTED")
        files = ["テーマ/main.php", "PRIVATE_PLUGIN.php"]
        notes = ["PRIVATE_CONTEXT\n未確認", "", "PRIVATE_CONTEXT\n未確認"]
        self.select(files, notes)
        before_selection = self.selection.read_bytes()
        destination = self.work / "result"
        result = self.cli(self.root, self.selection, destination)
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        self.assertEqual(len(result.stdout.splitlines()), 1)
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.check_result(json.loads(result.stdout), destination, files, notes)
        for marker in (b"PRIVATE", str(self.root).encode(), b"CANARY_DO_NOT_EXECUTE"):
            self.assertNotIn(marker, result.stdout)
        for name, body in bodies.items():
            self.assertEqual(self.root.joinpath(*name.split("/")).read_bytes(), body)
        self.assertEqual(self.selection.read_bytes(), before_selection)
        self.assertNotIn(b"PRIVATE_CODE", (destination / "inventory.json").read_bytes())

    def test_public_api_empty_file_and_utf8_note_byte_boundary(self):
        self.source(body=b"")
        note = "é" * 512  # 1024 UTF-8 bytes, not 1024 Unicode scalars.
        self.select(["entry.php"], [note])
        destination = self.work / "empty"
        summary = inventory.inventory_migration_sources(self.root, self.selection, destination)
        self.check_result(summary, destination, ["entry.php"], [note])
        self.assertEqual(summary["total_bytes"], 0)
        self.select(["entry.php"], [note + "x"])
        self.refused(self.work / "note-too-long")
        self.assertFalse((self.work / "note-too-long").exists())

    def test_invalid_selection_shapes_paths_types_and_json_are_opaque(self):
        self.source()
        good = {"format_version": 1, "files": ["entry.php"], "unconfirmed": []}
        values = [{**good, "PRIVATE_unknown": True}, {key: value for key, value in good.items() if key != "unconfirmed"},
                  {**good, "format_version": True}, {**good, "format_version": 1.0}, {**good, "files": []},
                  {**good, "files": [False]}, {**good, "unconfirmed": [False]}, {**good, "unconfirmed": ["bad\0note"]}]
        paths = ["", "/entry.php", "./entry.php", "../entry.php", "x/../entry.php", "x/./entry.php", "x//entry.php", "entry.php/", "x\\entry.php", "C:entry.php", "x:y", "bad\npath", "x" * 1025]
        if os.name == "nt":
            paths += ["entry.php.", "entry.php "]
        values += [{**good, "files": [name]} for name in paths]
        values.append({**good, "files": ["entry.php", "entry.php"]})
        if os.name == "nt":
            values.append({**good, "files": ["entry.php", "ENTRY.PHP"]})
        for index, value in enumerate(values):
            with self.subTest(case=index):
                self.selection.write_bytes(json.dumps(value).encode())
                destination = self.work / str(index)
                self.refused(destination)
                self.assertFalse(destination.exists())
        wire = json.dumps(good).encode()
        for raw in (b"\xff", b"\xef\xbb\xbf" + wire, wire.replace(b'"format_version": 1', b'"format_version": 1, "format_version": 1'), b'{"format_version":NaN}'):
            self.selection.write_bytes(raw)
            self.refused(self.work / "bad-json")

    def test_selection_file_count_notes_and_json_caps_and_manifest_cap_before_mkdir(self):
        names = [f"f{index:03}.php" for index in range(256)]
        for name in names:
            self.source(name, b"")
        self.select(names, ["note"] * 64)
        destination = self.work / "max-counts"
        self.check_result(inventory.inventory_migration_sources(self.root, self.selection, destination), destination, names, ["note"] * 64)
        self.select(names + ["missing-257.php"])
        self.refused(self.work / "too-many-files")
        self.select(names[:1], ["note"] * 65)
        self.refused(self.work / "too-many-notes")
        self.select(names[:1])
        raw = self.selection.read_bytes()
        self.selection.write_bytes(raw + b" " * (65536 - len(raw)))
        exact = self.work / "selection-exact"
        self.check_result(inventory.inventory_migration_sources(self.root, self.selection, exact), exact, names[:1])
        with self.selection.open("ab") as stream:
            stream.write(b" ")
        self.refused(self.work / "selection-excess")
        # Selection remains <64 KiB, while the completed per-file hash records
        # plus notes exceed the independent 64 KiB manifest budget.
        long_names = [f"{index:03}-" + "a" * 156 for index in range(256)]
        for name in long_names:
            self.source(name, b"")
        self.select(long_names, ["n" * 1000] * 4)
        self.assertLess(len(self.selection.read_bytes()), 65536)
        destination = self.work / "manifest-excess"
        self.refused(destination)
        self.assertFalse(destination.exists())

    def test_per_file_and_total_exact_and_one_over_without_reading_unselected_files(self):
        body = b"\0\xffAB" * (FILE_LIMIT // 4)
        self.source("one.bin", body)
        unselected = self.root / "UNSELECTED_TOO_BIG.bin"
        with unselected.open("wb") as stream:
            stream.truncate(FILE_LIMIT + 1)
        self.select(["one.bin"])
        exact = self.work / "file-exact"
        self.check_result(inventory.inventory_migration_sources(self.root, self.selection, exact), exact, ["one.bin"])
        with (self.root / "one.bin").open("ab") as stream:
            stream.write(b"X")
        self.refused(self.work / "file-excess")
        for index in range(4):
            self.source(f"total-{index}.bin", body)
        names = [f"total-{index}.bin" for index in range(4)]
        self.select(names)
        exact_total = self.work / "total-exact"
        summary = inventory.inventory_migration_sources(self.root, self.selection, exact_total)
        self.check_result(summary, exact_total, names)
        self.assertEqual(summary["total_bytes"], 32 * 1024 * 1024)
        self.source("extra.bin", b"X")
        self.select(names + ["extra.bin"])
        destination = self.work / "total-excess"
        self.refused(destination)
        self.assertFalse(destination.exists())

    def test_existing_output_is_refused_before_reads_and_arguments_or_nonregular_inputs_fail(self):
        self.source()
        self.select(["entry.php"])
        destination = self.work / "existing"
        destination.mkdir()
        canary = destination / "inventory.json"
        canary.write_bytes(b"PRIVATE_CANARY")
        self.refused(destination)
        self.assertEqual(canary.read_bytes(), b"PRIVATE_CANARY")
        with patch.object(inventory, "_read_regular", side_effect=AssertionError("source read before output admission")) as read:
            with self.assertRaises(Exception):
                inventory._inventory_worker(self.root, self.selection, destination)
            read.assert_not_called()
        empty = self.work / "empty-existing"
        empty.mkdir()
        file = self.work / "existing-file"
        file.write_bytes(b"FILE_CANARY")
        for output in (empty, file, self.work / "missing-parent" / "new"):
            self.refused(output)
        self.assertEqual(file.read_bytes(), b"FILE_CANARY")
        self.assertEqual(list(empty.iterdir()), [])
        for args in ((), (self.root,), (self.root, self.selection), (self.root, self.selection, self.work / "new", "extra")):
            result = self.cli(*args)
            self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"Migration source inventory failed\n"))
        self.refused(self.work / "no-source", root=self.work / "missing-source")
        self.refused(self.work / "file-source", root=self.root / "entry.php")
        self.refused(self.work / "bad-selection", selection=self.root)
        self.select(["folder"])
        (self.root / "folder").mkdir()
        self.refused(self.work / "directory-leaf")

    def test_detected_root_directory_and_leaf_symlinks_are_refused(self):
        self.source()
        self.select(["entry.php"])
        root_link = self.work / "root-link"
        try:
            root_link.symlink_to(self.root, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest("symlink creation unavailable: " + type(error).__name__)
        self.refused(self.work / "linked-root", root=root_link)
        outside = self.work / "outside"
        outside.mkdir()
        (outside / "private.php").write_bytes(b"PRIVATE_OUTSIDE")
        (self.root / "linked-dir").symlink_to(outside, target_is_directory=True)
        self.select(["linked-dir/private.php"])
        self.refused(self.work / "linked-directory")
        (self.root / "linked.php").symlink_to(outside / "private.php")
        self.select(["linked.php"])
        self.refused(self.work / "linked-leaf")
        self.select(["entry.php"])
        output = self.work / "output-link"
        output.symlink_to(outside, target_is_directory=True)
        self.refused(output)
        self.assertEqual({path.name for path in outside.iterdir()}, {"private.php"})

    def test_failed_write_keeps_partial_output_without_success_receipt(self):
        self.source()
        self.select(["entry.php"])
        destination = self.work / "partial"
        request = json.dumps({"source_root": str(self.root), "selection_file": str(self.selection), "new_dir": str(destination)}).encode()
        output = io.BytesIO()

        def fail(path, data):
            with path.open("xb") as stream:
                stream.write(data[:7])
            raise OSError("PRIVATE_WRITE_FAILURE")

        with patch.object(inventory, "_write_file", side_effect=fail), patch.object(inventory.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(request))), patch.object(inventory.sys, "stdout", SimpleNamespace(buffer=output)):
            self.assertEqual(inventory._worker_main(), 2)
        self.assertEqual(output.getvalue(), b"")
        self.assertEqual({path.name for path in destination.iterdir()}, {"inventory.json"})
        self.assertEqual(len((destination / "inventory.json").read_bytes()), 7)
        self.refused(destination)

    def test_timeout_and_tampered_summary_or_pid_are_closed_with_cleanup_retry(self):
        self.source()
        self.select(["entry.php"])
        pid = 6789
        summary = {"format_version": 1, "file_count": 1, "total_bytes": 0, "manifest_sha256": "a" * 64}
        envelopes = [{"process_pid": pid + 1, "summary": summary},
                     {"process_pid": pid, "summary": {**summary, "file_count": True}},
                     {"process_pid": pid, "summary": {**summary, "total_bytes": FILE_LIMIT + 1}},
                     {"process_pid": pid, "summary": {**summary, "PRIVATE_unknown": True}}]
        for envelope in envelopes:
            result = subprocess.CompletedProcess(["mock-worker"], 0, (json.dumps(envelope) + "\n").encode(), b"")
            with patch.object(owned_probe, "run_captured_probe", return_value=(pid, result)), patch.object(owned_probe, "reap_owned_probes") as reap:
                with self.assertRaises(inventory.MigrationSourceInventoryError) as caught:
                    inventory.inventory_migration_sources(self.root, self.selection, self.work / "unused")
                self.assertEqual(str(caught.exception), "Migration source inventory failed")
                self.assertNotIn("PRIVATE", repr(caught.exception))
                reap.assert_called_once()
        with patch.object(owned_probe, "run_captured_probe", side_effect=subprocess.TimeoutExpired(["owned-worker"], 0.2)), patch.object(owned_probe, "reap_owned_probes") as reap:
            with self.assertRaises(inventory.MigrationSourceInventoryError):
                inventory.inventory_migration_sources(self.root, self.selection, self.work / "unused", timeout=0.2)
            reap.assert_called_once()


if __name__ == "__main__":
    unittest.main()
