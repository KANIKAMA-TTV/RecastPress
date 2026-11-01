"""Build fixed P0 tests, then supervise their executable with a short deadline.

The guest infinite-loop tests must not outlive a failed middleware check.
This establishes neither a PHP CPU budget nor product request deadlines.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

import check as host
from language_check import equal, parse_json

ROOT, HERE = host.ROOT, host.HERE
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

require = host.require


def select_test(wire):
    require(0 < len(wire) <= 16 * 1024 * 1024, 'Cargo capture budget')
    selected, finished = [], False
    for line in wire.splitlines():
        item = json.loads(line)
        require(not finished, 'Cargo output after completion')
        if item.get('reason') == 'build-finished':
            require(item.get('success') is True, 'Cargo build failed')
            finished = True
        if item.get('reason') != 'compiler-artifact' or item.get('target', {}).get('name') != host.NAME:
            continue
        require(item['target']['kind'] == ['bin'] and item['profile']['test'] is True, 'wrong test target')
        require(Path(item['manifest_path']).resolve() == HERE / 'Cargo.toml', 'wrong test manifest')
        require(Path(item['target']['src_path']).resolve() == HERE / 'src/main.rs', 'wrong test source')
        executable = Path(item['executable']).resolve(strict=True)
        require(executable.is_relative_to(host.TARGET.resolve()), 'test outside fixed target')
        selected.append(executable)
    require(finished and len(selected) == 1, 'missing or ambiguous test executable')
    return selected[0]


def source_hashes():
    paths = host.compiled_inputs() + [Path(__file__).resolve(), HERE / 'check.py',
                                    HERE / 'language_check.py', ROOT / 'scripts/owned_probe.py']
    return {p.relative_to(ROOT).as_posix(): host.digest(p) for p in paths}


def validate_metering_rows(rows, backend, system):
    tests = ['plain'] + (['formal_eh'] if system == 'Linux' else [])
    require(len(rows) == 7 * len(tests), 'missing WAT metering observations')
    require({row['test'] for row in rows} == set(tests), 'unexpected WAT metering tests')
    summaries = []
    for name in tests:
        group = [row for row in rows if row['test'] == name]
        equal([r['event'] for r in group], ['case_start', 'case_result'] * 3 + ['test_complete'], 'metering events')
        for row in group:
            for key, expected in [('schema', 'recastpress.wasmer-metering-wat.v0.5'),
                                  ('engine', 'wasmer-7.2.0/' + backend), ('formal_eh', name == 'formal_eh'),
                                  ('php_executed', False)]:
                equal(row[key], expected, 'metering ' + key)
        summary = group[-1]
        for key, expected in [('ok', True), ('configured_budget', 10000), ('explicit_module_compile_calls', 1),
                              ('fresh_instances', 3), ('normal_recovery_verified', True),
                              ('hostcall_timeout_verified', False), ('cpu_wall_clock_deadline_verified', False),
                              ('cost_policy', 'one-point-per-original-Wasm-operator')]:
            equal(summary[key], expected, 'metering summary ' + key)
        equal(len(summary['observations']), 3, 'metering instance count')
        for index, case in enumerate(['normal_first', 'infinite_loop', 'normal_recovery']):
            start, result = group[2 * index:2 * index + 2]
            observation = result['observation']
            equal(summary['observations'][index], observation, 'metering raw result/summary consistency')
            for row in [start, observation]:
                equal(row['case'], case, 'metering case')
                equal(row['instance_number'], index + 1, 'fresh instance number')
                equal(row['initial_points'], {'kind': 'remaining', 'points': 10000}, 'fresh exact budget')
            equal(observation['verified'], True, 'metering observation verified')
            looping = index == 1
            equal(observation['call_failed'], looping, 'metering call failure')
            equal(observation['runtime_trap_observed'], looping, 'actual runtime trap')
            equal(observation['normal_result'], None if looping else 42, 'normal result')
            equal(observation['entered'], 1 if looping else 0, 'loop/catch state')
            if looping:
                equal(observation['final_points'], {'kind': 'exhausted'}, 'actual exhausted state')
            else:
                final = observation['final_points']
                equal(final['kind'], 'remaining', 'normal remainder type')
                require(type(final['points']) is int and 0 < final['points'] < 10000, 'normal consumed points')
        summaries.append(summary)
    return summaries


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'backend': backend, 'platform': {'system': platform.system(), 'machine': platform.machine()},
        'scope': 'fixed Rust filesystem, parser, memory and WAT metering tests only',
        'outcome': 'not_run', 'tests_passed': False, 'owned_test_reaped': False,
        'php_executed': False, 'php_cpu_budget_verified': False, 'product_runtime_selected': False,
        'outer_build_timeout_seconds': 1200, 'outer_list_timeout_seconds': 10,
        'outer_test_timeout_seconds': 30,
    }
    passed, stage = False, 'build'
    try:
        before = source_hashes()
        report['input_files'] = before
        environment = os.environ.copy()
        command = ['cargo', 'test', '--release', '--no-run', '--manifest-path', str(HERE / 'Cargo.toml'),
                   '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode:
            report['diagnostics'] = host.diagnostic(built.stderr, 8192)
            require(len(built.stdout) <= 16 * 1024 * 1024, 'failed Cargo capture budget')
            messages = [item['message']['rendered'] for line in built.stdout.splitlines()
                        if (item := json.loads(line)).get('reason') == 'compiler-message'
                        and item.get('message', {}).get('level') == 'error'
                        and isinstance(item.get('message', {}).get('rendered'), str)]
            report['compiler_error_excerpt'] = host.diagnostic('\n'.join(messages).encode('utf-8'), 8192)
        require(built.returncode == 0, 'P0 test build failed')
        executable = select_test(built.stdout)
        report['test_artifact'] = executable.relative_to(ROOT).as_posix()
        binary_sha = host.digest(executable)
        report['test_artifact_sha256'] = binary_sha
        child_env = {k: v for k, v in environment.items() if k.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        stage = 'list'
        pid, listed = owned_probe.run_captured_probe(
            [str(executable), '--list', '--format', 'terse'], child_env, timeout=10)
        report['owned_list_pid'], report['list_exit_code'] = pid, listed.returncode
        require(listed.returncode == 0 and len(listed.stdout) <= 65536 and not listed.stderr,
                'test listing failed or exceeded capture budget')
        names = [line[:-6] for line in listed.stdout.decode('utf-8').splitlines() if line.endswith(': test')]
        require(platform.system() in ['Linux', 'Windows'], 'only Linux and Windows unit inventories are fixed')
        expected = {'cgi_contract::cgi_response::tests::': 9, 'language_contract::tests::': 2,
                    'context_contract::tests::': 1,
                    'fiber_adapter::tests::': 1, 'runtime_fixture_fs::tests::': 4,
                    'memory_limits::': 3, 'rpc_fs::tests::': 9,
                    'metering_limits::': 1 if platform.system() == 'Windows' else 2}
        require(len(names) == len(set(names)) == sum(expected.values()), 'unexpected test count')
        for prefix, count in expected.items():
            require(sum(name.startswith(prefix) for name in names) == count, 'missing tests: ' + prefix)
        report['listed_tests'] = names
        stage = 'execution'
        pid, completed = owned_probe.run_captured_probe(
            [str(executable), '--test-threads=1', '--show-output'], child_env, timeout=30)
        report['owned_test_pid'], report['exit_code'] = pid, completed.returncode
        report['owned_test_reaped'] = True
        require(len(completed.stdout) <= 128 * 1024 and len(completed.stderr) <= 65536, 'unit capture budget')
        report['stdout_sha256'] = hashlib.sha256(completed.stdout).hexdigest()
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        report['stdout_excerpt'] = host.diagnostic(completed.stdout, 16384)
        if completed.stderr:
            report['stderr_excerpt'] = host.diagnostic(completed.stderr, 8192)
        require(completed.returncode == 0, 'P0 unit executable failed')
        output = completed.stdout.decode('utf-8').replace('\r\n', '\n')
        ok_names = re.findall(r'^test (\S+) \.\.\. ok$', output, flags=re.MULTILINE)
        require(sorted(ok_names) == sorted(names), 'not every listed test passed')
        require(re.search(r'test result: ok\. ' + str(len(names)) + r' passed; 0 failed; 0 ignored;', output),
                'tests were failed or ignored')
        observations = [parse_json(line.split('=', 1)[1].encode('utf-8')) for line in output.splitlines()
                        if line.startswith('P0_METERING_JSON=')]
        report['wat_metering_observations'] = observations
        report['wat_metering_summaries'] = validate_metering_rows(observations, backend, platform.system())
        require(host.digest(executable) == binary_sha and source_hashes() == before, 'test binary or source changed')
        report['tests_passed'], passed = True, True
        report['outcome'] = 'fixed_p0_unit_tests_passed'
    except subprocess.TimeoutExpired:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline'
    except (OSError, ValueError, TypeError, KeyError, RuntimeError) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
        except RuntimeError as error:
            report['outcome'], passed, report['tests_passed'] = 'cleanup_failed', False, False
            report['cleanup_diagnostics'] = str(error)[:8192]
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(parents=True, exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-unit-contract.json')).write_text(
            json.dumps(report, indent=2) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX bounded unit tests: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
