"""Build and supervise one fixed version-only PHP WASIX candidate.

Failures are recorded as failures. This does not establish a product worker's
memory/CPU limits; the deadline belongs to the owned outer process supervisor.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

NAME = 'recastpress-php-wasix-host-probe'
TARGET = ROOT / 'target/php-wasix-host-probe'


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for data in iter(lambda: source.read(1024 * 1024), b''):
            value.update(data)
    return value.hexdigest()


def compiled_inputs():
    """Local compile inputs shared by every closed host mode, including embeds."""
    return ([HERE / 'Cargo.toml', HERE / 'Cargo.lock']
            + sorted((HERE / 'src').glob('*.rs'))
            + sorted((HERE / 'fixtures').glob('*.php'))
            + [ROOT / 'experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php',
               ROOT / 'experiments/php-wasix-cgi-build/manifest.json',
               ROOT / 'experiments/php-wasix-context-probe/manifest.json',
               ROOT / 'experiments/php-wasix-context-probe/fixtures/context_contract.c',
               ROOT / 'experiments/php-wasi-host-probe/src/cgi_response.rs'])


def require(condition, message):
    if not condition:
        raise ValueError(message)


def diagnostic(wire, limit):
    text = wire.decode('utf-8', errors='replace').replace('\\', '/')
    cargo_home = Path(os.environ.get('CARGO_HOME', str(Path.home() / '.cargo')))
    for path, label in [(ROOT, '<workspace>'), (cargo_home, '<cargo-cache>'), (Path.home(), '<home>')]:
        text = text.replace(path.as_posix(), label)
    return text[-limit:]


def select_artifact(wire):
    require(0 < len(wire) <= 16 * 1024 * 1024, 'invalid Cargo output budget')
    artifacts, finished = [], False
    for line in wire.splitlines():
        item = json.loads(line)
        require(not finished, 'Cargo output after completion')
        if item.get('reason') == 'build-finished':
            require(item.get('success') is True, 'Cargo did not finish successfully')
            finished = True
        if item.get('reason') != 'compiler-artifact' or item.get('target', {}).get('name') != NAME:
            continue
        require(item['target']['kind'] == ['bin'] and item['profile']['test'] is False, 'wrong binary target')
        require(Path(item['manifest_path']).resolve() == HERE / 'Cargo.toml', 'wrong binary manifest')
        require(Path(item['target']['src_path']).resolve() == HERE / 'src/main.rs', 'wrong binary source')
        executable = Path(item['executable']).resolve(strict=True)
        require(executable.is_relative_to(TARGET.resolve()), 'binary outside fixed target')
        artifacts.append(executable)
    require(finished and len(artifacts) == 1, 'missing or ambiguous binary')
    return artifacts[0]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    environment = os.environ.copy()
    command = ['cargo', 'build', '--release', '--manifest-path', str(HERE / 'Cargo.toml'),
               '--target-dir', str(TARGET), '--locked', '--offline', '--message-format=json']
    if args.llvm:
        command += ['--no-default-features', '--features', 'llvm']
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'backend': backend, 'outcome': 'not_run', 'php_version_verified': False,
        'production_compatible': False, 'memory_limit_verified': False,
        'outer_execution_timeout_seconds': 300,
    }
    passed = False
    try:
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        if built.returncode != 0:
            report['outcome'] = 'build_failed'
            report['build_exit_code'] = built.returncode
            report['diagnostics'] = diagnostic(built.stderr, 8192)
        else:
            executable = select_artifact(built.stdout)
            report['native_artifact'] = executable.relative_to(ROOT).as_posix()
            report['native_artifact_sha256'] = digest(executable)
            child_environment = {key: value for key, value in environment.items() if key.upper() not in {
                'GITHUB_TOKEN', 'GH_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE',
            }}
            try:
                pid, completed = owned_probe.run_captured_probe([str(executable)], child_environment, timeout=300)
                report['owned_process_pid'] = pid
                report['exit_code'] = completed.returncode
                require(len(completed.stdout) <= 256 * 1024 and len(completed.stderr) <= 64 * 1024, 'native capture budget')
                report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
                if completed.stderr:
                    report['stderr_excerpt'] = diagnostic(completed.stderr, 4096)
                report['observed_phases'] = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
                                             if line.startswith('PHP_WASIX_PHASE=')]
                if completed.stdout:
                    try:
                        value = json.loads(completed.stdout)
                        require(type(value) is dict and value['schema'] == 'recastpress.php-wasix-version-only.v0.5', 'unknown native protocol')
                        require(type(value['process_pid']) is int and value['process_pid'] == pid, 'wrong native process')
                        require(value['engine'] == 'wasmer-7.2.0/' + backend, 'wrong compiler backend')
                        report['native_result'] = value
                        passed = completed.returncode == 0 and value['ok'] is True
                        if passed:
                            require(value['phase'] == 'version_only_verified' and value['php']['php_version'] == '8.5.12', 'version was not verified')
                            require(value['full_php_compatibility_verified'] is False, 'version-only became full compatibility')
                            require(value['artifact']['verified'] is True, 'unverified artifact')
                            report['php_version_verified'] = True
                            baseline = ROOT / 'experiments/php-runtime-candidate-probe/evidence/native-contract.json'
                            if baseline.exists():
                                native = json.loads(baseline.read_text(encoding='utf-8'))
                                for path, expected in native['input_files'].items():
                                    require(digest(ROOT / path) == expected, 'native baseline source changed')
                                width = native['observation']['runtime']['int_size']
                                report['integer_width_comparison'] = {'native_bytes': width, 'wasm_bytes': value['php']['php_int_size'],
                                                                      'equal': width == value['php']['php_int_size'], 'reference_sha256': digest(baseline)}
                        report['outcome'] = 'version_only_passed' if passed else 'execution_failed'
                    except (ValueError, KeyError, TypeError) as error:
                        passed = False
                        report['outcome'] = 'invalid_native_result'
                        report['diagnostics'] = str(error)
                else:
                    report['outcome'] = 'process_failed_without_result'
            except subprocess.TimeoutExpired as error:
                report['outcome'] = 'execution_timeout'
                if error.stderr:
                    report['stderr_excerpt'] = diagnostic(error.stderr, 4096)
                    report['observed_phases'] = [line.split('=', 1)[1]
                                                for line in error.stderr.decode('utf-8', errors='replace').splitlines()
                                                if line.startswith('PHP_WASIX_PHASE=')]
    except subprocess.TimeoutExpired as error:
        report['outcome'] = 'build_timeout'
        if error.stderr:
            report['diagnostics'] = diagnostic(error.stderr, 8192)
    finally:
        owned_probe.reap_owned_probes()
    inputs = compiled_inputs() + [HERE / 'check.py', ROOT / 'scripts/owned_probe.py']
    report['input_files'] = {path.relative_to(ROOT).as_posix(): digest(path) for path in inputs}
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        name = platform.system().lower() + '-' + backend + '.json'
        (directory / name).write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX version-only candidate: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (Exception, KeyboardInterrupt) as error:
        print(f'PHP WASIX candidate check failed: {error}', file=sys.stderr)
        sys.exit(1)
