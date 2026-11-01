import copy
import json
import unittest

from mysql_overlap_result import FIXED, PREFIX, parse_overlap_result


def valid():
    return {**FIXED, "process_pid": 1234,
            "a_gtid_set": "01234567-89ab-cdef-0123-456789abcdef:1-9:11",
            "floor_position": 1000, "c_position": 2000, "strict_position": 3000,
            "prefix_groups_skipped": 10, "sql_disconnect_resets": {}}


def wire(value):
    return PREFIX + json.dumps(value) + "\n"


class OverlapResultTests(unittest.TestCase):
    def test_owned_child_and_exact_profile(self):
        self.assertEqual(parse_overlap_result(wire(valid()), expected_pid=1234), valid())
        for key in FIXED:
            changed = valid()
            changed[key] = False if type(changed[key]) is bool else None
            with self.subTest(key=key), self.assertRaises(ValueError):
                parse_overlap_result(wire(changed), expected_pid=1234)

    def test_numeric_bool_pid_and_positions_are_not_interchangeable(self):
        for key, value in (("process_pid", True), ("process_pid", 999),
                           ("floor_position", True), ("floor_position", 4),
                           ("c_position", 3000), ("strict_position", 2**64),
                           ("prefix_groups_skipped", 0), ("prefix_groups_skipped", 999), ("rows", 12.0)):
            changed = valid(); changed[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(ValueError):
                parse_overlap_result(wire(changed), expected_pid=1234)
        with self.assertRaises(ValueError):
            parse_overlap_result(wire(valid()), expected_pid=True)

    def test_gtid_intervals_and_cleanup_are_strict(self):
        base = valid()["a_gtid_set"].split(":")[0]
        for suffix in (":0", ":2-1", ":1-9:8", ":9223372036854775808", ":1,other:2", ":tag:1"):
            changed = valid(); changed["a_gtid_set"] = base + suffix
            with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                parse_overlap_result(wire(changed), expected_pid=1234)
        for resets in ({"control_final": True}, {"unknown": 1}, {"control_final": 2}):
            changed = valid(); changed["sql_disconnect_resets"] = resets
            with self.assertRaises(ValueError):
                parse_overlap_result(wire(changed), expected_pid=1234)

    def test_wire_duplicates_extra_fields_and_multiline_refused(self):
        good = wire(valid())
        extra = copy.deepcopy(valid()); extra["unreviewed"] = True
        duplicate = good.replace('"process_pid": 1234', '"process_pid": 1234, "process_pid": 1234')
        for candidate in (good.strip(), good + "\n", good.replace("\n", "\r\n"),
                          "prefix" + good, wire(extra), duplicate, PREFIX + "[]\n",
                          PREFIX + " " * 65536 + "\n"):
            with self.assertRaises(ValueError):
                parse_overlap_result(candidate, expected_pid=1234)


if __name__ == "__main__":
    unittest.main()
