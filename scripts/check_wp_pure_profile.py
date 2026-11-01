"""Check only the fixed no-default-features WP native package profile.

Installed tools, Cargo config/wrappers/runners and checkout are trusted. Output
caps are post-capture admission, not RSS caps. Owned cleanup targets captured
process handles, not an OS-enforced process tree for arbitrary tool descendants.
"""
import json
import os
from pathlib import Path
import re
import sys

import owned_probe

ROOT = Path(__file__).resolve().parents[1]
MANIFEST = ROOT / 'crates/recastpress-wp/Cargo.toml'
LIB_SOURCE = ROOT / 'crates/recastpress-wp/src/lib.rs'
TARGET_DIR = ROOT / 'target/wp-pure-profile'
PACKAGE = 'recastpress-wp'
LIB = 'recastpress_wp'
TIMEOUT = 600
MAX_OUTPUT = 16 * 1024 * 1024
MAX_DIAGNOSTICS = 4 * 1024 * 1024
FAILURE = 'WP pure profile check failed'


class PureProfileError(RuntimeError):
    def __init__(self):
        super().__init__(FAILURE)


def _object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise PureProfileError()
        result[key] = value
    return result


def _constant(_):
    raise PureProfileError()


def _json(raw):
    return json.loads(raw.decode('utf-8', errors='strict'), object_pairs_hook=_object, parse_constant=_constant)


def rust_host(raw):
    if type(raw) is not bytes or not raw or len(raw) > 16384:
        raise PureProfileError()
    hosts = [line[6:] for line in raw.decode('utf-8', errors='strict').splitlines() if line.startswith('host: ')]
    if len(hosts) != 1 or not re.fullmatch(r'[A-Za-z0-9_]+(?:-[A-Za-z0-9_.]+){2,}', hosts[0]):
        raise PureProfileError()
    return hosts[0]


def _artifact_path(value):
    if type(value) is not str:
        raise PureProfileError()
    path = Path(value)
    if not path.is_absolute() or not path.is_file():
        raise PureProfileError()
    path = path.resolve(strict=True)
    if not path.is_relative_to(TARGET_DIR.resolve()):
        raise PureProfileError()
    return path


def validate_cargo_output(raw, *, tests):
    """Validate Cargo's completed build prefix and native test summaries.

    Test-harness output follows build-finished; it is never parsed as Cargo JSON
    or used to select a guessed executable. Cargo itself runs the current tests.
    """
    if type(raw) is not bytes or not raw or len(raw) > MAX_OUTPUT or not raw.endswith(b'\n'):
        raise PureProfileError()
    finished = False
    libraries = []
    test_artifacts = 0
    harness = []
    for line in raw.splitlines():
        if finished:
            harness.append(line)
            continue
        message = _json(line)
        if type(message) is not dict or type(message.get('reason')) is not str:
            raise PureProfileError()
        reason = message['reason']
        if reason == 'build-finished':
            if message.get('success') is not True:
                raise PureProfileError()
            finished = True
            continue
        if reason != 'compiler-artifact':
            continue
        target = message.get('target')
        if type(target) is not dict:
            raise PureProfileError()
        manifest = message.get('manifest_path')
        is_wp = type(manifest) is str and Path(manifest).resolve() == MANIFEST.resolve()
        if target.get('name') == LIB and not is_wp:
            raise PureProfileError()
        if not is_wp:
            continue
        if message.get('features') != [] or type(message.get('features')) is not list:
            raise PureProfileError()
        profile = message.get('profile')
        source = target.get('src_path')
        if (type(profile) is not dict or type(profile.get('test')) is not bool
                or type(source) is not str or not Path(source).resolve().is_relative_to(MANIFEST.parent.resolve())):
            raise PureProfileError()
        filenames = message.get('filenames')
        if type(filenames) is not list or not filenames:
            raise PureProfileError()
        for filename in filenames:
            _artifact_path(filename)
        if profile['test']:
            if tests:
                _artifact_path(message.get('executable'))
            test_artifacts += 1
        if target.get('name') == LIB and target.get('kind') == ['lib'] and not profile['test']:
            if Path(source).resolve() != LIB_SOURCE.resolve() or message.get('executable') is not None:
                raise PureProfileError()
            libraries.append(message)
    if not finished or len(libraries) != 1:
        raise PureProfileError()
    passed = 0
    summaries = 0
    for line in harness:
        if line.startswith(b'test result:'):
            match = re.fullmatch(rb'test result: ok\. ([0-9]+) passed; 0 failed; [0-9]+ ignored; [0-9]+ measured; [0-9]+ filtered out; finished in [0-9.]+s', line)
            if not match:
                raise PureProfileError()
            passed += int(match[1])
            summaries += 1
    if tests and (test_artifacts == 0 or summaries == 0 or passed == 0):
        raise PureProfileError()
    if not tests and harness:
        raise PureProfileError()
    return {'wp_features': [], 'library_artifacts': 1, 'test_artifacts': test_artifacts,
            'test_summaries': summaries, 'passed': passed}


def validate_normal_graph(raw):
    if type(raw) is not bytes or not raw or len(raw) > MAX_OUTPUT or not raw.endswith(b'\n'):
        raise PureProfileError()
    names = []
    for line in raw.decode('utf-8', errors='strict').splitlines():
        match = re.fullmatch(r'([A-Za-z0-9_-]+) v[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9_.-]+)?(?: \([^\r\n]*\))?', line)
        if not match:
            raise PureProfileError()
        names.append(match[1])
    # All existing normal providers become optional behind auth. Serde used by
    # development tests is deliberately absent from --edges normal.
    if names != [PACKAGE]:
        raise PureProfileError()
    return names


def _run(command, environment, *, timeout=TIMEOUT):
    pid, result = owned_probe.run_captured_probe(command, environment, timeout=timeout)
    if (type(pid) is not int or pid <= 0 or type(result.returncode) is not int or result.returncode != 0
            or type(result.stdout) is not bytes or len(result.stdout) > MAX_OUTPUT
            or type(result.stderr) is not bytes or len(result.stderr) > MAX_DIAGNOSTICS):
        raise PureProfileError()
    return pid, result.stdout


def run_gate():
    original = None
    try:
        environment = os.environ.copy()
        rust_pid, metadata = _run(['rustc', '-vV'], environment, timeout=30)
        host = rust_host(metadata)
        environment['CARGO_BUILD_TARGET'] = host
        environment['CARGO_TERM_COLOR'] = 'never'
        common = ['--manifest-path', str(MANIFEST), '--package', PACKAGE, '--no-default-features',
                  '--target', host, '--offline', '--locked']
        test_pid, tested = _run(['cargo', 'test', *common, '--all-targets', '--target-dir', str(TARGET_DIR), '--message-format=json'], environment)
        test_summary = validate_cargo_output(tested, tests=True)
        clippy_pid, checked = _run(['cargo', 'clippy', *common, '--all-targets', '--target-dir', str(TARGET_DIR), '--message-format=json', '--', '-D', 'warnings'], environment)
        validate_cargo_output(checked, tests=False)
        graph_pid, graph = _run(['cargo', 'tree', *common, '--edges', 'normal', '--no-dedupe', '--prefix', 'none', '--format', '{p}'], environment, timeout=60)
        packages = validate_normal_graph(graph)
        report = {'format_version': 1, 'scope': 'wp_no_default_features_native_profile',
                  'host': host, 'wp_features': [], 'native_test_passed': test_summary['passed'],
                  'native_test_artifacts': test_summary['test_artifacts'], 'normal_packages': packages,
                  'command_process_pids': [rust_pid, test_pid, clippy_pid, graph_pid]}
    except BaseException as error:
        original = error
        raise
    finally:
        try:
            owned_probe.reap_owned_probes()
        except BaseException as cleanup:
            if original is not None:
                original.add_note('Pure WP cleanup retry failed: ' + type(cleanup).__name__)
            else:
                raise PureProfileError() from None
    return report


def main(argv=None):
    arguments = sys.argv[1:] if argv is None else list(argv)
    if arguments in (['--help'], ['-h']):
        print('Usage: python scripts/check_wp_pure_profile.py')
        return 0
    if arguments:
        print(FAILURE, file=sys.stderr)
        return 2
    try:
        wire = (json.dumps(run_gate(), sort_keys=True, separators=(',', ':')) + '\n').encode('ascii')
        if len(wire) > 2048:
            raise PureProfileError()
        output = getattr(sys.stdout, 'buffer', None)
        if output is None:
            sys.stdout.write(wire.decode('ascii'))
        else:
            output.write(wire)
        sys.stdout.flush()
        return 0
    except (Exception, KeyboardInterrupt):
        print(FAILURE, file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
