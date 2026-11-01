"""Run the actual public and management binaries on OS-assigned loopback ports."""

import http.client
from dataclasses import dataclass, field
import json
import os
from pathlib import Path
import queue
import re
import signal
import socket
import subprocess
import threading
import tempfile

ROOT = Path(__file__).resolve().parents[1]
BINARY = ROOT / "target" / "debug" / ("recastpress.exe" if os.name == "nt" else "recastpress")
STARTUP_TIMEOUT = 10
STOP_TIMEOUT = 5
READER_TIMEOUT = 5
_OWNED_SERVERS = {}
_SERVERS_LOCK = threading.Lock()


@dataclass
class Server:
    process: subprocess.Popen
    port: int
    lines: queue.Queue
    reader: threading.Thread


@dataclass
class _CapturedServer:
    process: subprocess.Popen
    reader: threading.Thread | None = None
    reader_errors: list = field(default_factory=list)


def _remember(process):
    owner = _CapturedServer(process)
    with _SERVERS_LOCK:
        _OWNED_SERVERS[id(process)] = owner
    return owner


def _read_startup(owner, lines):
    try:
        for line in owner.process.stderr:
            lines.put(line)
    except BaseException as error:
        owner.reader_errors.append(error)
    finally:
        lines.put(None)


def _stop_owned(owner):
    """Try every cleanup stage without a context-manager or unbounded wait.

    A live/unconfirmed process or active reader keeps its buffered pipe and
    strong registry entry. Closing a TextIOWrapper while readline is blocked
    could otherwise wait forever on the reader's internal buffer lock.
    """
    process = owner.process
    errors = []
    try:
        if process.poll() is None:
            process.terminate()
    except BaseException as error:
        errors.append(f"owned server {process.pid} terminate: {type(error).__name__}: {error}")
    try:
        process.wait(timeout=STOP_TIMEOUT)
    except subprocess.TimeoutExpired:
        pass  # Escalate a slow graceful stop to this same captured handle.
    except BaseException as error:
        errors.append(f"owned server {process.pid} wait: {type(error).__name__}: {error}")
    try:
        if process.poll() is None:
            process.kill()
    except BaseException as error:
        errors.append(f"owned server {process.pid} kill: {type(error).__name__}: {error}")
    try:
        process.wait(timeout=STOP_TIMEOUT)
    except BaseException as error:
        errors.append(f"owned server {process.pid} final wait: {type(error).__name__}: {error}")
    try:
        stopped = process.poll() is not None
    except BaseException as error:
        stopped = False
        errors.append(f"owned server {process.pid} poll: {type(error).__name__}: {error}")
    reader = owner.reader
    reader_active = False
    if reader is not None:
        try:
            # ident remains set after a started thread exits. A failed start
            # with ident=None must not be joined as if it had started.
            if reader.ident is not None:
                reader.join(timeout=READER_TIMEOUT)
            reader_active = reader.is_alive()
        except BaseException as error:
            reader_active = True
            errors.append(f"owned server {process.pid} reader join: {type(error).__name__}: {error}")
    if owner.reader_errors:
        errors.append(f"owned server {process.pid} stderr reader failed")
    closed = False
    if stopped and not reader_active:
        close_errors = []
        for name in ("stdin", "stdout", "stderr"):
            stream = getattr(process, name, None)
            if stream is not None:
                try:
                    stream.close()
                except BaseException as error:
                    close_errors.append(f"owned server {process.pid} {name} close: {type(error).__name__}: {error}")
        errors.extend(close_errors)
        try:
            closed = not close_errors and all(
                getattr(process, name, None) is None or getattr(process, name).closed
                for name in ("stdin", "stdout", "stderr"))
        except BaseException as error:
            errors.append(f"owned server {process.pid} IO close confirmation: {type(error).__name__}: {error}")
        if closed:
            with _SERVERS_LOCK:
                _OWNED_SERVERS.pop(id(process), None)
    if not stopped or reader_active or not closed:
        errors.append(f"owned server {process.pid} process/reader/IO unconfirmed; captured handle retained")
    return errors


def _cleanup_result(errors, original):
    if errors:
        message = "owned HTTP cleanup failed: " + "; ".join(errors)
        if original is not None:
            original.add_note(message)
        else:
            raise RuntimeError(message)


def reap_servers(*, original=None):
    """Retry all captured HTTP children, continuing after one cleanup failure."""
    with _SERVERS_LOCK:
        owners = list(_OWNED_SERVERS.values())
    errors = []
    for owner in reversed(owners):
        try:
            errors.extend(_stop_owned(owner))
        except BaseException as error:
            errors.append(f"owned server {owner.process.pid} cleanup: {type(error).__name__}: {error}")
    _cleanup_result(errors, original)


def start(role, *options, environment=None, binary=None):
    owner = _remember(subprocess.Popen(
        [str(binary or BINARY), role, "--listen", "127.0.0.1:0", *options],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        env=environment,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    ))
    proc = owner.process
    try:
        lines = queue.Queue()
        reader = threading.Thread(target=_read_startup, args=(owner, lines),
            name=f"smoke-server-reader-{proc.pid}", daemon=True)
        owner.reader = reader
        reader.start()
        try:
            line = lines.get(timeout=STARTUP_TIMEOUT)
        except queue.Empty as error:
            raise TimeoutError("owned server did not announce readiness before timeout") from error
        if owner.reader_errors:
            raise RuntimeError("owned server stderr reader failed") from owner.reader_errors[0]
        match = re.search(r"listening on 127\.0\.0\.1:(\d+)", line or "")
        if not match:
            raise RuntimeError(f"server did not start: {line!r}")
        return Server(proc, int(match[1]), lines, reader)
    except BaseException as error:
        _cleanup_result(_stop_owned(owner), error)
        raise


def stop_process(proc, reader, *, original=None):
    with _SERVERS_LOCK:
        owner = _OWNED_SERVERS.get(id(proc))
    if owner is None:
        owner = _remember(proc)
    if owner.reader is None:
        owner.reader = reader
    _cleanup_result(_stop_owned(owner), original)


def stop(server, *, original=None):
    stop_process(server.process, server.reader, original=original)


def expect_start_failure(*arguments):
    result = subprocess.run(
        [str(BINARY), *arguments], cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        timeout=10, creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0,
    )
    assert result.returncode != 0, "invalid startup unexpectedly succeeded"
    assert result.stderr, "startup failure had no diagnostic"
    assert b"listening on" not in result.stderr, "failed startup announced a listener"


def startup_failures():
    root = (ROOT / "target/process-smoke-fixtures").resolve()
    assert root.is_relative_to(ROOT.resolve())
    root.mkdir(parents=True, exist_ok=True)
    temporary = tempfile.TemporaryDirectory(dir=root)
    assert Path(temporary.name).resolve().parent == root
    with temporary as directory:
        bundle = Path(directory) / "invalid.json"
        arguments = ["public", "--listen", "127.0.0.1:0", "--host", "example.test", "--bundle", str(bundle)]
        expect_start_failure(*arguments)  # Missing file.
        bundle.write_bytes(b"{")
        expect_start_failure(*arguments)
        page = {"target": "/", "status": 200, "content_type": "text/html; charset=utf-8", "body": "fixture"}
        for document in [
            {"format_version": 3, "pages": []},
            {"format_version": 2, "pages": []},
            {"format_version": 1, "pages": [page, page]},
            {"format_version": 1, "pages": [{**page, "target": "/health/ready"}]},
            {"format_version": 1, "pages": [], "unexpected": True},
            {"format_version": 2, "pages": [{"target": "/image.png", "status": 200,
                "content_type": "image/png", "body_hex": "89504e4Z"}]},
            {"format_version": 2, "pages": [{"target": "/image.png", "status": 200,
                "content_type": "image/png", "body_hex": "3c68746d6c3e"}]},
            {"format_version": 2, "pages": [{"target": "/", "status": 200,
                "content_type": "text/html; charset=utf-8", "body_hex": "ff"}]},
            {"format_version": 2, "pages": [{"target": "/old", "status": 302,
                "location": "//evil.test/"}]},
            {"format_version": 2, "pages": [{"target": "/old", "status": 200,
                "location": "/next"}]},
            {"format_version": 2, "pages": [{"target": "/old", "status": 302,
                "location": "/next", "body_hex": ""}]},
        ]:
            bundle.write_text(json.dumps(document), encoding="utf-8")
            expect_start_failure(*arguments)
        # One extra byte over the documented bounded-file limit.
        with bundle.open("wb") as output:
            output.truncate(8 * 1024 * 1024 + 1)
        expect_start_failure(*arguments)
    for arguments in [
        ["public", "--listen", "127.0.0.1:0"],
        ["public", "--listen", "127.0.0.1:0", "--host", "bad host"],
        ["management", "--listen", "127.0.0.1:0", "--bundle", "examples/public-bundle.json"],
    ]:
        expect_start_failure(*arguments)
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as occupied:
        if hasattr(socket, "SO_EXCLUSIVEADDRUSE"):
            occupied.setsockopt(socket.SOL_SOCKET, socket.SO_EXCLUSIVEADDRUSE, 1)
        occupied.bind(("127.0.0.1", 0))
        occupied.listen(1)
        address = f"127.0.0.1:{occupied.getsockname()[1]}"
        expect_start_failure("public", "--listen", address, "--host", "example.test")
        expect_start_failure("management", "--listen", address)


def unix_signals():
    if os.name != "posix":
        return "Unix signal checks not run on this platform"
    for role in ["public", "management"]:
        for number, name in [(signal.SIGINT, "SIGINT"), (signal.SIGTERM, "SIGTERM")]:
            server = start(role, *(["--host", "example.test"] if role == "public" else []))
            original = None
            try:
                server.process.send_signal(number)
                assert server.process.wait(timeout=5) == 0, f"{role} did not exit cleanly after {name}"
                server.reader.join(timeout=5)
                assert not server.reader.is_alive(), "shutdown log was not drained"
                lines = []
                while not server.lines.empty():
                    lines.append(server.lines.get_nowait())
                assert any(f"shutting down after {name}" in line for line in lines if line), "shutdown handler was not observed"
            except BaseException as error:
                original = error
                raise
            finally:
                stop(server, original=original)
    return "SIGINT/SIGTERM clean exits observed for both roles"


def request(port, target, method="GET", host="example.test", extra_headers=None):
    connection = http.client.HTTPConnection("127.0.0.1", port, timeout=5)
    try:
        request_headers = {"Host": host}
        request_headers.update(extra_headers or {})
        connection.request(method, target, headers=request_headers)
        response = connection.getresponse()
        assert response.getheader("cache-control") == "no-store"
        assert response.getheader("x-content-type-options") == "nosniff"
        return response.status, dict(response.getheaders()), response.read()
    finally:
        connection.close()


def binary_bundle():
    root = (ROOT / "target/process-smoke-fixtures").resolve()
    assert root.is_relative_to(ROOT.resolve())
    root.mkdir(parents=True, exist_ok=True)
    # Signature-only fixture: the server preserves bytes, not decoded pixels.
    image = b"\x89PNG\r\n\x1a\n" + bytes(range(256))
    temporary = tempfile.TemporaryDirectory(dir=root)
    assert Path(temporary.name).resolve().parent == root
    with temporary as directory:
        bundle = Path(directory) / "binary.json"
        bundle.write_text(json.dumps({"format_version": 2, "pages": [
            {"target": "/image.png?v=1", "status": 200, "content_type": "image/png",
             "body_hex": image.hex()},
            {"target": "/", "status": 200, "content_type": "text/plain; charset=utf-8",
             "body_hex": "binary bundle text".encode().hex()},
            {"target": "/legacy-image", "status": 308, "location": "/image.png?v=1"},
            {"target": "/missing-destination", "status": 302, "location": "/not-prepared"},
        ]}), encoding="utf-8")
        server = start("public", "--host", "example.test", "--bundle", str(bundle))
        original = None
        try:
            status, headers, body = request(server.port, "/image.png?v=1")
            assert status == 200 and body == image
            assert headers["content-type"] == "image/png"
            assert int(headers["content-length"]) == len(image)
            etag = headers["etag"]
            assert re.fullmatch(r'"[0-9a-f]{64}"', etag)
            status, headers, body = request(server.port, "/image.png?v=1", "HEAD")
            assert status == 200 and body == b""
            assert int(headers["content-length"]) == len(image)
            assert request(server.port, "/image.png?v=2")[0] == 404
            assert request(server.port, "/image.png?v=1", host="evil.test")[0] == 421
            assert request(server.port, "/")[2] == b"binary bundle text"
            for method in ["GET", "HEAD"]:
                status, headers, body = request(server.port, "/legacy-image", method)
                assert status == 308 and body == b""
                assert headers["location"] == "/image.png?v=1"
                assert headers["content-type"] == "text/plain; charset=utf-8"
                assert headers["content-length"] == "0"
            status, headers, body = request(server.port, "/missing-destination")
            assert status == 302 and headers["location"] == "/not-prepared" and body == b""
            assert request(server.port, "/not-prepared")[0] == 404
            assert request(server.port, "/legacy-image", "POST")[0] == 405
            status, headers, body = request(server.port, "/image.png?v=1",
                                            extra_headers={"Range": "bytes=8-10"})
            assert status == 206 and body == image[8:11]
            assert headers["content-type"] == "image/png" and headers["content-length"] == "3"
            assert headers["content-range"] == f"bytes 8-10/{len(image)}"
            status, headers, body = request(server.port, "/image.png?v=1",
                                            extra_headers={"Range": f"bytes={len(image)}-"})
            assert status == 416 and body == b"" and headers["content-length"] == "0"
            assert headers["content-range"] == f"bytes */{len(image)}"
            status, headers, body = request(server.port, "/image.png?v=1", "HEAD",
                                            extra_headers={"Range": "bytes=8-10"})
            assert status == 200 and body == b"" and int(headers["content-length"]) == len(image)
            assert "content-range" not in headers
            for controls in [{"Range": "bytes=8-10", "If-Range": '"unknown"'},
                             {"Range": "bytes=0-1,4-5"}, {"Range": "other=0-1"}]:
                status, headers, body = request(server.port, "/image.png?v=1", extra_headers=controls)
                assert status == 200 and body == image and "content-range" not in headers
            status, headers, body = request(server.port, "/legacy-image",
                                            extra_headers={"Range": "bytes=0-0"})
            assert status == 308 and body == b"" and "content-range" not in headers
            assert headers["location"] == "/image.png?v=1"
            for method in ["GET", "HEAD"]:
                status, headers, body = request(server.port, "/image.png?v=1", method,
                                                extra_headers={"If-None-Match": "W/" + etag})
                assert status == 304 and body == b"" and headers["etag"] == etag
                # Axum gets the original length explicitly; HTTP/1 transport
                # may omit it for 304. Zero for a nonempty representation is wrong.
                assert headers.get("content-length") in (None, str(len(image)))
                assert "content-range" not in headers
            status, headers, body = request(server.port, "/image.png?v=1",
                                            extra_headers={"If-Match": '"foreign"', "If-None-Match": etag,
                                                           "Range": "bytes=8-10"})
            assert status == 412 and body == b"" and headers["content-length"] == "0"
            assert "content-range" not in headers
            status, headers, body = request(server.port, "/image.png?v=1",
                                            extra_headers={"If-None-Match": etag, "If-Range": '"foreign"',
                                                           "Range": "bytes=wrong"})
            assert status == 304 and body == b"" and headers.get("content-length") in (None, str(len(image)))
            for controls in [{"If-Match": etag, "Range": "bytes=8-10"},
                             {"If-None-Match": '"foreign"', "Range": "bytes=8-10"},
                             {"If-Range": etag, "Range": "bytes=8-10"}]:
                status, headers, body = request(server.port, "/image.png?v=1", extra_headers=controls)
                assert status == 206 and body == image[8:11] and headers["etag"] == etag
            for value, expected in [(etag + ", invalid", 400), ('"' + 'x' * 4095 + '"', 431)]:
                status, headers, body = request(server.port, "/image.png?v=1",
                                                extra_headers={"If-None-Match": value})
                assert status == expected and body == b"" and headers["content-length"] == "0"
        except BaseException as error:
            original = error
            raise
        finally:
            stop(server, original=original)


def _smoke_main():
    if not BINARY.is_file():
        raise SystemExit("Run cargo build --workspace --locked first")
    startup_failures()
    binary_bundle()
    public = start("public", "--host", "example.test", "--bundle", "examples/public-bundle.json")
    port = public.port
    original = None
    try:
        status, headers, body = request(port, "/")
        assert status == 200 and b"RecastPress" in body
        assert headers["cache-control"] == "no-store"
        assert int(headers["content-length"]) == len(body)
        status, head_headers, head_body = request(port, "/", "HEAD")
        assert status == 200 and not head_body
        assert head_headers["content-length"] == headers["content-length"]
        assert request(port, "/?s=rust")[0] == 200
        assert request(port, "/?s=php")[0] == 404
        assert request(port, "/", "POST")[0] == 405
        assert request(port, "/", host="evil.test")[0] == 421
        assert request(port, "/health/live")[0] == 200
        assert request(port, "/health/ready")[0] == 200
        assert request(port, "/health/live", "POST")[0] == 405
    except BaseException as error:
        original = error
        raise
    finally:
        stop(public, original=original)
    empty = start("public", "--host", "example.test")
    original = None
    try:
        assert request(empty.port, "/health/live")[0] == 200
        assert request(empty.port, "/health/ready")[0] == 503
        assert request(empty.port, "/")[0] == 503
    except BaseException as error:
        original = error
        raise
    finally:
        stop(empty, original=original)
    management = start("management")
    port = management.port
    original = None
    try:
        assert request(port, "/health/live")[0] == 200
        assert request(port, "/health/ready")[0] == 503
        assert request(port, "/publish", "POST")[0] == 404
        assert request(port, "/health/ready", "POST")[0] == 405
    except BaseException as error:
        original = error
        raise
    finally:
        stop(management, original=original)
    signals = unix_signals()
    print(f"Process smoke passed: startup rejection, binary bundle/redirect/Range/ETag, public/management isolation, HTTP, query, HEAD, host, readiness; {signals}")


def main():
    original = None
    try:
        _smoke_main()
    except BaseException as error:
        original = error
        raise
    finally:
        reap_servers(original=original)


if __name__ == "__main__":
    main()
