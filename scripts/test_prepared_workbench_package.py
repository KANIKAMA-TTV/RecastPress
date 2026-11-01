"""Main JS ZIP/native bytes and separate sealed HTTP relation.

Fixed local public fixtures; no arbitrary artifact/URL/module or external GET.
Node.js is an optional local test runtime: absent Node produces an explicit skip,
not a successful ZIP/native observation. These tests never install the runtime.
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
import tempfile
import unittest
import zipfile

import owned_probe
import smoke

ROOT = Path(__file__).resolve().parents[1]
NODE = shutil.which('node')
LIMIT = 8 * 1024 * 1024
NAMES = ['bundle.json', 'receipt.json']


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def wire(posts):
    return (json.dumps({'format_version': 1, 'posts': posts}, ensure_ascii=False,
                       separators=(',', ':'))+'\n').encode('utf-8')


def post(body='', title='', target='/zip?b=2&a=1'):
    return {'target': target, 'title': title, 'body_text': body}


def select_server(raw):
    if len(raw) > 16 * 1024 * 1024:
        raise AssertionError('Cargo output bound exceeded')
    manifest = (ROOT/'crates/recastpress-server/Cargo.toml').resolve()
    source = (manifest.parent/'src/main.rs').resolve()
    selected, finished = [], []
    for line in raw.splitlines():
        value = json.loads(line)
        if value.get('reason') == 'build-finished':
            finished.append(value.get('success') is True)
        target = value.get('target', {})
        if (value.get('reason') == 'compiler-artifact' and value.get('executable')
                and Path(value.get('manifest_path', '')).resolve() == manifest
                and target.get('name') == 'recastpress' and target.get('kind') == ['bin']
                and Path(target.get('src_path', '')).resolve() == source):
            selected.append(Path(value['executable']).resolve())
    if finished != [True] or len(selected) != 1 or not selected[0].is_file():
        raise AssertionError('Exactly one current server artifact is required')
    return selected[0]


@unittest.skipUnless(NODE, 'installed Node.js unavailable')
class WorkbenchPackageTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.addClassCleanup(owned_probe.reap_owned_probes)
        pid, result = owned_probe.run_captured_probe(
            ['cargo', 'build', '--manifest-path', str(ROOT/'Cargo.toml'), '-p', 'recastpress-server',
             '--bin', 'recastpress', '--offline', '--locked', '--message-format=json'],
            os.environ.copy(), timeout=240)
        if result.returncode != 0:
            raise AssertionError('Current server build failed')
        cls.build_pid, cls.server = pid, select_server(result.stdout)

    def setUp(self):
        self.work = Path(tempfile.mkdtemp(prefix='workbench-package-', dir=ROOT/'target')).resolve()
        self.pids = []
        self.addCleanup(self.cleanup_owned)

    def cleanup_owned(self):
        errors = []
        for reap in (smoke.reap_servers, owned_probe.reap_owned_probes):
            try:
                reap()
            except BaseException as error:
                errors.append(error)
        if errors:
            # Preserve the owned directory if any process/reader may retain it.
            raise RuntimeError('Owned fixture cleanup incomplete') from None
        if (self.work.parent != (ROOT/'target').resolve()
                or not self.work.name.startswith('workbench-package-')):
            raise AssertionError('Fixture cleanup containment failed')
        if self.work.exists():
            shutil.rmtree(self.work)

    def capture(self, args, raw=None, bound=16*1024*1024, timeout=30):
        pid, result = owned_probe.run_captured_probe(args, os.environ.copy(), timeout=timeout, input_bytes=raw)
        self.pids.append(pid)
        self.assertLessEqual(len(result.stdout), bound)
        return result

    def package(self, raw, accepted=True):
        result = self.capture([NODE, str(ROOT/'scripts/workbench_package_probe.js')],
                              raw)
        self.assertEqual((result.returncode, result.stderr), (0, b''))
        value = json.loads(result.stdout)
        if not accepted:
            self.assertEqual(value, {'ok': False, 'error': 'Package preparation failed'})
            return None
        self.assertEqual(set(value), {'ok', 'archive_base64', 'payloadBytes', 'receiptBytes', 'sha256', 'postCount'})
        self.assertIs(value['ok'], True)
        archive = base64.b64decode(value['archive_base64'], validate=True)
        files = self.unpack(archive)
        self.assertEqual((value['payloadBytes'], value['receiptBytes'], value['sha256']),
                         (len(files[0]), len(files[1]), sha(files[0])))
        self.assertEqual(json.loads(files[1]), {'format_version': 1, 'payload_bytes': len(files[0]), 'sha256': sha(files[0])})
        self.assertEqual(value['postCount'], len(json.loads(files[0])['pages']))
        return files

    def unpack(self, raw):
        with zipfile.ZipFile(io.BytesIO(raw)) as zipped:
            self.assertEqual(zipped.namelist(), NAMES)
            self.assertEqual(zipped.comment, b'')
            self.assertIsNone(zipped.testzip())
            files, offset = [], 0
            for name, info in zip(NAMES, zipped.infolist()):
                body = zipped.read(name)
                self.assertEqual((info.flag_bits, info.compress_type, info.date_time, info.extra, info.comment),
                                 (0, zipfile.ZIP_STORED, (1980, 1, 1, 0, 0, 0), b'', b''))
                self.assertEqual((info.file_size, info.compress_size, info.CRC, info.header_offset),
                                 (len(body), len(body), binascii.crc32(body) & 0xffffffff, offset))
                header = struct.unpack_from('<IHHHHHIIIHH', raw, offset)
                self.assertEqual((header[0], header[2], header[3], header[4], header[5]),
                                 (0x04034b50, 0, 0, 0, 33))
                self.assertEqual(header[6:9], (info.CRC, len(body), len(body)))
                self.assertEqual((header[9], header[10]), (len(name), 0))
                self.assertEqual(raw[offset+30:offset+30+len(name)], name.encode('ascii'))
                offset += 30 + len(name) + len(body)
                files.append(body)
            central_start = offset
            for name in NAMES:
                self.assertEqual(struct.unpack_from('<I', raw, offset)[0], 0x02014b50)
                fields = struct.unpack_from('<IHHHHHHIIIHHHHHII', raw, offset)
                self.assertEqual(fields[10:13], (len(name), 0, 0))
                offset += 46+len(name)
            end = struct.unpack_from('<IHHHHIIH', raw, offset)
            self.assertEqual(end, (0x06054b50, 0, 0, 2, 2, offset-central_start, central_start, 0))
            self.assertEqual(offset+22, len(raw))
            return files

    def cli(self, *args, success=True):
        result = self.capture([str(self.server), *map(str, args)], bound=65536)
        if success:
            self.assertEqual((result.returncode, result.stderr), (0, b''))
        else:
            self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b'', b'package check failed\n'))
        return result

    def write_package(self, files, name):
        directory = self.work/name
        directory.mkdir()
        for filename, raw in zip(NAMES, files):
            (directory/filename).write_bytes(raw)
        return directory

    def test_three_node_suites_use_actual_main_modules(self):
        for name in ('input', 'ui', 'package'):
            result = self.capture([NODE, '--test', str(ROOT/f'scripts/test_prepared_workbench_{name}.js')], timeout=90)
            self.assertEqual((result.returncode, result.stderr), (0, b''))

    def test_json_zip_native_two_file_byte_identity_and_refusals(self):
        texts = ['', '\r\n\t', ''.join(chr(i) for i in range(1, 32)), '\x7f',
                 '日本語🦀', '\u2028\u2029', '\ufdd0\ufffe\U0010ffff', '<script> &copy; \' " & >',
                 '\nfirst\rsecond\tlast', ' SPACE  ', '%41 &amp;', 'é e\u0301']
        for index, text in enumerate(texts):
            posts = [post(text, text, f'/zip-{index}?b=2&a=1')]
            if index == len(texts)-1:
                posts.append(post('', '', '/empty'))
            raw = wire(posts)
            files = self.package(raw)
            source = self.work/f'input-{index}.json'
            source.write_bytes(raw)
            native = self.work/f'native-{index}'
            result = self.cli('prepare-post-package', source, native)
            self.assertEqual(result.stdout, b'post package prepared\n')
            self.assertEqual([(native/name).read_bytes() for name in NAMES], files)
            archive = self.write_package(files, f'archive-{index}')
            self.cli('check-package', archive/NAMES[0], archive/NAMES[1])
            original = [(archive/name).read_bytes() for name in NAMES]
            bad = self.work/f'bad-{index}.json'
            bad.write_bytes(files[0]+b' ')
            self.cli('check-package', bad, archive/NAMES[1], success=False)
            other = json.loads(files[1]); other['sha256'] = '0'*64
            wrong = self.work/f'wrong-{index}.json'
            wrong.write_text(json.dumps(other), encoding='utf-8')
            self.cli('check-package', archive/NAMES[0], wrong, success=False)
            self.assertEqual([(archive/name).read_bytes() for name in NAMES], original)

    def test_invalid_input_refusals_remain_opaque(self):
        invalid = [b'PRIVATE invalid JSON', wire([post('\0PRIVATE')]),
                   wire([post(target='/same'), post(target='/same')]),
                   wire([post(target='/health/live?q=1')]), b' '*(LIMIT+1),
                   wire([post('<'*(2*1024*1024+1))])]
        for index, raw in enumerate(invalid):
            self.package(raw, accepted=False)
            source = self.work/f'PRIVATE-{index}.json'; source.write_bytes(raw)
            destination = self.work/f'refused-{index}'
            result = self.capture([str(self.server), 'prepare-post-package', str(source), str(destination)], bound=65536)
            self.assertEqual((result.returncode, result.stdout, result.stderr),
                             (2, b'', b'post package preparation failed\n'))
            self.assertFalse(destination.exists())
        # Explicit UI profile refusal, not claimed as native parser parity.
        self.package(wire([]), accepted=False)

    def test_fixed_zip_loader_and_sealed_http_after_both_files_change(self):
        target = '/zip-sealed?b=2&a=1'
        source_paths = [ROOT/'tools/prepared-post-workbench/publication-package.js',
                        ROOT/'tools/prepared-post-workbench/input-parser.js',
                        ROOT/'scripts/workbench_package_probe.js', self.server]
        source_hashes = {str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in source_paths}
        input_bytes = wire([post('\n<script>文字 & \r\n🦀', 'Public ZIP', target)])
        files = self.package(input_bytes)
        directory = self.write_package(files, 'sealed')
        self.cli('check-package', directory/NAMES[0], directory/NAMES[1])
        body = json.loads(files[0])['pages'][0]['body'].encode()
        server = smoke.start('public', '--host', 'example.test', '--bundle', str(directory/NAMES[0]),
                             '--receipt', str(directory/NAMES[1]), binary=self.server)
        checks = []
        try:
            status, headers, raw = smoke.request(server.port, target)
            self.assertEqual((status, raw), (200, body)); etag = headers['etag']; checks.append('get')
            status, headers, raw = smoke.request(server.port, target, method='HEAD')
            self.assertEqual((status, raw, int(headers['content-length']), headers['etag']), (200, b'', len(body), etag))
            checks.append('head')
            self.assertEqual(smoke.request(server.port, target, extra_headers={'If-None-Match': etag})[0], 304)
            checks.append('conditional')
            self.assertEqual(smoke.request(server.port, target, host='wrong.test')[0], 421); checks.append('host')
            self.assertEqual(smoke.request(server.port, '/zip-sealed?a=1&b=2')[0], 404); checks.append('query')
            for name in NAMES:
                (directory/name).write_bytes(b'changed after startup\n')
            status, headers, raw = smoke.request(server.port, target)
            self.assertEqual((status, raw, headers['etag']), (200, body, etag)); checks.append('sealed')
        finally:
            import sys
            smoke.stop(server, original=sys.exc_info()[1])
        smoke.reap_servers(); owned_probe.reap_owned_probes()
        self.assertIsNotNone(server.process.returncode)
        self.assertFalse(server.reader.is_alive())
        self.assertEqual({str(path.relative_to(ROOT)): sha(path.read_bytes()) for path in source_paths}, source_hashes)
        record = {'format_version': 1, 'scope': 'fixed_workbench_zip_native_sealed_http',
                  'build_pid': self.build_pid, 'probe_pids': self.pids, 'server_pid': server.process.pid,
                  'server_sha256': sha(self.server.read_bytes()), 'bundle_sha256': sha(files[0]),
                  'receipt_sha256': sha(files[1]), 'body_sha256': sha(body), 'checks': checks,
                  'input_sha256': sha(input_bytes), 'selected_source_hashes': source_hashes,
                  'selected_sources_unchanged': True,
                  'server_exit_code': server.process.returncode, 'reader_joined': True,
                  'all_owned_children_reaped': True,
                  'limitations': 'Public local fixture only; no auth/DB/theme/Linux or OS download-save claim.'}
        self.cleanup_owned()
        record['owned_directory_removed'] = not self.work.exists()
        (ROOT/'target/workbench-package-native-observation.json').write_text(
            json.dumps(record, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    unittest.main()
