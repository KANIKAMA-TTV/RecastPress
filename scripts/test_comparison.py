import unittest
import hashlib
import json
from pathlib import Path

from compare_primitives import batches, canonical, canonical_list_hash, cases, compare


class ComparisonTests(unittest.TestCase):
    def test_fixture_generator_is_executable_unique_and_covers_registered_api_names(self):
        inputs = cases()
        self.assertTrue(inputs)
        self.assertEqual(len({case["id"] for case in inputs}), len(inputs))
        registry = json.loads((Path(__file__).resolve().parents[1] / "compatibility/registry.json").read_text(encoding="utf-8"))
        self.assertEqual({case["api"] for case in inputs}, {api["wordpress_symbol"] for api in registry["apis"]})

    def test_batches_preserve_order_and_account_for_json_envelope_and_case_limits(self):
        inputs = [{"id": str(index), "value": "x" * (10 + index)} for index in range(9)]
        maximum = len(canonical(inputs[:3]))
        packed = list(batches(inputs, max_cases=3, max_bytes=maximum))
        self.assertEqual([case for batch in packed for case in batch], inputs)
        self.assertTrue(all(0 < len(batch) <= 3 and len(canonical(batch)) <= maximum for batch in packed))
        self.assertEqual(list(batches([], max_cases=1, max_bytes=2)), [])
        for limits in [(0, 100), (2, 1), (2, 10)]:
            with self.subTest(limits=limits), self.assertRaises(ValueError):
                list(batches(inputs, max_cases=limits[0], max_bytes=limits[1]))

    def test_streamed_list_hash_keeps_the_existing_whole_json_hash_contract(self):
        for values in [[], [{"b": True, "a": 1}], [{"value": "日本語"}, {"value": False}, {"value": None}]]:
            self.assertEqual(canonical_list_hash(values), hashlib.sha256(canonical(values)).hexdigest())

    def test_missing_reordered_and_duplicate_results_fail(self):
        inputs = [{"id": "one"}, {"id": "two"}]
        results = [{"id": "one", "value": True}, {"id": "two", "value": False}]
        self.assertEqual(compare(inputs, results, results), [])
        with self.assertRaises(ValueError):
            compare(inputs, results[:1], results)
        with self.assertRaises(ValueError):
            compare(inputs, list(reversed(results)), results)
        with self.assertRaises(ValueError):
            compare([{"id": "one"}, {"id": "one"}], [results[0], results[0]], [results[0], results[0]])

    def test_boolean_and_integer_results_are_not_treated_as_equal(self):
        inputs = [{"id": "one"}]
        failures = compare(inputs, [{"id": "one", "value": 1}], [{"id": "one", "value": True}])
        self.assertEqual(len(failures), 1)


if __name__ == "__main__":
    unittest.main()
