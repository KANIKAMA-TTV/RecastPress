"""Validate owned finite DB-save/preparation/sealed-HTTP evidence without SQL.

The receipt checksum identifies bytes; this is neither producer authentication
nor a production publication, theme, rollback-prevention or live-switch proof.
"""
from datetime import datetime, timedelta
import json
import re

from alpha_publication_path import SCOPE
from check_database_evidence import REQUIRED_CHECKS, required_files
import wordpress_reference as reference


MAX_BUNDLE_BYTES = 8 * 1024 * 1024
MAX_EVIDENCE_BYTES = 2 * 1024 * 1024
BACKEND_VERSIONS = {
    "mysql": {"8.4.13"},
    "mariadb": {"11.8.9-MariaDB", "11.8.9-MariaDB-log"},
}
TOP_FLAGS = {
    "mismatched_candidate_rejected", "old_process_exited_before_replacement",
    "public_processes_reaped", "database_stayed_running", "public_environment_scrubbed",
}
TOP_FIELDS = {
    "format_version", "scope", "generated_at_utc", "backend", "server_version",
    "database_pid", "contract_pid", "db_checks", "phases", "rejected_candidate_pid",
    "rejected_candidate_exit_code", "tracked_files", "limitations", *TOP_FLAGS,
}
PHASE_FIELDS = {
    "prepare_pid", "receipt_pid", "payload_bytes", "payload_sha256", "body_bytes",
    "body_sha256", "public_pid", "etag", "get_head_conditional", "exit_code",
}
PHASE_FLAGS = {
    "a": {"frozen_after_source_mutation", "frozen_after_source_deletion"},
    "b": {"old_validator_rejected"},
}


def unique_object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate alpha publication evidence field")
        value[key] = item
    return value


def _pid(value):
    return type(value) is int and 0 < value < 2**32


def _exit_code(value):
    return type(value) is int and -(2**31) <= value < 2**32


def _sha(value):
    return type(value) is str and re.fullmatch(r"[0-9a-f]{64}", value) is not None


def tracked_files():
    return {
        *required_files(), "crates/recastpress-server/Cargo.toml",
        "scripts/alpha_publication_path.py", "scripts/smoke.py", "scripts/owned_probe.py",
        "scripts/check.py", "scripts/check_alpha_publication_evidence.py",
        "scripts/test_alpha_publication_evidence.py", "scripts/test_publication_cli.py",
        "scripts/test_alpha_publication_inputs.py",
        "scripts/test_smoke_lifecycle.py",
        "scripts/test_prepared_posts_html.py", "scripts/test_bundle_check_cli.py",
        *[path.relative_to(reference.ROOT).as_posix()
          for directory in ("src", "tests")
          for path in (reference.ROOT/"crates/recastpress-server"/directory).rglob("*.rs")],
    }


def validate_record(record):
    if type(record) is not dict or set(record) != TOP_FIELDS:
        raise ValueError("alpha publication evidence fields are incomplete or unknown")
    if (type(record["format_version"]) is not int or record["format_version"] != 1
            or type(record["scope"]) is not str or record["scope"] != SCOPE):
        raise ValueError("alpha publication evidence version/scope differs")
    backend = record["backend"]
    if (type(backend) is not str or backend not in BACKEND_VERSIONS
            or type(record["server_version"]) is not str
            or record["server_version"] not in BACKEND_VERSIONS[backend]):
        raise ValueError("alpha publication backend/version profile differs")
    if any(record[flag] is not True for flag in TOP_FLAGS):
        raise ValueError("alpha publication lifecycle/environment observations did not pass")
    timestamp = record["generated_at_utc"]
    if type(timestamp) is not str or "T" not in timestamp:
        raise ValueError("alpha publication timestamp must be a UTC datetime")
    try:
        parsed = datetime.fromisoformat(timestamp)
    except ValueError as error:
        raise ValueError("alpha publication timestamp must be a UTC datetime") from error
    if parsed.utcoffset() != timedelta(0):
        raise ValueError("alpha publication timestamp must be a UTC datetime")
    checks = record["db_checks"]
    if (type(checks) is not list or len(checks) != 62 or len(REQUIRED_CHECKS) != 62
            or any(type(check) is not str for check in checks)
            or checks != sorted(REQUIRED_CHECKS)):
        raise ValueError("alpha publication owned DB contracts differ")
    db_pid, contract_pid = record["database_pid"], record["contract_pid"]
    if not _pid(db_pid) or not _pid(contract_pid) or db_pid == contract_pid:
        raise ValueError("alpha publication owned DB/contract process identities differ")
    phases = record["phases"]
    if type(phases) is not dict or set(phases) != {"a", "b"}:
        raise ValueError("alpha publication must contain exactly A/B phases")
    for phase, flags in PHASE_FLAGS.items():
        value = phases[phase]
        if type(value) is not dict or set(value) != PHASE_FIELDS | flags:
            raise ValueError("alpha publication phase fields are incomplete or unknown")
        if any(value[flag] is not True for flag in flags | {"get_head_conditional"}):
            raise ValueError("alpha publication phase observations did not pass")
        for field in ("prepare_pid", "receipt_pid", "public_pid"):
            # The DB is alive throughout. Sequential worker/public PIDs may be
            # reused after exit; PID values are not permanent process identity.
            if not _pid(value[field]) or value[field] == db_pid:
                raise ValueError("alpha publication process identity conflicts with live DB")
        if not _exit_code(value["exit_code"]):
            raise ValueError("alpha publication public process exit was not observed")
        if (type(value["payload_bytes"]) is not int or type(value["body_bytes"]) is not int
                or not 0 < value["body_bytes"] < value["payload_bytes"] <= MAX_BUNDLE_BYTES):
            raise ValueError("alpha publication byte counts exceed admitted bounds")
        if not _sha(value["payload_sha256"]) or not _sha(value["body_sha256"]):
            raise ValueError("alpha publication digest is not canonical SHA-256")
        if type(value["etag"]) is not str or re.fullmatch(r'"[0-9a-f]{64}"', value["etag"]) is None:
            raise ValueError("alpha publication validator is not the strong sealed tag")
    for field in ("payload_sha256", "body_sha256", "etag"):
        if phases["a"][field] == phases["b"][field]:
            raise ValueError("alpha publication committed A/B change was not observed")
    rejected_pid = record["rejected_candidate_pid"]
    if (not _pid(rejected_pid) or rejected_pid in {db_pid, phases["a"]["public_pid"]}
            or not _exit_code(record["rejected_candidate_exit_code"])
            or record["rejected_candidate_exit_code"] == 0):
        raise ValueError("alpha publication mismatched candidate refusal differs")
    limitations = record["limitations"]
    if (type(limitations) is not list or not limitations
            or any(type(value) is not str or not value.strip() for value in limitations)):
        raise ValueError("alpha publication scope limitations are missing")
    hashes = record["tracked_files"]
    if type(hashes) is not dict or set(hashes) != tracked_files():
        raise ValueError("alpha publication tracked source set differs")
    for path, digest in hashes.items():
        if not _sha(digest) or digest != reference.digest_file(reference.ROOT/path):
            raise ValueError("alpha publication evidence is stale: " + path)


def validate():
    for backend in ("mysql", "mariadb"):
        path = reference.ROOT/f"compatibility/evidence/alpha-publication-{backend}.json"
        with path.open("rb") as source:
            wire = source.read(MAX_EVIDENCE_BYTES+1)
        if len(wire) > MAX_EVIDENCE_BYTES:
            raise ValueError("alpha publication evidence exceeds its read bound")
        record = json.loads(wire, object_pairs_hook=unique_object)
        if type(record) is not dict or record.get("backend") != backend:
            raise ValueError("alpha publication evidence filename/backend differs")
        validate_record(record)
    print("Owned finite alpha post path evidence matches current sources for both DB backends; receipts are checksums, not authorization")


if __name__ == "__main__":
    validate()
