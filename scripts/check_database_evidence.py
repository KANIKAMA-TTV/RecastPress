"""Check current source/DDL against recorded disposable DB runs; never connects."""

import json
from pathlib import Path

import wordpress_reference as reference

REQUIRED_CHECKS = frozenset([
    "unicode_and_nul", "unsigned_id_boundary", "private_and_custom_status", "zero_dates",
    "null_empty_duplicate_meta", "orphan_meta", "serialized_bytes", "autoload_raw_values",
    "cursor_pagination", "repeatable_snapshot", "read_only_enforcement", "nontransactional_rejection",
    "missing_table_rejection", "binlog_configuration_observation", "zero_id_first_page",
    "taxonomy_raw_values", "taxonomy_composite_cursor", "taxonomy_repeatable_snapshot", "taxonomy_schema_rejection",
    "comment_raw_values", "comment_cursor_and_snapshot", "comment_schema_rejection",
    "user_raw_values", "user_cursor_and_snapshot", "user_schema_rejection",
    "meta_view_prepared_change_equivalence", "meta_view_complete_paged_load", "meta_view_limit_rejects_prefix",
    "meta_view_byte_limit_exactness", "meta_view_preflight_avoids_payload_transfer",
    "meta_index_binary_equivalence", "meta_index_scan_fallback", "meta_index_snapshot_isolation",
    "link_raw_values", "link_cursor_and_snapshot", "link_schema_rejection",
    "budgeted_meta_view_shared_pool_lifetime", "budgeted_meta_view_preflight_refusal",
    "budgeted_meta_view_refusal_avoids_payload_transfer",
    "post_text_cas_exactness", "post_text_cas_conflict", "post_text_cas_unsigned_zero_dates",
    "post_text_cas_truncation_rollback", "post_text_cas_concurrent_writers", "post_text_cas_schema_rejection",
    "user_lookup_text_collation", "user_lookup_snapshot", "user_lookup_profile_rejection",
    "credential_cas_exactness", "credential_cas_conflict", "credential_cas_unsigned_zero_dates",
    "credential_cas_truncation_rollback", "credential_cas_concurrent_writers", "credential_cas_schema_rejection",
    "credential_rehash_pipeline",
    "credential_preprocess_lookup_pipeline",
    "credential_username_email_stage_pipeline",
    "post_name_text_collation", "post_name_text_pagination_snapshot", "post_name_text_profile_rejection",
    "post_name_preprocess_lookup_pipeline",
    "option_meta_selected_decode_pipeline",
])


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate DB evidence field: {key}")
        result[key] = value
    return result


def validate_schema_generator_files(schema):
    expected = {"scripts/wp_schema_export.php", "scripts/reference_functions.php",
                "scripts/export_wordpress_schema.py"}
    files = schema.get("tool_files") if isinstance(schema, dict) else None
    if not isinstance(files, dict) or set(files) != expected:
        raise ValueError("schema generator file identities are incomplete or unknown")
    for path, digest in files.items():
        if type(digest) is not str or digest != reference.digest_file(reference.ROOT / path):
            raise ValueError("schema generator changed; run python scripts/export_wordpress_schema.py before starting fixtures")


def validate_fixture_profile(record, backend):
    """Check recorded fixture identity, without opening a database or binary."""
    profiles = {
        "mariadb": ("11.8.9-MariaDB-log", "loopback_plaintext_test_only"),
        "mysql": ("8.4.13", "verified_CA_TLS"),
    }
    if backend not in profiles or not isinstance(record, dict):
        raise ValueError("unknown DB fixture profile")
    version, transport = profiles[backend]
    if type(record.get("format_version")) is not int or record["format_version"] != 1:
        raise ValueError("DB evidence format version differs")
    if record.get("server_version") != version or record.get("fixture_transport") != transport:
        raise ValueError("DB evidence server/transport profile differs")
    cipher = record.get("tls_cipher")
    if type(cipher) is not str or (backend == "mysql" and not cipher) or (backend == "mariadb" and cipher):
        raise ValueError("DB fixture TLS observation differs")
    limitations = record.get("limitations")
    if not isinstance(limitations, list) or not limitations or any(type(item) is not str or not item for item in limitations):
        raise ValueError("DB evidence limitations are missing")
    for field in ("server_binary_sha256", "tool_archive_sha256"):
        digest = record.get(field)
        if type(digest) is not str or len(digest) != 64 or any(char not in "0123456789abcdef" for char in digest):
            raise ValueError("DB fixture digest is malformed")


def required_files():
    paths = {path.relative_to(reference.ROOT).as_posix()
             for crate in ("recastpress-db", "recastpress-core")
             for folder in ("src", "tests")
             for path in (reference.ROOT / "crates" / crate / folder).rglob("*.rs")}
    # The owned rehash pipeline invokes WP through a test-only dependency.
    # Bind its compiled implementation too; normal DB serving has no WP edge.
    paths.update(path.relative_to(reference.ROOT).as_posix()
                 for path in (reference.ROOT / "crates/recastpress-wp/src").rglob("*.rs"))
    return paths | {"scripts/database_smoke.py", "scripts/setup_mariadb.py", "scripts/setup_mysql.py",
                    "scripts/pinned_zip_tool.py", "scripts/wordpress_reference.py", "scripts/check_database_evidence.py",
                    "Cargo.toml", "Cargo.lock", "crates/recastpress-db/Cargo.toml", "crates/recastpress-wp/Cargo.toml",
                    "crates/recastpress-core/Cargo.toml", "compatibility/schema/reference.json"}


def validate_file_identities(files):
    if not isinstance(files, dict) or set(files) != required_files():
        raise ValueError("DB evidence file identities are missing, substituted or unexpected")


def validate_checks(checks):
    if not isinstance(checks, list) or any(not isinstance(check, str) for check in checks):
        raise ValueError("DB checks must be a list of named observations")
    if len(checks) != len(REQUIRED_CHECKS) or set(checks) != REQUIRED_CHECKS:
        raise ValueError("DB evidence required observations are missing, duplicated or substituted")


def validate():
    root=reference.ROOT.resolve()
    schema=json.loads((root/"compatibility/schema/reference.json").read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    validate_schema_generator_files(schema)
    sql=root/f"compatibility/schema/wordpress-{reference.VERSION}-single-site.sql"
    if schema["wordpress_version"]!=reference.VERSION or reference.digest_file(sql)!=schema["sql_sha256"]:raise ValueError("reference DDL identity changed")
    if schema["scope"]!="reference_DDL_only_not_installation_or_DB_compatibility" or len(set(schema["tables"]))!=12:raise ValueError("reference DDL scope/count mismatch")
    records=[("schema generator",schema["tool_files"])]
    for backend in ["mariadb","mysql"]:
        evidence=json.loads((root/f"compatibility/evidence/database-{backend}.json").read_text(encoding="utf-8"), object_pairs_hook=unique_object)
        validate_fixture_profile(evidence, backend)
        if evidence["wordpress_version"]!=reference.VERSION or evidence["schema_sha256"]!=schema["sql_sha256"]:raise ValueError("DB evidence baseline differs")
        if evidence["scope"]!="isolated_editing_schema_reader_fixture" or not evidence["limitations"]:raise ValueError("DB evidence scope/checks incomplete")
        validate_checks(evidence["checks"])
        validate_file_identities(evidence["tracked_files"])
        if backend=="mysql" and (evidence["fixture_transport"]!="verified_CA_TLS" or not evidence["tls_cipher"]):raise ValueError("MySQL CA/TLS evidence missing")
        records.append((backend,evidence["tracked_files"]))
    for label,files in records:
        if not files:raise ValueError(f"empty file identities: {label}")
        for relative,expected in files.items():
            path=(root/relative).resolve()
            if not path.is_relative_to(root) or reference.digest_file(path)!=expected:raise ValueError(f"{label} evidence is stale: {relative}; regenerate its fixture run")
    print("DB evidence matches current DDL and source: MariaDB/MySQL fixture subsets only")


if __name__=="__main__":
    try:validate()
    except (OSError,ValueError,KeyError,TypeError) as error:raise SystemExit(f"DB evidence check failed: {error}")
