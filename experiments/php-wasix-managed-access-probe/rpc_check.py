"""Supervise four unchanged private-RPC cases on the three-patch CGI build.

The ignored incomplete frame remains rejected although PHP exits successfully.
CGI stdout is captured data, never a dispatch channel. No real DB, standard
wpdb, native PHP execution, permission-query claim or product ABI is supplied.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cgi = load('rp_managed_rpc_generated_cgi_gate', HERE / 'cgi_check.py')
managed = cgi.managed
original = load('rp_managed_rpc_original_expectations', HOST / 'rpc_check.py')
host, owned_probe = managed.host, managed.owned_probe
require, equal, digest, parse_json = managed.require, managed.equal, managed.digest, managed.parse_json
MODULE, RECEIPT, VARIANT = managed.MODULE, managed.RECEIPT, managed.VARIANT
FIXTURE = HERE / 'fixtures/rpc_cgi_contract.php'
CANONICAL = HOST / 'src/rpc_contract.rs'
GUEST = '/fixture/rpc_cgi_contract.php'
FIXTURE_SHA = '382d54a93357a480926255d7af8d69dadcedaabd34e6dade030ba932abdf5871'
SCHEMA = 'recastpress.php-wasix-managed-rpc-host.v0.5'
PHASE = 'managed_rpc_contract_verified'
SCOPE = 'three-patch-managed-candidate-four-CGI-private-RPC-requests'
HEAD = b'''<?php
// Closed CGI entry; the two original RPC program bodies below are unchanged.
if (PHP_SAPI !== 'cgi-fcgi'
    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'
    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_rpc_cgi=1'
    || ($_SERVER['argc'] ?? 0) !== 0) {
    throw new RuntimeException('Fixed RPC CGI request required');
}
$rp_case = getenv('RP_RPC_CASE');
if ($rp_case !== 'normal' && $rp_case !== 'incomplete_ignored') {
    throw new RuntimeException('Fixed RPC case required');
}
header('Content-Type: application/octet-stream');
if ($rp_case === 'normal') {'''
BODY_PINS = {'NORMAL': (1941, 'f7d9409fe4d8dd4a33fc5b7b27602d805767c11f074f9ccc5e1c56aa30fec6c8'),
    'INCOMPLETE': (556, 'ae9d8a08a45d0e019e054b646900d1f63e7ba3c86d24c6d38340dde8e83ee558')}


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def verify_fixture(read_source=None):
    source = managed.source_bytes(CANONICAL, read_source)
    bodies = {}
    for name, (size, identity) in BODY_PINS.items():
        matches = re.findall(rb'const ' + name.encode('ascii') + rb': &str = r#"(.*?)"#;', source, re.S)
        require(len(matches) == 1, 'one fixed original RPC program ' + name)
        bodies[name] = matches[0]
        equal(len(matches[0]), size, 'original RPC program bytes ' + name)
        equal(sha(matches[0]), identity, 'original RPC program identity ' + name)
    actual = managed.source_bytes(FIXTURE, read_source)
    equal(actual, HEAD + bodies['NORMAL'] + b'} else {' + bodies['INCOMPLETE'] + b'}\n',
        'only the fixed CGI entry wrapper differs from the two original RPC bodies')
    equal(sha(actual), FIXTURE_SHA, 'closed CGI RPC fixture identity')
    return {'source': FIXTURE.relative_to(ROOT).as_posix(), 'guest_path': GUEST,
        'expected_sha256': FIXTURE_SHA, 'verified': True,
        'canonical_source': CANONICAL.relative_to(ROOT).as_posix(),
        'case_bodies_byte_identical': True, 'observed_sha256': FIXTURE_SHA}


def source_hashes():
    verify_fixture()
    hashes = cgi.source_hashes()
    for path in [Path(__file__).resolve(), FIXTURE, CANONICAL, HOST / 'rpc_check.py']:
        require(path.resolve(strict=True) == path and path.is_relative_to(ROOT), 'RPC input outside workspace/linked')
        name, identity = path.relative_to(ROOT).as_posix(), digest(path)
        if name in hashes: equal(hashes[name], identity, 'shared RPC source identity')
        hashes[name] = identity
    return dict(sorted(hashes.items()))


def expected_environment(index):
    return [['GATEWAY_INTERFACE', 'CGI/1.1'], ['SERVER_PROTOCOL', 'HTTP/1.1'],
        ['SERVER_SOFTWARE', 'recastpress-v0.5-fixed-rpc-cgi-probe'], ['SERVER_NAME', 'fixture.invalid'],
        ['SERVER_PORT', '80'], ['REDIRECT_STATUS', '200'], ['REQUEST_METHOD', 'GET'],
        ['SCRIPT_FILENAME', GUEST], ['SCRIPT_NAME', '/__rp_rpc.php'], ['DOCUMENT_ROOT', '/fixture'],
        ['QUERY_STRING', 'rp_rpc_cgi=1'], ['REQUEST_URI', '/__rp_rpc.php?rp_rpc_cgi=1'],
        ['CONTENT_LENGTH', '0'], ['RP_RPC_REQUEST_MARKER', original.MARKERS[index]],
        ['RP_RPC_CASE', 'incomplete_ignored' if index == 2 else 'normal']]


def parse_cgi(wire):
    """Pure complete-response checks for this fixed binary-body 200 response."""
    require(type(wire) is bytes and 0 < len(wire) <= 65536, 'RPC CGI wire budget')
    newline = wire.find(b'\n')
    require(newline > 0, 'RPC CGI header newline')
    separator = b'\r\n' if wire[newline - 1:newline] == b'\r' else b'\n'
    boundary = wire.find(separator * 2)
    require(0 <= boundary and boundary + 2 * len(separator) <= 16384, 'RPC CGI header terminator/budget')
    lines = wire[:boundary].split(separator)
    require(0 < len(lines) <= 64, 'RPC CGI header count')
    fields = []
    for line in lines:
        require(line and line[:1] not in (b' ', b'\t') and b':' in line, 'invalid/folded RPC CGI header')
        name, value = line.split(b':', 1)
        require(re.fullmatch(rb"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name) is not None, 'RPC CGI header name')
        value = value.strip(b' \t')
        require(all(byte == 9 or 32 <= byte != 127 for byte in value), 'RPC CGI control/mixed header newline')
        fields.append((name.decode('ascii').lower(), value))
    values = lambda name: [value for key, value in fields if key == name]
    require(not any(key in {'connection', 'keep-alive', 'proxy-connection', 'te', 'trailer',
        'transfer-encoding', 'upgrade'} for key, _ in fields), 'RPC CGI hop-by-hop field')
    statuses, lengths = values('status'), values('content-length')
    require(len(statuses) <= 1 and len(lengths) <= 1, 'RPC CGI duplicate status/length')
    if statuses: require(re.fullmatch(rb'200(?: [^\r\n]*)?', statuses[0]) is not None, 'fixed RPC CGI status')
    body = wire[boundary + 2 * len(separator):]
    if lengths:
        require(re.fullmatch(rb'[0-9]+', lengths[0]) is not None and int(lengths[0]) == len(body),
            'RPC CGI completed Content-Length')
    headers = [{'name': name, 'value_hex': value.hex()} for name, value in fields if name not in {'status', 'content-length'}]
    equal(headers, [{'name': 'x-powered-by', 'value_hex': b'PHP/8.5.12'.hex()},
        {'name': 'content-type', 'value_hex': b'application/octet-stream'.hex()}], 'fixed RPC CGI response headers')
    return {'status': 200, 'headers': headers, 'body_bytes': len(body), 'body_hex': body.hex(),
        'body_sha256': sha(body), 'representation_length': None}, body


def validate_result(value, pid, backend, provenance, *, read_source=None, build_inputs=None):
    require(backend in ['cranelift', 'llvm'] and 'error' not in value, 'closed successful managed RPC host')
    managed.integer(pid, 1)
    managed.validate_provenance(provenance, read_source, build_inputs)
    for key, expected in [('schema', SCHEMA), ('scope', SCOPE), ('phase', PHASE), ('variant', VARIANT),
            ('process_pid', pid), ('engine', 'wasmer-7.2.0/' + backend), ('wasmer_wasix', '0.702.0'),
            ('ok', True), ('rpc_contract_verified', True), ('request_isolation_verified', True),
            ('stdout_not_rpc_verified', True), ('incomplete_frame_rejected', True), ('observed_php_int_size', 8),
            ('standard_wpdb', False), ('real_db', False), ('wordpress_executed', False),
            ('native_comparison_performed', False), ('full_php_compatibility_verified', False),
            ('exceptions_executed', False), ('product_adopted', False), ('product_abi_selected', False),
            ('product_runtime_selected', False), ('managed_access_verified_for_fixed_cases', False)]:
        equal(value[key], expected, 'managed RPC host ' + key)
    equal(value['source'], {'repository': 'wasix-org/php', 'commit': 'f4bfae79d5c19028349082f707930cf0951d23b9',
        'kind': 'same-job-patched-source-build', 'verified': True}, 'managed RPC source')
    for key, expected in [('path', MODULE.relative_to(ROOT).as_posix()), ('verified', True),
            ('identity_policy', 'same-job-three-source-patch-build'), ('observed_bytes', provenance['module']['bytes']),
            ('observed_sha256', provenance['module']['sha256'])]:
        equal(value['artifact'][key], expected, 'managed RPC module ' + key)
    for key, expected in [('path', RECEIPT.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_sha256', provenance['build_receipt_sha256']),
            ('manifest_sha256', provenance['builder_provenance']['manifest_sha256']),
            ('full_source_tool_chain_verified_by_host', False),
            ('chain_owner', 'managed_check.py verifies all source/tool/patch/module provenance')]:
        equal(value['build_receipt'][key], expected, 'managed RPC build ' + key)
    equal(value['source_files'], managed.sources([ROOT / name for name in managed.SOURCES], read_source), 'compiled managed sources')
    managed.fiber.validate_inspection(value['postopt_adapter'], provenance['module'])
    equal(value['fixture'], verify_fixture(read_source), 'compiled original-body CGI fixture')
    isolation = value['isolation']
    for key in ['host_environment_forwarded', 'home_mapped', 'host_directories_mapped', 'http_client',
            'memory_limit_enforced', 'in_process_deadline_enforced', 'other_virtual_root_writes_forbidden', 'deep_sleep']:
        equal(isolation[key], False, 'RPC isolation ' + key)
    for key, expected in [('networking', 'UnsupportedVirtualNetworking'), ('asynchronous_threading', True),
            ('max_guest_threads', 1), ('guest_private_mounts', ['/rp-p0-rpc']), ('rpc_channel', '/rp-p0-rpc/channel'),
            ('guest_fixture_files', [GUEST]), ('guest_readonly_mounts', ['/fixture']), ('stdin', 'closed-pipe'),
            ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536),
            ('guest_environment_policy', 'fixed CGI scaffold, one per-request marker and closed normal/incomplete case')]:
        equal(isolation[key], expected, 'RPC isolation ' + key)
    equal(value['rpc_limits'], {'frame_bytes': 128, 'single_write_bytes': 128, 'request_bytes': 1024,
        'operations': 8, 'response_bytes': 1024, 'total_generated_response_bytes': 8192,
        'reply_read_bytes': 8192, 'simultaneous_channels': 1}, 'unchanged RPC budgets')
    for key, expected in [('php_module_compile_invocations', 1), ('module_clone_count', 4), ('observed_cases', 4),
            ('request_path_module_compile_calls', 0), ('module_delivery', 'Module.clone')]:
        equal(value['reuse'][key], expected, 'RPC reuse ' + key)
    identity = value['reuse']['configured_engine_identity']
    require(type(identity) is str and identity, 'actual configured engine identity')
    managed.integer(value['compile_elapsed_ms'])
    require(type(value['cases']) is list and len(value['cases']) == 4, 'four fixed CGI RPC cases')
    for index, case in enumerate(value['cases']):
        normal = index != 2
        for key, expected in [('name', original.CASES[index]), ('request_number', index + 1), ('nonce', 101 + index),
                ('configured_engine_identity', identity), ('module_delivery', 'Module.clone'), ('module_compile_invocations', 0),
                ('success_expected', normal), ('ok', normal), ('rpc_valid', normal), ('expectation_met', True),
                ('php_completed', True), ('fresh_wasi_runtime', True), ('fresh_rpc_state', True),
                ('program_sha256', FIXTURE_SHA), ('guest_environment_supplied', expected_environment(index)),
                ('guest_arguments', ['-n', '-d', 'fiber.stack_size=2097152', '-d', 'register_argc_argv=0']),
                ('runtime_shutdown_returned', True), ('runtime_shutdown_wait_limit_ms', 2000),
                ('stdout_eof', True), ('stderr_eof', True), ('stdout_overflow', False), ('stderr_overflow', False),
                ('stderr_bytes', 0), ('stderr_hex', ''), ('host_environment_forwarded', False),
                ('host_directories_mapped', False), ('home_mapped', False), ('http_client', False),
                ('networking', 'UnsupportedVirtualNetworking'), ('private_rpc_mount', '/rp-p0-rpc'),
                ('rpc_channel', '/rp-p0-rpc/channel'), ('stdin', 'closed-pipe'),
                ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536)]:
            equal(case[key], expected, 'RPC case ' + key)
        require(not any(key in case for key in ['execution_error', 'host_execution_error', 'expectation_error']), 'unexpected retained RPC execution failure')
        require('managed_access' not in case, 'RPC-only corpus has no permission-query execution claim')
        managed.integer(case['guest_execution_elapsed_ms'])
        equal(case['rpc'], original.expected_rpc(index), 'actual unchanged Rust callbacks, bytes, failure and sealed state')
        if normal: require('rpc_error' not in case, 'normal RPC retained failure')
        else: require(type(case['rpc_error']) is str and 'incomplete RPC frame' in case['rpc_error'], 'retained incomplete rejection')
        wire = bytes.fromhex(case['stdout_hex'])
        equal(case['stdout_bytes'], len(wire), 'complete original CGI capture bytes')
        equal(case['stdout_sha256'], sha(wire), 'complete original CGI capture hash')
        response, body = parse_cgi(wire)
        equal(case['response'], response, 'original wire/CGI response identity')
        prefix = original.LITERAL if normal else b''
        equal(case['stdout_literal_hex'], prefix.hex(), 'literal stdout prefix remains data')
        require(body.startswith(prefix), 'unchanged literal binary prefix missing')
        php = parse_json(body[len(prefix):])
        equal(php, original.expected_php(index, 8), 'unchanged PHP RPC semantics')
        equal(case['php'], php, 'original CGI body/decoded PHP consistency')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-managed-rpc-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend, 'variant': VARIANT,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'scope': SCOPE,
        'outcome': 'not_run', 'rpc_contract_passed': False, 'expected_incomplete_case_accepted': False,
        'owned_host_reaped': False, 'owned_cleanup_completed': False, 'native_comparison_performed': False,
        'native_php_executed_by_this_helper': False, 'native_runtime_fallback_used': False,
        'product_runtime_selected': False, 'product_abi_selected': False, 'product_adopted': False,
        'wordpress_executed': False, 'real_db': False, 'standard_wpdb': False, 'listener_created': False,
        'managed_access_verified_for_fixed_cases': False, 'full_php_compatibility_verified': False,
        'all_resources_immediately_reclaimed': False, 'memory_limit_verified': False, 'cpu_limit_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 600}
    passed, stage = False, 'source_verification'
    try:
        require(platform.system() == 'Linux', 'managed CGI RPC executes only in its Linux source-build job')
        before = source_hashes(); report['input_files'] = before
        report['fixture_derivation'] = verify_fixture()
        stage = 'build_provenance'
        provenance = managed.verify_build(); report['generated_provenance'] = provenance
        stage = 'host_build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HOST / 'Cargo.toml'),
            '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm: command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode: report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'managed CGI RPC host build failed')
        executable = host.select_artifact(built.stdout); binary_sha = digest(executable)
        report.update(native_artifact=executable.relative_to(ROOT).as_posix(), native_artifact_sha256=binary_sha)
        stage = 'execution'
        child_environment = {key: val for key, val in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--managed-rpc-contract'], child_environment, timeout=600)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=completed.returncode,
            host_stdout_sha256=sha(completed.stdout), host_stderr_sha256=sha(completed.stderr))
        require(len(completed.stdout) <= 512 * 1024 and len(completed.stderr) <= 65536, 'managed RPC host capture budget')
        phases = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
            if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr: report['host_stderr_excerpt'] = host.diagnostic(completed.stderr, 8192)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated managed RPC host JSON line')
        value = parse_json(completed.stdout); report['native_result'] = value
        equal(source_hashes(), before, 'RPC sources/oracles unchanged')
        equal(digest(executable), binary_sha, 'RPC host binary unchanged')
        equal(managed.verify_build(), provenance, 'RPC source/tool/three-patch/module chain unchanged')
        require(completed.returncode == 0, 'managed CGI RPC host execution failed')
        stage = 'contract_verification'
        equal(phases, ['module_compile'] + original.CASES, 'one compilation/four unchanged RPC cases')
        validate_result(value, pid, backend, provenance)
        report['rpc_contract_passed'], passed = True, True
        report['outcome'] = 'fixed_managed_rpc_contract_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'; report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr: report['host_stderr_excerpt'] = host.diagnostic(error.stderr, 8192)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try:
            owned_probe.reap_owned_probes(); report['owned_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as error:
            passed = report['rpc_contract_passed'] = False; report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / (platform.system().lower() + '-' + backend + '-managed-rpc-contract.json')
        require(path.resolve() == path and not path.is_symlink(), 'managed RPC evidence path linked')
        wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'managed RPC receipt public budget')
        path.write_bytes(wire)
    print('PHP WASIX managed private RPC: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
