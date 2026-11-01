import dataclasses
import os
import re
import subprocess
import sys
import threading
import time
import unittest
from unittest.mock import Mock, patch

import owned_probe


class OwnedProbeTests(unittest.TestCase):
    def test_success_output_requires_one_raw_line_zero_exit_and_empty_stderr(self):
        def completed(code=0, stdout=b"RESULT={}\n", stderr=b""):
            return subprocess.CompletedProcess(["owned-fixture"], code, stdout, stderr)
        self.assertEqual(owned_probe.require_success_output(completed(), "fixture"), "RESULT={}\n")
        for result in [completed(code=1), completed(code=False), completed(stderr=b"diagnostic"),
                       completed(stdout=b"RESULT={}\n \n"), completed(stdout=b"RESULT={}"),
                       completed(stdout=b"\xff\n"), completed(stdout=b"x"*65537+b"\n")]:
            with self.subTest(code=result.returncode, size=len(result.stdout)), self.assertRaises(ValueError):
                owned_probe.require_success_output(result, "fixture")

    def setUp(self):
        self.children = []
        self.real_popen = subprocess.Popen
        self.patcher = patch("owned_probe.subprocess.Popen", side_effect=self.spawn)
        self.patcher.start()

    def spawn(self, *args, **kwargs):
        expected_flags = subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0
        self.assertEqual(kwargs["creationflags"], expected_flags)
        for key in kwargs["env"]:
            self.assertNotIn(key.upper(), owned_probe._MYSQL_ENV)
        child = self.real_popen(*args, **kwargs)
        self.children.append(child)
        return child

    def tearDown(self):
        self.patcher.stop()
        for child in self.children:
            self.assertIsNotNone(child.poll())
            self.assertTrue(child.stdout.closed)
            self.assertTrue(child.stderr.closed)
        self.assertFalse(any(thread.name.startswith("owned-probe-ready-") for thread in threading.enumerate()))

    @staticmethod
    def parse_ready(wire, *, expected_pid):
        match = re.fullmatch(r"READY=([1-9][0-9]*)\r?\n", wire)
        if not match or int(match[1]) != expected_pid:
            raise ValueError("READY marker or owned PID mismatch")
        return {"process_pid": expected_pid}

    def run_fixture(self, source, parser=None, timeout=5):
        environment = dict(os.environ)
        environment.update({key: "must-remove" for key in owned_probe._MYSQL_ENV})
        environment["mysql_pwd"] = "also-remove-case-variant"
        environment["RECASTPRESS_CHILD_TEST"] = "kept"
        before = environment.copy()
        try:
            return owned_probe.run_until_killed([sys.executable, "-u", "-c", source], environment,
                                                parser or self.parse_ready, timeout)
        finally:
            self.assertEqual(environment, before)

    def test_owned_ready_pid_is_validated_then_pending_child_is_killed_and_reaped(self):
        result = self.run_fixture("import os,time; assert os.environ['RECASTPRESS_CHILD_TEST']=='kept'; print('READY='+str(os.getpid()),flush=True); time.sleep(30)")
        self.assertEqual(result.process_pid, self.children[-1].pid)
        self.assertEqual(result.ready, {"process_pid": result.process_pid})
        self.assertNotEqual(result.exit_code, 0)
        self.assertEqual(self.parse_ready(result.ready_line, expected_pid=result.process_pid), result.ready)
        with self.assertRaises(dataclasses.FrozenInstanceError):
            result.exit_code = 0

    def test_silent_or_stderr_blocked_child_times_out_and_is_killed_without_reader_leak(self):
        for source in ["import time; time.sleep(30)",
                       "import os,time; os.write(2,b'x'*2000000); time.sleep(30)"]:
            with self.subTest(source=source), self.assertRaises(TimeoutError):
                self.run_fixture(source, timeout=0.2)

    def test_bad_marker_wrong_pid_invalid_utf8_and_callback_baseexception_are_rejected(self):
        for source in ["import time; print('wrong',flush=True); time.sleep(30)",
                       "import time; print('READY=1',flush=True); time.sleep(30)",
                       "import os,time; os.write(1,b'\\xff\\n'); time.sleep(30)"]:
            with self.subTest(source=source), self.assertRaises((ValueError, UnicodeError)):
                self.run_fixture(source)
        def interrupted(wire, *, expected_pid):
            self.parse_ready(wire, expected_pid=expected_pid)
            raise KeyboardInterrupt("fixture interruption")
        with self.assertRaises(KeyboardInterrupt):
            self.run_fixture("import os,time; print('READY='+str(os.getpid()),flush=True); time.sleep(30)", interrupted)

    def test_normal_exit_or_missing_newline_cannot_be_accepted_as_kill_success(self):
        def after_exit(wire, *, expected_pid):
            ready = self.parse_ready(wire, expected_pid=expected_pid)
            time.sleep(0.1)
            return ready
        for exit_code in [0, 17]:
            with self.subTest(exit_code=exit_code), self.assertRaisesRegex(ValueError, "exited before"):
                self.run_fixture(f"import os,sys; print('READY='+str(os.getpid()),flush=True); sys.exit({exit_code})", after_exit)
        with self.assertRaises(TimeoutError):
            self.run_fixture("import os,time; print('READY='+str(os.getpid()),end='',flush=True); time.sleep(30)", timeout=0.2)

    def test_oversized_line_and_extra_stdout_or_stderr_are_rejected(self):
        for source, message in [
            ("import os,time; os.write(1,b'x'*65537); time.sleep(30)", "exceeds"),
            ("import os,time; os.write(1,('READY='+str(os.getpid())+'\\nEXTRA\\n').encode()); time.sleep(30)", "extra stdout"),
            ("import os,time; os.write(2,b'failure'); print('READY='+str(os.getpid()),flush=True); time.sleep(30)", "stderr"),
        ]:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                self.run_fixture(source)


    def test_child_kill_failure_returns_original_error_and_retains_handle_for_outer_retry(self):
        original = None
        try:
            with patch.object(owned_probe, "CLEANUP_TIMEOUT", 0.05), patch.object(
                    self.real_popen, "kill", side_effect=OSError("injected child kill denied")):
                started = time.monotonic()
                with self.assertRaisesRegex(ValueError, "marker") as raised:
                    self.run_fixture("import time; print('wrong',flush=True); time.sleep(30)")
                original = raised.exception
                self.assertLess(time.monotonic()-started, 2)
                child = self.children[-1]
                self.assertIsNone(child.poll())
                self.assertIs(owned_probe._OWNED_CHILDREN[id(child)], child)
                self.assertTrue(any("retained" in note for note in original.__notes__))
                self.assertTrue(child.stdout.closed)
                self.assertTrue(child.stderr.closed)
                self.assertFalse(any(thread.name.startswith("owned-probe-") for thread in threading.enumerate()))
            # A later outer cleanup retry uses the exact captured handle.
            owned_probe.reap_owned_probes(timeout=2)
            self.assertNotIn(id(child), owned_probe._OWNED_CHILDREN)
            self.assertIsNotNone(child.returncode)
        finally:
            for child in self.children:
                if child.poll() is None:
                    child.kill()
                    child.wait(timeout=2)
            owned_probe.reap_owned_probes(timeout=2)

    def test_fake_cleanup_failure_still_stops_io_closes_pipes_and_tries_other_handles(self):
        failed = Mock(pid=123)
        failed.poll.return_value = None
        failed.kill.side_effect = OSError("denied")
        failed.wait.side_effect = subprocess.TimeoutExpired(["owned"], 0.01)
        done = Mock(pid=124)
        done.poll.return_value = 0
        old = dict(owned_probe._OWNED_CHILDREN)
        try:
            owned_probe._OWNED_CHILDREN.clear()
            owned_probe._remember(failed)
            owned_probe._remember(done)
            stop = Mock()
            with patch.object(owned_probe, "CLEANUP_TIMEOUT", 0.01):
                with self.assertRaisesRegex(ValueError, "original") as raised:
                    original = ValueError("original")
                    try:
                        raise original
                    finally:
                        owned_probe._cleanup_child(failed, stop, original)
            stop.assert_called_once_with()
            failed.stdout.close.assert_called_once_with()
            failed.stderr.close.assert_called_once_with()
            self.assertTrue(raised.exception.__notes__)
            try:
                raise ValueError("unrelated caller exception")
            except ValueError as ambient:
                with self.assertRaisesRegex(RuntimeError, "retained"):
                    owned_probe._cleanup_child(failed, Mock())
                self.assertFalse(hasattr(ambient, "__notes__"))
            with self.assertRaisesRegex(RuntimeError, "retained"):
                owned_probe.reap_owned_probes(timeout=0.01)
            done.wait.assert_called_once_with(timeout=0.01)
            self.assertNotIn(id(done), owned_probe._OWNED_CHILDREN)
            self.assertIn(id(failed), owned_probe._OWNED_CHILDREN)
        finally:
            owned_probe._OWNED_CHILDREN.clear()
            owned_probe._OWNED_CHILDREN.update(old)


if __name__ == "__main__":
    unittest.main()
