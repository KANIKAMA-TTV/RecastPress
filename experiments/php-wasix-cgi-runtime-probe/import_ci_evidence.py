"""Import fixed JSON-only CGI runtime IO CI evidence against its original tree.

Success requires the strict historical checker. A failed runtime comparison is
kept failed: complete captured JSON and VFS snapshots are summarized separately
from native differences. No guest, native PHP, tool or binary is acquired/run.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
COMBINED = ROOT / 'experiments/php-wasix-fiber-cgi-probe'
RUNTIME_PREFIX = 'experiments/php-wasix-cgi-runtime-probe/'
SOURCE = '2649132e3fd082a01d13ffe4850cc268a3a62889'
RUN = '41328695777'
SPECS = {
    'llvm': {'artifact': 13092804904, 'bytes': 72811,
        'sha256': '376e11a429968917c55b591e2bb303fbb30801492a5c5a3880423c9a2bc598e4',
        'runtime_verdict': 'failure'},
    'cranelift': {'artifact': 13093006555, 'bytes': 72375,
        'sha256': 'b31ee5695fc208d35d6751c9811879923943d3b0f5491935b047ce3556490b84',
        'runtime_verdict': 'failure'},
}


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


base = load('recastpress_cgi_runtime_ci_combined_build_verifier', COMBINED / 'import_ci_evidence.py')
# No old cached Git blob may survive switching from the initial combined run.
base.SOURCE, base.RUN, base.reader.COMMIT = SOURCE, RUN, SOURCE
base.original.cache_clear()
checker = load('recastpress_cgi_runtime_ci_historical_checker', HERE / 'runtime_check.py')
original, equal, require = base.original, base.equal, base.require
sha, integer, digest, parse_json = base.sha, base.integer, base.digest, base.parse_json


def members(backend):
    result = base.members(backend)
    for name in ['native-cgi-runtime-contract.json', 'wordpress-source.json',
                 'linux-' + backend + '-cgi-runtime-contract.json']:
        result[name] = 'php-wasix-cgi-runtime-probe/evidence/' + name
    return result


def read_archive(backend):
    require(backend in SPECS, 'fixed backend artifact metadata has not arrived')
    spec = SPECS[backend]
    path = ROOT / ('target/php-wasix-ci-evidence/run-' + RUN + '-' + backend + '-1.zip')
    require(path.resolve(strict=True) == path and path.is_file(), 'fixed CI artifact missing/linked')
    equal(path.stat().st_size, spec['bytes'], 'fixed artifact ZIP length')
    wire = path.read_bytes()
    equal((len(wire), sha(wire)), (spec['bytes'], spec['sha256']), 'fixed artifact ZIP identity')
    mapping = members(backend)
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        equal(len(entries), len(mapping), 'exact artifact member count')
        equal({row.filename for row in entries}, set(mapping.values()), 'exact artifact member paths')
        for row in entries:
            require(0 < row.file_size <= 512 * 1024 and not row.is_dir()
                and (row.external_attr >> 16) & 0o170000 != 0o120000 and row.flag_bits & 1 == 0,
                'artifact member type/size/encryption')
        raw = {name: archive.read(member) for name, member in mapping.items()}
    return raw, {name: parse_json(wire) for name, wire in raw.items()}


def original_stage_inputs():
    names = {'.github/workflows/php-wasix-fiber-cgi.yml'}
    for prefix in [base.COMBINED, RUNTIME_PREFIX]:
        for name in base.original_names(prefix):
            suffix = name[len(prefix):]
            if ('/' not in suffix and name.endswith(('.py', '.json'))) or (
                name.startswith(prefix + 'fixtures/') and '/' not in suffix[len('fixtures/'):]) or (
                name.startswith(prefix + 'patches/') and name.endswith('.patch') and '/' not in suffix[len('patches/'):]):
                names.add(name)
    names.update([base.COMBINED + 'evidence/native-cgi-fiber-contract.json',
        RUNTIME_PREFIX + 'evidence/native-cgi-runtime-contract.json',
        'experiments/php-wasix-host-probe/llvm-toolchain.json',
        'experiments/php-wasix-host-probe/prepare_host_llvm.py',
        'experiments/php-wasix-host-probe/unit_check.py'])
    return {name: sha(original(name)) for name in sorted(names)}


def verify_units(value, backend):
    for key, expected in [('outcome', 'fixed_p0_unit_tests_passed'), ('tests_passed', True),
            ('owned_test_reaped', True), ('exit_code', 0), ('backend', backend), ('php_executed', False),
            ('php_cpu_budget_verified', False), ('product_runtime_selected', False),
            ('outer_build_timeout_seconds', 1200), ('outer_list_timeout_seconds', 10),
            ('outer_test_timeout_seconds', 30), ('build_exit_code', 0), ('list_exit_code', 0)]:
        equal(value[key], expected, 'original Linux31 unit ' + key)
    equal(value['platform']['system'], 'Linux', 'Linux31 unit platform')
    names = value['listed_tests']
    equal((len(names), len(set(names))), (31, 31), 'original unique Linux31 inventory')
    groups = {'cgi_contract::cgi_response::tests::': 9, 'language_contract::tests::': 2,
        'context_contract::tests::': 1, 'fiber_adapter::tests::': 1, 'memory_limits::': 3,
        'rpc_fs::tests::': 9, 'runtime_fixture_fs::tests::': 4, 'metering_limits::': 2}
    for prefix, count in groups.items():
        equal(sum(name.startswith(prefix) for name in names), count, 'original unit group ' + prefix)
    for key in ['owned_list_pid', 'owned_test_pid']: integer(value[key], 1)
    for key in ['test_artifact_sha256', 'stdout_sha256', 'stderr_sha256']: digest(value[key])
    require('test result: ok. 31 passed; 0 failed; 0 ignored;' in value['stdout_excerpt'], 'all 31 units executed without ignores')
    summaries = base.unit_checker.validate_metering_rows(value['wat_metering_observations'], backend, 'Linux')
    equal(value['wat_metering_summaries'], summaries, 'original WAT observations and summaries')


def verify_preparation(receipt, raw_wire):
    manifest = parse_json(original(RUNTIME_PREFIX + 'manifest.json'))
    source_manifest_wire = original('experiments/php-wasi-host-probe/wordpress-fixture.json')
    source_manifest = parse_json(source_manifest_wire)
    wp = manifest['wordpress']
    equal(wp['version'], '7.1.3', 'fixed WP version')
    equal(wp['source_manifest_sha256'], sha(source_manifest_wire), 'fixed pure WP source manifest')
    equal(wp['files'], source_manifest['files'], 'same three pinned WP source identities')
    equal(set(wp['files']), {'plugin.php', 'class-wp-hook.php', 'class-wp-filter-sentinel.php'}, 'only three hook files')
    equal(wp['archive_sha256'], source_manifest['archive_sha256'], 'same fixed source archive')
    expected_inputs = {name: sha(original(name)) for name in [RUNTIME_PREFIX + 'prepare.py',
        RUNTIME_PREFIX + 'manifest.json', 'experiments/php-wasi-host-probe/wordpress-fixture.json',
        'scripts/wordpress_reference.py', 'experiments/php-wasix-host-probe/language_check.py']}
    files = {name: {'path': 'target/php-wasix-cgi-runtime-probe/wp-core/' + name, **identity}
        for name, identity in wp['files'].items()}
    archive = {'path': wp['archive_path'], 'bytes': wp['archive_bytes'], 'sha256': wp['archive_sha256']}
    for key, expected in [('schema', 'recastpress.php-wasix-cgi-runtime-source.v0.5'),
            ('outcome', 'fixed_three_wordpress_sources_prepared'), ('wordpress_executed', False),
            ('native_php_executed', False), ('network_used_by_preparer', False), ('inputs_unchanged', True),
            ('input_files', expected_inputs), ('files', files), ('archive', archive)]:
        equal(receipt[key], expected, 'WP preparation ' + key)
    integer(receipt['archive_members_checked'], 1, 10000)
    integer(receipt['declared_expanded_bytes_checked'], 1, 256 * 1024 * 1024)
    return {'receipt_sha256': sha(raw_wire), 'input_files': expected_inputs, 'files': files, 'archive': archive}


def json_identity(value):
    wire = json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True, allow_nan=False).encode('utf-8')
    return {'bytes': len(wire), 'sha256': sha(wire)}


def short_value(value):
    identity = json_identity(value)
    if identity['bytes'] <= 2048: return value
    return {'value_omitted': True, 'type': type(value).__name__, 'canonical_json': identity}


def native_differences(observed, native, path=''):
    """JSON Pointer paths, exact PHP JSON types and values; never normalize data."""
    if type(observed) is not type(native):
        return [{'path': path or '/', 'kind': 'type', 'observed': short_value(observed), 'native': short_value(native)}]
    if type(native) is dict:
        result = []
        for key in sorted(set(observed) | set(native)):
            child = path + '/' + key.replace('~', '~0').replace('/', '~1')
            if key not in observed: result.append({'path': child, 'kind': 'missing_observed', 'native': short_value(native[key])})
            elif key not in native: result.append({'path': child, 'kind': 'extra_observed', 'observed': short_value(observed[key])})
            else: result.extend(native_differences(observed[key], native[key], child))
            if len(result) > 128: break
        return result[:129]
    if type(native) is list:
        result = []
        if len(observed) != len(native):
            result.append({'path': path or '/', 'kind': 'length', 'observed': len(observed), 'native': len(native)})
        for index, (actual, expected) in enumerate(zip(observed, native)):
            result.extend(native_differences(actual, expected, path + '/' + str(index)))
            if len(result) > 128: break
        return result[:129]
    return [] if observed == native else [{'path': path or '/', 'kind': 'value', 'observed': short_value(observed), 'native': short_value(native)}]


def verify_failed_case(case, index, oracle):
    for key, expected in [('name', ['normal_first', 'normal_repeat', 'normal_final'][index]),
            ('request_number', index + 1), ('module_delivery', 'Module.clone'), ('module_compile_invocations', 0),
            ('fresh_wasi_runtime', True), ('fresh_scratch', True), ('expectation_met', False), ('native_semantics_match', False),
            ('guest_arguments', checker.native.command_arguments()),
            ('guest_environment_supplied', checker.native.expected_environment())]:
        equal(case[key], expected, 'failed runtime request scope ' + key)
    require(type(case.get('expectation_error')) is str and case['expectation_error'], 'retained runtime request error')
    summary = {'name': case['name'], 'expectation_met': False, 'native_semantics_match': False,
        'php_run_completed': case['ok'], 'expectation_error': case['expectation_error'],
        'stdout_capture_binding_verified': False, 'native_comparison_passed': False}
    counters = ['ordinary_write_calls', 'ordinary_bytes_written', 'ordinary_truncate_opens', 'ordinary_set_len_calls',
        'readonly_mutating_open_rejections', 'readonly_handle_mutation_rejections', 'namespace_mutation_rejections',
        'open_handles', 'closed_handles']
    initial = {'ordinary_hex': '', 'readonly_hex': b'readonly fixture\n'.hex(), **{key: 0 for key in counters}}
    equal(case['scratch_initial'], initial, 'failed request starts with fresh scratch')
    final = case['scratch_final']
    equal(set(final), set(initial), 'complete retained VFS snapshot')
    for key in counters: integer(final[key], 0, 65536)
    for key in ['ordinary_hex', 'readonly_hex']:
        require(type(final[key]) is str and len(final[key]) <= 8192, 'retained VFS bytes quota')
        bytes.fromhex(final[key])
    summary['scratch_final'] = copy.deepcopy(final)
    summary['scratch_vfs_observations'] = {
        'ordinary_equals_12_byte_fixture': final['ordinary_hex'] == b'ordinary\0\xff\r\n'.hex(),
        'readonly_equals_unchanged_17_byte_fixture': final['readonly_hex'] == initial['readonly_hex'],
        'readonly_mutating_open_rejection_observed': final['readonly_mutating_open_rejections'] >= 1,
        'open_handles_after_request': final['open_handles']}
    wire = bytes.fromhex(case['stdout_hex'])
    integer(len(wire), 0, 65536)
    equal(case['stdout_bytes'], len(wire), 'failed raw stdout byte count')
    equal(case['stdout_sha256'], sha(wire), 'failed raw stdout hash')
    equal(case['stdout_eof'], True, 'failed request complete stdout capture')
    equal(case['stderr_eof'], True, 'failed request complete stderr capture')
    equal(case['stdout_overflow'], False, 'failed request no stdout overflow')
    equal(case['stderr_overflow'], False, 'failed request no stderr overflow')
    summary['stdout_capture_binding_verified'] = True
    summary['stdout'] = {'bytes': len(wire), 'sha256': sha(wire)}
    response = case.get('response')
    if response is None:
        summary['complete_cgi_response_reparsed'] = False
        summary['json_observation_available'] = False
        return summary
    body = bytes.fromhex(response['body_hex'])
    equal(response['body_bytes'], len(body), 'failed captured CGI body count')
    equal(response['body_sha256'], sha(body), 'failed captured CGI body digest')
    require(wire.endswith(body), 'failed body must match complete raw stdout suffix')
    try:
        parsed_response, parsed_body, _ = checker.native.parse_completed_cgi(wire)
    except (ValueError, TypeError, KeyError) as error:
        summary['complete_cgi_response_reparsed'] = False
        summary['native_cgi_parser_error'] = str(error)[:2048]
    else:
        equal(parsed_response, response, 'failed raw/decoded CGI response consistency')
        equal(parsed_body, body, 'failed raw/decoded CGI body consistency')
        summary['complete_cgi_response_reparsed'] = True
    summary['body'] = {'bytes': len(body), 'sha256': sha(body)}
    if not case.get('guest_json_decoded'):
        summary['json_observation_available'] = False
        return summary
    observation = parse_json(body)
    equal(observation, case['guest_json'], 'failed raw/decoded PHP JSON consistency')
    summary['json_observation_available'] = True
    summary['php_json_identity'] = json_identity(observation)
    differences = native_differences(observation, oracle['observation'])
    summary['native_difference_count_retained'] = min(len(differences), 128)
    summary['native_differences_truncated'] = len(differences) > 128
    summary['native_differences'] = differences[:128]
    observed_cases, native_cases = observation.get('cases'), oracle['observation']['cases']
    if type(observed_cases) is dict:
        summary['observed_case_statuses'] = {name: value.get('status') for name, value in observed_cases.items() if type(value) is dict}
        summary['case_names_identical_to_native'] = [name for name, value in observed_cases.items()
            if name in native_cases and type(value) is type(native_cases[name]) and not native_differences(value, native_cases[name])]
        readonly = observed_cases.get('readonly_io', {}).get('value', {})
        summary['observed_readonly_php_metadata'] = {key: readonly.get(key) for key in ['readable', 'writable', 'write_result', 'unchanged']}
        scratch_php = observed_cases.get('scratch_io', {}).get('value', {})
        summary['scratch_vfs_bytes_match_php_io_observations'] = (
            scratch_php.get('read_hex') == final['ordinary_hex'] and
            readonly.get('before_hex') == final['readonly_hex'] and readonly.get('after_hex') == final['readonly_hex'])
        summary['readonly_php_metadata_vs_actual_vfs_denial_disagree'] = (
            readonly.get('writable') is True and final['readonly_mutating_open_rejections'] >= 1 and
            readonly.get('write_result') is False)
    return summary


def verify_runtime_failure(probe, oracle, backend, provenance, prepared):
    equal(probe['runtime_contract_passed'], False, 'failed runtime verdict retained')
    require(probe['outcome'] != 'fixed_php_cgi_runtime_contract_passed', 'failed runtime record became successful')
    equal(probe['owned_host_reaped'], True, 'failed runtime owned host reaped')
    require(probe['exit_code'] != 0, 'this fixed failure must originate in the runtime host')
    value = probe['native_result']
    for key, expected in [('schema', 'recastpress.php-wasix-cgi-runtime-host.v0.5'), ('ok', False),
            ('engine', 'wasmer-7.2.0/' + backend), ('process_pid', probe['owned_process_pid']),
            ('phase', 'cgi_runtime_execution'), ('runtime_contract_verified_for_fixed_cases', False),
            ('php_metadata_matches_actual_readonly_denial', False), ('module_compile_invocations', 1),
            ('full_wordpress_bootstrap_executed', False), ('real_db', False), ('full_php_compatibility_verified', False),
            ('product_runtime_selected', False), ('memory_limit_verified', False), ('cpu_limit_verified', False),
            ('all_resources_immediately_reclaimed', False), ('native_runtime_fallback_used', False),
            ('php_fiber_compatibility_verified_for_fixed_cases', False), ('sysroot_source_build_attestation_verified', False),
            ('physical_stack_free_verified', False), ('physical_stack_reuse_verified', False), ('stack_guards_verified', False),
            ('persistent_requests_supported', False), ('zts_supported', False), ('engine_supports_async', True)]:
        equal(value[key], expected, 'failed runtime host ' + key)
    require(type(value.get('error')) is str and value['error'], 'failed host error retained')
    for key, expected in [('path', checker.combined.MODULE.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_sha256', provenance['module']['sha256']), ('observed_bytes', provenance['module']['bytes'])]:
        equal(value['artifact'][key], expected, 'failed host combined artifact ' + key)
    for key, expected in [('path', checker.combined.RECEIPT.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_sha256', provenance['build_receipt_sha256']), ('full_source_tool_chain_verified_by_host', False)]:
        equal(value['build_receipt'][key], expected, 'failed host source build receipt ' + key)
    equal(value['isolation'], {'asynchronous_threading': True, 'deep_sleep': False, 'max_guest_threads': 1,
        'host_environment_forwarded': False, 'home_mapped': False, 'guest_environment_supplied': checker.native.expected_environment(),
        'host_directories_mapped': False, 'guest_mounts': ['/fixture', '/wp-core', '/scratch'],
        'networking': 'UnsupportedVirtualNetworking', 'http_client': False, 'stdin': 'closed-pipe',
        'stdout_limit_bytes': 65536, 'stderr_limit_bytes': 65536, 'other_virtual_root_writes_forbidden': False,
        'writable_fixture_entries': ['/scratch/ordinary.txt'], 'scratch_file_limit_bytes': 4096}, 'failed host capability observations')
    equal(value['source_files'], checker.sources(checker.combined.embedded_sources(), read_source=original), 'failed host combined source bindings')
    equal(value['runtime_source_files'], checker.sources(checker.runtime_sources(), read_source=original), 'failed host runtime source bindings')
    equal(value['wordpress_source_files'], {name: {**row, 'verified': True} for name, row in prepared['files'].items()}, 'failed host WP3 source bindings')
    checker.combined.validate_inspection(value['postopt_adapter'], provenance['module'])
    equal(len(value['cases']), 3, 'three retained failed runtime requests')
    return {'runtime_contract_passed': False, 'outcome': probe['outcome'], 'host_phase': value['phase'],
        'host_error': value['error'], 'postopt_adapter_verified': True,
        'observed_requests': [verify_failed_case(case, index, oracle) for index, case in enumerate(value['cases'])],
        'full_php_compatibility_verified': False, 'full_wordpress_bootstrap_executed': False,
        'php_metadata_matches_actual_readonly_denial': False, 'product_runtime_selected': False}


def verify(raw, data, backend):
    spec = SPECS[backend]
    for value in data.values():
        if 'input_files' in value: base.verify_sources(value)
    stages, build = data['ci-stages.json'], data['build.json']
    equal((stages['source_commit'], stages['run_id'], stages['run_attempt'], stages['backend']),
        (SOURCE, RUN, '1', backend), 'runtime CI identity')
    equal(stages['input_files'], original_stage_inputs(), 'complete original workflow input set')
    expected_stages = dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions',
        'tools', 'build', 'rust', 'host_llvm', 'units', 'wordpress_source', 'fiber_contract', 'runtime_contract'], 'success')
    expected_stages['host_llvm'] = 'success' if backend == 'llvm' else 'skipped'
    expected_stages['runtime_contract'] = spec['runtime_verdict']
    equal(stages['stages'], expected_stages, 'original runtime CI stages')
    equal(stages['product_runtime_selected'], False, 'no product adoption')
    equal(stages['redistributed_binaries'], False, 'JSON-only bundle')
    inputs = base.verify_build(data, raw)
    verify_units(data['linux-' + backend + '-unit-contract.json'], backend)
    for name, prefix in [('native-cgi-fiber-contract.json', base.COMBINED),
                         ('native-cgi-runtime-contract.json', RUNTIME_PREFIX),
                         ('windows-cranelift-unit-contract.json', 'experiments/php-wasix-host-probe/')]:
        equal(raw[name], original(prefix + 'evidence/' + name), 'original unexecuted reference unchanged')
    checker.combined.verify_native_reference(data['native-cgi-fiber-contract.json'], read_source=original)
    oracle = checker.verify_native_reference(read_source=original)
    equal(oracle, data['native-cgi-runtime-contract.json'], 'same original native runtime oracle')
    prepared = verify_preparation(data['wordpress-source.json'], raw['wordpress-source.json'])
    fiber, runtime = data['linux-' + backend + '-fiber-cgi-contract.json'], data['linux-' + backend + '-cgi-runtime-contract.json']
    provenance = fiber['generated_provenance']
    equal(provenance['build_receipt_sha256'], sha(raw['build.json']), 'same generated build receipt')
    equal(provenance['module'], build['php_wasm'], 'same generated module')
    equal(provenance['builder_provenance']['raw_module_sha256'], build['raw_wasm']['sha256'], 'same raw module')
    checker.combined.validate_provenance(provenance, read_source=original, build_inputs=inputs)
    for key, expected in [('outcome', 'fixed_php_fiber_cgi_contract_passed'), ('fiber_contract_passed', True),
            ('owned_host_reaped', True), ('exit_code', 0), ('product_runtime_selected', False), ('full_php_compatibility_verified', False)]:
        equal(fiber[key], expected, 'same-candidate Fiber regression ' + key)
    checker.combined.validate_result(fiber['native_result'], fiber['owned_process_pid'], backend, provenance,
        read_source=original, build_inputs=inputs)
    equal(runtime['generated_provenance'], provenance, 'runtime and Fiber use same generated candidate')
    equal(runtime['native_artifact_sha256'], fiber['native_artifact_sha256'], 'both modes use same native Rust binary')
    equal(runtime['wordpress_preparation'], prepared, 'runtime/preparer same verified WP3 inputs')
    for key in ['product_runtime_selected', 'full_php_compatibility_verified', 'native_runtime_fallback_used',
            'all_resources_immediately_reclaimed', 'memory_limit_verified', 'cpu_limit_verified', 'stack_free_or_reuse_verified']:
        equal(runtime[key], False, 'unclaimed runtime scope ' + key)
    if spec['runtime_verdict'] == 'success':
        for key, expected in [('outcome', 'fixed_php_cgi_runtime_contract_passed'), ('runtime_contract_passed', True),
                ('owned_host_reaped', True), ('exit_code', 0)]:
            equal(runtime[key], expected, 'successful fixed runtime ' + key)
        checker.validate_result(runtime['native_result'], runtime['owned_process_pid'], backend, provenance, prepared,
            read_source=original, build_inputs=inputs)
        runtime_summary = {'runtime_contract_passed': True, 'fixed_native_cases': 6, 'fresh_requests': 3,
            'physical_ordinary_fixture_bytes': 12, 'physical_readonly_fixture_bytes': 17,
            'php_metadata_matches_actual_readonly_denial': True, 'full_php_compatibility_verified': False,
            'full_wordpress_bootstrap_executed': False, 'product_runtime_selected': False}
    else:
        equal(spec['runtime_verdict'], 'failure', 'fixed failure verdict required')
        runtime_summary = verify_runtime_failure(runtime, oracle, backend, provenance, prepared)
    if backend == 'llvm':
        llvm, manifest = data['llvm-toolchain-verification.json'], parse_json(original('experiments/php-wasix-host-probe/llvm-toolchain.json'))
        for key, expected in [('llvm_toolchain_manifest_sha256', sha(original('experiments/php-wasix-host-probe/llvm-toolchain.json'))),
                ('archive_bytes', manifest['archive_bytes']), ('archive_sha256', manifest['archive_sha256']), ('llvm_config_version', '22.1.8')]:
            equal(llvm[key], expected, 'separate host LLVM pin ' + key)
    return {'combined_build_passed': True, 'canonical_patch_sets_applied': 2, 'bounded_linux_units_passed': 31,
        'fixed_native_fiber_cases_passed': 6, 'fresh_fiber_requests': 3, 'wordpress_pure_source_files_prepared': 3,
        'module_bytes': build['php_wasm']['bytes'], 'module_sha256': build['php_wasm']['sha256'],
        'runtime': runtime_summary, 'product_runtime_selected': False, 'source_or_license_attestation_verified': False,
        'all_resources_immediately_reclaimed': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['cranelift', 'llvm'], required=True)
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    raw, data = read_archive(args.backend)
    observations = verify(raw, data, args.backend)
    if not args.verify_only:
        destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'import destination escaped experiment')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'raw import destination is linked')
            if path.exists(): equal(path.read_bytes(), wire, 'raw evidence immutable')
            else: path.write_bytes(wire)
        inputs = [Path(__file__).resolve(), COMBINED / 'import_ci_evidence.py', COMBINED / 'fiber_cgi_check.py',
            COMBINED / 'native_contract.py', HERE / 'runtime_check.py', HERE / 'native_contract.py', HERE / 'prepare.py',
            HOST / 'import_ci_evidence.py', HOST / 'language_check.py', HOST / 'unit_check.py', HOST / 'check.py',
            COMBINED / 'build.py', COMBINED / 'patch_source.py', ROOT / 'experiments/php-wasix-cgi-build/prepare.py',
            ROOT / 'experiments/php-runtime-candidate-probe/native_contract.py',
            ROOT / 'experiments/php-wasix-fiber-probe/native_contract.py', ROOT / 'scripts/owned_probe.py']
        spec = SPECS[args.backend]
        provenance = {'format_version': 1, 'kind': 'source_verified_original_cgi_runtime_io_ci_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'ci_verdict': spec['runtime_verdict'], 'original_git_source_hashes_verified': True,
            'php_executed_by_importer': False, 'native_or_wasm_binaries_acquired_by_importer': False,
            'source_archive_acquired_by_importer': False, 'source_tool_or_module_binaries_reverified_by_importer': False,
            'raw_failed_observations_preserved': spec['runtime_verdict'] == 'failure',
            'product_runtime_selected': False, 'full_php_compatibility_verified': False,
            'archive_member_map': members(args.backend),
            'input_files': {path.relative_to(ROOT).as_posix(): sha(path.read_bytes()) for path in inputs},
            'imported_files': {name: sha(wire) for name, wire in raw.items()}, 'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'derived provenance is linked')
        wire = (json.dumps(provenance, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'derived runtime import summary exceeds public quota')
        path.write_bytes(wire)
    print('Verified original CGI runtime ' + args.backend + ' evidence; runtime verdict=' + SPECS[args.backend]['runtime_verdict'] + '.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print('CGI runtime evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
