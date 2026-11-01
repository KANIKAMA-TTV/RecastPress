"""Texturize profile/type/counter/prefix contracts, without PHP or Cargo."""
import copy
import hashlib
import json
import unittest

import compatibility_registry as registry
import texturize_profile as profile


class TexturizeProfileTests(unittest.TestCase):
    def test_profile_requires_exact_fields_types_and_fixed_explicit_limits(self):
        profile.validate_profile(copy.deepcopy(profile.PROFILE))
        variants = []
        for key, value in [('reset', 1), ('native_filter_bookkeeping', 0), ('shortcode_registry', 'ambient'),
                           ('ctype_locale', 'other'), ('source_sha256', '0'*64), ('unknown', 'PRIVATE_CANARY')]:
            item = copy.deepcopy(profile.PROFILE)
            item[key] = value
            variants.append(item)
        for key, value in [('max_input_bytes', True), ('max_work_bytes', 1048576.0), ('max_output_bytes', 65537)]:
            item = copy.deepcopy(profile.PROFILE)
            item['fixture_limits'][key] = value
            variants.append(item)
        item = copy.deepcopy(profile.PROFILE)
        del item['reset']
        variants.extend([item, None, []])
        for value in variants:
            with self.subTest(value=value), self.assertRaises(ValueError):
                profile.validate_profile(value)

    def test_counts_distinguish_initial_zero_empty_from_whitespace_and_process_spaces_cache(self):
        def case(raw):
            return {'api': 'wptexturize', 'input': {'type': 'string', 'hex': raw.hex()}}
        empty = [case(b''), case(b'0')]
        active = [case(b' '), case(b'\0'), case(b'word')]
        self.assertEqual(profile.expected_counts(empty), dict.fromkeys(profile.COUNT_KEYS, 0) | {'calls': 2, 'restored': 2})
        expected = profile.expected_counts(empty + active)
        self.assertEqual(expected, {'calls': 5, 'initialized': 3, 'x_calls': 33, 'run_hook': 3,
                                    'no_tags_hook': 3, 'no_shortcodes_hook': 3, 'spaces_hook': 1, 'restored': 5})
        profile.validate_counts(dict(expected), expected)
        aggregate = profile.aggregate_counts([empty, active[:1], active[1:]])
        self.assertEqual(aggregate['spaces_hook'], 2)
        for key in profile.COUNT_KEYS:
            for value in (True, -1, float(expected[key]), expected[key]+1):
                item = dict(expected)
                item[key] = value
                with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                    profile.validate_counts(item, expected)
        for value in (dict(expected, unknown=0), None, expected | {'calls': False}):
            with self.assertRaises(ValueError):
                profile.validate_counts(value, expected)
        with self.assertRaises(ValueError):
            profile.validate_counts(expected, expected | {'spaces_hook': True})

    def test_old73_requires_count_input_and_each_actual_result_hash_independently(self):
        good = [profile.OLD_COUNT, profile.OLD_INPUT_SHA256, profile.OLD_OUTPUT_SHA256, profile.OLD_OUTPUT_SHA256]
        profile.validate_previous(*good)
        for index in range(4):
            for bad in ([True, profile.OLD_COUNT-1] if index == 0 else [None, '0'*64, good[index].upper()]):
                changed = list(good)
                changed[index] = bad
                with self.subTest(index=index), self.assertRaises(ValueError):
                    profile.validate_previous(*changed)
        # The public evidence validator derives the mixed-corpus process count,
        # rather than trusting matching actual/expected fields in a record.
        expected = profile.expected_counts(profile.cases())
        expected['spaces_hook'] = 5
        arguments = [copy.deepcopy(profile.PROFILE), expected, dict(expected), 9183, *good]
        registry.validate_texturize_coverage(*arguments)
        for index in (6, 7):  # Native/PHP result hashes independently bound.
            changed = copy.deepcopy(arguments)
            changed[index] = '0'*64
            with self.assertRaises(ValueError):
                registry.validate_texturize_coverage(*changed)
        for key in profile.COUNT_KEYS:
            changed = copy.deepcopy(arguments)
            changed[1][key] += 1
            changed[2][key] += 1
            with self.subTest(key=key), self.assertRaises(ValueError):
                registry.validate_texturize_coverage(*changed)

    def test_corpus_is_deterministic_text_only_and_covers_source_rule_boundaries(self):
        first, second = profile.cases(), profile.cases()
        canonical = lambda value: json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True).encode('ascii')
        self.assertEqual(hashlib.sha256(canonical(first)).digest(), hashlib.sha256(canonical(second)).digest())
        self.assertEqual(len(first), 9183)
        values = {bytes.fromhex(case['input']['hex']) for case in first}
        for raw in (b'', b'0', b'\0', b"'99'", b"'99\"", b"7'. 8'.", b'&a5;', b'&copy;', b'xn--', b'&'*4096, '\u00a0'.encode()+b"'A'"):
            self.assertIn(raw, values)
        for case in first:
            self.assertEqual(set(case), {'api', 'input', 'texturize_profile', 'reset', 'limits'})
            self.assertEqual(case['api'], 'wptexturize')
            self.assertIs(case['reset'], True)
            self.assertEqual(case['limits'], profile.LIMITS)
            raw = bytes.fromhex(case['input']['hex'])
            raw.decode('utf-8', errors='strict')
            self.assertNotIn(b'<', raw)
            self.assertLessEqual(len(raw), profile.LIMITS['max_input_bytes'])


if __name__ == '__main__':
    unittest.main()
