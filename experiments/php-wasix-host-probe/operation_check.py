"""Supervise a fixed PHP Wasm-operation-budget trial and preserve loop failure.

The weighted Wasm points are neither CPU seconds nor PHP opcodes. No arbitrary
code, argument, path, product limit or native PHP fallback is accepted.
Start-section execution spends the compiled initial budget before setup reads
the remainder. That observed remainder is never reset or called the initial B.
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
OBSERVER_STRATEGY = 'setup-captured-typed-global-handles'
OBSERVER_IDENTITY = 'sha256-of-pinned-wasmer-7.2.0-Global-Debug'
require, digest = host.require, host.digest
HOST_TIMEOUT_SECONDS = {'cranelift': 300, 'llvm': 600}


def source_hashes():
    paths = host.compiled_inputs() + [HERE / 'operation_check.py', HERE / 'check.py',
                                     HERE / 'language_check.py', ROOT / 'scripts/owned_probe.py']
    return {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}


def observed_phases(stderr):
    return [line.split('=', 1)[1] for line in stderr.decode('utf-8', errors='replace').splitlines()
            if line.startswith('PHP_WASIX_PHASE=')]


def record_timeout_capture(report, error):
    stdout, stderr = error.stdout or b'', error.stderr or b''
    phases = observed_phases(stderr)
    report.update(observed_phases=phases, last_observed_phase=phases[-1] if phases else None,
        guest_case_phase_observed=any(phase in CASES for phase in phases),
        timeout_compiler_subphase_distinguished=False,
        partial_stdout_bytes=len(stdout), partial_stdout_sha256=hashlib.sha256(stdout).hexdigest(),
        stderr_bytes=len(stderr), stderr_sha256=hashlib.sha256(stderr).hexdigest())
    if stderr:
        report['stderr_excerpt'] = host.diagnostic(stderr, 4096)


CGI_FIXTURE = 'experiments/php-wasix-managed-access-probe/fixtures/operation_cgi_contract.php'
CGI_GUEST = '/fixture/operation_cgi_contract.php'
CGI_FIXTURE_SHA = '455204fd5af80680dca3b32de236bb22db2a212ab91f124fc2fb02b8007a1f88'


def operation_cgi_environment(index):
    return [['GATEWAY_INTERFACE', 'CGI/1.1'], ['SERVER_PROTOCOL', 'HTTP/1.1'],
        ['SERVER_SOFTWARE', 'recastpress-v0.5-fixed-operation-cgi-probe'], ['SERVER_NAME', 'fixture.invalid'],
        ['SERVER_PORT', '80'], ['REDIRECT_STATUS', '200'], ['REQUEST_METHOD', 'GET'],
        ['SCRIPT_FILENAME', CGI_GUEST], ['SCRIPT_NAME', '/__rp_operation.php'], ['DOCUMENT_ROOT', '/fixture'],
        ['QUERY_STRING', 'rp_operation_cgi=1'], ['REQUEST_URI', '/__rp_operation.php?rp_operation_cgi=1'],
        ['CONTENT_LENGTH', '0'], ['RP_OPERATION_CASE', 'loop' if index == 1 else 'normal']]


def parse_operation_cgi(wire):
    """Parse only this normal request's completed fixed 200 JSON response."""
    require(type(wire) is bytes and 0 < len(wire) <= 65536, 'operation CGI wire quota')
    newline = wire.find(b'\n')
    require(newline > 0, 'operation CGI newline')
    separator = b'\r\n' if wire[newline - 1:newline] == b'\r' else b'\n'
    boundary = wire.find(separator * 2)
    require(0 <= boundary and boundary + 2 * len(separator) <= 16384, 'operation CGI header terminator/quota')
    lines = wire[:boundary].split(separator)
    require(0 < len(lines) <= 64, 'operation CGI header count')
    fields = []
    for line in lines:
        require(line and line[:1] not in (b' ', b'\t') and b':' in line, 'invalid/folded operation CGI header')
        name, value = line.split(b':', 1)
        require(re.fullmatch(rb"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name) is not None, 'operation CGI header name')
        value = value.strip(b' \t')
        require(all(byte == 9 or 32 <= byte != 127 for byte in value), 'operation CGI control/mixed header newline')
        fields.append((name.decode('ascii').lower(), value))
    values = lambda name: [value for key, value in fields if key == name]
    statuses, lengths = values('status'), values('content-length')
    require(len(statuses) <= 1 and len(lengths) <= 1, 'operation CGI duplicate status/length')
    if statuses: require(re.fullmatch(rb'200(?: [^\r\n]*)?', statuses[0]) is not None, 'normal operation CGI status')
    body = wire[boundary + 2 * len(separator):]
    if lengths:
        require(re.fullmatch(rb'[0-9]+', lengths[0]) is not None and int(lengths[0]) == len(body), 'operation CGI body length')
    headers = [{'name': name, 'value_hex': value.hex()} for name, value in fields if name not in {'status', 'content-length'}]
    equal(headers, [{'name': 'x-powered-by', 'value_hex': b'PHP/8.5.12'.hex()},
        {'name': 'content-type', 'value_hex': b'application/json'.hex()}], 'fixed normal operation CGI headers')
    return {'status': 200, 'headers': headers, 'body_bytes': len(body), 'body_hex': body.hex(),
        'body_sha256': hashlib.sha256(body).hexdigest(), 'representation_length': None}, body


def expected_php(*, profile='cli'):
    require(profile in ['cli', 'managed-cgi'], 'closed operation entry profile')
    return {'php_version': '8.5.12', 'version_id': 80512, 'php_int_size': 8,
            'sapi': 'cli' if profile == 'cli' else 'cgi-fcgi',
            'kind': 'normal', 'globals_before_present': False, 'globals_after': 'request-local',
            'echo': 'recastpress-v0.5-php-operation-budget'}


def validate_handle_lifecycle(case):
    lifecycle = case['metering_handle_lifecycle']
    require(type(lifecycle) is dict and set(lifecycle) == {
        'strategy', 'capture_count', 'take_count', 'drop_count', 'retained_global_handles',
        'setup_global_identity_sha256', 'recycle_global_identity_sha256', 'identity_scheme',
        'same_store_verified', 'recycle_types_verified', 'released_before_prior_recycle',
        'prior_recycle_present', 'prior_recycle_calls', 'prior_recycle_returns',
        'instance_retained', 'environment_lookup_at_recycle', 'globals_reconstructed', 'global_setter_calls',
    }, 'captured-global lifecycle shape')
    for field, expected in [
        ('strategy', OBSERVER_STRATEGY), ('identity_scheme', OBSERVER_IDENTITY),
        ('capture_count', 1), ('take_count', 1), ('drop_count', 1), ('retained_global_handles', 0),
        ('same_store_verified', True), ('recycle_types_verified', True),
        ('released_before_prior_recycle', True), ('instance_retained', False),
        ('environment_lookup_at_recycle', False), ('globals_reconstructed', False), ('global_setter_calls', 0),
    ]:
        equal(lifecycle[field], expected, 'metering lifecycle ' + field)
    identity = lifecycle['setup_global_identity_sha256']
    require(type(identity) is str and len(identity) == 64
            and all(character in '0123456789abcdef' for character in identity), 'opaque setup-global identity')
    equal(lifecycle['recycle_global_identity_sha256'], identity, 'same setup/recycle global handles')
    require(type(lifecycle['prior_recycle_present']) is bool, 'prior-recycle presence')
    count = int(lifecycle['prior_recycle_present'])
    equal(lifecycle['prior_recycle_calls'], count, 'prior-recycle exact forwarding')
    equal(lifecycle['prior_recycle_returns'], count, 'prior-recycle returned')


def validate_result(value, pid, backend, *, profile='cli', provenance=None):
    require(profile in ['cli', 'managed-cgi'], 'closed operation validation profile')
    cgi = profile == 'managed-cgi'
    for field, expected in [
        ('schema', 'recastpress.php-wasix-managed-operation-host.v0.5' if cgi else 'recastpress.php-wasix-operation-budget.v0.5'), ('process_pid', pid),
        ('engine', 'wasmer-7.2.0/' + backend), ('phase', 'managed_operation_budget_verified' if cgi else 'php_operation_budget_diagnostic_verified'),
        ('ok', True), ('php_wasm_operation_budget_verified_for_fixed_cases', True),
        ('trial_points', POINTS), ('cost_policy', 'one-point-per-original-Wasm-operator'),
        ('module_start_section_present', True), ('budget_reset_count', 0),
        ('configured_initial_points', {'kind': 'remaining', 'points': POINTS}),
        ('start_execution_included', True), ('fresh_post_start_points_equal', True),
        ('setup_observation_phase', 'after-Instance.new-start-before-_initialize'),
        ('final_observer_strategy', OBSERVER_STRATEGY),
        ('final_observation_phase', 'original-recycle-store-after-guest-before-prior-recycle'),
    ]:
        equal(value[field], expected, 'host ' + field)
    start = value['module_start_function']
    require(type(start) is dict and set(start) == {'index', 'name', 'exported_names', 'imported'}, 'start metadata shape')
    require(type(start['index']) is int and 0 <= start['index'] <= 2**32 - 1, 'start function index')
    require(start['name'] is None or type(start['name']) is str, 'optional start name')
    require(type(start['exported_names']) is list and all(type(name) is str for name in start['exported_names']),
            'start export names')
    equal(start['imported'], False, 'start must be a metered local function')
    initializers = value['compiled_metering_initializers']
    equal(initializers['verified'], True, 'compiled global initializers verified through public ModuleInfo')
    rows = initializers['globals']
    require(type(rows) is list and len(rows) == 2, 'exactly two compiled metering globals')
    for row, name, kind, ty, number in [
        (rows[0], 'wasmer_metering_remaining_points', 'I64Const', 'mutable i64', POINTS),
        (rows[1], 'wasmer_metering_points_exhausted', 'I32Const', 'mutable i32', 0),
    ]:
        require(type(row) is dict and set(row) == {'export', 'global_index', 'type', 'initializer_kind', 'initializer_value'},
                'compiled global metadata shape')
        for key, expected in [('export', name), ('type', ty), ('initializer_kind', kind), ('initializer_value', number)]:
            equal(row[key], expected, 'compiled initializer ' + key)
        require(type(row['global_index']) is int and 0 <= row['global_index'] <= 2**32 - 1, 'compiled global index')
    require(rows[0]['global_index'] != rows[1]['global_index'], 'compiled budget globals alias')
    for field in ['production_operation_budget_verified', 'cpu_time_budget_verified', 'php_opcode_budget_verified',
                  'hostcall_timeout_verified', 'production_memory_limit_verified', 'fiber_compatibility_verified',
                  'native_comparison_performed', 'product_adopted', 'real_db', 'wordpress_executed',
                  'full_php_compatibility_verified', 'exceptions_executed']:
        equal(value[field], False, 'unverified scope ' + field)
    if cgi:
        require(type(provenance) is dict and type(provenance.get('module')) is dict, 'explicit managed operation provenance')
        equal(value['artifact']['path'], 'target/php-wasix-managed-access-probe/php-fiber-cgi.wasm', 'fixed managed CGI path')
        equal(value['artifact']['observed_bytes'], provenance['module']['bytes'], 'managed CGI module size')
        equal(value['artifact']['observed_sha256'], provenance['module']['sha256'], 'managed CGI module digest')
    else:
        require(provenance is None, 'CLI operation validation uses its fixed reference')
        equal(value['artifact']['path'], '.reference/php-wasix-8.5.12/php.wasm', 'fixed CLI module path')
        equal(value['artifact']['expected_bytes'], 39727549, 'CLI module byte size')
        for key in ['expected_sha256', 'observed_sha256']:
            equal(value['artifact'][key], ARTIFACT_SHA, 'module ' + key)
    equal(value['artifact']['verified'], True, 'fixed identity verification')
    equal(value['source']['commit'], 'f4bfae79d5c19028349082f707930cf0951d23b9', 'PHP source')
    isolation = value['isolation']
    for field in ['host_environment_forwarded', 'host_directories_mapped', 'home_mapped', 'http_client',
                  'memory_limit_enforced', 'in_process_deadline_enforced', 'other_virtual_root_writes_forbidden']:
        equal(isolation[field], False, 'isolation ' + field)
    equal(isolation['guest_environment_supplied'], None if cgi else [], 'closed supplied environment profile')
    equal(isolation['guest_fixture_files'], [CGI_GUEST] if cgi else [], 'fixed fixture profile')
    if cgi:
        for key, expected in [('guest_readonly_mounts', ['/fixture']), ('asynchronous_threading', False),
                ('deep_sleep', False), ('max_guest_threads', 1),
                ('guest_environment_policy', 'fixed CGI scaffold and closed normal/loop case')]:
            equal(isolation[key], expected, 'operation CGI isolation ' + key)
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
    reference = value['post_start_reference_points']
    require(type(reference) is int and 0 < reference <= POINTS, 'valid first post-start remaining points')
    for index, case in enumerate(value['cases']):
        validate_handle_lifecycle(case)
        loop = index == 1
        for field, expected in [
            ('name', CASES[index]), ('request_number', index + 1),
            ('configured_engine_identity', reuse['configured_engine_identity']), ('module_delivery', 'Module.clone'),
            ('module_compile_invocations', 0), ('fixed_program_sha256', LOOP_SHA if loop else NORMAL_SHA),
            ('guest_arguments', ['-n', '-d', 'fiber.stack_size=2097152', '-d', 'register_argc_argv=0'] if cgi else ['-n', '-r', '<fixed-loop>' if loop else '<fixed-normal>']),
            ('success_expected', not loop), ('ok', not loop), ('expectation_met', True),
            ('configured_initial_points', {'kind': 'remaining', 'points': POINTS}), ('budget_reset_count', 0),
            ('setup_observation_phase', 'after-Instance.new-start-before-_initialize'),
            ('post_start_points', {'kind': 'remaining', 'points': reference}),
            ('start_charged_points', POINTS - reference), ('post_start_difference_from_first', 0),
            ('post_start_matches_first', True),
            ('setup_observation_count', 1), ('recycle_observation_count', 1),
            ('metering_observation_error', None), ('ready_reached', loop), ('budget_exhausted', loop),
            ('runtime_shutdown_returned', True), ('runtime_shutdown_wait_limit_ms', 2000),
            ('stdout_eof', True), ('stderr_eof', True), ('stdout_overflow', False), ('stderr_overflow', False),
            ('guest_environment_supplied', operation_cgi_environment(index) if cgi else []), ('host_environment_forwarded', False),
            ('host_directories_mapped', False), ('home_mapped', False), ('http_client', False),
            ('networking', 'UnsupportedVirtualNetworking'), ('stdin', 'closed-pipe'),
            ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536),
        ]:
            equal(case[field], expected, 'case ' + field)
        require('initial_points' not in case, 'post-start observation must not be relabeled initial points')
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
            if cgi:
                equal(case['complete_cgi_response_verified'], False, 'exhausted loop is not a completed CGI response')
                require('response' not in case, 'no fabricated complete CGI response for loop')
        else:
            require('execution_error' not in case, 'normal execution must have no Runner error')
            points = case['final_points']
            require(type(points) is dict and set(points) == {'kind', 'points'}, 'normal remaining-points shape')
            equal(points['kind'], 'remaining', 'normal remaining kind')
            require(type(points['points']) is int and 0 < points['points'] < reference,
                    'normal must consume points after start and leave a positive remainder')
            body = captured['stdout']
            if cgi:
                equal(case['complete_cgi_response_verified'], True, 'normal complete CGI response')
                response, body = parse_operation_cgi(body)
                equal(case['response'], response, 'actual original CGI capture/decoded response identity')
            php = parse_json(body)
            equal(php, expected_php(profile=profile), 'normal PHP JSON/types and fresh globals')
            equal(case['php'], php, 'normal decoded/captured consistency')
        if cgi:
            equal(case['fresh_wasi_runtime'], True, 'fresh CGI operation runtime')
            equal(case['program_sha256'], CGI_FIXTURE_SHA, 'actual CGI fixture source identity')


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
              'budget_reset_count': 0, 'start_execution_included': True,
              'production_operation_budget_verified': False, 'cpu_time_budget_verified': False,
              'php_opcode_budget_verified': False, 'hostcall_timeout_verified': False,
              'memory_limit_verified': False, 'product_runtime_selected': False,
              'wordpress_executed': False, 'fiber_compatibility_verified': False,
              'full_php_compatibility_verified': False, 'native_comparison_performed': False,
              'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': HOST_TIMEOUT_SECONDS[backend],
              'outer_execution_timeout_scope': 'owned child wait for compilation and all cases; EOF and forced cleanup use separate finite waits',
              'outer_cleanup_wait_seconds': owned_probe.CLEANUP_TIMEOUT,
              'outer_io_join_wait_seconds': owned_probe.IO_JOIN_TIMEOUT,
              'normal_host_result_received': False}
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
        pid, completed = owned_probe.run_captured_probe([str(executable), '--php-operation-budget'], child_environment, timeout=HOST_TIMEOUT_SECONDS[backend])
        report['owned_host_reaped'] = True
        report['normal_host_result_received'] = True
        report['owned_host_reap_kind'] = 'normal-host-return'
        report['owned_process_pid'], report['exit_code'] = pid, completed.returncode
        require(len(completed.stdout) <= 1024 * 1024 and len(completed.stderr) <= 64 * 1024, 'host capture budget')
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        phases = observed_phases(completed.stderr)
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
        record_timeout_capture(report, error)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
            report['owned_process_cleanup_completed'] = True
            if report['outcome'] == 'execution_timeout':
                report['owned_host_reaped'] = True
                report['owned_host_reap_kind'] = 'forced-timeout-cleanup-without-normal-result'
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
