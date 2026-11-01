"""Fixed request probe selector/protocol/lifecycle mocks; no Rust or HTTP run."""
import json
from pathlib import Path
import queue
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import check_wasm_request_probe as gate
import owned_probe


class WasmRequestProbeGateTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="wasm-request-gate-test-", dir=gate.ROOT / "target")
        self.work = Path(self.directory.name)
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(owned_probe.reap_owned_probes)
        self.artifact = self.work / "request-probe.exe"
        self.artifact.write_bytes(b"fixed artifact canary")
        self.target = patch.object(gate, "TARGET_DIR", self.work)
        self.target.start()
        self.addCleanup(self.target.stop)

    def ready(self, **changes):
        value = {"format_version": 1, "profile": "wasmtime_36_0_19_fixed_request_time_component", "process_pid": 12345,
                 "authority": "127.0.0.1:32123", "compilations": 1, "warmups": 1}
        value.update(changes)
        return gate.READY_PREFIX.decode() + json.dumps(value) + "\n"

    def calls(self):
        return [gate.CALL_PREFIX.decode() + json.dumps({"format_version": 1, "ordinal": index, "fixture": fixture,
            "store_counter": 1, "input_bytes": length, "output_bytes": length}) + "\n"
            for index, (fixture, length) in enumerate((("a", 58), ("b", 77), ("a", 58), ("a", 58)), 1)]

    def cargo(self):
        return {"reason": "compiler-artifact", "manifest_path": str(gate.MANIFEST),
                "target": {"name": gate.BIN, "kind": ["bin"], "src_path": str(gate.MAIN)},
                "profile": {"test": False}, "executable": str(self.artifact)}

    def wire(self, items):
        return b"".join((json.dumps(value) + "\n").encode() for value in [*items, {"reason": "build-finished", "success": True}])

    def running(self):
        process = Mock(pid=12345, returncode=None)
        process.poll.return_value = None
        reader = Mock()
        reader.is_alive.return_value = True
        owner = SimpleNamespace(process=process, reader=reader, reader_errors=[])
        lines = queue.Queue()
        for line in [*self.calls(), None]:
            lines.put(line)
        return gate._Running(owner, lines, gate.validate_ready(self.ready(), 12345), self.work / "stdout")

    def stop(self, running):
        def stopped(*args, **kwargs):
            running.owner.process.returncode = 1
            running.owner.process.poll.return_value = 1
            running.owner.reader.is_alive.return_value = False
        return stopped

    def response(self, authority, method, target, host):
        headers = {"content-length": "0"}
        if host:
            return 421, headers, b""
        if method not in ("GET", "HEAD"):
            return 405, {**headers, "allow": "GET, HEAD"}, b""
        if target.startswith("http://"):
            return 400, headers, b""
        if target not in ("/a", "/b"):
            return 404, headers, b""
        body = gate.HTML_A if target == "/a" else gate.HTML_B
        return 200, {"content-length": str(len(body)), "content-type": "text/html; charset=utf-8",
                     "cache-control": "no-store", "x-content-type-options": "nosniff"}, b"" if method == "HEAD" else body

    def test_selector_requires_current_manifest_source_unique_completed_build(self):
        self.assertEqual(gate.select_artifact(self.wire([self.cargo()])), self.artifact.resolve())
        old = self.cargo()
        old["manifest_path"] = str(gate.ROOT / "experiments/wasm-component-probe/Cargo.toml")
        wrong = self.cargo()
        wrong["target"]["src_path"] = str(self.work / "wrong.rs")
        for wire in (self.wire([old]), self.wire([wrong]), self.wire([self.cargo(), self.cargo()]), self.wire([]),
                     b'{"reason":"build-finished","success":false}\n', self.wire([self.cargo()])[:-1]):
            with self.assertRaises(Exception):
                gate.select_artifact(wire)

    def test_ready_checks_actual_pid_loopback_authority_and_typed_compile_warm_counts(self):
        self.assertEqual(gate.validate_ready(self.ready(), 12345)["compilations"], 1)
        for changes in ({"process_pid": 12346}, {"process_pid": True}, {"compilations": True}, {"warmups": 2},
                        {"authority": "localhost:32123"}, {"authority": "127.0.0.1:0"}, {"authority": "127.0.0.1:65536"},
                        {"authority": "127.0.0.1:0123"}, {"profile": "old"}, {"PRIVATE_unknown": True}):
            with self.assertRaises(gate.WasmRequestProbeGateError):
                gate.validate_ready(self.ready(**changes), 12345)
        duplicate = self.ready().replace('"process_pid": 12345', '"process_pid": 12345, "process_pid": 12345')
        with self.assertRaises(gate.WasmRequestProbeGateError):
            gate.validate_ready(duplicate, 12345)

    def test_complete_call_stream_requires_exact_order_fresh_counter_and_byte_counts(self):
        self.assertEqual([value["fixture"] for value in gate.validate_calls(self.calls())], ["a", "b", "a", "a"])
        for lines in (self.calls()[:3], self.calls() + [self.calls()[0]], [self.ready(), *self.calls()[1:]]):
            with self.assertRaises(gate.WasmRequestProbeGateError):
                gate.validate_calls(lines)
        for key, value in (("store_counter", 2), ("ordinal", True), ("output_bytes", 77), ("fixture", "b"), ("PRIVATE_extra", True)):
            lines = self.calls()
            first = json.loads(lines[0][len(gate.CALL_PREFIX):])
            first[key] = value
            lines[0] = gate.CALL_PREFIX.decode() + json.dumps(first) + "\n"
            with self.assertRaises(gate.WasmRequestProbeGateError):
                gate.validate_calls(lines)

    def test_http_plan_interleaves_refusals_then_stops_before_checking_stdout_and_call_tail(self):
        running = self.running()
        with patch.object(gate, "_start", return_value=running), patch.object(gate, "_request", side_effect=self.response) as requests, patch.object(gate.smoke, "stop_process", side_effect=self.stop(running)) as stop, patch.object(gate, "_read_regular", return_value=b"") as read, patch.object(gate, "_remove_stdout") as remove:
            value = gate._observe(self.artifact)
            stop.assert_called_once_with(running.owner.process, running.owner.reader, original=None)
            read.assert_called_once_with(running.stdout_dir / "stdout.bin", 0)
            remove.assert_called_once_with(running.stdout_dir)
        self.assertEqual(requests.call_count, 9)
        self.assertEqual([item["status"] for item in value["http_observations"]], [200, 404, 200, 421, 404, 405, 400, 200, 200])
        self.assertEqual(value["http_observations"][-1]["body_bytes"], 0)
        self.assertEqual(len(value["calls"]), 4)
        self.assertTrue(value["stdout_empty"] and value["owned_process_reaped"])

    def test_stdout_failure_is_cleaned_but_unconfirmed_process_retains_its_directory(self):
        running = self.running()
        with patch.object(gate, "_start", return_value=running), patch.object(gate, "_request", side_effect=self.response), patch.object(gate.smoke, "stop_process", side_effect=self.stop(running)), patch.object(gate, "_read_regular", side_effect=RuntimeError("PRIVATE_STDOUT")), patch.object(gate, "_remove_stdout") as remove:
            with self.assertRaises(RuntimeError):
                gate._observe(self.artifact)
            remove.assert_called_once_with(running.stdout_dir)
        running = self.running()
        with patch.object(gate, "_start", return_value=running), patch.object(gate, "_request", side_effect=self.response), patch.object(gate.smoke, "stop_process"), patch.object(gate, "_remove_stdout") as remove:
            with self.assertRaises(gate.WasmRequestProbeGateError):
                gate._observe(self.artifact)
            remove.assert_not_called()

    def test_reader_start_failure_has_registry_before_thread_and_removes_confirmed_stdout_sink(self):
        process = Mock(pid=12345, returncode=None)
        process.poll.return_value = None
        owner = SimpleNamespace(process=process, reader=None, reader_errors=[])
        reader = Mock()
        reader.is_alive.return_value = False
        reader.start.side_effect = RuntimeError("PRIVATE_THREAD_START")
        order = []

        def remembered(child):
            order.append("registered")
            return owner

        def thread(*args, **kwargs):
            order.append("reader-created")
            return reader

        def stop(*args, **kwargs):
            process.poll.return_value = 1
            process.returncode = 1

        before = set((gate.ROOT / "target").glob("wasm-request-stdout-*"))
        with patch.object(gate.subprocess, "Popen", return_value=process) as popen, patch.object(gate.smoke, "_remember", side_effect=remembered), patch.object(gate.threading, "Thread", side_effect=thread), patch.object(gate.smoke, "stop_process", side_effect=stop):
            with self.assertRaises(RuntimeError):
                gate._start(self.artifact)
        self.assertEqual(order, ["registered", "reader-created"])
        self.assertTrue(popen.call_args.kwargs["stdout"].closed)
        self.assertEqual(set((gate.ROOT / "target").glob("wasm-request-stdout-*")), before)

    def test_fixed_build_runs_selected_artifact_and_all_cleanup_retries_even_after_failure(self):
        built = subprocess.CompletedProcess(["cargo"], 0, self.wire([self.cargo()]), b"Cargo progress")
        with patch.object(owned_probe, "run_captured_probe", return_value=(11223, built)) as run, patch.object(gate, "_observe", return_value={"stdout_empty": True}) as observe, patch.object(gate.smoke, "reap_servers") as reap_servers, patch.object(owned_probe, "reap_owned_probes") as reap_probes:
            self.assertTrue(gate.run_gate()["stdout_empty"])
            observe.assert_called_once_with(self.artifact.resolve())
            command = run.call_args.args[0]
            self.assertEqual(command, ["cargo", "build", "--manifest-path", str(gate.MANIFEST), "--package", gate.BIN, "--bin", gate.BIN,
                                       "--target-dir", str(self.work), "--offline", "--locked", "--message-format=json"])
            reap_servers.assert_called_once()
            reap_probes.assert_called_once()
        built.stdout = self.wire([self.cargo(), self.cargo()])
        with patch.object(owned_probe, "run_captured_probe", return_value=(11223, built)), patch.object(gate, "_observe") as observe, patch.object(gate.smoke, "reap_servers", side_effect=RuntimeError("PRIVATE_CLEANUP")), patch.object(owned_probe, "reap_owned_probes") as reap_probes:
            with self.assertRaises(gate.WasmRequestProbeGateError) as caught:
                gate.run_gate()
            observe.assert_not_called()
            reap_probes.assert_called_once()
            self.assertEqual(caught.exception.__notes__, ["Request probe cleanup retry failed: RuntimeError"])


if __name__ == "__main__":
    unittest.main()
