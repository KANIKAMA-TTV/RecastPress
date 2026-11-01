"""Fixed Component producer and sealed-server build profiles for an offline example.

Only a new output directory is caller-selected. Native admission is performed
by existing Rust APIs; Python verifies bounded saved-file identity, not a new
general bundle parser. Cargo, stable parent paths and host tools are trusted.
"""
from dataclasses import dataclass
from enum import Enum
import hashlib
import json
import os
from pathlib import Path
import re
import stat

import owned_probe
from http_baseline import _new_directory
from http_baseline_compare import _linked, _read_regular

ROOT = Path(__file__).resolve().parents[1]
TARGET = "/wasm-fixture"
EXPECTED_HTML = '<!doctype html><html lang="ja"><meta charset="utf-8"><title>Wasm fixture</title><body><h1>固定 Component</h1><p>Prepared before GET.</p></body></html>\n'.encode("utf-8")
PREFIX = b"WASM_PUBLICATION_PROBE_RESULT="
MAX_BUNDLE = 8 * 1024 * 1024
MAX_RECEIPT = 1024
MAX_RESULT = 4096
MAX_BUILD_JSON = 16 * 1024 * 1024
MAX_BUILD_DIAGNOSTICS = 1024 * 1024
FAILURE = "Wasm publication example failed"


class WasmPublicationExampleError(Exception):
    def __init__(self):
        super().__init__(FAILURE)

    def __repr__(self):
        return "WasmPublicationExampleError()"


class _Role(Enum):
    PRODUCER = "producer"
    SERVER = "server"


@dataclass(frozen=True)
class _BuildProfile:
    build_manifest: Path
    artifact_manifest: Path
    main: Path
    package: str
    binary: str
    target: Path


_PROFILES = {
    _Role.PRODUCER: _BuildProfile(ROOT / "experiments/wasm-publication-probe/Cargo.toml",
        ROOT / "experiments/wasm-publication-probe/Cargo.toml", ROOT / "experiments/wasm-publication-probe/src/main.rs",
        "recastpress-wasm-publication-probe", "recastpress-wasm-publication-probe", ROOT / "target/wasm-publication-probe"),
    _Role.SERVER: _BuildProfile(ROOT / "Cargo.toml", ROOT / "crates/recastpress-server/Cargo.toml",
        ROOT / "crates/recastpress-server/src/main.rs", "recastpress-server", "recastpress",
        ROOT / "target/wasm-publication-probe/public-server"),
}


def _object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise WasmPublicationExampleError()
        value[key] = item
    return value


def _constant(_):
    raise WasmPublicationExampleError()


def _json(wire):
    return json.loads(wire.decode("utf-8"), object_pairs_hook=_object, parse_constant=_constant)


def _select_artifact(wire, role):
    profile = _PROFILES[role]
    if type(wire) is not bytes or not wire or len(wire) > MAX_BUILD_JSON or not wire.endswith(b"\n"):
        raise WasmPublicationExampleError()
    candidates = []
    finished = False
    for line in wire.splitlines():
        message = _json(line)
        if type(message) is not dict or type(message.get("reason")) is not str or finished:
            raise WasmPublicationExampleError()
        if message["reason"] == "build-finished":
            if message.get("success") is not True:
                raise WasmPublicationExampleError()
            finished = True
            continue
        if message["reason"] != "compiler-artifact":
            continue
        target = message.get("target")
        if type(target) is not dict:
            raise WasmPublicationExampleError()
        if target.get("name") != profile.binary or target.get("kind") != ["bin"]:
            continue
        if (type(message.get("manifest_path")) is not str or Path(message["manifest_path"]).resolve() != profile.artifact_manifest.resolve()
            or type(target.get("src_path")) is not str or Path(target["src_path"]).resolve() != profile.main.resolve()
            or type(message.get("profile")) is not dict or message["profile"].get("test") is not False
            or type(message.get("executable")) is not str):
            raise WasmPublicationExampleError()
        executable = Path(message["executable"])
        if not executable.is_absolute() or not executable.is_file():
            raise WasmPublicationExampleError()
        executable = executable.resolve(strict=True)
        if not executable.is_relative_to(profile.target.resolve()):
            raise WasmPublicationExampleError()
        candidates.append(executable)
    if not finished or len(candidates) != 1:
        raise WasmPublicationExampleError()
    return candidates[0]


def _build(role):
    profile = _PROFILES[role]
    command = ["cargo", "build", "--manifest-path", str(profile.build_manifest), "--package", profile.package,
               "--bin", profile.binary, "--target-dir", str(profile.target), "--offline", "--locked", "--message-format=json"]
    pid, result = owned_probe.run_captured_probe(command, os.environ.copy(), timeout=600)
    if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int or result.returncode != 0
        or type(result.stderr) is not bytes or len(result.stderr) > MAX_BUILD_DIAGNOSTICS):
        raise WasmPublicationExampleError()
    return _select_artifact(result.stdout, role)


def _validate_native(pid, result):
    if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int or result.returncode != 0
        or type(result.stdout) is not bytes or type(result.stderr) is not bytes or result.stderr
        or len(result.stdout) > MAX_RESULT or not result.stdout.startswith(PREFIX)
        or not result.stdout.endswith(b"\n") or result.stdout.count(b"\n") != 1):
        raise WasmPublicationExampleError()
    value = _json(result.stdout[len(PREFIX):])
    expected = {"format_version": 1, "profile": "wasmtime_36_0_19_fixed_component_prepared_bundle",
                "body_bytes": len(EXPECTED_HTML), "post_return_completed": True,
                "guest_memory_changed_after_copy": True, "unpublished_only": True}
    if type(value) is not dict or set(value) != set(expected) | {"process_pid", "bundle_bytes", "receipt_bytes"}:
        raise WasmPublicationExampleError()
    if (type(value["process_pid"]) is not int or value["process_pid"] != pid
        or type(value["bundle_bytes"]) is not int or not len(EXPECTED_HTML) <= value["bundle_bytes"] <= MAX_BUNDLE
        or type(value["receipt_bytes"]) is not int or not 1 <= value["receipt_bytes"] <= MAX_RECEIPT
        or any(type(value[key]) is not type(item) or value[key] != item for key, item in expected.items())):
        raise WasmPublicationExampleError()
    return value


def _check_package(directory, summary):
    info = directory.lstat()
    if _linked(info) or not stat.S_ISDIR(info.st_mode):
        raise WasmPublicationExampleError()
    names = []
    with os.scandir(directory) as entries:
        for entry in entries:
            names.append(entry.name)
            if len(names) == 3:
                break
    if len(names) != 2 or set(names) != {"bundle.json", "receipt.json"}:
        raise WasmPublicationExampleError()
    payload = _read_regular(directory / "bundle.json", MAX_BUNDLE)
    receipt_wire = _read_regular(directory / "receipt.json", MAX_RECEIPT)
    if len(payload) != summary["bundle_bytes"] or len(receipt_wire) != summary["receipt_bytes"]:
        raise WasmPublicationExampleError()
    receipt = _json(receipt_wire)
    if (type(receipt) is not dict or set(receipt) != {"format_version", "payload_bytes", "sha256"}
        or type(receipt["format_version"]) is not int or receipt["format_version"] != 1
        or type(receipt["payload_bytes"]) is not int or receipt["payload_bytes"] != len(payload)
        or type(receipt["sha256"]) is not str or re.fullmatch(r"[0-9a-f]{64}", receipt["sha256"]) is None
        or receipt["sha256"] != hashlib.sha256(payload).hexdigest()):
        raise WasmPublicationExampleError()


def _digest(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _cleanup(original):
    try:
        owned_probe.reap_owned_probes()
    except BaseException as error:
        if original is not None:
            original.add_note("Publication example cleanup retry failed: " + type(error).__name__)
        else:
            raise WasmPublicationExampleError() from None


def prepare_example(new_dir):
    original = None
    try:
        directory = _new_directory(new_dir)
        artifact = _build(_Role.PRODUCER)
        pid, result = owned_probe.run_captured_probe([str(artifact), str(directory)], os.environ.copy(), timeout=20)
        summary = _validate_native(pid, result)
        _check_package(directory, summary)
        return {"artifact": artifact, "artifact_sha256": _digest(artifact), "summary": summary}
    except BaseException:
        original = WasmPublicationExampleError()
        raise original from None
    finally:
        _cleanup(original)


def build_public_server():
    original = None
    try:
        return _build(_Role.SERVER)
    except BaseException:
        original = WasmPublicationExampleError()
        raise original from None
    finally:
        _cleanup(original)
