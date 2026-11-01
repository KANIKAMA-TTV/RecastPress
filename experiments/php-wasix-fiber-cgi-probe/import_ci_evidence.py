"""Import the fixed first combined CGI/Fiber CI observations from JSON-only ZIPs.

Every receipt source binding is checked against the original af4bceb Git tree,
not the current development tree. No PHP/Wasm/tool binary is acquired or run.
The historical semantic checker revalidates successful fixed-corpus records;
failed jobs retain their failure verdict and never establish unobserved gates.
"""
import argparse
from functools import lru_cache
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
sys.path.insert(0, str(HOST))
from language_check import equal, parse_json


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


reader = load('recastpress_combined_fiber_ci_original_reader', HOST / 'import_ci_evidence.py')
checker = load('recastpress_combined_fiber_ci_historical_checker', HERE / 'fiber_cgi_check.py')
unit_checker = load('recastpress_combined_fiber_ci_unit_checker', HOST / 'unit_check.py')
SOURCE = 'af4bcebe61992d8299f6e5ff0a4f16777fdb8211'
RUN = '41326284529'
reader.COMMIT = SOURCE
require = reader.require
VARIANT = 'nts-fresh-wasi-zend-fibers-cgi-v1'
CGI = 'experiments/php-wasix-cgi-build/'
FIBER = 'experiments/php-wasix-fiber-probe/'
COMBINED = 'experiments/php-wasix-fiber-cgi-probe/'
WORK = 'target/php-wasix-fiber-cgi-probe/'
SPECS = {
    'llvm': {'artifact': 13091958146, 'bytes': 49261,
        'sha256': '7b73670641a023511d1b5185c5067c07f2062e40ec18f499f8642231842f59b6', 'verdict': 'success'},
    'cranelift': {'artifact': 13092230711, 'bytes': 48820,
        'sha256': 'a5a4c7cb7e89f5508f602f345fa6e30a96d1cf277358f262136fd8d3250a4e19', 'verdict': 'success'},
}
SHA = re.compile('[0-9a-f]{64}')


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


@lru_cache(maxsize=512)
def original(name):
    return reader.original(name)


def digest(value):
    require(type(value) is str and SHA.fullmatch(value), 'invalid recorded SHA-256')


def integer(value, minimum=0, maximum=2**32):
    require(type(value) is int and minimum <= value <= maximum, 'invalid recorded integer/type/budget')


def verify_sources(value):
    hashes = value['input_files']
    require(type(hashes) is dict and hashes, 'missing original source bindings')
    for name, expected in hashes.items():
        digest(expected)
        equal(sha(original(name)), expected, 'original Git source ' + name)


def original_names(prefix):
    result = subprocess.run(['git', 'ls-tree', '-r', '--name-only', SOURCE, '--', prefix], cwd=ROOT,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    require(len(result.stdout) <= 1024 * 1024 and not result.stderr, 'original Git listing budget')
    return result.stdout.decode('utf-8', errors='strict').splitlines()


def original_prepare_inputs():
    names = {name for name in original_names(CGI) if
        ('/' not in name[len(CGI):] and name.endswith('.py')) or
        (name.startswith(CGI + 'patches/') and name.endswith('.patch'))}
    names.update([CGI + 'manifest.json', CGI + 'README.ja.md', '.github/workflows/php-wasix-cgi-build.yml'])
    return {name: sha(original(name)) for name in sorted(names)}


def original_build_inputs():
    inputs = original_prepare_inputs()
    names = [COMBINED + name for name in ['build.py', 'patch_source.py', 'manifest.json', 'fixtures/fiber_cgi_contract.php']]
    names += [FIBER + name for name in ['build.py', 'patch_source.py', 'manifest.json',
        'patches/0001-nts-fresh-wasi-zend-fibers.patch', 'fixtures/zend_wasix_fiber.S', 'fixtures/fiber_contract.php']]
    names += [CGI + 'patches/0001-wasi-disable-cgi-prefork.patch']
    for name in names: inputs[name] = sha(original(name))
    return dict(sorted(inputs.items()))


def members(backend):
    result = {name: 'php-wasix-fiber-cgi-probe/evidence/' + name for name in
        ['build.json', 'ci-stages.json', 'native-cgi-fiber-contract.json', 'linux-' + backend + '-fiber-cgi-contract.json']}
    result['toolchain.json'] = 'php-wasix-cgi-build/evidence/toolchain.json'
    for name in ['windows-cranelift-unit-contract.json', 'linux-' + backend + '-unit-contract.json']:
        result[name] = 'php-wasix-host-probe/evidence/' + name
    if backend == 'llvm':
        result['llvm-toolchain-verification.json'] = 'php-wasix-host-probe/evidence/llvm-toolchain-verification.json'
    return result


def read_archive(backend):
    spec = SPECS[backend]
    path = ROOT / ('target/php-wasix-ci-evidence/run-' + RUN + '-' + backend + '-1.zip')
    require(path.resolve(strict=True) == path and path.is_file(), 'fixed artifact missing or linked')
    equal(path.stat().st_size, spec['bytes'], 'fixed artifact ZIP length')
    wire = path.read_bytes()
    equal((len(wire), sha(wire)), (spec['bytes'], spec['sha256']), 'fixed artifact ZIP identity')
    mapping = members(backend)
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        equal(len(entries), len(mapping), 'exact evidence member count')
        equal({entry.filename for entry in entries}, set(mapping.values()), 'exact evidence member paths')
        for entry in entries:
            require(0 < entry.file_size <= 512 * 1024 and not entry.is_dir()
                and (entry.external_attr >> 16) & 0o170000 != 0o120000 and entry.flag_bits & 1 == 0,
                'evidence member type/size/encryption')
        raw = {name: archive.read(member) for name, member in mapping.items()}
    return raw, {name: parse_json(wire) for name, wire in raw.items()}


def verify_build(data, raw):
    build, tools = data['build.json'], data['toolchain.json']
    definition = parse_json(original(COMBINED + 'manifest.json'))
    tool_definition = parse_json(original(CGI + 'manifest.json'))
    equal(build['input_files'], original_build_inputs(), 'complete original combined builder inputs')
    equal(tools['input_files'], original_prepare_inputs(), 'complete original prepared-tool inputs')
    for key, expected in [('schema', 'recastpress.php-wasix-fiber-cgi-build.v0.5'), ('variant', VARIANT),
            ('outcome', 'fiber_cgi_candidate_built_not_executed'), ('phase', 'build_complete'),
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
    equal(len(build['source_patches']), 2, 'both canonical source patches')
    for patch, plan in zip(build['source_patches'], definition['source_patches']):
        for key in ['kind', 'path', 'bytes', 'sha256']:
            equal(patch[key], plan[key], 'canonical combined patch ' + key)
        equal(sha(original(plan['path'])), plan['sha256'], 'fixed original patch bytes')
        equal(patch['source_commit'], definition['upstream']['commit'], 'patched PHP commit')
        for key in ['combined_tracked_source_clean_before_first_patch', 'applied', 'git_apply_check_passed', 'git_apply_passed']:
            equal(patch[key], True, 'source patch gate ' + key)
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
            if name != 'Zend/asm/recastpress_wasix_fiber.S':
                equal(build['tracked_changes_after_build'][name], {key: after[key] for key in ['bytes', 'sha256']},
                    'patched source unchanged after build')
    return original_build_inputs()


def verify_units(value, backend):
    equal((value['outcome'], value['tests_passed'], value['owned_test_reaped'], value['exit_code']),
        ('fixed_p0_unit_tests_passed', True, True, 0), 'bounded Linux unit result')
    equal(value['backend'], backend, 'Linux unit backend')
    equal(value['platform']['system'], 'Linux', 'Linux unit system')
    for key, expected in [('php_executed', False), ('php_cpu_budget_verified', False), ('product_runtime_selected', False),
            ('outer_build_timeout_seconds', 1200), ('outer_list_timeout_seconds', 10), ('outer_test_timeout_seconds', 30),
            ('build_exit_code', 0), ('list_exit_code', 0)]:
        equal(value[key], expected, 'unit scope/deadline ' + key)
    names = value['listed_tests']
    equal(len(names), 27, 'original Linux27 unit inventory')
    equal(len(set(names)), 27, 'unique Linux27 units')
    groups = {'cgi_contract::cgi_response::tests::': 9, 'language_contract::tests::': 2,
        'context_contract::tests::': 1, 'fiber_adapter::tests::': 1, 'memory_limits::': 3, 'rpc_fs::tests::': 9, 'metering_limits::': 2}
    for prefix, size in groups.items(): equal(sum(name.startswith(prefix) for name in names), size, 'original unit group ' + prefix)
    for key in ['owned_list_pid', 'owned_test_pid']: integer(value[key], 1)
    for key in ['test_artifact_sha256', 'stdout_sha256', 'stderr_sha256']: digest(value[key])
    require('test result: ok. 27 passed; 0 failed; 0 ignored;' in value['stdout_excerpt'], 'all original tests ran without ignores')
    summaries = unit_checker.validate_metering_rows(value['wat_metering_observations'], backend, 'Linux')
    equal(value['wat_metering_summaries'], summaries, 'actual bounded WAT rows and summaries')


def verify(raw, data, backend):
    spec = SPECS[backend]
    stages, build, tools = [data[name] for name in ['ci-stages.json', 'build.json', 'toolchain.json']]
    for value in data.values():
        if 'input_files' in value: verify_sources(value)
    equal((stages['source_commit'], stages['run_id'], stages['run_attempt'], stages['backend']),
        (SOURCE, RUN, '1', backend), 'original CI identity')
    equal(stages['product_runtime_selected'], False, 'CI does not select product runtime')
    equal(stages['redistributed_binaries'], False, 'JSON-only artifact scope')
    build_inputs = verify_build(data, raw)
    for name, source in [('native-cgi-fiber-contract.json', COMBINED), ('windows-cranelift-unit-contract.json', 'experiments/php-wasix-host-probe/')]:
        equal(raw[name], original(source + 'evidence/' + name), 'unchanged checked-out reference ' + name)
    oracle = data['native-cgi-fiber-contract.json']
    checker.verify_native_reference(oracle, read_source=original)
    unit = data['linux-' + backend + '-unit-contract.json']
    probe = data['linux-' + backend + '-fiber-cgi-contract.json']
    expected_stages = dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions',
        'tools', 'build', 'rust', 'host_llvm', 'units', 'fiber_contract'], 'success')
    expected_stages['host_llvm'] = 'success' if backend == 'llvm' else 'skipped'
    for key in ['product_runtime_selected', 'full_php_compatibility_verified', 'native_runtime_fallback_used',
            'all_resources_immediately_reclaimed', 'memory_limit_verified', 'cpu_limit_verified', 'stack_free_or_reuse_verified']:
        equal(probe[key], False, 'unclaimed comparison gate ' + key)
    equal(probe['backend'], backend, 'comparison backend')
    equal(probe['platform']['system'], 'Linux', 'comparison system')
    provenance = probe['generated_provenance']
    equal(provenance['build_receipt_sha256'], sha(raw['build.json']), 'same generated build receipt')
    equal(provenance['module'], build['php_wasm'], 'same generated final module')
    equal(provenance['builder_provenance']['raw_module_sha256'], build['raw_wasm']['sha256'], 'same generated raw module')
    checker.validate_provenance(provenance, read_source=original, build_inputs=build_inputs)
    if spec['verdict'] == 'success':
        equal(stages['stages'], expected_stages, 'all original combined stages passed')
        verify_units(unit, backend)
        for key, expected in [('outcome', 'fixed_php_fiber_cgi_contract_passed'), ('fiber_contract_passed', True),
                ('owned_host_reaped', True), ('exit_code', 0), ('outer_build_timeout_seconds', 1200), ('outer_execution_timeout_seconds', 600)]:
            equal(probe[key], expected, 'successful combined comparison ' + key)
        checker.validate_result(probe['native_result'], probe['owned_process_pid'], backend, provenance,
            read_source=original, build_inputs=build_inputs)
        result = probe['native_result']
        return {'combined_php_build_passed': True, 'canonical_source_patch_sets_applied': 2,
            'module_bytes': build['php_wasm']['bytes'], 'module_sha256': build['php_wasm']['sha256'],
            'postopt_adapter_verified': True, 'bounded_linux_units_passed': 27, 'fixed_native_cases': 6,
            'fresh_cgi_wasi_instances': 3, 'module_compile_calls': 1, 'compile_elapsed_ms': result['compile_elapsed_ms'],
            'guest_execution_elapsed_ms': [case['guest_execution_elapsed_ms'] for case in result['cases']],
            'fixed_cgi_fiber_comparison_passed': True, 'full_php_compatibility_verified': False,
            'all_resources_immediately_reclaimed': False, 'physical_stack_free_or_reuse_verified': False,
            'product_runtime_selected': False}
    # A failed checkpoint must be explicitly pinned as such. Keep PHP's raw
    # observations, phase, error and boolean verdicts; never normalize a failure.
    require(spec['verdict'] == 'failure', 'unknown fixed CI verdict')
    expected_stages['fiber_contract'] = 'failure'
    equal(stages['stages'], expected_stages, 'original failed comparison stage')
    verify_units(unit, backend)
    equal(probe['fiber_contract_passed'], False, 'failed comparison stays false')
    require(probe['outcome'] != 'fixed_php_fiber_cgi_contract_passed', 'failed checkpoint became successful')
    result = probe.get('native_result')
    if result is not None:
        equal(result['ok'], False, 'failed host verdict retained')
        equal(result['php_fiber_compatibility_verified_for_fixed_cases'], False, 'failed fixed-corpus gate retained')
    return {'combined_php_build_passed': True, 'fixed_cgi_fiber_comparison_passed': False,
        'bounded_linux_units_passed': 27, 'observed_outcome': probe['outcome'],
        'observed_host_phase': result.get('phase') if result else None,
        'full_php_compatibility_verified': False, 'product_runtime_selected': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=sorted(SPECS), required=True)
    parser.add_argument('--verify-only', action='store_true')
    args = parser.parse_args()
    raw, data = read_archive(args.backend)
    observations = verify(raw, data, args.backend)
    if not args.verify_only:
        destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'evidence path escaped experiment')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'import destination is linked')
            if path.exists(): equal(path.read_bytes(), wire, 'raw imported receipt immutable')
            else: path.write_bytes(wire)
        inputs = [Path(__file__).resolve(), HOST / 'import_ci_evidence.py', HOST / 'language_check.py',
            HERE / 'fiber_cgi_check.py', HERE / 'native_contract.py', HERE / 'build.py', HERE / 'patch_source.py', HOST / 'unit_check.py',
            ROOT / 'experiments/php-wasix-cgi-build/prepare.py', HOST / 'check.py',
            ROOT / 'experiments/php-wasix-fiber-probe/native_contract.py',
            ROOT / 'experiments/php-runtime-candidate-probe/native_contract.py', ROOT / 'scripts/owned_probe.py']
        spec = SPECS[args.backend]
        provenance = {'format_version': 1, 'kind': 'source_verified_first_combined_cgi_fiber_ci_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'ci_verdict': spec['verdict'], 'original_git_source_hashes_verified': True,
            'php_executed_by_importer': False, 'native_or_wasm_binaries_acquired_by_importer': False,
            'source_tool_or_module_binaries_reverified_by_importer': False,
            'product_runtime_selected': False, 'full_php_compatibility_verified': False,
            'archive_member_map': members(args.backend), 'input_files': {p.relative_to(ROOT).as_posix(): sha(p.read_bytes()) for p in inputs},
            'imported_files': {name: sha(wire) for name, wire in raw.items()}, 'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'derived provenance is linked')
        path.write_text(json.dumps(provenance, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('Verified original combined CGI/Fiber ' + args.backend + ' checkpoint; CI verdict=' + SPECS[args.backend]['verdict'] + '.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print('Combined CGI/Fiber evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
