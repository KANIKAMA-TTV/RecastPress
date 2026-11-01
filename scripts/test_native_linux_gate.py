"""Pure mocked gate boundaries; these tests do not execute Rust or Linux binaries."""
from contextlib import redirect_stdout
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

import check_native_linux as gate


class NativeLinuxGateTests(unittest.TestCase):
    def fixture(self):
        parent = gate.ROOT / "target"
        parent.mkdir(exist_ok=True)
        return tempfile.TemporaryDirectory(prefix="native-linux-unit-", dir=parent)

    def test_plan_and_non_linux_execution_start_no_tool_or_smoke_child(self):
        with patch.object(gate.sys, "platform", "win32"), patch.object(gate.subprocess, "run") as run, patch.object(gate.subprocess, "Popen") as popen, patch.object(gate.importlib, "import_module") as imported:
            with redirect_stdout(io.StringIO()) as output:
                gate.main(["--plan"])
            self.assertIn("PLAN ONLY", output.getvalue())
            self.assertIn("test_package_check_cli.py", output.getvalue())
            self.assertIn("test_title_publication.py", output.getvalue())
            self.assertIn("check_wp_pure_profile.py", output.getvalue())
            with self.assertRaises(gate.NativeLinuxGateError):
                gate.main([])
            run.assert_not_called(); popen.assert_not_called(); imported.assert_not_called()

    def test_old_python_is_refused_before_any_tool(self):
        with patch.object(gate.sys, "platform", "linux"), patch.object(gate.sys, "version_info", (3, 10)), patch.object(gate, "capture") as capture:
            with self.assertRaises(gate.NativeLinuxGateError): gate.run_gate()
            capture.assert_not_called()

    def artifact_wire(self, artifact):
        item = {"reason":"compiler-artifact", "manifest_path":str(gate.ROOT / "crates/recastpress-server/Cargo.toml"),
                "target":{"name":"recastpress","kind":["bin"],"src_path":str(gate.ROOT / "crates/recastpress-server/src/main.rs")},
                "profile":{"test":False},"executable":str(artifact)}
        return item, (json.dumps(item)+"\n"+json.dumps({"reason":"build-finished","success":True})+"\n").encode()

    def test_cargo_artifact_requires_unique_completed_correct_server_and_strict_json(self):
        with self.fixture() as directory:
            artifact=Path(directory)/"other-target-dir"/"recastpress";artifact.parent.mkdir();artifact.write_bytes(b"synthetic artifact, never executed")
            item, wire=self.artifact_wire(artifact)
            self.assertEqual(gate.server_artifact(wire),artifact.resolve())
            cases=[wire+wire,wire.rstrip(),b'{"reason":"build-finished","reason":"build-finished","success":true}\n',
                   (json.dumps(item)+"\n").encode(),wire.replace(b'"success": true',b'"success": 1')]
            for field,value in [("manifest_path",str(gate.ROOT/"crates/recastpress-wp/Cargo.toml")),("executable",str(artifact.parent/"missing")),("profile",{"test":True})]:
                changed=dict(item);changed[field]=value
                cases.append((json.dumps(changed)+'\n{"reason":"build-finished","success":true}\n').encode())
            duplicate=(json.dumps(item)+"\n"+json.dumps(item)+'\n{"reason":"build-finished","success":true}\n').encode()
            cases.append(duplicate)
            for bad in cases:
                with self.subTest(bad=bad[:60]), self.assertRaises(gate.NativeLinuxGateError): gate.server_artifact(bad)

    def test_smoke_uses_exact_cargo_artifact_and_restores_binding_and_host_environment_on_failure(self):
        previous=Path("old-fixed-target/debug/recastpress")
        chosen=Path("custom-target/host/debug/recastpress")
        def main():
            self.assertEqual(smoke.BINARY,chosen)
            self.assertEqual(os.environ["CARGO_BUILD_TARGET"],"aarch64-unknown-linux-gnu")
            raise RuntimeError("mock smoke failure")
        smoke=types.SimpleNamespace(BINARY=previous,main=main)
        with patch.dict(os.environ,{"CARGO_BUILD_TARGET":"wrong-target"}), patch.object(gate.importlib,"import_module",return_value=smoke):
            with self.assertRaisesRegex(RuntimeError,"mock smoke failure"):
                gate.smoke_artifact(chosen,{"CARGO_BUILD_TARGET":"aarch64-unknown-linux-gnu"})
            self.assertEqual(smoke.BINARY,previous)
            self.assertEqual(os.environ["CARGO_BUILD_TARGET"],"wrong-target")

    def test_workflow_pins_host_in_all_children_and_binds_the_selected_artifact_before_python(self):
        captures=[]; commands=[]; events=[]
        with self.fixture() as directory:
            artifact=Path(directory)/"arbitrary-target"/"recastpress";artifact.parent.mkdir();artifact.write_bytes(b"not an executable")
            _,wire=self.artifact_wire(artifact)
            def capture(command,environment,limit,timeout):
                captures.append((command,dict(environment)))
                return b"rustc 1.94.0\nhost: aarch64-unknown-linux-gnu\n" if command[0]=="rustc" else b"cargo 1.94.0\n" if command[1]=="--version" else wire
            def run(command,environment):
                commands.append((command,dict(environment)))
                events.append("pure" if command == (sys.executable, str(gate.PURE_WP_SCRIPT))
                              else "python" if command[0]==sys.executable else "rust")
            def smoke(path,environment):
                self.assertEqual(path,artifact.resolve());self.assertEqual(environment["CARGO_BUILD_TARGET"],"aarch64-unknown-linux-gnu");events.append("smoke")
            with patch.object(gate.sys,"platform","linux"), patch.dict(os.environ,{"CARGO_BUILD_TARGET":"wrong-target","CARGO_TARGET_DIR":"custom-output"}), patch.object(gate,"capture",side_effect=capture),patch.object(gate,"run",side_effect=run),patch.object(gate,"smoke_artifact",side_effect=smoke),redirect_stdout(io.StringIO()):
                gate.run_gate()
            for _,environment in commands+captures[1:]:
                self.assertEqual(environment["CARGO_BUILD_TARGET"],"aarch64-unknown-linux-gnu")
                self.assertEqual(environment["CARGO_TARGET_DIR"],"custom-output")
                self.assertEqual(environment["CARGO_NET_OFFLINE"],"true")
            self.assertLess(events.index("smoke"),events.index("python"))
            self.assertEqual(events[:4], ["rust", "rust", "rust", "pure"])
            self.assertLess(events.index("pure"), events.index("smoke"))
            selected = {command[-2] for command, _ in commands if "unittest" in command}
            self.assertTrue({"test_package_check_cli.py", "test_title_publication.py"}.issubset(selected))

    def test_first_command_failure_is_propagated_without_build_smoke_or_python(self):
        failure=subprocess.CalledProcessError(17,["cargo","test"])
        def capture(command,*args):
            return b"host: x86_64-unknown-linux-gnu\n" if command[0]=="rustc" else b"cargo installed\n"
        with patch.object(gate.sys,"platform","linux"),patch.object(gate,"capture",side_effect=capture) as captured,patch.object(gate,"run",side_effect=[None,failure]) as run,patch.object(gate,"smoke_artifact") as smoke,redirect_stdout(io.StringIO()):
            with self.assertRaises(subprocess.CalledProcessError) as caught: gate.run_gate()
            self.assertIs(caught.exception,failure);self.assertEqual(run.call_count,2);self.assertEqual(captured.call_count,2);smoke.assert_not_called()

    def test_invalid_or_non_linux_rustc_host_does_not_start_cargo_checks(self):
        for wire in [b"host: x86_64-pc-windows-msvc\n",b"host: aarch64-unknown-linux-gnu\nhost: aarch64-unknown-linux-gnu\n",b"host: ../linux/target\n"]:
            with self.assertRaises(gate.NativeLinuxGateError): gate.rust_host(wire)

    def test_missing_selected_test_or_pure_controller_is_refused_before_any_child(self):
        for pure_missing in (False, True):
            with self.fixture() as directory:
                absent = Path(directory) / "missing-controller.py"
                selected = gate.SELECTED_TESTS if pure_missing else ("missing-native-selected.py",)
                with patch.object(gate.sys, "platform", "linux"), patch.object(gate, "SELECTED_TESTS", selected), patch.object(gate, "PURE_WP_SCRIPT", absent), patch.object(gate, "capture") as captured, patch.object(gate, "run") as run, patch.object(gate, "smoke_artifact") as smoke:
                    with self.assertRaises(gate.NativeLinuxGateError): gate.run_gate()
                    captured.assert_not_called(); run.assert_not_called(); smoke.assert_not_called()

    def test_pure_stage_failure_keeps_original_error_and_never_builds_or_serves(self):
        failure = subprocess.CalledProcessError(19, [sys.executable, "pure-controller"])
        def capture(command, *args):
            return b"host: x86_64-unknown-linux-gnu\n" if command[0] == "rustc" else b"cargo installed\n"
        with patch.object(gate.sys, "platform", "linux"), patch.object(gate, "capture", side_effect=capture) as captured, patch.object(gate, "run", side_effect=[None, None, None, failure]) as run, patch.object(gate, "smoke_artifact") as smoke, redirect_stdout(io.StringIO()):
            with self.assertRaises(subprocess.CalledProcessError) as caught: gate.run_gate()
            self.assertIs(caught.exception, failure)
            self.assertEqual(run.call_count, 4)
            self.assertEqual(run.call_args.args[0], (sys.executable, str(gate.PURE_WP_SCRIPT)))
            self.assertEqual(captured.call_count, 2)
            smoke.assert_not_called()

    def test_live_fixed_python_child_success_nonzero_and_timeout_are_reaped(self):
        # Executes only the installed Python on this host, never Linux/Rust tools.
        actual_popen = subprocess.Popen
        children = []
        def start(*args, **kwargs):
            self.assertFalse(kwargs["shell"])
            self.assertEqual(kwargs["creationflags"], subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
            child = actual_popen(*args, **kwargs)
            children.append(child)
            return child
        try:
            with patch.object(gate.subprocess, "Popen", side_effect=start), redirect_stdout(io.StringIO()):
                gate.run([sys.executable, "-c", "import sys; sys.exit(0 if sys.stdin.buffer.read(1) == b'' else 6)"], os.environ.copy(), timeout=10)
                with self.assertRaises(subprocess.CalledProcessError) as failed:
                    gate.run([sys.executable, "-c", "import sys; sys.exit(7)"], os.environ.copy(), timeout=10)
                self.assertEqual(failed.exception.returncode, 7)
                with self.assertRaises(subprocess.TimeoutExpired):
                    gate.run([sys.executable, "-c", "import time; time.sleep(30)"], os.environ.copy(), timeout=0.2)
            self.assertEqual(len(children), 3)
            self.assertTrue(all(child.poll() is not None and child.stdout is None for child in children))
            self.assertEqual(gate._CAPTURED, {})
        finally:
            for child in children:
                gate._stop_capture(child)

    def test_live_cleanup_failure_retains_handle_and_preserves_timeout_or_interrupt_for_outer_retry(self):
        for original in [subprocess.TimeoutExpired("synthetic", 1), KeyboardInterrupt()]:
            class Child:
                stdout = None
                returncode = None
                kills = 0
                def poll(self): return self.returncode
                def wait(self, timeout):
                    if self.returncode is None: raise original
                    return self.returncode
                def kill(self):
                    self.kills += 1
                    if self.kills == 1: raise OSError("synthetic kill failure")
                    self.returncode = -9
            child = Child()
            try:
                with patch.object(gate.subprocess, "Popen", return_value=child), redirect_stdout(io.StringIO()):
                    with self.assertRaises(type(original)) as caught:
                        gate.run(["never-executed"], {}, timeout=1)
                    self.assertIs(caught.exception, original)
                self.assertIs(gate._CAPTURED[id(child)], child)
                self.assertEqual(child.kills, 1)
                self.assertTrue(any("cleanup failed" in note for note in original.__notes__))
                outer = RuntimeError("synthetic next-stage failure")
                with patch.object(gate.sys, "platform", "linux"), patch.object(gate, "capture", side_effect=outer), redirect_stdout(io.StringIO()):
                    with self.assertRaises(RuntimeError) as caught: gate.run_gate()
                    self.assertIs(caught.exception, outer)
                self.assertEqual(child.kills, 2)
                self.assertNotIn(id(child), gate._CAPTURED)
            finally:
                if id(child) in gate._CAPTURED: gate._stop_capture(child)

    def test_successful_command_cleanup_failure_is_not_hidden_by_ambient_exception(self):
        class Stream:
            failed = False
            def close(self):
                if not self.failed:
                    self.failed = True
                    raise OSError("synthetic close failure")
        child = types.SimpleNamespace(stdout=Stream(), poll=lambda: 0, wait=lambda **kw: 0)
        try:
            try: raise ValueError("unrelated ambient exception")
            except ValueError:
                with patch.object(gate.subprocess, "Popen", return_value=child), redirect_stdout(io.StringIO()):
                    with self.assertRaises(gate.NativeLinuxGateError): gate.run(["never-executed"], {})
            self.assertIs(gate._CAPTURED[id(child)], child)
        finally:
            gate._stop_capture(child)

    def test_final_cleanup_continues_after_one_retained_failure_and_never_prints_passed(self):
        class Child:
            stdout = None
            returncode = None
            fail = False
            def poll(self): return self.returncode
            def kill(self):
                if self.fail: raise OSError("synthetic kill failure")
                self.returncode = -9
            def wait(self, timeout): return self.returncode
        first, second = Child(), Child()
        first.fail = True
        gate._CAPTURED[id(first)] = first
        gate._CAPTURED[id(second)] = second
        try:
            with self.fixture() as directory:
                artifact = Path(directory) / "recastpress"
                artifact.write_bytes(b"synthetic artifact, never executed")
                _, wire = self.artifact_wire(artifact)
                def capture(command, *args):
                    return b"host: x86_64-unknown-linux-gnu\n" if command[0] == "rustc" else b"cargo installed\n" if command[1] == "--version" else wire
                with patch.object(gate.sys, "platform", "linux"), patch.object(gate, "capture", side_effect=capture), patch.object(gate, "run"), patch.object(gate, "smoke_artifact"), redirect_stdout(io.StringIO()) as output:
                    with self.assertRaises(gate.NativeLinuxGateError): gate.run_gate()
                self.assertNotIn("subset passed", output.getvalue())
                self.assertIs(gate._CAPTURED[id(first)], first)
                self.assertNotIn(id(second), gate._CAPTURED)
        finally:
            first.fail = False
            for child in [first, second]: gate._stop_capture(child)


if __name__ == "__main__": unittest.main()
