"""Import the fixed first Fiber configure failure, never a build/Fiber pass."""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import subprocess
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
sys.path.insert(0, str(HOST))
spec = importlib.util.spec_from_file_location('recastpress_fiber_original_git_reader', HOST / 'import_ci_evidence.py')
reader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reader)
from language_check import equal, parse_json

SOURCE = 'a05a2ca1e795e64dacbb2ee0845b6fdc9464da2a'
RUN = '41320223325'
SPECS = {
    'cranelift': (13089117018, 27366, '85ab104ae2d6f022fff7585f1d9da3311b13cdea1ff138c23f29c6a1d5ab7407'),
    'llvm': (13089580398, 27362, 'c1e8c620cb14003ee0182317e8069501e63797a34d8399ca7958ae190f4e8fb9'),
}
PASSED_SOURCE = '24c0b57ab583337cbe42fbda5f3daec6b9849658'
PASSED_RUN = '41320732582'
PASSED_SPECS = {
    'cranelift': (13089433987, 43208, '6cb13b790b4841659accd7524cb16339622825bc21b157bbb64ca00c59035c16'),
    'llvm': (13089890519, 43665, '622a3927c09e7a2c066ffe664ec83fa07fef5ccfa2a3de11e22855e52e5ba67f'),
}
MEMBERS = {name: 'php-wasix-fiber-probe/evidence/' + name for name in
           ['build.json', 'ci-stages.json', 'native-fiber-contract.json']}
MEMBERS['toolchain.json'] = 'php-wasix-cgi-build/evidence/toolchain.json'
MEMBERS['windows-cranelift-unit-contract.json'] = 'php-wasix-host-probe/evidence/windows-cranelift-unit-contract.json'


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def verify_sources(value):
    require = reader.require
    require(type(value['input_files']) is dict and value['input_files'], 'missing source bindings')
    for name, expected in value['input_files'].items():
        require(sha(reader.original(name)) == expected, 'original Git input differs: ' + name)


def verify(data, raw, backend):
    require = reader.require
    stages, build, tools = data['ci-stages.json'], data['build.json'], data['toolchain.json']
    for value in [stages, build, tools]:
        verify_sources(value)
    equal((stages['source_commit'], stages['run_id'], stages['run_attempt'], stages['backend']),
          (SOURCE, RUN, '1', backend), 'CI identity')
    equal(stages['stages'], dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies',
          'definitions', 'tools'], 'success') | {'build':'failure','rust':'skipped','host_llvm':'skipped',
          'units':'skipped','fiber_contract':'skipped'}, 'configure failure stages')
    equal(build['schema'], 'recastpress.php-wasix-fiber-build.v0.5', 'build schema')
    equal(build['variant'], 'nts-fresh-wasi-zend-fibers-v1', 'backend variant')
    equal((build['outcome'], build['phase']), ('fiber_build_failed', 'configure'), 'original failed phase')
    for key in ['guest_executed','php_guest_executed','patched_php_executed','product_runtime_selected',
                'native_runtime_fallback_used','redistributed_binaries','php_wasm_fiber_compatibility_verified',
                'postopt_module_inspected','sysroot_source_build_attestation_verified']:
        equal(build[key], False, 'unclaimed build gate ' + key)
    require('php_wasm' not in build and 'raw_wasm' not in build, 'configure failure became a Wasm artifact')
    require('C compiler cannot create executables' in build['diagnostics'], 'original configure diagnostic')
    equal(len(build['build_steps']), 1, 'no make/optimizer execution')
    step = build['build_steps'][0]
    for key, expected in [('name','configure'), ('command',['bash','wasix-configure-eh-64.sh']),
            ('working_directory','target/php-wasix-fiber-probe/source/php'), ('completed',True),
            ('exit_code',77), ('timeout_seconds',600), ('output_quota_bytes',16777216),
            ('live_output_quota_enforced',True), ('output_capture_complete',True), ('owned_leader_reaped',True)]:
        equal(step[key], expected, 'failed command ' + key)
    equal(build['tracked_sources_clean_before_build'], True, 'clean source before build')
    definition = parse_json(reader.original('experiments/php-wasix-fiber-probe/manifest.json'))
    plan = definition['source_patches'][0]
    patch = build['source_patch']
    equal({key:patch[key] for key in ['path','bytes','sha256']}, {key:plan[key] for key in ['path','bytes','sha256']}, 'patch pin')
    equal(sha(reader.original('experiments/php-wasix-fiber-probe/' + plan['path'])), plan['sha256'], 'original patch bytes')
    equal(patch['source_commit'], definition['upstream']['commit'], 'patched source commit')
    for key in ['clean_tracked_source_immediately_before','applied','git_apply_check_passed',
                'git_apply_passed','added_assembly_matches_fixed_fixture']:
        equal(patch[key], True, 'patch gate ' + key)
    equal(patch['index_updated'], False, 'no source index mutation')
    equal(patch['before'], {row['path']:row['before'] for row in plan['files']}, 'whole-file original identities')
    equal(patch['after'], {row['path']:row['after'] for row in plan['files']}, 'whole-file patched identities')
    tool_manifest = parse_json(reader.original('experiments/php-wasix-cgi-build/manifest.json'))
    equal(build['toolchain']['receipt_sha256'], sha(raw['toolchain.json']), 'same-job tool receipt')
    equal(tools['outcome'], 'verified_host_build_tools_prepared', 'verified host tools')
    equal(len(tools['assets']), 4, 'four tools')
    for asset in tool_manifest['assets']:
        row = next(row for row in tools['assets'] if row['name'] == asset['name'])
        equal((row['verified'],row['observed_bytes'],row['observed_sha256']),
              (True,asset['bytes'],asset['sha256']), 'tool archive pin')
    for key in ['php','deps','parser']:
        equal(build['sources'][key]['commit'], tool_manifest['sources'][key]['commit'], 'source checkout pin')
    equal(len(build['sources']['deps']['prebuilt_archive_files']), 28, '28 dependency archives inventoried')
    for name in ['native-fiber-contract.json','windows-cranelift-unit-contract.json']:
        directory = 'php-wasix-fiber-probe' if name.startswith('native-') else 'php-wasix-host-probe'
        equal(raw[name], reader.original('experiments/' + directory + '/evidence/' + name), 'unexecuted reference unchanged')
    return {'configure_exit_code':77,'patch_applied':True,'tool_archive_pins_verified':True,
        'php_make_executed':False,'wasm_module_generated':False,'php_guest_executed':False,
        'exact_linker_error_observed':False,'full_config_log_acquired':False,
        'scope':'compiler-check failure preserved; PHP-only exports were present, but first config.log was not captured'}


def original_build_inputs():
    result = subprocess.run(['git','ls-tree','-r','--name-only',SOURCE,'--','experiments/php-wasix-cgi-build'],
        cwd=ROOT,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=30,check=True)
    reader.require(len(result.stdout) <= 256 * 1024 and not result.stderr, 'original source listing budget')
    prefix = 'experiments/php-wasix-cgi-build/'
    names = {name for name in result.stdout.decode('utf-8').splitlines()
        if (name.startswith(prefix) and '/' not in name[len(prefix):] and name.endswith('.py'))
        or (name.startswith(prefix + 'patches/') and name.endswith('.patch'))}
    names.update([prefix+'manifest.json',prefix+'README.ja.md','.github/workflows/php-wasix-cgi-build.yml'])
    names.update('experiments/php-wasix-fiber-probe/' + name for name in [
        'build.py','patch_source.py','manifest.json','patches/0001-nts-fresh-wasi-zend-fibers.patch',
        'fixtures/zend_wasix_fiber.S','fixtures/fiber_contract.php'])
    return {name:sha(reader.original(name)) for name in sorted(names)}


def verify_passed(data, raw, backend):
    require = reader.require
    stages, build, tools = data['ci-stages.json'], data['build.json'], data['toolchain.json']
    probe = data['linux-' + backend + '-fiber-contract.json']
    unit = data['linux-' + backend + '-unit-contract.json']
    for value in [stages, build, tools, probe, unit]:
        verify_sources(value)
    equal((stages['source_commit'],stages['run_id'],stages['run_attempt'],stages['backend']),
        (SOURCE,RUN,'1',backend), 'passed CI identity')
    expected_stages = dict.fromkeys(['php_source','deps_source','parser_source','dependencies','definitions',
        'tools','build','rust','host_llvm','units','fiber_contract'],'success')
    expected_stages['host_llvm'] = 'success' if backend == 'llvm' else 'skipped'
    equal(stages['stages'],expected_stages,'all required Fiber stages')
    equal((build['schema'],build['outcome'],build['phase']),
        ('recastpress.php-wasix-fiber-build.v0.5','fiber_candidate_built_not_executed','build_complete'),'build completed')
    for key in ['guest_executed','php_guest_executed','patched_php_executed','product_runtime_selected',
        'native_runtime_fallback_used','redistributed_binaries','php_wasm_fiber_compatibility_verified',
        'postopt_module_inspected','sysroot_source_build_attestation_verified']:
        equal(build[key],False,'builder does not claim execution: '+key)
    expected_inputs = original_build_inputs()
    equal(build['input_files'],expected_inputs,'complete original builder input set')
    equal(build['toolchain']['receipt_sha256'],sha(raw['toolchain.json']),'same-job prepared tools')
    equal(tools['outcome'],'verified_host_build_tools_prepared','tool verification')
    tool_definition = parse_json(reader.original('experiments/php-wasix-cgi-build/manifest.json'))
    equal(len(tools['assets']),4,'four fixed tools')
    for asset in tool_definition['assets']:
        row = next(row for row in tools['assets'] if row['name'] == asset['name'])
        equal((row['verified'],row['observed_bytes'],row['observed_sha256']),
            (True,asset['bytes'],asset['sha256']),'tool identity')
    for key in ['php','deps','parser']:
        equal(build['sources'][key]['commit'],tool_definition['sources'][key]['commit'],'source HEAD pin')
    equal(build['tracked_sources_clean_before_build'],True,'clean source before patch')
    definition = parse_json(reader.original('experiments/php-wasix-fiber-probe/manifest.json'))
    plan, patch = definition['source_patches'][0], build['source_patch']
    equal(patch['before'],{row['path']:row['before'] for row in plan['files']},'original whole-file identities')
    equal(patch['after'],{row['path']:row['after'] for row in plan['files']},'patched whole-file identities')
    equal(patch['sha256'],plan['sha256'],'patch digest')
    for key in ['applied','git_apply_check_passed','git_apply_passed','clean_tracked_source_immediately_before',
        'added_assembly_matches_fixed_fixture']:
        equal(patch[key],True,'source patch '+key)
    equal(patch['index_updated'],False,'source index unchanged')
    equal(build['binaryen_validation_enabled'],True,'validation enabled')
    equal(build['configuration_policy']['backend_enabled'],True,'backend configured')
    equal(build['configuration_policy']['zts_enabled'],False,'NTS configured')
    equal(build['configuration_policy']['ucontext_enabled'],False,'separate backend')
    config = build['build_config']
    require('WASIXCC_LINKER_FLAGS' not in config['tool_environment_additions'], 'no PHP-only conftest exports')
    equal(config['extra_configure_flags'],['--enable-rp-wasix-fibers','--disable-zts'],'explicit opt-in flags')
    equal(config['tool_environment_additions']['WASIXCC_COMPILER_POST_FLAGS'],'-fno-lto','compiler post flags')
    equal(config['make_only_environment_additions']['WASIXCC_LINKER_FLAGS'],
        ':'.join(flag.removeprefix('-Wl,') for flag in definition['build_design']['required_link_flags']),'make-only exports')
    require('-O3' not in config['optimizer_flags'] and '--no-validation' not in config['optimizer_flags'], 'optimizer scope')
    equal([step['name'] for step in build['build_steps']],['configure','make_cli','optimize_cli'],'three build steps')
    equal(len(config['command_recipe']),3,'fixed command recipe')
    for step, recipe in zip(build['build_steps'],config['command_recipe']):
        for key in ['name','command','working_directory','timeout_seconds']:
            equal(step[key],recipe[key],'recorded fixed command '+key)
        for key in ['completed','owned_leader_reaped','live_output_quota_enforced','output_capture_complete']:
            equal(step[key],True,'completed owned command '+key)
        equal(step['exit_code'],0,'successful compiler command')
    equal([row['timeout_seconds'] for row in config['command_recipe']],[600,1200,300],'bounded command durations')
    require(0 < build['php_wasm']['bytes'] <= 128*1024*1024 and 0 < build['raw_wasm']['bytes'] <= 128*1024*1024,'module byte budgets')
    equal(build['raw_wasm']['sha256'],build['linked_php_output']['sha256'],'linked/raw copy identity')
    equal(probe['outcome'],'fixed_php_fiber_contract_passed','fixed PHP Fiber comparison')
    for key, expected in [('fiber_contract_passed',True),('owned_host_reaped',True),('exit_code',0),
        ('product_runtime_selected',False),('full_php_compatibility_verified',False),('native_runtime_fallback_used',False),
        ('all_resources_immediately_reclaimed',False),('memory_limit_verified',False),('cpu_limit_verified',False),
        ('stack_free_or_reuse_verified',False),('outer_execution_timeout_seconds',600)]:
        equal(probe[key],expected,'comparison scope '+key)
    provenance = probe['generated_provenance']
    equal(provenance['build_receipt_sha256'],sha(raw['build.json']),'exact generated build receipt')
    equal(provenance['module'],build['php_wasm'],'exact generated final module')
    equal(provenance['builder_provenance']['raw_module_sha256'],build['raw_wasm']['sha256'],'raw module provenance')
    fspec = importlib.util.spec_from_file_location('recastpress_fiber_frozen_validator',HERE/'fiber_check.py')
    checker = importlib.util.module_from_spec(fspec)
    fspec.loader.exec_module(checker)
    checker.validate_result(probe['native_result'],probe['owned_process_pid'],backend,provenance,
        read_source=reader.original,build_inputs=expected_inputs)
    equal((unit['outcome'],unit['tests_passed'],unit['owned_test_reaped'],unit['exit_code'],len(unit['listed_tests'])),
        ('fixed_p0_unit_tests_passed',True,True,0,27),'bounded Linux27 units')
    for name in ['native-fiber-contract.json','windows-cranelift-unit-contract.json']:
        directory = 'php-wasix-fiber-probe' if name.startswith('native-') else 'php-wasix-host-probe'
        equal(raw[name],reader.original('experiments/'+directory+'/evidence/'+name),'unchanged unexecuted reference')
    if backend == 'llvm':
        llvm = data['llvm-toolchain-verification.json']
        manifest_wire = reader.original('experiments/php-wasix-host-probe/llvm-toolchain.json')
        manifest = parse_json(manifest_wire)
        equal((llvm['llvm_toolchain_manifest_sha256'],llvm['archive_bytes'],llvm['archive_sha256'],llvm['llvm_config_version']),
            (sha(manifest_wire),manifest['archive_bytes'],manifest['archive_sha256'],'22.1.8'),'separate host LLVM')
    value = probe['native_result']
    return {'php_source_patch_applied':True,'php_module_bytes':build['php_wasm']['bytes'],
        'php_module_sha256':build['php_wasm']['sha256'],'postopt_adapter_verified':True,
        'bounded_linux_units_passed':27,'fixed_native_cases':6,'fresh_wasi_instances':3,'module_compile_calls':1,
        'compile_elapsed_ms':value['compile_elapsed_ms'],
        'guest_execution_elapsed_ms':[case['guest_execution_elapsed_ms'] for case in value['cases']],
        'full_php_compatibility_verified':False,'physical_stack_free_or_reuse_verified':False,
        'all_resources_immediately_reclaimed':False,'product_runtime_selected':False}


def main():
    global SOURCE, RUN
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=sorted(SPECS), required=True)
    parser.add_argument('--passed', action='store_true', help='import the later fixed-corpus success')
    args = parser.parse_args()
    if args.passed:
        SOURCE, RUN = PASSED_SOURCE, PASSED_RUN
    artifact, size, expected = (PASSED_SPECS if args.passed else SPECS)[args.backend]
    members = MEMBERS.copy()
    if args.passed:
        members['linux-'+args.backend+'-fiber-contract.json'] = 'php-wasix-fiber-probe/evidence/linux-'+args.backend+'-fiber-contract.json'
        members['linux-'+args.backend+'-unit-contract.json'] = 'php-wasix-host-probe/evidence/linux-'+args.backend+'-unit-contract.json'
        if args.backend == 'llvm':
            members['llvm-toolchain-verification.json'] = 'php-wasix-host-probe/evidence/llvm-toolchain-verification.json'
    reader.COMMIT = SOURCE
    path = ROOT / ('target/php-wasix-ci-evidence/run-' + RUN + '-' + args.backend + '-1.zip')
    wire = path.read_bytes()
    reader.require(len(wire) == size and sha(wire) == expected, 'fixed artifact ZIP identity')
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        reader.require(len(entries) == len({e.filename for e in entries}) == len(members)
            and {e.filename for e in entries} == set(members.values()), 'fixed member set')
        for entry in entries:
            reader.require(0 < entry.file_size <= 512 * 1024 and not entry.is_dir()
                and (entry.external_attr >> 16) & 0o170000 != 0o120000, 'evidence member type/size')
        raw = {name:archive.read(member) for name, member in members.items()}
    data = {name:parse_json(value) for name,value in raw.items()}
    observations = (verify_passed if args.passed else verify)(data, raw, args.backend)
    destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
    destination.mkdir(parents=True, exist_ok=True)
    for name,value in raw.items():
        (destination / name).write_bytes(value)
    inputs = [Path(__file__).resolve(), HOST / 'import_ci_evidence.py', HOST / 'language_check.py']
    if args.passed:
        inputs += [HERE/'fiber_check.py',HERE/'native_contract.py',HERE/'build.py',HERE/'patch_source.py',
                   ROOT/'experiments/php-wasix-cgi-build/prepare.py',HOST/'check.py',
                   ROOT/'experiments/php-runtime-candidate-probe/native_contract.py',ROOT/'scripts/owned_probe.py']
    provenance = {'format_version':1,'kind':'source_verified_import_of_fixed_fiber_corpus' if args.passed else 'source_verified_import_of_first_fiber_configure_failure',
        'source_commit':SOURCE,'run_url':'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
        'artifact_id':artifact,'artifact_zip_bytes':size,'artifact_zip_sha256':expected,
        'original_git_source_hashes_verified':True,'ci_verdict':'success' if args.passed else 'failure','observations':observations,
        'php_executed_by_importer':False,'native_or_wasm_binaries_acquired_by_importer':False,
        'product_runtime_selected':False,'input_files':{p.relative_to(ROOT).as_posix():sha(p.read_bytes()) for p in inputs},
        'imported_files':{name:sha(value) for name,value in raw.items()}}
    (destination / 'provenance.json').write_text(json.dumps(provenance,indent=2) + '\n',encoding='utf-8',newline='\n')
    print('Imported Fiber evidence; original '+('fixed-corpus success' if args.passed else 'configure failure')+' preserved.')


if __name__ == '__main__':
    main()
