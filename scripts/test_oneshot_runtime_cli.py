"""Actual one-shot/runtime boundary with a fixed public ZIP fixture.

Child-only provider env control; no latency/RSS/thread-count claim.
The historical pre-adoption observer remains under target, outside this suite.
"""
import base64
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
import zipfile

ROOT = Path(__file__).resolve().parents[1]
BUILD_STREAM = ROOT / "target/oneshot-runtime-current-server-build.jsonl"
AFTER_OBSERVATION = ROOT / "target/oneshot-runtime-after-observation.json"
BASELINE_OBSERVATION = ROOT / "target/oneshot-runtime-draft/before-observation.json"
sys.path.insert(0, str(ROOT / "scripts"))
import owned_probe
from test_prepared_workbench_package import select_server

MARKER = b'"TOKIO_WORKER_THREADS" cannot be set to 0'
# Immutable adopted fixture; no staged-path fallback in the promoted test.
FIXTURE = ROOT / "scripts/fixtures/oneshot-runtime-package.json"
ONE_SHOTS = frozenset({"prepare-posts", "bundle-receipt", "check-bundle", "check-package",
                       "prepare-post-package", "unpack-package", "inspect-memory"})


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode("utf-8")


def source_hashes():
    paths = ["Cargo.toml", "Cargo.lock", "crates/recastpress-server/Cargo.toml",
             "crates/recastpress-server/src/main.rs", "crates/recastpress-server/src/cli.rs",
             "crates/recastpress-server/src/shutdown.rs"]
    return {name: sha((ROOT / name).read_bytes()) for name in paths}


def current_build():
    before = source_hashes()
    pid, result = owned_probe.run_captured_probe(
        ["cargo", "build", "-p", "recastpress-server", "--bin", "recastpress",
         "--manifest-path", str(ROOT / "Cargo.toml"), "--offline", "--locked",
         "--message-format=json"], os.environ.copy(), timeout=240)
    if result.returncode != 0 or len(result.stdout) > 16 * 1024 * 1024:
        raise RuntimeError("Current artifact build failed")
    binary = select_server(result.stdout)
    if source_hashes() != before:
        raise RuntimeError("Selected sources changed")
    BUILD_STREAM.write_bytes(result.stdout)
    # Do not copy arbitrary build stderr into observations.
    return pid, binary, before, sha(result.stdout)


def zero_environment():
    environment = os.environ.copy()
    environment["TOKIO_WORKER_THREADS"] = "0"
    return environment


class OneShotRuntimeCliTests(unittest.TestCase):
    def run(self, result=None):
        result = super().run(result)
        type(self)._result = result
        return result

    @classmethod
    def setUpClass(cls):
        # Class finalizer runs only after all per-test cleanups. No artifact is
        # emitted for partial selection, skip, assertion failure or cleanup error.
        # Its first operation is reap; register it before the first Cargo child.
        cls.addClassCleanup(cls.finish_observation)
        cls._runs = []
        cls._started = set()
        cls._cleaned = set()
        cls._result = None
        cls._expected = {f"{cls.__module__}.{cls.__qualname__}.{name}" for name in dir(cls)
                         if name.startswith("test_") and callable(getattr(cls, name))}
        AFTER_OBSERVATION.unlink(missing_ok=True)
        cls.build_pid, cls.binary, cls.sources, cls.cargo_sha = current_build()
        cls.binary_sha = sha(cls.binary.read_bytes())
        fixture_bytes = FIXTURE.read_bytes()
        cls.fixture_sha = sha(fixture_bytes)
        cls.test_sha = sha(Path(__file__).read_bytes())
        fixture = json.loads(fixture_bytes)
        cls.input = base64.b64decode(fixture["input_base64"], validate=True)
        cls.zip = base64.b64decode(fixture["archive_base64"], validate=True)
        if sha(cls.zip) != fixture["archive_sha256"]:
            raise AssertionError("Fixed actual-JS archive differs")
        with zipfile.ZipFile(io.BytesIO(cls.zip)) as archive:
            if archive.namelist() != ["bundle.json", "receipt.json"] or archive.testzip() is not None:
                raise AssertionError("Fixed actual-JS pair differs")
            cls.payload = archive.read("bundle.json")
            cls.receipt = archive.read("receipt.json")
        if json.loads(cls.receipt) != {"format_version": 1, "payload_bytes": len(cls.payload), "sha256": sha(cls.payload)}:
            raise AssertionError("Fixed receipt differs")

    @classmethod
    def finish_observation(cls):
        owned_probe.reap_owned_probes()
        result = cls._result
        if (result is None or not result.wasSuccessful() or cls._started != cls._expected
                or cls._cleaned != cls._expected):
            return
        after = source_hashes()
        binary_after = sha(cls.binary.read_bytes())
        if after != cls.sources or binary_after != cls.binary_sha:
            raise AssertionError("Selected identities changed")
        if sha(FIXTURE.read_bytes()) != cls.fixture_sha or sha(Path(__file__).read_bytes()) != cls.test_sha:
            raise AssertionError("Test inputs changed")
        runs = cls._runs
        counts = {"tests": len(cls._expected),
                  "one_shot_successes": sum(run["env0"] and run["command"] in ONE_SHOTS
                                             and run["exit_code"] in (0, 3) for run in runs),
                  "closed_one_shot_refusals": sum(run["env0"] and run["command"] in ONE_SHOTS
                                                 and run["exit_code"] == 2 for run in runs),
                  "help_successes": sum(run["env0"] and run["command"] in ("--help", "-h")
                                        and run["exit_code"] == 0 for run in runs),
                  "serve_runtime_refusals": sum(run["command"] in ("public", "management")
                                                and run["provider_refusal"] for run in runs),
                  "env0_children": sum(run["env0"] for run in runs),
                  "normal_reference_children": sum(not run["env0"] for run in runs)}
        if counts != {"tests": 5, "one_shot_successes": 7, "closed_one_shot_refusals": 9,
                      "help_successes": 2, "serve_runtime_refusals": 2,
                      "env0_children": 20, "normal_reference_children": 2}:
            raise AssertionError("Actual runtime-boundary counts differ")
        record = {"format_version": 1, "scope": "owned_one_shot_runtime_boundary",
                  "build_pid": cls.build_pid, "runs": runs, "counts": counts,
                  "cargo_stream_sha256": cls.cargo_sha,
                  "selected_source_hashes_before": cls.sources,
                  "selected_source_hashes_after": after,
                  "binary_sha256_before": cls.binary_sha, "binary_sha256_after": binary_after,
                  "fixture_sha256": cls.fixture_sha, "test_source_sha256": cls.test_sha,
                  "selected_sources_unchanged": True, "binary_unchanged": True,
                  "all_owned_children_reaped": True, "all_owned_directories_removed": True,
                  "limitations": "Pinned provider boundary, not thread count/latency/RSS/Linux or general process isolation proof."}
        wire = (json.dumps(record, indent=2) + "\n").encode("utf-8")
        if len(wire) > 65536:
            raise AssertionError("Observation bound exceeded")
        AFTER_OBSERVATION.write_bytes(wire)

    def setUp(self):
        type(self)._started.add(self.id())
        self.work = Path(tempfile.mkdtemp(prefix="oneshot-runtime-", dir=ROOT / "target")).resolve()
        self.addCleanup(self.cleanup_owned)
        self.source = self.work / "input.json"
        self.bundle = self.work / "bundle.json"
        self.proof = self.work / "receipt.json"
        self.archive = self.work / "package.zip"
        for path, raw in ((self.source, self.input), (self.bundle, self.payload),
                          (self.proof, self.receipt), (self.archive, self.zip)):
            path.write_bytes(raw)

    def cleanup_owned(self):
        owned_probe.reap_owned_probes()
        if self.work.parent != (ROOT / "target").resolve() or not self.work.name.startswith("oneshot-runtime-"):
            raise AssertionError("Owned cleanup containment failed")
        if self.work.exists():
            shutil.rmtree(self.work)
        self.assertEqual(source_hashes(), self.sources)
        self.assertEqual(sha(self.binary.read_bytes()), self.binary_sha)
        type(self)._cleaned.add(self.id())

    def child(self, args, zero=True):
        environment = zero_environment() if zero else os.environ.copy()
        if not zero:
            environment.pop("TOKIO_WORKER_THREADS", None)
        pid, result = owned_probe.run_captured_probe([str(self.binary), *map(os.fspath, args)], environment, timeout=20)
        type(self)._runs.append({"process_pid": pid, "command": str(args[0]), "env0": zero,
                                "exit_code": result.returncode, "provider_refusal": MARKER in result.stderr})
        self.assertLessEqual(len(result.stdout), 65536)
        self.assertLessEqual(len(result.stderr), 65536)
        return result

    def successful(self, args, expected):
        result = self.child(args)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (0, expected, b""))

    def test_help_and_existing_one_shot_outputs_without_runtime(self):
        # Baseline with the actual same selected binary, no injected factory.
        expected_help = self.child(["--help"], zero=False)
        self.assertEqual((expected_help.returncode, expected_help.stderr), (0, b""))
        for option in ("--help", "-h"):
            self.successful([option], expected_help.stdout)
        self.successful(["prepare-posts", self.source], self.payload)
        self.successful(["bundle-receipt", self.bundle], self.receipt)
        normal = self.child(["check-bundle", self.bundle], zero=False)
        self.assertEqual((normal.returncode, normal.stderr), (0, b""))
        self.successful(["check-bundle", self.bundle], normal.stdout)
        self.successful(["check-package", self.bundle, self.proof], normal.stdout)

    def test_native_package_writes_and_zip_import_without_runtime(self):
        for command, source in (("prepare-post-package", self.source), ("unpack-package", self.archive)):
            output = self.work / command
            expected = b"post package prepared\n" if command == "prepare-post-package" else b"package unpacked\n"
            self.successful([command, source, output], expected)
            self.assertEqual(sorted(path.name for path in output.iterdir()), ["bundle.json", "receipt.json"])
            self.assertEqual((output / "bundle.json").read_bytes(), self.payload)
            self.assertEqual((output / "receipt.json").read_bytes(), self.receipt)
            refusal = self.child([command, source, output])
            message = b"post package preparation failed\n" if command == "prepare-post-package" else b"package unpack failed\n"
            self.assertEqual((refusal.returncode, refusal.stdout, refusal.stderr), (2, b"", message))

    def test_all_one_shot_refusals_keep_exact_closed_diagnostics(self):
        messages = {"prepare-posts": b"post preparation failed\n", "bundle-receipt": b"bundle receipt failed\n",
                    "check-bundle": b"bundle check failed\n", "check-package": b"package check failed\n",
                    "prepare-post-package": b"post package preparation failed\n", "unpack-package": b"package unpack failed\n",
                    "inspect-memory": b"memory inspection failed\n"}
        for command, message in messages.items():
            args = [command, "PRIVATE_EXTRA"] if command == "inspect-memory" else [command]
            result = self.child(args)
            self.assertEqual((result.returncode, result.stdout, result.stderr), (2, b"", message))

    def test_memory_observation_keeps_platform_status_without_runtime(self):
        result = self.child(["inspect-memory"])
        self.assertIn(result.returncode, (0, 3))
        self.assertEqual(result.stderr, b"")
        self.assertEqual(len(result.stdout.splitlines()), 1)
        value = json.loads(result.stdout)
        self.assertEqual(value["format_version"], 1)
        self.assertEqual(value["consistency"], "non_atomic")
        self.assertEqual(set(value), {"format_version", "consistency", "host_physical", "self_rss", "visible_cgroup_v2"})
        if os.name == "nt":
            self.assertEqual(result.returncode, 3)
            for section in ("host_physical", "self_rss", "visible_cgroup_v2"):
                self.assertEqual(value[section]["status"], "unsupported")
                self.assertEqual(value[section]["kind"], "unsupported")

    def test_both_serve_roles_still_construct_the_same_provider_runtime(self):
        # Deliberate provider refusal is the boundary control, not a benchmark.
        for args in (["public", "--host", "example.test", "--listen", "127.0.0.1:0"],
                     ["management", "--listen", "127.0.0.1:0"]):
            result = self.child(args)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(result.stdout, b"")
            self.assertTrue(MARKER in result.stderr)
            self.assertNotIn(b" listening on ", result.stderr)


if __name__ == "__main__":
    unittest.main()
