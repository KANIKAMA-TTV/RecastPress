import tempfile
from pathlib import Path
import unittest

from dependency_inventory import ROOT, package_record


class DependencyInventoryTests(unittest.TestCase):
    def temporary(self):
        root = (ROOT / "target/dependency-inventory-tests").resolve()
        assert root.is_relative_to(ROOT.resolve())
        root.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=root)
        assert Path(temporary.name).resolve().parent == root
        return temporary

    def test_registry_requires_exact_source_version_and_checksum(self):
        source = "registry+https://example.invalid/index"
        package = {"name": "fixture", "version": "1.0.0", "source": source, "manifest_path": __file__, "license": "MIT/Apache-2.0"}
        for locked in [{}, {("fixture", "2.0.0", source): {"checksum": "a" * 64}}, {("fixture", "1.0.0", source): {"checksum": "not-a-hash"}}]:
            with self.assertRaises(ValueError):
                package_record(package, locked)
        _, record = package_record(package, {("fixture", "1.0.0", source): {"checksum": "a" * 64}})
        self.assertEqual(record["declared_license"], "MIT/Apache-2.0")
        self.assertNotIn(str(Path(__file__).resolve().parent), str(record))

    def test_declared_license_file_must_be_within_package(self):
        with self.temporary() as directory:
            root = Path(directory).resolve()
            package_dir = root / "package"
            package_dir.mkdir()
            (root / "outside.txt").write_text("fixture", encoding="utf-8")
            package = {"name": "fixture", "version": "1", "source": None, "manifest_path": str(package_dir / "Cargo.toml"), "license": None, "license_file": "../outside.txt"}
            locked = {("fixture", "1", None): {}}
            with self.assertRaises(ValueError):
                package_record(package, locked, root)
            package["license_file"] = "LICENSE"
            (package_dir / "LICENSE").write_text("fixture license", encoding="utf-8")
            identity, record = package_record(package, locked, root)
            self.assertEqual(identity, "fixture@1 (path:package)")
            self.assertIsNone(record["declared_license"])
            self.assertEqual(record["declared_license_file"]["path"], "LICENSE")
            self.assertNotIn(str(root), str(record))

    def test_external_local_dependency_is_not_recorded_as_repository_content(self):
        with self.temporary() as directory:
            root = Path(directory).resolve()
            package = {"name": "fixture", "version": "1", "source": None, "manifest_path": str(root / "external/Cargo.toml"), "license": "MIT"}
            with self.assertRaises(ValueError):
                package_record(package, {("fixture", "1", None): {}}, root / "repository")


if __name__ == "__main__":
    unittest.main()
