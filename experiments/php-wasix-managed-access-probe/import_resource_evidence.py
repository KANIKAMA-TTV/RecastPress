"""Import fixed CGI resource diagnostics without rewriting expected failures.

The original 57368ee Git tree supplies every compiler/fixture/helper binding.
Private historical reader graphs exclude later development changes. Successful
memory/operation reports go directly to their strict pure validators. Failed
stages retain their pinned outcomes and raw observations without compatibility
or resource-limit claims. Artifact identities and outcomes must arrive first.
No guest, native PHP, compiler, source/tool archive or binary is acquired/run.
"""
import argparse
import ast
import copy
import importlib.util
import json
from pathlib import Path
import sys
from types import ModuleType

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
SOURCE = '57368eeff7f53aef860290a29a665d3163fa27a2'
RUN = '41358796010'
HOST = 'experiments/php-wasix-host-probe/'
PREFIX = 'experiments/php-wasix-managed-access-probe/'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


rpc_import = load('rp_managed_resource_ci_private_rpc_import', HERE / 'import_rpc_evidence.py')
cgi_import, managed_import, base = rpc_import.cgi_import, rpc_import.managed_import, rpc_import.base
for module in [rpc_import, cgi_import, managed_import, managed_import.prior, base]:
    module.SOURCE, module.RUN = SOURCE, RUN
base.reader.COMMIT = SOURCE
base.original.cache_clear()
original, equal, require = managed_import.original, managed_import.equal, managed_import.require
sha, integer, digest, parse_json = managed_import.sha, managed_import.integer, managed_import.digest, managed_import.parse_json
memory = load('rp_managed_resource_ci_historical_memory_checker', HERE / 'memory_check.py')
operation = load('rp_managed_resource_ci_historical_operation_checker', HERE / 'operation_check.py')
legacy_members, legacy_stages = rpc_import.members, rpc_import.stages
SPECS = {backend: {'artifact': None, 'bytes': None, 'sha256': None, 'members': None, 'stages': None,
    'fiber_verdict': None, 'runtime_verdict': None, 'cgi_verdict': None, 'rpc_verdict': None,
    'memory_verdict': None, 'operation_verdict': None, 'memory_outcome': None, 'operation_outcome': None}
    for backend in ['cranelift', 'llvm']}
SPECS['cranelift'].update(artifact=13105465913, bytes=130510,
    sha256='0afeac7500b2030337b497b6485e0c2b7e0d2829474dd278cff28f88a22c849f',
    fiber_verdict='success', runtime_verdict='success', cgi_verdict='success', rpc_verdict='success',
    memory_verdict='success', operation_verdict='success', memory_outcome='fixed_managed_memory_cap_passed',
    operation_outcome='fixed_managed_operation_budget_passed')
SPECS['llvm'].update(artifact=13105916876, bytes=130846,
    sha256='cd4bc878249e5c6f0315c5ed6de0fd183b0453c36b23d1736b71ce5b61a75998',
    fiber_verdict='success', runtime_verdict='success', cgi_verdict='success', rpc_verdict='success',
    memory_verdict='success', operation_verdict='success', memory_outcome='fixed_managed_memory_cap_passed',
    operation_outcome='fixed_managed_operation_budget_passed')


def members(backend):
    result = legacy_members(backend)
    for suffix in ['managed-memory-cap.json', 'managed-operation-budget.json']:
        name = 'linux-' + backend + '-' + suffix
        result[name] = 'php-wasix-managed-access-probe/evidence/' + name
    return result


def stages(backend):
    result = legacy_stages(backend)
    result.update(memory_cap=SPECS[backend]['memory_verdict'], operation_budget=SPECS[backend]['operation_verdict'])
    return result


SPECS['cranelift'].update(members=members('cranelift'), stages=stages('cranelift'))
SPECS['llvm'].update(members=members('llvm'), stages=stages('llvm'))


def pinned_spec(backend):
    require(backend in SPECS, 'closed managed resource backend')
    spec = SPECS[backend]
    integer(spec['artifact'], 1, 2**63); integer(spec['bytes'], 1, 8 * 1024 * 1024); digest(spec['sha256'])
    equal(spec['members'], members(backend), 'exact resource JSON member pin')
    for kind in ['fiber', 'runtime', 'cgi', 'rpc']:
        equal(spec[kind + '_verdict'], 'success', 'fixed prerequisite result')
    for kind in ['memory', 'operation']:
        require(spec[kind + '_verdict'] in ['success', 'failure'], 'resource stage verdict not yet pinned')
        outcome = spec[kind + '_outcome']
        require(type(outcome) is str and 0 < len(outcome) <= 128, 'resource outcome not yet pinned')
    equal(spec['stages'], stages(backend), 'exact seventeen-stage pin')
    return spec


def original_compile_inputs():
    tree = ast.parse(original(HOST + 'check.py'))
    matches = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'compiled_inputs']
    require(len(matches) == 1, 'original compiler input function')
    names = {HOST + 'Cargo.toml', HOST + 'Cargo.lock'}
    for directory, extension in [('src/', '.rs'), ('fixtures/', '.php')]:
        prefix = HOST + directory
        names.update(name for name in base.original_names(prefix)
            if name.endswith(extension) and '/' not in name[len(prefix):])
    for node in ast.walk(matches[0]):
        if (isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div)
                and isinstance(node.left, ast.Name) and node.left.id == 'ROOT'
                and isinstance(node.right, ast.Constant) and type(node.right.value) is str):
            names.add(node.right.value)
    equal(len(names), 52, 'original complete resource compile inventory')
    require(all(PREFIX + 'fixtures/' + name + '_cgi_contract.php' in names for name in ['rpc', 'memory', 'operation']),
        'all original CGI fixtures compiled')
    return names


def original_cgi_inputs(native, fiber_native, runtime_native, prepared):
    result = managed_import.original_comparison_inputs([fiber_native, runtime_native],
        managed_import.original_build_inputs(), prepared)
    result.update(managed_import.hashes([PREFIX + 'cgi_check.py', PREFIX + 'managed_check.py',
        HOST + 'cgi_check.py', HOST + 'native_cgi_contract.py', cgi_import.NATIVE] + list(native['input_files'])))
    equal(len(result), 84, 'original CGI input inventory')
    return dict(sorted(result.items()))


def original_rpc_inputs(data, prepared):
    result = original_cgi_inputs(data['native-cgi-contract.json'], data['native-cgi-fiber-contract.json'],
        data['native-cgi-runtime-contract.json'], prepared)
    result.update(managed_import.hashes([PREFIX + 'rpc_check.py', PREFIX + 'fixtures/rpc_cgi_contract.php',
        HOST + 'rpc_check.py', HOST + 'src/rpc_contract.rs']))
    equal(len(result), 86, 'original RPC input inventory')
    return dict(sorted(result.items()))


for module in [rpc_import, cgi_import, managed_import]:
    module.SPECS = SPECS
    module.pinned_spec = pinned_spec
rpc_import.members = cgi_import.members = managed_import.expected_members = members
rpc_import.stages = cgi_import.stages = stages
rpc_import.original_compile_inputs = managed_import.original_compile_inputs = original_compile_inputs
rpc_import.original_cgi_inputs = cgi_import.original_cgi_inputs = original_cgi_inputs
rpc_import.original_rpc_inputs = original_rpc_inputs


def original_resource_inputs(data, prepared, kind):
    result = original_cgi_inputs(data['native-cgi-contract.json'], data['native-cgi-fiber-contract.json'],
        data['native-cgi-runtime-contract.json'], prepared)
    files = [PREFIX + kind + '_check.py', HOST + ('memory_check.py' if kind == 'memory' else 'operation_check.py')]
    if kind == 'memory': files += [HOST + 'reuse_check.py']
    result.update(managed_import.hashes(files))
    equal(len(result), 87 if kind == 'memory' else 86, 'original resource source inventory')
    return dict(sorted(result.items()))


def failed_resource_observations(probe, kind):
    """Only bind actual capture/snapshot values; never turn failure into a gate."""
    summary = {'comparison_passed': False, 'outcome': probe['outcome'], 'host_result_available': False}
    value = probe.get('native_result')
    if value is None: return summary
    helper = memory if kind == 'memory' else operation
    equal(value['schema'], helper.SCHEMA, 'failed resource host schema')
    equal(value['scope'], helper.SCOPE, 'failed resource host scope')
    equal(value['process_pid'], probe['owned_process_pid'], 'failed resource host identity')
    equal(value['engine'], 'wasmer-7.2.0/' + probe['backend'], 'failed resource backend')
    require(type(value['ok']) is bool, 'actual resource host verdict type')
    summary.update(host_result_available=True, host_ok=value['ok'], host_phase=value['phase'], host_error=value.get('error'))
    cases = value.get('cases', [])
    require(type(cases) is list and len(cases) <= (4 if kind == 'memory' else 3), 'bounded failed resource cases')
    rows = []
    for case in cases:
        row = {key: copy.deepcopy(case[key]) for key in ['name', 'ok', 'php_completed', 'expectation_met',
            'expectation_error', 'execution_error', 'php', 'memory', 'memory_observation_count', 'oom_observed',
            'cgi_response_parsed', 'cgi_response_accepted', 'failed_stdout_completed_json_observed', 'final_points',
            'post_start_points', 'start_charged_points', 'budget_exhausted', 'ready_reached',
            'metering_handle_lifecycle', 'metering_observation_error'] if key in case}
        captures = {}
        for stream in ['stdout', 'stderr']:
            if stream + '_hex' not in case: continue
            wire = bytes.fromhex(case[stream + '_hex']); integer(len(wire), 0, 65536)
            equal(case[stream + '_bytes'], len(wire), 'failed resource raw capture length')
            equal(case[stream + '_sha256'], sha(wire), 'failed resource raw capture digest')
            captures[stream] = wire
            row[stream] = {'bytes': len(wire), 'sha256': sha(wire), 'eof': case.get(stream + '_eof')}
        if kind == 'memory' and 'stdout' in captures:
            found, observed = memory.failed_json_observation(captures['stdout'])
            row['completed_json_observed_in_raw_stdout'] = found
            row['completed_json_value'] = observed if found else None
            # Observing JSON null is parse success, never evidence of OOM.
            row['completed_json_null_observed'] = found and observed is None
        if kind == 'operation' and case.get('metering_handle_lifecycle'):
            try: operation.original.validate_handle_lifecycle(case)
            except (ValueError, KeyError, TypeError): row['typed_handle_lifecycle_shape_verified'] = False
            else: row['typed_handle_lifecycle_shape_verified'] = True
        rows.append(row)
    summary['observed_cases'] = rows
    return summary


def verify_resource(probe, kind, backend, data, prepared):
    spec = pinned_spec(backend)
    helper = memory if kind == 'memory' else operation
    provenance = data['linux-' + backend + '-managed-fiber-contract.json']['generated_provenance']
    inputs = original_resource_inputs(data, prepared, kind)
    timeout = 600 if kind == 'memory' else operation.original.HOST_TIMEOUT_SECONDS[backend]
    for key, expected in [('schema', 'recastpress.php-wasix-managed-' + kind + '-comparison.v0.5'),
            ('scope', helper.SCOPE), ('variant', helper.VARIANT), ('backend', backend),
            ('outcome', spec[kind + '_outcome']), ('outer_build_timeout_seconds', 1200),
            ('outer_execution_timeout_seconds', timeout)]: equal(probe[key], expected, 'source-bound resource comparison ' + key)
    equal(probe['platform']['system'], 'Linux', 'resource platform')
    for key in ['product_runtime_selected', 'product_abi_selected', 'native_comparison_performed',
            'native_php_executed_by_this_helper', 'native_runtime_fallback_used', 'product_adopted', 'wordpress_executed',
            'real_db', 'listener_created', 'managed_access_verified_for_fixed_cases', 'full_php_compatibility_verified',
            'all_resources_immediately_reclaimed', 'cpu_limit_verified']:
        equal(probe[key], False, 'unclaimed resource scope ' + key)
    if 'input_files' in probe: equal(probe['input_files'], inputs, 'complete original resource inputs')
    if 'generated_provenance' in probe: equal(probe['generated_provenance'], provenance, 'same resource PHP candidate')
    if kind == 'memory':
        equal(probe['expected_execution_results'], [True, False, True, True], 'fixed memory expected failures')
        for key in ['production_memory_limit_verified', 'rss_limit_verified', 'compiler_memory_limit_verified',
                'aggregate_memory_limit_verified', 'standard_wpdb']:
            equal(probe[key], False, 'unclaimed memory wrapper scope ' + key)
    else:
        for key in ['production_operation_budget_verified', 'cpu_time_budget_verified', 'php_opcode_budget_verified',
                'hostcall_timeout_verified', 'memory_limit_verified']:
            equal(probe[key], False, 'unclaimed operation wrapper scope ' + key)
    flag = 'diagnostic_passed'
    if spec[kind + '_verdict'] == 'failure':
        equal(probe[flag], False, 'failed resource comparison remains failed')
        if kind == 'operation':
            equal(probe['php_wasm_operation_budget_verified_for_fixed_cases'], False, 'failed operation gate remains false')
        require(probe['outcome'] not in ['fixed_managed_memory_cap_passed', 'fixed_managed_operation_budget_passed'],
            'failed resource outcome cannot become successful')
        return failed_resource_observations(probe, kind)
    success_outcome = 'fixed_managed_memory_cap_passed' if kind == 'memory' else 'fixed_managed_operation_budget_passed'
    for key, expected in [('outcome', success_outcome), (flag, True), ('owned_host_reaped', True),
            ('owned_process_cleanup_completed' if kind == 'operation' else 'owned_cleanup_completed', True),
            ('exit_code', 0), ('build_exit_code', 0), ('input_files', inputs), ('generated_provenance', provenance),
            ('fixture_derivation', helper.verify_fixture(read_source=original))]:
        equal(probe[key], expected, 'strict successful resource ' + key)
    if kind == 'operation':
        equal(probe['php_wasm_operation_budget_verified_for_fixed_cases'], True, 'fixed operation gate')
        equal(probe['normal_host_result_received'], True, 'operation normal host return')
        equal(probe['owned_host_reap_kind'], 'normal-host-return', 'operation host reaped normally')
    integer(probe['owned_process_pid'], 1); digest(probe['native_artifact_sha256'])
    equal(probe['native_artifact_sha256'], data['linux-' + backend + '-managed-fiber-contract.json']['native_artifact_sha256'],
        'same Rust binary for resource modes')
    equal(probe['observed_phases'], ['module_compile'] + helper.original.CASES, 'resource fixed case phases')
    helper.validate_result(probe['native_result'], probe['owned_process_pid'], backend, provenance,
        read_source=original, build_inputs=managed_import.original_build_inputs())
    value = probe['native_result']
    summary = {'comparison_passed': True, 'fixed_cases': len(value['cases']), 'module_compile_calls': 1,
        'compile_elapsed_ms': value['compile_elapsed_ms'], 'fixture': value['fixture'], 'original_php_bodies_byte_identical': True,
        'permission_query_execution_verified': False, 'product_runtime_selected': False}
    if kind == 'memory':
        summary.update(cap_bytes=value['test_cap_bytes'], allocation_bytes=value['allocation_bytes'],
            actual_memory_snapshots=[case['memory'] for case in value['cases']],
            case_ok=[case['ok'] for case in value['cases']],
            failed_stdout_completed_json_observed=value['cases'][1]['failed_stdout_completed_json_observed'],
            failed_cgi_response_accepted=value['cases'][1]['cgi_response_accepted'])
    else:
        summary.update(trial_points=value['trial_points'], budget_reset_count=value['budget_reset_count'],
            start_execution_included=value['start_execution_included'], post_start_reference_points=value['post_start_reference_points'],
            case_ok=[case['ok'] for case in value['cases']],
            observed_cases=[{key: case[key] for key in ['name', 'post_start_points', 'start_charged_points',
                'final_points', 'budget_exhausted', 'ready_reached', 'complete_cgi_response_verified',
                'metering_handle_lifecycle']} for case in value['cases']])
    return summary


def verify(raw, data, backend):
    observations = rpc_import.verify(raw, data, backend)
    prepared = managed_import.prior.verify_preparation(data['wordpress-source.json'], raw['wordpress-source.json'])
    for kind, suffix in [('memory', 'managed-memory-cap.json'), ('operation', 'managed-operation-budget.json')]:
        observations[kind] = verify_resource(data['linux-' + backend + '-' + suffix], kind, backend, data, prepared)
    return observations


def verifier_inputs():
    paths, queue, seen = {Path(__file__).resolve()}, [rpc_import, memory, operation], set()
    while queue:
        module = queue.pop()
        if id(module) in seen: continue
        seen.add(id(module))
        filename = getattr(module, '__file__', None)
        if not filename: continue
        path = Path(filename).resolve()
        if not path.is_relative_to(ROOT) or path.suffix != '.py': continue
        require(path.is_file(), 'resource verifier disappeared')
        paths.add(path)
        queue.extend(value for value in vars(module).values() if isinstance(value, ModuleType))
    return {path.relative_to(ROOT).as_posix(): sha(path.read_bytes()) for path in sorted(paths)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['cranelift', 'llvm'], required=True)
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    raw, data = managed_import.read_archive(args.backend)
    observations = verify(raw, data, args.backend)
    if not args.verify_only:
        destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'resource import destination escaped')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'linked raw resource evidence')
            if path.exists(): equal(path.read_bytes(), wire, 'raw resource evidence immutable')
            else: path.write_bytes(wire)
        spec = pinned_spec(args.backend)
        proof = {'format_version': 1, 'kind': 'source_verified_original_managed_cgi_resource_ci_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'pinned_stages': spec['stages'], 'pinned_resource_outcomes': {kind: spec[kind + '_outcome'] for kind in ['memory', 'operation']},
            'pinned_verdicts': {kind: spec[kind + '_verdict'] for kind in ['fiber', 'runtime', 'cgi', 'rpc', 'memory', 'operation']},
            'original_git_source_hashes_verified': True, 'php_executed_by_importer': False,
            'native_or_wasm_binaries_acquired_by_importer': False, 'source_archive_acquired_by_importer': False,
            'source_tool_or_module_binaries_reverified_by_importer': False,
            'raw_failed_resource_observations_preserved': any(spec[kind + '_verdict'] == 'failure' for kind in ['memory', 'operation']),
            'product_runtime_selected': False, 'full_php_compatibility_verified': False,
            'archive_member_map': spec['members'], 'input_files': verifier_inputs(),
            'imported_files': {name: sha(value) for name, value in raw.items()}, 'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'linked derived resource proof')
        wire = (json.dumps(proof, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'derived resource summary quota')
        path.write_bytes(wire)
    print('Verified original managed resources ' + args.backend + ' evidence.')


if __name__ == '__main__':
    try: main()
    except (Exception, KeyboardInterrupt) as error:
        print('Managed resource evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
