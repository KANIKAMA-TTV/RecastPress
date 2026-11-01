"""Supervise a fixed PHP Wasm-operation-budget trial and preserve loop failure.

The weighted Wasm points are neither CPU seconds nor PHP opcodes. No arbitrary
code, argument, path, product limit or native PHP fallback is accepted.
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

import check as host
from language_check import equal, parse_json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

POINTS = 100_000_000
ARTIFACT_SHA = 'e64850fde91d98dff0bce2b95ed467591376586469b1f4d8bad5c8759c4edbe4'
NORMAL_SHA = '8c6589c02c7142f7d12eedfce4294a601698e1dfdccbc3319561c35a8cf81f30'
LOOP_SHA = 'c2452884022076d2aa1441754dd64429be1850d21b191898d186beb2f27fa111'
READY = b'RP_PHP_OPERATION_BUDGET_READY\n'
CASES = ['normal_first', 'loop_exhausted', 'normal_recovery']
require, digest = host.require, host.digest


def source_hashes():
    paths = host.compiled_inputs() + [HERE / 'operation_check.py', HERE / 'check.py',
                                     HERE / 'language_check.py', ROOT / 'scripts/owned_probe.py']
    return {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}


def expected_php():
    return {'php_version': '8.5.12', 'version_id': 80512, 'php_int_size': 8, 'sapi': 'cli',
            'kind': 'normal', 'globals_before_present': False, 'globals_after': 'request-local',
            'echo': 'recastpress-v0.5-php-operation-budget'}


def validate_result(value, pid, backend):
    for field, expected in [
        ('schema', 'recastpress.php-wasix-operation-budget.v0.5'), ('process_pid', pid),
        ('engine', 'wasmer-7.2.0/' + backend), ('phase', 'php_operation_budget_diagnostic_verified'),
        ('ok', True), ('php_wasm_operation_budget_verified_for_fixed_cases', True),
        ('trial_points', POINTS), ('cost_policy', 'one-point-per-original-Wasm-operator'),
        ('module_start_section_present', False),
    ]:
        equal(value[field], expected, 'host ' + field)
    for field in ['production_operation_budget_verified', 'cpu_time_budget_verified', 'php_opcode_budget_verified',
                  'hostcall_timeout_verified', 'production_memory_limit_verified', 'fiber_compatibility_verified',
                  'native_comparison_performed', 'product_adopted', 'real_db', 'wordpress_executed',
                  'full_php_compatibility_verified', 'exceptions_executed']:
        equal(value[field], False, 'unverified scope ' + field)
    equal(value['artifact']['path'], '.reference/php-wasix-8.5.12/php.wasm', 'fixed CLI module path')
    equal(value['artifact']['verified'], True, 'fixed CLI identity verification')
    equal(value['artifact']['expected_bytes'], 39727549, 'CLI module byte size')
    for key in ['expected_sha256', 'observed_sha256']:
        equal(value['artifact'][key], ARTIFACT_SHA, 'module ' + key)
    equal(value['source']['commit'], 'f4bfae79d5c19028349082f707930cf0951d23b9', 'PHP source')
    isolation = value['isolation']
    for field in ['host_environment_forwarded', 'host_directories_mapped', 'home_mapped', 'http_client',
                  'memory_limit_enforced', 'in_process_deadline_enforced', 'other_virtual_root_writes_forbidden']:
        equal(isolation[field], False, 'isolation ' + field)
    equal(isolation['guest_environment_supplied'], [], 'no supplied environment')
    equal(isolation['guest_fixture_files'], [], 'no mounted fixture or host directory')
    equal(isolation['networking'], 'UnsupportedVirtualNetworking', 'no guest networking')
    equal(isolation['stdin'], 'closed-pipe', 'closed input')
    equal(isolation['configured_guest_max_threads'], 1, 'one guest thread')
    for stream in ['stdout', 'stderr']:
        equal(isolation[stream + '_limit_bytes'], 65536, 'guest ' + stream + ' budget')
    reuse = value['reuse']
    for field, expected in [('php_module_compile_invocations', 1), ('module_clone_count', 3),
                            ('request_path_module_compile_calls', 0), ('observed_cases', 3),
                            ('module_delivery', 'Module.clone')]:
        equal(reuse[field], expected, 'reuse ' + field)
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms'] >= 0, 'compile duration')
    require(type(value['cases']) is list and len(value['cases']) == 3, 'exactly three operation-budget cases required')
    for index, case in enumerate(value['cases']):
        loop = index == 1
        for field, expected in [
            ('name', CASES[index]), ('request_number', index + 1),
            ('configured_engine_identity', reuse['configured_engine_identity']), ('module_delivery', 'Module.clone'),
            ('module_compile_invocations', 0), ('fixed_program_sha256', LOOP_SHA if loop else NORMAL_SHA),
            ('guest_arguments', ['-n', '-r', '<fixed-loop>' if loop else '<fixed-normal>']),
            ('success_expected', not loop), ('ok', not loop), ('expectation_met', True),
            ('initial_points', {'kind': 'remaining', 'points': POINTS}),
            ('setup_observation_count', 1), ('recycle_observation_count', 1),
            ('metering_observation_error', None), ('ready_reached', loop), ('budget_exhausted', loop),
            ('runtime_shutdown_returned', True), ('runtime_shutdown_wait_limit_ms', 2000),
            ('stdout_eof', True), ('stderr_eof', True), ('stdout_overflow', False), ('stderr_overflow', False),
            ('guest_environment_supplied', []), ('host_environment_forwarded', False),
            ('host_directories_mapped', False), ('home_mapped', False), ('http_client', False),
            ('networking', 'UnsupportedVirtualNetworking'), ('stdin', 'closed-pipe'),
            ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536),
        ]:
            equal(case[field], expected, 'case ' + field)
        require('expectation_error' not in case, 'case expectation failure must not become success')
        require(type(case['php_execution_elapsed_ms']) is int and case['php_execution_elapsed_ms'] >= 0,
                'execution duration')
        captured = {}
        for stream in ['stdout', 'stderr']:
            wire = bytes.fromhex(case[stream + '_hex'])
            require(len(wire) <= 65536, 'guest capture budget')
            equal(case[stream + '_bytes'], len(wire), stream + ' exact count')
            equal(case[stream + '_sha256'], hashlib.sha256(wire).hexdigest(), stream + ' exact digest')
            captured[stream] = wire
        equal(captured['stderr'], b'', 'guest stderr')
        if loop:
            equal(case['final_points'], {'kind': 'exhausted'}, 'actual exhausted global')
            equal(captured['stdout'], READY, 'READY reached before the loop exhaustion')
            equal(case['php'], None, 'loop has no completed PHP response')
            require(type(case['execution_error']) is str and bool(case['execution_error']), 'loop must retain its Runner error')
        else:
            require('execution_error' not in case, 'normal execution must have no Runner error')
            points = case['final_points']
            require(type(points) is dict and set(points) == {'kind', 'points'}, 'normal remaining-points shape')
            equal(points['kind'], 'remaining', 'normal remaining kind')
            require(type(points['points']) is int and 0 < points['points'] < POINTS,
                    'normal must consume points and leave a positive remainder')
            php = parse_json(captured['stdout'])
            equal(php, expected_php(), 'normal PHP JSON/types and fresh globals')
            equal(case['php'], php, 'normal decoded/captured consistency')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'platform': {'system': platform.system(), 'machine': platform.machine()}, 'backend': backend,
              'scope': 'fixed PHP weighted-Wasm-operation-budget diagnostic only',
              'outcome': 'not_run', 'diagnostic_passed': False, 'owned_host_reaped': False,
              'owned_process_cleanup_completed': False,
              'php_wasm_operation_budget_verified_for_fixed_cases': False,
              'expected_execution_results': [True, False, True], 'trial_points': POINTS,
              'production_operation_budget_verified': False, 'cpu_time_budget_verified': False,
              'php_opcode_budget_verified': False, 'hostcall_timeout_verified': False,
              'memory_limit_verified': False, 'product_runtime_selected': False,
              'wordpress_executed': False, 'fiber_compatibility_verified': False,
              'full_php_compatibility_verified': False, 'native_comparison_performed': False,
              'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300}
    passed, stage = False, 'platform_verification'
    before = source_hashes()
    report['input_files'] = before
    try:
        require(platform.system() == 'Linux', 'formal-EH PHP operation-budget execution is restricted to Linux; Windows/MSVC remains unexecuted')
        stage = 'build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HERE / 'Cargo.toml'),
                   '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode:
            report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'operation-budget host build failed')
        executable = host.select_artifact(built.stdout)
        binary_sha = digest(executable)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = binary_sha
        require(source_hashes() == before, 'operation sources changed during compilation')
        stage = 'execution'
        child_environment = {key: value for key, value in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--php-operation-budget'], child_environment, timeout=300)
        report['owned_host_reaped'] = True
        report['owned_process_pid'], report['exit_code'] = pid, completed.returncode
        require(len(completed.stdout) <= 1024 * 1024 and len(completed.stderr) <= 64 * 1024, 'host capture budget')
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        phases = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
                  if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr:
            report['stderr_excerpt'] = host.diagnostic(completed.stderr, 4096)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated host JSON result required')
        value = parse_json(completed.stdout)
        report['native_result'] = value
        require(completed.returncode == 0, 'fixed operation-budget diagnostic failed')
        stage = 'verification'
        equal(phases, ['module_compile'] + CASES, 'one compilation and three fixed cases')
        validate_result(value, pid, backend)
        require(source_hashes() == before and digest(executable) == binary_sha, 'operation sources or host binary changed')
        report['outcome'] = 'php_operation_budget_diagnostic_passed'
        report['diagnostic_passed'], report['php_wasm_operation_budget_verified_for_fixed_cases'], passed = True, True, True
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline; this is not proof of metering exhaustion'
        if error.stderr:
            report['stderr_excerpt'] = host.diagnostic(error.stderr, 4096)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
            report['owned_process_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as error:
            passed = False
            report['diagnostic_passed'] = report['php_wasm_operation_budget_verified_for_fixed_cases'] = False
            report['outcome'], report['cleanup_diagnostics'] = 'cleanup_failed', str(error)[:8192]
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-php-operation-budget.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX operation-budget diagnostic: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
