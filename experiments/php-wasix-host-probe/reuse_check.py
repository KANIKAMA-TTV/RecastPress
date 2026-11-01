"""Verify one PHP module compilation and three fresh, sequential WASIX requests.

This does not compare against native PHP or verify product compatibility.
The existing owned-process supervisor bounds the complete host execution.
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

import check as version_probe

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

ARTIFACT_SHA256 = 'e64850fde91d98dff0bce2b95ed467591376586469b1f4d8bad5c8759c4edbe4'
KEYS = ['RP_MODULE_REUSE_REQUEST_' + str(number) for number in range(1, 4)]
MARKERS = ['recastpress-v0.5-module-reuse-request-' + str(number) for number in range(1, 4)]
PHASES = ['module_compile'] + ['module_reuse_request_' + str(number) for number in range(1, 4)]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def equal(observed, expected, label):
    require(type(observed) is type(expected), label + ' type mismatch')
    if type(expected) is dict:
        require(set(observed) == set(expected), label + ' keys mismatch')
        for key in expected:
            equal(observed[key], expected[key], label + '.' + key)
    elif type(expected) is list:
        require(len(observed) == len(expected), label + ' length mismatch')
        for index, (actual, wanted) in enumerate(zip(observed, expected)):
            equal(actual, wanted, label + '[' + str(index) + ']')
    else:
        require(observed == expected, label + ' mismatch')


def validate_result(value, pid, backend):
    equal(value['schema'], 'recastpress.php-wasix-module-reuse.v0.5', 'host protocol')
    equal(value['process_pid'], pid, 'owned process PID')
    equal(value['engine'], 'wasmer-7.2.0/' + backend, 'compiler backend')
    equal(value['phase'], 'module_reuse_verified', 'completion phase')
    equal(value['ok'], True, 'host completion')
    equal(value['module_reuse_verified'], True, 'module reuse verdict')
    equal(value['request_isolation_verified'], True, 'request isolation verdict')
    for key in ['native_comparison_performed', 'real_db', 'wordpress_executed',
                'full_php_compatibility_verified', 'exceptions_executed']:
        equal(value[key], False, key)
    equal(value['artifact']['verified'], True, 'artifact verification')
    equal(value['artifact']['expected_bytes'], 39727549, 'artifact byte count')
    equal(value['artifact']['expected_sha256'], ARTIFACT_SHA256, 'expected artifact identity')
    equal(value['artifact']['observed_sha256'], ARTIFACT_SHA256, 'observed artifact identity')
    isolation = value['isolation']
    for key in ['host_environment_forwarded', 'home_mapped', 'host_directories_mapped',
                'http_client', 'wordpress_directory_supplied', 'scratch_directory_supplied',
                'memory_limit_enforced', 'in_process_deadline_enforced']:
        equal(isolation[key], False, key)
    equal(isolation['networking'], 'UnsupportedVirtualNetworking', 'network implementation')
    equal(isolation['guest_fixture_files'], [], 'guest fixture files')
    equal(isolation['stdin'], 'closed-pipe', 'guest stdin')
    equal(isolation['stdout_limit_bytes'], 65536, 'stdout bound')
    equal(isolation['stderr_limit_bytes'], 65536, 'stderr bound')
    reuse = value['reuse']
    for key, expected in [('requested_requests', 3), ('php_module_compile_invocations', 1),
                          ('module_clone_count', 3), ('completed_requests', 3),
                          ('request_path_module_compile_calls', 0)]:
        equal(reuse[key], expected, key)
    equal(reuse['module_delivery'], 'Module.clone', 'compiled module delivery')
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms'] >= 0,
            'invalid compilation duration')
    requests = value['requests']
    require(type(requests) is list and len(requests) == 3, 'expected exactly three requests')
    widths = []
    for index, request in enumerate(requests):
        equal(request['request_number'], index + 1, 'request ordering')
        equal(request['configured_engine_identity'], reuse['configured_engine_identity'], 'shared engine')
        equal(request['module_delivery'], 'Module.clone', 'request module delivery')
        equal(request['module_compile_invocations'], 0, 'per-request compiler calls')
        equal(request['guest_environment_supplied'], [{'key': KEYS[index], 'value': MARKERS[index]}],
              'explicit request environment')
        for key in ['host_environment_forwarded', 'host_directories_mapped', 'home_mapped',
                    'http_client', 'stdout_overflow', 'stderr_overflow']:
            equal(request[key], False, 'request ' + key)
        for key in ['php_completed', 'isolation_verified', 'stdout_eof', 'stderr_eof',
                    'runtime_shutdown_returned']:
            equal(request[key], True, 'request ' + key)
        equal(request['networking'], 'UnsupportedVirtualNetworking', 'request network')
        equal(request['stdin'], 'closed-pipe', 'request stdin')
        equal(request['stdout_limit_bytes'], 65536, 'request stdout bound')
        equal(request['stderr_limit_bytes'], 65536, 'request stderr bound')
        equal(request['stderr'], '', 'request stderr')
        equal(request['stderr_bytes'], 0, 'request stderr count')
        require(type(request['stdout_bytes']) is int and 0 < request['stdout_bytes'] <= 65536,
                'invalid request stdout count')
        equal(len(request['stdout'].encode('utf-8')), request['stdout_bytes'], 'request stdout count')
        require(type(request['php_execution_elapsed_ms']) is int and
                request['php_execution_elapsed_ms'] >= 0, 'invalid request execution duration')
        php = request['php']
        equal(json.loads(request['stdout']), php, 'PHP capture consistency')
        width = php['php_int_size']
        require(type(width) is int and width in (4, 8), 'invalid PHP integer width')
        widths.append(width)
        expected_markers = {key: MARKERS[index] if key_index == index else False
                            for key_index, key in enumerate(KEYS)}
        equal(php, {
            'php_version': '8.5.12', 'php_int_size': width,
            'request_markers': expected_markers, 'request_marker_count': 1,
            'globals_before_present': False, 'globals_before_value': None,
            'globals_after': MARKERS[index], 'echo': 'recastpress-v0.5-module-reuse',
        }, 'request PHP observation')
    equal(widths, [widths[0]] * 3, 'PHP integer width consistency')
    equal(value['observed_php_int_size'], widths[0], 'observed PHP integer width')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'backend': backend, 'scope': 'one compiled PHP module, three fresh sequential requests',
        'outcome': 'not_run', 'reuse_passed': False, 'production_compatible': False,
        'native_comparison_performed': False, 'wordpress_executed': False,
        'memory_limit_verified': False, 'real_db': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300,
        'compile_counter_scope': 'explicit PHP Module::new call site, not internal backend instrumentation',
    }
    passed = False
    stage = 'build'
    try:
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HERE / 'Cargo.toml'),
                   '--target-dir', str(version_probe.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode != 0:
            report['diagnostics'] = version_probe.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'module reuse build failed')
        executable = version_probe.select_artifact(built.stdout)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = version_probe.digest(executable)
        stage = 'execution'
        child_environment = {key: value for key, value in environment.items() if key.upper() not in {
            'GITHUB_TOKEN', 'GH_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE',
        }}
        pid, completed = owned_probe.run_captured_probe(
            [str(executable), '--module-reuse'], child_environment, timeout=300)
        report['owned_process_pid'] = pid
        report['exit_code'] = completed.returncode
        require(len(completed.stdout) <= 512 * 1024 and len(completed.stderr) <= 64 * 1024,
                'module reuse capture budget')
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        phases = [line.split('=', 1)[1]
                  for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
                  if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr:
            report['stderr_excerpt'] = version_probe.diagnostic(completed.stderr, 4096)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1,
                'host must emit exactly one terminated JSON result')
        value = json.loads(completed.stdout)
        report['native_result'] = value
        require(completed.returncode == 0, 'module reuse host failed')
        stage = 'verification'
        equal(phases, PHASES, 'single compilation and three request phases')
        validate_result(value, pid, backend)
        report['reuse_passed'] = True
        report['outcome'] = 'module_reuse_passed'
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
            report['reuse_passed'] = False
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = str(error)[:8192]
    inputs = version_probe.compiled_inputs() + [HERE / 'check.py', HERE / 'reuse_check.py',
                                               ROOT / 'scripts/owned_probe.py']
    report['input_files'] = {path.relative_to(ROOT).as_posix(): version_probe.digest(path)
                           for path in inputs if path.is_file()}
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-module-reuse.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX module reuse: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
