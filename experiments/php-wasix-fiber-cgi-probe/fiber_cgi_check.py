"""Supervise three fixed buffered CGI requests using the opt-in PHP Fiber backend.

Strict final-Wasm assembly inspection precedes compilation and guest execution.
Build, parser, guest, semantic, deadline and cleanup failures remain failures.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
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


builder = load('recastpress_fiber_cgi_build_driver', HERE / 'build.py')
native = load('recastpress_fiber_cgi_native_oracle', HERE / 'native_contract.py')
require, digest = host.require, host.digest
RECEIPT = HERE / 'evidence/build.json'
ORACLE = HERE / 'evidence/native-cgi-fiber-contract.json'
CANONICAL = ROOT / 'experiments/php-wasix-fiber-probe'
FIXTURE = HERE / 'fixtures/fiber_cgi_contract.php'
MODULE = ROOT / 'target/php-wasix-fiber-cgi-probe/php-fiber-cgi.wasm'
VARIANT = 'nts-fresh-wasi-zend-fibers-cgi-v1'


def read_fixed(path, limit):
    require(path.resolve(strict=True) == path and path.is_file(), 'fixed Fiber input symlink/type')
    size = path.stat().st_size
    require(0 < size <= limit, 'fixed Fiber input byte budget')
    with path.open('rb') as source:
        wire = source.read(limit + 1)
    require(len(wire) == size and len(wire) <= limit, 'fixed Fiber input changed while reading')
    return wire


def embedded_sources():
    return [FIXTURE, HERE / 'manifest.json', CANONICAL / 'manifest.json',
        CANONICAL / 'patches/0001-nts-fresh-wasi-zend-fibers.patch',
        CANONICAL / 'fixtures/zend_wasix_fiber.S', CANONICAL / 'fixtures/fiber_contract.php',
        ROOT / 'experiments/php-wasix-cgi-build/manifest.json',
        ROOT / 'experiments/php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch', ORACLE]


def environment():
    return native.expected_environment()


def validation_source(path, read_source=None):
    if read_source is None:
        return read_fixed(path, 256 * 1024)
    wire = read_source(path.relative_to(ROOT).as_posix())
    require(type(wire) is bytes and 0 < len(wire) <= 256 * 1024, 'historical validation source budget/type')
    return wire


def verify_native_reference(oracle=None, read_source=None):
    if oracle is None:
        oracle = parse_json(validation_source(ORACLE, read_source))
    for name, expected in oracle['input_files'].items():
        equal(hashlib.sha256(validation_source(ROOT / name, read_source)).hexdigest(), expected,
            'original native source binding')
    # This pure validator also requires the native reference inputs to remain
    # byte-identical to this verifier. A changed native corpus fails closed.
    native.validate_receipt(oracle)
    return oracle


def validate_provenance(provenance, read_source=None, build_inputs=None):
    if read_source is None:
        require(build_inputs is None, 'current validation derives its own complete builder input set')
        expected_inputs = builder.input_hashes()
    else:
        require(type(build_inputs) is dict and build_inputs, 'historical importer derives original complete builder inputs')
        expected_inputs = build_inputs
        for name, expected in build_inputs.items():
            equal(hashlib.sha256(read_source(name)).hexdigest(), expected, 'historical builder source')
    source_digest = lambda path: hashlib.sha256(validation_source(path, read_source)).hexdigest()
    observed = provenance['builder_provenance']
    for key, expected in [('build_receipt_sha256', provenance['build_receipt_sha256']),
            ('module_bytes', provenance['module']['bytes']), ('module_sha256', provenance['module']['sha256']),
            ('variant', VARIANT), ('manifest_sha256', source_digest(HERE / 'manifest.json')),
            ('patch_sha256', source_digest(CANONICAL / 'patches/0001-nts-fresh-wasi-zend-fibers.patch')),
            ('cgi_patch_sha256', source_digest(ROOT / 'experiments/php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch')),
            ('assembly_sha256', source_digest(CANONICAL / 'fixtures/zend_wasix_fiber.S')),
            ('input_files', expected_inputs)]:
        equal(observed[key], expected, 'combined builder/wrapper provenance ' + key)
    identity = provenance['module']
    equal(identity['path'], MODULE.relative_to(ROOT).as_posix(), 'fixed combined module path')
    equal(identity['wasm_header_verified'], True, 'combined module header')
    equal(identity['executed'], False, 'builder does not execute guest')
    require(type(identity['bytes']) is int and 0 < identity['bytes'] <= 128 * 1024 * 1024, 'combined module budget/type')
    for value in [provenance['build_receipt_sha256'], identity['sha256'], observed['raw_module_sha256']]:
        require(type(value) is str and len(value) == 64 and all(c in '0123456789abcdef' for c in value),
            'combined provenance digest format')


def validate_inspection(inspected, module):
    for key, expected in [('verified', True), ('parser', 'wasmer::wasmparser'),
            ('inspected_bytes', module['bytes']), ('inspected_bytes_sha256', module['sha256']),
            ('stack_global_export', '__stack_pointer'), ('stack_global_type', 'nonshared mutable i32'),
            ('bootstrap_export', 'rp_zend_wasix_bootstrap_sp'), ('bootstrap_export_type', 'defined immutable i32 address'),
            ('bootstrap_word_bytes', 4), ('entry_wasm_local_count', 0), ('linear_stack_memory_writes_in_shims', 0),
            ('target_stack_pointer_changed_before_switch', False), ('restores_before_return_to_C', True),
            ('sets_stack_before_initial_C_dispatch', True), ('engine_invocations', 0), ('guest_executed', False),
            ('php_fiber_compatibility_verified', False)]:
        equal(inspected[key], expected, 'Fiber postopt ' + key)
    def u32(value):
        return type(value) is int and 0 <= value <= 2**32 - 1
    indices = inspected['function_indices']
    equal(set(indices), {'rp_zend_wasix_switch_raw', 'rp_zend_wasix_entry', 'rp_zend_wasix_dispatch'}, 'adapter exports')
    require(all(u32(value) for value in indices.values()) and len(set(indices.values())) == 3, 'defined function identity/alias')
    require(u32(inspected['stack_global_index']) and u32(inspected['bootstrap_global_index'])
        and inspected['stack_global_index'] != inspected['bootstrap_global_index'], 'SP/bootstrap global identity')
    require(type(inspected['stack_global_imported']) is bool, 'SP import observation type')
    require(type(inspected['memory_count']) is int and inspected['memory_count'] >= 1, 'linear memory count')
    memory = inspected['initial_memory_bytes']
    address = inspected['bootstrap_address']
    require(type(memory) is int and 0 < memory <= 2**32 and memory % 65536 == 0
        and u32(address) and address > 0 and address % 4 == 0 and address + 4 <= memory, 'bootstrap word initial memory range')
    equal(inspected['entry_effective_load_address'], address, 'entry load/exported address identity')
    raw = inspected['switch_import']
    require(u32(raw['index']) and raw['index'] < min(indices.values()), 'raw import versus defined function indices')
    equal(raw, {'module': 'wasix_32v1', 'name': 'context_switch', 'index': raw['index'],
        'parameters': ['i64'], 'results': ['i32']}, 'raw switch ABI')
    sequences = {
        1: ['global.get', 'local.set', 'local.get', 'call', 'local.get', 'global.set', 'end'],
        2: ['global.get', 'local.set', 'local.get', 'call', 'local.set', 'local.get', 'global.set', 'local.get', 'end'],
    }
    count = inspected['switch_wasm_local_count']
    require(type(count) is int and count in sequences, 'register-only switch locals')
    equal(inspected['switch_operator_sequence'], sequences[count], 'exact postopt switch instructions')
    equal(inspected['entry_operator_sequence'], ['i32.const', 'i32.load', 'global.set', 'call', 'unreachable', 'end'],
        'exact postopt entry instructions')


def source_hashes():
    paths = host.compiled_inputs() + [Path(__file__).resolve(), HOST / 'check.py',
        HOST / 'language_check.py', HERE / 'native_contract.py', ROOT / 'scripts/owned_probe.py']
    hashes = {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}
    for name, value in builder.input_hashes().items():
        if name in hashes:
            equal(hashes[name], value, 'shared Fiber build source')
        hashes[name] = value
    oracle = verify_native_reference()
    for name, value in oracle['input_files'].items():
        path = ROOT / name
        require(path.resolve() == path and path.is_relative_to(ROOT), 'native oracle input outside workspace')
        equal(digest(path), value, 'native oracle input binding')
        hashes[name] = value
    return hashes


def verify_build():
    verified = builder.verify_build()
    require(type(verified) is dict and verified, 'Fiber builder returned no provenance')
    wire = read_fixed(RECEIPT, 2 * 1024 * 1024)
    receipt = parse_json(wire)
    for key, expected in [('schema', 'recastpress.php-wasix-fiber-cgi-build.v0.5'),
            ('variant', VARIANT), ('outcome', 'fiber_cgi_candidate_built_not_executed'),
            ('phase', 'build_complete'), ('guest_executed', False)]:
        equal(receipt[key], expected, 'Fiber builder ' + key)
    equal(receipt['input_files'], builder.input_hashes(), 'Fiber builder input set')
    identity = receipt['php_wasm']
    equal(identity['path'], MODULE.relative_to(ROOT).as_posix(), 'Fiber artifact path')
    equal(identity['wasm_header_verified'], True, 'Fiber Wasm header')
    equal(identity['executed'], False, 'builder does not execute PHP')
    require(MODULE.resolve(strict=True) == MODULE and MODULE.is_file(), 'Fiber module symlink/type')
    equal(identity['bytes'], MODULE.stat().st_size, 'Fiber module bytes')
    require(0 < identity['bytes'] <= 128 * 1024 * 1024, 'Fiber module budget')
    equal(identity['sha256'], digest(MODULE), 'Fiber module digest')
    provenance = {'build_receipt_sha256': hashlib.sha256(wire).hexdigest(),
                  'module': identity, 'builder_provenance': verified}
    validate_provenance(provenance)
    return provenance


def validate_result(value, pid, backend, provenance, read_source=None, build_inputs=None):
    validate_provenance(provenance, read_source, build_inputs)
    require('error' not in value, 'failed host result must not become success')
    for key, expected in [('schema', 'recastpress.php-wasix-fiber-cgi-host.v0.5'), ('process_pid', pid),
            ('engine', 'wasmer-7.2.0/' + backend), ('wasmer_wasix', '0.702.0'),
            ('ok', True), ('phase', 'fiber_cgi_contract_verified'), ('variant', VARIANT),
            ('engine_supports_async', True), ('module_compile_invocations', 1),
            ('php_fiber_compatibility_verified_for_fixed_cases', True)]:
        equal(value[key], expected, 'Fiber host ' + key)
    for key in ['full_php_compatibility_verified', 'product_runtime_selected', 'native_runtime_fallback_used',
            'memory_limit_verified', 'cpu_limit_verified', 'all_resources_immediately_reclaimed',
            'sysroot_source_build_attestation_verified', 'physical_stack_free_verified',
            'physical_stack_reuse_verified', 'stack_guards_verified', 'persistent_requests_supported',
            'zts_supported', 'real_db', 'wordpress_executed']:
        equal(value[key], False, 'unclaimed Fiber gate ' + key)
    equal(value['source']['commit'], 'f4bfae79d5c19028349082f707930cf0951d23b9', 'PHP source pin')
    equal(value['source']['repository'], 'wasix-org/php', 'PHP source repository')
    equal(value['source']['kind'], 'same-job-patched-source-build', 'PHP source kind')
    equal(value['source']['verified'], True, 'PHP source verified')
    for key, expected in [('path', MODULE.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_bytes', provenance['module']['bytes']), ('observed_sha256', provenance['module']['sha256'])]:
        equal(value['artifact'][key], expected, 'Fiber host artifact ' + key)
    equal(value['build_receipt']['observed_sha256'], provenance['build_receipt_sha256'], 'host builder digest')
    equal(value['build_receipt']['verified'], True, 'host receipt verified')
    equal(value['build_receipt']['path'], RECEIPT.relative_to(ROOT).as_posix(), 'fixed host build receipt path')
    equal(value['build_receipt']['full_source_tool_chain_verified_by_host'], False, 'external chain ownership')
    equal(value['source_files'], [{'path': p.relative_to(ROOT).as_posix(), 'bytes': len(validation_source(p, read_source)),
        'sha256': hashlib.sha256(validation_source(p, read_source)).hexdigest(), 'verified': True} for p in embedded_sources()], 'compiled source/oracle identities')
    policy = value['isolation']
    equal(policy, {'asynchronous_threading': True, 'deep_sleep': False, 'max_guest_threads': 1,
        'host_environment_forwarded': False, 'home_mapped': False, 'guest_environment_supplied': environment(),
        'host_directories_mapped': False, 'guest_mounts': ['/fixture'], 'networking': 'UnsupportedVirtualNetworking',
        'http_client': False, 'stdin': 'closed-pipe', 'stdout_limit_bytes': 65536, 'stderr_limit_bytes': 65536,
        'other_virtual_root_writes_forbidden': False}, 'Fiber runtime capability policy')
    validate_inspection(value['postopt_adapter'], provenance['module'])
    oracle = verify_native_reference(read_source=read_source)
    equal(value['native_oracle'], {'sha256': hashlib.sha256(validation_source(ORACLE, read_source)).hexdigest(), 'expected_case_count': 6,
        'expected_values_weakened': False}, 'strict native oracle binding')
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms'] >= 0, 'compile span')
    require(type(value['cases']) is list and len(value['cases']) == 3, 'three fresh Fiber instances')
    equal(value['exceptions_executed'], True, 'fixed corpus exception execution')
    equal(value['exception_support']['execution_verdict'], 'fixed-cgi-fiber-corpus-passed', 'fixed exception scope')
    equal(value['integer_width_matches_native'], True, 'fixed PHP integer width')
    program = validation_source(FIXTURE, read_source)
    require(program.startswith(b'<?php'), 'fixed Fiber PHP tag')
    for index, name in enumerate(['normal_first', 'normal_repeat', 'normal_final']):
        case = value['cases'][index]
        require(not any(key in case for key in ['host_execution_error', 'expectation_error', 'guest_json_error']),
            'failed case must not become a successful recovery')
        for key, expected in [('name', name), ('request_number', index + 1), ('module_delivery', 'Module.clone'),
                ('module_compile_invocations', 0), ('fresh_wasi_runtime', True), ('ok', True),
                ('expectation_met', True), ('native_semantics_match', True), ('stdout_eof', True),
                ('stderr_eof', True), ('stdout_overflow', False), ('stderr_overflow', False),
                ('stderr_bytes', 0), ('stderr_hex', ''), ('runtime_shutdown_returned', True),
                ('runtime_shutdown_wait_limit_ms', 2000), ('guest_json_decoded', True),
                ('program_sha256', hashlib.sha256(program).hexdigest()),
                ('guest_arguments', ['-n', '-d', 'fiber.stack_size=2097152', '-d', 'register_argc_argv=0']),
                ('guest_environment_supplied', environment())]:
            equal(case[key], expected, 'Fiber instance ' + key)
        require(type(case['guest_execution_elapsed_ms']) is int and case['guest_execution_elapsed_ms'] >= 0, 'guest span')
        wire = bytes.fromhex(case['stdout_hex'])
        require(0 < len(wire) <= 65536, 'bounded complete CGI response')
        equal(case['stdout_bytes'], len(wire), 'guest output bytes')
        equal(case['stdout_sha256'], hashlib.sha256(wire).hexdigest(), 'guest output hash')
        response, body, _ = native.parse_completed_cgi(wire)
        equal(response, case['response'], 'guest raw/decoded CGI response')
        equal(response, oracle['response'], 'exact native CGI response')
        observation = parse_json(body)
        equal(observation, case['guest_json'], 'guest raw/decoded JSON')
        native.validate_observation(observation)
        equal(observation, oracle['observation'], 'exact native six-case semantics')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-fiber-cgi-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'variant': VARIANT,
        'outcome': 'not_run', 'fiber_contract_passed': False, 'owned_host_reaped': False,
        'product_runtime_selected': False, 'full_php_compatibility_verified': False,
        'native_runtime_fallback_used': False, 'all_resources_immediately_reclaimed': False,
        'memory_limit_verified': False, 'cpu_limit_verified': False, 'stack_free_or_reuse_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 600}
    passed, stage = False, 'build_provenance'
    try:
        require(platform.system() == 'Linux', 'Fiber prototype requires its Linux build job')
        before = source_hashes()
        report['input_files'] = before
        provenance = verify_build()
        report['generated_provenance'] = provenance
        stage = 'host_build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HOST / 'Cargo.toml'),
            '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode:
            report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'Fiber Rust host build failed')
        executable = host.select_artifact(built.stdout)
        binary_sha = digest(executable)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = binary_sha
        stage = 'execution'
        child_environment = {k: v for k, v in environment.items() if k.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, result = owned_probe.run_captured_probe([str(executable), '--fiber-cgi-contract'], child_environment, timeout=600)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=result.returncode)
        require(len(result.stdout) <= 512 * 1024 and len(result.stderr) <= 65536, 'Fiber host capture budget')
        report['host_stderr_sha256'] = hashlib.sha256(result.stderr).hexdigest()
        if result.stderr:
            report['host_stderr_excerpt'] = host.diagnostic(result.stderr, 8192)
        require(result.stdout.endswith(b'\n') and result.stdout.count(b'\n') == 1, 'one host JSON line')
        value = parse_json(result.stdout)
        report['native_result'] = value
        equal(source_hashes(), before, 'Fiber sources unchanged after execution')
        equal(digest(executable), binary_sha, 'Fiber native host unchanged')
        equal(verify_build(), provenance, 'Fiber generated source/tool/module chain unchanged')
        require(result.returncode == 0, 'patched PHP Fiber host execution failed')
        stage = 'contract_verification'
        validate_result(value, pid, backend, provenance)
        passed, report['fiber_contract_passed'] = True, True
        report['outcome'] = 'fixed_php_fiber_cgi_contract_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr:
            report['host_stderr_excerpt'] = host.diagnostic(error.stderr, 8192)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try:
            owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            passed, report['fiber_contract_passed'], report['outcome'] = False, False, 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        (HERE / 'evidence').mkdir(exist_ok=True)
        (HERE / ('evidence/' + platform.system().lower() + '-' + backend + '-fiber-cgi-contract.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX Fiber CGI prototype: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
