"""Build and observe the fixed isolated Wasmtime request-time Component HTTP probe; no arbitrary guest input.

Cargo/tool/environment configuration is trusted. Capture sizes are checked after
the owned child exits; these checks are not a process RSS or allocator budget.
"""
from dataclasses import dataclass
import hashlib
import http.client
import os
import queue
import re
import shutil
import subprocess
import tempfile
import threading
import json
from pathlib import Path
import sys

import owned_probe
import smoke
from http_baseline_compare import _read_regular

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "experiments/wasm-request-probe/Cargo.toml"
MAIN = ROOT / "experiments/wasm-request-probe/src/main.rs"
TARGET_DIR = ROOT / "target/wasm-request-probe"
BIN = "recastpress-wasm-request-probe"
READY_PREFIX = b"WASM_REQUEST_READY="
CALL_PREFIX = b"WASM_REQUEST_CALL="
BUILD_TIMEOUT = 600
STARTUP_TIMEOUT = 20
MAX_BUILD_JSON = 16 * 1024 * 1024
MAX_BUILD_DIAGNOSTICS = 1024 * 1024
MAX_RESULT = 4096
FAILURE = "Wasm request probe check failed"


class WasmRequestProbeGateError(RuntimeError):
    def __init__(self):
        super().__init__(FAILURE)


def _object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise WasmRequestProbeGateError()
        value[key] = item
    return value


def _constant(_):
    raise WasmRequestProbeGateError()


def _json(wire):
    return json.loads(wire.decode("utf-8", errors="strict"), object_pairs_hook=_object, parse_constant=_constant)


def select_artifact(wire):
    """One completed non-test binary from this fixed Cargo manifest and target."""
    if type(wire) is not bytes or not wire or len(wire) > MAX_BUILD_JSON or not wire.endswith(b"\n"):
        raise WasmRequestProbeGateError()
    candidates = []
    finished = False
    for line in wire.splitlines():
        message = _json(line)
        if type(message) is not dict or type(message.get("reason")) is not str or finished:
            raise WasmRequestProbeGateError()
        if message["reason"] == "build-finished":
            if message.get("success") is not True:
                raise WasmRequestProbeGateError()
            finished = True
            continue
        if message["reason"] != "compiler-artifact":
            continue
        target = message.get("target")
        if type(target) is not dict:
            raise WasmRequestProbeGateError()
        if target.get("name") != BIN or target.get("kind") != ["bin"]:
            continue
        if (type(message.get("manifest_path")) is not str or Path(message["manifest_path"]).resolve() != MANIFEST.resolve()
            or type(target.get("src_path")) is not str or Path(target["src_path"]).resolve() != MAIN.resolve()
            or type(message.get("profile")) is not dict or message["profile"].get("test") is not False
            or type(message.get("executable")) is not str):
            raise WasmRequestProbeGateError()
        executable = Path(message["executable"])
        if not executable.is_absolute() or not executable.is_file():
            raise WasmRequestProbeGateError()
        executable = executable.resolve(strict=True)
        if not executable.is_relative_to(TARGET_DIR.resolve()):
            raise WasmRequestProbeGateError()
        candidates.append(executable)
    if not finished or len(candidates) != 1:
        raise WasmRequestProbeGateError()
    return candidates[0]


HTML_A = b"<!doctype html><html><body><h1>Fixed A</h1></body></html>\n"
HTML_B = b"<!doctype html><html><body><h1>Fixed B</h1><p>Fresh Store.</p></body></html>\n"
_HTML = {"a": HTML_A, "b": HTML_B}
_PROFILE = "wasmtime_36_0_19_fixed_request_time_component"


def _record(line, prefix):
    if (type(line) is not str or len(line.encode("utf-8")) > MAX_RESULT or not line.endswith("\n")
        or line.count("\n") != 1 or not line.startswith(prefix.decode())):
        raise WasmRequestProbeGateError()
    return _json(line[len(prefix):].encode("utf-8"))


def validate_ready(line, pid):
    value = _record(line, READY_PREFIX)
    expected = {"format_version": 1, "profile": _PROFILE, "compilations": 1, "warmups": 1}
    if type(value) is not dict or set(value) != set(expected) | {"process_pid", "authority"}:
        raise WasmRequestProbeGateError()
    if (type(pid) is not int or pid <= 0 or type(value["process_pid"]) is not int or value["process_pid"] != pid
        or any(type(value[key]) is not type(item) or value[key] != item for key, item in expected.items())
        or type(value["authority"]) is not str):
        raise WasmRequestProbeGateError()
    match = re.fullmatch(r"127\.0\.0\.1:([1-9][0-9]{0,4})", value["authority"])
    if match is None or not 1 <= int(match[1]) <= 65535:
        raise WasmRequestProbeGateError()
    return value


def validate_calls(lines):
    expected = ("a", "b", "a", "a")
    if len(lines) != len(expected):
        raise WasmRequestProbeGateError()
    result = []
    for ordinal, (line, fixture) in enumerate(zip(lines, expected), 1):
        value = _record(line, CALL_PREFIX)
        fields = {"format_version": 1, "ordinal": ordinal, "fixture": fixture, "store_counter": 1,
                  "input_bytes": len(_HTML[fixture]), "output_bytes": len(_HTML[fixture])}
        if type(value) is not dict or set(value) != set(fields) or any(type(value[key]) is not type(item) or value[key] != item for key, item in fields.items()):
            raise WasmRequestProbeGateError()
        result.append(value)
    return result


@dataclass
class _Running:
    owner: object
    lines: queue.Queue
    ready: dict
    stdout_dir: Path


def _confirmed(owner):
    return owner.process.poll() is not None and (owner.reader is None or not owner.reader.is_alive())


def _remove_stdout(directory):
    target = (ROOT / "target").resolve()
    resolved = directory.resolve(strict=True)
    if (not target.is_relative_to(ROOT.resolve()) or resolved.parent != target
        or not resolved.name.startswith("wasm-request-stdout-")):
        raise WasmRequestProbeGateError()
    shutil.rmtree(resolved)


def _start(artifact):
    stdout_dir = Path(tempfile.mkdtemp(prefix="wasm-request-stdout-", dir=ROOT / "target")).resolve()
    process = None
    owner = None
    try:
        # Parent closes its file after spawn; the child's inherited handle is
        # independent. Only stderr needs a reader, managed by the old registry.
        with (stdout_dir / "stdout.bin").open("xb") as sink:
            process = subprocess.Popen([str(artifact)], cwd=ROOT, env=os.environ.copy(), stdout=sink,
                stderr=subprocess.PIPE, stdin=subprocess.DEVNULL, text=True, encoding="utf-8",
                creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            owner = smoke._remember(process)
        lines = queue.Queue()
        reader = threading.Thread(target=smoke._read_startup, args=(owner, lines),
            name=f"wasm-request-stderr-{process.pid}", daemon=True)
        owner.reader = reader
        reader.start()
        line = lines.get(timeout=STARTUP_TIMEOUT)
        if owner.reader_errors:
            raise WasmRequestProbeGateError()
        ready = validate_ready(line, process.pid)
        if process.poll() is not None:
            raise WasmRequestProbeGateError()
        return _Running(owner, lines, ready, stdout_dir)
    except BaseException as error:
        if owner is not None:
            smoke.stop_process(process, owner.reader, original=error)
            if _confirmed(owner):
                _remove_stdout(stdout_dir)
        elif process is None:
            _remove_stdout(stdout_dir)
        # An unconfirmed captured child keeps the directory and registry entry.
        raise


def _request(authority, method, target, host=None):
    port = int(authority.rsplit(":", 1)[1])
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        connection.request(method, target, headers={"Host": host or authority, "Connection": "close"})
        response = connection.getresponse()
        body = response.read(4097)
        if len(body) > 4096:
            raise WasmRequestProbeGateError()
        return response.status, {key.lower(): value for key, value in response.getheaders()}, body
    finally:
        connection.close()


def _observe(artifact):
    running = _start(artifact)
    original = None
    observations = []
    try:
        authority = running.ready["authority"]
        # Refusals are interleaved, then all diagnostics are drained after stop.
        # Queue emptiness during a live response is not evidence of CALL absence.
        plan = [("GET", "/a", None, 200, "a"), ("GET", "/a?x=1", None, 404, None),
                ("GET", "/b", None, 200, "b"), ("GET", "/a", "wrong.example", 421, None),
                ("GET", "/unknown", None, 404, None), ("POST", "/a", None, 405, None),
                ("GET", f"http://{authority}/a", None, 400, None),
                ("GET", "/a", None, 200, "a"), ("HEAD", "/a", None, 200, "a")]
        for method, target, host, expected_status, fixture in plan:
            status, headers, body = _request(authority, method, target, host)
            if status != expected_status:
                raise WasmRequestProbeGateError()
            if fixture is not None:
                expected_body = b"" if method == "HEAD" else _HTML[fixture]
                if (body != expected_body or headers.get("content-length") != str(len(_HTML[fixture]))
                    or headers.get("content-type") != "text/html; charset=utf-8"
                    or headers.get("cache-control") != "no-store" or headers.get("x-content-type-options") != "nosniff"):
                    raise WasmRequestProbeGateError()
            elif status == 405 and headers.get("allow") != "GET, HEAD":
                raise WasmRequestProbeGateError()
            observations.append({"method": method, "target": target, "status": status, "body_bytes": len(body)})
        if running.owner.process.poll() is not None:
            raise WasmRequestProbeGateError()
    except BaseException as error:
        original = error
        raise
    finally:
        smoke.stop_process(running.owner.process, running.owner.reader, original=original)
        if _confirmed(running.owner):
            try:
                if original is None:
                    if _read_regular(running.stdout_dir / "stdout.bin", 0) != b"":
                        raise WasmRequestProbeGateError()
            finally:
                _remove_stdout(running.stdout_dir)
        elif original is None:
            raise WasmRequestProbeGateError()
    tail = []
    while not running.lines.empty():
        tail.append(running.lines.get_nowait())
    if not tail or tail[-1] is not None or tail.count(None) != 1 or running.owner.reader_errors:
        raise WasmRequestProbeGateError()
    calls = validate_calls(tail[:-1])
    return {"ready": running.ready, "calls": calls, "http_observations": observations,
            "stdout_empty": True, "owned_process_reaped": True, "stderr_reader_stopped": True,
            "exit_code": running.owner.process.returncode, "stdout_directory_removed": True}


def _digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_gate():
    environment = os.environ.copy()
    command = ["cargo", "build", "--manifest-path", str(MANIFEST), "--package", BIN, "--bin", BIN,
               "--target-dir", str(TARGET_DIR), "--offline", "--locked", "--message-format=json"]
    original = None
    try:
        pid, built = owned_probe.run_captured_probe(command, environment, timeout=BUILD_TIMEOUT)
        if (type(pid) is not int or pid <= 0 or type(built.returncode) is not int or built.returncode != 0
            or type(built.stderr) is not bytes or len(built.stderr) > MAX_BUILD_DIAGNOSTICS):
            raise WasmRequestProbeGateError()
        artifact = select_artifact(built.stdout)
        observed = _observe(artifact)
        return {"format_version": 1, "scope": "isolated_fixed_request_time_component_http_probe",
                "artifact": str(artifact), "artifact_sha256": _digest(artifact), **observed}
    except BaseException as error:
        original = error
        raise
    finally:
        failures = []
        for cleanup in (lambda: smoke.reap_servers(original=original), owned_probe.reap_owned_probes):
            try:
                cleanup()
            except BaseException as error:
                failures.append(type(error).__name__)
        if failures:
            if original is not None:
                original.add_note("Request probe cleanup retry failed: " + ",".join(failures))
            else:
                raise WasmRequestProbeGateError()


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else list(argv)
    if arguments in (["--help"], ["-h"]):
        print("Usage: python scripts/check_wasm_request_probe.py")
        return 0
    if arguments:
        print(FAILURE, file=sys.stderr)
        return 2
    try:
        observation = run_gate()
    except (Exception, KeyboardInterrupt):
        print(FAILURE, file=sys.stderr)
        return 1
    print(json.dumps(observation, ensure_ascii=True, sort_keys=True, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())


