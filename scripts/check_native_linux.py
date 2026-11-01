"""Run a native Linux-only offline Rust/HTTP/selected-Python subset, not release evidence.

Requires preinstalled Rust tools, Python >=3.11, cached dependencies, native
linker and loopback access. Does not install/fetch, run DB/PHP/reference gates,
or produce formal proof JSON. Global wrappers/runners and host config are trusted;
pinning the reported host target does not attest their behavior or OS isolation.
"""
import argparse
from contextlib import contextmanager
import hashlib
import importlib
import json
import os
from pathlib import Path
import re
import select
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SELECTED_TESTS = (
    "test_owned_probe.py", "test_smoke_lifecycle.py", "test_bundle_check_cli.py",
    "test_publication_cli.py", "test_post_package_cli.py", "test_prepared_posts_html.py",
    "test_feed_xml.py", "test_comment_feed_xml.py",
    "test_package_check_cli.py", "test_title_publication.py",
)
RUST_CHECKS = (
    ("cargo", "fmt", "--all", "--", "--check"),
    ("cargo", "test", "--workspace", "--offline", "--locked"),
    ("cargo", "clippy", "--workspace", "--all-targets", "--offline", "--locked", "--", "-D", "warnings"),
)
BUILD = ("cargo", "build", "-p", "recastpress-server", "--bin", "recastpress",
         "--offline", "--locked", "--message-format=json")
MAX_BUILD_JSON = 16 * 1024 * 1024
COMMAND_TIMEOUT = 1800
PURE_WP_SCRIPT = ROOT / "scripts/check_wp_pure_profile.py"
_CAPTURED = {}


class NativeLinuxGateError(RuntimeError):
    pass


def _stop_capture(child):
    if child.poll() is None:
        child.kill()
    child.wait(timeout=10)
    if child.stdout is not None:
        child.stdout.close()
    _CAPTURED.pop(id(child), None)


def capture(command, environment, limit, timeout):
    """Linux nonblocking bounded stdout capture; stderr stays inherited.

    No Popen context/unbounded wait. A failed kill/wait retains its exact handle
    for the outer retry; it never becomes a successful partial capture.
    """
    child = subprocess.Popen(list(command), cwd=ROOT, env=environment,
        stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=None, bufsize=0, shell=False)
    _CAPTURED[id(child)] = child
    original = None
    try:
        descriptor = child.stdout.fileno()
        os.set_blocking(descriptor, False)
        output = bytearray()
        deadline = time.monotonic() + timeout
        while True:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                raise subprocess.TimeoutExpired(command, timeout)
            readable, _, _ = select.select([descriptor], [], [], min(remaining, 0.1))
            if not readable:
                continue
            chunk = os.read(descriptor, min(65536, limit + 1 - len(output)))
            if not chunk:
                break
            output.extend(chunk)
            if len(output) > limit:
                raise NativeLinuxGateError("captured tool stdout exceeds its bound")
        code = child.wait(timeout=max(0.001, deadline - time.monotonic()))
        if code != 0:
            raise subprocess.CalledProcessError(code, command)
        return bytes(output)
    except BaseException as error:
        original = error
        raise
    finally:
        try:
            _stop_capture(child)
        except BaseException as cleanup:
            if original is not None:
                original.add_note("captured tool cleanup failed: " + type(cleanup).__name__)
            else:
                raise NativeLinuxGateError("captured tool cleanup was not confirmed") from cleanup


def run(command, environment, *, timeout=COMMAND_TIMEOUT):
    """Live inherited stdout/stderr, EOF stdin and bounded direct-child cleanup.

    These handles are not OS process-tree containment: trusted tools/runners
    may create descendants this registry does not own or independently reap.
    """
    print("Checking: " + " ".join(command), flush=True)
    child = subprocess.Popen(list(command), cwd=ROOT, env=environment,
        stdin=subprocess.DEVNULL, shell=False,
        creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
    _CAPTURED[id(child)] = child
    original = None
    try:
        code = child.wait(timeout=timeout)
        if code != 0:
            raise subprocess.CalledProcessError(code, list(command))
    except BaseException as error:
        original = error
        raise
    finally:
        try:
            _stop_capture(child)
        except BaseException as cleanup:
            if original is not None:
                original.add_note("live tool cleanup failed: " + type(cleanup).__name__)
            else:
                raise NativeLinuxGateError("live tool cleanup was not confirmed") from cleanup


def rust_host(wire):
    try:
        text = wire.decode("utf-8", errors="strict")
    except UnicodeError as error:
        raise NativeLinuxGateError("rustc metadata is not UTF-8") from error
    hosts = [line.removeprefix("host: ") for line in text.splitlines() if line.startswith("host: ")]
    if len(hosts) != 1 or not re.fullmatch(r"[A-Za-z0-9_.-]+", hosts[0]) or "-linux-" not in hosts[0]:
        raise NativeLinuxGateError("rustc did not identify one native Linux host triple")
    return hosts[0], text


def _object(pairs):
    value = {}
    for key, item in pairs:
        if key in value:
            raise ValueError("duplicate Cargo JSON key")
        value[key] = item
    return value


def _constant(_):
    raise ValueError("non-JSON numeric constant")


def server_artifact(wire):
    """Strict complete Cargo JSON stream; exactly one non-test server binary."""
    if not wire or len(wire) > MAX_BUILD_JSON or not wire.endswith(b"\n"):
        raise NativeLinuxGateError("Cargo JSON stream is empty, incomplete or oversized")
    candidates = []
    finished = False
    try:
        for line in wire.decode("utf-8", errors="strict").splitlines():
            message = json.loads(line, object_pairs_hook=_object, parse_constant=_constant)
            if type(message) is not dict or type(message.get("reason")) is not str or finished:
                raise ValueError("invalid Cargo message ordering")
            if message["reason"] == "build-finished":
                if message.get("success") is not True:
                    raise ValueError("unsuccessful Cargo completion")
                finished = True
                continue
            if message["reason"] != "compiler-artifact":
                continue
            target = message.get("target")
            if type(target) is not dict:
                raise ValueError("invalid artifact target")
            if target.get("name") != "recastpress" or target.get("kind") != ["bin"]:
                continue
            if (type(message.get("manifest_path")) is not str
                or Path(message["manifest_path"]).resolve() != (ROOT / "crates/recastpress-server/Cargo.toml").resolve()
                or type(target.get("src_path")) is not str
                or Path(target["src_path"]).resolve() != (ROOT / "crates/recastpress-server/src/main.rs").resolve()
                or type(message.get("profile")) is not dict or message["profile"].get("test") is not False
                or type(message.get("executable")) is not str):
                raise ValueError("wrong server artifact provenance")
            path = Path(message["executable"])
            if not path.is_absolute() or not path.is_file():
                raise ValueError("server artifact is not an existing absolute file")
            candidates.append(path.resolve(strict=True))
    except (ValueError, TypeError, KeyError, OSError) as error:
        raise NativeLinuxGateError("Cargo did not emit a valid complete server artifact stream") from error
    if not finished or len(candidates) != 1:
        raise NativeLinuxGateError("Cargo did not identify exactly one completed server binary")
    return candidates[0]


@contextmanager
def _host_environment(environment):
    key = "CARGO_BUILD_TARGET"
    previous = os.environ.get(key)
    os.environ[key] = environment[key]
    try:
        yield
    finally:
        if previous is None:
            os.environ.pop(key, None)
        else:
            os.environ[key] = previous


def smoke_artifact(artifact, environment):
    smoke = importlib.import_module("smoke")
    previous = smoke.BINARY
    try:
        smoke.BINARY = artifact
        with _host_environment(environment):
            smoke.main()  # Existing bounded owner cleanup and real Unix signals.
    finally:
        smoke.BINARY = previous


def _sha256(path):
    result = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            result.update(block)
    return result.hexdigest()


def run_gate():
    # Guard before metadata probes/imports/builds: non-Linux starts no child.
    if sys.platform != "linux":
        raise NativeLinuxGateError("native Linux execution requires sys.platform == 'linux'; use --plan")
    if sys.version_info < (3, 11):
        raise NativeLinuxGateError("native Linux execution requires Python >=3.11")
    if any(not (ROOT / "scripts" / name).is_file() for name in SELECTED_TESTS):
        raise NativeLinuxGateError("a selected Python test file is missing")
    if not PURE_WP_SCRIPT.is_file():
        raise NativeLinuxGateError("the pure WP controller file is missing")
    original = None
    try:
        environment = os.environ.copy()
        environment["CARGO_NET_OFFLINE"] = "true"
        host, rustc = rust_host(capture(("rustc", "-vV"), environment, 65536, 30))
        environment["CARGO_BUILD_TARGET"] = host
        cargo = capture(("cargo", "--version"), environment, 65536, 30).decode("utf-8", errors="strict")
        print(f"Host: {sys.platform}; Python: {sys.version.splitlines()[0]} ({sys.executable})")
        print("Rustc:\n" + rustc.rstrip())
        print("Cargo: " + cargo.rstrip())
        print("CARGO_BUILD_TARGET=" + host + "; CARGO_TARGET_DIR/configuration retained (trusted host)", flush=True)
        for command in RUST_CHECKS:
            run(command, environment)
        run((sys.executable, str(PURE_WP_SCRIPT)), environment)
        print("Building the Cargo-selected native server artifact", flush=True)
        artifact = server_artifact(capture(BUILD, environment, MAX_BUILD_JSON, 600))
        print(f"Server artifact: {artifact}\nSHA256: {_sha256(artifact)}", flush=True)
        smoke_artifact(artifact, environment)
        for name in SELECTED_TESTS:
            run((sys.executable, "-m", "unittest", "discover", "-s", "scripts", "-p", name, "-v"), environment)
    except BaseException as error:
        original = error
        raise
    finally:
        first_cleanup = None
        for child in list(_CAPTURED.values()):
            try:
                _stop_capture(child)
            except BaseException as cleanup:
                if original is not None:
                    original.add_note("retained tool cleanup retry failed: " + type(cleanup).__name__)
                elif first_cleanup is None:
                    first_cleanup = cleanup
                else:
                    first_cleanup.add_note("another retained tool cleanup retry failed: " + type(cleanup).__name__)
        if first_cleanup is not None:
            raise NativeLinuxGateError("retained tool cleanup was not confirmed") from first_cleanup
    print("Requested native Linux subset passed: Rust, pure WP, real HTTP/Unix signals and selected Python tests.")
    print("No DB/PHP/reference/whole-evidence/installation/CI/deployment proof was produced.")


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--plan", action="store_true", help="print the limited scope without starting any tool/server")
    options = parser.parse_args(argv)
    if options.plan:
        print("PLAN ONLY: no tools/servers are started; execution requires native Linux and Python >=3.11.")
        print("Preinstalled Rust/Python, offline dependencies, linker and loopback are caller prerequisites.")
        print("rustc -vV; cargo --version; CARGO_BUILD_TARGET=<reported Linux host>; target-dir retained.")
        for command in RUST_CHECKS:
            print("  " + " ".join(command))
        print("  " + sys.executable + " " + str(PURE_WP_SCRIPT) + " (native no-default WP controller)")
        print("  " + " ".join(BUILD) + " (bounded strict JSON -> unique actual artifact)")
        print("  smoke.main() with smoke.BINARY bound to that artifact (real HTTP + Unix signals)")
        print("  selected Python: " + ", ".join(SELECTED_TESTS))
        print("No install/fetch/CI, DB/PHP/reference/full evidence, production deployment or untrusted-runner attestation.")
        return
    run_gate()


if __name__ == "__main__":
    try:
        main()
    except subprocess.CalledProcessError as error:
        raise SystemExit(error.returncode)
    except (NativeLinuxGateError, OSError, UnicodeError, subprocess.TimeoutExpired) as error:
        print("Native Linux gate failed: " + str(error), file=sys.stderr)
        raise SystemExit(1)
