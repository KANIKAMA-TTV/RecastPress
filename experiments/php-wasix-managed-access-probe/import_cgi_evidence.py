"""Verify the fixed second managed CI run including three native CGI cases.

The first managed run remains immutable in import_ci_evidence.py. This private
reader graph is switched to the new original Git tree before its first read.
Only fixed JSON ZIPs are read; no PHP, Wasm, compiler or native oracle runs.
Actual guest reports and native expectations are never converted or rewritten.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
from types import ModuleType

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
SOURCE = '0f994c31ccf542468460b48d17326a8e212dffcf'
RUN = '41340215839'
HOST = 'experiments/php-wasix-host-probe/'
PREFIX = 'experiments/php-wasix-managed-access-probe/'
NATIVE = HOST + 'evidence/native-cgi-contract.json'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


previous = load('rp_managed_second_ci_private_previous_import', HERE / 'import_ci_evidence.py')
# No module from a different importer process or cached historical blob is reused.
previous.SOURCE = previous.base.SOURCE = previous.base.reader.COMMIT = previous.prior.SOURCE = SOURCE
previous.RUN = previous.base.RUN = previous.prior.RUN = RUN
previous.base.original.cache_clear()
original, equal, require = previous.original, previous.equal, previous.require
sha, integer, digest, parse_json = previous.sha, previous.integer, previous.digest, previous.parse_json
cgi = load('rp_managed_second_ci_historical_cgi_checker', HERE / 'cgi_check.py')
legacy_members = previous.expected_members
legacy_stage_inputs = previous.original_stage_inputs
SPECS = {
    'cranelift': {'artifact': 13098105616, 'bytes': 94949,
        'sha256': '0995888e0ca05a8cd2492c08ad6a1e736a1533ad7ae25a76578eacb184c71407',
        'fiber_verdict': 'success', 'runtime_verdict': 'success', 'cgi_verdict': 'success'},
    'llvm': {'artifact': 13098016429, 'bytes': 95350,
        'sha256': 'f79dc93c6e21dbe04a2d04329e5808b7e3c8b846e78b610678a770a33c225a04',
        'fiber_verdict': 'success', 'runtime_verdict': 'success', 'cgi_verdict': 'success'},
}


def members(backend):
    result = legacy_members(backend)
    name = 'linux-' + backend + '-managed-cgi-contract.json'
    result[name] = 'php-wasix-managed-access-probe/evidence/' + name
    result['native-cgi-contract.json'] = 'php-wasix-host-probe/evidence/native-cgi-contract.json'
    return result


def stages(backend):
    result = dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions',
        'tools', 'build', 'rust', 'host_llvm', 'units', 'wordpress_source', 'fiber_contract',
        'cgi_contract', 'runtime_contract'], 'success')
    result['host_llvm'] = 'success' if backend == 'llvm' else 'skipped'
    return result


for backend in SPECS:
    SPECS[backend]['members'] = members(backend)
    SPECS[backend]['stages'] = stages(backend)


def pinned_spec(backend):
    require(backend in SPECS, 'closed second managed CI backend')
    spec = SPECS[backend]
    integer(spec['artifact'], 1, 2**63); integer(spec['bytes'], 1, 8 * 1024 * 1024); digest(spec['sha256'])
    equal(spec['members'], members(backend), 'exact second managed JSON member pin')
    equal(spec['stages'], stages(backend), 'exact fourteen-stage success pin')
    for kind in ['fiber', 'runtime', 'cgi']:
        equal(spec[kind + '_verdict'], 'success', 'fixed observed successful mode')
    return spec


def original_stage_inputs():
    result = legacy_stage_inputs()
    result.update(previous.hashes([NATIVE, HOST + 'cgi_check.py', HOST + 'native_cgi_contract.py']))
    return dict(sorted(result.items()))


# This only changes the private verifier graph's closed profile. Neither raw
# receipt values nor the first importer's file or evidence are changed.
previous.SPECS = SPECS
previous.expected_members = members
previous.pinned_spec = pinned_spec
previous.original_stage_inputs = original_stage_inputs


def original_cgi_inputs(native, fiber_native, runtime_native, prepared):
    inputs = previous.original_comparison_inputs([fiber_native, runtime_native],
        previous.original_build_inputs(), prepared)
    inputs.update(previous.hashes([PREFIX + 'cgi_check.py', PREFIX + 'managed_check.py',
        HOST + 'cgi_check.py', HOST + 'native_cgi_contract.py', NATIVE] + list(native['input_files'])))
    equal(len(inputs), 80, 'original complete CGI comparison source count')
    return dict(sorted(inputs.items()))


def verify(raw, data, backend):
    # Includes original-tree compile48/build27/stage input sets, source-bound
    # native Fiber/runtime, tools/three patches/config/module, WP3 and Linux37.
    observations = previous.verify(raw, data, backend)
    equal(raw['native-cgi-contract.json'], original(NATIVE), 'original native CGI reference bytes')
    native = data['native-cgi-contract.json']
    cgi.baseline.validate_native_baseline(native, read_source=original)
    prepared = previous.prior.verify_preparation(data['wordpress-source.json'], raw['wordpress-source.json'])
    inputs = original_cgi_inputs(native, data['native-cgi-fiber-contract.json'],
        data['native-cgi-runtime-contract.json'], prepared)
    probe = data['linux-' + backend + '-managed-cgi-contract.json']
    fiber = data['linux-' + backend + '-managed-fiber-contract.json']
    runtime = data['linux-' + backend + '-managed-runtime-contract.json']
    provenance = fiber['generated_provenance']
    for key, expected in [('schema', 'recastpress.php-wasix-managed-cgi-comparison.v0.5'),
            ('scope', cgi.SCOPE), ('variant', cgi.VARIANT), ('backend', backend),
            ('outcome', 'fixed_managed_cgi_contract_passed'), ('cgi_contract_passed', True),
            ('native_comparison_performed', True), ('native_comparison_passed', True),
            ('owned_host_reaped', True), ('owned_cleanup_completed', True), ('exit_code', 0),
            ('build_exit_code', 0), ('outer_build_timeout_seconds', 1200), ('outer_execution_timeout_seconds', 600),
            ('generated_provenance', provenance), ('input_files', inputs),
            ('native_reference', cgi.native_reference(native, sha(raw['native-cgi-contract.json']))),
            ('native_artifact_sha256', fiber['native_artifact_sha256'])]:
        equal(probe[key], expected, 'strict second managed CGI ' + key)
    for key in ['managed_access_verified_for_fixed_cases', 'product_runtime_selected', 'product_abi_selected',
            'native_runtime_fallback_used', 'native_php_executed_by_this_helper', 'wordpress_executed', 'real_db',
            'listener_created', 'full_php_compatibility_verified', 'all_resources_immediately_reclaimed',
            'memory_limit_verified', 'cpu_limit_verified', 'redistributed_binaries']:
        equal(probe[key], False, 'CGI unclaimed scope ' + key)
    equal(probe['platform']['system'], 'Linux', 'CGI platform')
    equal(probe['observed_phases'], ['module_compile', 'cgi_request_1', 'cgi_request_2', 'cgi_request_3'], 'CGI observed execution phases')
    equal(probe['native_artifact_sha256'], runtime['native_artifact_sha256'], 'same Rust binary for all three modes')
    integer(probe['owned_process_pid'], 1); digest(probe['native_artifact_sha256'])
    cgi.validate_result(probe['native_result'], probe['owned_process_pid'], backend, provenance,
        read_source=original, build_inputs=previous.original_build_inputs())
    comparisons = cgi.baseline.compare_native_requests(probe['native_result'], native)
    equal(probe['native_comparison_results'], comparisons, 'native comparisons recomputed from actual unchanged observations')
    require(len(comparisons) == 3 and all(row['equal_to_native'] is True for row in comparisons.values()),
        'all three CGI cases must match the unchanged native oracle')
    requests = probe['native_result']['requests']
    observations['cgi'] = {'comparison_passed': True, 'fixed_native_cases': 3, 'fresh_requests': 3,
        'module_compile_calls': 1, 'compile_elapsed_ms': probe['native_result']['compile_elapsed_ms'],
        'request_names': [row['name'] for row in requests],
        'native_comparison_results': comparisons,
        'stdout_bindings': [{'bytes': row['stdout_bytes'], 'sha256': row['stdout_sha256']} for row in requests],
        'binary_body_hex': requests[2]['response']['body_hex'],
        'permission_query_execution_verified': False,
        'native_path_normalization': native['comparison_normalization'],
        'native_json_serialization_bytes_compared': False, 'native_transport_framing_compared': False,
        'actual_guest_cgi_wire_reparsed': True, 'full_php_compatibility_verified': False}
    return observations


def verifier_inputs():
    paths = {Path(__file__).resolve()}
    queue, seen = [previous, cgi], set()
    while queue:
        module = queue.pop()
        if id(module) in seen: continue
        seen.add(id(module))
        filename = getattr(module, '__file__', None)
        if not filename: continue
        path = Path(filename).resolve()
        if not path.is_relative_to(ROOT) or path.suffix != '.py': continue
        require(path.is_file(), 'verifier source disappeared')
        paths.add(path)
        queue.extend(value for value in vars(module).values() if isinstance(value, ModuleType))
    return {path.relative_to(ROOT).as_posix(): sha(path.read_bytes()) for path in sorted(paths)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['cranelift', 'llvm'], required=True)
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    raw, data = previous.read_archive(args.backend)
    observations = verify(raw, data, args.backend)
    if not args.verify_only:
        destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'import destination escaped experiment')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'linked raw import destination')
            if path.exists(): equal(path.read_bytes(), wire, 'raw evidence immutable')
            else: path.write_bytes(wire)
        spec = pinned_spec(args.backend)
        proof = {'format_version': 1, 'kind': 'source_verified_original_managed_access_cgi_ci_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'pinned_stages': spec['stages'], 'pinned_fiber_verdict': 'success',
            'pinned_runtime_verdict': 'success', 'pinned_cgi_verdict': 'success',
            'original_git_source_hashes_verified': True, 'php_executed_by_importer': False,
            'native_or_wasm_binaries_acquired_by_importer': False, 'source_archive_acquired_by_importer': False,
            'source_tool_or_module_binaries_reverified_by_importer': False,
            'product_runtime_selected': False, 'full_php_compatibility_verified': False,
            'archive_member_map': spec['members'], 'input_files': verifier_inputs(),
            'imported_files': {name: sha(value) for name, value in raw.items()}, 'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'derived provenance linked')
        wire = (json.dumps(proof, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'derived summary quota')
        path.write_bytes(wire)
    print('Verified original managed CGI ' + args.backend + ' evidence.')


if __name__ == '__main__':
    try: main()
    except (Exception, KeyboardInterrupt) as error:
        print('Managed CGI evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
