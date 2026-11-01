import tempfile
from pathlib import Path
import unittest
from unittest.mock import Mock

from mysql_server_restart import reap_owned_servers, validate_restart_command


class RestartLifecycleTests(unittest.TestCase):
    def test_only_exact_common_command_and_config_without_init_file_is_accepted(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).resolve().parents[1]/"target") as directory:
            work = Path(directory)
            config = work/"server.ini"
            config.write_text("[mysqld]\nport=1234\n", encoding="utf-8")
            command = ["mysqld.exe", f"--defaults-file={config}", "--no-monitor", "--console"]
            validate_restart_command(command, work)
            for bad in [command+["--initialize-insecure"], command+["--init-file=x"],
                        ["other.exe", *command[1:]], [*command[:2], "--console"]]:
                with self.subTest(command=bad), self.assertRaises(ValueError):
                    validate_restart_command(bad, work)
            for directive in ["init-file=x", "  INIT_FILE =x"]:
                config.write_text("[mysqld]\n"+directive+"\n", encoding="utf-8")
                with self.subTest(directive=directive), self.assertRaises(ValueError):
                    validate_restart_command(command, work)

    def test_cleanup_attempts_every_handle_even_when_one_fails(self):
        failed = Mock(pid=101)
        failed.poll.return_value = None
        failed.kill.side_effect = OSError("cannot kill")
        alive = Mock(pid=102)
        alive.poll.return_value = None
        exited = Mock(pid=103)
        exited.poll.return_value = 1
        with self.assertRaisesRegex(RuntimeError, "101"):
            reap_owned_servers([exited, alive, failed])
        alive.kill.assert_called_once_with()
        alive.wait.assert_called_once_with(timeout=10)
        failed.wait.assert_called_once_with(timeout=10)
        exited.kill.assert_not_called()
        exited.wait.assert_called_once_with(timeout=10)


if __name__ == "__main__":
    unittest.main()
