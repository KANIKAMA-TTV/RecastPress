import gzip
import io
import json
from pathlib import Path
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import dependency_license_evidence as evidence


class LicenseFileEvidenceTests(unittest.TestCase):
    def setUp(self):
        parent = evidence.ROOT / "target/dependency-license-tests"
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=parent)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.package = self.root / "fixture-1.0.0"
        self.package.mkdir()
        self.archive = self.root / "fixture-1.0.0.crate"
        self.record = {"name": "fixture", "version": "1.0.0", "source": "registry+https://example.invalid/index",
            "declared_license": "MIT OR Apache-2.0", "locked_archive_sha256": ""}

    def build(self, entries, populate=True):
        raw = io.BytesIO()
        with tarfile.open(fileobj=raw, mode="w", format=tarfile.USTAR_FORMAT) as archive:
            for name, body, kind in entries:
                member = tarfile.TarInfo(name)
                member.type = kind
                if kind == tarfile.REGTYPE:
                    member.size = len(body)
                    archive.addfile(member, io.BytesIO(body))
                else:
                    member.linkname = "LICENSE"
                    archive.addfile(member)
                relative = name.removeprefix("fixture-1.0.0/")
                if populate and kind == tarfile.REGTYPE and not relative.startswith(("/", "..")) and ":" not in relative and "\\" not in relative:
                    path = self.package / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(body)
        compressed = gzip.compress(raw.getvalue(), mtime=0)
        self.archive.write_bytes(compressed)
        self.record["locked_archive_sha256"] = evidence.digest(compressed)
        return compressed

    def observe(self):
        return evidence.observe_registry(self.record, self.package, self.archive)

    def test_nested_candidates_exact_hashes_declaration_and_determinism_without_paths(self):
        self.build([(f"fixture-1.0.0/{name}", body, tarfile.REGTYPE) for name, body in
            [("LICENSE-MIT", b"MIT\n"), ("nested/COPYING", b"third party\0\xff"), ("permissions.txt", b"declared"), ("README.md", b"not selected")]])
        self.record["declared_license_file"] = {"path": "permissions.txt", "sha256": evidence.digest(b"declared")}
        actual = self.observe()
        self.assertEqual(actual, self.observe())
        self.assertEqual(actual["checksum_json"], {"status": "missing"})
        self.assertEqual([v["path"] for v in actual["selected_files"]], ["LICENSE-MIT", "nested/COPYING", "permissions.txt"])
        for item in actual["selected_files"]:
            self.assertEqual(item["local_sha256"], item["archive_member_sha256"])
            self.assertEqual(item["status"], "matched")
            self.assertEqual(item["checksum_file_status"], "unavailable")
        self.assertEqual(actual["selected_files"][-1]["selection_reasons"], ["declared_license_file"])
        self.assertNotIn(str(self.root), json.dumps(actual))
        for name in ["license", "LiCeNcE.md", "NOTICE", "COPYRIGHT-foo", "UNLICENSE"]:
            self.assertIsNotNone(evidence.NAME.fullmatch(name))
        for name in ["LICENSES", "LICENSEfake", "NOTICES", "README"]:
            self.assertIsNone(evidence.NAME.fullmatch(name))

    def test_archive_and_local_mismatch_missing_file_and_declared_hash_fail_closed(self):
        compressed = self.build([("fixture-1.0.0/LICENSE", b"original", tarfile.REGTYPE)])
        self.archive.write_bytes(compressed + b"changed")
        with self.assertRaisesRegex(ValueError, "Cargo.lock"):
            self.observe()
        self.archive.write_bytes(compressed)
        (self.package / "LICENSE").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "local file"):
            self.observe()
        (self.package / "LICENSE").unlink()
        with self.assertRaises(OSError):
            self.observe()
        (self.package / "LICENSE").write_bytes(b"original")
        self.record["declared_license_file"] = {"path": "LICENSE", "sha256": "0" * 64}
        with self.assertRaisesRegex(ValueError, "inventory"):
            self.observe()

    def test_traversal_wrong_roots_duplicate_members_and_selected_links_are_rejected(self):
        for path in ["/fixture-1.0.0/LICENSE", "fixture-1.0.0/../LICENSE", "fixture-1.0.0/C:/LICENSE",
                     "fixture-1.0.0/a\\LICENSE", "wrong-1.0.0/LICENSE", "fixture-1.0.0/a//LICENSE"]:
            self.build([(path, b"x", tarfile.REGTYPE)], populate=False)
            with self.subTest(path=path), self.assertRaises(ValueError):
                self.observe()
        self.build([("fixture-1.0.0/LICENSE", b"x", tarfile.REGTYPE)] * 2)
        with self.assertRaisesRegex(ValueError, "duplicate"):
            self.observe()
        for kind in [tarfile.SYMTYPE, tarfile.LNKTYPE, tarfile.DIRTYPE]:
            self.build([("fixture-1.0.0/LICENSE", b"", kind)])
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, "regular"):
                self.observe()

    def test_compressed_expanded_member_candidate_path_and_selected_size_bounds(self):
        compressed = self.build([("fixture-1.0.0/LICENSE", b"1234", tarfile.REGTYPE)])
        for constant, value in [("MAX_ARCHIVE", len(compressed) - 1), ("MAX_EXPANDED", 511),
                                ("MAX_MEMBERS", 0), ("MAX_CANDIDATES", 0), ("MAX_PATH", 10), ("MAX_SELECTED", 3)]:
            with self.subTest(bound=constant), patch.object(evidence, constant, value), self.assertRaises(ValueError):
                self.observe()
        with patch.object(evidence, "MAX_ARCHIVE", len(compressed)), patch.object(evidence, "MAX_SELECTED", 4), patch.object(evidence, "MAX_MEMBERS", 1):
            self.assertEqual(self.observe()["selected_files"][0]["size_bytes"], 4)
        # Expansion bounds cover unselected member data and bytes after tar EOF too.
        self.build([("fixture-1.0.0/README", b"x" * 20000, tarfile.REGTYPE)])
        with patch.object(evidence, "MAX_EXPANDED", 16000), self.assertRaises(ValueError):
            self.observe()
        compressed = self.build([("fixture-1.0.0/README", b"", tarfile.REGTYPE)])
        self.archive.write_bytes(gzip.compress(gzip.decompress(compressed) + b"Z" * 20000, mtime=0))
        self.record["locked_archive_sha256"] = evidence.digest(self.archive.read_bytes())
        with patch.object(evidence, "MAX_EXPANDED", 16000), self.assertRaises(ValueError):
            self.observe()
        # GNU/PAX extension headers count even though tarfile hides them from iteration.
        self.build([("fixture-1.0.0/extension", b"", tarfile.XHDTYPE), ("fixture-1.0.0/LICENSE", b"x", tarfile.REGTYPE)])
        with patch.object(evidence, "MAX_MEMBERS", 1), self.assertRaisesRegex(ValueError, "physical header"):
            self.observe()
        self.build([(f"fixture-1.0.0/extension{i}", b"", tarfile.XHDTYPE) for i in range(34)] +
                   [("fixture-1.0.0/LICENSE", b"x", tarfile.REGTYPE)])
        with self.assertRaisesRegex(ValueError, "nesting"):
            self.observe()

    def test_checksum_json_missing_valid_and_every_malformed_state_fail_without_fallback(self):
        self.build([("fixture-1.0.0/LICENSE", b"MIT", tarfile.REGTYPE)])
        path = self.package / ".cargo-checksum.json"
        valid = {"package": self.record["locked_archive_sha256"], "files": {"LICENSE": evidence.digest(b"MIT")}}
        path.write_text(json.dumps(valid), encoding="utf-8")
        actual = self.observe()
        self.assertEqual(actual["checksum_json"]["status"], "verified")
        self.assertEqual(actual["selected_files"][0]["checksum_file_status"], "matched")
        with patch.object(evidence, "MAX_JSON", path.stat().st_size - 1), self.assertRaises(ValueError):
            self.observe()
        variants = [None, {**valid, "package": "0" * 64}, {**valid, "files": {}},
                    {**valid, "files": {"LICENSE": "0" * 64}}, {**valid, "files": {"../LICENSE": "0" * 64}},
                    {**valid, "files": {"LICENSE": True}}, {**valid, "extra": 1}]
        for wrong in variants:
            path.write_text(json.dumps(wrong), encoding="utf-8")
            with self.subTest(wrong=wrong), self.assertRaises(ValueError):
                self.observe()
        for raw in ["{", '{"package":0,"package":1,"files":{}}']:
            path.write_text(raw, encoding="utf-8")
            with self.assertRaises(ValueError):
                self.observe()

    def test_missing_candidates_remain_observations_but_missing_declared_file_fails(self):
        self.build([("fixture-1.0.0/README.md", b"publisher", tarfile.REGTYPE)])
        actual = self.observe()
        self.assertEqual(actual["discovery_status"], "candidate_not_found")
        self.assertEqual(actual["selected_files"], [])
        self.record["declared_license_file"] = {"path": "permissions.txt", "sha256": "0" * 64}
        with self.assertRaisesRegex(ValueError, "absent"):
            self.observe()

    def test_local_symlink_reparse_and_nonregular_inputs_are_rejected(self):
        self.build([("fixture-1.0.0/LICENSE", b"original", tarfile.REGTYPE)])
        real_lstat = Path.lstat
        selected = self.package / "LICENSE"

        class Reparse:
            st_mode = 0o100644
            st_file_attributes = 0x400

        def lstat(path, *args, **kwargs):
            return Reparse() if path == selected else real_lstat(path, *args, **kwargs)

        with patch.object(Path, "lstat", lstat), self.assertRaisesRegex(ValueError, "reparse"):
            self.observe()
        selected.unlink()
        selected.mkdir()
        with self.assertRaisesRegex(ValueError, "regular"):
            self.observe()
        selected.rmdir()
        # Link creation may require OS privilege; exercise rejection deterministically above.
        outside = self.root / "outside"
        outside.write_bytes(b"original")
        try:
            selected.symlink_to(outside)
        except OSError:
            return
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.observe()

    def test_check_mode_is_read_only_and_stale_inventory_rejected(self):
        output = self.root / "evidence.json"
        value = {"packages": {"fixture": {"archive_status": "local_not_archive"}}}
        wire = json.dumps(value, ensure_ascii=True, indent=2) + "\n"
        output.write_text(wire, encoding="utf-8", newline="\n")
        with patch.object(evidence, "OUTPUT", output), patch.object(evidence, "ROOT", self.root), patch.object(evidence, "evidence", return_value=value), patch("sys.argv", ["tool", "--check"]):
            evidence.main()
            self.assertEqual(output.read_text(encoding="utf-8"), wire)
            output.write_text("{}\n", encoding="utf-8", newline="\n")
            with self.assertRaisesRegex(ValueError, "stale"):
                evidence.main()
            self.assertEqual(output.read_text(encoding="utf-8"), "{}\n")
        for mode in [[], ["--check"]]:
            with self.subTest(mode=mode), patch.object(evidence, "OUTPUT", output), patch.object(evidence, "ROOT", self.root), patch.object(evidence, "evidence", return_value=value), patch.object(evidence, "MAX_JSON", len(wire.encode("utf-8")) - 1), patch("sys.argv", ["tool", *mode]), self.assertRaisesRegex(ValueError, "JSON byte bound"):
                evidence.main()
            self.assertEqual(output.read_bytes(), b"{}\n")
        with patch.object(evidence, "OUTPUT", output), patch.object(evidence, "ROOT", self.root), patch.object(evidence, "evidence", return_value=value), patch.object(evidence, "MAX_JSON", len(wire.encode("utf-8"))), patch("sys.argv", ["tool"]):
            evidence.main()
            self.assertEqual(output.read_bytes(), wire.encode("utf-8"))
        inventory = self.root / "compatibility/dependency-inventory.json"
        inventory.parent.mkdir()
        inventory.write_text("{}", encoding="utf-8")
        with patch.object(evidence, "ROOT", self.root), patch.object(evidence.declarations, "inventory", return_value={"different": True}), self.assertRaisesRegex(ValueError, "stale"):
            evidence.evidence()
        for wrong in [True, 1.0]:
            inventory.write_text(json.dumps({"format_version": wrong}), encoding="utf-8")
            with self.subTest(inventory_version=wrong), patch.object(evidence, "ROOT", self.root), patch.object(evidence.declarations, "inventory", return_value={"format_version": 1}), self.assertRaisesRegex(ValueError, "stale"):
                evidence.evidence()
        identity = "fixture@1.0.0 (registry+https://example.invalid/index)"
        baseline = {"packages": {identity: self.record}, "input_files": {}}
        inventory.write_text(json.dumps(baseline), encoding="utf-8")
        package = {"id": "one", "name": "fixture", "version": "1.0.0", "source": self.record["source"],
                   "license": self.record["declared_license"], "manifest_path": str(self.package / "Cargo.toml")}
        for field, wrong in [("name", "other"), ("version", "2.0.0"), ("source", "registry+https://other.invalid"), ("license", "other")]:
            metadata = {"resolve": {"nodes": [{"id": "one"}]}, "packages": [{**package, field: wrong}]}
            with self.subTest(identity_field=field), patch.object(evidence, "ROOT", self.root), patch.object(evidence.declarations, "inventory", return_value=baseline), patch.object(evidence.subprocess, "run", return_value=SimpleNamespace(stdout=json.dumps(metadata))), self.assertRaisesRegex(ValueError, "identity"):
                evidence.evidence()

    def test_generate_rejects_output_links_reparse_nonregular_and_external_parent(self):
        value = {"packages": {}}
        destination = self.root / "out.json"
        destination.write_bytes(b"existing")
        real_lstat = Path.lstat

        class Reparse:
            st_mode = 0o100644
            st_file_attributes = 0x400

        for substituted in [destination, destination.parent]:
            def lstat(path, *args, **kwargs):
                return Reparse() if path == substituted else real_lstat(path, *args, **kwargs)

            with self.subTest(substituted=substituted.name), patch.object(evidence, "ROOT", self.root), patch.object(evidence, "OUTPUT", destination), patch.object(evidence, "evidence", return_value=value), patch.object(Path, "lstat", lstat), patch("sys.argv", ["tool"]), self.assertRaisesRegex(ValueError, "reparse"):
                evidence.main()
            self.assertEqual(destination.read_bytes(), b"existing")
        destination.unlink()
        destination.mkdir()
        with patch.object(evidence, "ROOT", self.root), patch.object(evidence, "OUTPUT", destination), patch.object(evidence, "evidence", return_value=value), patch("sys.argv", ["tool"]), self.assertRaisesRegex(ValueError, "regular"):
            evidence.main()
        destination.rmdir()
        other = tempfile.TemporaryDirectory(dir=self.root.parent)
        self.addCleanup(other.cleanup)
        external = Path(other.name) / "outside.json"
        external.write_bytes(b"outside")
        with patch.object(evidence, "ROOT", self.root), patch.object(evidence, "OUTPUT", external), patch.object(evidence, "evidence", return_value=value), patch("sys.argv", ["tool"]), self.assertRaisesRegex(ValueError, "workspace"):
            evidence.main()
        self.assertEqual(external.read_bytes(), b"outside")
        destination.write_bytes(b"existing")

        class ManyLinks:
            st_mode = 0o100644
            st_file_attributes = 0
            st_nlink = 2

        def linked_lstat(path, *args, **kwargs):
            return ManyLinks() if path == destination else real_lstat(path, *args, **kwargs)

        # Hardlink creation is unavailable in the sandbox; inject its OS metadata.
        with patch.object(evidence, "ROOT", self.root), patch.object(evidence, "OUTPUT", destination), patch.object(evidence, "evidence", return_value=value), patch.object(Path, "lstat", linked_lstat), patch("sys.argv", ["tool"]), self.assertRaisesRegex(ValueError, "hard links"):
            evidence.main()
        self.assertEqual(destination.read_bytes(), b"existing")
        self.assertEqual(external.read_bytes(), b"outside")
        destination.unlink()
        try:
            destination.symlink_to(external)
        except OSError:
            return
        with patch.object(evidence, "ROOT", self.root), patch.object(evidence, "OUTPUT", destination), patch.object(evidence, "evidence", return_value=value), patch("sys.argv", ["tool"]), self.assertRaisesRegex(ValueError, "symlink"):
            evidence.main()
        self.assertEqual(external.read_bytes(), b"outside")


if __name__ == "__main__":
    unittest.main()
