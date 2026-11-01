"""Inventory only caller-selected local files, without executing or copying code.

Private paths and unconfirmed notes stay in the local manifest. No recursive
discovery, dependency inference, PHP, network, AI or license validation occurs.
This unsigned manual observation is not a complete migration diagnosis. Stable
trusted parents/host are required; visible checks are not an atomic snapshot,
an ABA-proof identity or a hostile filesystem sandbox. Failed writes retain
partial output, with no overwrite/delete, atomic publication or parent fsync.
"""

from datetime import datetime, timezone
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import sys
import unicodedata

import owned_probe
from http_baseline import _new_directory, _write_file
from http_baseline_compare import _identity, _linked, _read_regular

MAX_FILE = 8 * 1024 * 1024
MAX_TOTAL = 32 * 1024 * 1024
MAX_JSON = 64 * 1024
MAX_FILES = 256
MAX_PATH = 1024
MAX_UNCONFIRMED = 64
_FAILURE = "Migration source inventory failed"


class MigrationSourceInventoryError(Exception):
    """Closed error without paths, notes, file contents or IO diagnostics."""

    def __init__(self):
        super().__init__(_FAILURE)

    def __repr__(self):
        return "MigrationSourceInventoryError()"


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise MigrationSourceInventoryError()
        result[key] = value
    return result


def _not_number(_):
    raise MigrationSourceInventoryError()


def _json(raw):
    return json.loads(raw.decode("utf-8"), object_pairs_hook=_object,
        parse_float=_not_number, parse_constant=_not_number)


def _wire(value):
    return (json.dumps(value, ensure_ascii=True, sort_keys=True,
        separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")


def _relative(value):
    if type(value) is not str or not value or len(value.encode("utf-8")) > MAX_PATH:
        raise MigrationSourceInventoryError()
    if any(char in "\\:" or unicodedata.category(char) == "Cc" for char in value):
        raise MigrationSourceInventoryError()
    parts = value.split("/")
    if any(part in ("", ".", "..") for part in parts):
        raise MigrationSourceInventoryError()
    if os.name == "nt" and any(part.endswith((".", " ")) for part in parts):
        # Win32's trailing-dot/space aliases include '.. ' traversal forms.
        raise MigrationSourceInventoryError()
    return parts


def _selection(raw):
    value = _json(raw)
    if type(value) is not dict or set(value) != {"format_version", "files", "unconfirmed"}:
        raise MigrationSourceInventoryError()
    if (type(value["format_version"]) is not int or value["format_version"] != 1
        or type(value["files"]) is not list or not 1 <= len(value["files"]) <= MAX_FILES
        or type(value["unconfirmed"]) is not list or len(value["unconfirmed"]) > MAX_UNCONFIRMED):
        raise MigrationSourceInventoryError()
    selected = set()
    for path in value["files"]:
        _relative(path)
        normalized = os.path.normcase(path)
        if normalized in selected:
            raise MigrationSourceInventoryError()
        selected.add(normalized)
    for note in value["unconfirmed"]:
        if type(note) is not str or "\0" in note or len(note.encode("utf-8")) > MAX_PATH:
            raise MigrationSourceInventoryError()
    return value


def _directory(path):
    info = path.lstat()
    if _linked(info) or not stat.S_ISDIR(info.st_mode):
        raise MigrationSourceInventoryError()
    return info


def _unchanged_directory(path, before):
    after = _directory(path)
    if _identity(after) != _identity(before):
        raise MigrationSourceInventoryError()


def _selected_file(root, relative):
    parts = _relative(relative)
    path = root
    directories = [(root, _directory(root))]
    for part in parts[:-1]:
        path = path / part
        directories.append((path, _directory(path)))
    leaf = path / parts[-1]
    info = leaf.lstat()
    if _linked(info) or not stat.S_ISREG(info.st_mode):
        raise MigrationSourceInventoryError()
    # Defense against platform lexical aliases. Parents are still trusted;
    # resolve/lstat/open are not an adversarial-race-safe descriptor walk.
    if not leaf.resolve(strict=True).is_relative_to(root.resolve(strict=True)):
        raise MigrationSourceInventoryError()
    body = _read_regular(leaf, MAX_FILE)
    for directory, before in reversed(directories):
        _unchanged_directory(directory, before)
    return body


def _inventory_worker(source_root, selection_file, new_dir):
    # Reject an existing output or absent parent before any source reads. No
    # output directory is made until every selected observation/JSON cap fits.
    output = _new_directory(new_dir)
    root_before = _directory(source_root)
    selection = _selection(_read_regular(selection_file, MAX_JSON))
    files = []
    total = 0
    for relative in selection["files"]:
        body = _selected_file(source_root, relative)
        total += len(body)
        if total > MAX_TOTAL:
            raise MigrationSourceInventoryError()
        files.append({"path": relative, "bytes": len(body), "sha256": hashlib.sha256(body).hexdigest()})
    _unchanged_directory(source_root, root_before)
    manifest = {"format_version": 1,
        "scope": "manual_selected_migration_source_inventory",
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "files": files,
        "unconfirmed": selection["unconfirmed"]}
    encoded = _wire(manifest)
    if len(encoded) > MAX_JSON:
        raise MigrationSourceInventoryError()
    output.mkdir()
    _write_file(output / "inventory.json", encoded)
    return {"process_pid": os.getpid(), "summary": {
        "format_version": 1, "file_count": len(files), "total_bytes": total,
        "manifest_sha256": hashlib.sha256(encoded).hexdigest()}}


def _validate_summary(summary):
    if type(summary) is not dict or set(summary) != {"format_version", "file_count", "total_bytes", "manifest_sha256"}:
        raise MigrationSourceInventoryError()
    if (type(summary["format_version"]) is not int or summary["format_version"] != 1
        or type(summary["file_count"]) is not int or not 1 <= summary["file_count"] <= MAX_FILES
        or type(summary["total_bytes"]) is not int or not 0 <= summary["total_bytes"] <= MAX_TOTAL
        or summary["total_bytes"] > summary["file_count"] * MAX_FILE
        or type(summary["manifest_sha256"]) is not str
        or re.fullmatch(r"[0-9a-f]{64}", summary["manifest_sha256"]) is None):
        raise MigrationSourceInventoryError()
    return summary


def _worker_main():
    try:
        raw = sys.stdin.buffer.read(MAX_JSON + 1)
        if len(raw) > MAX_JSON:
            raise MigrationSourceInventoryError()
        request = _json(raw)
        if type(request) is not dict or set(request) != {"source_root", "selection_file", "new_dir"}:
            raise MigrationSourceInventoryError()
        if any(type(request[key]) is not str or not Path(request[key]).is_absolute() for key in request):
            raise MigrationSourceInventoryError()
        result = _inventory_worker(Path(request["source_root"]), Path(request["selection_file"]), Path(request["new_dir"]))
        sys.stdout.buffer.write(_wire(result))
        sys.stdout.buffer.flush()
        return 0
    except Exception:
        return 2


def inventory_migration_sources(source_root: Path, selection_file: Path, new_dir: Path, timeout=20) -> dict:
    """Read/hash explicitly selected files and create one bounded local manifest.

    The owned worker covers validation, file IO and fsync. Timeout excludes OS
    scheduling/cleanup stalls. Only fixed summary fields leave this API; no
    source content is copied or executed. Partial output is never auto-deleted.
    """
    try:
        if type(timeout) not in (int, float) or not math.isfinite(timeout) or timeout <= 0:
            raise MigrationSourceInventoryError()
        request = _wire({"source_root": os.path.abspath(os.fspath(source_root)),
            "selection_file": os.path.abspath(os.fspath(selection_file)),
            "new_dir": os.path.abspath(os.fspath(new_dir))})
        if len(request) > MAX_JSON:
            raise MigrationSourceInventoryError()
        pid, result = owned_probe.run_captured_probe(
            [sys.executable, str(Path(__file__).resolve()), "--worker"],
            os.environ.copy(), timeout=timeout, input_bytes=request)
        if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int or result.returncode != 0
            or type(result.stdout) is not bytes or type(result.stderr) is not bytes or result.stderr):
            raise MigrationSourceInventoryError()
        if len(result.stdout) > 1024 or not result.stdout.endswith(b"\n") or result.stdout.count(b"\n") != 1:
            raise MigrationSourceInventoryError()
        result = _json(result.stdout)
        if (type(result) is not dict or set(result) != {"process_pid", "summary"}
            or type(result["process_pid"]) is not int or result["process_pid"] != pid):
            raise MigrationSourceInventoryError()
        return _validate_summary(result["summary"])
    except BaseException:
        try:
            owned_probe.reap_owned_probes()
        except BaseException:
            pass  # Unreaped captured handles remain registered; no success.
        raise MigrationSourceInventoryError() from None


if __name__ == "__main__":
    raise SystemExit(_worker_main() if sys.argv[1:] == ["--worker"] else 2)
