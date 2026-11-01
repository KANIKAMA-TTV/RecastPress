"""Compare three fixed WASIX language cases against the recorded native PHP.

No native PHP is executed here. WordPress hooks and filesystem cases are left
uncompared, with no WordPress or scratch directory supplied to the guest.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import subprocess
import sys

import check as version_probe

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

CASES = ['basic', 'exceptions', 'fiber']
UNCOMPARED = ['wordpress_hooks', 'scratch_io', 'readonly_io']
BASELINE = ROOT / 'experiments/php-runtime-candidate-probe/evidence/native-contract.json'
FIXTURE = ROOT / 'experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php'
FIXTURE_SHA256 = '5ab341e1207fc6bfb8634758bc38b6af7f6b60ba3c621c8acf52b603e5023080'
RUNTIME = {'version': '8.5.12', 'version_id': 80512, 'int_size': 8, 'sapi': 'cli'}
require = version_probe.require
digest = version_probe.digest


def json_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key: ' + key)
        result[key] = value
    return result


def parse_json(wire):
    def bad_number(_):
        raise ValueError('non-finite JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=json_object,
                      parse_constant=bad_number)


def equal(actual, expected, label):
    require(type(actual) is type(expected), 'contract type mismatch: ' + label)
    if isinstance(expected, dict):
        require(set(actual) == set(expected), 'contract fields mismatch: ' + label)
        for key, value in expected.items():
            equal(actual[key], value, label + '.' + key)
    elif isinstance(expected, list):
        require(len(actual) == len(expected), 'contract list length mismatch: ' + label)
        for index, value in enumerate(expected):
            equal(actual[index], value, label + '[' + str(index) + ']')
    else:
        require(actual == expected, f'contract mismatch: {label}; observed={actual!r}, expected={expected!r}')


def load_baseline():
    wire = BASELINE.read_bytes()
    require(0 < len(wire) <= 256 * 1024, 'native receipt budget')
    native = parse_json(wire)
    equal(native['wasm_executed'], False, 'native receipt execution kind')
    equal(native['native_reference']['version'], '8.5.12', 'native reference version')
    equal(native['platform']['system'], 'Windows', 'native reference platform')
    expected_paths = {
        'experiments/php-runtime-candidate-probe/native_contract.py',
        'experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php',
        'scripts/owned_probe.py',
        'experiments/php-wasi-host-probe/wordpress-fixture.json',
    }
    equal(set(native['input_files']), expected_paths, 'native receipt source set')
    for name, expected in native['input_files'].items():
        path = PurePosixPath(name)
        require(not path.is_absolute() and '\\' not in name and
                all(part not in ('', '.', '..') and ':' not in part for part in name.split('/')),
                'unsafe native receipt source path')
        source = ROOT.joinpath(*path.parts).resolve(strict=True)
        require(source.is_relative_to(ROOT.resolve()), 'native receipt source escaped workspace')
        require(digest(source) == expected, 'native baseline source changed: ' + name)
    require(digest(FIXTURE) == FIXTURE_SHA256, 'fixed fixture source changed')
    observation = native['observation']
    equal(observation['format_version'], 1, 'native observation format')
    equal(observation['runtime'], RUNTIME, 'native runtime identity')
    equal(observation['unexpected_output_hex'], '', 'native unexpected output')
    for name in CASES:
        equal(observation['cases'][name]['status'], 'ok', 'native ' + name + ' status')
        equal(observation['cases'][name]['warnings'], [], 'native ' + name + ' warnings')
    require({'php.exe', 'php8.dll'} <= set(native['native_reference']['verified_files']),
            'native reference is missing executable/core DLL identities')
    return native


def compare_cases(observed, native):
    equal(observed['format_version'], 1, 'observation format')
    equal(observed['runtime'], native['observation']['runtime'], 'runtime identity')
    equal(observed['unexpected_output_hex'], '', 'unexpected output')
    require(type(observed['cases']) is dict and set(observed['cases']) == set(CASES + UNCOMPARED),
            'missing or unexpected fixture cases')
    # Preserve each comparison even when a different case fails. This does not
    # change the all-three acceptance condition or the host execution verdict.
    results = {}
    for name in CASES:
        try:
            equal(observed['cases'][name], native['observation']['cases'][name], name)
        except ValueError as error:
            results[name] = {'equal_to_native': False, 'difference': str(error)}
        else:
            results[name] = {'equal_to_native': True}
    return results


def compare_observation(observed, native):
    results = compare_cases(observed, native)
    for name, result in results.items():
        require(result['equal_to_native'], result.get('difference', 'case differs: ' + name))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'backend': backend, 'scope': 'fixed language contract only', 'outcome': 'not_run',
        'compared_cases': CASES, 'uncompared_cases': UNCOMPARED,
        'comparison_passed': False, 'wordpress_executed': False,
        'filesystem_contract_compared': False, 'native_php_executed_by_this_helper': False,
        'production_compatible': False, 'memory_limit_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300,
    }
    passed = False
    stage = 'native_reference_verification'
    try:
        native = load_baseline()
        report['native_reference'] = {
            'receipt': BASELINE.relative_to(ROOT).as_posix(), 'receipt_sha256': digest(BASELINE),
            'platform': native['platform'], 'role': native['role'],
            'php_executable_sha256': native['native_reference']['verified_files']['php.exe'],
            'php_core_dll_sha256': native['native_reference']['verified_files']['php8.dll'],
        }
        report['expected_language_cases'] = {name: native['observation']['cases'][name] for name in CASES}
        stage = 'build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HERE / 'Cargo.toml'),
                   '--target-dir', str(version_probe.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode != 0:
            report['diagnostics'] = version_probe.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'language contract build failed')
        executable = version_probe.select_artifact(built.stdout)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = digest(executable)
        stage = 'execution'
        child_environment = {key: value for key, value in environment.items() if key.upper() not in {
            'GITHUB_TOKEN', 'GH_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE',
        }}
        pid, completed = owned_probe.run_captured_probe(
            [str(executable), '--language-contract'], child_environment, timeout=300)
        report['owned_process_pid'] = pid
        report['exit_code'] = completed.returncode
        require(len(completed.stdout) <= 256 * 1024 and len(completed.stderr) <= 64 * 1024,
                'language capture budget')
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        report['observed_phases'] = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
                                     if line.startswith('PHP_WASIX_PHASE=')]
        if completed.stderr:
            report['stderr_excerpt'] = version_probe.diagnostic(completed.stderr, 4096)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1,
                'host must emit exactly one terminated JSON result')
        value = parse_json(completed.stdout)
        report['native_result'] = value
        if type(value) is dict and 'php' in value:
            report['observation'] = value['php']
        equal(value['schema'], 'recastpress.php-wasix-language-contract.v0.5', 'host protocol')
        equal(value['process_pid'], pid, 'owned host PID')
        equal(value['engine'], 'wasmer-7.2.0/' + backend, 'compiler backend')
        equal(value['comparison_scope'], CASES, 'host comparison scope')
        equal(value['uncompared_cases'], UNCOMPARED, 'host uncompared cases')
        equal(value['native_comparison_performed'], False, 'host comparison claim')
        equal(value['wordpress_executed'], False, 'WordPress execution claim')
        equal(value['full_php_compatibility_verified'], False, 'full compatibility claim')
        equal(value['filesystem_contract_compared'], False, 'filesystem comparison claim')
        equal(value['fixture']['verified'], True, 'fixture verification')
        equal(value['fixture']['observed_sha256'], FIXTURE_SHA256, 'executed fixture identity')
        equal(value['artifact']['verified'], True, 'Wasm artifact verification')
        equal(value['isolation']['host_environment_forwarded'], False, 'environment forwarding')
        equal(value['isolation']['host_directories_mapped'], False, 'host mounts')
        equal(value['isolation']['home_mapped'], False, 'home mapping')
        equal(value['isolation']['networking'], 'UnsupportedVirtualNetworking', 'network implementation')
        equal(value['isolation']['http_client'], False, 'HTTP client')
        equal(value['isolation']['guest_fixture_files'], ['/fixture/runtime_contract.php'], 'guest files')
        equal(value['isolation']['guest_environment_supplied'], [], 'guest environment')
        equal(value['isolation']['wordpress_directory_supplied'], False, 'WordPress directory')
        equal(value['isolation']['scratch_directory_supplied'], False, 'scratch directory')
        stage = 'comparison'
        report['case_results'] = compare_cases(value['php'], native)
        require(completed.returncode == 0 and value['ok'] is True and
                value['phase'] == 'language_contract_observed',
                'host language contract validation failed: ' + str(value.get('error', 'no successful result')))
        equal(value['exceptions_executed'], True, 'exception case observation')
        equal(value['fiber_executed'], True, 'Fiber case observation')
        compare_observation(value['php'], native)
        report['comparison_passed'] = True
        report['outcome'] = 'language_contract_passed'
        passed = True
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = f'owned {stage} child exceeded its finite deadline'
        if error.stderr:
            report['stderr_excerpt'] = version_probe.diagnostic(error.stderr, 4096)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            passed = False
            report['comparison_passed'] = False
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = str(error)[:8192]
    inputs = version_probe.compiled_inputs() + [HERE / 'check.py', HERE / 'language_check.py',
              ROOT / 'scripts/owned_probe.py', BASELINE,
              ROOT / 'experiments/php-runtime-candidate-probe/artifact.json']
    report['input_files'] = {path.relative_to(ROOT).as_posix(): digest(path) for path in inputs if path.is_file()}
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-language-contract.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX language contract: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
