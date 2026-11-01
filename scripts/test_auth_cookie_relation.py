"""Focused public-dummy oracle admission, branch and privacy contracts.

Not the full native/PHP relation or a static WP API comparison. These tests use
only short actual PHP calls and never build Rust or start a database/listener.
"""
import copy
import json
import os
from pathlib import Path
import unittest

import auth_cookie_cases as fixtures
import owned_probe

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / ".reference/php-8.5.12/php.exe"
ORACLE = ROOT / "scripts/auth_cookie_oracle.php"
MANIFEST = ROOT / ".reference/wordpress-7.1.3/recastpress-reference.json"


def wire(value):
    return (json.dumps(value, ensure_ascii=True, separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")


def unique(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate oracle output field")
        value[key] = item
    return value


class AuthCookieCaseContracts(unittest.TestCase):
    def test_uppercase_fixture_is_same_binary_tag_and_raw_name_is_not_canonical_row_login(self):
        cases = {case["name"]: case for case in fixtures.cases(2_000_000_000)}
        first = cases["current_cookie_and_record_session"]
        upper = cases["uppercase_mac_is_not_lowercase_identity"]
        original = bytes.fromhex(first["request_without_now"]["cookie_hex"]).rsplit(b"|", 1)[1]
        changed = bytes.fromhex(upper["request_without_now"]["cookie_hex"]).rsplit(b"|", 1)[1]
        self.assertNotEqual(original, changed)
        self.assertEqual(original.upper(), changed)
        self.assertEqual(bytes.fromhex(original.decode()), bytes.fromhex(changed.decode()))
        self.assertEqual(upper["expected_stage"], "bad_mac")
        alias = cases["raw_username_differs_from_returned_canonical_row"]
        self.assertEqual(alias["expected_stage"], "valid")
        self.assertEqual(alias["request_without_now"]["cookie_hex"], first["request_without_now"]["cookie_hex"])
        self.assertNotEqual(alias["request_without_now"]["user"]["login_hex"], first["request_without_now"]["user"]["login_hex"])

    def test_falsey_tokens_are_manually_signed_validation_cases_and_revocation_reuses_cookie(self):
        cases = fixtures.cases(2_000_000_000)
        selected = {case["name"]: case for case in cases}
        for name, expected in (("manual_empty_token_is_valid_not_generation_input", b""),
                               ("manual_ascii_zero_token_is_valid_not_generation_input", b"0")):
            case = selected[name]
            self.assertEqual(case["expected_stage"], "valid")
            self.assertEqual(bytes.fromhex(case["request_without_now"]["cookie_hex"]).split(b"|")[2], expected)
        current = selected["current_cookie_and_record_session"]["request_without_now"]
        revoked = selected["revoked_same_cookie_as_first_complete_cache_empty"]["request_without_now"]
        self.assertEqual(current["cookie_hex"], revoked["cookie_hex"])
        self.assertTrue(current["sessions"])
        self.assertEqual(revoked["sessions"], [])
        self.assertEqual({case["expected_stage"] for case in cases},
                         {"valid", "malformed", "expired", "unknown_user", "bad_mac", "bad_session"})
        self.assertEqual(len({case["request_without_now"]["id"] for case in cases}), len(cases))
        self.assertTrue(all("now" not in case["request_without_now"] for case in cases))


@unittest.skipUnless(PHP.is_file() and MANIFEST.is_file(), "pinned PHP/WordPress unavailable")
class AuthCookieOracleContracts(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.addClassCleanup(owned_probe.reap_owned_probes)
        from check_password_generation_evidence import verify_pinned
        verify_pinned()

    def setUp(self):
        self.addCleanup(owned_probe.reap_owned_probes)
        self.cases = fixtures.cases()

    def call(self, raw):
        pid, result = owned_probe.run_captured_probe(
            [str(PHP), "-n", "-d", "memory_limit=128M", "-d", "zend.exception_ignore_args=0", str(ORACLE), str(ROOT)],
            os.environ.copy(), timeout=15, input_bytes=raw)
        self.assertLessEqual(len(result.stdout), 1024 * 1024)
        self.assertLessEqual(len(result.stderr), 65536)
        return pid, result

    def refused(self, raw):
        _, result = self.call(raw)
        self.assertEqual((result.returncode, result.stdout, result.stderr),
                         (2, b"", b"Prepared cookie relation failed\n"))

    def test_actual_manager_branches_falsey_tokens_and_all_seed_states_restore_per_case(self):
        names = ("raw_username_differs_from_returned_canonical_row", "manual_empty_token_is_valid_not_generation_input",
                 "manual_ascii_zero_token_is_valid_not_generation_input", "expired_cookie_precedes_missing_user_and_bad_mac",
                 "unselected_expired_rows_are_processed_without_removing_selected", "valid_legacy_integer_session",
                 "missing_user_precedes_bad_mac", "uppercase_mac_is_not_lowercase_identity",
                 "revoked_same_cookie_as_first_complete_cache_empty", "malformed_pipe_count")
        selected = [case for case in self.cases if case["name"] in names]
        pid, result = self.call(wire({"cases": [case["request_without_now"] for case in selected]}))
        self.assertEqual((result.returncode, result.stderr), (0, b""))
        output = json.loads(result.stdout, object_pairs_hook=unique)
        self.assertEqual(set(output), {"format_version", "process_pid", "profile", "cases"})
        self.assertIs(type(output["process_pid"]), int)
        self.assertEqual(output["process_pid"], pid)
        self.assertEqual((output["profile"]["php_version"], output["profile"]["php_int_size"]), ("8.5.12", 8))
        self.assertIs(output["profile"]["builtin_clock"], True)
        self.assertEqual(len(output["cases"]), len(selected))
        for metadata, actual in zip(selected, output["cases"]):
            request = metadata["request_without_now"]
            self.assertEqual(set(actual), {"id", "outcome", "stage", "prepared_user_id", "clock_before", "clock_after",
                                           "clock_stable", "counts", "actions"})
            self.assertEqual(actual["id"], request["id"])
            self.assertEqual(actual["stage"], metadata["expected_stage"])
            matched = metadata["expected_stage"] == "valid"
            self.assertEqual(actual["outcome"], "match" if matched else "no_match")
            self.assertEqual(actual["prepared_user_id"], request["user"]["id"] if matched else None)
            self.assertEqual(actual["counts"], fixtures.expected_counts(metadata["expected_stage"]))
            self.assertTrue(all(type(value) is int for value in actual["counts"].values()))
            self.assertEqual(actual["actions"], fixtures.expected_actions(metadata["expected_stage"]))
            self.assertTrue(all(type(value) is int for value in actual["actions"].values()))
            before, after = actual["clock_before"], actual["clock_after"]
            self.assertIs(type(before), int)
            self.assertIs(type(after), int)
            self.assertLessEqual(before, after)
            self.assertLessEqual(after - before, 2)
            self.assertIs(actual["clock_stable"], True)
            pieces = bytes.fromhex(request["cookie_hex"]).split(b"|")
            if len(pieces) == 4:
                expiry = int(pieces[1])
                self.assertEqual(expiry < before, expiry < after)
            for session in request["sessions"]:
                expiry = int(session["expiration"])
                self.assertEqual(expiry >= before, expiry >= after)

    def test_unknown_duplicate_and_native_only_now_fields_fail_before_partial_case_output(self):
        original = self.cases[0]["request_without_now"]
        for added in ({"PRIVATE_RELATION_CANARY": "PRIVATE_SECRET"}, {"now": "2000000000"}):
            self.refused(wire({"cases": [original, {**original, "id": 99, **added}]}))
        raw = wire({"cases": [original]})
        duplicate = raw.replace(b'"cookie_hex":', b'"cookie\\u005fhex":"31","cookie_hex":', 1)
        self.assertNotEqual(duplicate, raw)
        self.assertEqual(json.loads(duplicate), json.loads(raw), "the counterexample must be duplicate decoded keys, not an unknown key")
        self.refused(duplicate)

    def test_caps_and_unselected_session_invalidity_are_not_wp_false_results(self):
        good = self.cases[0]["request_without_now"]
        bad = copy.deepcopy(good)
        bad["user"]["hash_hex"] = "00" * 65536
        self.refused(wire({"cases": [bad]}))
        for key, value in (("max_input_bytes", 65535), ("max_sessions", True), ("max_session_key_bytes", 4096.0)):
            bad = copy.deepcopy(good)
            bad["limits"][key] = value
            self.refused(wire({"cases": [bad]}))
        bad = copy.deepcopy(good)
        bad["sessions"].append({"verifier": "0" * 64, "expiration": "9223372036854775808", "legacy": False})
        self.refused(wire({"cases": [bad]}))
        bad = copy.deepcopy(good)
        bad["sessions"].append(copy.deepcopy(bad["sessions"][0]))
        self.refused(wire({"cases": [bad]}))
        bad = copy.deepcopy(good)
        bad["sessions"].append({"verifier": "0" * 64, "expiration": "2000000000", "legacy": 1})
        self.refused(wire({"cases": [bad]}))

    def test_environment_cookie_branch_and_noncanonical_expiry_are_profile_refusals(self):
        for cookie in (b"", b"0", b"user|+1|token|tag", b"user|01|token|tag", b"user|-0|token|tag"):
            bad = copy.deepcopy(self.cases[0]["request_without_now"])
            bad["cookie_hex"] = cookie.hex()
            self.refused(wire({"cases": [bad]}))


if __name__ == "__main__":
    unittest.main()
