import json
import unittest
from pathlib import Path
import tempfile
from unittest.mock import patch
import wordpress_reference as reference

from check_database_evidence import REQUIRED_CHECKS, required_files, unique_object, validate_checks, validate_file_identities, validate_fixture_profile
from check_database_evidence import validate_schema_generator_files


class DatabaseEvidenceTests(unittest.TestCase):
    def test_schema_generator_changes_are_refused_before_fixture_startup(self):
        parent = reference.ROOT / "target" / "schema-generator-tests"
        parent.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=parent) as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            paths = ["scripts/wp_schema_export.php", "scripts/reference_functions.php", "scripts/export_wordpress_schema.py"]
            for path in paths:
                (root / path).write_text("fixed fixture source", encoding="utf-8")
            schema = {"tool_files": {path: reference.digest_file(root / path) for path in paths}}
            with patch.object(reference, "ROOT", root):
                validate_schema_generator_files(schema)
                for files in ({}, {**schema["tool_files"], "../outside": "ignored"},
                              {**schema["tool_files"], paths[0]: False}):
                    with self.assertRaises(ValueError):
                        validate_schema_generator_files({"tool_files": files})
                (root / paths[1]).write_text("new cache implementation", encoding="utf-8")
                with self.assertRaisesRegex(ValueError, "before starting fixtures"):
                    validate_schema_generator_files(schema)

    def test_backend_identity_transport_and_numeric_types_are_required(self):
        for backend, version, transport, cipher in [
            ("mysql", "8.4.13", "verified_CA_TLS", "TLS_AES_256_GCM_SHA384"),
            ("mariadb", "11.8.9-MariaDB-log", "loopback_plaintext_test_only", ""),
        ]:
            valid = {"format_version": 1, "server_version": version, "fixture_transport": transport,
                     "tls_cipher": cipher, "limitations": ["owned fixture only"],
                     "server_binary_sha256": "a" * 64, "tool_archive_sha256": "b" * 64}
            validate_fixture_profile(valid, backend)
            for field, wrong in [("format_version", True), ("format_version", 1.0),
                                 ("server_version", "unverified-version"), ("fixture_transport", "unknown"),
                                 ("tls_cipher", None), ("limitations", "not a list"), ("limitations", [""]),
                                 ("server_binary_sha256", "A" * 64), ("tool_archive_sha256", "b" * 63)]:
                with self.subTest(backend=backend, field=field), self.assertRaises(ValueError):
                    validate_fixture_profile(dict(valid, **{field: wrong}), backend)
            with self.assertRaises(ValueError):
                validate_fixture_profile(valid, "mariadb" if backend == "mysql" else "mysql")
            with self.assertRaises(ValueError):
                validate_fixture_profile(dict(valid, tls_cipher="" if backend == "mysql" else "unexpected TLS"), backend)

    def test_duplicate_evidence_fields_are_rejected_at_every_depth(self):
        for wire in ['{"format_version":1,"format_version":1}', '{"files":{"same":"a","same":"a"}}']:
            with self.subTest(wire=wire), self.assertRaisesRegex(ValueError, "duplicate"):
                json.loads(wire, object_pairs_hook=unique_object)

    def test_substituted_same_count_and_duplicate_observations_are_rejected(self):
        valid = sorted(REQUIRED_CHECKS)
        self.assertEqual(len(valid), 62)
        self.assertIn("option_meta_selected_decode_pipeline", valid)
        validate_checks(valid)
        for invalid in [valid[:-1] + ["not-actually-tested"], valid[:-1] + [valid[0]], valid + [valid[0]], valid[:-1], None, [None] * len(valid)]:
            with self.subTest(invalid=invalid), self.assertRaises(ValueError):
                validate_checks(invalid)

    def test_file_omission_and_same_count_substitution_are_rejected(self):
        valid = {name: "fixture-hash" for name in required_files()}
        validate_file_identities(valid)
        removed = next(iter(valid))
        missing = {name: value for name, value in valid.items() if name != removed}
        substituted = dict(missing, **{"unrelated.rs": "fixture-hash"})
        for invalid in (missing, substituted, {}, None):
            with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "identities"):
                validate_file_identities(invalid)
        self.assertIn("crates/recastpress-db/src/meta_view.rs", valid)
        self.assertIn("crates/recastpress-db/src/post_name_lookup.rs", valid)
        self.assertIn("crates/recastpress-db/src/lib.rs", valid)
        self.assertIn("crates/recastpress-db/Cargo.toml", valid)
        self.assertIn("crates/recastpress-core/src/resource_budget.rs", valid)
        self.assertIn("crates/recastpress-core/src/lib.rs", valid)
        self.assertIn("crates/recastpress-core/Cargo.toml", valid)
        # The owned credential rehash test depends on these implementations.
        # Omitting this edge could accept a DB proof after its KDF code changed.
        for path in ("Cargo.lock", "crates/recastpress-wp/Cargo.toml",
                     "crates/recastpress-wp/src/lib.rs", "crates/recastpress-wp/src/password_dispatch.rs",
                     "crates/recastpress-wp/src/password_hash.rs", "crates/recastpress-wp/src/password_rehash.rs",
                     "crates/recastpress-wp/src/password_legacy.rs", "crates/recastpress-wp/src/password_verify.rs",
                     "crates/recastpress-wp/src/credential_stage.rs", "crates/recastpress-wp/src/credential_input.rs",
                     "crates/recastpress-wp/src/sanitize_title.rs", "crates/recastpress-wp/src/post_slug_truncate.rs",
                     "crates/recastpress-wp/src/serialization.rs", "crates/recastpress-wp/src/value.rs"):
            self.assertIn(path, valid)


if __name__ == "__main__":
    unittest.main()
