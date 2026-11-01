"""Import the fixed managed CGI/RPC checkpoint against its original Git tree.

The earlier managed imports and raw observations remain unchanged. All readers
in this private verifier graph are switched before reading any historical blob.
Current resource-probe additions are excluded by original-tree AST/glob reads.
Unset artifact identities/member/stage/verdict pins fail closed. No PHP, Wasm,
compiler, native oracle or source/tool binary is executed or acquired here.
"""
import argparse
import ast
import importlib.util
import json
from pathlib import Path
import sys
from types import ModuleType

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
SOURCE = 'bc628143449a924653d23845568845cd0baeb1a9'
RUN = '41356526706'
HOST = 'experiments/php-wasix-host-probe/'
PREFIX = 'experiments/php-wasix-managed-access-probe/'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cgi_import = load('rp_managed_rpc_ci_private_cgi_import', HERE / 'import_cgi_evidence.py')
managed_import = cgi_import.previous
base = managed_import.base
for module in [cgi_import, managed_import, managed_import.prior, base]:
    module.SOURCE, module.RUN = SOURCE, RUN
base.reader.COMMIT = SOURCE
base.original.cache_clear()
original, equal, require = managed_import.original, managed_import.equal, managed_import.require
sha, integer, digest, parse_json = managed_import.sha, managed_import.integer, managed_import.digest, managed_import.parse_json
rpc = load('rp_managed_rpc_ci_historical_checker', HERE / 'rpc_check.py')
legacy_members, legacy_stages = cgi_import.members, cgi_import.stages
SPECS = {backend: {'artifact': None, 'bytes': None, 'sha256': None, 'members': None, 'stages': None,
    'fiber_verdict': None, 'runtime_verdict': None, 'cgi_verdict': None, 'rpc_verdict': None}
    for backend in ['cranelift', 'llvm']}
SPECS['llvm'].update(artifact=13104640645, bytes=107522,
    sha256='7fe352e8a7f149cc64ff953a1f7afeca59e7f54be0e083d3b95e943a90e9ccd2',
    fiber_verdict='success', runtime_verdict='success', cgi_verdict='success', rpc_verdict='success')
SPECS['cranelift'].update(artifact=13104037182, bytes=107134,
    sha256='654f38f462c1f75a776f2bbf9e28b8b35d83117c1df94ae211b875ac0b1072b3',
    fiber_verdict='success', runtime_verdict='success', cgi_verdict='success', rpc_verdict='success')


def members(backend):
    result = legacy_members(backend)
    name = 'linux-' + backend + '-managed-rpc-contract.json'
    result[name] = 'php-wasix-managed-access-probe/evidence/' + name
    return result


def stages(backend):
    result = legacy_stages(backend)
    result['rpc_contract'] = 'success'
    return result


SPECS['llvm'].update(members=members('llvm'), stages=stages('llvm'))
SPECS['cranelift'].update(members=members('cranelift'), stages=stages('cranelift'))


def pinned_spec(backend):
    require(backend in SPECS, 'closed managed RPC CI backend')
    spec = SPECS[backend]
    integer(spec['artifact'], 1, 2**63); integer(spec['bytes'], 1, 8 * 1024 * 1024); digest(spec['sha256'])
    equal(spec['members'], members(backend), 'exact managed RPC artifact member pin')
    equal(spec['stages'], stages(backend), 'exact fifteen-stage success pin')
    for kind in ['fiber', 'runtime', 'cgi', 'rpc']:
        equal(spec[kind + '_verdict'], 'success', 'fixed observed successful mode')
    return spec


def original_compile_inputs():
    """Use bc6281's source50, never today's resource-extended source52."""
    tree = ast.parse(original(HOST + 'check.py'))
    matches = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'compiled_inputs']
    require(len(matches) == 1, 'original compile-input function')
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
    equal(len(names), 50, 'original complete compile inventory')
    require(PREFIX + 'fixtures/rpc_cgi_contract.php' in names, 'original RPC fixture compiled')
    require(not any(name.endswith(('memory_cgi_contract.php', 'operation_cgi_contract.php')) for name in names),
        'later resource fixtures cannot enter original RPC source set')
    return names


def original_cgi_inputs(native, fiber_native, runtime_native, prepared):
    result = managed_import.original_comparison_inputs([fiber_native, runtime_native],
        managed_import.original_build_inputs(), prepared)
    result.update(managed_import.hashes([PREFIX + 'cgi_check.py', PREFIX + 'managed_check.py',
        HOST + 'cgi_check.py', HOST + 'native_cgi_contract.py', cgi_import.NATIVE] + list(native['input_files'])))
    equal(len(result), 82, 'original source-bound CGI input count')
    return dict(sorted(result.items()))


# Only private module globals select the new historical profile. Actual input
# JSON and guest/native values pass to the unchanged pure validators directly.
cgi_import.SPECS = managed_import.SPECS = SPECS
cgi_import.pinned_spec = managed_import.pinned_spec = pinned_spec
cgi_import.members = managed_import.expected_members = members
cgi_import.stages = stages
managed_import.original_compile_inputs = original_compile_inputs
cgi_import.original_cgi_inputs = original_cgi_inputs


def original_rpc_inputs(data, prepared):
    result = original_cgi_inputs(data['native-cgi-contract.json'], data['native-cgi-fiber-contract.json'],
        data['native-cgi-runtime-contract.json'], prepared)
    result.update(managed_import.hashes([PREFIX + 'rpc_check.py', PREFIX + 'fixtures/rpc_cgi_contract.php',
        HOST + 'rpc_check.py', HOST + 'src/rpc_contract.rs']))
    equal(len(result), 84, 'original complete RPC input count')
    return dict(sorted(result.items()))


def verify(raw, data, backend):
    observations = cgi_import.verify(raw, data, backend)
    prepared = managed_import.prior.verify_preparation(data['wordpress-source.json'], raw['wordpress-source.json'])
    probe = data['linux-' + backend + '-managed-rpc-contract.json']
    fiber = data['linux-' + backend + '-managed-fiber-contract.json']
    provenance = fiber['generated_provenance']
    inputs = original_rpc_inputs(data, prepared)
    for key, expected in [('schema', 'recastpress.php-wasix-managed-rpc-comparison.v0.5'),
            ('scope', rpc.SCOPE), ('variant', rpc.VARIANT), ('backend', backend),
            ('outcome', 'fixed_managed_rpc_contract_passed'), ('rpc_contract_passed', True),
            ('owned_host_reaped', True), ('owned_cleanup_completed', True), ('exit_code', 0), ('build_exit_code', 0),
            ('outer_build_timeout_seconds', 1200), ('outer_execution_timeout_seconds', 600),
            ('generated_provenance', provenance), ('input_files', inputs),
            ('fixture_derivation', rpc.verify_fixture(read_source=original)),
            ('native_artifact_sha256', fiber['native_artifact_sha256'])]:
        equal(probe[key], expected, 'strict source-bound RPC comparison ' + key)
    for key in ['expected_incomplete_case_accepted', 'native_comparison_performed', 'native_php_executed_by_this_helper',
            'native_runtime_fallback_used', 'product_runtime_selected', 'product_abi_selected', 'product_adopted',
            'wordpress_executed', 'real_db', 'standard_wpdb', 'listener_created', 'managed_access_verified_for_fixed_cases',
            'full_php_compatibility_verified', 'all_resources_immediately_reclaimed', 'memory_limit_verified', 'cpu_limit_verified']:
        equal(probe[key], False, 'RPC unclaimed scope ' + key)
    equal(probe['platform']['system'], 'Linux', 'RPC platform')
    equal(probe['observed_phases'], ['module_compile'] + rpc.original.CASES, 'one compilation/four unchanged cases')
    integer(probe['owned_process_pid'], 1); digest(probe['native_artifact_sha256'])
    rpc.validate_result(probe['native_result'], probe['owned_process_pid'], backend, provenance,
        read_source=original, build_inputs=managed_import.original_build_inputs())
    value = probe['native_result']
    observations['rpc'] = {'comparison_passed': True, 'fixed_cases': 4, 'module_compile_calls': 1,
        'compile_elapsed_ms': value['compile_elapsed_ms'], 'fixture': value['fixture'],
        'original_php_bodies_byte_identical': True, 'raw_cgi_stdout_preserved': True,
        'expected_incomplete_case_accepted': False, 'incomplete_frame_rejected': True,
        'permission_query_execution_verified': False,
        'observed_cases': [{'name': case['name'], 'nonce': case['nonce'], 'php_completed': case['php_completed'],
            'ok': case['ok'], 'rpc_valid': case['rpc_valid'], 'expectation_met': case['expectation_met'],
            'stdout_bytes': case['stdout_bytes'], 'stdout_sha256': case['stdout_sha256'],
            'rpc': case['rpc']} for case in value['cases']],
        'real_db': False, 'standard_wpdb': False, 'product_runtime_selected': False}
    return observations


def verifier_inputs():
    paths, queue, seen = {Path(__file__).resolve()}, [cgi_import, rpc], set()
    while queue:
        module = queue.pop()
        if id(module) in seen: continue
        seen.add(id(module))
        filename = getattr(module, '__file__', None)
        if not filename: continue
        path = Path(filename).resolve()
        if not path.is_relative_to(ROOT) or path.suffix != '.py': continue
        require(path.is_file(), 'verifier helper disappeared')
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
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'import destination escaped experiment')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'linked raw evidence destination')
            if path.exists(): equal(path.read_bytes(), wire, 'raw JSON immutable')
            else: path.write_bytes(wire)
        spec = pinned_spec(args.backend)
        proof = {'format_version': 1, 'kind': 'source_verified_original_managed_cgi_rpc_ci_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'pinned_stages': spec['stages'], 'pinned_verdicts': {kind: spec[kind + '_verdict'] for kind in ['fiber', 'runtime', 'cgi', 'rpc']},
            'original_git_source_hashes_verified': True, 'php_executed_by_importer': False,
            'native_or_wasm_binaries_acquired_by_importer': False, 'source_archive_acquired_by_importer': False,
            'source_tool_or_module_binaries_reverified_by_importer': False,
            'product_runtime_selected': False, 'full_php_compatibility_verified': False,
            'archive_member_map': spec['members'], 'input_files': verifier_inputs(),
            'imported_files': {name: sha(value) for name, value in raw.items()}, 'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'derived proof linked')
        wire = (json.dumps(proof, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'derived RPC summary quota')
        path.write_bytes(wire)
    print('Verified original managed RPC ' + args.backend + ' evidence.')


if __name__ == '__main__':
    try: main()
    except (Exception, KeyboardInterrupt) as error:
        print('Managed RPC evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
