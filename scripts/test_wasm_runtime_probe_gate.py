"""Strict selection/protocol and owned timeout tests; no Rust build in discovery."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import check_wasm_runtime_probe as gate
import owned_probe


class WasmRuntimeProbeGateTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="wasm-probe-gate-test-", dir=gate.ROOT / "target")
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(owned_probe.reap_owned_probes)
        self.work = Path(self.directory.name)
        self.executable = self.work / "probe.exe"
        self.executable.write_bytes(b"fixed artifact canary")
        self.target = patch.object(gate, "TARGET_DIR", self.work)
        self.target.start()
        self.addCleanup(self.target.stop)

    def message(self):
        return {"reason": "compiler-artifact", "manifest_path": str(gate.MANIFEST),
                "target": {"name": gate.BIN, "kind": ["bin"], "src_path": str(gate.MAIN)},
                "profile": {"test": False}, "executable": str(self.executable)}

    def stream(self, messages=None, finished=True):
        values = [self.message()] if messages is None else messages
        if finished:
            values = [*values, {"reason": "build-finished", "success": True}]
        return b"".join((json.dumps(value) + "\n").encode() for value in values)

    def native(self, **changes):
        value = {"format_version": 1, "profile": "wasmtime_36_0_19_fixed_core_module_fixtures", "process_pid": 7890,
                 "fixture_checks": 27, "literal_output_bytes": 19, "fuel_per_store": 10000, "unpublished_only": True}
        value.update(changes)
        return value

    def completed(self, value=None, code=0, stderr=b""):
        wire = gate.PREFIX + (json.dumps(self.native() if value is None else value) + "\n").encode()
        return subprocess.CompletedProcess(["fixed-probe"], code, wire, stderr)

    def test_current_exact_manifest_binary_is_selected_without_stale_path_guessing(self):
        other = {"reason": "compiler-artifact", "target": {"name": "dependency", "kind": ["lib"]}}
        self.assertEqual(gate.select_artifact(self.stream([other, self.message()])), self.executable.resolve())

    def test_wrong_manifest_source_profile_ambiguity_and_build_failure_are_refused(self):
        bad = []
        for field, value in [("manifest_path", str(self.work / "OTHER_Cargo.toml")), ("executable", "relative.exe")]:
            item = self.message()
            item[field] = value
            bad.append(self.stream([item]))
        for field, value in [("src_path", str(self.work / "OTHER_main.rs")), ("kind", ["example"])]:
            item = self.message()
            item["target"][field] = value
            bad.append(self.stream([item]))
        item = self.message()
        item["profile"]["test"] = True
        bad.append(self.stream([item]))
        bad.extend([self.stream([self.message(), self.message()]), self.stream([], finished=True), self.stream(finished=False),
                    self.stream(finished=False) + b'{"reason":"build-finished","success":false}\n',
                    self.stream() + b'{"reason":"compiler-message"}\n', self.stream()[:-1], b"not JSON\n"])
        for wire in bad:
            with self.subTest(size=len(wire)), self.assertRaises(Exception):
                gate.select_artifact(wire)
        duplicate = self.stream().replace(b'"reason": "compiler-artifact"', b'"reason": "compiler-artifact", "reason": "compiler-artifact"')
        with self.assertRaises(gate.WasmRuntimeProbeGateError):
            gate.select_artifact(duplicate)

    def test_exact_native_summary_requires_actual_pid_strict_types_and_no_unknown_fields(self):
        self.assertEqual(gate.validate_result(7890, self.completed()), self.native())
        for change in ({"process_pid": 7891}, {"process_pid": True}, {"fixture_checks": True}, {"fixture_checks": 25},
                       {"literal_output_bytes": 19.0}, {"unpublished_only": 1}, {"profile": "other"}, {"unknown": "PRIVATE"}):
            with self.subTest(change=change), self.assertRaises(gate.WasmRuntimeProbeGateError):
                gate.validate_result(7890, self.completed(self.native(**change)))
        for result in (self.completed(code=1), self.completed(stderr=b"PRIVATE_DIAGNOSTIC"), self.completed(code=False)):
            with self.assertRaises(gate.WasmRuntimeProbeGateError):
                gate.validate_result(7890, result)
        duplicate = self.completed()
        duplicate.stdout = duplicate.stdout.replace(b'"process_pid": 7890', b'"process_pid": 7890, "process_pid": 7890')
        with self.assertRaises(gate.WasmRuntimeProbeGateError):
            gate.validate_result(7890, duplicate)

    def test_gate_builds_only_fixed_package_then_runs_only_the_selected_artifact(self):
        built = subprocess.CompletedProcess(["cargo"], 0, self.stream(), b"ordinary Cargo progress")
        with patch.object(owned_probe, "run_captured_probe", side_effect=[(6789, built), (7890, self.completed())]) as run, patch.object(owned_probe, "reap_owned_probes") as reap:
            result = gate.run_gate()
            self.assertEqual(result["native"], self.native())
            command = run.call_args_list[0].args[0]
            self.assertEqual(command, ["cargo", "build", "--manifest-path", str(gate.MANIFEST), "--package", gate.BIN, "--bin", gate.BIN,
                                       "--target-dir", str(self.work), "--offline", "--locked", "--message-format=json"])
            self.assertEqual(run.call_args_list[1].args[0], [str(self.executable.resolve())])
            self.assertEqual(run.call_args_list[1].kwargs["timeout"], 20)
            reap.assert_called_once()

    def test_failure_retries_cleanup_without_running_an_ambiguous_or_failed_artifact(self):
        built = subprocess.CompletedProcess(["cargo"], 0, self.stream([self.message(), self.message()]), b"")
        with patch.object(owned_probe, "run_captured_probe", return_value=(6789, built)) as run, patch.object(owned_probe, "reap_owned_probes") as reap:
            with self.assertRaises(gate.WasmRuntimeProbeGateError):
                gate.run_gate()
            run.assert_called_once()
            reap.assert_called_once()
        error = subprocess.TimeoutExpired(["fixed-owned-command"], 0.2)
        with patch.object(owned_probe, "run_captured_probe", side_effect=error), patch.object(owned_probe, "reap_owned_probes", side_effect=RuntimeError("PRIVATE_DETAIL")):
            with self.assertRaises(subprocess.TimeoutExpired) as caught:
                gate.run_gate()
            self.assertEqual(caught.exception.__notes__, ["Wasm probe cleanup retry failed: RuntimeError"])

    def test_real_owned_timeout_reaps_only_its_captured_child_and_closes_pipes(self):
        children = []
        real_popen = subprocess.Popen

        def spawn(*args, **kwargs):
            child = real_popen(*args, **kwargs)
            children.append(child)
            return child

        with patch("owned_probe.subprocess.Popen", side_effect=spawn):
            with self.assertRaises(subprocess.TimeoutExpired):
                owned_probe.run_captured_probe([sys.executable, "-c", "import time; time.sleep(30)"], os.environ.copy(), timeout=0.2)
        self.assertEqual(len(children), 1)
        child = children[0]
        self.assertIsNotNone(child.poll())
        self.assertTrue(child.stdout.closed and child.stderr.closed)


if __name__ == "__main__":
    unittest.main()
