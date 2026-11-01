"""Supervise four fixed real-PHP/private-Rust-RPC requests with one compilation.

The expected incomplete-frame request stays rejected despite PHP exiting zero.
No arbitrary PHP code/path, real database or standard wpdb interface is offered.
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

import check as host_probe
from language_check import equal, parse_json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

ARTIFACT_SHA = 'e64850fde91d98dff0bce2b95ed467591376586469b1f4d8bad5c8759c4edbe4'
CASES = ['normal_first', 'normal_second', 'incomplete_ignored', 'normal_recovery']
MARKERS = ['recastpress-v0.5-rpc-request-' + str(i) for i in range(1, 5)]
LITERAL = b'RP_P0 CACHE_SET 7\n\0\xff'
INCOMPLETE = b'RP_P0 CACHE_'
COMMANDS = [b'RP_P0 CACHE_GET\n', b'RP_P0 CACHE_SET 7\n', b'RP_P0 CACHE_GET\n',
            b'RP_P0 DB_SELECT_FIXTURE\n', b'RP_P0 CACHE_GET\n']
require, digest = host_probe.require, host_probe.digest


def expected_php(index, width):
    result = {'php_version': '8.5.12', 'php_int_size': width,
              'kind': 'incomplete_ignored' if index == 2 else 'normal', 'marker': MARKERS[index],
              'channel_closed': True, 'echo': 'recastpress-v0.5-private-rpc'}
    if index == 2:
        result.update(wire_hex=INCOMPLETE.hex(), written=len(INCOMPLETE), flushed=True)
    else:
        result.update(globals_before_present=False, globals_after=MARKERS[index],
                      cache_before={'value': None}, cache_set={'ok': True}, cache_after={'value': 7},
                      database={'rows': [{'nonce': 101 + index}]}, reopened_cache={'value': 7})
    return result


def expected_rpc(index):
    normal = index != 2
    replies = [{'value': None}, {'ok': True}, {'value': 7},
               {'rows': [{'nonce': 101 + index}]}, {'value': 7}]
    reply_bytes = sum(len(json.dumps(reply, separators=(',', ':')).encode('utf-8')) + 1 for reply in replies)
    return {
        'operations': 5 if normal else 0,
        'request_bytes': sum(map(len, COMMANDS)) if normal else len(INCOMPLETE),
        'response_bytes': reply_bytes if normal else 0,
        'response_read_bytes': reply_bytes if normal else 0,
        'pending_bytes': 0 if normal else len(INCOMPLETE),
        'cache_entries': 1 if normal else 0, 'channel_open': False, 'finished': True,
        'failure': None if normal else 'incomplete RPC frame on channel close',
        'real_db': False, 'standard_wpdb': False,
    }


def validate_result(value, pid, backend):
    for field, expected in [
        ('schema', 'recastpress.php-wasix-rpc-contract.v0.5'), ('process_pid', pid),
        ('engine', 'wasmer-7.2.0/' + backend), ('phase', 'rpc_contract_diagnostic_verified'),
        ('ok', True), ('rpc_contract_verified', True), ('request_isolation_verified', True),
        ('stdout_not_rpc_verified', True), ('incomplete_frame_rejected', True),
        ('standard_wpdb', False), ('real_db', False), ('wordpress_executed', False),
        ('native_comparison_performed', False), ('full_php_compatibility_verified', False),
        ('exceptions_executed', False), ('product_adopted', False),
    ]:
        equal(value[field], expected, 'host ' + field)
    equal(value['artifact']['path'], '.reference/php-wasix-8.5.12/php.wasm', 'fixed module path')
    equal(value['artifact']['verified'], True, 'fixed module verified')
    equal(value['artifact']['expected_bytes'], 39727549, 'module byte count')
    for field in ['expected_sha256', 'observed_sha256']:
        equal(value['artifact'][field], ARTIFACT_SHA, 'module ' + field)
    equal(value['source']['commit'], 'f4bfae79d5c19028349082f707930cf0951d23b9', 'PHP source')
    isolation = value['isolation']
    for field in ['host_environment_forwarded', 'home_mapped', 'host_directories_mapped', 'http_client',
                  'memory_limit_enforced', 'in_process_deadline_enforced', 'other_virtual_root_writes_forbidden']:
        equal(isolation[field], False, 'isolation ' + field)
    equal(isolation['networking'], 'UnsupportedVirtualNetworking', 'no network')
    equal(isolation['guest_private_mounts'], ['/rp-p0-rpc'], 'private mount')
    equal(isolation['rpc_channel'], '/rp-p0-rpc/channel', 'fixed channel')
    equal(isolation['guest_fixture_files'], [], 'no source file mount')
    equal(isolation['stdin'], 'closed-pipe', 'stdin')
    equal(isolation['stdout_limit_bytes'], 65536, 'stdout bound')
    equal(isolation['stderr_limit_bytes'], 65536, 'stderr bound')
    equal(value['rpc_limits'], {'frame_bytes': 128, 'single_write_bytes': 128, 'request_bytes': 1024,
                              'operations': 8, 'response_bytes': 1024,
                              'total_generated_response_bytes': 8192, 'reply_read_bytes': 8192,
                              'simultaneous_channels': 1}, 'RPC budgets')
    for field, expected in [('php_module_compile_invocations', 1), ('module_clone_count', 4),
                            ('observed_cases', 4), ('request_path_module_compile_calls', 0),
                            ('module_delivery', 'Module.clone')]:
        equal(value['reuse'][field], expected, 'reuse ' + field)
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms'] >= 0, 'compile duration')
    require(type(value['cases']) is list and len(value['cases']) == 4, 'four fixed cases required')
    widths = []
    for index, case in enumerate(value['cases']):
        normal = index != 2
        for field, expected in [('name', CASES[index]), ('request_number', index + 1), ('nonce', 101 + index),
                                ('configured_engine_identity', value['reuse']['configured_engine_identity']),
                                ('module_delivery', 'Module.clone'), ('module_compile_invocations', 0),
                                ('success_expected', normal), ('ok', normal), ('rpc_valid', normal),
                                ('expectation_met', True), ('php_completed', True),
                                ('runtime_shutdown_returned', True), ('runtime_shutdown_wait_limit_ms', 2000),
                                ('stdout_eof', True), ('stderr_eof', True), ('stdout_overflow', False),
                                ('stderr_overflow', False), ('host_environment_forwarded', False),
                                ('host_directories_mapped', False), ('home_mapped', False), ('http_client', False),
                                ('networking', 'UnsupportedVirtualNetworking'), ('private_rpc_mount', '/rp-p0-rpc'),
                                ('rpc_channel', '/rp-p0-rpc/channel'), ('stdin', 'closed-pipe'),
                                ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536),
                                ('stderr_bytes', 0), ('stderr_hex', '')]:
            equal(case[field], expected, 'case ' + field)
        equal(case['guest_environment_supplied'], [['RP_RPC_REQUEST_MARKER', MARKERS[index]]], 'fixed marker')
        require('execution_error' not in case and 'expectation_error' not in case, 'unexpected case failure')
        require(type(case['php_execution_elapsed_ms']) is int and case['php_execution_elapsed_ms'] >= 0,
                'execution duration')
        equal(case['rpc'], expected_rpc(index), 'actual Rust callback state')
        if normal:
            require('rpc_error' not in case, 'normal request has retained RPC error')
        else:
            require(type(case['rpc_error']) is str and 'incomplete RPC frame' in case['rpc_error'],
                    'expected incomplete rejection must retain its error')
        wire = bytes.fromhex(case['stdout_hex'])
        equal(len(wire), case['stdout_bytes'], 'stdout byte count')
        require(0 < len(wire) <= 65536, 'stdout capture budget')
        prefix = LITERAL if normal else b''
        equal(case['stdout_literal_hex'], prefix.hex(), 'literal output marker')
        require(wire.startswith(prefix), 'literal stdout bytes missing')
        php = parse_json(wire[len(prefix):])
        width = php['php_int_size']
        require(type(width) is int and width in (4, 8), 'PHP integer width')
        widths.append(width)
        equal(php, expected_php(index, width), 'PHP RPC observation')
        equal(case['php'], php, 'decoded PHP/raw stdout consistency')
    equal(widths, [widths[0]] * 4, 'PHP integer width consistency')
    equal(value['observed_php_int_size'], widths[0], 'observed PHP integer width')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'platform': {'system': platform.system(), 'machine': platform.machine()}, 'backend': backend,
              'scope': 'one module compile, four fixed private-RPC requests, fixture DB only',
              'outcome': 'not_run', 'rpc_contract_passed': False, 'owned_host_reaped': False,
              'expected_incomplete_case_accepted': False, 'product_runtime_selected': False,
              'wordpress_executed': False, 'real_db': False, 'standard_wpdb': False,
              'full_php_compatibility_verified': False, 'memory_limit_verified': False,
              'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300}
    passed, stage = False, 'build'
    try:
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HERE / 'Cargo.toml'),
                   '--target-dir', str(host_probe.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode != 0:
            report['diagnostics'] = host_probe.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'private RPC host build failed')
        executable = host_probe.select_artifact(built.stdout)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = digest(executable)
        stage = 'execution'
        child_environment = {key: val for key, val in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--rpc-contract'], child_environment, timeout=300)
        report['owned_host_reaped'] = True
        report['owned_process_pid'] = pid
        report['exit_code'] = completed.returncode
        require(len(completed.stdout) <= 512 * 1024 and len(completed.stderr) <= 64 * 1024, 'host output budget')
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        phases = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
                  if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr:
            report['stderr_excerpt'] = host_probe.diagnostic(completed.stderr, 4096)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated host JSON required')
        value = parse_json(completed.stdout)
        report['native_result'] = value
        require(completed.returncode == 0, 'fixed private RPC host execution failed')
        stage = 'verification'
        equal(phases, ['module_compile'] + CASES, 'one compilation/four cases')
        validate_result(value, pid, backend)
        report['rpc_contract_passed'], passed = True, True
        report['outcome'] = 'rpc_contract_diagnostic_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr:
            report['stderr_excerpt'] = host_probe.diagnostic(error.stderr, 4096)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = str(error)[:8192]
            report['rpc_contract_passed'], passed = False, False
    inputs = host_probe.compiled_inputs() + [HERE / name for name in ['check.py', 'language_check.py', 'rpc_check.py']]
    inputs += [ROOT / 'scripts/owned_probe.py']
    report['input_files'] = {path.relative_to(ROOT).as_posix(): digest(path) for path in inputs}
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-rpc-contract.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX private RPC: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
