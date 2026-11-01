"""Build and observe the fixed isolated Wasmtime probe; no arbitrary guest input.

Cargo/tool/environment configuration is trusted. Capture sizes are checked after
the owned child exits; these checks are not a process RSS or allocator budget.
"""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import owned_probe

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / "experiments/wasm-runtime-probe/Cargo.toml"
MAIN = ROOT / "experiments/wasm-runtime-probe/src/main.rs"
TARGET_DIR = ROOT / "target/wasm-runtime-probe"
BIN = "recastpress-wasm-runtime-probe"
PREFIX = b"WASM_RUNTIME_PROBE_RESULT="
BUILD_TIMEOUT = 600
PROBE_TIMEOUT = 20
MAX_BUILD_JSON = 16 * 1024 * 1024
MAX_BUILD_DIAGNOSTICS = 1024 * 1024
MAX_RESULT = 4096
FAILURE = "Wasm runtime probe check failed"


class WasmRuntimeProbeGateError(RuntimeError):
    def __init__(self):
        super().__init__(FAILURE)


def _object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise WasmRuntimeProbeGateError()
        value[key] = item
    return value


def _constant(_):
    raise WasmRuntimeProbeGateError()


def _json(wire):
    return json.loads(wire.decode("utf-8", errors="strict"), object_pairs_hook=_object, parse_constant=_constant)


def select_artifact(wire):
    """One completed non-test binary from this fixed Cargo manifest and target."""
    if type(wire) is not bytes or not wire or len(wire) > MAX_BUILD_JSON or not wire.endswith(b"\n"):
        raise WasmRuntimeProbeGateError()
    candidates = []
    finished = False
    for line in wire.splitlines():
        message = _json(line)
        if type(message) is not dict or type(message.get("reason")) is not str or finished:
            raise WasmRuntimeProbeGateError()
        if message["reason"] == "build-finished":
            if message.get("success") is not True:
                raise WasmRuntimeProbeGateError()
            finished = True
            continue
        if message["reason"] != "compiler-artifact":
            continue
        target = message.get("target")
        if type(target) is not dict:
            raise WasmRuntimeProbeGateError()
        if target.get("name") != BIN or target.get("kind") != ["bin"]:
            continue
        if (type(message.get("manifest_path")) is not str or Path(message["manifest_path"]).resolve() != MANIFEST.resolve()
            or type(target.get("src_path")) is not str or Path(target["src_path"]).resolve() != MAIN.resolve()
            or type(message.get("profile")) is not dict or message["profile"].get("test") is not False
            or type(message.get("executable")) is not str):
            raise WasmRuntimeProbeGateError()
        executable = Path(message["executable"])
        if not executable.is_absolute() or not executable.is_file():
            raise WasmRuntimeProbeGateError()
        executable = executable.resolve(strict=True)
        if not executable.is_relative_to(TARGET_DIR.resolve()):
            raise WasmRuntimeProbeGateError()
        candidates.append(executable)
    if not finished or len(candidates) != 1:
        raise WasmRuntimeProbeGateError()
    return candidates[0]


def validate_result(pid, result):
    if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int or result.returncode != 0
        or type(result.stdout) is not bytes or type(result.stderr) is not bytes or result.stderr
        or len(result.stdout) > MAX_RESULT or not result.stdout.endswith(b"\n")
        or result.stdout.count(b"\n") != 1 or not result.stdout.startswith(PREFIX)):
        raise WasmRuntimeProbeGateError()
    value = _json(result.stdout[len(PREFIX):])
    expected = {"format_version": 1, "profile": "wasmtime_36_0_19_fixed_core_module_fixtures",
                "fixture_checks": 27, "literal_output_bytes": 19, "fuel_per_store": 10000, "unpublished_only": True}
    if type(value) is not dict or set(value) != set(expected) | {"process_pid"}:
        raise WasmRuntimeProbeGateError()
    if type(value["process_pid"]) is not int or value["process_pid"] != pid:
        raise WasmRuntimeProbeGateError()
    if any(type(value[key]) is not type(item) or value[key] != item for key, item in expected.items()):
        raise WasmRuntimeProbeGateError()
    return value


def _digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def run_gate():
    import os
    environment = os.environ.copy()
    command = ["cargo", "build", "--manifest-path", str(MANIFEST), "--package", BIN,
               "--bin", BIN, "--target-dir", str(TARGET_DIR), "--offline", "--locked", "--message-format=json"]
    original = None
    try:
        build_pid, built = owned_probe.run_captured_probe(command, environment, timeout=BUILD_TIMEOUT)
        if (type(build_pid) is not int or build_pid <= 0 or type(built.returncode) is not int or built.returncode != 0
            or type(built.stderr) is not bytes or len(built.stderr) > MAX_BUILD_DIAGNOSTICS):
            raise WasmRuntimeProbeGateError()
        artifact = select_artifact(built.stdout)
        pid, result = owned_probe.run_captured_probe([str(artifact)], environment, timeout=PROBE_TIMEOUT)
        native = validate_result(pid, result)
        observation = {"artifact": str(artifact), "artifact_sha256": _digest(artifact), "native": native}
    except BaseException as error:
        original = error
        raise
    finally:
        try:
            owned_probe.reap_owned_probes()
        except BaseException as cleanup:
            if original is not None:
                original.add_note("Wasm probe cleanup retry failed: " + type(cleanup).__name__)
            else:
                raise WasmRuntimeProbeGateError() from None
    return observation


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else list(argv)
    if arguments in (["--help"], ["-h"]):
        print("Usage: python scripts/check_wasm_runtime_probe.py")
        return 0
    if arguments:
        print(FAILURE, file=sys.stderr)
        return 2
    try:
        value = run_gate()
    except (Exception, KeyboardInterrupt):
        print(FAILURE, file=sys.stderr)
        return 1
    print("Cargo-selected artifact: " + value["artifact"])
    print("Artifact SHA256: " + value["artifact_sha256"])
    print(PREFIX.decode() + json.dumps(value["native"], sort_keys=True, separators=(",", ":")))
    print("Scope: aggregate native assertion summary for fixed public WAT fixtures.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
