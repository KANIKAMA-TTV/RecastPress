"""One caller-selected GET, recorded as a private, bounded development baseline.

No cookies, authentication, environment proxies, redirects, retries or content
decoding. This is neither raw wire capture nor a signed migration proof. Files
can contain private URLs, Location values and bodies. Failures retain partial
directories; pairs/parent-directory durability and adversarial filesystem races
are not guaranteed. Only this worker is terminated by its parent deadline.
"""

from datetime import datetime, timezone
import hashlib
import http.client
import json
import math
import os
from pathlib import Path
import re
import ssl
import sys
from urllib.parse import urlsplit

import owned_probe

MAX_BODY = 8 * 1024 * 1024
MAX_METADATA = 64 * 1024
MAX_URL = 8192
MAX_CONTROL_BYTES = 64 * 1024
_HEADERS = frozenset(("content-type", "content-encoding", "content-length",
    "cache-control", "etag", "last-modified", "location", "vary"))
_FAILURE = "HTTP baseline capture failed"


class HttpBaselineError(Exception):
    """Closed error without URL, path, body or transport diagnostics."""

    def __init__(self):
        super().__init__(_FAILURE)

    def __repr__(self):
        return "HttpBaselineError()"


def _url_parts(url):
    if type(url) is not str or not 1 <= len(url) <= MAX_URL:
        raise HttpBaselineError()
    if any(ord(char) <= 32 or ord(char) >= 127 for char in url) or "#" in url:
        raise HttpBaselineError()
    parts = urlsplit(url)
    if parts.scheme not in ("http", "https") or not parts.netloc or "@" in parts.netloc:
        raise HttpBaselineError()
    # Backslashes, invalid brackets, empty explicit ports and authority escapes
    # have no implicit browser normalization in this profile.
    if "\\" in parts.netloc or "%" in parts.netloc or parts.netloc.endswith(":"):
        raise HttpBaselineError()
    host, port = parts.hostname, parts.port
    if not host or port is not None and not 1 <= port <= 65535:
        raise HttpBaselineError()
    if ":" in host:
        import ipaddress
        ipaddress.IPv6Address(host)
        if re.fullmatch(r"\[[0-9A-Fa-f:.]+\](?::[0-9]+)?", parts.netloc) is None:
            raise HttpBaselineError()
    elif not re.fullmatch(r"[A-Za-z0-9](?:[A-Za-z0-9.-]*[A-Za-z0-9.])?", host):
        raise HttpBaselineError()
    elif any(not label or len(label) > 63 or label.startswith("-") or label.endswith("-")
             for label in host.rstrip(".").split(".")) or len(host) > 253:
        raise HttpBaselineError()
    # urlsplit intentionally drops an empty '?' delimiter, so consult the
    # original lexical tail to preserve it in the request target.
    tail = url[url.index("://") + 3 + len(parts.netloc):]
    target = parts.path or "/"
    if "?" in tail:
        target += "?" + parts.query
    if not target.startswith("/"):
        raise HttpBaselineError()
    return parts, host, port, target


def _new_directory(path):
    path = Path(os.path.abspath(os.fspath(path)))
    if os.path.lexists(path) or not path.parent.is_dir():
        raise HttpBaselineError()
    return path


def _strict_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise HttpBaselineError()
        result[key] = value
    return result


def _parse_json(raw):
    return json.loads(raw, object_pairs_hook=_strict_object,
        parse_constant=lambda _: (_ for _ in ()).throw(HttpBaselineError()))


def _json_bytes(value):
    return (json.dumps(value, ensure_ascii=True, sort_keys=True,
        separators=(",", ":"), allow_nan=False) + "\n").encode("ascii")


class _Lines:
    """Track finite control lines without reimplementing the HTTP parser."""

    def __init__(self, wrapped):
        self.wrapped = wrapped
        self.total = 0
        self.last_line = b""
        self.last_empty = False
        self.expected_crlf = False

    def readline(self, size=-1):
        line = self.wrapped.readline(size)
        self.total += len(line)
        if not line or not line.endswith(b"\n") or self.total > MAX_CONTROL_BYTES:
            raise HttpBaselineError()
        self.last_line = line
        self.last_empty = line in (b"\r\n", b"\n")
        return line

    def read(self, size=-1):
        result = self.wrapped.read(size)
        if self.expected_crlf:
            self.expected_crlf = False
            if size != 2 or result != b"\r\n":
                raise HttpBaselineError()
        return result

    def __getattr__(self, name):
        return getattr(self.wrapped, name)


class _Response(http.client.HTTPResponse):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.fp = _Lines(self.fp)
        self._control = self.fp

    def _read_status(self):
        result = super()._read_status()
        if result[0] not in ("HTTP/1.0", "HTTP/1.1"):
            raise HttpBaselineError()
        return result

    def begin(self):
        super().begin()
        if not self._control.last_empty or not 200 <= self.status <= 599:
            raise HttpBaselineError()
        lengths = self.headers.get_all("Content-Length", [])
        transfers = self.headers.get_all("Transfer-Encoding", [])
        if len(lengths) > 1 or len(transfers) > 1 or lengths and transfers:
            raise HttpBaselineError()
        if lengths and re.fullmatch(r"[0-9]+", lengths[0]) is None:
            raise HttpBaselineError()
        if lengths and self.status not in (204, 304):
            # Avoid both Python's configurable integer-digit limit and the
            # stdlib's fallback from a failed CL parse to close-delimited mode.
            canonical = lengths[0].lstrip("0") or "0"
            maximum = str(MAX_BODY)
            if len(canonical) > len(maximum) or (len(canonical) == len(maximum) and canonical > maximum):
                raise HttpBaselineError()
            self.length = int(canonical)
        if transfers and transfers[0].lower() != "chunked":
            raise HttpBaselineError()
        # Close the malformed-field cases reported by the stdlib parser.
        # This is not a complete RFC field validator; accepted obs-fold values
        # retain the parser's representation in selected private metadata.
        if self.headers.defects:
            raise HttpBaselineError()
        # HTTPResponse.read(amt) checks chunked before its zero length. These
        # statuses have no message body even when framing headers are retained
        # as observations; do not wait for a chunk stream or TLS EOF.
        if self.status in (204, 304):
            self.chunked = False
            self.length = 0

    def _get_chunk_left(self):
        if self.chunk_left == 0:
            self._control.expected_crlf = True
        return super()._get_chunk_left()

    def _read_next_chunk_size(self):
        size = super()._read_next_chunk_size()
        lexical = self._control.last_line.split(b";", 1)[0].rstrip(b"\r\n")
        if re.fullmatch(b"[0-9a-fA-F]+", lexical) is None or size < 0:
            raise HttpBaselineError()
        return size

    def _read_and_discard_trailer(self):
        trailers = http.client.parse_headers(self.fp)
        if trailers.defects or not self._control.last_empty:
            raise HttpBaselineError()


class _PlainConnection(http.client.HTTPConnection):
    response_class = _Response


class _SecureConnection(http.client.HTTPConnection):
    default_port = 443
    response_class = _Response

    def connect(self):
        # Public APIs only. No HTTP proxy CONNECT and no insecure fallback.
        super().connect()
        context = ssl.create_default_context()
        context.set_alpn_protocols(["http/1.1"])
        self.sock = context.wrap_socket(self.sock, server_hostname=self.host,
            suppress_ragged_eofs=False)


def _fetch(url):
    parts, host, port, target = _url_parts(url)
    connection = (_SecureConnection if parts.scheme == "https" else _PlainConnection)(
        host, port, timeout=20)
    try:
        connection.putrequest("GET", target, skip_host=True, skip_accept_encoding=True)
        connection.putheader("Host", parts.netloc)
        connection.putheader("Accept-Encoding", "identity")
        connection.putheader("Connection", "close")
        connection.putheader("User-Agent", "RecastPress-Manual-HTTP-Baseline/1")
        connection.endheaders()
        with connection.getresponse() as response:
            selected = {}
            for name, value in response.getheaders():
                lower = name.lower()
                if lower in _HEADERS:
                    selected.setdefault(lower, []).append(value)
            # Reject oversized selected metadata before reading the body too.
            if len(_json_bytes(selected)) > MAX_METADATA:
                raise HttpBaselineError()
            body = bytearray()
            # read(amt) does not itself reject a short Content-Length EOF.
            # Keep the original expected length to check it independently.
            expected = response.length
            while True:
                chunk = response.read(min(65536, MAX_BODY + 1 - len(body)))
                if not chunk:
                    break
                body.extend(chunk)
                if len(body) > MAX_BODY:
                    raise HttpBaselineError()
            if expected is not None and len(body) != expected:
                raise HttpBaselineError()
            metadata = {
                "format_version": 1,
                "scope": "manual_single_url_http_baseline",
                "url": url,
                "request_target": target,
                "method": "GET",
                "status": response.status,
                "http_version": "HTTP/1.1" if response.version == 11 else "HTTP/1.0",
                "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
                "transport": parts.scheme,
                "tls_verification": "default_verified" if parts.scheme == "https" else "not_applicable",
                "headers": selected,
                "body_file": "body.bin",
                "body_bytes": len(body),
                "body_sha256": hashlib.sha256(body).hexdigest(),
            }
            encoded = _json_bytes(metadata)
            if len(encoded) > MAX_METADATA:
                raise HttpBaselineError()
            return bytes(body), encoded, metadata
    finally:
        connection.close()


def _write_file(path, data):
    with path.open("xb") as output:
        if output.write(data) != len(data):
            raise HttpBaselineError()
        output.flush()
        os.fsync(output.fileno())


def _capture_worker(url, new_dir):
    _url_parts(url)
    directory = _new_directory(new_dir)
    body, encoded, metadata = _fetch(url)
    # All fetch/body/metadata validation precedes the first filesystem mutation.
    directory.mkdir()
    _write_file(directory / "body.bin", body)
    _write_file(directory / "capture.json", encoded)
    return {"format_version": 1, "process_pid": os.getpid(),
        "body_bytes": metadata["body_bytes"], "body_sha256": metadata["body_sha256"]}


def _worker_main():
    try:
        raw = sys.stdin.buffer.read(MAX_METADATA + 1)
        if len(raw) > MAX_METADATA:
            raise HttpBaselineError()
        request = _parse_json(raw)
        if type(request) is not dict or set(request) != {"url", "new_dir"}:
            raise HttpBaselineError()
        if type(request["new_dir"]) is not str or not Path(request["new_dir"]).is_absolute():
            raise HttpBaselineError()
        receipt = _capture_worker(request["url"], Path(request["new_dir"]))
        sys.stdout.buffer.write(_json_bytes(receipt))
        sys.stdout.buffer.flush()
        return 0
    except Exception:
        return 2


def capture_http_baseline(url: str, new_dir: Path, *, timeout: float = 20.0) -> None:
    """Capture one GET in an owned worker, with fixed errors and partial retain.

    Timeout bounds the child operation, not OS cleanup, scheduler or filesystem
    stalls. Default CA trust environment is retained; TLS key logging is not.
    Parent directory/ancestors are trusted, not a filesystem sandbox.
    """
    try:
        _url_parts(url)
        directory = _new_directory(new_dir)
        if type(timeout) not in (int, float) or not math.isfinite(timeout) or timeout <= 0:
            raise HttpBaselineError()
        request = _json_bytes({"url": url, "new_dir": str(directory)})
        if len(request) > MAX_METADATA:
            raise HttpBaselineError()
        environment = {key: value for key, value in os.environ.items()
            if key.upper() != "SSLKEYLOGFILE"}
        pid, result = owned_probe.run_captured_probe(
            [sys.executable, str(Path(__file__).resolve()), "--worker"],
            environment, timeout=timeout, input_bytes=request)
        if (type(pid) is not int or pid <= 0
            or type(result.returncode) is not int or result.returncode != 0
            or type(result.stdout) is not bytes or type(result.stderr) is not bytes
            or result.stderr):
            raise HttpBaselineError()
        if len(result.stdout) > 1024 or not result.stdout.endswith(b"\n") or result.stdout.count(b"\n") != 1:
            raise HttpBaselineError()
        receipt = _parse_json(result.stdout)
        if type(receipt) is not dict or set(receipt) != {"format_version", "process_pid", "body_bytes", "body_sha256"}:
            raise HttpBaselineError()
        if (type(receipt["format_version"]) is not int or receipt["format_version"] != 1
            or type(receipt["process_pid"]) is not int or receipt["process_pid"] != pid
            or type(receipt["body_bytes"]) is not int or not 0 <= receipt["body_bytes"] <= MAX_BODY
            or type(receipt["body_sha256"]) is not str
            or re.fullmatch(r"[0-9a-f]{64}", receipt["body_sha256"]) is None):
            raise HttpBaselineError()
    except BaseException:
        try:
            owned_probe.reap_owned_probes()
        except BaseException:
            pass  # Original/cleanup details must not escape the closed API.
        raise HttpBaselineError() from None


if __name__ == "__main__":
    raise SystemExit(_worker_main() if sys.argv[1:] == ["--worker"] else 2)
