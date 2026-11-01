from pathlib import Path
import stat
import tempfile
import unittest
import zipfile

from pinned_zip_tool import extract_zip
import wordpress_reference as reference


class ZipToolTests(unittest.TestCase):
    def temporary(self):
        root=(reference.ROOT/"target/zip-tool-tests").resolve()
        assert root.is_relative_to(reference.ROOT.resolve())
        root.mkdir(parents=True,exist_ok=True)
        temporary=tempfile.TemporaryDirectory(dir=root)
        assert Path(temporary.name).resolve().parent==root
        return temporary

    def test_fresh_root_stripping_and_file_identity(self):
        with self.temporary() as folder:
            folder=Path(folder);archive=folder/"tool.zip"
            with zipfile.ZipFile(archive,"w") as output: output.writestr("tool/bin/tool.exe",b"fixture, not executable")
            rows=extract_zip(archive,folder/"output",strip_root="tool")
            self.assertEqual(rows,[{"path":"bin/tool.exe","sha256":reference.digest_file(folder/"output/bin/tool.exe")}])
            with self.assertRaises(FileExistsError): extract_zip(archive,folder/"output",strip_root="tool")

    def test_traversal_links_collisions_and_expansion_limits(self):
        for names in [["tool/../../outside"],["tool/NUL.txt"],["tool/A","tool/a"],["wrong/bin/tool.exe"]]:
            with self.subTest(names=names),self.temporary() as folder:
                folder=Path(folder);archive=folder/"tool.zip"
                with zipfile.ZipFile(archive,"w") as output:
                    for name in names: output.writestr(name,b"fixture")
                with self.assertRaises(ValueError):extract_zip(archive,folder/"output",strip_root="tool")
        with self.temporary() as folder:
            folder=Path(folder);archive=folder/"link.zip"
            link=zipfile.ZipInfo("tool/link");link.create_system=3;link.external_attr=(stat.S_IFLNK|0o777)<<16
            with zipfile.ZipFile(archive,"w") as output:output.writestr(link,"../../outside")
            with self.assertRaises(ValueError):extract_zip(archive,folder/"output",strip_root="tool")
            with self.assertRaises(ValueError):extract_zip(archive,folder/"limited",strip_root="tool",max_expanded=1)


if __name__=="__main__":unittest.main()
