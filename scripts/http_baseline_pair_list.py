"""All-or-error offline comparison of a caller-selected bounded capture list.

No crawl, URL interpretation, refetch, automatic pairing or HTML execution.
Paths are explicit filesystem choices, not a filesystem/network sandbox;
remote shares/mounts and stable parent/host behavior remain host assumptions.
Each result describes individually validated buffers, not an atomic snapshot
of two packages or the whole list. Differences never imply migration failure
or success. No input, path, header value, body or hash enters the report.
"""

import math
import os
from pathlib import Path
import sys

import owned_probe
from http_baseline_compare import (
    _BOOLEANS, _compare_worker, _json, _read_regular, _validate_report, _wire,
)

MAX_PAIRS = 32
MAX_MANIFEST_BYTES = 64 * 1024
MAX_REPORT_BYTES = 64 * 1024
_SCOPE = "manual_http_baseline_pair_list_comparison"
_FAILURE = "HTTP baseline pair comparison failed"
_PAIR_KEYS = {"ordinal", "changed_header_names", *_BOOLEANS}


class HttpBaselinePairListError(Exception):
    """Closed error without private input or arbitrary parser/IO diagnostics."""

    def __init__(self):
        super().__init__(_FAILURE)

    def __repr__(self):
        return "HttpBaselinePairListError()"


def _path(value, base):
    if type(value) is not str or not value or "\0" in value:
        raise HttpBaselinePairListError()
    value.encode("utf-8")  # Reject JSON surrogate escapes as path strings.
    path = Path(value)
    if not path.is_absolute():
        # Rooted/drive-relative Win32 paths are neither fully absolute nor
        # relative to the manifest's parent. Do not silently reinterpret them.
        if path.anchor:
            raise HttpBaselinePairListError()
        path = base / path
    return Path(os.path.abspath(path))


def _manifest(raw, base):
    value = _json(raw)
    if type(value) is not dict or set(value) != {"format_version", "pairs"}:
        raise HttpBaselinePairListError()
    if (type(value["format_version"]) is not int or value["format_version"] != 1
        or type(value["pairs"]) is not list or not 1 <= len(value["pairs"]) <= MAX_PAIRS):
        raise HttpBaselinePairListError()
    selected = []
    for pair in value["pairs"]:
        if type(pair) is not dict or set(pair) != {"left", "right"}:
            raise HttpBaselinePairListError()
        selected.append((_path(pair["left"], base), _path(pair["right"], base)))
    return selected


def _pair_list_worker(manifest):
    manifest = Path(os.path.abspath(os.fspath(manifest)))
    selected = _manifest(_read_regular(manifest, MAX_MANIFEST_BYTES), manifest.parent)
    reports = []
    for ordinal, (left, right) in enumerate(selected, 1):
        # Direct existing strict helper, never the public API that spawns a
        # child. Each call releases its two bounded bodies before the next.
        observed = _compare_worker(left, right)
        if (type(observed) is not dict or set(observed) != {"process_pid", "report"}
            or type(observed["process_pid"]) is not int or observed["process_pid"] != os.getpid()):
            raise HttpBaselinePairListError()
        report = _validate_report(observed["report"])
        reports.append({"ordinal": ordinal, **{key: value for key, value in report.items()
            if key not in ("format_version", "scope")}})
    report = {"format_version": 1, "scope": _SCOPE, "pairs": reports}
    result = {"process_pid": os.getpid(), "report": report}
    if len(_wire(result)) > MAX_REPORT_BYTES:
        raise HttpBaselinePairListError()
    return result


def _validate_pair_list_report(report):
    if type(report) is not dict or set(report) != {"format_version", "scope", "pairs"}:
        raise HttpBaselinePairListError()
    if (type(report["format_version"]) is not int or report["format_version"] != 1
        or report["scope"] != _SCOPE or type(report["pairs"]) is not list
        or not 1 <= len(report["pairs"]) <= MAX_PAIRS):
        raise HttpBaselinePairListError()
    for ordinal, pair in enumerate(report["pairs"], 1):
        if (type(pair) is not dict or set(pair) != _PAIR_KEYS
            or type(pair["ordinal"]) is not int or pair["ordinal"] != ordinal):
            raise HttpBaselinePairListError()
        _validate_report({"format_version": 1, "scope": "manual_http_baseline_structural_comparison",
            **{key: value for key, value in pair.items() if key != "ordinal"}})
    return report


def _worker_main():
    try:
        raw = sys.stdin.buffer.read(MAX_MANIFEST_BYTES + 1)
        if len(raw) > MAX_MANIFEST_BYTES:
            raise HttpBaselinePairListError()
        request = _json(raw)
        if (type(request) is not dict or set(request) != {"manifest"}
            or type(request["manifest"]) is not str or not Path(request["manifest"]).is_absolute()):
            raise HttpBaselinePairListError()
        result = _pair_list_worker(Path(request["manifest"]))
        # No partial report is emitted before all jobs/validation have finished.
        sys.stdout.buffer.write(_wire(result))
        sys.stdout.buffer.flush()
        return 0
    except Exception:
        return 2


def compare_http_baseline_pairs(manifest: Path, timeout=20) -> dict:
    """Compare 1..32 explicit pairs in one owned worker with no nested child.

    All valid differences are ordinary results. Any invalid job rejects the
    entire list with a fixed error. No output files or existing inputs change.
    Timeout includes list IO and all jobs, excludes OS scheduling/cleanup SLA.
    Accepted body data is at most 512MiB across the list, not a total-I/O/RSS
    guarantee; metadata/sentinels/failed reads can add IO, including repeated
    caller-selected paths. This is not source authentication or migration pass.
    """
    try:
        if type(timeout) not in (int, float) or not math.isfinite(timeout) or timeout <= 0:
            raise HttpBaselinePairListError()
        request = _wire({"manifest": os.path.abspath(os.fspath(manifest))})
        if len(request) > MAX_MANIFEST_BYTES:
            raise HttpBaselinePairListError()
        pid, result = owned_probe.run_captured_probe(
            [sys.executable, str(Path(__file__).resolve()), "--worker"], os.environ.copy(),
            timeout=timeout, input_bytes=request)
        if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int or result.returncode != 0
            or type(result.stdout) is not bytes or type(result.stderr) is not bytes or result.stderr):
            raise HttpBaselinePairListError()
        if (len(result.stdout) > MAX_REPORT_BYTES or not result.stdout.endswith(b"\n")
            or result.stdout.count(b"\n") != 1):
            raise HttpBaselinePairListError()
        envelope = _json(result.stdout)
        if (type(envelope) is not dict or set(envelope) != {"process_pid", "report"}
            or type(envelope["process_pid"]) is not int or envelope["process_pid"] != pid):
            raise HttpBaselinePairListError()
        return _validate_pair_list_report(envelope["report"])
    except BaseException:
        try:
            owned_probe.reap_owned_probes()
        except BaseException:
            pass  # Retained handles are not evidence of successful cleanup.
        raise HttpBaselinePairListError() from None


if __name__ == "__main__":
    raise SystemExit(_worker_main() if sys.argv[1:] == ["--worker"] else 2)
