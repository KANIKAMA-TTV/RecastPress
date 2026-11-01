"""Fixed publication frontend mocks/local files; real Rust/HTTP belongs to E2E."""
from dataclasses import replace
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import owned_probe
import wasm_publication_example as example


class WasmPublicationExampleTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory(prefix="wasm-publication-front-test-", dir=example.ROOT / "target")
        self.work = Path(self.directory.name)
        self.addCleanup(self.directory.cleanup)
        self.addCleanup(owned_probe.reap_owned_probes)
        self.profiles = {role: replace(value, target=self.work / role.value) for role, value in example._PROFILES.items()}
        self.artifacts = {}
        for role, profile in self.profiles.items():
            profile.target.mkdir()
            artifact = profile.target / (profile.binary + ".exe")
            artifact.write_bytes(b"fixed artifact canary " + role.value.encode())
            self.artifacts[role] = artifact
        self.profiles_patch = patch.object(example, "_PROFILES", self.profiles)
        self.profiles_patch.start()
        self.addCleanup(self.profiles_patch.stop)

    def cargo_message(self, role):
        profile = self.profiles[role]
        return {"reason": "compiler-artifact", "manifest_path": str(profile.artifact_manifest),
                "target": {"name": profile.binary, "kind": ["bin"], "src_path": str(profile.main)},
                "profile": {"test": False}, "executable": str(self.artifacts[role])}

    def cargo_wire(self, messages):
        return b"".join((json.dumps(value) + "\n").encode() for value in [*messages, {"reason": "build-finished", "success": True}])

    def package(self, destination):
        destination.mkdir()
        payload = (json.dumps({"format_version": 1, "pages": [{"target": "/wasm-fixture", "status": 200,
            "content_type": "text/html; charset=utf-8", "body": example.EXPECTED_HTML.decode()}]}, ensure_ascii=False) + "\n").encode()
        receipt = (json.dumps({"format_version": 1, "payload_bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}) + "\n").encode()
        (destination / "bundle.json").write_bytes(payload)
        (destination / "receipt.json").write_bytes(receipt)
        return {"format_version": 1, "profile": "wasmtime_36_0_19_fixed_component_prepared_bundle", "process_pid": 34567,
                "body_bytes": 153, "bundle_bytes": len(payload), "receipt_bytes": len(receipt),
                "post_return_completed": True, "guest_memory_changed_after_copy": True, "unpublished_only": True}

    def native(self, summary, code=0, stderr=b""):
        return subprocess.CompletedProcess(["fixed-producer"], code, example.PREFIX + (json.dumps(summary) + "\n").encode(), stderr)

    def test_two_fixed_build_profiles_select_only_their_current_artifacts(self):
        for role in example._Role:
            profile = self.profiles[role]
            result = subprocess.CompletedProcess(["cargo"], 0, self.cargo_wire([self.cargo_message(role)]), b"Cargo progress")
            with patch.object(owned_probe, "run_captured_probe", return_value=(12345, result)) as run:
                self.assertEqual(example._build(role), self.artifacts[role].resolve())
                self.assertEqual(run.call_args.args[0], ["cargo", "build", "--manifest-path", str(profile.build_manifest),
                    "--package", profile.package, "--bin", profile.binary, "--target-dir", str(profile.target),
                    "--offline", "--locked", "--message-format=json"])
            other = next(item for item in example._Role if item != role)
            for messages in ([self.cargo_message(other)], [self.cargo_message(role), self.cargo_message(role)]):
                with self.assertRaises(example.WasmPublicationExampleError):
                    example._select_artifact(self.cargo_wire(messages), role)
            item = self.cargo_message(role)
            item["manifest_path"] = str(self.profiles[other].artifact_manifest)
            with self.assertRaises(example.WasmPublicationExampleError):
                example._select_artifact(self.cargo_wire([item]), role)
            item = self.cargo_message(role)
            item["executable"] = str(self.artifacts[other])
            with self.assertRaises(example.WasmPublicationExampleError):
                example._select_artifact(self.cargo_wire([item]), role)
        failed = b'{"reason":"build-finished","success":false}\n'
        with self.assertRaises(example.WasmPublicationExampleError):
            example._select_artifact(failed, example._Role.PRODUCER)

    def test_prepare_builds_only_producer_forwards_only_new_dir_and_checks_saved_identity(self):
        parent = self.work / "PRIVATE folder"
        parent.mkdir()
        destination = parent / "package"
        producer = self.artifacts[example._Role.PRODUCER]

        def run(command, environment, timeout):
            self.assertEqual(command, [str(producer), str(destination)])
            self.assertEqual(timeout, 20)
            return 34567, self.native(self.package(destination))

        with patch.object(example, "_build", return_value=producer) as build, patch.object(owned_probe, "run_captured_probe", side_effect=run), patch.object(owned_probe, "reap_owned_probes") as reap:
            value = example.prepare_example(destination)
            build.assert_called_once_with(example._Role.PRODUCER)
            reap.assert_called_once()
        self.assertEqual(set(value), {"artifact", "artifact_sha256", "summary"})
        self.assertEqual(value["artifact"], producer)
        self.assertEqual(value["artifact_sha256"], hashlib.sha256(producer.read_bytes()).hexdigest())
        self.assertEqual(value["summary"]["body_bytes"], len(example.EXPECTED_HTML))
        self.assertEqual(len(example.EXPECTED_HTML), 153)
        self.assertEqual({path.name for path in destination.iterdir()}, {"bundle.json", "receipt.json"})

    def test_native_protocol_requires_actual_pid_fixed_values_true_flags_and_integer_bounds(self):
        summary = self.package(self.work / "protocol-package")
        self.assertEqual(example._validate_native(34567, self.native(summary)), summary)
        mutations = {"process_pid": 34568, "body_bytes": 153.0, "bundle_bytes": True, "receipt_bytes": 1025,
                     "post_return_completed": False, "guest_memory_changed_after_copy": 1, "unpublished_only": False,
                     "profile": "other", "PRIVATE_unknown": True}
        for key, value in mutations.items():
            with self.subTest(key=key), self.assertRaises(example.WasmPublicationExampleError):
                example._validate_native(34567, self.native({**summary, key: value}))
        for result in (self.native(summary, code=2), self.native(summary, stderr=b"PRIVATE_DIAGNOSTIC")):
            with self.assertRaises(example.WasmPublicationExampleError):
                example._validate_native(34567, result)
        duplicate = self.native(summary)
        duplicate.stdout = duplicate.stdout.replace(b'"process_pid": 34567', b'"process_pid": 34567, "process_pid": 34567')
        with self.assertRaises(example.WasmPublicationExampleError):
            example._validate_native(34567, duplicate)

    def test_existing_output_and_missing_parent_are_opaque_before_any_build(self):
        existing = self.work / "existing"
        existing.mkdir()
        (existing / "bundle.json").write_bytes(b"PRIVATE_EXISTING")
        empty = self.work / "empty"
        empty.mkdir()
        file = self.work / "file"
        file.write_bytes(b"FILE_CANARY")
        for destination in (existing, empty, file, self.work / "missing" / "new"):
            with patch.object(example, "_build") as build, patch.object(owned_probe, "reap_owned_probes") as reap:
                with self.assertRaises(example.WasmPublicationExampleError) as caught:
                    example.prepare_example(destination)
                self.assertEqual(str(caught.exception), "Wasm publication example failed")
                self.assertNotIn("PRIVATE", repr(caught.exception))
                build.assert_not_called()
                reap.assert_called_once()
        self.assertEqual((existing / "bundle.json").read_bytes(), b"PRIVATE_EXISTING")
        self.assertEqual(file.read_bytes(), b"FILE_CANARY")
        self.assertEqual(list(empty.iterdir()), [])

    def test_saved_lengths_receipt_digest_schema_and_regular_file_caps_are_checked(self):
        destination = self.work / "package"
        summary = self.package(destination)
        example._check_package(destination, summary)
        payload = (destination / "bundle.json").read_bytes()
        receipt_wire = (destination / "receipt.json").read_bytes()
        (destination / "bundle.json").write_bytes(b"X" + payload[1:])
        with self.assertRaises(example.WasmPublicationExampleError):
            example._check_package(destination, summary)
        (destination / "bundle.json").write_bytes(payload)
        receipt = json.loads(receipt_wire)
        for value in ({**receipt, "payload_bytes": True}, {**receipt, "sha256": "0" * 64}, {**receipt, "PRIVATE_unknown": True}):
            wire = (json.dumps(value) + "\n").encode()
            (destination / "receipt.json").write_bytes(wire)
            with self.assertRaises(example.WasmPublicationExampleError):
                example._check_package(destination, {**summary, "receipt_bytes": len(wire)})
        (destination / "receipt.json").write_bytes(receipt_wire)
        with patch.object(example, "MAX_BUNDLE", len(payload)):
            example._check_package(destination, summary)
        with patch.object(example, "MAX_BUNDLE", len(payload) - 1), self.assertRaises(Exception):
            example._check_package(destination, summary)
        (destination / "bundle.json").unlink()
        (destination / "bundle.json").mkdir()
        with self.assertRaises(Exception):
            example._check_package(destination, summary)

    def test_native_failure_preserves_partial_files_and_server_build_uses_only_server_profile(self):
        producer = self.artifacts[example._Role.PRODUCER]
        destination = self.work / "partial"

        def fail(*args, **kwargs):
            destination.mkdir()
            (destination / "bundle.json").write_bytes(b"PARTIAL_CANARY")
            return 34567, subprocess.CompletedProcess(["producer"], 2, b"", b"fixed native refusal")

        with patch.object(example, "_build", return_value=producer), patch.object(owned_probe, "run_captured_probe", side_effect=fail), patch.object(owned_probe, "reap_owned_probes") as reap:
            with self.assertRaises(example.WasmPublicationExampleError):
                example.prepare_example(destination)
            reap.assert_called_once()
        self.assertEqual((destination / "bundle.json").read_bytes(), b"PARTIAL_CANARY")
        server = self.artifacts[example._Role.SERVER]
        with patch.object(example, "_build", return_value=server) as build, patch.object(owned_probe, "reap_owned_probes") as reap:
            self.assertEqual(example.build_public_server(), server)
            build.assert_called_once_with(example._Role.SERVER)
            reap.assert_called_once()
        with patch.object(example, "_build", side_effect=subprocess.TimeoutExpired(["fixed-build"], 0.2)), patch.object(owned_probe, "reap_owned_probes", side_effect=RuntimeError("PRIVATE_CLEANUP")):
            with self.assertRaises(example.WasmPublicationExampleError) as caught:
                example.build_public_server()
            self.assertEqual(caught.exception.__notes__, ["Publication example cleanup retry failed: RuntimeError"])

    def test_existing_output_symlink_is_refused_without_build_or_target_mutation(self):
        target = self.work / "target"
        target.mkdir()
        canary = target / "canary"
        canary.write_bytes(b"PRIVATE_LINK_CANARY")
        link = self.work / "link"
        try:
            link.symlink_to(target, target_is_directory=True)
        except (OSError, NotImplementedError) as error:
            self.skipTest("symlink creation unavailable: " + type(error).__name__)
        with patch.object(example, "_build") as build:
            with self.assertRaises(example.WasmPublicationExampleError):
                example.prepare_example(link)
            build.assert_not_called()
        self.assertEqual(canary.read_bytes(), b"PRIVATE_LINK_CANARY")
        self.assertTrue(link.is_symlink())


if __name__ == "__main__":
    unittest.main()
