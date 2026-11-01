import os
import subprocess
import sys
import threading
import time

import owned_probe
import unittest
from unittest.mock import patch

from database_smoke import main, run_owned_probe


class DatabaseProcessTests(unittest.TestCase):
    def setUp(self):
        self.children = []
        self.real_popen = subprocess.Popen
        self.patcher = patch("database_smoke.subprocess.Popen", side_effect=self.spawn)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)
        self.addCleanup(self.reap_children)

    def spawn(self, *args, **kwargs):
        expected_flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
        self.assertEqual(kwargs["creationflags"], expected_flags)
        child = self.real_popen(*args, **kwargs)
        self.children.append(child)
        return child

    def reap_children(self):
        # Keep a broken harness/test assertion from leaving this test's process
        # alive. The test checks the harness's returncode BEFORE this fallback.
        for child in self.children:
            if child.poll() is None:
                child.kill()
                child.communicate(timeout=2)

    def test_owned_python_reports_the_actual_spawned_pid_and_exits(self):
        command = [sys.executable, "-c", "import os; print(os.getpid())"]
        pid, result = run_owned_probe(command, os.environ.copy(), timeout=2)
        self.assertEqual(len(self.children), 1)
        child = self.children[0]
        self.assertEqual(pid, child.pid)
        self.assertEqual(int(result.stdout.strip()), child.pid)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(result.returncode, 0)
        self.assertEqual(child.returncode, 0)
        self.assertEqual(child.poll(), 0)

    def test_overlap_refuses_other_profiles_before_starting_any_process(self):
        with patch("database_smoke.subprocess.run") as run:
            for backend, cleanup, ordinary in (("mariadb", False, False),
                                               ("mysql", True, False),
                                               ("mysql", False, True)):
                with self.subTest(backend=backend, cleanup=cleanup, ordinary=ordinary):
                    with self.assertRaisesRegex(ValueError, "own MySQL fixture profile"):
                        main(False, backend, cleanup, ordinary, True)
            run.assert_not_called()
        self.assertEqual(self.children, [])

    def test_timeout_propagates_after_killing_and_reaping_owned_python(self):
        command = [sys.executable, "-c", "import time; time.sleep(2)"]
        with self.assertRaises(subprocess.TimeoutExpired) as raised:
            run_owned_probe(command, os.environ.copy(), timeout=0.1)
        self.assertEqual(raised.exception.cmd, command)
        self.assertEqual(len(self.children), 1)
        child = self.children[0]
        # The harness has already waited for termination, without relying on
        # this test's poll() or fallback cleanup to perform the reaping.
        self.assertIsNotNone(child.returncode)
        self.assertNotEqual(child.returncode, 0)
        self.assertEqual(child.poll(), child.returncode)


    def test_binary_stdin_and_large_outputs_are_drained_without_communicate_threads(self):
        data = bytes(range(256))*1024
        command = [sys.executable, "-c", "import sys; data=sys.stdin.buffer.read(); sys.stdout.buffer.write(data); sys.stderr.buffer.write(data[::-1])"]
        pid, result = run_owned_probe(command, os.environ.copy(), timeout=5, input_bytes=data)
        self.assertEqual(pid, self.children[-1].pid)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(result.stdout, data)
        self.assertEqual(result.stderr, data[::-1])
        self.assertFalse(any(thread.name.startswith("owned-probe-") for thread in threading.enumerate()))
        self.assertTrue(self.children[-1].stdin.closed)
        # A child stuck before reading stdin must not block the writer or unwind.
        with self.assertRaises(subprocess.TimeoutExpired):
            run_owned_probe([sys.executable, "-c", "import time; time.sleep(30)"],
                            os.environ.copy(), timeout=0.1, input_bytes=data)
        self.assertIsNotNone(self.children[-1].returncode)
        self.assertTrue(self.children[-1].stdin.closed)
        self.assertFalse(any(thread.name.startswith("owned-probe-") for thread in threading.enumerate()))

    def test_capture_timeout_kill_failure_is_bounded_and_retains_actual_child(self):
        child = None
        try:
            with patch.object(owned_probe, "CLEANUP_TIMEOUT", 0.05), patch.object(
                    self.real_popen, "kill", side_effect=OSError("injected kill denied")):
                started = time.monotonic()
                with self.assertRaises(subprocess.TimeoutExpired) as raised:
                    run_owned_probe([sys.executable, "-c", "import time; time.sleep(30)"], os.environ.copy(), timeout=0.05)
                self.assertLess(time.monotonic()-started, 2)
                child = self.children[-1]
                self.assertIsNone(child.poll())
                self.assertIs(owned_probe._OWNED_CHILDREN[id(child)], child)
                self.assertTrue(raised.exception.__notes__)
                self.assertTrue(child.stdout.closed)
                self.assertTrue(child.stderr.closed)
                self.assertFalse(any(thread.name.startswith("owned-probe-") for thread in threading.enumerate()))
            owned_probe.reap_owned_probes(timeout=2)
            self.assertIsNotNone(child.returncode)
            self.assertNotIn(id(child), owned_probe._OWNED_CHILDREN)
        finally:
            if child is not None and child.poll() is None:
                child.kill()
                child.wait(timeout=2)
            owned_probe.reap_owned_probes(timeout=2)

    def test_capture_read_failure_and_keyboard_interrupt_reap_before_unwinding(self):
        def bad_read(stream, output, cancelled, errors):
            errors.append(OSError("injected read failure"))
        with patch.object(owned_probe, "_read_output", side_effect=bad_read):
            with self.assertRaisesRegex(OSError, "injected read failure"):
                run_owned_probe([sys.executable, "-c", "import time; time.sleep(30)"], os.environ.copy(), timeout=2)
        self.assertIsNotNone(self.children[-1].returncode)
        # Simulate pipe EOF retained elsewhere without spawning an untracked
        # descendant: the cancellable worker waits until cleanup cancels it.
        def delayed_eof(stream, output, cancelled, errors):
            cancelled.wait(2)
        with patch.object(owned_probe, "_read_output", side_effect=delayed_eof), patch.object(
                owned_probe, "CLEANUP_TIMEOUT", 0.05):
            started = time.monotonic()
            with self.assertRaisesRegex(TimeoutError, "IO worker"):
                run_owned_probe([sys.executable, "-c", "print('done')"], os.environ.copy(), timeout=2)
            self.assertLess(time.monotonic()-started, 2)
        self.assertIsNotNone(self.children[-1].returncode)
        self.assertFalse(any(thread.name.startswith("owned-probe-") for thread in threading.enumerate()))
        real_wait = self.real_popen.wait
        interrupted = False
        def wait(process, *args, **kwargs):
            nonlocal interrupted
            if not interrupted:
                interrupted = True
                raise KeyboardInterrupt("injected interruption")
            return real_wait(process, *args, **kwargs)
        with patch.object(self.real_popen, "wait", wait):
            with self.assertRaisesRegex(KeyboardInterrupt, "injected interruption"):
                run_owned_probe([sys.executable, "-c", "import time; time.sleep(30)"], os.environ.copy(), timeout=2)
        self.assertIsNotNone(self.children[-1].returncode)
        self.assertFalse(owned_probe._OWNED_CHILDREN)
        self.assertFalse(any(thread.name.startswith("owned-probe-") for thread in threading.enumerate()))
        real_cleanup = owned_probe._kill_and_wait
        def cleanup_failure(child, timeout):
            return real_cleanup(child, timeout) + ["injected post-success cleanup failure"]
        try:
            raise ValueError("unrelated caller exception")
        except ValueError as ambient:
            with patch.object(owned_probe, "_kill_and_wait", side_effect=cleanup_failure):
                with self.assertRaisesRegex(RuntimeError, "post-success cleanup failure"):
                    run_owned_probe([sys.executable, "-c", "print('done')"], os.environ.copy(), timeout=2)
            self.assertFalse(hasattr(ambient, "__notes__"))
        self.assertFalse(owned_probe._OWNED_CHILDREN)


if __name__ == "__main__":
    unittest.main()
