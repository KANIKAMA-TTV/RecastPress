"""Fixed workbench ZIP -> native unpack/check -> owned sealed HTTP.

Node is optional; missing runtime skips explicitly, never installs.
Only fixed public local fixtures; no arbitrary external URL, DB or auth.
"""
import base64
import binascii
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import owned_probe
import smoke
from test_prepared_workbench_package import select_server

NODE = shutil.which("node")
NAMES = ("bundle.json", "receipt.json")
CANARY = "PRIVATE_UNPACK_CANARY"
MAX_ARCHIVE = 8 * 1024 * 1024 + 1024 + 220


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")


def receipt(payload):
    return encoded({"format_version": 1, "payload_bytes": len(payload),
                    "sha256": hashlib.sha256(payload).hexdigest()})


def reframe_actual_js(template, payload, proof):
    """Replace only fixed member contents using actual JS header templates.

    Recompute lengths/CRC/offsets for v2 and strict-admission negative fixtures.
    Main positive fixtures always use actual unmodified JS ZIP bytes.
    """
    local_headers, central_headers = [], []
    at = 0
    for name in NAMES:
        assert struct.unpack_from("<I", template, at)[0] == 0x04034B50
        size = struct.unpack_from("<I", template, at + 18)[0]
        name_size, extra = struct.unpack_from("<HH", template, at + 26)
        assert name_size == len(name) and extra == 0
        local_headers.append(template[at:at + 30 + name_size])
        at += 30 + name_size + size
    for name in NAMES:
        assert struct.unpack_from("<I", template, at)[0] == 0x02014B50
        central_headers.append(template[at:at + 46 + len(name)])
        at += 46 + len(name)
    end = bytearray(template[at:])
    assert len(end) == 22
    output, offsets, crcs = bytearray(), [], []
    for header, body in zip(local_headers, (payload, proof)):
        offsets.append(len(output))
        crc = binascii.crc32(body) & 0xFFFFFFFF
        crcs.append(crc)
        modified = bytearray(header)
        struct.pack_into("<III", modified, 14, crc, len(body), len(body))
        output.extend(modified)
        output.extend(body)
    central_start = len(output)
    for header, body, offset, crc in zip(central_headers, (payload, proof), offsets, crcs):
        modified = bytearray(header)
        struct.pack_into("<III", modified, 16, crc, len(body), len(body))
        struct.pack_into("<I", modified, 42, offset)
        output.extend(modified)
    struct.pack_into("<II", end, 12, len(output) - central_start, central_start)
    output.extend(end)
    return bytes(output)


@unittest.skipUnless(NODE, "installed Node.js unavailable; no ZIP import observation")
class UnpackPackageCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.addClassCleanup(owned_probe.reap_owned_probes)
        cls.build_pid, result = owned_probe.run_captured_probe(
            ["cargo", "build", "--manifest-path", str(ROOT / "Cargo.toml"), "-p",
             "recastpress-server", "--bin", "recastpress", "--offline", "--locked",
             "--message-format=json"], os.environ.copy(), timeout=240)
        if result.returncode != 0:
            raise AssertionError("Current server build failed")
        cls.binary = select_server(result.stdout)

    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix="unpack-package-cli-", dir=ROOT / "target")).resolve()
        self.addCleanup(self.cleanup_owned)
        self.archive_path = self.work / (CANARY + ".zip")
        self.output = self.work / "new-package"
        self.pids = []

    def cleanup_owned(self):
        # Never remove the directory while a retained child/IO worker may use it.
        errors = []
        for reap in (smoke.reap_servers, owned_probe.reap_owned_probes):
            try:
                reap()
            except BaseException as error:
                errors.append(error)
        if errors:
            raise RuntimeError("Owned fixture cleanup incomplete") from None
        if self.work.parent != (ROOT / "target").resolve() or not self.work.name.startswith("unpack-package-cli-"):
            raise AssertionError("Owned cleanup containment failed")
        if self.work.exists():
            shutil.rmtree(self.work)

    def capture(self, args, raw=None, timeout=20, bound=65536):
        pid, result = owned_probe.run_captured_probe(list(map(os.fspath, args)), os.environ.copy(),
                                                  timeout=timeout, input_bytes=raw)
        self.pids.append(pid)
        self.assertLessEqual(len(result.stdout), bound)
        self.assertLessEqual(len(result.stderr), bound)
        return result

    def js_zip(self, posts=None):
        if posts is None:
            posts = [{"target": "/unpack?b=2&a=1", "title": "日本 <&\"'🦀",
                      "body_text": "\r\n<script>literal</script> &copy;\t"}]
        result = self.capture([NODE, str(ROOT / "scripts/workbench_package_probe.js")],
                              encoded({"format_version": 1, "posts": posts}), bound=16 * 1024 * 1024)
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        value = json.loads(result.stdout)
        self.assertIs(value["ok"], True)
        raw = base64.b64decode(value["archive_base64"], validate=True)
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            self.assertEqual(archive.namelist(), list(NAMES))
            self.assertIsNone(archive.testzip())
            files = tuple(archive.read(name) for name in NAMES)
        self.assertEqual(json.loads(files[1]), json.loads(receipt(files[0])))
        return raw, files

    def cli(self, *args):
        result = self.capture([self.binary, *args])
        for private in (CANARY.encode(), str(self.work).encode()):
            self.assertNotIn(private, result.stdout)
            self.assertNotIn(private, result.stderr)
        return result

    def refused(self, result):
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (2, b"", b"package unpack failed\n"))

    def unpacked(self, raw, files, output=None):
        output = output or self.output
        self.archive_path.write_bytes(raw)
        result = self.cli("unpack-package", self.archive_path, output)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (0, b"package unpacked\n", b""))
        self.assertEqual(sorted(path.name for path in output.iterdir()), sorted(NAMES))
        self.assertEqual(tuple((output / name).read_bytes() for name in NAMES), files)
        checked = self.cli("check-package", output / NAMES[0], output / NAMES[1])
        self.assertEqual((checked.returncode, checked.stderr), (0, b""))
        self.assertLessEqual(len(checked.stdout), 1024)
        self.assertEqual(self.archive_path.read_bytes(), raw)

    def test_actual_js_zip_unpacks_exact_two_files_and_passes_existing_checker(self):
        for index, posts in enumerate([
            None,
            [{"target": "/empty", "title": "", "body_text": ""},
             {"target": "/raw?x=1&y=2", "title": "e\u0301 é", "body_text": "\x01\x7f\u2028\u2029"}],
        ]):
            raw, files = self.js_zip(posts)
            self.unpacked(raw, files, self.work / f"accepted-{index}")

    def test_envelope_does_not_restrict_existing_payload_admission_to_ui_v1(self):
        template, _ = self.js_zip()
        payload = encoded({"format_version": 2, "pages": [
            {"target": "/redirect", "status": 308, "location": "/next"},
            {"target": "/text", "status": 200, "content_type": "text/plain; charset=utf-8", "body_hex": "00c3a9"}]})
        files = (payload, receipt(payload))
        self.unpacked(reframe_actual_js(template, *files), files)

    def test_existing_empty_populated_and_non_directory_outputs_are_never_overwritten(self):
        raw, files = self.js_zip()
        self.archive_path.write_bytes(raw)
        empty = self.work / "empty"
        empty.mkdir()
        populated = self.work / "populated"
        populated.mkdir()
        (populated / "canary").write_bytes(b"PRIVATE retained")
        ordinary = self.work / "ordinary"
        ordinary.write_bytes(b"PRIVATE original")
        for output in (empty, populated, ordinary):
            self.refused(self.cli("unpack-package", self.archive_path, output))
        self.assertEqual(list(empty.iterdir()), [])
        self.assertEqual((populated / "canary").read_bytes(), b"PRIVATE retained")
        self.assertEqual(ordinary.read_bytes(), b"PRIVATE original")
        self.unpacked(raw, files)
        self.refused(self.cli("unpack-package", self.archive_path, self.output))
        self.assertEqual(tuple((self.output / name).read_bytes() for name in NAMES), files)

    def test_malformed_crc_headers_names_offsets_and_bounds_refuse_before_output_creation(self):
        raw, _ = self.js_zip()
        mutations = [b"PRIVATE invalid archive", raw[:-1], raw + b"\0", b"\0" * (MAX_ARCHIVE + 1)]
        # Local signature, version, flags/descriptor, compression, time/date,
        # CRC, size, extra length; then central attrs/offset and end count.
        with zipfile.ZipFile(io.BytesIO(raw)) as archive:
            central = archive.start_dir
        for offset in (0, 4, 6, 8, 10, 12, 14, 18, 28,
                       central, central + 4, central + 38, central + 42, len(raw) - 12):
            altered = bytearray(raw)
            altered[offset] ^= 1
            mutations.append(bytes(altered))
        # Both names changed together: failure cannot rely only on disagreement
        # between local and central headers. All paths are public dummy names.
        for name in (b"../evil.txt", b"C:/evil.txt", b"/evil......", b"other.json!"):
            self.assertEqual(len(name), len(b"bundle.json"))
            mutations.append(raw.replace(b"bundle.json", name))
        # Corrupt both CRC declarations coherently while retaining original body.
        corrupt_crc = bytearray(raw)
        wrong = struct.unpack_from("<I", raw, 14)[0] ^ 1
        struct.pack_into("<I", corrupt_crc, 14, wrong)
        struct.pack_into("<I", corrupt_crc, central + 16, wrong)
        mutations.append(bytes(corrupt_crc))
        for index, changed in enumerate(mutations):
            with self.subTest(case=index):
                self.archive_path.write_bytes(changed)
                self.refused(self.cli("unpack-package", self.archive_path, self.output))
                self.assertFalse(self.output.exists())
                self.assertFalse((self.work / "evil.txt").exists())

    def test_valid_envelope_never_bypasses_receipt_identity_or_strict_admission(self):
        template, (valid, proof) = self.js_zip()
        invalid_bundle = encoded({"format_version": 1, "pages": [
            {"target": "/health/ready?x=1", "status": 200,
             "content_type": "text/html; charset=utf-8", "body": CANARY}]})
        for payload, current_proof in (
            (valid + b" ", proof),
            (valid, encoded({**json.loads(proof), "sha256": "0" * 64})),
            (valid, proof.replace(b'"format_version":1', b'"format_version":1,"format_version":1')),
            (b"PRIVATE invalid bundle", receipt(b"PRIVATE invalid bundle")),
            (invalid_bundle, receipt(invalid_bundle)),
        ):
            self.archive_path.write_bytes(reframe_actual_js(template, payload, current_proof))
            self.refused(self.cli("unpack-package", self.archive_path, self.output))
            self.assertFalse(self.output.exists())

    def test_exact_arguments_missing_input_and_help_are_closed(self):
        raw, _ = self.js_zip()
        self.archive_path.write_bytes(raw)
        for args in ((), (self.archive_path,), (self.archive_path, self.output, "extra"),
                     ("", self.output), (self.archive_path, ""),
                     (self.work / "missing.zip", self.output), (self.work, self.output),
                     (self.archive_path, self.work / "missing-parent" / "child")):
            with self.subTest(arg_count=len(args)):
                self.refused(self.cli("unpack-package", *args))
                self.assertFalse(self.output.exists())
        result = self.cli("--help")
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        self.assertIn(b"recastpress unpack-package <zip> <new-dir>", result.stdout)

    def test_unpacked_pair_enters_existing_sealed_http_without_live_file_reads(self):
        # This separate owned fixture is the real end-to-end import relation.
        # Root may execute it only after adopting the command; no external URL.
        paths = [ROOT / "tools/prepared-post-workbench/publication-package.js",
                 ROOT / "tools/prepared-post-workbench/input-parser.js",
                 ROOT / "scripts/workbench_package_probe.js",
                 ROOT / "crates/recastpress-server/src/package_zip.rs",
                 ROOT / "crates/recastpress-server/src/bundle.rs",
                 ROOT / "crates/recastpress-server/src/bundle_receipt.rs",
                 ROOT / "crates/recastpress-server/src/main.rs",
                 ROOT / "crates/recastpress-server/src/cli.rs",
                 ROOT / "Cargo.toml", ROOT / "Cargo.lock", self.binary]
        digest = lambda raw: hashlib.sha256(raw).hexdigest()
        identities = {path.relative_to(ROOT).as_posix(): digest(path.read_bytes()) for path in paths}
        target = "/unpack-sealed?b=2&a=1"
        posts = [{"target": target, "title": "Public imported ZIP", "body_text": "\n<script>文字 & \r\n🦀"}]
        raw, files = self.js_zip(posts)
        self.unpacked(raw, files)
        body = json.loads(files[0])["pages"][0]["body"].encode("utf-8")
        server = smoke.start("public", "--host", "example.test", "--bundle", str(self.output / NAMES[0]),
                             "--receipt", str(self.output / NAMES[1]), binary=self.binary)
        checks = ["unpack", "check_package"]
        try:
            status, headers, data = smoke.request(server.port, target)
            self.assertEqual((status, data), (200, body))
            self.assertEqual(headers["content-type"], "text/html; charset=utf-8")
            self.assertEqual(int(headers["content-length"]), len(body))
            etag = headers["etag"]
            checks.append("get")
            status, headers, data = smoke.request(server.port, target, method="HEAD")
            self.assertEqual((status, data, int(headers["content-length"]), headers["etag"]),
                             (200, b"", len(body), etag))
            checks.append("head")
            self.assertEqual(smoke.request(server.port, target, extra_headers={"If-None-Match": etag})[0], 304)
            checks.append("conditional")
            self.assertEqual(smoke.request(server.port, target, host="wrong.test")[0], 421)
            checks.append("host")
            self.assertEqual(smoke.request(server.port, "/unpack-sealed?a=1&b=2")[0], 404)
            checks.append("query")
            for name in NAMES:
                (self.output / name).write_bytes(b"changed after startup\n")
            status, headers, data = smoke.request(server.port, target)
            self.assertEqual((status, data, headers["etag"]), (200, body, etag))
            checks.append("sealed")
        finally:
            smoke.stop(server, original=sys.exc_info()[1])
        smoke.reap_servers()
        owned_probe.reap_owned_probes()
        self.assertIsNotNone(server.process.returncode)
        self.assertFalse(server.reader.is_alive())
        self.assertEqual({path.relative_to(ROOT).as_posix(): digest(path.read_bytes()) for path in paths}, identities)
        record = {"format_version": 1, "scope": "fixed_js_zip_unpack_native_sealed_http",
                  "build_pid": self.build_pid, "probe_pids": self.pids,
                  "server_pid": server.process.pid, "server_sha256": digest(self.binary.read_bytes()),
                  "archive_sha256": digest(raw), "bundle_sha256": digest(files[0]),
                  "receipt_sha256": digest(files[1]), "body_sha256": digest(body),
                  "checks": checks, "selected_source_hashes": identities,
                  "selected_sources_unchanged": True, "server_exit_code": server.process.returncode,
                  "reader_joined": True, "all_owned_children_reaped": True,
                  "limitations": "Owned public fixture only; no auth/DB/theme/Linux, general ZIP or OS download-save claim."}
        self.cleanup_owned()
        self.assertFalse(self.work.exists())
        record["owned_directory_removed"] = True
        (ROOT / "target/package-unpack-native-observation.json").write_text(
            json.dumps(record, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
