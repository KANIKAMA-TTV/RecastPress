"""Verify a fixed 256MiB PHP linear-memory cap and an uncapped control.

The expected OOM case remains a failed execution. Only the four-case diagnostic
can pass; this does not select a product limit or establish an RSS limit.
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
from reuse_check import ARTIFACT_SHA256, equal, require

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

CASES = ['capped_normal', 'capped_allocation_oom', 'capped_normal_recovery',
         'uncapped_allocation_control']
READY = 'RP_PHP_MEMORY_ALLOCATION_READY memory_limit=-1\n'


def validate_result(value, pid, backend):
    equal(value['schema'], 'recastpress.php-wasix-memory-cap.v0.5', 'host protocol')
    equal(value['process_pid'], pid, 'owned host PID')
    equal(value['engine'], 'wasmer-7.2.0/' + backend, 'compiler backend')
    equal(value['phase'], 'php_memory_cap_diagnostic_verified', 'completion phase')
    for key in ['ok', 'diagnostic_verified', 'php_linear_memory_cap_verified_for_fixed_cases']:
        equal(value[key], True, key)
    for key in ['production_memory_limit_verified', 'rss_limit_verified',
                'compiler_memory_limit_verified', 'aggregate_memory_limit_verified',
                'native_comparison_performed', 'product_adopted', 'real_db',
                'wordpress_executed', 'full_php_compatibility_verified']:
        equal(value[key], False, key)
    equal(value['test_cap_pages'], 4096, 'trial page cap')
    equal(value['test_cap_bytes'], 256 * 1024 * 1024, 'trial byte cap')
    equal(value['allocation_bytes'], 128 * 1024 * 1024, 'allocation size')
    equal(value['artifact']['verified'], True, 'artifact verification')
    equal(value['artifact']['expected_bytes'], 39727549, 'artifact length')
    for key in ['expected_sha256', 'observed_sha256']:
        equal(value['artifact'][key], ARTIFACT_SHA256, 'artifact ' + key)
    isolation = value['isolation']
    for key in ['host_environment_forwarded', 'host_directories_mapped', 'home_mapped',
                'http_client', 'memory_limit_enforced', 'in_process_deadline_enforced']:
        equal(isolation[key], False, key)
    equal(isolation['networking'], 'UnsupportedVirtualNetworking', 'network implementation')
    equal(isolation['guest_environment_supplied'], [], 'guest environment')
    equal(isolation['guest_fixture_files'], [], 'guest files')
    reuse = value['reuse']
    for key, wanted in [('php_module_compile_invocations', 1), ('module_clone_count', 4),
                        ('request_path_module_compile_calls', 0), ('observed_cases', 4)]:
        equal(reuse[key], wanted, key)
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms'] >= 0,
            'invalid compilation duration')
    cases = value['cases']
    require(type(cases) is list and len(cases) == 4, 'expected exactly four cases')
    widths = []
    for index, case in enumerate(cases):
        equal(case['name'], CASES[index], 'case order')
        equal(case['configured_engine_identity'], reuse['configured_engine_identity'], 'same configured engine')
        equal(case['module_delivery'], 'Module.clone', 'compiled module reuse')
        equal(case['module_compile_invocations'], 0, 'per-case compilation')
        equal(case['cap_pages'], 4096 if index < 3 else None, 'case cap')
        equal(case['success_expected'], index != 1, 'expected execution outcome')
        equal(case['ok'], index != 1, 'actual execution outcome')
        equal(case['expectation_met'], True, 'case expectation')
        for key in ['host_environment_forwarded', 'host_directories_mapped', 'home_mapped',
                    'http_client', 'stdout_overflow', 'stderr_overflow']:
            equal(case[key], False, 'case ' + key)
        for key in ['stdout_eof', 'stderr_eof', 'runtime_shutdown_returned']:
            equal(case[key], True, 'case ' + key)
        equal(case['guest_environment_supplied'], [], 'case environment')
        equal(case['networking'], 'UnsupportedVirtualNetworking', 'case networking')
        equal(case['stdin'], 'closed-pipe', 'case stdin')
        for stream in ['stdout', 'stderr']:
            equal(case[stream + '_limit_bytes'], 65536, stream + ' bound')
            count = case[stream + '_bytes']
            require(type(count) is int and 0 <= count <= 65536, 'invalid ' + stream + ' count')
            equal(len(case[stream].encode('utf-8')), count, stream + ' encoded count')
        equal(case['memory_observation_count'], 1, 'safe recycle observation count')
        memory = case['memory']
        maximum = 4096 if index < 3 else 65536
        equal(memory['maximum_pages'], maximum, 'actual maximum pages')
        equal(memory['shared'], True, 'actual shared type')
        require(type(memory['size_pages']) is int and 403 <= memory['size_pages'] <= maximum,
                'actual memory size outside declared range')
        require(type(case['php_execution_elapsed_ms']) is int and case['php_execution_elapsed_ms'] >= 0,
                'invalid PHP execution duration')
        if index == 1:
            equal(case['oom_observed'], True, 'allocation OOM observation')
            equal(case['php'], None, 'OOM has no completed PHP response')
            require(type(case['execution_error']) is str and bool(case['execution_error']),
                    'expected failed execution must retain its error')
            require(case['stderr'].startswith(READY) and 'out of memory' in case['stderr'].lower(),
                    'failure was not allocation OOM with memory_limit=-1')
            continue
        equal(case['oom_observed'], False, 'unexpected OOM observation')
        php = case['php']
        equal(json.loads(case['stdout']), php, 'captured PHP response')
        width = php['php_int_size']
        require(type(width) is int and width in (4, 8), 'invalid PHP integer width')
        widths.append(width)
        allocation = index == 3
        equal(php, {'php_version': '8.5.12', 'php_int_size': width,
                    'kind': 'allocation' if allocation else 'normal', 'memory_limit': '-1',
                    'allocated_bytes': 128 * 1024 * 1024 if allocation else 0,
                    'first_byte': 120 if allocation else None,
                    'last_byte': 120 if allocation else None,
                    'echo': 'recastpress-v0.5-php-memory-cap'}, 'fixed PHP result')
        equal(case['stderr'], READY if allocation else '', 'case stderr')
        if allocation:
            require(memory['size_pages'] > 4096, 'uncapped allocation did not exceed the trial cap')
    equal(widths, [widths[0]] * 3, 'PHP width consistency')
    equal(value['observed_php_int_size'], widths[0], 'observed PHP width')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'backend': backend, 'scope': 'fixed four-case PHP imported-memory diagnostic only',
        'outcome': 'not_run', 'diagnostic_passed': False, 'production_compatible': False,
        'rss_limit_verified': False, 'compiler_memory_limit_verified': False,
        'aggregate_memory_limit_verified': False, 'product_adopted': False,
        'native_comparison_performed': False, 'real_db': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300,
        'expected_execution_results': [True, False, True, True],
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
        require(built.returncode == 0, 'PHP memory diagnostic build failed')
        executable = version_probe.select_artifact(built.stdout)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = version_probe.digest(executable)
        stage = 'execution'
        child_environment = {key: value for key, value in environment.items() if key.upper() not in {
            'GITHUB_TOKEN', 'GH_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE',
        }}
        pid, completed = owned_probe.run_captured_probe(
            [str(executable), '--php-memory-cap'], child_environment, timeout=300)
        report['owned_process_pid'] = pid
        report['exit_code'] = completed.returncode
        require(len(completed.stdout) <= 1024 * 1024 and len(completed.stderr) <= 64 * 1024,
                'memory diagnostic capture budget')
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
        require(completed.returncode == 0, 'PHP memory diagnostic failed')
        stage = 'verification'
        equal(phases, ['module_compile'] + CASES, 'one compile and four case phases')
        validate_result(value, pid, backend)
        report['outcome'] = 'php_memory_cap_diagnostic_passed'
        report['diagnostic_passed'] = True
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
            report['diagnostic_passed'] = False
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = str(error)[:8192]
            passed = False
    inputs = version_probe.compiled_inputs() + [HERE / name for name in ['check.py', 'reuse_check.py', 'memory_check.py']]
    inputs += [ROOT / 'scripts/owned_probe.py']
    report['input_files'] = {path.relative_to(ROOT).as_posix(): version_probe.digest(path)
                           for path in inputs if path.is_file()}
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-php-memory-cap.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX memory diagnostic: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
