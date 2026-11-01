import copy
import json
import unittest

from binlog_probe_result import EXPECTED, parse_result, parse_restore_result, exact, RESTORE_REJECTIONS, validate_restore_rejection, validate_process_restore
from check_binlog_evidence import validate_server_profile


def result(backend="mysql"):
    value = copy.deepcopy(EXPECTED)
    maria = backend == "mariadb"
    value.update({"probe_pid":100,"postmeta_source_file":"probe-bin.000001","postmeta_saved_position":9000,"sql_disconnect_resets":{},"events": 19, "checksums": 19 if maria else 17, "derived_positions": 9 if maria else 0,
                  "ddl_prefix_rejected": maria, "missing_log_driver_fatal": False, "typed_unsigned_reinterpreted": 2,
                  "event_types": {"2": 6, "4": 2, "15": 2, "16": 3, "19": 3, "23": 1, "24": 1, "25": 1} if maria else
                  {"2": 3, "4": 2, "15": 2, "16": 3, "19": 3, "30": 1, "31": 1, "32": 1, "34": 3}})
    value["maria_snapshot_anchor"] = ({"profile":"mariadb_11_8_9", "source_file":"probe-bin.000001",
        "position":5000, "global_after":5100, "stable_during_transaction":True,
        "old_row_visible_after_commit":True, "readonly_write_rejected":True, "readonly_error_code":1792} if maria else None)
    value["mysql_snapshot_race"] = (None if maria else {"profile":"mysql_8_4_13", "source_file":"probe-bin.000001",
        "before_position":5000, "reported_position":5100, "old_row_visible_after_commit":True,
        "new_row_visible_to_writer":True, "reported_position_includes_unseen_commit":True,
        "readonly_write_rejected":True, "readonly_error_code":1792})
    value["filtered_postmeta"] = {"format_version":1,"profile":"fixture_same_db_filtered_postmeta",
        "source_file":"probe-bin.000001","start_position":10000,"end_position":11000,
        "rows":11,"commits":3,"selected_changes":3,"ignored_changes":2,
        "mixed_commit_verified":True,"ignored_only_commit_verified":True,"apply_failure_preserved":True,
        "ack_loss_replayed":True,"receipt_includes_ignored":True,"ignored_crc_rejected":True,
        "ignored_gap_rejected":True,"ignored_ddl_rejected":True,"ignored_ddl_prefix_rejected":maria,
        "wrong_filter_rejected":True,"row_data_matches":True,"index_matches":True,"cross_database_live_verified":False}
    return value


def wire(value):
    return "BINLOG_PROBE_RESULT=" + json.dumps(value)


class BinlogProbeResultTests(unittest.TestCase):
    def test_filtered_commit_checks_keep_strict_numeric_boolean_and_backend_types(self):
        for backend in ["mysql","mariadb"]:
            valid = result(backend)
            self.assertEqual(parse_result(wire(valid),backend),valid)
            mutations=[("rows",True),("format_version",1.0),("commits",2),("selected_changes",True),
                       ("ignored_changes",3),("cross_database_live_verified",0),
                       ("cross_database_live_verified",True),("ignored_ddl_prefix_rejected",backend!="mariadb")]
            for field in ["mixed_commit_verified","ignored_only_commit_verified","apply_failure_preserved",
                          "ack_loss_replayed","receipt_includes_ignored","ignored_crc_rejected",
                          "ignored_gap_rejected","ignored_ddl_rejected","wrong_filter_rejected",
                          "row_data_matches","index_matches"]:
                mutations.extend([(field,False),(field,1)])
            for field,wrong in mutations:
                invalid=copy.deepcopy(valid)
                invalid["filtered_postmeta"][field]=wrong
                with self.subTest(backend=backend,field=field),self.assertRaises(ValueError):
                    parse_result(wire(invalid),backend)

    def test_filtered_boundaries_and_complete_object_are_required(self):
        valid=result()
        for replacement in [None,{},[],True]:
            invalid=copy.deepcopy(valid);invalid["filtered_postmeta"]=replacement
            with self.subTest(replacement=replacement),self.assertRaises(ValueError):
                parse_result(wire(invalid),"mysql")
        for field,wrong in [("source_file","probe-bin.000002"),("start_position",True),
                            ("start_position",9000),("end_position",10000),("end_position",11000.0),
                            ("end_position",2**64),("profile","unverified-profile"),("unexpected",True)]:
            invalid=copy.deepcopy(valid);invalid["filtered_postmeta"][field]=wrong
            with self.subTest(field=field),self.assertRaises(ValueError):
                parse_result(wire(invalid),"mysql")

    def test_evidence_server_and_transport_profiles_are_exact(self):
        for backend, valid, other in [
            ("mysql", {"server_version": "8.4.13", "transport": "ca_and_name_verified_tls"},
             {"server_version": "11.8.9-MariaDB-log", "transport": "loopback_plaintext_test_only"}),
            ("mariadb", {"server_version": "11.8.9-MariaDB-log", "transport": "loopback_plaintext_test_only"},
             {"server_version": "8.4.13", "transport": "ca_and_name_verified_tls"}),
        ]:
            validate_server_profile(valid, backend)
            for field in valid:
                for replacement in (None, True, 1, [], {}, valid[field] + "-other", other[field]):
                    invalid = dict(valid)
                    invalid[field] = replacement
                    with self.subTest(backend=backend, field=field, replacement=replacement), self.assertRaises(ValueError):
                        validate_server_profile(invalid, backend)
                invalid = dict(valid)
                del invalid[field]
                with self.subTest(backend=backend, missing=field), self.assertRaises(ValueError):
                    validate_server_profile(invalid, backend)
        for record, backend in [(None, "mysql"), ([], "mariadb"), ({}, "unknown")]:
            with self.subTest(record=record, backend=backend), self.assertRaises(ValueError):
                validate_server_profile(record, backend)

    def test_snapshot_experiments_require_the_correct_backend_and_complete_object(self):
        for backend, active, inactive in [("mysql", "mysql_snapshot_race", "maria_snapshot_anchor"),
                                           ("mariadb", "maria_snapshot_anchor", "mysql_snapshot_race")]:
            valid = result(backend)
            self.assertEqual(parse_result(wire(valid), backend), valid)
            for replacement in (None, {}, [], True, 1):
                invalid = copy.deepcopy(valid)
                invalid[active] = replacement
                with self.subTest(backend=backend, replacement=replacement), self.assertRaises(ValueError):
                    parse_result(wire(invalid), backend)
            invalid = copy.deepcopy(valid)
            invalid[inactive] = result("mariadb" if backend == "mysql" else "mysql")[inactive]
            with self.subTest(backend=backend, inactive=inactive), self.assertRaises(ValueError):
                parse_result(wire(invalid), backend)
            invalid = copy.deepcopy(valid)
            invalid[active], invalid[inactive] = invalid[inactive], invalid[active]
            with self.subTest(swapped=backend), self.assertRaises(ValueError):
                parse_result(wire(invalid), backend)
            for field in valid[active]:
                invalid = copy.deepcopy(valid)
                del invalid[active][field]
                with self.subTest(backend=backend, missing=field), self.assertRaises(ValueError):
                    parse_result(wire(invalid), backend)
            invalid = copy.deepcopy(valid)
            invalid[active]["unknown"] = True
            with self.subTest(extra=backend), self.assertRaises(ValueError):
                parse_result(wire(invalid), backend)

    def test_snapshot_experiment_fixed_fields_have_exact_types_and_values(self):
        for backend, active in [("mysql", "mysql_snapshot_race"), ("mariadb", "maria_snapshot_anchor")]:
            valid = result(backend)
            for field, expected in valid[active].items():
                if type(expected) is bool:
                    replacements = (False, 1, "true", None)
                elif field == "readonly_error_code":
                    replacements = (True, 1792.0, 1793, "1792", None)
                elif field == "profile":
                    replacements = ("other_profile", None, True)
                else:
                    continue
                for replacement in replacements:
                    invalid = copy.deepcopy(valid)
                    invalid[active][field] = replacement
                    with self.subTest(backend=backend, field=field, replacement=replacement), self.assertRaises(ValueError):
                        parse_result(wire(invalid), backend)

    def test_snapshot_source_files_and_positions_are_bounded_and_increasing(self):
        for backend, active, positions in [("mysql", "mysql_snapshot_race", ("before_position", "reported_position")),
                                            ("mariadb", "maria_snapshot_anchor", ("position", "global_after"))]:
            valid = result(backend)
            for name in ("", "../probe-bin.000001", "probe-bin.00001", "probe-bin.0000001", "probe-bin.１２３４５６", "probe-bin.000001\n", 1, None):
                invalid = copy.deepcopy(valid)
                invalid[active]["source_file"] = name
                with self.subTest(backend=backend, file=name), self.assertRaises(ValueError):
                    parse_result(wire(invalid), backend)
            for field in positions:
                for replacement in (True, 5.0, "5", None, -1, 0, 4, 2**64):
                    invalid = copy.deepcopy(valid)
                    invalid[active][field] = replacement
                    with self.subTest(backend=backend, field=field, replacement=replacement), self.assertRaises(ValueError):
                        parse_result(wire(invalid), backend)
            for before, after in [(5100, 5100), (5200, 5100)]:
                invalid = copy.deepcopy(valid)
                invalid[active].update(zip(positions, (before, after)))
                with self.subTest(backend=backend, before=before, after=after), self.assertRaises(ValueError):
                    parse_result(wire(invalid), backend)
            valid[active].update(zip(positions, (5, 2**64 - 1)))
            self.assertEqual(parse_result(wire(valid), backend), valid)

    def test_snapshot_reader_cleanup_labels_are_backend_specific_and_integer(self):
        for backend, allowed, other in [("mysql", "mysql_race_reader", "maria_anchor_reader"),
                                         ("mariadb", "maria_anchor_reader", "mysql_race_reader")]:
            valid = result(backend)
            valid["sql_disconnect_resets"] = {allowed: 1}
            self.assertEqual(parse_result(wire(valid), backend), valid)
            for resets in ({other: 1}, {allowed: True}, {allowed: 1.0}, {allowed: 0}, {allowed: -1}):
                invalid = copy.deepcopy(valid)
                invalid["sql_disconnect_resets"] = resets
                with self.subTest(backend=backend, resets=resets), self.assertRaises(ValueError):
                    parse_result(wire(invalid), backend)

    def test_nontext_protocol_values_are_rejected(self):
        for wire in (None, 1, {}, [], b"not text"):
            with self.subTest(wire=wire), self.assertRaises(ValueError):
                parse_result(wire, "mysql")
            with self.subTest(restore_wire=wire), self.assertRaises(ValueError):
                parse_restore_result(wire, result())

    def test_nested_metadata_preserves_boolean_integer_distinctions(self):
        self.assertTrue(exact({"close":{"old":1}}, {"close":{"old":1}}))
        self.assertFalse(exact({"close":{"old":True}}, {"close":{"old":1}}))
        self.assertFalse(exact({"close":{"old":1,"extra":2}}, {"close":{"old":1}}))

    def test_both_backend_profiles_and_driver_fatal_observations(self):
        for backend in ["mysql", "mariadb"]:
            value = result(backend)
            self.assertEqual(parse_result(wire(value), backend), value)
            value["missing_log_driver_fatal"] = True
            self.assertEqual(parse_result(wire(value), backend), value)

    def test_incomplete_substituted_boolean_integer_and_counter_results_fail(self):
        for field, wrong in [("probe_pid",True), ("postmeta_source_file","../probe-bin.000001"), ("postmeta_saved_position",True), ("postmeta_saved_rows",0), ("sql_disconnect_resets", {"original_journal_old":True}), ("sql_disconnect_resets", {"unexpected_stage":1}), ("postmeta_source_resumed",False), ("postmeta_rollback_preserved",False), ("postmeta_context_rejected",1), ("postmeta_durable_replays",0), ("postmeta_projection_reopened",False), ("postmeta_changes",4), ("postmeta_memory_commits",True), ("postmeta_projected_matches",False), ("postmeta_apply_refusal_preserved",1), ("rows", True), ("retry_preserved", 1), ("events", True), ("checksums", 20),
                             ("mutations", [True, 1, 1]), ("derived_positions", -1), ("missing_log_driver_fatal", "false"), ("typed_unsigned_reinterpreted", 3)]:
            value = result()
            value[field] = wrong
            with self.subTest(field=field), self.assertRaises(ValueError):
                parse_result(wire(value), "mysql")
        value = result()
        del value["wrong_context_rejected"]
        with self.assertRaises(ValueError):
            parse_result(wire(value), "mysql")
        value = result()
        value["unknown"] = True
        with self.assertRaises(ValueError):
            parse_result(wire(value), "mysql")
        with self.assertRaises(ValueError):
            parse_result(wire(result()), "mariadb")

    def test_duplicate_json_keys_multiline_and_inconsistent_event_maps_fail(self):
        valid = wire(result())
        for invalid in [valid[:-1] + ',"rows":3}', valid + "\n" + valid, "{}", "BINLOG_PROBE_RESULT=[]"]:
            with self.subTest(wire=invalid), self.assertRaises(ValueError):
                parse_result(invalid, "mysql")
        for kinds in [{"4": 2, "15": 2, "16": 3}, {"04": 2, "15": 2, "16": 15}, {"4": True, "15": 2, "16": 16}, {"256": 19}]:
            value = result()
            value["event_types"] = kinds
            with self.subTest(kinds=kinds), self.assertRaises(ValueError):
                parse_result(wire(value), "mysql")

    def test_distinct_restore_process_matches_the_saved_boundary(self):
        producer = result()
        restored = {"format_version":1,"process_pid":200,"source_file":producer["postmeta_source_file"],
                    "position":producer["postmeta_saved_position"],"rows":producer["postmeta_saved_rows"],
                    "row_data_matches":True,"index_matches":True,"receipt_validated":True,"store_unchanged":True,
                    "sql_disconnect_resets":{}}
        def encoded(value): return "POSTMETA_RESTORE_RESULT=" + json.dumps(value)
        self.assertEqual(parse_restore_result(encoded(restored),producer,expected_pid=200),restored)
        with self.assertRaises(ValueError):
            parse_restore_result(encoded(restored),producer,expected_pid=True)
        for field,value in [("process_pid",100),("process_pid",201),("process_pid",True),
                            ("position",8999),("rows",10),("receipt_validated",1),("store_unchanged",False),
                            ("source_file","probe-bin.000002"),("sql_disconnect_resets",{"control_final":True})]:
            invalid=dict(restored); invalid[field]=value
            with self.subTest(field=field), self.assertRaises(ValueError):
                parse_restore_result(encoded(invalid),producer,expected_pid=200)

    def test_restore_duplicate_fields_unknown_fields_and_extra_output_fail(self):
        producer=result()
        restored={"format_version":1,"process_pid":200,"source_file":producer["postmeta_source_file"],
                  "position":9000,"rows":11,"row_data_matches":True,"index_matches":True,
                  "receipt_validated":True,"store_unchanged":True,"sql_disconnect_resets":{}}
        valid="POSTMETA_RESTORE_RESULT="+json.dumps(restored)
        for invalid in (valid[:-1]+',"rows":11}',valid+"\n"+valid,valid[:-1]+',"unknown":true}',"POSTMETA_RESTORE_RESULT=[]"):
            with self.subTest(wire=invalid), self.assertRaises(ValueError):
                parse_restore_result(invalid,producer,expected_pid=200)

    def test_restore_refusal_requires_the_specific_reason_not_any_failed_process(self):
        for case,(stage,reason) in RESTORE_REJECTIONS.items():
            valid={"case":case,"process_pid":300,"exit_code":1,"stderr":f"Binlog capability probe failed in {stage}: {reason}"}
            validate_restore_rejection(valid)
            for field,value in [("exit_code",True),("exit_code",2),("process_pid",True),("stderr","connection refused")]:
                invalid=dict(valid);invalid[field]=value
                with self.subTest(case=case,field=field),self.assertRaises(ValueError):
                    validate_restore_rejection(invalid)

    def test_process_restore_requires_all_negative_cases_and_a_final_positive_confirmation(self):
        producer=result()
        def restored(pid):
            return "POSTMETA_RESTORE_RESULT="+json.dumps({"format_version":1,"process_pid":pid,"source_file":producer["postmeta_source_file"],"position":9000,"rows":11,"row_data_matches":True,"index_matches":True,"receipt_validated":True,"store_unchanged":True,"sql_disconnect_resets":{}})
        refused=[{"case":case,"process_pid":300+index,"exit_code":1,"stderr":f"Binlog capability probe failed in {stage}: {reason}"} for index,(case,(stage,reason)) in enumerate(RESTORE_REJECTIONS.items())]
        valid={"result":restored(200),"producer_pid":100,"restore_pid":200,"producer_exited_before_restore":True,"database_stayed_running":True,"rejections":refused,"confirmation_result":restored(400),"confirmation_pid":400}
        validate_process_restore(valid,producer)
        for field,value in [("rejections",refused[:-1]),("rejections",refused[:-1]+[refused[0]]),("restore_pid",None),("confirmation_pid",None),("confirmation_pid",False),("producer_exited_before_restore",1),("database_stayed_running",False)]:
            invalid=dict(valid);invalid[field]=value
            with self.subTest(field=field),self.assertRaises(ValueError):
                validate_process_restore(invalid,producer)


if __name__ == "__main__":
    unittest.main()
