"""Import pinned managed-access JSON evidence against its original Git tree.

Artifact identities, exact members and stage/verdict pins remain unset until
GitHub supplies the completed run. Unset pins fail closed. Raw JSON is immutable;
derived verification never rewrites PHP observations or native expectations.
No network, PHP, Wasm, compiler, archive extractor or native oracle is executed.
"""
import argparse
import ast
import copy
import importlib.util
import json
from pathlib import Path
import re
import sys
from types import ModuleType
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
PREFIX = 'experiments/php-wasix-managed-access-probe/'
CANONICAL = 'experiments/php-wasix-fiber-cgi-probe/'
RUNTIME = 'experiments/php-wasix-cgi-runtime-probe/'
CGI = 'experiments/php-wasix-cgi-build/'
FIBER = 'experiments/php-wasix-fiber-probe/'
WORK = 'target/php-wasix-managed-access-probe/'
SOURCE = 'edc6b645aead72b00eb482994a58e69957879118'
RUN = '41337652464'
VARIANT = 'nts-fresh-wasi-zend-fibers-managed-access-v1'
# Pin only observed API metadata and exact ZIP paths, never an expected result.
SPECS = {backend: {'artifact': None, 'bytes': None, 'sha256': None,
    'members': None, 'stages': None, 'fiber_verdict': None, 'runtime_verdict': None}
    for backend in ['cranelift', 'llvm']}
SPECS['cranelift'] = {
    'artifact': 13097255095, 'bytes': 77047,
    'sha256': '8606468a7a0159e70ebd5a29e14ac8830fe5793019873ee65198c3af35e6432f',
    'members': {
        'build.json': 'php-wasix-managed-access-probe/evidence/build.json',
        'ci-stages.json': 'php-wasix-managed-access-probe/evidence/ci-stages.json',
        'linux-cranelift-managed-fiber-contract.json': 'php-wasix-managed-access-probe/evidence/linux-cranelift-managed-fiber-contract.json',
        'linux-cranelift-managed-runtime-contract.json': 'php-wasix-managed-access-probe/evidence/linux-cranelift-managed-runtime-contract.json',
        'native-cgi-fiber-contract.json': 'php-wasix-fiber-cgi-probe/evidence/native-cgi-fiber-contract.json',
        'native-cgi-runtime-contract.json': 'php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json',
        'wordpress-source.json': 'php-wasix-cgi-runtime-probe/evidence/wordpress-source.json',
        'toolchain.json': 'php-wasix-cgi-build/evidence/toolchain.json',
        'linux-cranelift-unit-contract.json': 'php-wasix-host-probe/evidence/linux-cranelift-unit-contract.json',
        'windows-cranelift-unit-contract.json': 'php-wasix-host-probe/evidence/windows-cranelift-unit-contract.json',
    },
    'stages': {'php_source': 'success', 'deps_source': 'success', 'parser_source': 'success',
        'dependencies': 'success', 'definitions': 'success', 'tools': 'success', 'build': 'success',
        'rust': 'success', 'host_llvm': 'skipped', 'units': 'success', 'wordpress_source': 'success',
        'fiber_contract': 'success', 'runtime_contract': 'success'},
    'fiber_verdict': 'success', 'runtime_verdict': 'success',
}
SPECS['llvm'] = {
    'artifact': 13096917311, 'bytes': 77484,
    'sha256': '72df469d83f33e98c30ccbbae2e7ea5307a30f060f38f4c34176156008536de7',
    'members': {
        'build.json': 'php-wasix-managed-access-probe/evidence/build.json',
        'ci-stages.json': 'php-wasix-managed-access-probe/evidence/ci-stages.json',
        'linux-llvm-managed-fiber-contract.json': 'php-wasix-managed-access-probe/evidence/linux-llvm-managed-fiber-contract.json',
        'linux-llvm-managed-runtime-contract.json': 'php-wasix-managed-access-probe/evidence/linux-llvm-managed-runtime-contract.json',
        'native-cgi-fiber-contract.json': 'php-wasix-fiber-cgi-probe/evidence/native-cgi-fiber-contract.json',
        'native-cgi-runtime-contract.json': 'php-wasix-cgi-runtime-probe/evidence/native-cgi-runtime-contract.json',
        'wordpress-source.json': 'php-wasix-cgi-runtime-probe/evidence/wordpress-source.json',
        'toolchain.json': 'php-wasix-cgi-build/evidence/toolchain.json',
        'linux-llvm-unit-contract.json': 'php-wasix-host-probe/evidence/linux-llvm-unit-contract.json',
        'llvm-toolchain-verification.json': 'php-wasix-host-probe/evidence/llvm-toolchain-verification.json',
        'windows-cranelift-unit-contract.json': 'php-wasix-host-probe/evidence/windows-cranelift-unit-contract.json',
    },
    'stages': {'php_source': 'success', 'deps_source': 'success', 'parser_source': 'success',
        'dependencies': 'success', 'definitions': 'success', 'tools': 'success', 'build': 'success',
        'rust': 'success', 'host_llvm': 'success', 'units': 'success', 'wordpress_source': 'success',
        'fiber_contract': 'success', 'runtime_contract': 'success'},
    'fiber_verdict': 'success', 'runtime_verdict': 'success',
}


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


prior = load('rp_managed_ci_previous_runtime_import', ROOT / RUNTIME / 'import_ci_evidence.py')
base = prior.base
# The module graph is private to this importer. Switch before any cached read.
base.SOURCE = base.reader.COMMIT = prior.SOURCE = SOURCE
base.RUN = prior.RUN = RUN
base.original.cache_clear()
checker = load('rp_managed_ci_historical_checker', HERE / 'managed_check.py')
original, equal, require = base.original, base.equal, base.require
sha, integer, digest, parse_json = base.sha, base.integer, base.digest, base.parse_json
original_prepare_inputs = base.original_prepare_inputs


def hashes(names):
    return {name: sha(original(name)) for name in sorted(set(names))}


def original_build_inputs():
    inputs = base.original_prepare_inputs()
    names = [PREFIX + name for name in ['build.py', 'patch_source.py', 'manifest.json',
        'source_profile.py', 'patch-plan.json', 'patches/0001-wasi-managed-access.patch',
        'fixtures/recastpress_fs_access.h']]
    names += [CANONICAL + name for name in ['build.py', 'manifest.json', 'patch_source.py', 'fixtures/fiber_cgi_contract.php']]
    names += [FIBER + name for name in ['build.py', 'patch_source.py', 'manifest.json',
        'patches/0001-nts-fresh-wasi-zend-fibers.patch', 'fixtures/zend_wasix_fiber.S', 'fixtures/fiber_contract.php']]
    names += [CGI + 'patches/0001-wasi-disable-cgi-prefork.patch']
    inputs.update(hashes(names))
    return dict(sorted(inputs.items()))


def original_stage_inputs():
    names = {'.github/workflows/php-wasix-managed-access.yml'}
    for prefix in [PREFIX, RUNTIME]:
        for name in base.original_names(prefix):
            suffix = name[len(prefix):]
            if ('/' not in suffix and name.endswith(('.py', '.json'))) or (
                    suffix.startswith('fixtures/') and '/' not in suffix[len('fixtures/'):]) or (
                    suffix.startswith('patches/') and name.endswith('.patch') and '/' not in suffix[len('patches/'):]):
                names.add(name)
    names.update([CANONICAL + 'evidence/native-cgi-fiber-contract.json',
        RUNTIME + 'evidence/native-cgi-runtime-contract.json',
        'experiments/php-wasix-host-probe/llvm-toolchain.json',
        'experiments/php-wasix-host-probe/prepare_host_llvm.py',
        'experiments/php-wasix-host-probe/unit_check.py'])
    return hashes(names)


def original_compile_inputs():
    """Reconstruct original globs and ROOT literals without executing old code."""
    prefix = 'experiments/php-wasix-host-probe/'
    tree = ast.parse(original(prefix + 'check.py'))
    functions = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'compiled_inputs']
    require(len(functions) == 1, 'original compile-input function missing')
    function = functions[0]
    names = {prefix + 'Cargo.toml', prefix + 'Cargo.lock'}
    names.update(name for name in base.original_names(prefix + 'src/') if name.endswith('.rs') and '/' not in name[len(prefix + 'src/'):])
    names.update(name for name in base.original_names(prefix + 'fixtures/') if name.endswith('.php') and '/' not in name[len(prefix + 'fixtures/'):])
    for node in ast.walk(function):
        if (isinstance(node, ast.BinOp) and isinstance(node.op, ast.Div)
                and isinstance(node.left, ast.Name) and node.left.id == 'ROOT'
                and isinstance(node.right, ast.Constant) and type(node.right.value) is str):
            names.add(node.right.value)
    equal(len(names), 48, 'original complete compile input count')
    return names


def original_comparison_inputs(oracles, build_inputs, prepared):
    names = original_compile_inputs()
    names.update(PREFIX + name for name in ['managed_check.py', 'build.py', 'source_profile.py', 'patch_source.py'])
    names.update('experiments/php-wasix-host-probe/' + name for name in ['check.py', 'language_check.py'])
    names.update(CANONICAL + name for name in ['build.py', 'patch_source.py', 'fiber_cgi_check.py', 'native_contract.py'])
    names.update(RUNTIME + name for name in ['runtime_check.py', 'prepare.py', 'native_contract.py'])
    names.update([FIBER + 'native_contract.py', 'experiments/php-runtime-candidate-probe/native_contract.py', 'scripts/owned_probe.py'])
    names.update(build_inputs)
    names.update(prepared['input_files'])
    for oracle in oracles: names.update(oracle['input_files'])
    return hashes(names)


def expected_members(backend):
    result = {name: 'php-wasix-managed-access-probe/evidence/' + name for name in
        ['build.json', 'ci-stages.json', 'linux-' + backend + '-managed-fiber-contract.json',
         'linux-' + backend + '-managed-runtime-contract.json']}
    for name, prefix in [('native-cgi-fiber-contract.json', CANONICAL),
            ('native-cgi-runtime-contract.json', RUNTIME), ('wordpress-source.json', RUNTIME),
            ('toolchain.json', CGI), ('windows-cranelift-unit-contract.json', 'experiments/php-wasix-host-probe/'),
            ('linux-' + backend + '-unit-contract.json', 'experiments/php-wasix-host-probe/')]:
        result[name] = prefix.removeprefix('experiments/') + 'evidence/' + name
    if backend == 'llvm':
        result['llvm-toolchain-verification.json'] = 'php-wasix-host-probe/evidence/llvm-toolchain-verification.json'
    return result


def pinned_spec(backend):
    require(backend in SPECS, 'unknown fixed backend')
    spec = SPECS[backend]
    integer(spec['artifact'], 1, 2**63); integer(spec['bytes'], 1, 8 * 1024 * 1024); digest(spec['sha256'])
    require(type(spec['members']) is dict and spec['members'], 'exact ZIP member pin has not arrived')
    require(set(spec['members']) <= set(expected_members(backend)), 'unexpected artifact names')
    for name, path in spec['members'].items(): equal(path, expected_members(backend)[name], 'exact fixed member location')
    require(type(spec['stages']) is dict and spec['stages'], 'final stage metadata has not arrived')
    equal(set(spec['stages']), {'php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions',
        'tools', 'build', 'rust', 'host_llvm', 'units', 'wordpress_source', 'fiber_contract', 'runtime_contract'},
        'fixed stage inventory')
    require(all(value in ['success', 'failure', 'skipped', 'not_run', 'cancelled'] for value in spec['stages'].values()),
        'unknown stage verdict')
    for kind in ['fiber', 'runtime']:
        require(spec[kind + '_verdict'] in ['success', 'failure', 'skipped', 'not_run'], 'final comparison verdict not pinned')
        equal(spec['stages'][kind + '_contract'], spec[kind + '_verdict'], 'pinned mode/stage agreement')
    return spec


def read_archive(backend):
    spec = pinned_spec(backend)
    path = ROOT / ('target/php-wasix-ci-evidence/run-' + RUN + '-' + backend + '-1.zip')
    require(path.resolve(strict=True) == path and path.is_file(), 'fixed artifact missing/linked')
    equal(path.stat().st_size, spec['bytes'], 'fixed ZIP length')
    wire = path.read_bytes()
    equal((len(wire), sha(wire)), (spec['bytes'], spec['sha256']), 'fixed ZIP identity')
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        equal(len(entries), len(spec['members']), 'exact ZIP member count')
        equal({row.filename for row in entries}, set(spec['members'].values()), 'exact ZIP member paths')
        require(len(set(spec['members'].values())) == len(spec['members']), 'duplicate pinned member')
        for row in entries:
            require(0 < row.file_size <= 512 * 1024 and not row.is_dir()
                and (row.external_attr >> 16) & 0o170000 != 0o120000 and row.flag_bits & 1 == 0,
                'ZIP JSON member type/size/encryption')
        raw = {name: archive.read(member) for name, member in spec['members'].items()}
    return raw, {name: parse_json(value) for name, value in raw.items()}


def verify_units(value, backend):
    for key, expected in [('outcome', 'fixed_p0_unit_tests_passed'), ('tests_passed', True),
            ('owned_test_reaped', True), ('exit_code', 0), ('backend', backend), ('php_executed', False),
            ('php_cpu_budget_verified', False), ('product_runtime_selected', False),
            ('outer_build_timeout_seconds', 1200), ('outer_list_timeout_seconds', 10),
            ('outer_test_timeout_seconds', 30), ('build_exit_code', 0), ('list_exit_code', 0)]:
        equal(value[key], expected, 'original Linux37 unit ' + key)
    equal(value['platform']['system'], 'Linux', 'original Linux unit platform')
    names = value['listed_tests']
    equal((len(names), len(set(names))), (37, 37), 'original unique Linux37 inventory')
    groups = {'cgi_contract::cgi_response::tests::': 9, 'language_contract::tests::': 2,
        'context_contract::tests::': 1, 'fiber_adapter::tests::': 1, 'memory_limits::': 3,
        'rpc_fs::tests::': 9, 'runtime_fixture_fs::tests::': 4, 'managed_access_fs::tests::': 6, 'metering_limits::': 2}
    for prefix, count in groups.items(): equal(sum(name.startswith(prefix) for name in names), count, 'unit group ' + prefix)
    for key in ['owned_list_pid', 'owned_test_pid']: integer(value[key], 1)
    for key in ['test_artifact_sha256', 'stdout_sha256', 'stderr_sha256']: digest(value[key])
    require('test result: ok. 37 passed; 0 failed; 0 ignored;' in value['stdout_excerpt'], 'all Linux37 units executed')
    expected_inputs = hashes(original_compile_inputs() | {'experiments/php-wasix-host-probe/unit_check.py',
        'experiments/php-wasix-host-probe/check.py', 'experiments/php-wasix-host-probe/language_check.py', 'scripts/owned_probe.py'})
    equal(value['input_files'], expected_inputs, 'complete original Linux unit inputs')
    summaries = base.unit_checker.validate_metering_rows(value['wat_metering_observations'], backend, 'Linux')
    equal(value['wat_metering_summaries'], summaries, 'metering observations/summaries')


def verify_build(data, raw):
    build, tools = data['build.json'], data['toolchain.json']
    definition = parse_json(original(PREFIX + 'manifest.json'))
    tool_definition = parse_json(original(CGI + 'manifest.json'))
    equal(build['input_files'], original_build_inputs(), 'complete original combined builder inputs')
    equal(tools['input_files'], original_prepare_inputs(), 'complete original prepared-tool inputs')
    for key, expected in [('schema', 'recastpress.php-wasix-managed-access-build.v0.5'), ('variant', VARIANT),
            ('outcome', 'managed_access_candidate_built_not_executed'), ('phase', 'build_complete'),
            ('tracked_sources_clean_before_build', True), ('binaryen_validation_enabled', True)]:
        equal(build[key], expected, 'combined build ' + key)
    for key in ['guest_executed', 'php_guest_executed', 'patched_php_executed', 'product_runtime_selected',
            'native_runtime_fallback_used', 'redistributed_binaries', 'php_wasm_fiber_cgi_compatibility_verified',
            'postopt_module_inspected', 'sysroot_source_build_attestation_verified']:
        equal(build[key], False, 'unclaimed builder gate ' + key)
    for key, expected in [('outcome', 'verified_host_build_tools_prepared'), ('guest_executed', False),
            ('product_runtime_selected', False), ('redistributed_binaries', False)]:
        equal(tools[key], expected, 'prepared tools ' + key)
    equal(build['toolchain']['receipt_sha256'], sha(raw['toolchain.json']), 'same-job prepared tools receipt')
    equal(build['toolchain']['manifest_sha256'], sha(original(CGI + 'manifest.json')), 'same fixed tools manifest')
    equal(len(tools['assets']), 4, 'four fixed tool archives')
    equal({row['name'] for row in tools['assets']}, {row['name'] for row in tool_definition['assets']}, 'fixed asset set')
    equal(build['toolchain']['assets'], [{key: asset[key] for key in ['name', 'asset_id', 'bytes', 'sha256']}
        for asset in tool_definition['assets']], 'build/tool archive identities')
    for asset in tool_definition['assets']:
        row = next(row for row in tools['assets'] if row['name'] == asset['name'])
        for key in ['name', 'repository', 'tag', 'asset_id', 'url', 'bytes', 'sha256']:
            equal(row[key], asset[key], 'fixed tool pin ' + key)
        equal((row['verified'], row['observed_bytes'], row['observed_sha256']),
            (True, asset['bytes'], asset['sha256']), 'observed tool archive identity')
        equal(set(row['required_files']), set(asset['required_files']), 'extracted required-file names')
        for entry in row['required_files'].values():
            integer(entry['bytes'], 1); digest(entry['sha256'])
    equal(set(build['sources']), set(tool_definition['sources']), 'source checkout set')
    for name, pin in tool_definition['sources'].items():
        row = build['sources'][name]
        equal((row['repository'], row['commit'], row['directory']),
            (pin['repository'], pin['commit'], WORK + pin['directory']), 'separate source checkout pin')
    equal(build['sources']['deps']['rebuilt_here'], False, 'prebuilt dependency scope')
    equal(build['sources']['deps']['all_archives_linked_verified'], False, 'no complete archive usage claim')
    inventory = build['sources']['deps']['prebuilt_archive_files']
    require(type(inventory) is dict and len(inventory) == 28, '28 prebuilt dependencies')
    for name, entry in inventory.items():
        require(name.endswith('.a') and not name.startswith('/') and '..' not in name.split('/'), 'archive inventory path')
        equal(set(entry), {'bytes', 'sha256'}, 'archive identity fields')
        integer(entry['bytes'], 8, 512 * 1024 * 1024); digest(entry['sha256'])
    equal(len(build['source_patches']), 3, 'ordered canonical two plus managed-access patch')
    for patch, plan in zip(build['source_patches'], definition['source_patches']):
        for key in ['kind', 'path', 'bytes', 'sha256']:
            equal(patch[key], plan[key], 'canonical combined patch ' + key)
        equal(sha(original(plan['path'])), plan['sha256'], 'fixed original patch bytes')
        equal(patch['source_commit'], definition['upstream']['commit'], 'patched PHP commit')
        gates = ['applied', 'git_apply_check_passed', 'git_apply_passed']
        if patch['kind'] == 'managed_access':
            gates += ['fixed_own_targets_verified_before_patch', 'prior_canonical_patch_set_verified',
                'installed_header_matches_fixed_fixture', 'canonical_patch_targets_disjoint']
            equal(patch['tracked_sources_clean_before_patch'], False, 'managed patch follows canonical patches')
            before_names = sorted(row['path'] for part in definition['source_patches'][:2] for row in part['files'] if row['before'] is not None)
            equal(patch['tracked_changes_before_patch'], before_names, 'exact canonical preceding tracked changes')
        else: gates += ['combined_tracked_source_clean_before_first_patch']
        for key in gates: equal(patch[key], True, 'source patch gate ' + key)
        equal(patch['index_updated'], False, 'patch does not mutate Git index')
        equal(patch['before'], {row['path']: row['before'] for row in plan['files']}, 'whole-file source bases')
        equal(set(patch['after']), {row['path'] for row in plan['files']}, 'whole-file patched names')
        for row in plan['files']:
            after = patch['after'][row['path']]
            equal(set(after), {'bytes', 'sha256', 'git_blob_sha1'}, 'whole-file patched identity fields')
            integer(after['bytes'], 1, 1024 * 1024); digest(after['sha256'])
            require(type(after['git_blob_sha1']) is str and re.fullmatch('[0-9a-f]{40}', after['git_blob_sha1']), 'patched Git blob identity')
            equal({key: after[key] for key in row['after']}, row['after'], 'pinned whole-file patched identity')
    policy = build['configuration_policy']
    for key, expected in [('backend_enabled', True), ('zts_enabled', False), ('ucontext_enabled', False),
            ('assembly_rule_observed', True), ('compiler_post_flags', '-fno-lto'), ('source_script_flto_preserved', True)]:
        equal(policy[key], expected, 'configured Fiber backend ' + key)
    config = build['build_config']
    for name in [PREFIX + 'build.py', CANONICAL + 'build.py']:
        equal((ROOT / name).read_bytes(), original(name), 'pure build policy helper matches original source')
    equal(config, checker.builder.driver.build_config(), 'entire original fixed managed build policy')
    equal((config['variant'], config['extra_configure_flags'], config['make_target'], config['make_jobs']),
        (VARIANT, ['--enable-cgi', '--enable-rp-wasix-fibers', '--disable-zts'], 'cgi', 2), 'fixed combined configure/make')
    equal(config['raw_output'], WORK + 'php-fiber-cgi.raw.wasm', 'raw output path')
    equal(config['wasm_output'], WORK + 'php-fiber-cgi.wasm', 'final output path')
    equal(config['linked_output'], WORK + 'source/php/sapi/cgi/php-cgi', 'linked output path')
    equal(config['tool_environment_additions'], {'WASIXCC_COMPILER_POST_FLAGS': '-fno-lto',
        'WASIXCC_RUN_WASM_OPT': 'no', 'WASIXCC_PIC': 'no', 'WASIXCC_GENERATE_SHELL_SCRIPT': 'no'}, 'fixed compiler environment')
    exports = ['rp_zend_wasix_switch_raw', 'rp_zend_wasix_entry', 'rp_zend_wasix_dispatch', 'rp_zend_wasix_bootstrap_sp', '__stack_pointer']
    equal(config['required_exports'], exports, 'exact postopt exports')
    flags = ':'.join('--export=' + name for name in exports)
    equal(config['make_only_environment_additions'], {'WASIXCC_LINKER_FLAGS': flags}, 'make-only exports')
    equal(config['link_export_scope'], 'make_cgi only; configure conftests do not define the PHP adapter symbols', 'export scoping')
    optimizer = ['--emit-exnref', '--all-features', '--strip-debug', '--asyncify',
        '--pass-arg=asyncify-imports@wasix_32v1.proc_snapshot', '--pass-arg=asyncify-ignore-indirect', '--pass-arg=max-func-params@32']
    equal(config['optimizer_flags'], optimizer, 'validation enabled without O3')
    recipe = [
        {'name': 'configure', 'command': ['bash', 'wasix-configure-eh-64.sh'], 'working_directory': WORK + 'source/php', 'timeout_seconds': 600},
        {'name': 'make_cgi', 'command': ['make', '-j2', 'PHP=<host-stub-php>', 'cgi'], 'working_directory': WORK + 'source/php', 'timeout_seconds': 1200},
        {'name': 'optimize_cgi', 'command': ['target/php-wasix-cgi-build/tools/binaryen/bin/wasm-opt', *optimizer,
            WORK + 'php-fiber-cgi.raw.wasm', '-o', WORK + 'php-fiber-cgi.wasm'], 'working_directory': '<workspace>', 'timeout_seconds': 300}]
    equal(config['command_recipe'], recipe, 'fixed three-command recipe')
    equal(len(build['build_steps']), 3, 'three completed build commands')
    for step, expected in zip(build['build_steps'], recipe):
        equal({key: step[key] for key in expected}, expected, 'recorded command recipe')
        for key in ['completed', 'owned_leader_reaped', 'live_output_quota_enforced', 'output_capture_complete']:
            equal(step[key], True, 'owned build completion ' + key)
        equal(step['exit_code'], 0, 'successful build command')
        equal(step['process_group_failure_cleanup'], False, 'normal command cleanup')
        equal(step['output_quota_bytes'], 16 * 1024 * 1024, 'live command quota')
        equal(step['linker_flags_environment'], {'present': step['name'] == 'make_cgi',
            'value_sha256': sha(flags.encode()) if step['name'] == 'make_cgi' else None}, 'PHP exports limited to CGI make')
        integer(step['elapsed_ms'])
        for key in ['stdout_bytes', 'stderr_bytes']: integer(step[key], 0, 16 * 1024 * 1024)
        for key in ['stdout_sha256', 'stderr_sha256']: digest(step[key])
    for key, path in [('php_wasm', config['wasm_output']), ('raw_wasm', config['raw_output']), ('linked_php_output', config['linked_output'])]:
        row = build[key]
        equal(row['path'], path, 'generated Wasm path')
        integer(row['bytes'], 8, 128 * 1024 * 1024); digest(row['sha256'])
        equal(row['wasm_header_verified'], True, 'generated Wasm header')
        equal(row['executed'], False, 'builder did not run guest')
    equal(build['raw_wasm']['sha256'], build['linked_php_output']['sha256'], 'raw copy identity')
    equal(build['raw_wasm']['bytes'], build['linked_php_output']['bytes'], 'raw copy size')
    require({'config.log', 'config.status', 'configure', 'Makefile', 'main/php_config.h', 'Zend/zend_config.h'}
        <= set(build['generated_configuration_files']), 'generated configuration record set')
    for row in build['generated_configuration_files'].values():
        integer(row['bytes'], 1, 16 * 1024 * 1024); digest(row['sha256'])
    require(type(build['stub_headers_after']) is dict and len(build['stub_headers_after']) > 100, 'generated stub header inventory')
    for value in build['stub_headers_after'].values(): digest(value)
    for patch in build['source_patches']:
        for name, after in patch['after'].items():
            if patch['before'][name] is not None:
                equal(build['tracked_changes_after_build'][name], {key: after[key] for key in ['bytes', 'sha256']},
                    'patched source unchanged after build')
    header = original(PREFIX + 'fixtures/recastpress_fs_access.h')
    installed = build['source_patches'][2]['after']['main/recastpress_fs_access.h']
    equal((installed['bytes'], installed['sha256']), (len(header), sha(header)), 'installed header equals original canonical bytes')
    equal(definition['parent_profile'], {'path': CANONICAL + 'manifest.json', 'sha256': sha(original(CANONICAL + 'manifest.json'))}, 'fixed parent manifest')
    equal(definition['managed_access']['patch_plan_sha256'], sha(original(PREFIX + 'patch-plan.json')), 'managed patch plan binding')
    return original_build_inputs()


def failed_observations(probe, kind, backend, oracle, provenance, prepared):
    """Describe retained observations without upgrading any failed comparison."""
    flag = kind + '_contract_passed'
    equal(probe[flag], False, 'failed comparison remains failed')
    equal(probe['native_comparison_passed'], False, 'failed native comparison remains failed')
    equal(probe['managed_access_verified_for_fixed_cases'], False, 'failed access comparison remains unverified')
    require(probe['outcome'] != 'fixed_managed_' + kind + '_contract_passed', 'failed outcome became successful')
    summary = {'comparison_passed': False, 'outcome': probe['outcome'], 'host_result_available': False}
    value = probe.get('native_result')
    if value is None: return summary
    summary['host_result_available'] = True
    for key, expected in [('schema', 'recastpress.php-wasix-managed-' + kind + '-host.v0.5'),
            ('variant', VARIANT), ('engine', 'wasmer-7.2.0/' + backend),
            ('process_pid', probe['owned_process_pid'])]:
        equal(value[key], expected, 'failed managed host identity ' + key)
    require(type(value['ok']) is bool, 'failed record actual host verdict type')
    summary.update(host_ok=value['ok'], host_phase=value['phase'], host_error=value.get('error'))
    for key in ['full_php_compatibility_verified', 'product_runtime_selected', 'native_runtime_fallback_used',
            'all_resources_immediately_reclaimed', 'memory_limit_verified', 'cpu_limit_verified',
            'sysroot_source_build_attestation_verified', 'physical_stack_free_verified',
            'physical_stack_reuse_verified', 'stack_guards_verified', 'persistent_requests_supported', 'zts_supported', 'real_db']:
        equal(value[key], False, 'failed record retains narrow scope ' + key)
    summary['host_candidate_binding_verified'] = False
    if provenance is not None and value.get('artifact', {}).get('verified') is True:
        for key, expected in [('path', WORK + 'php-fiber-cgi.wasm'), ('verified', True),
                ('observed_sha256', provenance['module']['sha256']), ('observed_bytes', provenance['module']['bytes'])]:
            equal(value['artifact'][key], expected, 'failed host candidate identity ' + key)
        equal(value['build_receipt']['observed_sha256'], provenance['build_receipt_sha256'], 'failed host receipt identity')
        summary['host_candidate_binding_verified'] = True
    if value.get('source_files') and all(row.get('verified') is True for row in value['source_files']):
        equal(value['source_files'], checker.sources([ROOT / name for name in checker.SOURCES], original), 'failed compiled source bindings')
    cases = value.get('cases', [])
    require(type(cases) is list and len(cases) <= 3, 'bounded failed request observations')
    native = checker.fiber.native if kind == 'fiber' else checker.runtime.native
    rows = []
    for case in cases:
        row = {'name': case.get('name'), 'ok': case.get('ok'), 'expectation_met': case.get('expectation_met'),
            'native_semantics_match': case.get('native_semantics_match'),
            'error': case.get('host_execution_error') or case.get('expectation_error') or case.get('guest_json_error'),
            'complete_cgi_response_reparsed': False, 'native_comparison_passed': False}
        for key in ['scratch_initial', 'scratch_final', 'managed_access']:
            if key in case: row[key] = copy.deepcopy(case[key])
        if 'managed_access' in case:
            try: checker.validate_access(case['managed_access'])
            except (ValueError, TypeError, KeyError): row['managed_counter_shape_passed'] = False
            else: row['managed_counter_shape_passed'] = True
            row['managed_counter_scope'] = 'underlying Rust VirtualFile events, not guest fd/fopen counts'
        if 'stdout_hex' in case:
            require(type(case['stdout_hex']) is str and len(case['stdout_hex']) <= 131072, 'failed stdout hex quota')
            wire = bytes.fromhex(case['stdout_hex'])
            equal(case['stdout_bytes'], len(wire), 'failed raw stdout length')
            equal(case['stdout_sha256'], sha(wire), 'failed raw stdout SHA')
            row['stdout'] = {'bytes': len(wire), 'sha256': sha(wire), 'eof': case.get('stdout_eof'),
                'overflow': case.get('stdout_overflow')}
            try: response, body, _ = native.parse_completed_cgi(wire)
            except (ValueError, TypeError, KeyError) as error: row['cgi_parser_error'] = str(error)[:2048]
            else:
                row['complete_cgi_response_reparsed'] = True
                if case.get('response') is not None: equal(case['response'], response, 'failed raw/decoded CGI response')
                row['body'] = {'bytes': len(body), 'sha256': sha(body)}
                try: observation = parse_json(body)
                except (ValueError, TypeError, UnicodeError) as error: row['json_parser_error'] = str(error)[:2048]
                else:
                    if case.get('guest_json_decoded'): equal(case['guest_json'], observation, 'failed raw/decoded PHP JSON')
                    differences = prior.native_differences(observation, oracle['observation'])
                    row['php_json_identity'] = prior.json_identity(observation)
                    row['native_differences'] = differences[:128]
                    row['native_differences_truncated'] = len(differences) > 128
        rows.append(row)
    summary['observed_requests'] = rows
    return summary


def verify_comparison(probe, kind, backend, verdict, oracle, inputs, provenance, prepared):
    fiber_mode = kind == 'fiber'
    flag = kind + '_contract_passed'
    for key, expected in [('schema', 'recastpress.php-wasix-managed-' + kind + '-comparison.v0.5'),
            ('backend', backend), ('variant', VARIANT), ('outer_build_timeout_seconds', 1200),
            ('outer_execution_timeout_seconds', 600)]: equal(probe[key], expected, 'managed comparison ' + key)
    equal(probe['platform']['system'], 'Linux', 'managed comparison platform')
    for key in ['product_runtime_selected', 'product_abi_selected', 'full_php_compatibility_verified',
            'native_php_executed_by_this_helper', 'native_runtime_fallback_used', 'all_resources_immediately_reclaimed',
            'memory_limit_verified', 'cpu_limit_verified', 'stack_free_or_reuse_verified']:
        equal(probe[key], False, 'comparison unclaimed gate ' + key)
    if 'input_files' in probe: equal(probe['input_files'], inputs, 'complete original managed comparison input set')
    if 'generated_provenance' in probe:
        require(provenance is not None, 'comparison provenance without verified build')
        equal(probe['generated_provenance'], provenance, 'same three-patch candidate provenance')
    if verdict == 'success':
        equal(probe['input_files'], inputs, 'successful complete source set')
        for key, expected in [('outcome', 'fixed_managed_' + kind + '_contract_passed'), (flag, True),
                ('native_comparison_passed', True), ('managed_access_verified_for_fixed_cases', not fiber_mode),
                ('owned_host_reaped', True), ('exit_code', 0), ('build_exit_code', 0)]:
            equal(probe[key], expected, 'strict successful managed comparison ' + key)
        equal(probe['generated_provenance'], provenance, 'successful build provenance required')
        oracle_name = (CANONICAL if fiber_mode else RUNTIME) + 'evidence/native-cgi-' + ('fiber' if fiber_mode else 'runtime') + '-contract.json'
        equal(probe['native_reference'], {'path': oracle_name, 'sha256': sha(original(oracle_name)),
            'native_executed_by_this_helper': False, 'expected_case_count': 6}, 'unchanged native reference identity')
        if not fiber_mode: equal(probe['wordpress_preparation'], prepared, 'same WP3 preparation')
        integer(probe['owned_process_pid'], 1); digest(probe['native_artifact_sha256'])
        checker.validate_result(probe['native_result'], probe['owned_process_pid'], backend, provenance,
            fiber_mode=fiber_mode, prepared=prepared, read_source=original, build_inputs=original_build_inputs())
        summary = {'comparison_passed': True, 'fixed_native_cases': 6, 'fresh_requests': 3,
            'module_compile_calls': 1, 'permission_query_execution_verified': not fiber_mode,
            'compile_elapsed_ms': probe['native_result']['compile_elapsed_ms'],
            'counter_scope': 'underlying Rust VirtualFile events, not guest fd/fopen counts'}
        if not fiber_mode:
            summary['php_metadata_matches_actual_readonly_denial'] = probe['native_result']['php_metadata_matches_actual_readonly_denial']
            summary['observed_requests'] = [{'name': case['name'],
                'managed_access': copy.deepcopy(case['managed_access']),
                'scratch_final': copy.deepcopy(case['scratch_final'])} for case in probe['native_result']['cases']]
        return summary
    equal(verdict, 'failure', 'only explicitly failed executed comparisons may enter failure summary')
    return failed_observations(probe, kind, backend, oracle, provenance, prepared)


def verify(raw, data, backend):
    spec = pinned_spec(backend)
    for value in data.values():
        if 'input_files' in value: base.verify_sources(value)
    stages = data['ci-stages.json']
    equal((stages['source_commit'], stages['run_id'], stages['run_attempt'], stages['backend']),
        (SOURCE, RUN, '1', backend), 'fixed managed CI identity')
    equal(stages['stages'], spec['stages'], 'observed pinned CI stage outcomes')
    equal(stages['input_files'], original_stage_inputs(), 'complete original workflow source set')
    equal(stages['product_runtime_selected'], False, 'no product adoption')
    equal(stages['redistributed_binaries'], False, 'JSON-only bundle')
    for name, prefix in [('native-cgi-fiber-contract.json', CANONICAL), ('native-cgi-runtime-contract.json', RUNTIME),
            ('windows-cranelift-unit-contract.json', 'experiments/php-wasix-host-probe/')]:
        equal(raw[name], original(prefix + 'evidence/' + name), 'original checked-out reference unchanged')
    oracles = [checker.native_reference(True, original), checker.native_reference(False, original)]
    for name, oracle in zip(['native-cgi-fiber-contract.json', 'native-cgi-runtime-contract.json'], oracles):
        equal(data[name], oracle, 'pure native source/package gate unchanged')
    prepared = None
    if stages['stages']['wordpress_source'] == 'success':
        prepared = prior.verify_preparation(data['wordpress-source.json'], raw['wordpress-source.json'])
    if stages['stages']['units'] == 'success': verify_units(data['linux-' + backend + '-unit-contract.json'], backend)
    provenance = None
    if stages['stages']['build'] == 'success':
        build_inputs = verify_build(data, raw)
        build = data['build.json']
        provenance = {'build_receipt_sha256': sha(raw['build.json']), 'module': build['php_wasm'],
            'builder_provenance': {'build_receipt_sha256': sha(raw['build.json']),
                'variant': VARIANT, 'toolchain_receipt_sha256': sha(raw['toolchain.json']),
                'manifest_sha256': sha(original(PREFIX + 'manifest.json')),
                'patch_sha256': sha(original(FIBER + 'patches/0001-nts-fresh-wasi-zend-fibers.patch')),
                'cgi_patch_sha256': sha(original(CGI + 'patches/0001-wasi-disable-cgi-prefork.patch')),
                'assembly_sha256': sha(original(FIBER + 'fixtures/zend_wasix_fiber.S')),
                'managed_access_patch_sha256': sha(original(PREFIX + 'patches/0001-wasi-managed-access.patch')),
                'managed_access_header_sha256': sha(original(PREFIX + 'fixtures/recastpress_fs_access.h')),
                'source_profile_sha256': sha(original(PREFIX + 'source_profile.py')),
                'raw_module_sha256': build['raw_wasm']['sha256'], 'module_bytes': build['php_wasm']['bytes'],
                'module_sha256': build['php_wasm']['sha256'], 'input_files': build_inputs,
                'scope': 'same-job NTS CGI candidate with managed-access adaptation; not product ABI or full compatibility'}}
        checker.validate_provenance(provenance, original, build_inputs)
    elif 'build.json' in data:
        equal(data['build.json']['outcome'], 'managed_access_build_failed', 'failed build remains failed')
    # Preparation inputs are fixed even if its stage did not run.
    if prepared is None:
        prepared_inputs = hashes([RUNTIME + 'prepare.py', RUNTIME + 'manifest.json',
            'experiments/php-wasi-host-probe/wordpress-fixture.json', 'scripts/wordpress_reference.py',
            'experiments/php-wasix-host-probe/language_check.py'])
        comparison_inputs = original_comparison_inputs(oracles, original_build_inputs(), {'input_files': prepared_inputs})
    else: comparison_inputs = original_comparison_inputs(oracles, original_build_inputs(), prepared)
    summaries = {}
    for kind, oracle in zip(['fiber', 'runtime'], oracles):
        name = 'linux-' + backend + '-managed-' + kind + '-contract.json'
        verdict = spec[kind + '_verdict']
        if verdict in ['skipped', 'not_run']:
            require(name not in data, 'unexecuted mode unexpectedly has comparison receipt')
            summaries[kind] = {'comparison_passed': False, 'execution_observed': False, 'stage_verdict': verdict}
        else:
            summaries[kind] = verify_comparison(data[name], kind, backend, verdict, oracle,
                comparison_inputs, provenance, prepared)
    if all(spec[kind + '_verdict'] == 'success' for kind in ['fiber', 'runtime']):
        a, b = [data['linux-' + backend + '-managed-' + kind + '-contract.json'] for kind in ['fiber', 'runtime']]
        equal(a['native_artifact_sha256'], b['native_artifact_sha256'], 'same native Rust binary in both modes')
    if stages['stages']['host_llvm'] == 'success':
        require(backend == 'llvm', 'host LLVM verification on wrong backend')
        value = data['llvm-toolchain-verification.json']
        manifest_wire = original('experiments/php-wasix-host-probe/llvm-toolchain.json')
        manifest = parse_json(manifest_wire)
        for key, expected in [('llvm_toolchain_manifest_sha256', sha(manifest_wire)), ('archive_bytes', manifest['archive_bytes']),
                ('archive_sha256', manifest['archive_sha256']), ('llvm_config_version', '22.1.8')]:
            equal(value[key], expected, 'separate host LLVM identity ' + key)
    return {'managed_build_passed': provenance is not None, 'source_patch_sets_applied': 3 if provenance else None,
        'bounded_linux_units_passed': 37 if stages['stages']['units'] == 'success' else None,
        'module': provenance['module'] if provenance else None, **summaries,
        'product_runtime_selected': False, 'full_php_compatibility_verified': False,
        'source_or_license_attestation_verified': False, 'all_resources_immediately_reclaimed': False,
        'full_wordpress_bootstrap_executed': False, 'real_db': False}


def verifier_inputs():
    """Include private spec-loader modules as well as normal Python imports."""
    paths, seen = {Path(__file__).resolve()}, set()
    queue = [prior, base, checker]
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
    raw, data = read_archive(args.backend)
    observations = verify(raw, data, args.backend)
    if not args.verify_only:
        destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'import destination escaped experiment')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'linked raw import destination')
            if path.exists(): equal(path.read_bytes(), wire, 'raw JSON immutable')
            else: path.write_bytes(wire)
        spec = pinned_spec(args.backend)
        provenance = {'format_version': 1, 'kind': 'source_verified_original_managed_access_ci_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'pinned_stages': spec['stages'], 'pinned_fiber_verdict': spec['fiber_verdict'], 'pinned_runtime_verdict': spec['runtime_verdict'],
            'original_git_source_hashes_verified': True, 'php_executed_by_importer': False,
            'native_or_wasm_binaries_acquired_by_importer': False, 'source_archive_acquired_by_importer': False,
            'source_tool_or_module_binaries_reverified_by_importer': False,
            'raw_failed_observations_preserved': 'failure' in spec['stages'].values(),
            'product_runtime_selected': False, 'full_php_compatibility_verified': False,
            'archive_member_map': spec['members'], 'input_files': verifier_inputs(),
            'imported_files': {name: sha(value) for name, value in raw.items()}, 'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'derived provenance linked')
        wire = (json.dumps(provenance, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'derived summary quota')
        path.write_bytes(wire)
    print('Verified original managed access ' + args.backend + ' evidence.')


if __name__ == '__main__':
    try: main()
    except (Exception, KeyboardInterrupt) as error:
        print('Managed access evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
