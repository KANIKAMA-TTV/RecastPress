import unittest

from check_binlog_evidence import validate_numeric_observations


def metadata():
    return {
        "format_version": 1,
        "commits": 3,
        "committed_row_changes": 3,
        "buffer_applied_batches": 3,
        "fixture_durable_batches": 3,
        "fixture_duplicate_receipts": 1,
        "missing_log_error_code": 1236,
        "postmeta_durable_commits": 3,
        "postmeta_durable_replays": 1,
        "postmeta_changes": 5,
        "postmeta_memory_commits": 3,
        "postmeta_limited_commits": 3,
        "postmeta_byte_limited_indexes": 3,
        "postmeta_shared_pool_commits": 3,
        "typed_rows_verified": 5,
        "event_count": 19,
        "verified_checksums": 17,
        "derived_interior_positions": 0,
        "typed_unsigned_reinterpreted": 0,
    }


class BinlogEvidenceTests(unittest.TestCase):
    def test_current_mysql_and_mariadb_integer_observations_are_accepted(self):
        validate_numeric_observations(metadata())
        maria = metadata()
        maria.update(verified_checksums=19, derived_interior_positions=9, typed_unsigned_reinterpreted=2)
        validate_numeric_observations(maria)

    def test_boolean_float_and_missing_numeric_fields_are_rejected(self):
        valid = metadata()
        for field, value in valid.items():
            for replacement in (True, float(value), None):
                invalid = dict(valid)
                invalid[field] = replacement
                with self.subTest(field=field, replacement=replacement), self.assertRaises(ValueError):
                    validate_numeric_observations(invalid)
            missing = dict(valid)
            del missing[field]
            with self.subTest(missing=field), self.assertRaises(ValueError):
                validate_numeric_observations(missing)

    def test_wrong_counts_and_inconsistent_bounds_are_rejected(self):
        for field, value in [
            ("commits", 4), ("fixture_duplicate_receipts", 0),
            ("event_count", 0), ("verified_checksums", 20),
            ("verified_checksums", 0), ("derived_interior_positions", -1),
            ("typed_unsigned_reinterpreted", -1), ("typed_unsigned_reinterpreted", 3),
        ]:
            invalid = metadata()
            invalid[field] = value
            with self.subTest(field=field, replacement=value), self.assertRaises(ValueError):
                validate_numeric_observations(invalid)

    def test_nonmapping_observations_are_rejected(self):
        for invalid in (None, [], True, 1):
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                validate_numeric_observations(invalid)


if __name__ == "__main__":
    unittest.main()
