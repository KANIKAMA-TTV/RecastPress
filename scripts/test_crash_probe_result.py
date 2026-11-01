import copy
import json
import unittest

from crash_probe_result import READY_PREFIX, RESULT_PREFIX, parse_crash_result, parse_ready, validate_crash_recovery

NONCE = "ab" * 16
EPOCH = ("mysql-" + "a" * 32).encode()
PRODUCER = {"probe_pid": 100, "postmeta_source_file": "probe-bin.000001", "postmeta_saved_position": 9000, "postmeta_saved_rows": 11}


def payload(value):
    return json.dumps(value, separators=(",", ":")).encode().hex()


def ready_value():
    rows = [[i, 2**64-1, None if i % 2 else [], [0, 255]] for i in range(10)]
    rows.append([2**64-1, 0, [97], None])
    return {"format_version": 1, "nonce": NONCE, "process_pid": 200, "source_connection_id": 10,
            "source_file": "probe-bin.000001", "start_position": 10000, "baseline_position": 11000,
            "target_position": 12000, "rows": 11, "epoch_hex": EPOCH.hex(),
            "old_tuple": {"epoch_hex": EPOCH.hex(), "file_hex": b"probe-bin.000001".hex(),
                          "from_position": 10000, "through_position": 11000,
                          "receipt_hex": payload([[0, 1, 255]]), "row_data_hex": payload(rows)}}


def wire(value, prefix=READY_PREFIX):
    return prefix + json.dumps(value, sort_keys=True, separators=(",", ":")) + "\n"


def parse(value):
    return parse_ready(wire(value), expected_pid=200, nonce=NONCE, epoch=EPOCH, producer=PRODUCER)


def result_value():
    ready = ready_value()
    return {"format_version": 1, "profile": "fixture_postmeta_precommit_process_kill", "nonce": NONCE,
            "original_pid": 200, "process_pid": 300, "source_connection_id": 10,
            **{key: ready[key] for key in ["source_file", "start_position", "baseline_position", "target_position", "rows"]},
            "old_pair_preserved": True, "rollback_drained": True, "source_resumed": True,
            "row_data_matches": True, "index_matches": True, "receipt_validated": True,
            "resumed_commits": 1, "resumed_updates": 1, "sql_disconnect_resets": {}}


def evidence():
    confirmation = {"format_version": 1, "process_pid": 400, "source_file": PRODUCER["postmeta_source_file"],
                    "position": 9000, "rows": 11, "row_data_matches": True, "index_matches": True,
                    "receipt_validated": True, "store_unchanged": True, "sql_disconnect_resets": {}}
    return {"ready": wire(ready_value()), "process_pid": 200, "exit_code": 1,
            "was_running_before_kill": True, "forced_kill": True, "database_stayed_running": True,
            "nonce": NONCE, "epoch_hex": EPOCH.hex(), "result": wire(result_value(), RESULT_PREFIX),
            "resume_pid": 300, "confirmation_result": "POSTMETA_RESTORE_RESULT=" + json.dumps(confirmation),
            "confirmation_pid": 400}


class CrashProbeResultTests(unittest.TestCase):
    def test_complete_marker_result_and_process_evidence(self):
        ready = parse(ready_value())
        parse_crash_result(wire(result_value(), RESULT_PREFIX), ready, expected_pid=300)
        validate_crash_recovery(evidence(), PRODUCER, "mysql")
        with self.assertRaises(ValueError):
            validate_crash_recovery(evidence(), PRODUCER, "mariadb")

    def test_ready_rejects_wrong_identity_types_boundaries_and_noncanonical_json(self):
        for field, wrong in [("format_version", True), ("rows", 11.0), ("process_pid", True),
                             ("process_pid", 201), ("source_connection_id", 0), ("nonce", "0" * 32),
                             ("epoch_hex", b"other".hex()), ("source_file", "../probe-bin.000001"),
                             ("start_position", 9000), ("start_position", True),
                             ("baseline_position", 10000), ("target_position", 2**64)]:
            with self.subTest(field=field, wrong=wrong), self.assertRaises(ValueError):
                parse(dict(ready_value(), **{field: wrong}))
        for invalid in [wire(ready_value()).rstrip("\n"), " " + wire(ready_value()),
                        READY_PREFIX + json.dumps(ready_value()) + "\n",
                        wire(ready_value()).replace('"format_version":1', '"format_version":1,"format_version":1'),
                        wire(dict(ready_value(), unexpected=1)), "x" * 65537]:
            with self.subTest(invalid=invalid[:60]), self.assertRaises(ValueError):
                parse_ready(invalid, expected_pid=200, nonce=NONCE, epoch=EPOCH, producer=PRODUCER)

    def test_old_tuple_context_receipt_and_row_payloads_are_bounded_and_typed(self):
        invalids = [
            ("from_position", True), ("through_position", 12000), ("file_hex", b"probe-bin.000002".hex()),
            ("receipt_hex", payload([])), ("receipt_hex", payload([[1], [2]])), ("receipt_hex", payload([[True]])), ("receipt_hex", "AA"),
            ("row_data_hex", payload([])), ("row_data_hex", "00" * (12 * 1024 + 1)),
        ]
        for rows in [[[0, 0, [True], None]] * 11, [[0, 0, None, None]] * 11,
                     [[i, -1, None, None] for i in range(11)]]:
            invalids.append(("row_data_hex", payload(rows)))
        for field, wrong in invalids:
            marker = ready_value()
            marker["old_tuple"][field] = wrong
            with self.subTest(field=field), self.assertRaises(ValueError):
                parse(marker)

    def test_resume_requires_exact_observations_distinct_pid_and_cleanup_scope(self):
        ready = ready_value()
        for field, wrong in [("process_pid", 200), ("original_pid", 201), ("old_pair_preserved", 1),
                             ("rollback_drained", False), ("resumed_commits", True),
                             ("resumed_updates", 2), ("rows", 11.0), ("target_position", 12001),
                             ("sql_disconnect_resets", {"control_final": True}),
                             ("sql_disconnect_resets", {"control_final": 2}),
                             ("sql_disconnect_resets", {"original_journal_old": 1})]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                parse_crash_result(wire(dict(result_value(), **{field: wrong}), RESULT_PREFIX), ready, expected_pid=300)
        for pid in [None, True, 0, 200]:
            with self.subTest(pid=pid), self.assertRaises(ValueError):
                parse_crash_result(wire(result_value(), RESULT_PREFIX), ready, expected_pid=pid)
        with self.assertRaises(ValueError):
            parse_crash_result(wire(result_value(), RESULT_PREFIX) + " \n", ready, expected_pid=300)

    def test_lifetime_claims_and_final_normal_restore_cannot_be_omitted(self):
        for field, wrong in [("exit_code", 0), ("exit_code", True), ("forced_kill", 1),
                             ("was_running_before_kill", False), ("database_stayed_running", False),
                             ("process_pid", None), ("resume_pid", None), ("confirmation_pid", None),
                             ("confirmation_pid", 401), ("epoch_hex", b"unowned".hex())]:
            with self.subTest(field=field), self.assertRaises(ValueError):
                validate_crash_recovery(dict(evidence(), **{field: wrong}), PRODUCER, "mysql")
        missing = copy.deepcopy(evidence())
        del missing["confirmation_result"]
        with self.assertRaises(ValueError):
            validate_crash_recovery(missing, PRODUCER, "mysql")


if __name__ == "__main__":
    unittest.main()
