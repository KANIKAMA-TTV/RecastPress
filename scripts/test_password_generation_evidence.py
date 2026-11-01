"""Synthetic strict relation/scope checks; no real credentials or bcrypt CPU work."""
import copy
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import tempfile
import unittest

import check_password_generation_evidence as checker
from owned_probe import run_captured_probe

HASH = '$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C'

def valid_record():
    rows = []
    for name, password in checker.cases():
        over = len(password) > 4096
        checks = [] if over else [True, password == checker.trim6(password), False]
        rows.append({'name': name, 'native_hash_hex': '2a' if over else HASH.encode().hex(),
            'php_hash_hex': '2a' if over else HASH.encode().hex(), 'algorithm_calls': 0 if over else 1,
            'options_calls': 0 if over else 1, 'globals_restored': True, 'actions_unchanged': True,
            'php_checks': checks, 'native_checks': checks, 'check_calls': [] if over else [1, 1, 1]})
    return {'format_version': 1, 'scope': 'public_dummy_wp_password_generation_cross_verification',
        'generated_at_utc': datetime.now(timezone.utc).isoformat(), 'profile': copy.deepcopy(checker.PROFILE),
        'corpus_sha256': checker.corpus_hash(), 'runs': {name: {'owned_pid': 1234, 'reported_pid': 1234,
        'exit_code': 0, 'stderr_empty': True} for name in ('rust_generate', 'php_generate', 'php_verify', 'rust_verify')},
        'cases': rows, 'host_refusals': {'within_max4': 'cost_limit', 'over_max4': '2a'},
        'tracked_files': dict.fromkeys(checker.tracked_files(), 'a' * 64),
        'binaries': {name: {'path': path, 'sha256': 'b' * 64} for name, path in checker.binary_paths().items()},
        'limitations': checker.LIMITATIONS.copy()}

class PasswordGenerationEvidenceTests(unittest.TestCase):
    def rejected(self, mutate):
        value = valid_record()
        mutate(value)
        with self.assertRaises((ValueError, TypeError)):
            checker.validate_record(value, verify_files=False)

    def test_relation_accepts_separate_random_hashes_but_requires_all_both_direction_checks(self):
        value = valid_record()
        # Another canonical checksum is not required to equal the first random output.
        value['cases'][0]['native_hash_hex'] = (HASH[:-1] + 'G').encode().hex()
        checker.validate_record(value, verify_files=False)
        for key in ('php_checks', 'native_checks'):
            self.rejected(lambda v, k=key: v['cases'][0].__setitem__(k, [False, True, False]))
            self.rejected(lambda v, k=key: v['cases'][0].__setitem__(k, [1, True, False]))
        self.rejected(lambda v: v['cases'][2].__setitem__('php_checks', [True, True, False]))

    def test_exact_types_field_sets_scope_and_finite_coverage_are_required(self):
        for mutate in (lambda v: v.__setitem__('format_version', True),
                       lambda v: v['profile'].__setitem__('cost', 12.0),
                       lambda v: v.__setitem__('scope', 'full_wp_compatibility'),
                       lambda v: v['cases'].pop(), lambda v: v['cases'].reverse(),
                       lambda v: v['cases'][0].__setitem__('extra', True),
                       lambda v: v['profile'].__setitem__('trim_bytes_hex', '090a0b0c0d20'),
                       lambda v: v.__setitem__('corpus_sha256', '0'*64)):
            self.rejected(mutate)

    def test_pid_exit_context_filter_and_overlength_precedence_fail_closed(self):
        for mutate in (lambda v: v['runs']['php_generate'].__setitem__('reported_pid', 4321),
                       lambda v: v['runs']['rust_verify'].__setitem__('owned_pid', True),
                       lambda v: v['runs']['rust_generate'].__setitem__('exit_code', False),
                       lambda v: v['runs']['php_verify'].__setitem__('stderr_empty', False),
                       lambda v: v['cases'][0].__setitem__('globals_restored', False),
                       lambda v: v['cases'][0].__setitem__('actions_unchanged', False),
                       lambda v: v['cases'][0].__setitem__('algorithm_calls', True),
                       lambda v: v['cases'][-1].__setitem__('options_calls', 1),
                       lambda v: v['host_refusals'].__setitem__('over_max4', 'cost_limit')):
            self.rejected(mutate)

    def test_canonical_hash_bits_and_current_source_binary_sets_are_required(self):
        for index, replacement in ((31, '/'), (62, '/'), (6, 'b'), (8, '4')):
            raw = bytearray(HASH.encode()); raw[index] = ord(replacement)
            self.rejected(lambda v, h=raw.hex(): v['cases'][0].__setitem__('php_hash_hex', h))
        self.rejected(lambda v: v['tracked_files'].pop('Cargo.lock'))
        self.rejected(lambda v: v['binaries']['rust'].__setitem__('path', '../outside.exe'))
        self.rejected(lambda v: v['binaries']['php'].__setitem__('sha256', False))
        with self.assertRaises(ValueError):
            checker.validate_record(valid_record())  # Synthetic source hashes never attest current bytes.

    def test_json_duplicate_unknown_and_overbound_inputs_are_rejected(self):
        with tempfile.TemporaryDirectory(dir=checker.ROOT / 'target') as temporary:
            path = Path(temporary) / 'evidence.json'
            for wire in (b'{"format_version":1,"format_version":1}', b' '*(checker.MAX_JSON+1), b'\xff'):
                path.write_bytes(wire)
                with self.assertRaises((ValueError, UnicodeError)):
                    checker.read_json(path)
        self.rejected(lambda v: v.__setitem__('generated_at_utc', '2026-10-29T00:00:00'))

    @unittest.skipUnless((checker.ROOT / checker.binary_paths()['php']).is_file(), 'pinned development PHP unavailable')
    def test_actual_oracle_rejects_nonbyte_duplicate_and_unknown_wire_without_stdout(self):
        command = [str(checker.ROOT / checker.binary_paths()['php']), '-n', '-d', 'memory_limit=128M',
                   str(checker.ROOT / 'scripts/password_generation_oracle.php'),
                   str(checker.ROOT / '.reference/wordpress-7.1.3')]
        wires = [b'{"operation":"generate","items":[{"password_hex":null}]}',
                 b'{"operation":"generate","operation":"generate","items":[]}',
                 b'{"operation":"generate","items":[{"password_hex":"","maximum":12}]}',
                 b'{"operation":"generate","items":[{"password_hex":"0g"}]}']
        for wire in wires:
            _, result = run_captured_probe(command, os.environ.copy(), timeout=15, input_bytes=wire)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b'')

if __name__ == '__main__':
    unittest.main()
