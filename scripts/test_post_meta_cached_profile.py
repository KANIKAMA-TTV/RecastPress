"""Cached-single fixture semantics and evidence binding, without DB or PHP."""
import copy
import unittest

import compatibility_registry as registry
import post_meta_cached_profile as profile


class PostMetaCachedProfileTests(unittest.TestCase):
    def coverage(self, **changes):
        cases = profile.cases()
        counts = profile.expected_counts(cases)
        arguments = dict(profile=copy.deepcopy(profile.PROFILE), counts=dict(counts),
            expected_counts=dict(counts), total=len(cases), previous_count=profile.OLD_COUNT,
            input_sha=profile.OLD_INPUT_SHA256, rust_sha=profile.OLD_OUTPUT_SHA256,
            php_sha=profile.OLD_OUTPUT_SHA256)
        arguments.update(changes)
        return registry.validate_post_meta_cached_coverage(**arguments)

    def test_null_default_paths_are_distinct_from_equal_empty_values_and_false(self):
        key = b"_wp_attachment_image_alt"
        cases = [profile.fixture(key), profile.fixture(key, None),
            profile.fixture(key, outer_null=True),
            profile.fixture(key, b"N;", witness={"type": "null"}),
            profile.fixture(key, b""),
            profile.fixture(key, b's:0:"";', witness=profile.string(b"")),
            profile.fixture(key, b"b:0;", witness={"type": "bool", "value": False})]
        self.assertEqual([profile.default_calls(case) for case in cases], [1, 1, 1, 1, 0, 0, 0])
        self.assertEqual(profile.expected_counts(cases), {"calls": 7, "get_hook": 7,
            "default_hook": 4, "cache_get": 7, "cache_update": 0, "db_calls": 0, "restored": 7})
        # The later raw object string deliberately has no witness or decode role.
        self.assertNotIn("witness", cases[4])
        self.assertEqual(cases[4]["cache"]["entries"][0]["value"]["entries"][1]["value"],
                         profile.string(b'O:1:"X":0:{}'))

    def test_php_key_rules_and_falsey_key_exclusion_are_not_python_truthiness(self):
        self.assertEqual(profile.key(b"1"), profile.integer(1))
        self.assertEqual(profile.key(b"-9223372036854775808"), profile.integer(-(2**63)))
        for raw in (b"-0", b"01", b"+1", b"9223372036854775808", b"\0\xff"):
            self.assertEqual(profile.key(raw), profile.string(raw))
        for raw in (b"", b"0"):
            with self.assertRaises(ValueError):
                profile.fixture(raw)
        self.assertEqual(profile.fixture(b"\0")["meta_key"], profile.string(b"\0"))

    def test_profile_and_counters_reject_bool_integer_equivalence_and_extra_fields(self):
        profile.validate_profile(copy.deepcopy(profile.PROFILE))
        for key, value in (("single", 1), ("database_calls", False), ("empty_filters", 1),
                           ("unverified", True)):
            changed = copy.deepcopy(profile.PROFILE)
            changed[key] = value
            with self.subTest(profile_field=key), self.assertRaises(ValueError):
                profile.validate_profile(changed)
        changed = copy.deepcopy(profile.PROFILE)
        changed["fixture_limits"]["max_input_bytes"] = float(profile.LIMITS["max_input_bytes"])
        with self.assertRaises(ValueError):
            profile.validate_profile(changed)
        expected = profile.expected_counts([profile.fixture(b"_thumbnail_id", b"0")])
        profile.validate_counts(dict(expected), dict(expected))
        for which in ("actual", "expected"):
            for key in profile.COUNT_KEYS:
                actual, target = dict(expected), dict(expected)
                (actual if which == "actual" else target)[key] = False
                with self.subTest(which=which, field=key), self.assertRaises(ValueError):
                    profile.validate_counts(actual, target)

    def test_evidence_binds_actual_calls_to_generated_cases_and_both_old72_results(self):
        self.coverage()
        cases = profile.cases()
        self.assertEqual(len(cases), 3202)
        expected = profile.expected_counts(cases)
        self.assertEqual(expected["default_hook"], 626)
        for key in profile.COUNT_KEYS:
            changed = dict(expected)
            changed[key] += 1
            with self.subTest(count=key), self.assertRaises(ValueError):
                self.coverage(counts=changed)
        forged = dict.fromkeys(profile.COUNT_KEYS, 0)
        with self.assertRaises(ValueError):
            self.coverage(counts=forged, expected_counts=forged)
        for name, value in (("total", 3201), ("previous_count", profile.OLD_COUNT - 1),
                            ("input_sha", "0" * 64), ("rust_sha", "0" * 64),
                            ("php_sha", "0" * 64)):
            with self.subTest(prefix_field=name), self.assertRaises(ValueError):
                self.coverage(**{name: value})


if __name__ == "__main__":
    unittest.main()
