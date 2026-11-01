"""Negative evidence contracts; synthetic rows are never actual WP observations."""
import copy
from datetime import datetime, timezone
import tempfile
import time
import unittest
from unittest.mock import patch

import check_auth_cookie_evidence as checker
import compare_auth_cookies as comparison


def synthetic_record():
    anchor = int(time.time())
    cases = checker.corpus.cases(anchor)
    native = [checker.expected_result(case) for case in cases]
    php = [dict(checker.expected_result(case), clock_before=anchor, clock_after=anchor,
                clock_stable=True, counts=checker.corpus.expected_counts(case['expected_stage']),
                actions=checker.corpus.expected_actions(case['expected_stage'])) for case in cases]
    return {
        'format_version': 1, 'scope': checker.SCOPE, 'generated_at_utc': datetime.now(timezone.utc).isoformat(),
        'fixture_anchor': anchor, 'corpus_sha256': checker.corpus_hash(cases),
        'profile': checker.profile({'wp-includes/' + name: 'a' * 64 for name in checker.REFERENCE_FILES}),
        'runs': {key: {'owned_pid': 1234, 'reported_pid': 1234, 'exit_code': 0, 'stderr_empty': True}
                 for key in ('php', 'native_before', 'native_after')},
        'php': php, 'native_before': copy.deepcopy(native), 'native_after': copy.deepcopy(native),
        'closed_controls': [{'id': name, 'owned_pid': 1234, 'exit_code': 2, 'stdout_empty': True,
                             'fixed_opaque_stderr': True} for name in checker.CONTROL_NAMES],
        'tracked_files': dict.fromkeys(checker.tracked_files(), 'a' * 64),
        'binaries': {name: {'path': path, 'sha256': 'b' * 64} for name, path in checker.binary_paths().items()},
        'limitations': checker.LIMITATIONS.copy(),
    }


class AuthCookieEvidenceTests(unittest.TestCase):
    def reject(self, mutate):
        value = synthetic_record()
        mutate(value)
        with self.assertRaises((ValueError, TypeError)):
            checker.validate_record(value, verify_files=False)

    def test_branch_stage_identity_and_every_endpoint_must_agree_with_independent_expected_cases(self):
        checker.validate_record(synthetic_record(), verify_files=False)
        for key in ('php', 'native_before', 'native_after'):
            self.reject(lambda v, key=key: v[key][0].__setitem__('prepared_user_id', '8'))
            self.reject(lambda v, key=key: v[key][0].__setitem__('prepared_user_id', True))
            self.reject(lambda v, key=key: v[key][0].__setitem__('stage', 'bad_mac'))
        self.reject(lambda v: v['native_after'].pop())
        self.reject(lambda v: v['php'].reverse())

    def test_each_case_counters_actions_and_restoration_are_not_aggregate_totals(self):
        for mutate in (
            lambda v: v['php'][0]['counts'].__setitem__('restored', 0),
            lambda v: v['php'][0]['counts'].__setitem__('writes', 1),
            lambda v: v['php'][0]['counts'].__setitem__('hash_salt', True),
            lambda v: v['php'][0]['actions'].__setitem__('auth_cookie_valid', False),
            lambda v: (v['php'][0]['counts'].__setitem__('user_lookup', 0),
                       v['php'][1]['counts'].__setitem__('user_lookup', 2)),
        ):
            self.reject(mutate)

    def test_clock_bracket_anchor_and_nonselected_expiration_are_verified(self):
        for mutate in (
            lambda v: v['php'][0].__setitem__('clock_before', True),
            lambda v: v['php'][0].__setitem__('clock_stable', False),
            lambda v: v['php'][0].__setitem__('clock_after', v['fixture_anchor'] - 1),
            lambda v: v['php'][0].__setitem__('clock_after', v['fixture_anchor'] + 3),
            lambda v: v['php'][0].__setitem__('clock_before', v['fixture_anchor'] + 3600),
            lambda v: v.__setitem__('generated_at_utc', '2020-01-01T00:00:00+00:00'),
        ):
            self.reject(mutate)
        anchor = 1_800_000_000
        cases = checker.corpus.cases(anchor)
        value = synthetic_record()
        rows = copy.deepcopy(value['php'])
        # A future unselected row crossing a one-second bracket must refuse,
        # even though the selected session and cookie stay safely in the future.
        cases[0]['request_without_now']['sessions'].append(
            {'verifier': 'a' * 64, 'expiration': str(anchor), 'legacy': True})
        for row in rows:
            row['clock_before'] = anchor
            row['clock_after'] = anchor + 1
        with self.assertRaises(ValueError):
            checker.validate_php_cases(rows, cases, anchor)

    def test_strict_coverage_profile_pids_sources_and_binary_names(self):
        for mutate in (
            lambda v: v.__setitem__('format_version', True),
            lambda v: v['profile'].__setitem__('builtin_clock', 1),
            lambda v: v['runs']['php'].__setitem__('owned_pid', True),
            lambda v: v['runs']['php'].__setitem__('reported_pid', 5678),
            lambda v: v['tracked_files'].pop('scripts/auth_cookie_oracle.php'),
            lambda v: v['binaries']['rust'].__setitem__('path', 'foreign.exe'),
            lambda v: v.__setitem__('corpus_sha256', '0' * 64),
            lambda v: v['native_before'][0].__setitem__('extra', 0),
        ):
            self.reject(mutate)
        with self.assertRaises(ValueError):
            checker.validate_record(synthetic_record())

    def test_closed_cli_controls_are_complete_and_require_no_partial_result(self):
        for mutate in (
            lambda v: v['closed_controls'].pop(),
            lambda v: v['closed_controls'].reverse(),
            lambda v: v['closed_controls'][0].__setitem__('owned_pid', False),
            lambda v: v['closed_controls'][3].__setitem__('stdout_empty', False),
            lambda v: v['closed_controls'][0].__setitem__('exit_code', 0),
        ):
            self.reject(mutate)

    def test_bounded_duplicate_json_and_original_failure_survives_cleanup_error(self):
        with tempfile.TemporaryDirectory(dir=checker.ROOT / 'target') as folder:
            path = checker.ROOT / folder / 'record.json'
            for wire in (b'{"scope":"x","scope":"y"}', b'\xff', b'NaN', b' ' * (checker.MAX_JSON + 1)):
                path.write_bytes(wire)
                with self.assertRaises((ValueError, UnicodeError)):
                    checker.read_json(path)
        original = ValueError('synthetic public failure')
        with patch.object(comparison, 'compare', side_effect=original), patch.object(
                comparison, 'reap_owned_probes', side_effect=RuntimeError('cleanup')) as cleanup:
            with self.assertRaises(ValueError) as raised:
                comparison.main()
            self.assertIs(raised.exception, original)
            cleanup.assert_called_once()
            self.assertEqual(original.__notes__, ['Owned cookie cleanup retry failed: RuntimeError'])


if __name__ == '__main__':
    unittest.main()
