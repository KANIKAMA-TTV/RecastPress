"""Fresh-binary one-shot memory diagnostics; Linux native sampling is not exercised."""
import json
import os
from pathlib import Path
import subprocess
import unittest

from owned_probe import run_captured_probe

ROOT = Path(__file__).resolve().parents[1]


class MemoryDiagnosticCliTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        built = subprocess.run(["cargo", "build", "-p", "recastpress-server", "--bin", "recastpress", "--offline", "--locked", "--message-format=json"],
            cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=120, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        artifacts = [json.loads(line) for line in built.stdout.splitlines()]
        paths = [item["executable"] for item in artifacts if item.get("reason") == "compiler-artifact"
                 and item.get("target", {}).get("name") == "recastpress"
                 and "bin" in item.get("target", {}).get("kind", []) and item.get("executable")]
        if len(paths) != 1 or not Path(paths[0]).is_file():
            raise AssertionError("Cargo did not identify one current server binary")
        cls.binary = Path(paths[0]).resolve()

    def run_binary(self, args):
        _, result = run_captured_probe([str(self.binary), *args], os.environ.copy(), timeout=5)
        return result

    @unittest.skipUnless(os.name == "nt", "Windows unsupported behavior only; no native Linux sampling")
    def test_windows_is_partial_unsupported_without_zero_or_fake_observed_data(self):
        result = self.run_binary(["inspect-memory"])
        self.assertEqual(result.returncode, 3)
        self.assertEqual(result.stderr, b"")
        self.assertLessEqual(len(result.stdout), 65536)
        self.assertTrue(result.stdout.endswith(b"\n"))
        self.assertEqual(len(result.stdout.splitlines()), 1)
        value = json.loads(result.stdout)
        self.assertEqual(value, {"format_version": 1, "consistency": "non_atomic",
            "host_physical": {"source": "host_meminfo", "status": "unsupported", "kind": "unsupported"},
            "self_rss": {"source": "self_smaps_rollup", "status": "unsupported", "kind": "unsupported"},
            "visible_cgroup_v2": {"source": "self_visible_cgroup_v2", "status": "unsupported", "kind": "unsupported"}})
        self.assertNotIn(b"bytes", result.stdout)
        self.assertNotIn(b"/proc", result.stdout)
        self.assertNotIn(b"C:\\", result.stdout)

    def test_arguments_are_exact_no_manual_knob_and_existing_one_shots_stay_separate(self):
        for args in [["inspect-memory", "extra-SECRET"], ["inspect-memory", "--limit", "123"],
                     ["inspect-memory", "--cgroup", "/SECRET_PATH"], ["inspect-memory", "--listen", "127.0.0.1:8080"],
                     ["inspect-memory", "check-bundle", "SECRET_FILE"]]:
            with self.subTest(args=args):
                result = self.run_binary(args)
                self.assertEqual(result.returncode, 2)
                self.assertEqual(result.stdout, b"")
                self.assertEqual(result.stderr, b"memory inspection failed\n")
        result = self.run_binary(["check-bundle"])
        self.assertEqual(result.returncode, 2)
        self.assertEqual(result.stdout, b"")
        self.assertEqual(result.stderr, b"bundle check failed\n")
        help_result = self.run_binary(["--help"])
        self.assertEqual(help_result.returncode, 0)
        self.assertIn(b"recastpress inspect-memory", help_result.stdout)
        self.assertIn(b"recastpress check-bundle <path>", help_result.stdout)


if __name__ == "__main__":
    unittest.main()
