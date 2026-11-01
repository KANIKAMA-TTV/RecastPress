"""Import pinned P0 checkpoint ZIPs without changing their execution verdicts."""
import argparse
import json
from pathlib import Path
import sys
import zipfile

import import_ci_evidence as reader
import language_check
import reuse_check
import cgi_check
import memory_check
import rpc_check
import unit_check

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = '071c9443ba56b2b5dd27032567ed3e7a9c8d0045'
SPECS = {
    '41308237911-llvm': {
        'run_id': '41308237911', 'backend': 'llvm',
        'source': 'de33e0a8db80b6b5d632b25d7e3dfc30deb3299a',
        'kind': 'compiler-matrix', 'artifact': 13084916171, 'bytes': 132600,
        'sha256': 'c8b50bb2dec510bbb66420577ce89969147cc416199c7781eb98935772f6a906',
        'directory': 'experiments/php-wasix-host-probe',
        'current': ['ci-stages.json', 'linux-llvm.json', 'linux-llvm-module-reuse.json',
                    'linux-llvm-rpc-contract.json', 'linux-llvm-php-memory-cap.json',
                    'linux-llvm-language-contract.json', 'linux-llvm-unit-contract.json',
                    'linux-llvm-dependency-inventory.json', 'llvm-toolchain-verification.json'],
    },
    '41308237911-cranelift': {
        'run_id': '41308237911', 'backend': 'cranelift',
        'source': 'de33e0a8db80b6b5d632b25d7e3dfc30deb3299a',
        'kind': 'compiler-matrix', 'artifact': 13084366721, 'bytes': 132366,
        'sha256': '39e9505cf09111516185c14d6ce7a63423f14da3564b83bcfcc658558bc67936',
        'directory': 'experiments/php-wasix-host-probe',
        'current': ['ci-stages.json', 'linux-cranelift.json', 'linux-cranelift-module-reuse.json',
                    'linux-cranelift-rpc-contract.json', 'linux-cranelift-php-memory-cap.json',
                    'linux-cranelift-language-contract.json', 'linux-cranelift-unit-contract.json',
                    'linux-cranelift-dependency-inventory.json'],
    },
    '41305668059': {
        'source': 'e9a2e6c050f69c325a75378aa78dc847abe8581f',
        'kind': 'cgi-contract', 'artifact': 13083584092, 'bytes': 28215,
        'sha256': '1cd057351001e6f21359a8bf67991d76b1c213293aa568e5f470b63baec1187c',
        'module_sha256': 'ad35f71e03ea04af4955fcbe860f34a3a9988c2cb55006ff1a69c919969017b0',
        'directory': 'experiments/php-wasix-cgi-build',
        'current': ['ci-stages.json', 'build.json', 'toolchain.json', 'linux-llvm-cgi-contract.json',
                    'llvm-toolchain-verification.json', 'native-cgi-contract.json'],
        'archive_members': {
            **{name: 'php-wasix-cgi-build/evidence/' + name
               for name in ['ci-stages.json', 'build.json', 'toolchain.json']},
            **{name: 'php-wasix-host-probe/evidence/' + name for name in
               ['linux-llvm-cgi-contract.json', 'llvm-toolchain-verification.json', 'native-cgi-contract.json']},
        },
    },
    '41305668144': {
        'source': 'e9a2e6c050f69c325a75378aa78dc847abe8581f',
        'kind': 'rpc-memory-passed', 'artifact': 13084011078, 'bytes': 78874,
        'sha256': 'e754bae66ee90740b8eb0770659ec0613c9f8e184bc43994fca1f8cf4cfbc5a3',
        'directory': 'experiments/php-wasix-host-probe',
        'current': ['ci-stages.json', 'linux-llvm.json', 'linux-llvm-module-reuse.json',
                    'linux-llvm-rpc-contract.json', 'linux-llvm-php-memory-cap.json',
                    'linux-llvm-language-contract.json', 'llvm-toolchain-verification.json'],
    },
    '41302992818': {
        'source': '7c57fad7a7b996f4dddad3a23c3d71ef79114910',
        'kind': 'cgi-contract', 'artifact': 13082738133, 'bytes': 28128,
        'sha256': '716a20927a6e2e9f7f6b713bf30164dab31bcfab39fd66b1a8ce82a63f68696c',
        'module_sha256': '2879787251cea507843d6ab98c0f0fd4eea5005b5fe74120c8c512541a73ed59',
        'directory': 'experiments/php-wasix-cgi-build',
        'current': ['ci-stages.json', 'build.json', 'toolchain.json', 'linux-llvm-cgi-contract.json',
                    'llvm-toolchain-verification.json', 'native-cgi-contract.json'],
        'archive_members': {
            **{name: 'php-wasix-cgi-build/evidence/' + name
               for name in ['ci-stages.json', 'build.json', 'toolchain.json']},
            **{name: 'php-wasix-host-probe/evidence/' + name for name in
               ['linux-llvm-cgi-contract.json', 'llvm-toolchain-verification.json', 'native-cgi-contract.json']},
        },
    },
    '41302992753': {
        'source': '7c57fad7a7b996f4dddad3a23c3d71ef79114910',
        'kind': 'php-memory-startup-failed', 'artifact': 13082023573, 'bytes': 57423,
        'sha256': '2b5326b20eaa7ab1be6462678920e553568699accd20a0c766a0872bb0689926',
        'directory': 'experiments/php-wasix-host-probe',
        'current': ['ci-stages.json', 'linux-llvm.json', 'linux-llvm-module-reuse.json',
                    'linux-llvm-php-memory-cap.json', 'llvm-toolchain-verification.json'],
    },
    '41300641489': {
        'source': '98215a27f7f48f7738ad6e0008d3f1fd97473fe0',
        'kind': 'module-reuse', 'artifact': 13081821666, 'bytes': 38109,
        'sha256': '4682233d47bc5588a8b21609fb7a13aeaea262a1c9be54403ff0031a8c3d13b6',
        'directory': 'experiments/php-wasix-host-probe',
        'current': ['ci-stages.json', 'linux-llvm.json', 'linux-llvm-module-reuse.json',
                    'linux-llvm-language-contract.json', 'llvm-toolchain-verification.json'],
    },
    '41300641538': {
        'source': '98215a27f7f48f7738ad6e0008d3f1fd97473fe0',
        'kind': 'cgi-build-passed', 'artifact': 13081128026, 'bytes': 13851,
        'sha256': '74d4636b3f3800ce4ec8acc0bf910ed6edba259fa81666f3c7115cddb49b05f0',
        'directory': 'experiments/php-wasix-cgi-build',
        'current': ['ci-stages.json', 'build.json', 'toolchain.json'],
    },
    '41298369677': {
        'kind': 'module-reuse', 'artifact': 13080950246, 'bytes': 23762,
        'sha256': '39cc2f495a627010c76adbcd50f9a6a353746cdea202118c4ec99d6fadfe2045',
        'directory': 'experiments/php-wasix-host-probe',
        'current': ['ci-stages.json', 'linux-llvm.json', 'linux-llvm-module-reuse.json',
                    'linux-llvm-language-contract.json', 'llvm-toolchain-verification.json'],
    },
    '41298369729': {
        'kind': 'cgi-build-failed', 'artifact': 13080108223, 'bytes': 12816,
        'sha256': '8f852304dd0190d8c41ad01c69565c3d6dc14024db9f485590ffb86f064c4feb',
        'directory': 'experiments/php-wasix-cgi-build',
        'current': ['ci-stages.json', 'build.json', 'toolchain.json'],
    },
}
require, sha = reader.require, reader.sha


def verify_sources(report):
    for path, expected in report['input_files'].items():
        require(sha(reader.original(path)) == expected, 'original source mismatch: ' + path)


def verify_reuse(data, extended=False, backend='llvm', matrix=False):
    prefix = 'linux-' + backend
    stages = data['ci-stages.json']['stages']
    expected = dict.fromkeys(['dependencies', 'rust', 'php', 'cargo', 'llvm', 'fixture_fs', 'probe', 'reuse'], 'success')
    expected['language'] = 'failure'
    if extended:
        expected.update(rpc='success', php_memory='success')
    if matrix:
        expected.update(license_inventory='success', llvm='success' if backend == 'llvm' else 'skipped')
        require(data['ci-stages.json']['backend'] == backend, 'wrong CI backend')
    require(stages == expected, 'unexpected reuse CI stage outcomes')
    for name in [prefix + '.json', prefix + '-module-reuse.json', prefix + '-language-contract.json']:
        verify_sources(data[name])
    version = data[prefix + '.json']
    reuse = data[prefix + '-module-reuse.json']
    language = data[prefix + '-language-contract.json']
    require(version['outcome'] == 'version_only_passed' and version['php_version_verified'] is True,
            'version result changed')
    require(reuse['outcome'] == 'module_reuse_passed' and reuse['reuse_passed'] is True
            and reuse['exit_code'] == 0 and reuse['production_compatible'] is False,
            'missing successful module reuse result')
    reuse_check.equal(reuse['observed_phases'], reuse_check.PHASES, 'observed reuse phases')
    reuse_check.validate_result(reuse['native_result'], reuse['owned_process_pid'], backend)
    require(len({item['native_artifact_sha256'] for item in [version, reuse, language]}) == 1,
            'modes used different native binaries')
    require(language['outcome'] == 'comparison_failed' and language['comparison_passed'] is False,
            'language failure became success')
    native = language_check.parse_json(reader.original('experiments/php-runtime-candidate-probe/evidence/native-contract.json'))
    verify_sources(native)
    cases = language_check.compare_cases(language['observation'], native)
    require(cases == language['case_results'], 'captured individual language comparisons changed')
    require([cases[name]['equal_to_native'] for name in language_check.CASES] == [True, True, False],
            'language observations changed')
    result = reuse['native_result']
    return {
        'module_reuse_passed': True, 'language_comparison_passed': False,
        'case_results': cases, 'compile_elapsed_ms': result['compile_elapsed_ms'],
        'request_execution_elapsed_ms': [item['php_execution_elapsed_ms'] for item in result['requests']],
        'counter_scope': result['reuse']['counter_scope'],
        'timing_scope': 'one CI runner; run_wasm includes startup/shutdown; not a product latency benchmark',
    }


def verify_cgi(data, success, contract=False):
    expected = dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions', 'tools'], 'success')
    expected['build'] = 'success' if success else 'failure'
    if contract:
        expected.update(rust='success', host_llvm='success', cgi_contract='success')
    require(data['ci-stages.json']['stages'] == expected, 'unexpected CGI CI stage outcomes')
    build, tools = data['build.json'], data['toolchain.json']
    verify_sources(build)
    verify_sources(tools)
    require(build['guest_executed'] is False and build['product_runtime_selected'] is False
            and build['module_validation_performed'] is False, 'build became an execution/adoption claim')
    if success:
        require(build['outcome'] == 'cgi_candidate_built_not_executed' and build['phase'] == 'build_complete',
                'CGI build did not complete')
    else:
        require(build['outcome'] == 'build_failed' and build['phase'] == 'make_cgi' and 'cgi_wasm' not in build,
                'failed CGI build became a module claim')
    require(tools['outcome'] == 'verified_host_build_tools_prepared'
            and sha(raw_toolchain(data)) == build['toolchain_receipt_sha256'], 'wrong toolchain receipt')
    manifest = json.loads(reader.original('experiments/php-wasix-cgi-build/manifest.json'))
    for asset in manifest['assets']:
        row = next(row for row in tools['assets'] if row['name'] == asset['name'])
        require(row['verified'] is True and row['observed_bytes'] == asset['bytes']
                and row['observed_sha256'] == asset['sha256'], 'unverified CGI build tool')
    for name, source in manifest['sources'].items():
        require(build['sources'][name]['commit'] == source['commit']
                and build['sources'][name]['tracked_worktree_clean_before_build'] is True,
                'unexpected CGI source checkout')
    if success:
        require(build['configure_difference']['extra_flags'] == '--enable-cgi'
                and build['configure_difference']['php_source_patch'] is True,
                'CGI source/configure difference changed')
        require([(s['name'], s['exit_code']) for s in build['build_steps']] ==
                [('configure', 0), ('make_cgi', 0), ('optimize_cgi', 0)], 'CGI build steps did not complete')
        require(len(build['source_patches']) == len(manifest['source_patches']) == 1, 'wrong patch count')
        patch, fixed = build['source_patches'][0], manifest['source_patches'][0]
        for key in ['path', 'sha256', 'source_commit', 'target', 'before_bytes', 'before_sha256', 'after_sha256']:
            require(patch[key] == fixed[key], 'patch identity changed: ' + key)
        require(patch['applied'] is True and patch['clean_tracked_source_before_patch'] is True
                and patch['observed_patch_sha256'] == fixed['sha256']
                and patch['observed_before_sha256'] == fixed['before_sha256']
                and patch['observed_after_sha256'] == fixed['after_sha256'], 'patch provenance incomplete')
        expected_sha = (data['_fixed_module_sha'] if contract else
                        '6fb1d3c02e57aef7fbd9f06b315a93eab1d985453d104aab9147bc34c6837c43')
        require(build['cgi_wasm'] == {'path': 'sapi/cgi/php-cgi.wasm', 'bytes': 40002084,
                'sha256': expected_sha,
                'wasm_header_verified': True, 'executed': False}, 'CGI output observation changed')
        return {'tool_preparation_passed': True, 'configure_passed': True, 'cgi_build_passed': True,
                'cgi_guest_executed': False, 'cgi_wasm': build['cgi_wasm']}
    require(build['configure_difference'] == {'extra_flags': '--enable-cgi', 'php_source_patch': False,
                                             'cli_disabled': False, 'make_target': 'cgi', 'make_jobs': 2},
            'initial build source/configure difference changed')
    require([(s['name'], s['exit_code']) for s in build['build_steps']] == [('configure', 0), ('make_cgi', 2)],
            'initial build failure phase changed')
    return {'tool_preparation_passed': True, 'configure_passed': True, 'cgi_build_passed': False,
            'cgi_guest_executed': False, 'failure_phase': 'make_cgi',
            'failure_diagnostics': build['diagnostics']}


def raw_toolchain(data):
    return data['_raw_toolchain']


def verify_memory_startup_failure(data):
    expected = dict.fromkeys(['dependencies', 'rust', 'php', 'cargo', 'llvm', 'fixture_fs',
                              'probe', 'reuse'], 'success')
    expected.update(php_memory='failure', language='skipped')
    require(data['ci-stages.json']['stages'] == expected, 'unexpected memory failure stages')
    version, reuse, memory = [data[name] for name in
        ['linux-llvm.json', 'linux-llvm-module-reuse.json', 'linux-llvm-php-memory-cap.json']]
    for report in [version, reuse, memory]:
        verify_sources(report)
    require(version['outcome'] == 'version_only_passed' and version['php_version_verified'] is True,
            'version control failed')
    require(reuse['outcome'] == 'module_reuse_passed' and reuse['reuse_passed'] is True
            and reuse['exit_code'] == 0, 'reuse control failed')
    reuse_check.validate_result(reuse['native_result'], reuse['owned_process_pid'], 'llvm')
    require(len({r['native_artifact_sha256'] for r in [version, reuse, memory]}) == 1,
            'modes used different host binaries')
    require(memory['outcome'] == 'execution_failed' and memory['diagnostic_passed'] is False
            and memory['exit_code'] == 1, 'memory failure became success')
    result = memory['native_result']
    require(result['ok'] is False and result['diagnostic_verified'] is False
            and result['php_linear_memory_cap_verified_for_fixed_cases'] is False
            and result['test_cap_pages'] == 1024 and result['allocation_bytes'] == 134217728,
            'original failed trial changed')
    cases = result['cases']
    require([c['name'] for c in cases] == ['capped_normal', 'capped_allocation_oom',
            'capped_normal_recovery', 'uncapped_allocation_control'], 'wrong memory cases')
    require([c['ok'] for c in cases] == [False, False, False, True], 'memory results changed')
    require([c['expectation_met'] for c in cases] == [False, False, False, True],
            'startup failures became expected script OOM or recovery')
    for case in cases:
        require(case['memory_observation_count'] == 1 and case['memory']['shared'] is True
                and case['stdout_eof'] is True and case['stderr_eof'] is True
                and case['stdout_overflow'] is False and case['stderr_overflow'] is False
                and case['runtime_shutdown_returned'] is True, 'incomplete memory observation')
    for case in cases[:3]:
        require(case['cap_pages'] == case['memory']['maximum_pages'] == 1024
                and case['memory']['size_pages'] <= 1024 and case['php'] is None
                and case['stdout_bytes'] == 0 and case['oom_observed'] is False
                and 'Unable to allocate shared memory segment of 134217728 bytes' in case['stderr']
                and 'Out of memory' in case['stderr']
                and 'RP_PHP_MEMORY_ALLOCATION_READY' not in case['stderr'],
                'failure did not precede the PHP script')
    control = cases[3]
    require(control['cap_pages'] is None and control['memory']['maximum_pages'] == 65536
            and control['memory']['size_pages'] > 1024
            and control['php']['allocated_bytes'] == 134217728
            and control['php']['memory_limit'] == '-1', 'uncapped control changed')
    return {'module_reuse_passed': True, 'php_memory_diagnostic_passed': False,
            'failed_cap_bytes': 67108864, 'failure_phase': 'OPcache startup before PHP script',
            'capped_script_execution_verified': False, 'recovery_verified': False,
            'uncapped_allocation_control_passed': True, 'language_cases_executed': False}


def verify_cgi_contract(data):
    summary = verify_cgi(data, True, contract=True)
    contract, native = data['linux-llvm-cgi-contract.json'], data['native-cgi-contract.json']
    verify_sources(contract)
    verify_sources(native)
    original_native = reader.original('experiments/php-wasix-host-probe/evidence/native-cgi-contract.json')
    require(sha(original_native) == sha(data['_raw_native']), 'CI native baseline changed')
    cgi_check.validate_native_baseline(native, reader.original)
    require(contract['outcome'] == 'cgi_contract_passed' and contract['cgi_contract_passed'] is True
            and contract['native_comparison_passed'] is True and contract['native_comparison_performed'] is True
            and contract['owned_host_reaped'] is True and contract['exit_code'] == 0,
            'CGI contract or process cleanup failed')
    provenance = contract['generated_provenance']
    manifest_wire = reader.original('experiments/php-wasix-cgi-build/manifest.json')
    manifest = json.loads(manifest_wire)
    require(provenance == {
        'build_receipt_sha256': sha(data['_raw_build']),
        'toolchain_receipt_sha256': sha(data['_raw_toolchain']),
        'manifest_sha256': sha(manifest_wire), 'module_bytes': data['build.json']['cgi_wasm']['bytes'],
        'module_sha256': data['build.json']['cgi_wasm']['sha256'],
        'source_commit': manifest['sources']['php']['commit'],
        'patch_sha256': manifest['source_patches'][0]['sha256'],
        'verification_scope': 'same trusted job observations; not signed attestation or reproducible build'},
        'CGI build and host execution were not bound to the same bytes')
    require(contract['native_reference']['receipt_sha256'] == sha(original_native)
            and contract['native_reference']['native_executed_by_this_helper'] is False,
            'wrong native oracle or fallback execution claim')
    cgi_check.validate_result(contract['native_result'], contract['owned_process_pid'], 'llvm', provenance)
    comparisons = cgi_check.compare_native_requests(contract['native_result'], native)
    require(comparisons == contract['native_comparison_results']
            and all(row['equal_to_native'] is True for row in comparisons.values()),
            'native CGI comparison changed')
    llvm = data['llvm-toolchain-verification.json']
    llvm_wire = reader.original('experiments/php-wasix-host-probe/llvm-toolchain.json')
    llvm_manifest = json.loads(llvm_wire)
    require(llvm['llvm_toolchain_manifest_sha256'] == sha(llvm_wire)
            and llvm['archive_sha256'] == llvm_manifest['archive_sha256']
            and llvm['archive_bytes'] == llvm_manifest['archive_bytes']
            and llvm['llvm_config_version'] == '22.1.8'
            and llvm['verifier_source_sha256'] == sha(reader.original(
                'experiments/php-wasix-host-probe/prepare_host_llvm.py')), 'host LLVM identity changed')
    summary.update(cgi_guest_executed=True, builder_executed_guest=False,
                   native_comparison_passed=True, native_comparison_results=comparisons,
                   compile_elapsed_ms=contract['native_result']['compile_elapsed_ms'],
                   request_execution_elapsed_ms=[r['php_execution_elapsed_ms']
                                                 for r in contract['native_result']['requests']],
                   actual_http_listener_tested=False, real_db=False, wordpress_bootstrapped=False,
                   reproducible_build_verified=False)
    return summary


def verify_rpc_memory(data, backend='llvm', matrix=False):
    prefix = 'linux-' + backend
    summary = verify_reuse(data, extended=True, backend=backend, matrix=matrix)
    rpc, memory = data[prefix + '-rpc-contract.json'], data[prefix + '-php-memory-cap.json']
    for report in [rpc, memory]:
        verify_sources(report)
    require(rpc['outcome'] == 'rpc_contract_diagnostic_passed' and rpc['rpc_contract_passed'] is True
            and rpc['owned_host_reaped'] is True and rpc['exit_code'] == 0,
            'RPC diagnostic did not pass')
    rpc_check.equal(rpc['observed_phases'], ['module_compile'] + rpc_check.CASES, 'RPC observed phases')
    rpc_check.validate_result(rpc['native_result'], rpc['owned_process_pid'], backend)
    require(memory['outcome'] == 'php_memory_cap_diagnostic_passed' and memory['diagnostic_passed'] is True
            and memory['exit_code'] == 0, 'PHP memory diagnostic did not pass')
    memory_check.equal(memory['observed_phases'], ['module_compile'] + memory_check.CASES, 'memory observed phases')
    memory_check.validate_result(memory['native_result'], memory['owned_process_pid'], backend)
    require(len({data[name]['native_artifact_sha256'] for name in
                 [prefix + '.json', prefix + '-module-reuse.json', prefix + '-rpc-contract.json',
                  prefix + '-php-memory-cap.json', prefix + '-language-contract.json']}) == 1,
            'diagnostics used different host binaries')
    summary.update(private_rpc_diagnostic_passed=True, php_memory_diagnostic_passed=True,
                   incomplete_rpc_case_accepted=False, test_memory_cap_bytes=268435456,
                   production_memory_limit_verified=False, real_db=False, standard_wpdb=False,
                   rpc_case_acceptance=[case['ok'] for case in rpc['native_result']['cases']],
                   memory_case_execution_results=[case['ok'] for case in memory['native_result']['cases']],
                   memory_observations=[case['memory'] for case in memory['native_result']['cases']])
    return summary


def verify_compiler_matrix(data, backend):
    summary = verify_rpc_memory(data, backend, matrix=True)
    prefix = 'linux-' + backend
    unit, inventory = data[prefix + '-unit-contract.json'], data[prefix + '-dependency-inventory.json']
    for report in [unit, inventory]:
        verify_sources(report)
    require(unit['outcome'] == 'fixed_p0_unit_tests_passed' and unit['tests_passed'] is True
            and unit['owned_test_reaped'] is True and unit['exit_code'] == 0
            and len(unit['listed_tests']) == 25 and unit['backend'] == backend
            and unit['php_executed'] is False and unit['php_cpu_budget_verified'] is False,
            'missing bounded WAT unit result')
    summaries = unit_check.validate_metering_rows(unit['wat_metering_observations'], backend, 'Linux')
    require(summaries == unit['wat_metering_summaries'], 'WAT metering summary changed')
    require(inventory['outcome'] == 'declared_dependency_inventory_recorded'
            and inventory['inventory_verified'] is True and inventory['owned_metadata_reaped'] is True
            and inventory['owned_cleanup_confirmed'] is True and inventory['input_files_unchanged'] is True
            and inventory['backend'] == backend and inventory['metadata_exit_code'] == 0
            and inventory['legal_compatibility_evaluated'] is False
            and inventory['native_llvm_toolchain_license_provenance_verified'] is False
            and inventory['php_static_archive_license_provenance_verified'] is False,
            'dependency inventory failed or became a legal adoption verdict')
    reach = inventory['compiler_reachability']
    require(any(reach['wasmer-compiler-llvm'].values()) == (backend == 'llvm')
            and any(reach['wasmer-compiler-cranelift'].values()) == (backend == 'cranelift'),
            'wrong compiler reachability')
    if backend == 'llvm':
        llvm = data['llvm-toolchain-verification.json']
        manifest_wire = reader.original('experiments/php-wasix-host-probe/llvm-toolchain.json')
        manifest = json.loads(manifest_wire)
        require(llvm['llvm_toolchain_manifest_sha256'] == sha(manifest_wire)
                and llvm['archive_sha256'] == manifest['archive_sha256']
                and llvm['archive_bytes'] == manifest['archive_bytes']
                and llvm['llvm_config_version'] == '22.1.8'
                and llvm['verifier_source_sha256'] == sha(reader.original(
                    'experiments/php-wasix-host-probe/prepare_host_llvm.py')), 'host LLVM identity changed')
    summary.update(backend=backend, bounded_unit_tests_passed=25, formal_eh_wat_metering_passed=True,
                   php_cpu_budget_verified=False, declared_license_inventory_recorded=True,
                   legal_compatibility_evaluated=False, compiler_reachability=reach)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', required=True, choices=sorted(SPECS))
    args = parser.parse_args()
    spec = SPECS[args.run]
    run_id = spec.get('run_id', args.run)
    source = spec.get('source', SOURCE)
    reader.COMMIT = source
    path = ROOT / 'target/php-wasix-ci-evidence' / ('run-' + args.run + '-1.zip')
    wire = path.read_bytes()
    require(len(wire) == spec['bytes'] and sha(wire) == spec['sha256'], 'CI ZIP identity changed')
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        names = {entry.filename for entry in entries}
        members = spec.get('archive_members', {name: name for name in spec['current']})
        require(len(names) == len(entries) and len(entries) <= 64 and set(members.values()) <= names,
                'unexpected or duplicate artifact entries')
        for entry in entries:
            require(0 < entry.file_size <= 512 * 1024 and not entry.is_dir()
                    and (entry.external_attr >> 16) & 0o170000 != 0o120000, 'invalid evidence member')
        archived = {name: archive.read(name) for name in names}
    for name in names - set(members.values()):
        require(spec['kind'] in ['module-reuse', 'php-memory-startup-failed', 'rpc-memory-passed', 'compiler-matrix'],
                'unexpected CGI artifact member')
        require(archived[name] == reader.original(spec['directory'] + '/evidence/' + name),
                'historical evidence changed in CI')
    raw = {name: archived[member] for name, member in members.items()}
    data = {name: language_check.parse_json(raw[name]) for name in spec['current']}
    stages = data['ci-stages.json']
    require(stages['source_commit'] == source and stages['run_id'] == run_id
            and stages['run_attempt'] == '1', 'wrong source or CI run')
    verify_sources(stages)
    if spec['kind'] == 'module-reuse':
        summary = verify_reuse(data)
    elif spec['kind'] == 'php-memory-startup-failed':
        summary = verify_memory_startup_failure(data)
    elif spec['kind'] == 'rpc-memory-passed':
        summary = verify_rpc_memory(data)
    elif spec['kind'] == 'compiler-matrix':
        summary = verify_compiler_matrix(data, spec['backend'])
    else:
        data['_raw_toolchain'] = raw['toolchain.json']
        if spec['kind'] == 'cgi-contract':
            data['_fixed_module_sha'] = spec['module_sha256']
            data['_raw_build'], data['_raw_native'] = raw['build.json'], raw['native-cgi-contract.json']
            summary = verify_cgi_contract(data)
        else:
            summary = verify_cgi(data, spec['kind'] == 'cgi-build-passed')
    destination = ROOT / spec['directory'] / 'evidence' / ('ci-' + args.run)
    destination.mkdir(parents=True, exist_ok=True)
    for name in spec['current']:
        (destination / name).write_bytes(raw[name])
    sources = [Path(__file__).resolve(), HERE / 'import_ci_evidence.py', HERE / 'language_check.py',
               HERE / 'reuse_check.py', HERE / 'check.py', HERE / 'cgi_check.py',
               HERE / 'memory_check.py', HERE / 'rpc_check.py', HERE / 'unit_check.py']
    report = {
        'format_version': 1, 'kind': 'source_verified_import_of_pinned_ci_checkpoint',
        'source_commit': source,
        'run_url': f'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/{run_id}',
        'artifact_id': spec['artifact'], 'artifact_zip_bytes': len(wire), 'artifact_zip_sha256': spec['sha256'],
        'original_git_source_hashes_verified': True,
        'ci_verdict': 'success' if spec['kind'] in ['cgi-build-passed', 'cgi-contract'] else 'failure',
        'php_executed_by_importer': False, 'product_runtime_selected': False,
        'native_or_wasm_binaries_acquired_by_importer': False,
        'input_files': {source.relative_to(ROOT).as_posix(): sha(source.read_bytes()) for source in sources},
        'imported_files': {name: sha(raw[name]) for name in spec['current']},
        'observations': summary,
    }
    (destination / 'provenance.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8', newline='\n')
    print('Imported ' + spec['kind'] + ' checkpoint; original CI verdict preserved.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'Checkpoint import failed: {error}', file=sys.stderr)
        sys.exit(1)
