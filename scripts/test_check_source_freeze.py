"""Synthetic source trees only: no Cargo/PHP/DB or full gate is executed."""
from contextlib import redirect_stdout, redirect_stderr
import ast
import io
import os
from pathlib import Path
import shutil
import stat
import subprocess
import tempfile
import types
import unittest
from unittest.mock import patch

import check

WORKSPACE = Path(__file__).resolve().parents[1]
TEMP_PARENT = WORKSPACE / "target"


class SourceFreezeTests(unittest.TestCase):
    def fixture(self):
        TEMP_PARENT.mkdir(exist_ok=True)
        parent = TEMP_PARENT.resolve(strict=True)
        if parent != WORKSPACE / "target" or check._source_link(TEMP_PARENT.lstat()):
            raise AssertionError("synthetic fixture parent must be the workspace target directory")
        root = Path(tempfile.mkdtemp(prefix="source-freeze-unit-", dir=parent))
        def cleanup():
            resolved = root.resolve(strict=True)
            if (resolved.parent != parent or not resolved.is_relative_to(WORKSPACE / "target")
                    or check._source_link(root.lstat())):
                raise AssertionError("synthetic fixture cleanup escaped its owned target directory")
            shutil.rmtree(resolved)
        self.addCleanup(cleanup)
        self.write(root, "Cargo.toml", b"[workspace]\n")
        self.write(root, "Cargo.lock", b"# synthetic lock\n")
        self.write(root, "compatibility/registry.json", b"{}\n")
        self.write(root, "crates/sample/src/lib.rs", b"pub fn sample() {}\n")
        return root

    def write(self, root, name, data):
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def test_selected_inputs_include_untracked_extensions_fixtures_and_non_evidence_metadata(self):
        root = self.fixture()
        expected = ["scripts/tool.py", "tools/app.js", "experiments/probe/Cargo.lock",
                    "examples/input.json", "compatibility/wordpress-default-html-entities.json",
                    "compatibility/schema/reference.json", "compatibility/schema/reference.sql",
                    "compatibility/dependency-inventory.json", "compatibility/dependency-license-evidence.json",
                    "crates/sample/testdata.unknown", "scripts/target"]
        for name in expected: self.write(root, name, b"PRIVATE synthetic input\n")
        snapshot = check._snapshot_sources(root)
        for name in expected:
            self.assertIn(name, snapshot)
            self.assertEqual(snapshot[name][0], len(b"PRIVATE synthetic input\n"))
            self.assertEqual(len(snapshot[name][1]), 64)
        self.assertIn("Cargo.toml", snapshot)
        self.assertIn("Cargo.lock", snapshot)

    def test_edit_add_delete_are_detected_and_failed_verification_never_rebases(self):
        for mutation in ("edit", "add", "delete"):
            root = self.fixture()
            guard = check.SourceFreeze(root)
            if mutation == "edit": self.write(root, "crates/sample/src/lib.rs", b"pub fn changed() {}\n")
            elif mutation == "add": self.write(root, "scripts/untracked-new.py", b"new code\n")
            else: (root / "crates/sample/src/lib.rs").unlink()
            for _ in range(2):
                with self.assertRaises(check.SourceFreezeError) as caught: guard.verify()
                self.assertEqual(str(caught.exception), check.SOURCE_FAILURE)
            # A distinct invocation may capture the changed tree; it must not
            # replace the original invocation's baseline.
            check.SourceFreeze(root).verify()
            with self.assertRaises(check.SourceFreezeError): guard.verify()

    def test_optional_build_config_new_appearance_and_removal_are_detected(self):
        for name in (".cargo/config", ".cargo/config.toml", "rustfmt.toml", ".rustfmt.toml",
                     "rust-toolchain", "rust-toolchain.toml"):
            root = self.fixture()
            before = check.SourceFreeze(root)
            config = self.write(root, name, b"synthetic build configuration\n")
            with self.assertRaises(check.SourceFreezeError): before.verify()
            after = check.SourceFreeze(root)
            config.unlink()
            with self.assertRaises(check.SourceFreezeError): after.verify()

    def test_generated_outputs_docs_and_caches_can_change_but_registry_cannot(self):
        root = self.fixture()
        guard = check.SourceFreeze(root)
        for name in ("target/build.log", "scripts/__pycache__/module.pyc", "tools/node_modules/module/code.js",
                     "experiments/probe/target/output", "compatibility/evidence/current.json",
                     "docs/note.md", "scripts/docs/notes.txt", "crates/notes.md", "scripts/loose.pyc",
                     ".reference/db-runs/fixture/server.log", ".git/ignored", ".cargo/credentials.toml"):
            self.write(root, name, b"allowed generated change\n")
        guard.verify()
        self.write(root, "compatibility/registry.json", b'{"PRIVATE":true}\n')
        with self.assertRaises(check.SourceFreezeError): guard.verify()

    def test_file_count_scan_depth_file_size_and_total_bounds_are_fail_closed(self):
        root = self.fixture()
        snapshot = check._snapshot_sources(root)
        total = sum(length for length, _ in snapshot.values())
        for constant, value in [("MAX_SOURCE_FILES", len(snapshot) - 1),
                                ("MAX_SOURCE_SCAN_ENTRIES", 0), ("MAX_SOURCE_DEPTH", 0),
                                ("MAX_SOURCE_TOTAL_BYTES", total - 1)]:
            with patch.object(check, constant, value), self.assertRaises(check.SourceFreezeError):
                check.SourceFreeze(root)
        self.write(root, "scripts/large.py", b"x" * 65)
        with patch.object(check, "MAX_SOURCE_FILE_BYTES", 64), self.assertRaises(check.SourceFreezeError):
            check.SourceFreeze(root)

    def test_nonregular_and_reparse_files_are_refused_before_open_and_visible_mutation_is_refused(self):
        root = self.fixture()
        path = self.write(root, "scripts/input.py", b"public\n")
        for info in [types.SimpleNamespace(st_mode=stat.S_IFIFO, st_file_attributes=0),
                     types.SimpleNamespace(st_mode=stat.S_IFREG, st_file_attributes=0x400)]:
            with patch.object(check.os, "open") as opened, self.assertRaises(check.SourceFreezeError):
                check._source_hash(path, info, 1024)
            opened.assert_not_called()
        real_read = os.read
        mutated = False
        def read(descriptor, count):
            nonlocal mutated
            chunk = real_read(descriptor, count)
            if not mutated:
                mutated = True
                with path.open("ab") as stream: stream.write(b"changed")
            return chunk
        before = path.lstat()
        with patch.object(check.os, "read", side_effect=read), self.assertRaises(check.SourceFreezeError):
            check._source_hash(path, before, 1024)

    def test_detected_real_symlink_is_not_followed(self):
        root = self.fixture()
        linked = root / "scripts/link.py"
        linked.parent.mkdir(exist_ok=True)
        try: linked.symlink_to(root / "Cargo.lock")
        except (OSError, NotImplementedError): self.skipTest("this host does not permit test symlinks")
        with self.assertRaises(check.SourceFreezeError): check.SourceFreeze(root)

    def test_relative_path_key_bound_is_checked_before_file_read(self):
        root = self.fixture()
        relative = "scripts/" + "x" * 32 + ".py"
        self.write(root, relative, b"public source\n")
        with patch.object(check, "MAX_SOURCE_PATH_BYTES", len(relative.encode())):
            check.SourceFreeze(root)
        with patch.object(check, "MAX_SOURCE_PATH_BYTES", len(relative.encode()) - 1):
            with self.assertRaises(check.SourceFreezeError): check.SourceFreeze(root)

    def test_cli_command_failure_preserves_exit_status_and_displays_only_fixed_drift_note(self):
        failure = subprocess.CalledProcessError(23, ["PRIVATE tool argument"])
        failure.add_note(check.SOURCE_FAILURE_NOTE)
        def main(): raise failure
        # Execute the actual entrypoint block without executing its command sequence.
        tree = ast.parse(Path(check.__file__).read_text(encoding="utf-8"))
        block = ast.Module(body=[tree.body[-1]], type_ignores=[])
        namespace = dict(vars(check), __name__="__main__", main=main)
        with redirect_stderr(io.StringIO()) as stderr:
            with self.assertRaises(SystemExit) as caught:
                exec(compile(block, check.__file__, "exec"), namespace)
        self.assertEqual(caught.exception.code, 23)
        self.assertEqual(stderr.getvalue(), check.SOURCE_FAILURE_NOTE + "\n")

    def test_command_failure_with_drift_preserves_original_status_and_adds_only_fixed_note(self):
        root = self.fixture()
        failure = subprocess.CalledProcessError(23, ["synthetic tool"])
        def run(_):
            self.write(root, "scripts/new.py", b"PRIVATE changed source\n")
            raise failure
        with patch.object(check, "ROOT", root), patch("sys.argv", ["check.py"]), patch.object(check, "run", side_effect=run) as commands, redirect_stdout(io.StringIO()) as output:
            with self.assertRaises(subprocess.CalledProcessError) as caught: check.main()
        self.assertIs(caught.exception, failure)
        self.assertEqual(caught.exception.returncode, 23)
        self.assertEqual(caught.exception.__notes__, [check.SOURCE_FAILURE_NOTE])
        self.assertEqual(commands.call_count, 1)
        self.assertNotIn("Requested development checks passed", output.getvalue())

    def test_post_command_and_final_only_drift_prevent_success(self):
        for final_only in (False, True):
            root = self.fixture()
            actual = check.SourceFreeze(root)
            last_command = False
            mutated = False
            class Guard:
                def verify(inner):
                    nonlocal mutated
                    actual.verify()
                    if final_only and last_command and not mutated:
                        mutated = True
                        self.write(root, "scripts/late.py", b"late source edit\n")
            def run(command):
                nonlocal last_command
                if final_only:
                    last_command = command[1:] == ["scripts/check_database_evidence.py"]
                else: self.write(root, "scripts/changed.py", b"changed\n")
            with patch.object(check, "ROOT", root), patch.object(check, "SourceFreeze", return_value=Guard()), patch("sys.argv", ["check.py"]), patch.object(check, "run", side_effect=run) as commands, redirect_stdout(io.StringIO()) as output:
                with self.assertRaises(check.SourceFreezeError): check.main()
            self.assertNotIn("Requested development checks passed", output.getvalue())
            if final_only: self.assertTrue(last_command)
            else: self.assertEqual(commands.call_count, 1)

    def test_help_has_no_source_snapshot_or_child_and_nested_main_does_not_replace_outer_baseline(self):
        with patch("sys.argv", ["check.py", "--help"]), patch.object(check, "SourceFreeze") as guard, patch.object(check.os, "scandir") as scanned, patch.object(check.os, "open") as opened, patch.object(check.subprocess, "run") as child, redirect_stdout(io.StringIO()):
            with self.assertRaises(SystemExit) as caught: check.main()
        self.assertEqual(caught.exception.code, 0)
        guard.assert_not_called(); scanned.assert_not_called(); opened.assert_not_called(); child.assert_not_called()
        outer, inner = self.fixture(), self.fixture()
        entered = False
        def run(_):
            nonlocal entered
            self.assertFalse(entered)
            entered = True
            with patch.object(check, "ROOT", inner), patch.object(check, "run"), redirect_stdout(io.StringIO()):
                check.main()
            self.write(outer, "scripts/outer-only.py", b"outer changed after nested gate\n")
        with patch.object(check, "ROOT", outer), patch("sys.argv", ["check.py"]), patch.object(check, "run", side_effect=run), redirect_stdout(io.StringIO()):
            with self.assertRaises(check.SourceFreezeError): check.main()
        self.assertTrue(entered)

    def test_legitimate_generated_command_outputs_do_not_contaminate_gate_baseline(self):
        root = self.fixture()
        def run(_):
            self.write(root, "compatibility/evidence/current.json", b"regenerated observation\n")
            self.write(root, "target/build.log", b"build output\n")
            self.write(root, "scripts/__pycache__/changed.pyc", b"cached Python\n")
        with patch.object(check, "ROOT", root), patch("sys.argv", ["check.py"]), patch.object(check, "run", side_effect=run), redirect_stdout(io.StringIO()) as output:
            check.main()
        self.assertIn("Requested development checks passed", output.getvalue())


if __name__ == "__main__": unittest.main()
