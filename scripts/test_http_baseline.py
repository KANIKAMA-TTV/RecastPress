"""Owned loopback HTTP/CLI contracts with public canaries; no external sites."""
from datetime import datetime, timedelta
import gzip
import hashlib
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import os
from pathlib import Path
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time as real_time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import http_baseline
import owned_probe
from owned_probe import reap_owned_probes, run_captured_probe

ROOT = Path(__file__).resolve().parents[1]
CLI = ROOT / "scripts/capture_http_baseline.py"
BODY_LIMIT = 8 * 1024 * 1024
SELECTED = {"content-type", "content-encoding", "content-length", "cache-control", "etag", "last-modified", "location", "vary"}


def response(body=b"", status=200, headers=(), framing="length"):
    fields = list(headers)
    if framing == "length":
        fields.append(("Content-Length", str(len(body))))
    elif framing == "chunked":
        fields.append(("Transfer-Encoding", "chunked"))
        body = format(len(body), "x").encode() + b"\r\n" + body + b"\r\n0\r\n\r\n" if body else b"0\r\n\r\n"
    return (f"HTTP/1.1 {status} Canary\r\n" + "".join(f"{name}: {value}\r\n" for name, value in fields) + "Connection: close\r\n\r\n").encode() + body


class Handler(BaseHTTPRequestHandler):
    def do_GET(self):
        self.close_connection = True
        self.server.observed.append((self.path, {key.lower(): value for key, value in self.headers.items()}))
        wire = self.server.routes[self.path]
        if wire is None:
            self.server.entered.set()
            self.server.release.wait(5)
            wire = response(b"released canary")
        try:
            self.connection.sendall(wire)
        except OSError:
            pass  # A refused/timeout client may already have closed its socket.

    def log_message(self, *args):
        pass


class OwnedServer(ThreadingHTTPServer):
    daemon_threads = True
    block_on_close = False

    def __init__(self, routes, tls_context=None):
        super().__init__(("127.0.0.1", 0), Handler)
        self.routes, self.context = routes, tls_context
        self.observed, self.handlers, self.sockets = [], [], []
        self.entered, self.release = threading.Event(), threading.Event()
        self.thread = threading.Thread(target=self.serve_forever, kwargs={"poll_interval": 0.02}, daemon=True)
        self.thread.start()

    def process_request(self, request, address):
        self.sockets.append(request)
        worker = threading.Thread(target=self.process_request_thread, args=(request, address), daemon=True)
        self.handlers.append(worker)
        worker.start()

    def process_request_thread(self, request, address):
        request.settimeout(3)
        if self.context is not None:
            try:
                request = self.context.wrap_socket(request, server_side=True)
                self.sockets.append(request)
            except (OSError, ssl.SSLError):
                request.close()
                return
        super().process_request_thread(request, address)

    def handle_error(self, request, address):
        pass

    def stop(self):
        self.release.set()
        shutdown = threading.Thread(target=self.shutdown, daemon=True)
        shutdown.start()
        shutdown.join(3)
        self.server_close()
        for connection in self.sockets:
            try:
                connection.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
            connection.close()
        self.thread.join(3)
        for worker in self.handlers:
            worker.join(3)
        if shutdown.is_alive() or self.thread.is_alive() or any(worker.is_alive() for worker in self.handlers):
            raise AssertionError("owned HTTP fixture did not stop before its join deadline")

    def url(self, target, *, hostname="127.0.0.1"):
        return f"{'https' if self.context else 'http'}://{hostname}:{self.server_port}{target}"


class HttpBaselineTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="http-baseline-test-", dir=ROOT / "target")
        self.work = Path(self.directory.name)
        self.servers = []
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(reap_owned_probes)
        self.addCleanup(self.stop_servers)

    def stop_servers(self):
        errors = []
        for server in reversed(self.servers):
            try:
                server.stop()
            except BaseException as error:
                errors.append(error)
        if errors:
            raise AssertionError("owned HTTP fixture cleanup failed") from errors[0]

    def server(self, routes, context=None):
        server = OwnedServer(routes, context)
        self.servers.append(server)
        return server

    def cli(self, *args, environment=None):
        _, result = run_captured_probe([sys.executable, str(CLI), *map(os.fspath, args)], environment or os.environ.copy(), timeout=30)
        return result

    def refused(self, *args, environment=None):
        result = self.cli(*args, environment=environment)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", b"HTTP baseline capture failed\n"))

    def recorded(self, url, destination, body, status=200, environment=None, tls="not_applicable", version="HTTP/1.1"):
        result = self.cli(url, destination, environment=environment)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, b"HTTP baseline recorded\n", b""))
        self.assertEqual({path.name for path in destination.iterdir()}, {"body.bin", "capture.json"})
        self.assertEqual((destination / "body.bin").read_bytes(), body)
        wire = (destination / "capture.json").read_bytes()
        self.assertLessEqual(len(wire), 65536)
        value = json.loads(wire)
        self.assertEqual(set(value), {"format_version", "scope", "url", "request_target", "method", "status", "http_version", "recorded_at_utc", "transport", "tls_verification", "headers", "body_file", "body_bytes", "body_sha256"})
        self.assertEqual((value["format_version"], value["scope"], value["url"], value["method"], value["status"]), (1, "manual_single_url_http_baseline", url, "GET", status))
        self.assertIs(type(value["status"]), int)
        self.assertEqual(value["http_version"], version)
        self.assertEqual(value["transport"], "https" if tls == "default_verified" else "http")
        self.assertEqual(value["tls_verification"], tls)
        self.assertEqual((value["body_file"], value["body_bytes"], value["body_sha256"]), ("body.bin", len(body), hashlib.sha256(body).hexdigest()))
        self.assertEqual(datetime.fromisoformat(value["recorded_at_utc"].replace("Z", "+00:00")).utcoffset(), timedelta(0))
        self.assertLessEqual(set(value["headers"]), SELECTED)
        self.assertTrue(all(isinstance(values, list) and values and all(isinstance(item, str) for item in values) for values in value["headers"].values()))
        return value

    def test_binary_entity_selected_duplicate_headers_and_proxy_credentials_are_not_used(self):
        body = gzip.compress(b"\0CANARY_BODY\xff <script>literal</script>", mtime=0)
        target = "/binary?x=%2f&x=2"
        headers = [("Content-Type", "application/octet-stream"), ("Content-Type", "application/canary"),
                   ("Content-Encoding", "gzip"), ("Cache-Control", "private"), ("ETag", '"canary"'),
                   ("Last-Modified", "Wed, 01 Jan 2020 00:00:00 GMT"), ("Vary", "Accept"), ("Vary", "X-Canary"),
                   ("Set-Cookie", "CANARY_COOKIE=private"), ("Authorization", "CANARY_AUTH"), ("X-Private", "CANARY_HEADER")]
        server = self.server({target: response(body, headers=headers)})
        proxy = self.server({target: response(b"PROXY_USED")})
        environment = os.environ.copy()
        for key in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"):
            environment[key] = proxy.url("")
        environment["NO_PROXY"] = environment["no_proxy"] = ""
        value = self.recorded(server.url(target), self.work / "binary", body, environment=environment)
        self.assertEqual(value["request_target"], target)
        self.assertEqual(value["headers"]["content-type"], ["application/octet-stream", "application/canary"])
        self.assertEqual(value["headers"]["vary"], ["Accept", "X-Canary"])
        self.assertEqual(set(value["headers"]), SELECTED - {"location"})
        self.assertEqual(proxy.observed, [])
        self.assertEqual(len(server.observed), 1)
        self.assertNotIn("cookie", server.observed[0][1])
        self.assertNotIn("authorization", server.observed[0][1])

    def test_empty_and_error_statuses_are_data_and_empty_query_target_is_preserved(self):
        for status, body in [(200, b""), (204, b""), (304, b""), (404, b"CANARY_NOT_FOUND"), (503, b"CANARY_UNAVAILABLE")]:
            target = "/status?"
            server = self.server({target: response(body, status)})
            value = self.recorded(server.url(target), self.work / str(status), body, status)
            self.assertEqual(value["request_target"], target)
            self.assertEqual(server.observed[0][0], target)
        for status in (204, 304):
            # These statuses are bodyless even with a chunked header. There is
            # deliberately no terminal chunk in this header-only response.
            server = self.server({"/bodyless-te": response(b"", status, [("Transfer-Encoding", "chunked")], framing="close")})
            self.recorded(server.url("/bodyless-te"), self.work / (str(status) + "-te"), b"", status)
        older = self.server({"/http10": response(b"CANARY_HTTP10").replace(b"HTTP/1.1", b"HTTP/1.0", 1)})
        self.recorded(older.url("/http10"), self.work / "http10", b"CANARY_HTTP10", version="HTTP/1.0")

    def test_redirect_is_recorded_once_without_following_location(self):
        server = self.server({"/start": response(b"redirect entity", 302, [("Location", "/CANARY_DESTINATION")]),
                              "/CANARY_DESTINATION": response(b"SHOULD_NOT_FETCH")})
        value = self.recorded(server.url("/start"), self.work / "redirect", b"redirect entity", 302)
        self.assertEqual(value["headers"]["location"], ["/CANARY_DESTINATION"])
        self.assertEqual([item[0] for item in server.observed], ["/start"])

    def test_exact_and_excess_body_caps_with_length_chunked_and_close_delimited(self):
        body = b"\0\xffAB" * (BODY_LIMIT // 4)
        for framing in ("length", "chunked", "close"):
            server = self.server({"/exact": response(body, framing=framing), "/excess": response(body + b"X", framing=framing)})
            with self.subTest(framing=framing):
                self.recorded(server.url("/exact"), self.work / (framing + "-exact"), body)
                destination = self.work / (framing + "-excess")
                self.refused(server.url("/excess"), destination)
                self.assertFalse(destination.exists())

    def test_damaged_framing_headers_and_control_budget_never_become_success(self):
        bad = [b"HTTP/1.1 200 OK\r\nX-Canary: unfinished", b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nab",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nContent-Length: 0\r\n\r\n0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Length: 0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nContent-Length: +0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: gzip, chunked\r\n\r\n0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nZ\r\nxx\r\n0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n1\r\nAxx0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\nBad-Trailer\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n0\r\n",
               b"HTTP/1.1 200 OK\r\nX-Canary: " + b"a" * 65536 + b"\r\n\r\n",
               # ~12 KiB control fits its budget; JSON's escaped Latin-1
               # selection independently exceeds the 64 KiB metadata budget.
               b"HTTP/1.1 200 OK\r\nContent-Type: " + b"\xff" * 12000 + b"\r\nContent-Length: 0\r\n\r\n",
               b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n" + b"1\r\nx\r\n" * 22000 + b"0\r\n\r\n",
               b"HTTP/2 200 OK\r\nContent-Length: 0\r\n\r\n"]
        for index, wire in enumerate(bad):
            with self.subTest(case=index):
                server = self.server({"/bad": wire})
                destination = self.work / str(index)
                self.refused(server.url("/bad"), destination)
                self.assertFalse(destination.exists())

    def test_timeout_reaps_the_exact_worker_and_io_threads_before_return(self):
        server = self.server({"/slow": None})
        children = []
        real_popen = subprocess.Popen
        real_run = owned_probe.run_captured_probe
        timeouts = []
        first_clock = True

        def after_get_clock():
            nonlocal first_clock
            if first_clock:
                first_clock = False
                # Isolate the blocked-GET timeout from interpreter startup.
                # The real capture has already started its pipe workers here;
                # all later deadline/cleanup clocks remain actual monotonic time.
                self.assertTrue(server.entered.wait(5), "owned worker did not reach GET before the fixture deadline")
            return real_time.monotonic()

        def observe_timeout(*args, **kwargs):
            try:
                return real_run(*args, **kwargs)
            except subprocess.TimeoutExpired as error:
                timeouts.append(error.timeout)
                raise

        def spawn(*args, **kwargs):
            child = real_popen(*args, **kwargs)
            children.append(child)
            return child

        destination = self.work / "timeout"
        try:
            clock = SimpleNamespace(monotonic=after_get_clock, sleep=real_time.sleep)
            # Replace only this module's binding, never global time.monotonic.
            with patch.object(owned_probe, "time", clock), patch.object(owned_probe, "run_captured_probe", side_effect=observe_timeout), patch("owned_probe.subprocess.Popen", side_effect=spawn):
                with self.assertRaises(http_baseline.HttpBaselineError) as caught:
                    http_baseline.capture_http_baseline(server.url("/slow"), destination, timeout=0.5)
            self.assertEqual(str(caught.exception), "HTTP baseline capture failed")
            self.assertEqual(timeouts, [0.5], "the refusal must be the actual capture deadline")
            self.assertTrue(server.entered.is_set())
            self.assertEqual(len(children), 1)
            for child in children:
                self.assertIsNotNone(child.poll())
                self.assertTrue(child.stdout.closed and child.stderr.closed and child.stdin.closed)
                self.assertFalse(any(thread.name.startswith("owned-probe-") and thread.name.endswith("-" + str(child.pid)) for thread in threading.enumerate()))
            self.assertFalse(destination.exists())
        finally:
            server.release.set()

    def test_invalid_urls_and_arguments_are_opaque_and_start_no_network(self):
        server = self.server({"/": response(b"CANARY")})
        destination = self.work / "SECRET_destination"
        for url in ("SECRET", "file:///SECRET", server.url("/#SECRET"), "http://user:SECRET@127.0.0.1/", "http://127.0.0.1:0/", "http://127.0.0.1:99999/", "http://127.0.0.1/\r\nSECRET", server.url("/日本"), server.url("/" + "x" * 8192)):
            self.refused(url, destination)
            self.assertFalse(destination.exists())
        for args in ((), (server.url("/"),), (server.url("/"), destination, "extra")):
            self.refused(*args)
        self.assertEqual(server.observed, [])

    def test_existing_files_directories_and_missing_parent_are_preserved_before_network(self):
        server = self.server({"/": response(b"CANARY")})
        existing = self.work / "existing"
        existing.mkdir()
        canary = existing / "body.bin"
        canary.write_bytes(b"PRIVATE_CANARY")
        empty = self.work / "empty"
        empty.mkdir()
        file = self.work / "file"
        file.write_bytes(b"FILE_CANARY")
        for destination in (existing, empty, file, self.work / "missing" / "new"):
            self.refused(server.url("/"), destination)
        self.assertEqual(canary.read_bytes(), b"PRIVATE_CANARY")
        self.assertEqual(file.read_bytes(), b"FILE_CANARY")
        self.assertEqual(list(empty.iterdir()), [])
        self.assertEqual(server.observed, [])

    def test_existing_and_dangling_symlinks_are_not_overwritten_before_network(self):
        server = self.server({"/": response(b"CANARY")})
        target = self.work / "target"
        target.mkdir()
        canary = target / "canary"
        canary.write_bytes(b"LINK_CANARY")
        link = self.work / "link"
        try:
            link.symlink_to(target, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest("symlink creation unavailable: " + type(error).__name__)
        dangling = self.work / "dangling"
        dangling.symlink_to(self.work / "absent")
        for destination in (link, dangling):
            self.refused(server.url("/"), destination)
            self.assertTrue(destination.is_symlink())
        self.assertEqual(canary.read_bytes(), b"LINK_CANARY")
        self.assertEqual(server.observed, [])

    def test_worker_second_write_failure_returns_no_receipt_and_retains_partial_directory(self):
        server = self.server({"/partial": response(b"CANARY_PARTIAL_BODY")})
        destination = self.work / "partial"
        original_write = http_baseline._write_file
        writes = []

        def write(path, data):
            writes.append(path.name)
            if len(writes) == 2:
                raise OSError("SECRET_WRITE_CANARY")
            original_write(path, data)

        request = json.dumps({"url": server.url("/partial"), "new_dir": str(destination)}).encode()
        output = io.BytesIO()
        with patch.object(http_baseline, "_write_file", side_effect=write), patch.object(http_baseline.sys, "stdin", SimpleNamespace(buffer=io.BytesIO(request))), patch.object(http_baseline.sys, "stdout", SimpleNamespace(buffer=output)):
            self.assertEqual(http_baseline._worker_main(), 2)
        self.assertEqual(output.getvalue(), b"")
        self.assertEqual(writes, ["body.bin", "capture.json"])
        self.assertEqual({path.name for path in destination.iterdir()}, {"body.bin"})
        self.assertEqual((destination / "body.bin").read_bytes(), b"CANARY_PARTIAL_BODY")
        self.refused(server.url("/partial"), destination)
        self.assertEqual(len(server.observed), 1)

    def test_parent_rejects_tampered_worker_receipts_and_retries_cleanup_without_network(self):
        pid = 4567
        receipt = {"format_version": 1, "process_pid": pid, "body_bytes": 0, "body_sha256": hashlib.sha256(b"").hexdigest()}
        wire = (json.dumps(receipt) + "\n").encode()
        duplicate = wire.replace(b'"process_pid": 4567', b'"process_pid": 4567, "process_pid": 4567')
        for code, stdout, stderr in [
            (0, (json.dumps({**receipt, "process_pid": pid + 1}) + "\n").encode(), b""),
            (0, duplicate, b""),
            (0, (json.dumps({**receipt, "body_bytes": False}) + "\n").encode(), b""),
            (2, wire, b""),
            (0, wire, b"SECRET_WORKER_DIAGNOSTIC"),
        ]:
            with self.subTest(exit=code, stderr=bool(stderr)):
                result = subprocess.CompletedProcess(["mock-owned-worker"], code, stdout, stderr)
                with patch.object(owned_probe, "run_captured_probe", return_value=(pid, result)) as run, patch.object(owned_probe, "reap_owned_probes") as reap:
                    with self.assertRaises(http_baseline.HttpBaselineError) as caught:
                        http_baseline.capture_http_baseline("http://127.0.0.1:9/SECRET_URL", self.work / "SECRET_unused")
                    self.assertEqual(str(caught.exception), "HTTP baseline capture failed")
                    self.assertNotIn("SECRET", repr(caught.exception))
                    run.assert_called_once()
                    reap.assert_called_once()
                self.assertFalse((self.work / "SECRET_unused").exists())

    def test_https_verified_success_untrusted_or_wrong_name_refusal_and_no_keylog(self):
        from http_baseline_test_tls import available_openssl, generate_loopback_tls
        if available_openssl() is None:
            self.skipTest("preinstalled OpenSSL unavailable; real TLS not exercised")
        tls_directory = self.work / "tls"
        tls_directory.mkdir()
        ca, certificate, key = generate_loopback_tls(tls_directory)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(certificate, key)
        server = self.server({"/tls": response(b"TLS_CANARY"), "/abrupt": response(b"TLS_ABRUPT_CANARY", framing="close")}, context)
        environment = os.environ.copy()
        environment["SSL_CERT_FILE"] = str(ca)
        keylog = self.work / "SECRET_keylog"
        environment["SSLKEYLOGFILE"] = str(keylog)
        self.recorded(server.url("/tls", hostname="localhost"), self.work / "verified", b"TLS_CANARY", environment=environment, tls="default_verified")
        self.assertFalse(keylog.exists())
        # An unrelated generated CA is not a trust anchor for this server.
        other = self.work / "other-tls"
        other.mkdir()
        other_ca, _, _ = generate_loopback_tls(other)
        unknown = {**environment, "SSL_CERT_FILE": str(other_ca)}
        self.refused(server.url("/tls", hostname="localhost"), self.work / "unknown", environment=unknown)
        self.refused(server.url("/tls", hostname="127.0.0.1"), self.work / "wrong-name", environment=environment)
        self.assertEqual(len(server.observed), 1)
        self.assertFalse(keylog.exists())
        # Socket close without TLS unwrap/close_notify cannot complete a
        # close-delimited TLS body; completed Content-Length above needs no EOF.
        abrupt = self.work / "abrupt-tls"
        self.refused(server.url("/abrupt", hostname="localhost"), abrupt, environment=environment)
        self.assertFalse(abrupt.exists())
        self.assertFalse(keylog.exists())


if __name__ == "__main__":
    unittest.main()
