"""Owned finite Component preparation -> sealed HTTP -> capture comparison demo.

Only fixed public fixtures reach this smoke. Wasm runs in the producer before
the public process starts; HTTP serves an admitted frozen bundle. The normal
preparation API retains completed/partial packages on later failure. This smoke
alone removes its own temporary fixtures, after all captured children/readers
are reaped. It neither operates on existing servers nor publishes to a site.
"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import queue
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import threading

import http_baseline
import http_baseline_compare
import owned_probe
import smoke

ROOT = Path(__file__).resolve().parents[1]
OBSERVATION = ROOT / "target/wasm-publication-example-http.json"
FAILURE = "Wasm publication HTTP smoke failed"
MIME = "text/html; charset=utf-8"
COMPARISONS = ("body_equal", "status_equal", "headers_equal", "http_version_equal",
               "request_target_equal", "url_equal", "transport_equal", "tls_verification_equal")


class PublicationSmokeError(RuntimeError):
    def __init__(self):
        super().__init__(FAILURE)


def require(condition):
    if not condition:
        raise PublicationSmokeError()


def _unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result)
        result[key] = value
    return result


def _json(wire):
    return json.loads(wire.decode("utf-8", errors="strict"), object_pairs_hook=_unique,
                      parse_constant=_constant)


def _constant(_):
    raise PublicationSmokeError()


def _read(path, limit):
    require(path.is_file() and not path.is_symlink())
    with path.open("rb") as stream:
        wire = stream.read(limit + 1)
    require(len(wire) <= limit)
    return wire


def _sha(path):
    digest = hashlib.sha256()
    total = 0
    with path.open("rb") as stream:
        while block := stream.read(1024 * 1024):
            total += len(block)
            require(total <= 256 * 1024 * 1024)
            digest.update(block)
    return digest.hexdigest()


def _environment():
    # Test boundary, not OS-user isolation or a general secret scrubber.
    return {key: value for key, value in os.environ.items()
            if not key.upper().startswith(("RECASTPRESS_DB_", "MYSQL_", "MARIADB_", "RECASTPRESS_ALPHA_"))
            and key.upper() not in {"DATABASE_URL", "SSLKEYLOGFILE"}}


def _command(binary, arguments, environment):
    pid, result = owned_probe.run_captured_probe([str(binary), *arguments], environment, timeout=20)
    require(type(pid) is int and pid > 0 and type(result.returncode) is int
            and result.returncode == 0 and type(result.stdout) is bytes
            and type(result.stderr) is bytes and not result.stderr and len(result.stdout) <= 4096)
    return pid, result.stdout


def _start(binary, bundle, receipt, environment):
    # This handoff is deliberately not atomic. A competing bind is a failure;
    # it never permits connecting to that competing process as our fixture.
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as reservation:
        reservation.bind(("127.0.0.1", 0))
        port = reservation.getsockname()[1]
    authority = f"127.0.0.1:{port}"
    owner = smoke._remember(subprocess.Popen(
        [str(binary), "public", "--listen", authority, "--host", authority,
         "--bundle", str(bundle), "--receipt", str(receipt)], cwd=ROOT,
        stdout=subprocess.DEVNULL, stderr=subprocess.PIPE, text=True, encoding="utf-8",
        env=environment, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0))
    try:
        lines = queue.Queue()
        reader = threading.Thread(target=smoke._read_startup, args=(owner, lines),
                                  name=f"wasm-publication-server-{owner.process.pid}", daemon=True)
        owner.reader = reader
        reader.start()
        line = lines.get(timeout=smoke.STARTUP_TIMEOUT)
        require(not owner.reader_errors and type(line) is str
                and re.search(r"listening on 127\.0\.0\.1:" + str(port) + r"(?:\s|$)", line) is not None
                and owner.process.poll() is None)
        return smoke.Server(owner.process, port, lines, reader), authority
    except BaseException as error:
        smoke._cleanup_result(smoke._stop_owned(owner), error)
        raise


def _http(server, target, authority, expected):
    require(server.process.poll() is None)
    status, headers, body = smoke.request(server.port, target, host=authority)
    tag = headers.get("etag")
    require(status == 200 and body == expected and headers.get("content-type") == MIME
            and headers.get("content-length") == str(len(expected)) and type(tag) is str
            and re.fullmatch(r'"[0-9a-f]{64}"', tag) is not None)
    status, head, body = smoke.request(server.port, target, "HEAD", host=authority)
    require(status == 200 and body == b"" and head.get("etag") == tag
            and head.get("content-type") == MIME and head.get("content-length") == str(len(expected)))
    status, conditional, body = smoke.request(server.port, target, host=authority,
                                               extra_headers={"If-None-Match": tag})
    require(status == 304 and body == b"" and conditional.get("etag") == tag)
    require(smoke.request(server.port, target, host="rejected.invalid")[0] == 421)
    require(server.process.poll() is None)
    return tag


def _exercise(work):
    import wasm_publication_example as example
    environment = _environment()
    package = work / "package"
    produced = example.prepare_example(package)
    binary = example.build_public_server()
    producer_sha = _sha(produced["artifact"])
    require(producer_sha == produced["artifact_sha256"])
    server_sha = _sha(binary)
    bundle, receipt = package / "bundle.json", package / "receipt.json"
    payload = _read(bundle, 8 * 1024 * 1024)
    receipt_wire = _read(receipt, 1024)
    value = _json(payload)
    expected = example.EXPECTED_HTML
    target = example.TARGET
    require(type(expected) is bytes and type(target) is str and target.startswith("/"))
    require(value == {"format_version": 1, "pages": [{"target": target, "status": 200, "content_type": MIME,
                                                       "body": expected.decode("utf-8")} ]})
    check_pid, checked = _command(binary, ["check-bundle", str(bundle)], environment)
    report = _json(checked)
    mimes = {name: 0 for name in (MIME, "text/plain; charset=utf-8", "application/rss+xml; charset=utf-8",
             "application/atom+xml; charset=utf-8", "image/png", "image/jpeg", "image/webp")}
    mimes[MIME] = 1
    require(report == {"format_version": 1, "accepted_bundle_version": 1, "pages": 1,
                       "redirects": 0, "body_bytes": len(expected), "mime_counts": mimes})
    receipt_pid, regenerated = _command(binary, ["bundle-receipt", str(bundle)], environment)
    require(regenerated == receipt_wire)
    require(_json(receipt_wire) == {"format_version": 1, "payload_bytes": len(payload),
                                   "sha256": hashlib.sha256(payload).hexdigest()})
    server, authority = _start(binary, bundle, receipt, environment)
    original = None
    try:
        tag = _http(server, target, authority, expected)
        left, right = work / "capture-a", work / "capture-b"
        url = "http://" + authority + target
        for capture in (left, right):
            require(server.process.poll() is None)
            http_baseline.capture_http_baseline(url, capture)
            require(_read(capture / "body.bin", 8 * 1024 * 1024) == expected)
        comparison = http_baseline_compare.compare_http_baselines(left, right)
        require(all(comparison.get(key) is True for key in COMPARISONS)
                and comparison.get("changed_header_names") == [] and server.process.poll() is None)
        require(_sha(binary) == server_sha and _sha(produced["artifact"]) == producer_sha)
        result = {"format_version": 1, "scope": "fixed_component_preparation_sealed_http_smoke",
                  "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
                  "producer_artifact_sha256": producer_sha, "server_artifact_sha256": server_sha,
                  "producer_summary": produced["summary"], "check_bundle_pid": check_pid,
                  "bundle_receipt_pid": receipt_pid, "public_pid": server.process.pid,
                  "body_bytes": len(expected), "body_sha256": hashlib.sha256(expected).hexdigest(),
                  "etag": tag, "get_head_conditional_host_rejection": True,
                  "capture_count": 2, "comparison": comparison,
                  "wasm_runs_before_public_http_only": True}
    except BaseException as error:
        original = error
        raise
    finally:
        smoke.stop(server, original=original)
    require(server.process.poll() is not None)
    result["public_exit_code"] = server.process.returncode
    return result


def run():
    # No automatic finalizer: unconfirmed cleanup must retain this directory.
    target_root = (ROOT / "target").resolve()
    require(target_root.is_relative_to(ROOT.resolve()) and target_root.is_dir())
    work = Path(tempfile.mkdtemp(prefix="wasm-publication-smoke-", dir=target_root)).resolve()
    require(work.parent == target_root and work.name.startswith("wasm-publication-smoke-"))
    original = None
    cleanup_errors = []
    try:
        result = _exercise(work)
    except BaseException as error:
        original = error
        raise
    finally:
        for cleanup in (smoke.reap_servers, owned_probe.reap_owned_probes):
            try:
                cleanup()
            except BaseException as error:
                cleanup_errors.append(error)
        if not cleanup_errors:
            require(work.parent == target_root and work.is_relative_to(ROOT.resolve())
                    and work.name.startswith("wasm-publication-smoke-"))
            shutil.rmtree(work)
        else:
            # Retain our fixtures while any process/reader may own them.
            if original is not None:
                original.add_note("Owned publication cleanup remains unconfirmed")
            else:
                raise PublicationSmokeError() from None
    result["owned_handles_reaped"] = True
    result["owned_temporary_directory_removed"] = not work.exists()
    require(result["owned_temporary_directory_removed"])
    OBSERVATION.write_text(json.dumps(result, ensure_ascii=True, indent=2) + "\n", encoding="utf-8", newline="\n")
    return result


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else list(argv)
    if arguments == ["--help"]:
        print("Usage: python scripts/smoke_wasm_publication_example.py")
        return 0
    if arguments:
        print(FAILURE, file=sys.stderr)
        return 2
    try:
        result = run()
    except (Exception, KeyboardInterrupt):
        print(FAILURE, file=sys.stderr)
        return 1
    print(json.dumps(result, ensure_ascii=True, sort_keys=True, separators=(",", ":")))
    print("Fixed Component preparation -> sealed HTTP -> two captures/comparison passed; owned fixtures reaped.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
