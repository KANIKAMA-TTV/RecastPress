"""Owned finite DB-save -> explicit preparation -> sealed HTTP experiment.

The caller supplies only the fixture's explicitly selected committed text.
No general WordPress visibility rule, login, theme engine or live switch exists.
"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import subprocess

import wordpress_reference as reference
from check_database_evidence import REQUIRED_CHECKS
from owned_probe import run_captured_probe, require_success_output
from smoke import start, stop, request

SCOPE = "candidate_owned_post_text_to_sealed_http_only"
TARGET = "/alpha-post"


def public_environment(environment):
    """Do not pass editing fixture credentials/configuration to public children.

    This is an explicit environment boundary, not OS-user/container isolation.
    """
    return {key: value for key, value in environment.items()
        if not key.upper().startswith(("RECASTPRESS_DB_", "MYSQL_", "MARIADB_", "RECASTPRESS_ALPHA_"))
        and key.upper() != "DATABASE_URL"}


def validate_inputs(value):
    if type(value) is not dict or set(value) != {"a", "b"}:
        raise ValueError("alpha fixture must provide two explicit prepared inputs")
    for item in value.values():
        if (type(item) is not dict or set(item) != {"format_version", "posts"}
                or type(item["format_version"]) is not int or item["format_version"] != 1
                or type(item["posts"]) is not list or len(item["posts"]) != 1):
            raise ValueError("alpha prepared input shape differs")
        post = item["posts"][0]
        if (type(post) is not dict or set(post) != {"target", "title", "body_text"}
                or post["target"] != TARGET
                or any(type(post[key]) is not str for key in post)):
            raise ValueError("alpha post is not the explicitly selected text projection")
    if value["a"] == value["b"]:
        raise ValueError("alpha committed input did not change")


def run_experiment(*, root, work, backend, result, contract_pid, database_pid, database_alive):
    from check_alpha_publication_evidence import tracked_files, validate_record
    if backend not in {"mysql", "mariadb"} or set(result["checks"]) != REQUIRED_CHECKS:
        raise ValueError("alpha experiment requires the complete owned DB contracts")
    inputs = result["alpha_publication_inputs"]
    validate_inputs(inputs)
    if not database_alive():
        raise ValueError("owned editing DB exited before publication preparation")
    binary = root/"target/debug"/("recastpress.exe" if os.name == "nt" else "recastpress")
    environment = public_environment(os.environ.copy())
    # Explicit injected canaries prove this filter is applied, rather than
    # assuming the parent happens not to have editing credentials today.
    filtered = public_environment({**environment, "RECASTPRESS_DB_PASSWORD": "DO_NOT_PASS",
        "MYSQL_PWD": "DO_NOT_PASS", "DATABASE_URL": "DO_NOT_PASS"})
    if filtered != environment:
        raise ValueError("public environment credential filter failed")
    records = {}
    bodies = {}
    paths = {}
    for phase in ("a", "b"):
        source = work/f"prepared-{phase}.json"
        payload = work/f"public-{phase}.json"
        receipt = work/f"public-{phase}.receipt.json"
        source.write_text(json.dumps(inputs[phase], ensure_ascii=False), encoding="utf-8")
        prepare_pid, prepared = run_captured_probe([str(binary), "prepare-posts", str(source)], environment, timeout=10)
        wire = require_success_output(prepared, "alpha prepared publication")
        data = prepared.stdout
        bundle = json.loads(wire)
        if set(bundle) != {"format_version", "pages"} or bundle["format_version"] != 1 or len(bundle["pages"]) != 1:
            raise ValueError("alpha export is not one prepared body page")
        page = bundle["pages"][0]
        if page["target"] != TARGET or page["status"] != 200 or page["content_type"] != "text/html; charset=utf-8":
            raise ValueError("alpha export admission differs")
        body = page["body"].encode("utf-8")
        for canary in (b"fixture-password", b"PRIVATE_ALPHA_CANARY", b"DO_NOT_PASS"):
            if canary in data:
                raise ValueError("unselected editing-only canary entered public artifact")
        payload.write_bytes(data)
        receipt_pid, sealed = run_captured_probe([str(binary), "bundle-receipt", str(payload)], environment, timeout=10)
        receipt_wire = require_success_output(sealed, "alpha bundle receipt")
        receipt_value = json.loads(receipt_wire)
        expected = {"format_version": 1, "payload_bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()}
        if receipt_value != expected:
            raise ValueError("alpha receipt differs from independent byte identity")
        receipt.write_bytes(sealed.stdout)
        paths[phase] = (payload, receipt)
        bodies[phase] = body
        records[phase] = {"prepare_pid": prepare_pid, "receipt_pid": receipt_pid,
            "payload_bytes": len(data), "payload_sha256": expected["sha256"],
            "body_bytes": len(body), "body_sha256": hashlib.sha256(body).hexdigest()}
    if bodies["a"] == bodies["b"]:
        raise ValueError("committed text change did not reach rendered output")

    def observe(server, phase, old_tag=None):
        status, headers, body = request(server.port, TARGET)
        expected = bodies[phase]
        if status != 200 or body != expected or headers.get("content-type") != "text/html; charset=utf-8":
            raise ValueError("alpha HTTP does not match the exact prepared generation")
        tag = headers["etag"]
        status, head, empty = request(server.port, TARGET, "HEAD")
        if status != 200 or empty or int(head["content-length"]) != len(expected) or head["etag"] != tag:
            raise ValueError("alpha HEAD representation differs")
        status, matched, empty = request(server.port, TARGET, extra_headers={"If-None-Match": tag})
        if status != 304 or empty or matched["etag"] != tag:
            raise ValueError("alpha conditional representation differs")
        if old_tag is not None:
            status, changed, body = request(server.port, TARGET, extra_headers={"If-None-Match": old_tag})
            if status != 200 or body != expected or changed["etag"] == old_tag:
                raise ValueError("old validator concealed committed generation change")
            if request(server.port, TARGET, extra_headers={"If-Match": old_tag})[0] != 412:
                raise ValueError("old strong validator was accepted after replacement")
        return tag

    first = start("public", "--host", "example.test", "--bundle", str(paths["a"][0]),
        "--receipt", str(paths["a"][1]), environment=environment, binary=binary)
    first_error = None
    try:
        old_tag = observe(first, "a")
        # A loaded process owns its exact bytes; later source mutation/deletion
        # neither reloads content nor starts DB/PHP/jobs from a public request.
        paths["a"][0].write_bytes(paths["b"][0].read_bytes())
        if observe(first, "a") != old_tag:
            raise ValueError("sealed generation changed after source mutation")
        rejected_pid, rejected = run_captured_probe([str(binary), "public", "--listen", "127.0.0.1:0",
            "--host", "example.test", "--bundle", str(paths["a"][0]), "--receipt", str(paths["a"][1])],
            environment, timeout=10)
        if (rejected.returncode == 0 or rejected.stdout or b"listening on" in rejected.stderr
                or b"PayloadDigestMismatch" not in rejected.stderr):
            raise ValueError("mismatched candidate reached a listener")
        if observe(first, "a") != old_tag:
            raise ValueError("refused candidate disturbed the existing process")
        paths["a"][0].unlink()
        if observe(first, "a") != old_tag:
            raise ValueError("sealed generation depended on its deleted input file")
        records["a"].update(public_pid=first.process.pid, etag=old_tag,
            get_head_conditional=True, frozen_after_source_mutation=True, frozen_after_source_deletion=True)
    except BaseException as error:
        first_error = error
        raise
    finally:
        stop(first, original=first_error)
    records["a"]["exit_code"] = first.process.returncode
    second = start("public", "--host", "example.test", "--bundle", str(paths["b"][0]),
        "--receipt", str(paths["b"][1]), environment=environment, binary=binary)
    second_error = None
    try:
        new_tag = observe(second, "b", old_tag)
        records["b"].update(public_pid=second.process.pid, etag=new_tag,
            get_head_conditional=True, old_validator_rejected=True)
    except BaseException as error:
        second_error = error
        raise
    finally:
        stop(second, original=second_error)
    records["b"]["exit_code"] = second.process.returncode
    if not database_alive():
        raise ValueError("owned editing DB exited during sealed publication experiment")
    evidence = {"format_version": 1, "scope": SCOPE, "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "backend": backend, "server_version": result["server_version"], "database_pid": database_pid,
        "contract_pid": contract_pid, "db_checks": sorted(result["checks"]), "phases": records,
        "rejected_candidate_pid": rejected_pid, "rejected_candidate_exit_code": rejected.returncode,
        "mismatched_candidate_rejected": True,
        "old_process_exited_before_replacement": first.process.poll() is not None,
        "public_processes_reaped": first.process.poll() is not None and second.process.poll() is not None,
        "database_stayed_running": True, "public_environment_scrubbed": True,
        "tracked_files": {path: reference.digest_file(root/path) for path in sorted(tracked_files())},
        "limitations": ["Owned finite fixture and explicitly selected committed text only, not WordPress public visibility or wp_update_post equivalence.",
            "The dedicated source row is restored/deleted before HTTP; A/B are retained committed snapshots, not current-source synchronized views.",
            "Fixed literal-text layout, not Wasm/theme execution. Authentication and management mutation routes remain unavailable.",
            "Separate sealed processes with an observed stop; no live/zero-downtime switch, revocation deadline, producer authorization or anti-rollback.",
            "Environment scrubbing is observed; OS-user/container/file-permission isolation is not established."]}
    validate_record(evidence)
    (root/f"compatibility/evidence/alpha-publication-{backend}.json").write_text(json.dumps(evidence, indent=2)+"\n", encoding="utf-8")
    print(f"Owned alpha post path passed: {backend}; committed A/B -> literal bundles -> receipts -> separate sealed HTTP")
