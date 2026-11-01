"""Fixed public title -> separate escaping -> receipt -> sealed HTTP relation.

Uses current Cargo JSON artifacts. This is a development observation, not a
theme SDK, authenticated publication path, or general HTML-safety claim.
"""
import hashlib
from html.parser import HTMLParser
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

import owned_probe
import smoke

ROOT = Path(__file__).resolve().parents[1]
TARGET = '/title-fixture'
EXPECTED = '“WordPress & friends” — 9×9 &a; <b> 日本'


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


class TitleParser(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.tags, self.stack, self.titles, self.headings = [], [], [], []

    def handle_starttag(self, tag, attrs):
        self.tags.append(tag)
        if tag != 'meta':
            self.stack.append(tag)

    def handle_endtag(self, tag):
        if not self.stack or self.stack.pop() != tag:
            raise AssertionError('Fixed title fixture has unbalanced elements')

    def handle_data(self, data):
        if self.stack and self.stack[-1] == 'title':
            self.titles.append(data)
        if self.stack and self.stack[-1] == 'h1':
            self.headings.append(data)


class TitlePublicationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.addClassCleanup(owned_probe.reap_owned_probes)
        cls.build_pids = []
        cls.producer = cls.build('recastpress-wp', 'example', 'title_publication_fixture')
        cls.server = cls.build('recastpress-server', 'bin', 'recastpress')

    @classmethod
    def build(cls, package, kind, name):
        pid, result = owned_probe.run_captured_probe(
            ['cargo', 'build', '-p', package, '--'+kind, name, '--offline', '--locked', '--message-format=json'],
            os.environ.copy(), timeout=180)
        cls.build_pids.append(pid)
        if result.returncode != 0:
            raise AssertionError('Current title publication artifact did not build')
        if len(result.stdout) > 16 * 1024 * 1024:
            raise AssertionError('Cargo JSON exceeded fixture bound')
        manifest = (ROOT / 'crates' / package / 'Cargo.toml').resolve()
        source = manifest.parent / ('examples/'+name+'.rs' if kind == 'example' else 'src/main.rs')
        candidates = []
        for line in result.stdout.splitlines():
            item = json.loads(line)
            if (item.get('reason') == 'compiler-artifact' and item.get('executable')
                    and Path(item.get('manifest_path', '')).resolve() == manifest
                    and item.get('target', {}).get('name') == name
                    and item.get('target', {}).get('kind') == [kind]
                    and Path(item['target']['src_path']).resolve() == source.resolve()):
                candidates.append(Path(item['executable']).resolve())
        if len(candidates) != 1 or not candidates[0].is_file():
            raise AssertionError('Cargo must select one current title publication artifact')
        (ROOT / 'target' / ('title-publication-build-'+name+'.jsonl')).write_bytes(result.stdout)
        return candidates[0]

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='title-publication-', dir=ROOT / 'target')
        self.addCleanup(temporary.cleanup)
        self.addCleanup(smoke.reap_servers)
        self.addCleanup(owned_probe.reap_owned_probes)
        self.work = Path(temporary.name).resolve()
        self.assertTrue(self.work.is_relative_to((ROOT / 'target').resolve()))
        self.pids = []

    def captured(self, command):
        pid, result = owned_probe.run_captured_probe(command, os.environ.copy(), timeout=20)
        self.pids.append(pid)
        self.assertEqual(result.returncode, 0, 'Title fixture command failed')
        self.assertEqual(result.stderr, b'')
        self.assertLessEqual(len(result.stdout), 65536)
        return result.stdout

    def test_filtered_title_is_separately_escaped_and_sealed_after_producer_and_files_change(self):
        payload = self.captured([str(self.producer)])
        self.assertTrue(payload.endswith(b'\n'))
        bundle = json.loads(payload)
        self.assertEqual(set(bundle), {'format_version', 'pages'})
        self.assertEqual(bundle['format_version'], 1)
        self.assertEqual(len(bundle['pages']), 1)
        page = bundle['pages'][0]
        self.assertEqual(set(page), {'target', 'status', 'content_type', 'body'})
        self.assertEqual((page['target'], page['status'], page['content_type']),
                         (TARGET, 200, 'text/html; charset=utf-8'))
        body = page['body'].encode('utf-8')
        self.assertNotIn(b'&amp;#8220;', body)
        self.assertIn(b'&#8220;WordPress', body)
        parsed = TitleParser()
        parsed.feed(page['body'])
        parsed.close()
        self.assertEqual(parsed.stack, [])
        self.assertEqual(parsed.tags, ['html', 'head', 'meta', 'title', 'body', 'h1', 'p'])
        self.assertEqual(''.join(parsed.titles), EXPECTED)
        self.assertEqual(''.join(parsed.headings), EXPECTED)

        payload_path, receipt_path = self.work/'bundle.json', self.work/'receipt.json'
        payload_path.write_bytes(payload)
        receipt = self.captured([str(self.server), 'bundle-receipt', str(payload_path)])
        self.assertEqual(json.loads(receipt), {'format_version': 1,
            'payload_bytes': len(payload), 'sha256': sha(payload)})
        receipt_path.write_bytes(receipt)
        report = json.loads(self.captured([str(self.server), 'check-package', str(payload_path), str(receipt_path)]))
        self.assertEqual(set(report), {'format_version', 'accepted_bundle_version', 'pages',
                                      'redirects', 'body_bytes', 'mime_counts'})
        self.assertEqual((report['format_version'], report['accepted_bundle_version'], report['pages'],
                          report['redirects'], report['body_bytes']), (1, 1, 1, 0, len(body)))
        self.assertEqual(report['mime_counts']['text/html; charset=utf-8'], 1)
        self.assertEqual(sum(report['mime_counts'].values()), 1)
        self.assertEqual(payload_path.read_bytes(), payload)
        self.assertEqual(receipt_path.read_bytes(), receipt)

        server = smoke.start('public', '--host', 'example.test', '--bundle', str(payload_path),
                             '--receipt', str(receipt_path), binary=self.server)
        checks = []
        try:
            status, headers, actual = smoke.request(server.port, TARGET)
            self.assertEqual((status, actual), (200, body))
            self.assertEqual(headers['content-type'], page['content_type'])
            self.assertEqual(int(headers['content-length']), len(body))
            etag = headers['etag']
            checks.append('get_exact_bytes')
            status, headers, actual = smoke.request(server.port, TARGET, method='HEAD')
            self.assertEqual((status, actual, headers['etag']), (200, b'', etag))
            self.assertEqual(int(headers['content-length']), len(body))
            checks.append('head_same_length_etag')
            status, _, actual = smoke.request(server.port, TARGET, extra_headers={'If-None-Match': etag})
            self.assertEqual((status, actual), (304, b''))
            checks.append('conditional_304')
            status, _, _ = smoke.request(server.port, TARGET, host='wrong.test')
            self.assertEqual(status, 421)
            checks.append('wrong_host_421')
            status, _, _ = smoke.request(server.port, TARGET+'?different=1')
            self.assertEqual(status, 404)
            checks.append('query_is_exact')
            # The producer has exited. The public process owns admitted bytes;
            # neither fixture file is a live source for this generation.
            payload_path.write_bytes(b'changed after startup\n')
            receipt_path.write_bytes(b'changed after startup\n')
            status, headers, actual = smoke.request(server.port, TARGET)
            self.assertEqual((status, headers['etag'], actual), (200, etag, body))
            checks.append('loaded_bytes_survive_both_file_changes')
        finally:
            smoke.stop(server, original=sys.exc_info()[1])
        smoke.reap_servers()
        owned_probe.reap_owned_probes()
        self.assertIsNotNone(server.process.returncode)
        self.assertFalse(server.reader.is_alive())
        observation = {'format_version': 1, 'scope': 'fixed_prepared_default_title_escaped_bundle_http',
            'build_pids': self.build_pids, 'probe_pids': self.pids, 'server_pid': server.process.pid,
            'producer_sha256': sha(self.producer.read_bytes()), 'server_sha256': sha(self.server.read_bytes()),
            'bundle_bytes': len(payload), 'bundle_sha256': sha(payload), 'receipt_bytes': len(receipt),
            'body_bytes': len(body), 'body_sha256': sha(body), 'checks': checks,
            'server_exit_code': server.process.returncode, 'reader_joined': True,
            'all_owned_children_reaped': True, 'source_files_changed_after_startup': True,
            'limitations': 'Fixed public fixture only; no DB/auth/theme SDK/Linux-native equivalence claim.'}
        (ROOT/'target/title-default-publication-observation.json').write_text(
            json.dumps(observation, indent=2)+'\n', encoding='utf-8')


if __name__ == '__main__':
    unittest.main()
