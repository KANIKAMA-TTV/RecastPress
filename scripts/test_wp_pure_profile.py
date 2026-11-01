"""Fixed-profile protocol/graph/cleanup contracts; no Cargo execution here."""
from contextlib import redirect_stderr, redirect_stdout
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import check_wp_pure_profile as gate


class PureProfileTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix='wp-pure-controller-test-', dir=gate.ROOT / 'target')
        self.addCleanup(self.directory.cleanup)
        self.target = Path(self.directory.name)
        self.library = self.target / 'librecastpress_wp.rlib'
        self.executable = self.target / 'native_test.exe'
        self.library.write_bytes(b'fixed public fixture')
        self.executable.write_bytes(b'fixed public fixture')
        self.target_patch = patch.object(gate, 'TARGET_DIR', self.target)
        self.target_patch.start()
        self.addCleanup(self.target_patch.stop)

    def artifact(self, test=False):
        return {'reason': 'compiler-artifact', 'manifest_path': str(gate.MANIFEST),
                'target': {'name': gate.LIB if not test else 'pure_contract', 'kind': ['lib'] if not test else ['test'],
                           'src_path': str(gate.LIB_SOURCE if not test else gate.MANIFEST.parent / 'tests/pure_contract.rs')},
                'features': [], 'profile': {'test': test},
                'filenames': [str(self.executable if test else self.library)],
                'executable': str(self.executable) if test else None}

    def stream(self, messages=None, *, finished=True, summary=True):
        values = [self.artifact(), self.artifact(True)] if messages is None else messages
        if finished:
            values = [*values, {'reason': 'build-finished', 'success': True}]
        raw = b''.join((json.dumps(value) + '\n').encode() for value in values)
        if summary:
            raw += b'\nrunning 3 tests\ntest public_contract ... ok\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n'
        return raw

    def result(self, raw, code=0, stderr=b''):
        return subprocess.CompletedProcess(['fixed-command'], code, raw, stderr)

    def outputs(self):
        return [(101, self.result(b'rustc 1.96.0\nhost: x86_64-pc-windows-msvc\n')),
                (102, self.result(self.stream())),
                (103, self.result(self.stream([self.artifact()], summary=False))),
                (104, self.result(b'recastpress-wp v0.1.0 (fixed checkout)\n'))]

    def test_current_no_auth_library_and_actual_native_test_summary_are_required(self):
        result = gate.validate_cargo_output(self.stream(), tests=True)
        self.assertEqual(result['passed'], 3)
        self.assertEqual(result['test_artifacts'], 1)
        clippy_test = self.artifact(True)
        clippy_test['executable'] = None  # cargo check/clippy does not link tests.
        clippy_test['filenames'] = [str(self.library)]
        self.assertEqual(gate.validate_cargo_output(self.stream([self.artifact(), clippy_test], summary=False), tests=False)['wp_features'], [])

    def test_auth_unrelated_manifest_stale_files_and_ambiguous_library_are_refused(self):
        variants = []
        for field, value in [('features', ['auth']), ('features', ['other']), ('features', None),
                             ('manifest_path', str(self.target / 'Cargo.toml')), ('filenames', [str(self.target / 'missing')])]:
            item = self.artifact()
            item[field] = value
            variants.append([item, self.artifact(True)])
        item = self.artifact()
        item['target']['src_path'] = str(gate.MANIFEST.parent / 'src/other.rs')
        variants.append([item, self.artifact(True)])
        item = self.artifact(True)
        item['features'] = ['auth']
        variants.append([self.artifact(), item])
        variants.extend([[self.artifact(), self.artifact(), self.artifact(True)], [self.artifact(True)]])
        for messages in variants:
            with self.subTest(messages=messages), self.assertRaises(gate.PureProfileError):
                gate.validate_cargo_output(self.stream(messages), tests=True)
        with patch.object(gate, 'TARGET_DIR', self.target / 'elsewhere'), self.assertRaises(gate.PureProfileError):
            gate.validate_cargo_output(self.stream(), tests=True)

    def test_failed_incomplete_or_zero_tests_never_become_a_success(self):
        raw = self.stream().replace(b'"success": true', b'"success": false')
        variants = [raw, self.stream(finished=False), self.stream(summary=False), self.stream([self.artifact()]),
                    self.stream().replace(b'3 passed', b'0 passed'), self.stream().replace(b'0 failed', b'1 failed'),
                    self.stream().replace(b'"features": []', b'"features": [], "features": []'), b'not Cargo JSON\n']
        for raw in variants:
            with self.subTest(size=len(raw)), self.assertRaises((gate.PureProfileError, ValueError)):
                gate.validate_cargo_output(raw, tests=True)

    def test_normal_graph_is_package_only_and_cannot_hide_crypto_or_development_edges(self):
        good = b'recastpress-wp v0.1.0 (fixed checkout)\n'
        self.assertEqual(gate.validate_normal_graph(good), [gate.PACKAGE])
        for added in ('bcrypt', 'getrandom', 'hmac', 'sha2', 'md-5', 'subtle', 'zeroize', 'serde_json', 'unknown-provider'):
            with self.subTest(added=added), self.assertRaises(gate.PureProfileError):
                gate.validate_normal_graph(good + f'{added} v1.2.3\n'.encode())
        for raw in (b'', good + good, b'wrong-root v0.1.0\n', good + b'PRIVATE_CANARY\n'):
            with self.assertRaises(gate.PureProfileError):
                gate.validate_normal_graph(raw)

    def test_controller_pins_host_package_and_target_without_workspace_or_auth_binary(self):
        original_target = os.environ.get('CARGO_BUILD_TARGET')
        with patch.object(gate.owned_probe, 'run_captured_probe', side_effect=self.outputs()) as run, patch.object(gate.owned_probe, 'reap_owned_probes') as reap:
            value = gate.run_gate()
        self.assertEqual(value['command_process_pids'], [101, 102, 103, 104])
        self.assertEqual(value['native_test_passed'], 3)
        self.assertEqual(len(run.call_args_list), 4)
        for call in run.call_args_list[1:]:
            args = call.args[0]
            self.assertIn('--no-default-features', args)
            self.assertIn('--offline', args)
            self.assertIn('--locked', args)
            self.assertNotIn('--workspace', args)
            self.assertEqual(args[args.index('--package') + 1], gate.PACKAGE)
            self.assertEqual(args[args.index('--target') + 1], 'x86_64-pc-windows-msvc')
            self.assertEqual(call.args[1]['CARGO_BUILD_TARGET'], 'x86_64-pc-windows-msvc')
        self.assertEqual(os.environ.get('CARGO_BUILD_TARGET'), original_target)
        self.assertEqual(run.call_args_list[1].args[0][0:2], ['cargo', 'test'])
        self.assertEqual(run.call_args_list[2].args[0][-3:], ['--', '-D', 'warnings'])
        self.assertIn('normal', run.call_args_list[3].args[0])
        reap.assert_called_once()

    def test_failure_timeout_and_cleanup_failure_are_closed_with_no_later_commands(self):
        for failure in ((102, self.result(b'PRIVATE_CANARY', code=1, stderr=b'PRIVATE_CANARY')), subprocess.TimeoutExpired(['fixed-cargo'], 1)):
            with patch.object(gate.owned_probe, 'run_captured_probe', side_effect=[self.outputs()[0], failure]), patch.object(gate.owned_probe, 'reap_owned_probes') as reap, self.assertRaises(Exception):
                gate.run_gate()
            reap.assert_called_once()
        original = subprocess.TimeoutExpired(['fixed-cargo'], 1)
        with patch.object(gate.owned_probe, 'run_captured_probe', side_effect=original), patch.object(gate.owned_probe, 'reap_owned_probes', side_effect=RuntimeError('PRIVATE_CANARY')):
            with self.assertRaises(subprocess.TimeoutExpired) as raised:
                gate.run_gate()
            self.assertEqual(raised.exception.__notes__, ['Pure WP cleanup retry failed: RuntimeError'])
        with patch.object(gate.owned_probe, 'run_captured_probe', side_effect=self.outputs()), patch.object(gate.owned_probe, 'reap_owned_probes', side_effect=RuntimeError('PRIVATE_CANARY')), self.assertRaises(gate.PureProfileError):
            gate.run_gate()

    def test_host_protocol_cli_refusals_and_fixed_failure_have_no_override_or_raw_diagnostic(self):
        for raw in (b'host: a-b-c\nhost: d-e-f\n', b'host: ../../private\n', b'host: single\n', b'PRIVATE_CANARY'):
            with self.assertRaises(gate.PureProfileError):
                gate.rust_host(raw)
        for argv in (['--target', 'PRIVATE_CANARY'], ['--workspace'], ['artifact.exe']):
            out, err = io.StringIO(), io.StringIO()
            with redirect_stdout(out), redirect_stderr(err), patch.object(gate, 'run_gate') as run:
                self.assertEqual(gate.main(argv), 2)
            run.assert_not_called()
            self.assertEqual(out.getvalue(), '')
            self.assertEqual(err.getvalue(), gate.FAILURE + '\n')
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()) as err, patch.object(gate, 'run_gate', side_effect=RuntimeError('PRIVATE_CANARY')):
            self.assertEqual(gate.main([]), 1)
            self.assertEqual(err.getvalue(), gate.FAILURE + '\n')


if __name__ == '__main__':
    unittest.main()
