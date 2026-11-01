"""Strict prepared-title evidence profile and deterministic finite corpus tests."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import title_profile as profile
import texturize_profile


class PreparedTitleProfileTests(unittest.TestCase):
    def test_profile_refuses_type_confusion_nested_changes_and_extra_fields(self):
        profile.validate_profile(copy.deepcopy(profile.PROFILE))
        mutations = []
        for key, value in [('frontend', 1), ('native_filter_bookkeeping', 0),
                           ('all_hook', 0), ('post_filter', 'display'), ('name', 'other')]:
            item = copy.deepcopy(profile.PROFILE); item[key] = value; mutations.append(item)
        item = copy.deepcopy(profile.PROFILE); item['callbacks'][0]['accepted_args'] = True; mutations.append(item)
        item = copy.deepcopy(profile.PROFILE); item['callbacks'].reverse(); mutations.append(item)
        item = copy.deepcopy(profile.PROFILE); item['priming']['reset'] = 1; mutations.append(item)
        item = copy.deepcopy(profile.PROFILE); item['fixture_limits']['max_input_bytes'] = 65536.0; mutations.append(item)
        item = copy.deepcopy(profile.PROFILE); item['extra'] = False; mutations.append(item)
        for item in mutations:
            with self.assertRaises(ValueError):
                profile.validate_profile(item)

    def test_per_batch_priming_and_shared_spaces_cache_have_distinct_actual_counts(self):
        empty, zero, active = [profile.fixture(value) for value in (b'', b'0', b' ') ]
        old_empty = {'api': 'wptexturize', 'input': {'type': 'string', 'hex': ''}}
        old_active = {'api': 'wptexturize', 'input': {'type': 'string', 'hex': '78'}}
        expected = {'calls': 3, 'the_title_hook': 3, 'getter_x_calls': 0, 'getter_run_hook': 0,
                    'getter_no_tags_hook': 1, 'getter_no_shortcodes_hook': 1,
                    'prime_calls': 3, 'prime_x_calls': 33, 'prime_run_hook': 3,
                    'prime_no_tags_hook': 3, 'prime_no_shortcodes_hook': 3,
                    'prime_spaces_hook': 1, 'registered_shape': 3, 'db_calls': 0,
                    'cache_calls': 0, 'restored': 3}
        self.assertEqual(profile.expected_counts([old_empty, empty, zero, active]), expected)
        already_initialized = dict(expected, prime_spaces_hook=0)
        self.assertEqual(profile.expected_counts([old_active, empty, zero, active]), already_initialized)
        self.assertEqual(profile.aggregate_counts([[empty], [zero], [old_active, active]]),
                         dict(expected, prime_spaces_hook=2))
        profile.validate_counts(expected, expected)
        for value in (False, 0.0, -1):
            bad = dict(expected, getter_run_hook=value)
            with self.assertRaises(ValueError):
                profile.validate_counts(bad, expected)
        with self.assertRaises(ValueError):
            profile.validate_counts(dict(expected, cache_calls=1), expected)

    def test_corpus_preserves_texturize_values_and_has_distinct_default_filter_boundaries(self):
        values = profile.cases()
        again = profile.cases()
        self.assertEqual(values, again)
        old = texturize_profile.cases()
        self.assertEqual([case['input'] for case in values[:len(old)]], [case['input'] for case in old])
        raws = {bytes.fromhex(case['input']['hex']) for case in values}
        self.assertTrue({b'&a;', b'&ab;', b'&aaaaaaaa;', b'&aaaaaaaaa;', b'&aaaaaaaaaa;',
                         b'&&a;', b'&&amp;', b'WordpressWordpress', b'XWordpressY',
                         b'\x0cWordpress\x0c', '\u00a0Wordpress\u00a0'.encode(),
                         b'A' * 65536, b' ' * 65536} <= raws)
        self.assertEqual({case['post_id']['value'] for case in values}, {'1', '713', str(2**63-1)})
        self.assertEqual({case['post_password']['hex'] for case in values}, {'', '30'})
        for case in values:
            self.assertEqual(case['post_status'], {'type': 'string', 'hex': b'publish'.hex()})
            self.assertIs(case['frontend'], True)
            self.assertEqual(case['post_filter'], 'raw')
            self.assertEqual(case['limits'], profile.LIMITS)
        self.assertEqual(profile.expected_counts([]), dict.fromkeys(profile.COUNT_KEYS, 0))

    def test_old74_identity_requires_all_four_typed_fields(self):
        expected = [profile.OLD_COUNT, profile.OLD_INPUT_SHA256,
                    profile.OLD_OUTPUT_SHA256, profile.OLD_OUTPUT_SHA256]
        profile.validate_previous(*expected)
        for index, replacement in [(0, float(profile.OLD_COUNT)), (0, profile.OLD_COUNT-1),
                                   (1, '0'*64), (2, '0'*64), (3, '0'*64)]:
            bad = list(expected); bad[index] = replacement
            with self.assertRaises(ValueError):
                profile.validate_previous(*bad)

    def test_registry_does_not_trust_matching_forged_actual_and_expected_counts(self):
        import compatibility_registry as registry
        matrix = profile.expected_counts(profile.cases())
        arguments = [profile.PROFILE, matrix, matrix, len(profile.cases()),
                     profile.OLD_COUNT, profile.OLD_INPUT_SHA256,
                     profile.OLD_OUTPUT_SHA256, profile.OLD_OUTPUT_SHA256]
        # The production routine derives this matrix from full mixed batches;
        # this unit isolates the refusal of matching forged artifact counters.
        with patch.object(registry, '_title_expected_coverage', return_value=matrix):
            registry.validate_title_coverage(*arguments)
            forged = dict(matrix, getter_x_calls=1)
            bad = list(arguments); bad[1] = forged; bad[2] = forged
            with self.assertRaises(ValueError):
                registry.validate_title_coverage(*bad)
            bad = list(arguments); bad[3] = float(bad[3])
            with self.assertRaises(ValueError):
                registry.validate_title_coverage(*bad)

    def test_default_callback_tuples_are_data_only_and_bound_to_pinned_source(self):
        source = profile.ROOT / '.reference/wordpress-7.1.3/wp-includes/default-filters.php'
        if not source.is_file():
            self.skipTest('pinned WordPress unavailable')
        self.assertEqual(profile.check_callbacks(), profile.CALLBACKS)
        wire = source.read_bytes()
        self.assertEqual(hashlib.sha256(wire).hexdigest(), profile.DEFAULT_FILTERS_SHA256)
        with tempfile.TemporaryDirectory(dir=profile.ROOT / 'target') as directory:
            path = Path(directory) / 'changed.php'
            path.write_bytes(wire.replace(b"'capital_P_dangit', 11", b"'capital_P_dangit', 12", 1))
            with self.assertRaises(ValueError):
                profile.check_callbacks(path)


if __name__ == '__main__':
    unittest.main()
