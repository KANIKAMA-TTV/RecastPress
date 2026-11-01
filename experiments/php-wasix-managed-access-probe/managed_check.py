"""Supervise the closed managed-access candidate with unchanged native corpora.

--fiber compares the extended Fiber oracle; default compares runtime/hooks/IO
and requires actual private permission-query counters. Native PHP is never run.
No guest JSON, CGI response, expected value or failed observation is rewritten.
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
COMBINED = ROOT / 'experiments/php-wasix-fiber-cgi-probe'
RUNTIME = ROOT / 'experiments/php-wasix-cgi-runtime-probe'
sys.path.insert(0, str(HOST))
import check as host
from language_check import equal, parse_json
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


builder = load('rp_managed_comparison_build_gate', HERE / 'build.py')
fiber = load('rp_managed_comparison_fiber_gate', COMBINED / 'fiber_cgi_check.py')
runtime = load('rp_managed_comparison_runtime_gate', RUNTIME / 'runtime_check.py')
require, digest = host.require, host.digest
VARIANT = 'nts-fresh-wasi-zend-fibers-managed-access-v1'
MODULE, RECEIPT = builder.MODULE, builder.RECEIPT
require(builder.VARIANT == VARIANT and MODULE == ROOT / 'target/php-wasix-managed-access-probe/php-fiber-cgi.wasm'
    and RECEIPT == HERE / 'evidence/build.json', 'closed managed profile changed')
SOURCES = [
    'experiments/php-wasix-managed-access-probe/manifest.json',
    'experiments/php-wasix-managed-access-probe/patch-plan.json',
    'experiments/php-wasix-managed-access-probe/patches/0001-wasi-managed-access.patch',
    'experiments/php-wasix-managed-access-probe/fixtures/recastpress_fs_access.h',
    'experiments/php-wasix-fiber-cgi-probe/manifest.json',
    'experiments/php-wasix-fiber-cgi-probe/fixtures/fiber_cgi_contract.php',
    'experiments/php-wasix-fiber-cgi-probe/evidence/native-cgi-fiber-contract.json',
    'experiments/php-wasix-fiber-probe/manifest.json',
    'experiments/php-wasix-fiber-probe/patches/0001-nts-fresh-wasi-zend-fibers.patch',
    'experiments/php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S',
    'experiments/php-wasix-fiber-probe/fixtures/fiber_contract.php',
    'experiments/php-wasix-cgi-build/manifest.json',
    'experiments/php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch',
    'experiments/php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json',
]
COUNTERS = ['requests', 'allowed', 'denied', 'missing', 'unmanaged', 'invalid', 'io_errors',
    'input_bytes', 'generated_bytes', 'read_bytes', 'pending_bytes', 'unread_response_bytes',
    'active_handles', 'closed_handles', 'open_events', 'reused_transactions', 'eof_reads', 'io_calls']
MAX_PATH = 4096
MAX_FRAME = 7 + MAX_PATH
MAX_REQUESTS = 64
MAX_INPUT = MAX_REQUESTS * MAX_FRAME
MAX_REPLY = MAX_REQUESTS * 5
MAX_IO_CALLS = MAX_INPUT + MAX_REPLY * 2 + MAX_REQUESTS * 16
SHA = re.compile('[0-9a-f]{64}')


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def integer(value, minimum=0, maximum=2**32):
    require(type(value) is int and minimum <= value <= maximum, 'invalid recorded count/type/budget')


def source_bytes(path, read_source=None):
    return fiber.validation_source(path, read_source)


def sources(paths, read_source=None):
    return [{'path': p.relative_to(ROOT).as_posix(), 'bytes': len(source_bytes(p, read_source)),
        'sha256': sha(source_bytes(p, read_source)), 'verified': True} for p in paths]


def native_reference(fiber_mode, read_source=None):
    return fiber.verify_native_reference(read_source=read_source) if fiber_mode else runtime.verify_native_reference(read_source)


def source_hashes():
    # Both references are embedded in every host binary/profile. Validate the
    # full pinned native package/source receipts purely, without native PHP.
    oracles = [native_reference(True), native_reference(False)]
    paths = host.compiled_inputs() + [Path(__file__).resolve(), HERE / 'build.py', HERE / 'source_profile.py',
        HERE / 'patch_source.py', HOST / 'check.py', HOST / 'language_check.py',
        COMBINED / 'build.py', COMBINED / 'patch_source.py', COMBINED / 'fiber_cgi_check.py',
        COMBINED / 'native_contract.py', RUNTIME / 'runtime_check.py', RUNTIME / 'prepare.py',
        RUNTIME / 'native_contract.py', ROOT / 'experiments/php-wasix-fiber-probe/native_contract.py',
        ROOT / 'experiments/php-runtime-candidate-probe/native_contract.py', ROOT / 'scripts/owned_probe.py']
    hashes = {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}
    groups = [builder.input_hashes(), runtime.preparer.input_hashes()] + [oracle['input_files'] for oracle in oracles]
    for group in groups:
        for name, value in group.items():
            path = ROOT / name
            require(path.resolve(strict=True) == path and path.is_relative_to(ROOT), 'managed input outside workspace/linked')
            equal(digest(path), value, 'managed source input binding')
            if name in hashes: equal(hashes[name], value, 'shared managed source input')
            hashes[name] = value
    for name in SOURCES:
        equal(hashes[name], digest(ROOT / name), 'managed embedded input in source inventory')
    return dict(sorted(hashes.items()))


def validate_provenance(provenance, read_source=None, build_inputs=None):
    if read_source is None:
        require(build_inputs is None, 'live managed validation derives complete build inputs')
        expected_inputs = builder.input_hashes()
    else:
        require(type(build_inputs) is dict and build_inputs, 'historical managed inputs must be derived by importer')
        expected_inputs = build_inputs
        for name, value in expected_inputs.items(): equal(sha(read_source(name)), value, 'historical managed input')
    observed = provenance['builder_provenance']
    expected = {
        'build_receipt_sha256': provenance['build_receipt_sha256'], 'variant': VARIANT,
        'manifest_sha256': sha(source_bytes(HERE / 'manifest.json', read_source)),
        'patch_sha256': sha(source_bytes(ROOT / 'experiments/php-wasix-fiber-probe/patches/0001-nts-fresh-wasi-zend-fibers.patch', read_source)),
        'cgi_patch_sha256': sha(source_bytes(ROOT / 'experiments/php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch', read_source)),
        'assembly_sha256': sha(source_bytes(ROOT / 'experiments/php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S', read_source)),
        'managed_access_patch_sha256': sha(source_bytes(HERE / 'patches/0001-wasi-managed-access.patch', read_source)),
        'managed_access_header_sha256': sha(source_bytes(HERE / 'fixtures/recastpress_fs_access.h', read_source)),
        'source_profile_sha256': sha(source_bytes(HERE / 'source_profile.py', read_source)),
        'module_bytes': provenance['module']['bytes'], 'module_sha256': provenance['module']['sha256'],
        'input_files': expected_inputs,
        'scope': 'same-job NTS CGI candidate with managed-access adaptation; not product ABI or full compatibility',
    }
    equal(set(observed), set(expected) | {'toolchain_receipt_sha256', 'raw_module_sha256'}, 'complete managed builder provenance')
    for key, value in expected.items(): equal(observed[key], value, 'managed builder provenance ' + key)
    identity = provenance['module']
    equal(set(identity), {'path', 'bytes', 'sha256', 'wasm_header_verified', 'executed'}, 'managed module identity fields')
    equal(identity['path'], MODULE.relative_to(ROOT).as_posix(), 'closed managed module path')
    equal(identity['wasm_header_verified'], True, 'managed Wasm header')
    equal(identity['executed'], False, 'builder does not execute guest')
    integer(identity['bytes'], 8, 128 * 1024 * 1024)
    for value in [provenance['build_receipt_sha256'], identity['sha256'], observed['raw_module_sha256'], observed['toolchain_receipt_sha256']]:
        require(type(value) is str and SHA.fullmatch(value), 'managed provenance digest format')
    for name in SOURCES:
        if '/evidence/native-' not in name:
            equal(expected_inputs[name], sha(source_bytes(ROOT / name, read_source)), 'managed compiler embedded source binding')


def verify_build():
    observed = builder.verify_build()  # Full source/tool/config/three-patch/module gate, no PHP execution.
    wire = fiber.read_fixed(RECEIPT, 2 * 1024 * 1024)
    receipt = parse_json(wire)
    for key, expected in [('schema', builder.driver.BUILD_SCHEMA), ('variant', VARIANT),
            ('outcome', builder.driver.BUILD_SUCCESS), ('phase', 'build_complete'), ('guest_executed', False),
            ('input_files', builder.input_hashes())]:
        equal(receipt[key], expected, 'managed build receipt ' + key)
    equal(len(receipt['source_patches']), 3, 'all three source patches')
    require(all(patch['applied'] is True for patch in receipt['source_patches']), 'incomplete managed patch chain')
    identity = receipt['php_wasm']
    equal(identity['path'], MODULE.relative_to(ROOT).as_posix(), 'managed generated path')
    require(MODULE.resolve(strict=True) == MODULE and MODULE.is_file(), 'managed module missing/linked')
    equal(identity['bytes'], MODULE.stat().st_size, 'managed generated bytes')
    equal(identity['sha256'], digest(MODULE), 'managed generated digest')
    provenance = {'build_receipt_sha256': sha(wire), 'module': identity, 'builder_provenance': observed}
    validate_provenance(provenance)
    return provenance


def validate_access(snapshot):
    equal(set(snapshot), set(COUNTERS) | {'finished', 'protocol_error'}, 'all managed access snapshot fields')
    for key in COUNTERS: integer(snapshot[key], 0, MAX_IO_CALLS)
    equal(snapshot['finished'], True, 'managed request sealed')
    equal(snapshot['protocol_error'], None, 'managed protocol has no retained error')
    requests = snapshot['requests']
    integer(requests, 5, MAX_REQUESTS)
    require(snapshot['allowed'] >= 3 and snapshot['denied'] >= 2, 'actual positive permission queries were not observed')
    equal(sum(snapshot[key] for key in ['allowed', 'denied', 'missing', 'unmanaged', 'invalid', 'io_errors']), requests, 'managed status total')
    for key in ['missing', 'invalid', 'io_errors', 'pending_bytes', 'unread_response_bytes', 'active_handles']:
        equal(snapshot[key], 0, 'managed completion ' + key)
    equal(snapshot['generated_bytes'], requests * 5, 'one exact five-byte reply per query')
    equal(snapshot['read_bytes'], snapshot['generated_bytes'], 'every generated reply consumed once')
    integer(snapshot['input_bytes'], requests * 8, requests * MAX_FRAME)
    integer(snapshot['open_events'], 1, MAX_REQUESTS)
    equal(snapshot['closed_handles'] + snapshot['active_handles'], snapshot['open_events'], 'underlying VirtualFile open/close balance')
    integer(snapshot['reused_transactions'], 0, requests - 1)
    integer(snapshot['io_calls'], requests, MAX_IO_CALLS)
    # These are VFile events, not guest descriptor/fopen counts. WASIX can reuse
    # one underlying VFile across distinct guest descriptors/transactions.


def validate_scratch(case):
    counters = ['ordinary_write_calls', 'ordinary_bytes_written', 'ordinary_truncate_opens', 'ordinary_set_len_calls',
        'readonly_mutating_open_rejections', 'readonly_handle_mutation_rejections', 'namespace_mutation_rejections',
        'open_handles', 'closed_handles']
    initial = {'ordinary_hex': '', 'readonly_hex': b'readonly fixture\n'.hex(), **{key: 0 for key in counters}}
    equal(case['scratch_initial'], initial, 'fresh request scratch snapshot')
    final = case['scratch_final']
    equal(set(final), set(initial), 'complete final scratch snapshot')
    equal(final['ordinary_hex'], b'ordinary\0\xff\r\n'.hex(), 'actual ordinary 12-byte write/read')
    equal(final['readonly_hex'], initial['readonly_hex'], 'actual readonly 17-byte payload unchanged')
    for key in counters: integer(final[key], 0, 65536)
    require(final['ordinary_write_calls'] >= 1 and final['ordinary_bytes_written'] == 12
        and final['readonly_mutating_open_rejections'] >= 1 and final['open_handles'] == 0
        and final['closed_handles'] >= 4, 'actual IO/denial and closed scratch handles')


def validate_result(value, pid, backend, provenance, fiber_mode=False, prepared=None, read_source=None, build_inputs=None):
    validate_provenance(provenance, read_source, build_inputs)
    require(type(fiber_mode) is bool and backend in ['cranelift', 'llvm'], 'closed managed comparison mode/backend')
    kind = 'fiber' if fiber_mode else 'runtime'
    require('error' not in value, 'failed managed host cannot become successful comparison')
    for key, expected in [('schema', 'recastpress.php-wasix-managed-' + kind + '-host.v0.5'),
            ('scope', 'three-patch-managed-candidate-fixed-Fiber-regression-only' if fiber_mode else
                'three-patch-managed-candidate-native-runtime-and-pure-permission-query-comparison'),
            ('variant', VARIANT), ('phase', 'managed_' + kind + '_contract_verified'),
            ('process_pid', pid), ('engine', 'wasmer-7.2.0/' + backend), ('wasmer_wasix', '0.702.0'),
            ('ok', True), ('engine_supports_async', True), ('module_compile_invocations', 1),
            ('php_fiber_compatibility_verified_for_fixed_cases', fiber_mode), ('product_abi_selected', False),
            ('exceptions_executed', True), ('integer_width_matches_native', True)]:
        equal(value[key], expected, 'managed host ' + key)
    integer(pid, 1)
    for key in ['full_php_compatibility_verified', 'product_runtime_selected', 'native_runtime_fallback_used',
            'memory_limit_verified', 'cpu_limit_verified', 'all_resources_immediately_reclaimed',
            'sysroot_source_build_attestation_verified', 'physical_stack_free_verified', 'physical_stack_reuse_verified',
            'stack_guards_verified', 'persistent_requests_supported', 'zts_supported', 'real_db']:
        equal(value[key], False, 'unclaimed managed gate ' + key)
    equal(value['wordpress_executed'], not fiber_mode, 'pure WordPress execution scope')
    equal(value['source'], {'repository': 'wasix-org/php', 'commit': 'f4bfae79d5c19028349082f707930cf0951d23b9',
        'kind': 'same-job-patched-source-build', 'verified': True}, 'fixed PHP source')
    for key, expected in [('path', MODULE.relative_to(ROOT).as_posix()), ('verified', True),
            ('identity_policy', 'same-job-three-source-patch-build'), ('observed_bytes', provenance['module']['bytes']),
            ('observed_sha256', provenance['module']['sha256'])]:
        equal(value['artifact'][key], expected, 'managed host artifact ' + key)
    for key, expected in [('path', RECEIPT.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_sha256', provenance['build_receipt_sha256']), ('full_source_tool_chain_verified_by_host', False),
            ('chain_owner', 'managed_check.py verifies all source/tool/patch/module provenance')]:
        equal(value['build_receipt'][key], expected, 'managed host build receipt ' + key)
    equal(value['source_files'], sources([ROOT / name for name in SOURCES], read_source), 'fourteen compiled managed source/oracle identities')
    expected_isolation = {'asynchronous_threading': True, 'deep_sleep': False, 'max_guest_threads': 1,
        'host_environment_forwarded': False, 'home_mapped': False,
        'guest_environment_supplied': fiber.environment() if fiber_mode else runtime.native.expected_environment(),
        'host_directories_mapped': False, 'guest_mounts': ['/fixture'] if fiber_mode else ['/fixture', '/wp-core', '/scratch', '/rp-p0-fs'],
        'networking': 'UnsupportedVirtualNetworking', 'http_client': False, 'stdin': 'closed-pipe',
        'stdout_limit_bytes': 65536, 'stderr_limit_bytes': 65536, 'other_virtual_root_writes_forbidden': False}
    if not fiber_mode:
        expected_isolation.update(writable_fixture_entries=['/scratch/ordinary.txt'], scratch_file_limit_bytes=4096,
            private_access_endpoint='/rp-p0-fs/access', access_max_path_bytes=MAX_PATH, access_max_requests=MAX_REQUESTS)
        require(type(prepared) is dict and prepared, 'verified WP source preparation required')
        equal(value['runtime_source_files'], sources(runtime.runtime_sources(), read_source), 'compiled runtime reference identities')
        equal(value['wordpress_source_files'], {name: {**row, 'verified': True} for name, row in prepared['files'].items()}, 'prepared actual WP3 source identities')
        for key, expected in [('runtime_contract_verified_for_fixed_cases', True), ('managed_access_verified_for_fixed_cases', True),
                ('php_metadata_matches_actual_readonly_denial', True), ('wordpress_pure_hooks_executed', True),
                ('full_wordpress_bootstrap_executed', False)]:
            equal(value[key], expected, 'managed runtime scope ' + key)
    else:
        require(value.get('managed_access_verified_for_fixed_cases', False) is False, 'Fiber-only corpus does not prove permission queries')
    equal(value['isolation'], expected_isolation, 'actual managed runtime capability policy')
    fiber.validate_inspection(value['postopt_adapter'], provenance['module'])
    oracle = native_reference(fiber_mode, read_source)
    oracle_path = fiber.ORACLE if fiber_mode else runtime.ORACLE
    expected_oracle = {'sha256': sha(source_bytes(oracle_path, read_source)), 'expected_case_count': 6, 'expected_values_weakened': False}
    if not fiber_mode: expected_oracle['path'] = oracle_path.relative_to(ROOT).as_posix()
    equal(value['native_oracle'], expected_oracle, 'unchanged native comparison oracle')
    equal(value['exception_support']['execution_verdict'], 'fixed-cgi-fiber-corpus-passed' if fiber_mode else 'fixed-cgi-runtime-corpus-passed', 'fixed exception scope')
    integer(value['compile_elapsed_ms'])
    require(type(value['cases']) is list and len(value['cases']) == 3, 'three fresh managed instances')
    fixture = fiber.FIXTURE if fiber_mode else runtime.FIXTURE
    program = source_bytes(fixture, read_source)
    require(program.startswith(b'<?php'), 'fixed CGI PHP source tag')
    native = fiber.native if fiber_mode else runtime.native
    environment = fiber.environment() if fiber_mode else runtime.native.expected_environment()
    for index, name in enumerate(['normal_first', 'normal_repeat', 'normal_final']):
        case = value['cases'][index]
        require(not any(key in case for key in ['host_execution_error', 'expectation_error', 'guest_json_error']), 'retained failed managed request')
        for key, expected in [('name', name), ('request_number', index + 1), ('module_delivery', 'Module.clone'),
                ('module_compile_invocations', 0), ('fresh_wasi_runtime', True), ('ok', True),
                ('native_semantics_match', True), ('expectation_met', True), ('guest_json_decoded', True),
                ('program_sha256', sha(program)), ('guest_arguments', native.command_arguments()),
                ('guest_environment_supplied', environment), ('stdout_eof', True), ('stderr_eof', True),
                ('stdout_overflow', False), ('stderr_overflow', False), ('stderr_bytes', 0), ('stderr_hex', ''),
                ('runtime_shutdown_returned', True), ('runtime_shutdown_wait_limit_ms', 2000)]:
            equal(case[key], expected, 'managed request ' + key)
        integer(case['guest_execution_elapsed_ms'])
        wire = bytes.fromhex(case['stdout_hex']); integer(len(wire), 1, 65536)
        equal(case['stdout_bytes'], len(wire), 'complete managed CGI capture length')
        equal(case['stdout_sha256'], sha(wire), 'complete managed CGI capture hash')
        response, body, _ = native.parse_completed_cgi(wire)
        equal(response, case['response'], 'actual raw/decoded managed CGI response')
        equal(response, oracle['response'], 'exact unchanged native CGI response')
        observation = parse_json(body)
        native.validate_observation(observation)
        equal(observation, case['guest_json'], 'actual raw/decoded managed PHP JSON')
        equal(observation, oracle['observation'], 'exact unchanged native six-case semantics')
        if fiber_mode:
            require('managed_access' not in case, 'no permission-query execution claim for Fiber-only regression')
        else:
            equal(case['fresh_scratch'], True, 'fresh managed scratch')
            validate_scratch(case)
            validate_access(case['managed_access'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fiber', action='store_true', help='unchanged extended Fiber regression; no permission-query claim')
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    kind, backend = ('fiber' if args.fiber else 'runtime'), ('llvm' if args.llvm else 'cranelift')
    flag = 'fiber_contract_passed' if args.fiber else 'runtime_contract_passed'
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-managed-' + kind + '-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend, 'variant': VARIANT,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'outcome': 'not_run',
        flag: False, 'native_comparison_passed': False, 'managed_access_verified_for_fixed_cases': False,
        'owned_host_reaped': False, 'product_runtime_selected': False, 'product_abi_selected': False,
        'full_php_compatibility_verified': False, 'native_php_executed_by_this_helper': False,
        'native_runtime_fallback_used': False, 'all_resources_immediately_reclaimed': False,
        'memory_limit_verified': False, 'cpu_limit_verified': False, 'stack_free_or_reuse_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 600,
        'managed_access_counter_scope': 'underlying Rust VirtualFile events, not guest fd/fopen counts'}
    passed, stage = False, 'build_provenance'
    try:
        require(platform.system() == 'Linux', 'managed CGI profile executes only in its Linux source-build job')
        before = source_hashes(); report['input_files'] = before
        provenance = verify_build(); report['generated_provenance'] = provenance
        if not args.fiber:
            prepared = runtime.preparer.verify_prepared(); report['wordpress_preparation'] = prepared
        else: prepared = None
        reference = native_reference(args.fiber)
        oracle_path = fiber.ORACLE if args.fiber else runtime.ORACLE
        report['native_reference'] = {'path': oracle_path.relative_to(ROOT).as_posix(), 'sha256': digest(oracle_path),
            'native_executed_by_this_helper': False, 'expected_case_count': 6}
        stage = 'host_build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HOST / 'Cargo.toml'),
            '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm: command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode: report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'managed Rust host build failed')
        executable = host.select_artifact(built.stdout); binary_sha = digest(executable)
        report.update(native_artifact=executable.relative_to(ROOT).as_posix(), native_artifact_sha256=binary_sha)
        stage = 'execution'
        child_environment = {key: value for key, value in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        selector = '--managed-fiber-cgi-contract' if args.fiber else '--managed-cgi-runtime-contract'
        pid, result = owned_probe.run_captured_probe([str(executable), selector], child_environment, timeout=600)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=result.returncode,
            host_stdout_sha256=sha(result.stdout), host_stderr_sha256=sha(result.stderr))
        require(len(result.stdout) <= 512 * 1024 and len(result.stderr) <= 65536, 'managed host capture quota')
        if result.stderr: report['host_stderr_excerpt'] = host.diagnostic(result.stderr, 8192)
        require(result.stdout.endswith(b'\n') and result.stdout.count(b'\n') == 1, 'one terminated managed host JSON line')
        value = parse_json(result.stdout)
        report['native_result'] = value  # Preserve the actual host result without metadata/value rewriting.
        equal(source_hashes(), before, 'managed sources unchanged after execution')
        equal(digest(executable), binary_sha, 'managed native binary unchanged')
        equal(verify_build(), provenance, 'managed source/tool/three-patch/module chain unchanged')
        if not args.fiber: equal(runtime.preparer.verify_prepared(), prepared, 'managed WP3 preparation unchanged')
        require(result.returncode == 0, 'managed CGI host execution failed')
        stage = 'contract_verification'
        validate_result(value, pid, backend, provenance, fiber_mode=args.fiber, prepared=prepared)
        passed = report[flag] = report['native_comparison_passed'] = True
        report['managed_access_verified_for_fixed_cases'] = not args.fiber
        report['outcome'] = 'fixed_managed_' + kind + '_contract_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'; report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr: report['host_stderr_excerpt'] = host.diagnostic(error.stderr, 8192)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try: owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            passed = report[flag] = report['native_comparison_passed'] = report['managed_access_verified_for_fixed_cases'] = False
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / (platform.system().lower() + '-' + backend + '-managed-' + kind + '-contract.json')
        require(path.resolve() == path and not path.is_symlink(), 'managed evidence path linked')
        wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'managed comparison receipt exceeds public quota')
        path.write_bytes(wire)
    print('PHP WASIX managed ' + kind + ' comparison: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
