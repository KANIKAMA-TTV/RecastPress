"""Real owned Python processes exercise ordering without starting a database."""
import os
import subprocess
import sys
import unittest
from unittest.mock import patch

from owned_probe import run_until_server_killed


PENDING = "import os,time; print('READY='+str(os.getpid()),flush=True); time.sleep(30)"


class OwnedServerCrashTests(unittest.TestCase):
    def setUp(self):
        self.server = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"],
            stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)

    def tearDown(self):
        if self.server.poll() is None:
            self.server.kill()
        self.server.wait(timeout=5)

    @staticmethod
    def parse(wire, *, expected_pid):
        if wire.strip() != f"READY={expected_pid}":
            raise ValueError("invalid marker")
        return {"process_pid": expected_pid}

    def run_probe(self, guard=lambda ready: None, source=PENDING, timeout=5):
        return run_until_server_killed([sys.executable, "-u", "-c", source],
            os.environ.copy(), self.parse, self.server, guard, timeout)

    def test_server_exit_is_observed_before_pending_child_cleanup(self):
        events = []
        real_kill = subprocess.Popen.kill
        real_wait = subprocess.Popen.wait

        def kill(process):
            if process is not self.server:
                self.assertIsNotNone(self.server.poll())
                self.assertIn("server_waited", events)
                events.append("child_kill")
            else:
                events.append("server_kill")
            return real_kill(process)

        def wait(process, *args, **kwargs):
            result = real_wait(process, *args, **kwargs)
            if process is self.server:
                events.append("server_waited")
            return result

        with patch.object(subprocess.Popen, "kill", kill), patch.object(subprocess.Popen, "wait", wait):
            result = self.run_probe(lambda ready: events.append("guard"))
        self.assertEqual(events[:4], ["guard", "server_kill", "server_waited", "child_kill"])
        self.assertEqual(result.server_pid, self.server.pid)
        self.assertNotEqual(result.server_exit_code, 0)
        self.assertNotEqual(result.child.exit_code, 0)

    def test_bad_marker_or_silent_child_never_kills_server(self):
        for source, exception in [("import time; print('bad',flush=True); time.sleep(30)", ValueError),
                                  ("import os,time; os.write(1,('READY='+str(os.getpid())+'\\nEXTRA\\n').encode()); time.sleep(30)", ValueError),
                                  ("import time; time.sleep(30)", TimeoutError)]:
            with self.subTest(source=source), self.assertRaises(exception):
                self.run_probe(source=source, timeout=0.2)
            self.assertIsNone(self.server.poll())

    def test_guard_failure_and_interruption_leave_server_running(self):
        for error in [ValueError("identity"), KeyboardInterrupt("interrupt")]:
            def guard(ready):
                raise error
            with self.subTest(error=type(error)), self.assertRaises(type(error)):
                self.run_probe(guard)
            self.assertIsNone(self.server.poll())

    def test_already_exited_server_is_not_accepted(self):
        self.server.kill()
        self.server.wait(timeout=5)
        with self.assertRaisesRegex(ValueError, "exited before"):
            self.run_probe()

    def test_kill_failure_reaps_child_and_preserves_failure(self):
        with patch.object(self.server, "kill", side_effect=OSError("kill refused")):
            with self.assertRaisesRegex(OSError, "kill refused"):
                self.run_probe()
        self.assertIsNone(self.server.poll())


if __name__ == "__main__":
    unittest.main()
