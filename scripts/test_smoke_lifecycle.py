"""Exercise captured HTTP process cleanup with real, locally owned children."""

import http.client
import os
import subprocess
import sys
import threading
import unittest
from unittest.mock import patch

import smoke


REAL_POPEN = subprocess.Popen
READY = "import sys,time; print('listening on 127.0.0.1:12345',file=sys.stderr,flush=True); time.sleep(30)"
SILENT = "import time; time.sleep(30)"
SOCKET = """
import http.server,sys
class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'owned fixture')
    def log_message(self,*args): pass
server=http.server.HTTPServer(('127.0.0.1',0),Handler)
print('listening on 127.0.0.1:'+str(server.server_port),file=sys.stderr,flush=True)
server.serve_forever()
"""


class LifecycleTests(unittest.TestCase):
    def setUp(self):
        self.assertFalse(smoke._OWNED_SERVERS)
        self.children = []
        self.readers = []
        self.timeouts = patch.multiple(smoke, STARTUP_TIMEOUT=3, STOP_TIMEOUT=0.1, READER_TIMEOUT=0.2)
        self.timeouts.start()
        self.addCleanup(self.timeouts.stop)
        self.addCleanup(self.cleanup_children)

    def cleanup_children(self):
        # Only these exact captured handles are used, including on assertion failures.
        for child in self.children:
            if child.poll() is None:
                child.kill()
            child.wait(timeout=5)
        smoke.reap_servers()
        self.assertFalse(smoke._OWNED_SERVERS)
        for reader in self.readers:
            self.assertFalse(reader.is_alive())
        owned_names = {f'smoke-server-reader-{child.pid}' for child in self.children}
        self.assertFalse([reader.name for reader in threading.enumerate()
                          if reader.name in owned_names])

    def spawn(self, source=READY, *, environment=None, binary=None):
        def factory(argv, **kwargs):
            self.assertEqual(argv[:4], [str(binary or smoke.BINARY), 'public', '--listen', '127.0.0.1:0'])
            self.assertEqual(kwargs['env'], environment)
            self.assertEqual(kwargs['creationflags'], subprocess.CREATE_NO_WINDOW if os.name == 'nt' else 0)
            child = REAL_POPEN([sys.executable, '-u', '-c', source], **kwargs)
            self.children.append(child)
            return child
        with patch.object(smoke.subprocess, 'Popen', side_effect=factory):
            server = smoke.start('public', environment=environment, binary=binary)
        self.readers.append(server.reader)
        return server

    def assert_reaped(self, server):
        self.assertIsNotNone(server.process.poll())
        self.assertFalse(server.reader.is_alive())
        self.assertTrue(server.process.stderr.closed)
        self.assertNotIn(id(server.process), smoke._OWNED_SERVERS)

    def test_normal_socket_stop_and_environment(self):
        environment = dict(os.environ, OWNED_FIXTURE='yes')
        server = self.spawn(SOCKET, environment=environment, binary='explicit-fixture-binary')
        connection = http.client.HTTPConnection('127.0.0.1', server.port, timeout=3)
        try:
            connection.request('GET', '/')
            response = connection.getresponse()
            self.assertEqual(response.status, 200)
            self.assertEqual(response.read(), b'owned fixture')
        finally:
            connection.close()
        smoke.stop(server)
        self.assert_reaped(server)
        smoke.reap_servers()

    def test_terminate_failure_still_kills_without_ambient_error_swallowing(self):
        server = self.spawn()
        with patch.object(server.process, 'terminate', side_effect=OSError('terminate denied')):
            try:
                raise ValueError('unrelated ambient exception')
            except ValueError:
                with self.assertRaisesRegex(RuntimeError, 'terminate denied'):
                    smoke.stop(server)
        self.assert_reaped(server)

    def test_denied_kill_retains_pipe_and_registry_then_retry_continues_all(self):
        good = self.spawn()
        bad = self.spawn()
        with patch.object(bad.process, 'terminate', side_effect=OSError('terminate denied')), \
                patch.object(bad.process, 'kill', side_effect=OSError('kill denied')):
            with self.assertRaisesRegex(RuntimeError, 'captured handle retained'):
                smoke.reap_servers()
            self.assertIsNone(bad.process.poll())
            self.assertFalse(bad.process.stderr.closed)
            self.assertTrue(bad.reader.is_alive())
            self.assertIn(id(bad.process), smoke._OWNED_SERVERS)
            self.assert_reaped(good)
        smoke.reap_servers()
        self.assert_reaped(bad)

    def test_reader_start_failure_reaps_before_reraising(self):
        with patch.object(smoke.threading.Thread, 'start', side_effect=RuntimeError('reader start denied')):
            with self.assertRaisesRegex(RuntimeError, 'reader start denied'):
                self.spawn()
        child = self.children[-1]
        self.assertIsNotNone(child.poll())
        self.assertTrue(child.stderr.closed)
        self.assertFalse(smoke._OWNED_SERVERS)

    def test_startup_timeout_reaps_reader_and_process(self):
        with patch.object(smoke, 'STARTUP_TIMEOUT', 0.05):
            with self.assertRaises(TimeoutError):
                self.spawn(SILENT)
        child = self.children[-1]
        self.assertIsNotNone(child.poll())
        self.assertTrue(child.stderr.closed)
        self.assertFalse(smoke._OWNED_SERVERS)

    def test_reader_decode_failure_keeps_original_error_and_reaps(self):
        source = "import sys,time; sys.stderr.buffer.write(b'\\xff\\n'); sys.stderr.buffer.flush(); time.sleep(30)"
        with self.assertRaisesRegex(RuntimeError, 'stderr reader failed') as caught:
            self.spawn(source)
        self.assertIsInstance(caught.exception.__cause__, UnicodeDecodeError)
        self.assertTrue(any('stderr reader failed' in note for note in caught.exception.__notes__))
        self.assertIsNotNone(self.children[-1].poll())
        self.assertTrue(self.children[-1].stderr.closed)
        self.assertFalse(smoke._OWNED_SERVERS)

    def test_main_preserves_body_error_and_retries_every_registered_child(self):
        servers = []
        def body():
            servers.extend([self.spawn(), self.spawn()])
            raise ValueError('original body failure')
        with patch.object(smoke, '_smoke_main', side_effect=body):
            with self.assertRaisesRegex(ValueError, 'original body failure'):
                smoke.main()
        for server in servers:
            self.assert_reaped(server)

    def test_original_error_has_cleanup_note_and_retained_handle_can_retry(self):
        server = self.spawn()
        original = ValueError('original body failure')
        with patch.object(server.process, 'terminate', side_effect=OSError('terminate denied')), \
                patch.object(server.process, 'kill', side_effect=OSError('kill denied')):
            smoke.stop(server, original=original)
            self.assertIn('captured handle retained', original.__notes__[0])
            self.assertIsNone(server.process.poll())
            self.assertFalse(server.process.stderr.closed)
        smoke.reap_servers()
        self.assert_reaped(server)


if __name__ == '__main__':
    unittest.main()
