import io
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import wordpress_reference as reference


class ReferenceTests(unittest.TestCase):
    def temporary(self):
        root = (reference.ROOT / "target" / "reference-tests").resolve()
        assert root.is_relative_to(reference.ROOT.resolve())
        root.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=root)
        assert Path(temporary.name).resolve().parent == root
        return temporary

    def archive(self, folder, entries):
        path = Path(folder) / "source.tar.gz"
        with tarfile.open(path, "w:gz") as archive:
            for name, content, kind in entries:
                member = tarfile.TarInfo(name)
                member.type = kind
                member.size = len(content) if kind == tarfile.REGTYPE else 0
                if kind in (tarfile.SYMTYPE, tarfile.LNKTYPE):
                    member.linkname = "../../outside"
                archive.addfile(member, io.BytesIO(content) if member.size else None)
        return path

    def version(self):
        return ("wordpress/wp-includes/version.php", b"<?php\n$wp_version = '7.1.3';\n", tarfile.REGTYPE)

    def test_paths(self):
        for path in ["../outside", "/wordpress/a", "wordpress/../a", "wordpress/./a", "wordpress//a", "wordpress/C:/a", "wordpress/a\\b", "wordpress/NUL.txt", "wordpress/foo.", "wordpress/foo ", "other/a"]:
            with self.subTest(path=path), self.assertRaises(ValueError):
                reference.safe_relative(path)
        self.assertEqual(reference.safe_relative("wordpress/wp-includes/version.php").as_posix(), "wp-includes/version.php")

    def test_extract_records_hashes_and_refuses_existing_destination(self):
        with self.temporary() as folder:
            archive = self.archive(folder, [self.version(), ("wordpress/readme.html", b"readme", tarfile.REGTYPE)])
            destination = Path(folder) / "result"
            records = reference.extract(archive, destination)
            self.assertEqual(len(records), 2)
            self.assertEqual(records[0]["sha256"], reference.digest_file(destination / records[0]["path"]))
            with self.assertRaises(FileExistsError):
                reference.extract(archive, destination)

    def test_links_collisions_and_wrong_versions_fail(self):
        invalid = [
            [self.version(), ("wordpress/link", b"", tarfile.SYMTYPE)],
            [self.version(), ("wordpress/link", b"", tarfile.LNKTYPE)],
            [self.version(), ("wordpress/A", b"a", tarfile.REGTYPE), ("wordpress/a", b"b", tarfile.REGTYPE)],
            [("wordpress/wp-includes/version.php", b"$wp_version = 'other';", tarfile.REGTYPE)],
            [self.version(), ("wordpress/../../outside", b"escape", tarfile.REGTYPE)],
        ]
        for entries in invalid:
            with self.temporary() as folder, self.assertRaises(ValueError):
                reference.extract(self.archive(folder, entries), Path(folder) / "result")

    def test_member_and_expansion_limits(self):
        with self.temporary() as folder:
            archive = self.archive(folder, [self.version()])
            for name, value in [("MAX_MEMBERS", 0), ("MAX_FILE_BYTES", 1), ("MAX_EXPANDED_BYTES", 1)]:
                with self.subTest(limit=name), patch.object(reference, name, value), self.assertRaises(ValueError):
                    reference.extract(archive, Path(folder) / name)


if __name__ == "__main__":
    unittest.main()
