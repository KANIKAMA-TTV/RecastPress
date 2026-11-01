"""Strict finite legacy experiment scope; synthetic evidence is never attestation."""
import copy
from datetime import datetime, timezone
import os
from pathlib import Path
import tempfile
import unittest

import check_legacy_password_evidence as checker
from owned_probe import run_captured_probe

def valid_record():
    return {'format_version': 1, 'scope': 'public_dummy_legacy_password_verification',
        'generated_at_utc': datetime.now(timezone.utc).isoformat(), 'profile': copy.deepcopy(checker.PROFILE),
        'corpus_sha256': checker.corpus_hash(), 'runs': {name: {'owned_pid': 1234, 'reported_pid': 1234,
        'exit_code': 0, 'stderr_empty': True} for name in ('rust', 'php')},
        'oracle': {'php_version': '8.5.12', 'globals_restored': True, 'actions_unchanged': True,
                   'restoration_states': ['absent', 'null', 'sentinel', 'empty']},
        'cases': [{'id': case['id'], 'native': {'verified': case['expected']} if type(case['expected']) is bool else {'error': case['expected']},
            'php': {**({'verified': case['expected']} if type(case['expected']) is bool else {'error': case['expected']}),
                    'actual_called': case['actual'], 'check_calls': int(case['actual'])}} for case in checker.cases()],
        'tracked_files': dict.fromkeys(checker.tracked_files(), 'a'*64),
        'binaries': {name: {'path': path, 'sha256': 'b'*64} for name, path in checker.binary_paths().items()},
        'limitations': checker.LIMITATIONS.copy()}

class LegacyPasswordEvidenceTests(unittest.TestCase):
    def rejected(self, mutate):
        record = valid_record(); mutate(record)
        with self.assertRaises((ValueError, TypeError)):
            checker.validate_record(record, verify_files=False)

    def test_boolean_results_and_domain_host_refusals_are_never_interchangeable(self):
        checker.validate_record(valid_record(), verify_files=False)
        self.rejected(lambda v: v['cases'][0]['native'].__setitem__('verified', 1))
        self.rejected(lambda v: v['cases'][0]['php'].__setitem__('verified', False))
        index = next(i for i, case in enumerate(checker.cases()) if case['expected'] == 'cost_limit')
        self.rejected(lambda v: v['cases'][index].__setitem__('native', {'verified': False}))
        self.rejected(lambda v: v['cases'][index]['php'].__setitem__('actual_called', True))
        self.rejected(lambda v: v['cases'][index]['php'].__setitem__('check_calls', 1))
        self.rejected(lambda v: v['cases'][0].__setitem__('native', {'error': 'provider_failure'}))

    def test_source_branch_boundaries_and_corpus_are_deliberate(self):
        cases = {case['id']: case for case in checker.cases()}
        self.assertIs(cases['md5_over4097']['expected'], True)
        self.assertIs(cases['md5_max_fixture8192']['expected'], True)
        self.assertIs(cases['md5_no_trim']['expected'], False)
        self.assertIs(cases['phpass_no_trim']['expected'], False)
        self.assertIs(cases['phpass_over4097_badcost']['expected'], False)
        self.assertIs(cases['phpass_over4097_binarybody']['actual'], True)
        self.assertEqual(cases['phpass_refuse_host30_badbody']['expected'], 'cost_limit')
        self.assertFalse(cases['phpass_refuse_host30_badbody']['actual'])
        self.rejected(lambda v: v['profile'].__setitem__('md5_password_4096_guard', True))
        self.rejected(lambda v: v.__setitem__('corpus_sha256', '0'*64))

    def test_combined_dispatch_preserves_original64_and_distinguishes_guard_and_cost_kinds(self):
        self.assertEqual(len(checker.legacy_cases()), 64)
        import hashlib
        self.assertEqual(hashlib.sha256(checker.common.canonical(checker.legacy_cases())).hexdigest(),
                         checker.PROFILE['combined']['legacy64_sha256'])
        self.assertEqual(checker.cases()[:64], checker.legacy_cases())
        rows = {row['id']: row for row in checker.combined_cases()}
        self.assertIs(rows['combined_md5_max_fixture8192']['expected'], True)
        self.assertIs(rows['combined_short_P_32_long']['expected'], False)
        for name in ('unknown33', 'binary33', 'wp_bad_envelope', 'P_noncanonical', 'H_unsupported'):
            self.assertEqual(rows['combined_'+name+'_4096']['expected'], 'unsupported_hash')
            self.assertFalse(rows['combined_'+name+'_4096']['actual'])
            self.assertIs(rows['combined_'+name+'_4097']['expected'], False)
            self.assertTrue(rows['combined_'+name+'_4097']['actual'])
        self.assertEqual(rows['combined_bcrypt_cost_limit']['expected'], 'bcrypt_cost_limit')
        self.assertEqual(rows['combined_portable_cost_limit']['expected'], 'portable_cost_limit')
        index = next(i for i, case in enumerate(checker.cases()) if case['id'] == 'combined_bcrypt_cost_limit')
        self.rejected(lambda v: v['cases'][index].__setitem__('native', {'error': 'portable_cost_limit'}))
        self.rejected(lambda v: v['profile']['combined'].__setitem__('limits', 'implicit_defaults'))

    def test_exact_fields_types_context_and_complete_order_are_required(self):
        for mutate in (lambda v: v.__setitem__('format_version', True),
                       lambda v: v['profile'].__setitem__('phpass_cost_min', 7.0),
                       lambda v: v['cases'].reverse(), lambda v: v['cases'].pop(),
                       lambda v: v['cases'][0].__setitem__('unknown', True),
                       lambda v: v['oracle'].__setitem__('globals_restored', False),
                       lambda v: v['oracle'].__setitem__('actions_unchanged', False),
                       lambda v: v['oracle']['restoration_states'].pop()):
            self.rejected(mutate)

    def test_pid_binary_and_source_provenance_cannot_be_relabelled(self):
        for mutate in (lambda v: v['runs']['php'].__setitem__('reported_pid', 99),
                       lambda v: v['runs']['rust'].__setitem__('owned_pid', True),
                       lambda v: v['runs']['rust'].__setitem__('exit_code', False),
                       lambda v: v['runs']['php'].__setitem__('stderr_empty', False),
                       lambda v: v['tracked_files'].pop('.reference/wordpress-7.1.3/wp-includes/class-phpass.php'),
                       lambda v: v['binaries']['rust'].__setitem__('path', '../foreign.exe'),
                       lambda v: v.__setitem__('generated_at_utc', '2026-10-29T00:00:00')):
            self.rejected(mutate)
        with self.assertRaises(ValueError): checker.validate_record(valid_record())

    def test_bounded_json_duplicate_and_type_confusion_rejection(self):
        with tempfile.TemporaryDirectory(dir=checker.ROOT/'target') as temporary:
            path = Path(temporary)/'legacy.json'
            for wire in (b'{"scope":"a","scope":"b"}', b'\xff', b' '*(checker.MAX_JSON+1)):
                path.write_bytes(wire)
                with self.assertRaises((ValueError, UnicodeError)): checker.common.read_json(path)

    @unittest.skipUnless((checker.ROOT/checker.binary_paths()['php']).is_file(), 'pinned development PHP unavailable')
    def test_actual_oracle_rejects_bad_wire_without_calling_wordpress_or_stdout(self):
        command = [str(checker.ROOT/checker.binary_paths()['php']), '-n', '-d', 'memory_limit=128M',
                   str(checker.ROOT/'scripts/legacy_password_oracle.php'), str(checker.ROOT/'.reference/wordpress-7.1.3')]
        requests = [{'items': [{'mode': 'md5', 'password_hex': None, 'hash_hex': ''}]},
                    {'items': [{'mode': 'phpass', 'password_hex': '', 'hash_hex': '', 'maximum': True}]},
                    {'items': [{'mode': 'md5', 'password_hex': '', 'hash_hex': '', 'extra': True}]},
                    {'items': [{'mode': 'md5', 'password_hex': '0g', 'hash_hex': ''}]},
                    {'items': [{'mode': 'combined', 'password_hex': '', 'hash_hex': '',
                                'bcrypt_maximum': True, 'portable_maximum': 7}]},
                    {'items': [{'mode': 'combined', 'password_hex': '', 'hash_hex': '',
                                'bcrypt_maximum': 4, 'portable_maximum': None}]},
                    {'items': [{'mode': 'combined', 'password_hex': '', 'hash_hex': '',
                                'bcrypt_maximum': 4}]},
                    {'items': [{'mode': 'combined', 'password_hex': 'SECRET_LEGACY_INPUT', 'hash_hex': '',
                                'bcrypt_maximum': 4, 'portable_maximum': 7}]}]
        wires = [checker.common.canonical(request) for request in requests] + [b'{"items":[],"items":[]}']
        for wire in wires:
            _, result = run_captured_probe(command, os.environ.copy(), timeout=15, input_bytes=wire)
            self.assertNotEqual(result.returncode, 0); self.assertEqual(result.stdout, b'')
            self.assertNotIn(b'SECRET_LEGACY_INPUT', result.stdout+result.stderr)

if __name__ == '__main__': unittest.main()
