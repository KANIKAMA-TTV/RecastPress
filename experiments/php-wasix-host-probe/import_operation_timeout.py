"""Preserve one pinned LLVM operation-budget timeout without rerunning PHP.

The ZIP identity was observed separately through GitHub's artifact API. This
offline importer accepts no URL, path, revision, backend or result override.
It reads the original af4 Git sources, never the current operation checker.
"""
import ast
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
READER = HERE / 'import_ci_evidence.py'
_spec = importlib.util.spec_from_file_location('recastpress_operation_timeout_original_git_reader', READER)
reader = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(reader)
SOURCE = 'af4bcebe61992d8299f6e5ff0a4f16777fdb8211'
PRIOR_SOURCE = 'a05a2ca1e795e64dacbb2ee0845b6fdc9464da2a'
RUN = '41326284424'
ARTIFACT = 13091962402
ZIP_BYTES = 72052
ZIP_SHA256 = 'd9371657f88e78bab126e06596f0c72e2cb011dc112345a045bcd220ec3d23c7'
ARCHIVE = ROOT / 'target/php-wasix-ci-evidence/run-41326284424-llvm-1.zip'
DESTINATION = HERE / 'evidence/ci-41326284424-llvm-operation-timeout'
OPERATION = 'linux-llvm-php-operation-budget.json'
LANGUAGE = 'linux-llvm-language-contract.json'
NAMES = {
    'ci-stages.json', 'linux-llvm-dependency-inventory.json', LANGUAGE,
    'linux-llvm-module-reuse.json', 'linux-llvm-php-memory-cap.json', OPERATION,
    'linux-llvm-rpc-contract.json', 'linux-llvm-unit-contract.json', 'linux-llvm.json',
    'llvm-toolchain-verification.json', 'memory-policy-windows.json', 'native-cgi-contract.json',
    'windows-cranelift-dependency-inventory.json', 'windows-cranelift-unit-contract.json',
    'windows-cranelift.json', 'windows-llvm-dependency-inventory.json',
}
SAVED = {'ci-stages.json', OPERATION, LANGUAGE}
PRIOR = 'experiments/php-wasix-host-probe/evidence/ci-41320223203-llvm/linux-llvm-php-operation-budget.json'
PRIOR_SHA256 = '1550b9c1e1342a81acdca391179cd229958caef04a1d76ab2293532b08d54a9f'
HOST_PREFIX = 'experiments/php-wasix-host-probe'
RELATED = [HOST_PREFIX + '/' + name for name in
           ['operation_check.py', 'src/php_operation_budget.rs', 'Cargo.toml', 'Cargo.lock']]
_ORIGINAL = {}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def equal(actual, expected, label):
    require(type(actual) is type(expected), 'type mismatch: ' + label)
    if isinstance(expected, dict):
        require(set(actual) == set(expected), 'field mismatch: ' + label)
        for name, value in expected.items():
            equal(actual[name], value, label + '.' + name)
    elif isinstance(expected, list):
        require(len(actual) == len(expected), 'list length mismatch: ' + label)
        for index, value in enumerate(expected):
            equal(actual[index], value, label + '[' + str(index) + ']')
    else:
        require(actual == expected, 'value mismatch: ' + label)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def parse_json(wire):
    def object_pairs(pairs):
        result = {}
        for name, value in pairs:
            require(name not in result, 'duplicate JSON key')
            result[name] = value
        return result

    def bad_number(_):
        raise ValueError('non-finite JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=object_pairs,
                      parse_constant=bad_number)


def original(path, revision=SOURCE):
    key = (revision, path)
    if key not in _ORIGINAL:
        previous = reader.COMMIT
        reader.COMMIT = revision
        try:
            value = reader.original(path)
        finally:
            reader.COMMIT = previous
        require(0 < len(value) <= 16 * 1024 * 1024, 'original Git source byte budget')
        _ORIGINAL[key] = value
    return _ORIGINAL[key]


def original_names():
    result = subprocess.run(['git', 'ls-tree', '-r', '--name-only', SOURCE, '--', HOST_PREFIX],
                            cwd=ROOT, capture_output=True, timeout=30, check=True)
    require(not result.stderr and 0 < len(result.stdout) <= 256 * 1024, 'original Git listing budget')
    names = result.stdout.decode('utf-8', errors='strict').splitlines()
    require(len(names) == len(set(names)), 'duplicate original Git tree entry')
    return set(names)


def declared_operation_inputs():
    """Evaluate only the original fixed path-list syntax, without exec/import.

    Globs use the af4 Git tree, so newly added current .rs/.php files cannot
    alter the historical input set. This covers the checker's declared inputs;
    it does not claim a complete Rust/compiler build dependency graph.
    """
    names = original_names()

    def path(node):
        if isinstance(node, ast.Name) and node.id in {'ROOT', 'HERE'}:
            return '' if node.id == 'ROOT' else HOST_PREFIX
        require(isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div)
                and isinstance(node.right, ast.Constant) and type(node.right.value) is str,
                'unexpected original path expression')
        base = path(node.left)
        return (base + '/' if base else '') + node.right.value

    def paths(node, compiled=None):
        if isinstance(node, ast.List):
            return [path(item) for item in node.elts]
        if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Add):
            return paths(node.left, compiled) + paths(node.right, compiled)
        if (isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
                and isinstance(node.func.value, ast.Name) and node.func.value.id == 'host'
                and node.func.attr == 'compiled_inputs' and not node.args and not node.keywords):
            require(compiled is not None, 'unexpected nested compiled_inputs call')
            return compiled
        require(isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == 'sorted'
                and len(node.args) == 1 and not node.keywords, 'unexpected original input-list expression')
        glob = node.args[0]
        require(isinstance(glob, ast.Call) and isinstance(glob.func, ast.Attribute) and glob.func.attr == 'glob'
                and len(glob.args) == 1 and not glob.keywords and isinstance(glob.args[0], ast.Constant)
                and glob.args[0].value in {'*.rs', '*.php'}, 'unexpected original source glob')
        prefix = path(glob.func.value) + '/'
        suffix = glob.args[0].value[1:]
        result = sorted(name for name in names if name.startswith(prefix)
                        and '/' not in name[len(prefix):] and name.endswith(suffix))
        require(result, 'original source glob is empty')
        return result

    source = ast.parse(original(HOST_PREFIX + '/check.py'))
    compiled = next(node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'compiled_inputs')
    require(len(compiled.body) == 2 and isinstance(compiled.body[0], ast.Expr)
            and isinstance(compiled.body[0].value, ast.Constant)
            and isinstance(compiled.body[1], ast.Return), 'original compiled_inputs shape changed')
    compiled_paths = paths(compiled.body[1].value)
    source = ast.parse(original(HOST_PREFIX + '/operation_check.py'))
    function = next(node for node in source.body if isinstance(node, ast.FunctionDef) and node.name == 'source_hashes')
    require(len(function.body) == 2 and isinstance(function.body[0], ast.Assign)
            and len(function.body[0].targets) == 1 and isinstance(function.body[0].targets[0], ast.Name)
            and function.body[0].targets[0].id == 'paths'
            and isinstance(function.body[1], ast.Return) and isinstance(function.body[1].value, ast.DictComp),
            'original operation source_hashes shape changed')
    result = paths(function.body[0].value, compiled_paths)
    return {name: sha(original(name)) for name in sorted(set(result))}


def verify_sources(value, expected=None):
    inputs = value.get('input_files')
    require(type(inputs) is dict and inputs, 'missing original source inputs')
    if expected is not None:
        equal(inputs, expected, 'complete original declared input set')
    for name, expected_sha in inputs.items():
        require(type(expected_sha) is str and len(expected_sha) == 64, 'invalid original source digest')
        equal(sha(original(name)), expected_sha, 'original Git input ' + name)
    return inputs


def read_archive():
    require(ARCHIVE.resolve(strict=True) == ARCHIVE and ARCHIVE.is_file(), 'linked/missing fixed timeout ZIP')
    wire = ARCHIVE.read_bytes()
    equal((len(wire), sha(wire)), (ZIP_BYTES, ZIP_SHA256), 'fixed GitHub API artifact identity')
    with zipfile.ZipFile(ARCHIVE) as archive:
        entries = archive.infolist()
        equal(len(entries), 16, 'exact artifact member count')
        equal({entry.filename for entry in entries}, NAMES, 'exact artifact member set')
        require(sum(entry.file_size for entry in entries) <= 2 * 1024 * 1024, 'artifact expanded JSON budget')
        for entry in entries:
            require(not entry.is_dir() and 0 < entry.file_size <= 256 * 1024
                    and not entry.flag_bits & 1 and (entry.external_attr >> 16) & 0o170000 != 0o120000,
                    'invalid JSON artifact member')
        raw = {name: archive.read(name) for name in sorted(NAMES)}
    return raw, {name: parse_json(value) for name, value in raw.items()}


def verify(data):
    stages, operation, language = data['ci-stages.json'], data[OPERATION], data[LANGUAGE]
    equal((stages['source_commit'], stages['run_id'], stages['run_attempt'], stages['backend']),
          (SOURCE, RUN, '1', 'llvm'), 'original workflow identity')
    expected_stages = dict.fromkeys(['dependencies', 'rust', 'php', 'cargo', 'llvm', 'fixture_fs',
                                    'probe', 'reuse', 'rpc', 'php_memory', 'license_inventory'], 'success')
    expected_stages.update(php_operations='failure', language='failure')
    equal(stages['stages'], expected_stages, 'original CI stage failures preserved')
    for name in ['wordpress_bootstrapped', 'product_runtime_selected']:
        equal(stages[name], False, 'stage scope ' + name)
    stage_paths = ['.github/workflows/php-wasix-candidate.yml', HOST_PREFIX + '/llvm-toolchain.json',
                   'experiments/php-runtime-candidate-probe/artifact.json']
    verify_sources(stages, {name: sha(original(name)) for name in stage_paths})
    expected_inputs = declared_operation_inputs()
    verify_sources(operation, expected_inputs)
    verify_sources(language)
    fixed_operation = {
        'format_version': 1, 'platform': {'system': 'Linux', 'machine': 'x86_64'}, 'backend': 'llvm',
        'scope': 'fixed PHP weighted-Wasm-operation-budget diagnostic only',
        'outcome': 'execution_timeout', 'diagnostic_passed': False,
        'owned_host_reaped': False, 'owned_process_cleanup_completed': True,
        'php_wasm_operation_budget_verified_for_fixed_cases': False,
        'expected_execution_results': [True, False, True], 'trial_points': 100000000,
        'budget_reset_count': 0, 'start_execution_included': True,
        'production_operation_budget_verified': False, 'cpu_time_budget_verified': False,
        'php_opcode_budget_verified': False, 'hostcall_timeout_verified': False,
        'memory_limit_verified': False, 'product_runtime_selected': False,
        'wordpress_executed': False, 'fiber_compatibility_verified': False,
        'full_php_compatibility_verified': False, 'native_comparison_performed': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300, 'build_exit_code': 0,
        'native_artifact': 'target/php-wasix-host-probe/release/recastpress-php-wasix-host-probe',
        'native_artifact_sha256': 'cb490fbd708c163b19867bcdc93c8b1dcd13ec96caa0c28e41ac3351cd2e67a5',
        'diagnostics': 'owned child exceeded finite deadline; this is not proof of metering exhaustion',
        'stderr_excerpt': 'PHP_WASIX_PHASE=module_compile\n',
    }
    equal(set(operation), set(fixed_operation) | {'observed_at_utc', 'input_files'},
          'original timeout field set')
    for name, expected in fixed_operation.items():
        equal(operation.get(name), expected, 'original timeout ' + name)
    for name in ['native_result', 'observed_phases', 'owned_process_pid', 'exit_code', 'stderr_sha256']:
        require(name not in operation, 'missing observation was synthesized: ' + name)
    equal(language['outcome'], 'comparison_failed', 'separate language failure')
    equal(language['comparison_passed'], False, 'separate language result')
    equal(language['case_results']['fiber']['equal_to_native'], False, 'original official Fiber difference')
    equal(language['observation']['cases']['fiber'], {
        'error': {'class': 'FiberError', 'message': 'Cannot resume a fiber that is not suspended'},
        'status': 'error', 'warnings': [],
    }, 'separate official Fiber failure observation')
    module_sha = 'e64850fde91d98dff0bce2b95ed467591376586469b1f4d8bad5c8759c4edbe4'
    equal(language['native_result']['artifact'], {
        'expected_bytes': 39727549, 'expected_sha256': module_sha, 'observed_sha256': module_sha,
        'path': '.reference/php-wasix-8.5.12/php.wasm', 'verified': True,
    }, 'separate language result records the original official module')
    prior_wire = original(PRIOR)
    equal(sha(prior_wire), PRIOR_SHA256, 'prior imported observation raw bytes')
    prior = parse_json(prior_wire)
    equal(prior['outcome'], 'php_operation_budget_diagnostic_passed', 'prior recorded outcome only')
    equal(prior['native_result']['compile_elapsed_ms'], 221189, 'prior recorded LLVM compile duration')
    same = {}
    for name in RELATED:
        current, previous = original(name), original(name, PRIOR_SOURCE)
        equal(current, previous, 'unchanged relevant source bytes ' + name)
        equal(prior['input_files'][name], sha(previous), 'prior relevant source binding ' + name)
        same[name] = sha(current)
    return {
        'original_operation_declared_input_count': len(expected_inputs),
        'complete_original_operation_declared_input_set_verified': True,
        'original_saved_receipt_source_hashes_verified': True,
        'full_compile_dependency_graph_verified': False,
        'original_ci_verdict': 'failure', 'original_php_operations_stage': 'failure',
        'original_language_stage': 'failure', 'original_operation_outcome': 'execution_timeout',
        'original_outer_host_timeout_seconds_including_compile': 300,
        'normal_completion_owned_host_reaped_flag': False,
        'owned_process_cleanup_completed_flag': True,
        'missing_owned_pid_or_normal_return_reap_observation_inferred': False,
        'observed_phase_markers_from_stored_stderr_excerpt': ['module_compile'],
        'phase_evidence_kind': 'stored bounded stderr excerpt; full stderr bytes were not acquired',
        'phase_window': 'module_compile marker, then Module::new and inspect_compiled_budget, before normal_first marker',
        'module_new_vs_following_initializer_inspection_distinguished': False,
        'slow_compile_vs_compiler_stop_distinguished': False,
        'guest_phase_marker_observed': False, 'guest_exhaustion_observed': False,
        'guest_execution_or_exhaustion_verdict_inferred': False,
        'actual_budget_exhausted_result_verified': False,
        'operation_diagnostic_passed': False,
        'separate_language_fiber_failure': 'Cannot resume a fiber that is not suspended',
        'other_runtime_case_results_revalidated': False,
        'prior_observation': {
            'source_commit': PRIOR_SOURCE, 'run_id': '41320223203', 'raw_receipt_sha256': PRIOR_SHA256,
            'recorded_compile_elapsed_ms': 221189, 'related_source_bytes_identical': same,
            'full_prior_runtime_result_revalidated': False,
            'timing_scope': 'one prior CI observation; not a latency benchmark or proof that af4 merely compiled slowly',
        },
    }


def main():
    require(sys.argv[1:] == [], 'this fixed importer accepts no arguments')
    raw, data = read_archive()
    observations = verify(data)
    inputs = {path.relative_to(ROOT).as_posix(): sha(path.read_bytes())
              for path in [Path(__file__).resolve(), READER]}
    provenance = {
        'format_version': 1, 'kind': 'source_verified_import_of_fixed_operation_compile_window_timeout',
        'source_commit': SOURCE,
        'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
        'run_attempt': '1', 'workflow_job_id': 124906787928,
        'artifact_id': ARTIFACT, 'artifact_name': 'php-wasix-linux-llvm-evidence-41326284424-1',
        'artifact_zip_bytes': ZIP_BYTES, 'artifact_zip_sha256': ZIP_SHA256,
        'artifact_repository_id': 1432882501, 'exact_json_member_count': 16,
        'artifact_identity_source': 'fixed GitHub API metadata observed separately; importer makes no network calls',
        'github_api_metadata_requeried_by_importer': False,
        'original_git_source_hashes_verified': True,
        'original_receipt_verdicts_preserved': True, 'ci_verdict': 'failure',
        'php_executed_by_importer': False, 'native_or_wasm_binaries_acquired_by_importer': False,
        'source_binary_identity_reverified_by_importer': False, 'full_raw_stderr_reverified': False,
        'product_runtime_selected': False, 'guest_exhaustion_verified': False,
        'input_files': inputs, 'imported_files': {name: sha(raw[name]) for name in sorted(SAVED)},
        'archive_json_members': {name: {'bytes': len(raw[name]), 'sha256': sha(raw[name])} for name in sorted(NAMES)},
        'observations': observations,
    }
    require(DESTINATION.resolve() == DESTINATION and DESTINATION.is_relative_to(HERE / 'evidence'),
            'timeout evidence destination escaped experiment')
    DESTINATION.mkdir(parents=True, exist_ok=True)
    for name in sorted(SAVED):
        path = DESTINATION / name
        require(path.resolve() == path and not path.is_symlink(), 'linked raw evidence destination')
        if path.exists():
            equal(path.read_bytes(), raw[name], 'existing raw evidence must remain byte-identical')
        else:
            with path.open('xb') as output:
                output.write(raw[name])
    path = DESTINATION / 'provenance.json'
    require(path.resolve() == path and not path.is_symlink(), 'linked derived evidence destination')
    path.write_text(json.dumps(provenance, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('Imported original af4 LLVM operation compile-window timeout; no guest exhaustion verdict.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as failure:
        print('Operation timeout evidence import failed: ' + str(failure), file=sys.stderr)
        sys.exit(1)
