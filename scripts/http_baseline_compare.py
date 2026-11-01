"""Read-only, private-value-free comparison of two bounded v1 baselines.

Differences are observations, never a migration pass/fail or semantic proof.
Parent directories and the host are trusted. Detected links/reparse points,
special files and apparent mutations are refused, without claiming a hostile
filesystem sandbox or an absolute OS cleanup deadline.
"""

from datetime import datetime, timedelta
import hashlib
import json
import os
from pathlib import Path
import re
import stat
import sys
import math

import owned_probe
from http_baseline import MAX_BODY, MAX_METADATA, _url_parts

_FAILURE = "HTTP baseline comparison failed"
_SCOPE = "manual_http_baseline_structural_comparison"
_HEADERS = frozenset(("content-type", "content-encoding", "content-length",
    "cache-control", "etag", "last-modified", "location", "vary"))
_FIELDS = frozenset(("format_version", "scope", "url", "request_target", "method",
    "status", "http_version", "recorded_at_utc", "transport", "tls_verification",
    "headers", "body_file", "body_bytes", "body_sha256"))
_BOOLEANS = ("body_equal", "status_equal", "headers_equal", "http_version_equal",
    "request_target_equal", "url_equal", "transport_equal", "tls_verification_equal")


class HttpBaselineCompareError(Exception):
    """Closed error with no input, path, value or transport diagnostics."""

    def __init__(self):
        super().__init__(_FAILURE)

    def __repr__(self):
        return "HttpBaselineCompareError()"


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise HttpBaselineCompareError()
        result[key] = value
    return result


def _not_number(_):
    raise HttpBaselineCompareError()


def _json(raw):
    return json.loads(raw.decode("utf-8"), object_pairs_hook=_object,
        parse_float=_not_number, parse_constant=_not_number)


def _wire(value):
    return (json.dumps(value, ensure_ascii=True, sort_keys=True,
        separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")


def _linked(info):
    return stat.S_ISLNK(info.st_mode) or bool(getattr(info, "st_file_attributes", 0)
        & getattr(stat, "FILE_ATTRIBUTE_REPARSE_POINT", 0))


def _identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_size, info.st_mtime_ns)


def _directory(path):
    path = Path(os.path.abspath(os.fspath(path)))
    info = path.lstat()
    if _linked(info) or not stat.S_ISDIR(info.st_mode):
        raise HttpBaselineCompareError()
    names = []
    with os.scandir(path) as entries:
        for entry in entries:
            names.append(entry.name)
            if len(names) == 3:
                break
    if set(names) != {"body.bin", "capture.json"} or len(names) != 2:
        raise HttpBaselineCompareError()
    return path, info


def _read_regular(path, maximum):
    before = path.lstat()
    if _linked(before) or not stat.S_ISREG(before.st_mode) or before.st_size > maximum:
        raise HttpBaselineCompareError()
    flags = os.O_RDONLY | getattr(os, "O_BINARY", 0) | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
    descriptor = os.open(path, flags)
    try:
        opened = os.fstat(descriptor)
        if _linked(opened) or not stat.S_ISREG(opened.st_mode) or _identity(opened) != _identity(before):
            raise HttpBaselineCompareError()
        data = bytearray()
        while len(data) <= maximum:
            chunk = os.read(descriptor, min(65536, maximum + 1 - len(data)))
            if not chunk:
                break
            data.extend(chunk)
        if len(data) > maximum or _identity(os.fstat(descriptor)) != _identity(opened):
            raise HttpBaselineCompareError()
        after = path.lstat()
        if _linked(after) or _identity(after) != _identity(opened):
            raise HttpBaselineCompareError()
        return bytes(data)
    finally:
        os.close(descriptor)


def _metadata(value):
    if type(value) is not dict or set(value) != _FIELDS:
        raise HttpBaselineCompareError()
    if (type(value["format_version"]) is not int or value["format_version"] != 1
        or value["scope"] != "manual_single_url_http_baseline"
        or value["method"] != "GET" or value["body_file"] != "body.bin"
        or type(value["status"]) is not int or not 200 <= value["status"] <= 599
        or value["http_version"] not in ("HTTP/1.0", "HTTP/1.1")
        or type(value["body_bytes"]) is not int or not 0 <= value["body_bytes"] <= MAX_BODY
        or type(value["body_sha256"]) is not str
        or re.fullmatch(r"[0-9a-f]{64}", value["body_sha256"]) is None):
        raise HttpBaselineCompareError()
    parts, _, _, target = _url_parts(value["url"])
    if (type(value["request_target"]) is not str or value["request_target"] != target
        or value["transport"] != parts.scheme
        or value["tls_verification"] != ("default_verified" if parts.scheme == "https" else "not_applicable")):
        raise HttpBaselineCompareError()
    timestamp = value["recorded_at_utc"]
    if type(timestamp) is not str or re.fullmatch(
        r"[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}(?:\.[0-9]{1,6})?(?:Z|\+00:00)", timestamp) is None:
        raise HttpBaselineCompareError()
    if datetime.fromisoformat(timestamp.replace("Z", "+00:00")).utcoffset() != timedelta(0):
        raise HttpBaselineCompareError()
    headers = value["headers"]
    if type(headers) is not dict or not set(headers) <= _HEADERS:
        raise HttpBaselineCompareError()
    for values in headers.values():
        # The capture's HTTP parser decodes fields as Latin-1. Empty values,
        # obs-fold representation and duplicate order remain literal data.
        if type(values) is not list or not values or any(type(item) is not str
            or any(ord(char) > 255 for char in item) for item in values):
            raise HttpBaselineCompareError()
    if "content-length" in headers:
        lengths = headers["content-length"]
        if len(lengths) != 1 or re.fullmatch(r"[0-9]+", lengths[0]) is None:
            raise HttpBaselineCompareError()
        if value["status"] not in (204, 304) and (lengths[0].lstrip("0") or "0") != str(value["body_bytes"]):
            raise HttpBaselineCompareError()
    if value["status"] in (204, 304) and value["body_bytes"] != 0:
        raise HttpBaselineCompareError()
    return value


def _load(path):
    directory, before = _directory(path)
    metadata = _metadata(_json(_read_regular(directory / "capture.json", MAX_METADATA)))
    body = _read_regular(directory / "body.bin", MAX_BODY)
    if len(body) != metadata["body_bytes"] or hashlib.sha256(body).hexdigest() != metadata["body_sha256"]:
        raise HttpBaselineCompareError()
    # Detect ordinary mutation. This does not establish ABA-free filesystem
    # identity or authenticate who produced these unsigned local files.
    after = directory.lstat()
    if _linked(after) or _identity(after) != _identity(before):
        raise HttpBaselineCompareError()
    return metadata, body


def _compare_worker(left, right):
    first, first_body = _load(left)
    second, second_body = _load(right)
    changed = sorted(name for name in _HEADERS if first["headers"].get(name) != second["headers"].get(name))
    report = {"format_version": 1, "scope": _SCOPE,
        "body_equal": first_body == second_body,
        "status_equal": first["status"] == second["status"],
        "headers_equal": not changed,
        "http_version_equal": first["http_version"] == second["http_version"],
        "request_target_equal": first["request_target"] == second["request_target"],
        "url_equal": first["url"] == second["url"],
        "transport_equal": first["transport"] == second["transport"],
        "tls_verification_equal": first["tls_verification"] == second["tls_verification"],
        "changed_header_names": changed}
    return {"process_pid": os.getpid(), "report": report}


def _validate_report(report):
    if type(report) is not dict or set(report) != {"format_version", "scope", "changed_header_names", *_BOOLEANS}:
        raise HttpBaselineCompareError()
    if type(report["format_version"]) is not int or report["format_version"] != 1 or report["scope"] != _SCOPE:
        raise HttpBaselineCompareError()
    if any(type(report[key]) is not bool for key in _BOOLEANS):
        raise HttpBaselineCompareError()
    names = report["changed_header_names"]
    if type(names) is not list or any(type(name) is not str or name not in _HEADERS for name in names):
        raise HttpBaselineCompareError()
    if names != sorted(set(names)) or report["headers_equal"] != (not names):
        raise HttpBaselineCompareError()
    return report


def _worker_main():
    try:
        raw = sys.stdin.buffer.read(MAX_METADATA + 1)
        if len(raw) > MAX_METADATA:
            raise HttpBaselineCompareError()
        request = _json(raw)
        if type(request) is not dict or set(request) != {"left", "right"}:
            raise HttpBaselineCompareError()
        if any(type(request[key]) is not str or not Path(request[key]).is_absolute() for key in request):
            raise HttpBaselineCompareError()
        result = _compare_worker(Path(request["left"]), Path(request["right"]))
        sys.stdout.buffer.write(_wire(result))
        sys.stdout.buffer.flush()
        return 0
    except Exception:
        return 2


def compare_http_baselines(left: Path, right: Path, timeout=20) -> dict:
    """Compare validated bounded local artifacts; no network or file mutation.

    A valid difference is an ordinary result, not a failed migration. URLs,
    header values, bodies, hashes and private paths never enter the report.
    Worker timeout excludes the OS scheduling/cleanup SLA. Trusted stable
    parent paths are required; this is not an adversarial filesystem sandbox.
    """
    try:
        if type(timeout) not in (int, float) or not math.isfinite(timeout) or timeout <= 0:
            raise HttpBaselineCompareError()
        request = _wire({"left": os.path.abspath(os.fspath(left)), "right": os.path.abspath(os.fspath(right))})
        if len(request) > MAX_METADATA:
            raise HttpBaselineCompareError()
        pid, result = owned_probe.run_captured_probe(
            [sys.executable, str(Path(__file__).resolve()), "--worker"], os.environ.copy(),
            timeout=timeout, input_bytes=request)
        if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int
            or result.returncode != 0 or type(result.stdout) is not bytes
            or type(result.stderr) is not bytes or result.stderr):
            raise HttpBaselineCompareError()
        if len(result.stdout) > 4096 or not result.stdout.endswith(b"\n") or result.stdout.count(b"\n") != 1:
            raise HttpBaselineCompareError()
        envelope = _json(result.stdout)
        if (type(envelope) is not dict or set(envelope) != {"process_pid", "report"}
            or type(envelope["process_pid"]) is not int or envelope["process_pid"] != pid):
            raise HttpBaselineCompareError()
        return _validate_report(envelope["report"])
    except BaseException:
        try:
            owned_probe.reap_owned_probes()
        except BaseException:
            pass  # Registry retains unreaped handles; never claim success.
        raise HttpBaselineCompareError() from None


if __name__ == "__main__":
    raise SystemExit(_worker_main() if sys.argv[1:] == ["--worker"] else 2)
