"""Import the pinned raw-context stack-corruption observations, never a pass."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
HOST = ROOT / 'experiments/php-wasix-host-probe'
sys.path.insert(0, str(HOST))
import import_checkpoint as checkpoint
from language_check import equal, parse_json

reader = checkpoint.reader
SOURCE = 'a0cbadeb83916ad4896c0026235990a18fb40099'
RUN = '41310854007'
MODULE_SHA = 'f9591248d09700b7f67d9cd44d39fa5881b7447cacdd4aa153f58accad648ad3'
SPECS = {
    'llvm': (13084639405, 11665, 'a853f76dfdc7ac8132a1df25184128fa1607a464235c11b48384e0c26b19b7ce'),
    'cranelift': (13084664564, 11199, '1a99bb3a87308990402189f4e9f723c9faef8384cb2112162fee4f7e7c835507'),
}
require, sha = reader.require, reader.sha


def verify(data, raw, backend):
    stages, build, tools, host = [data[name] for name in
        ['ci-stages.json', 'build.json', 'toolchain.json', 'context-contract.json']]
    equal(stages['source_commit'], SOURCE, 'original commit')
    equal(stages['run_id'], RUN, 'original run')
    equal(stages['run_attempt'], '1', 'run attempt')
    equal(stages['backend'], backend, 'backend')
    expected = dict.fromkeys(['dependencies', 'definitions', 'guest_tools', 'build', 'rust'], 'success')
    expected.update(host_llvm='success' if backend == 'llvm' else 'skipped', contract='failure')
    equal(stages['stages'], expected, 'original failed context job')
    for report in [stages, build, tools, host]:
        checkpoint.verify_sources(report)
    equal(build['outcome'], 'context_candidate_built_not_executed', 'C build outcome')
    equal(build['guest_executed'], False, 'builder does not execute guest')
    equal(build['binaryen_validation_enabled'], True, 'optimizer validation enabled')
    manifest = parse_json(reader.original('experiments/php-wasix-context-probe/manifest.json'))
    commands = [
        ['target/php-wasix-cgi-build/tools/bin/wasixcc', *manifest['compile_flags'],
         manifest['fixture'], '-o', manifest['raw_output']],
        ['target/php-wasix-cgi-build/tools/binaryen/bin/wasm-opt', manifest['raw_output'],
         *manifest['optimizer_flags'], '-o', manifest['wasm_output']],
    ]
    equal([s['command'] for s in build['build_steps']], commands, 'original fixed build flags')
    equal([(s['name'], s['exit_code']) for s in build['build_steps']],
          [('compile_c', 0), ('optimize_context', 0)], 'completed build commands')
    for step in build['build_steps']:
        equal(step['completed'], True, 'build step completion')
        equal(step['timeout_seconds'], 120, 'build step deadline')
        equal(step['process_group_timeout_cleanup'], False, 'no build timeout')
    equal(build['toolchain']['receipt_sha256'], sha(raw['toolchain.json']), 'same tool receipt')
    tool_manifest = parse_json(reader.original('experiments/php-wasix-cgi-build/manifest.json'))
    for asset in tool_manifest['assets']:
        observed = next(row for row in tools['assets'] if row['name'] == asset['name'])
        equal(observed['verified'], True, 'verified tool asset')
        equal(observed['observed_bytes'], asset['bytes'], 'tool asset bytes')
        equal(observed['observed_sha256'], asset['sha256'], 'tool asset hash')
    if backend == 'llvm':
        llvm = data['llvm-toolchain-verification.json']
        llvm_wire = reader.original('experiments/php-wasix-host-probe/llvm-toolchain.json')
        llvm_manifest = parse_json(llvm_wire)
        equal(llvm['llvm_toolchain_manifest_sha256'], sha(llvm_wire), 'host LLVM manifest')
        equal(llvm['archive_bytes'], llvm_manifest['archive_bytes'], 'host LLVM archive bytes')
        equal(llvm['archive_sha256'], llvm_manifest['archive_sha256'], 'host LLVM archive hash')
        equal(llvm['llvm_config_version'], '22.1.8', 'host LLVM version')
        equal(llvm['verifier_source_sha256'], sha(reader.original(
            'experiments/php-wasix-host-probe/prepare_host_llvm.py')), 'host LLVM verifier')
    equal(build['context_wasm'], {'path': 'target/php-wasix-context-probe/context.wasm',
          'bytes': 46414, 'sha256': MODULE_SHA, 'wasm_header_verified': True, 'executed': False},
          'exact same compiled C module')
    equal(host['outcome'], 'execution_failed', 'host diagnostic remains failed')
    equal(host['context_contract_passed'], False, 'no context compatibility pass')
    equal(host['exit_code'], 1, 'failed owned host')
    equal(host['owned_host_reaped'], True, 'owned host reaped')
    for flag in ['php_executed', 'php_fiber_compatibility_verified', 'product_runtime_selected',
                 'memory_limit_verified', 'cpu_limit_verified', 'all_resources_immediately_reclaimed',
                 'sysroot_source_build_attestation_verified']:
        equal(host[flag], False, 'unverified scope ' + flag)
    value = host['native_result']
    equal(value['schema'], 'recastpress.wasix-context-host.v0.5', 'host schema')
    equal(value['engine'], 'wasmer-7.2.0/' + backend, 'actual backend')
    equal(value['process_pid'], host['owned_process_pid'], 'owned process identity')
    for name, expected_value in [('ok', False), ('guest_completed', False), ('context_contract_verified', False),
                                 ('guest_json_decoded', True), ('engine_supports_async', True),
                                 ('module_compile_invocations', 1), ('stdout_eof', True), ('stderr_eof', True),
                                 ('stdout_overflow', False), ('stderr_overflow', False),
                                 ('runtime_shutdown_returned', True), ('stderr_bytes', 0), ('stderr_hex', '')]:
        equal(value[name], expected_value, 'host observation ' + name)
    equal(value['phase'], 'context_execution', 'execution was reached')
    require('ExitCode::1' in value['host_execution_error'], 'not the guest failure exit')
    equal(value['artifact']['verified'], True, 'module identity verified')
    equal(value['artifact']['observed_sha256'], MODULE_SHA, 'module executed by both backends')
    equal(value['artifact']['observed_bytes'], 46414, 'executed module size')
    provenance = host['generated_provenance']
    equal(provenance['build_receipt_sha256'], sha(raw['build.json']), 'host/build receipt binding')
    equal(provenance['manifest_sha256'], sha(reader.original('experiments/php-wasix-context-probe/manifest.json')),
          'original build manifest')
    equal(provenance['source_sha256'], sha(reader.original('experiments/php-wasix-context-probe/fixtures/context_contract.c')),
          'original C source')
    equal(value['build_receipt']['observed_sha256'], sha(raw['build.json']), 'Rust receipt binding')
    equal(provenance['module_sha256'], MODULE_SHA, 'checker module identity')
    isolation = value['isolation']
    equal(isolation['asynchronous_threading'], True, 'async Runner enabled')
    equal(isolation['configured_guest_max_threads'], 1, 'configured thread limit')
    for name in ['host_environment_forwarded', 'home_mapped', 'host_directories_mapped', 'http_client']:
        equal(isolation[name], False, 'host capability ' + name)
    equal(isolation['networking'], 'UnsupportedVirtualNetworking', 'no network')
    wire = bytes.fromhex(value['stdout_hex'])
    equal(len(wire), value['stdout_bytes'], 'raw stdout length')
    equal(hashlib.sha256(wire).hexdigest(), value['stdout_sha256'], 'raw stdout hash')
    guest = parse_json(wire)
    equal(guest, value['guest_json'], 'raw guest JSON preserved')
    equal(guest['schema'], 'recastpress.wasix-context-c.v0.5', 'C fixture schema')
    for name, expected_value in [('ok', False), ('phase', 'post_a_unwind_clobber'),
                                 ('failure', 'volatile-stack-canary-changed'), ('a_done', True), ('b_done', False),
                                 ('a_rounds', 8), ('b_rounds', 8), ('a_unwound_frames', 4), ('b_unwound_frames', 0),
                                 ('range_count', 9), ('range_observation', 'initial-simultaneous-live-snapshot')]:
        equal(guest[name], expected_value, 'actual lifetime failure ' + name)
    equal(guest['abi'], {'context_id_bytes': 8, 'pointer_bytes': 4, 'einval': 28}, 'C ABI')
    equal(guest['trace'], [10, 20] + [v for i in range(8) for v in (100 + i, 200 + i)] + [40],
          'eight completed alternating rounds and A unwind')
    equal(guest['checks']['ranges_disjoint'], True, 'initial simultaneous ranges were disjoint')
    equal(guest['checks']['a_recursion_unwound'], True, 'A normally unwound')
    equal(guest['checks']['canaries_preserved'], False, 'stack corruption must remain a failure')
    equal(guest['checks']['b_preserved_after_a_unwind'], False, 'B preservation was not reached')
    ranges = sorted(guest['ranges'], key=lambda r: r['begin'])
    require(len(ranges) == 9 and all(type(r['begin']) is int and r['end'] - r['begin'] == 256 for r in ranges)
            and all(a['end'] <= b['begin'] for a, b in zip(ranges, ranges[1:])), 'invalid initial stack ranges')
    return {'raw_context_switching_completed_rounds': 8, 'initial_ranges_disjoint': True,
            'a_normally_unwound_frames': 4, 'b_frames_still_suspended': 4,
            'failure_phase': guest['phase'], 'failure': guest['failure'],
            'raw_sdk_api_provides_stack_independence_for_fixture': False,
            'php_fiber_compatibility_verified': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', required=True, choices=sorted(SPECS))
    args = parser.parse_args()
    artifact, size, expected_sha = SPECS[args.backend]
    reader.COMMIT = SOURCE
    path = ROOT / 'target/php-wasix-ci-evidence' / f'run-{RUN}-{args.backend}-1.zip'
    wire = path.read_bytes()
    require(len(wire) == size and sha(wire) == expected_sha, 'pinned context ZIP changed')
    members = {
        'ci-stages.json': 'php-wasix-context-probe/evidence/ci-stages.json',
        'build.json': 'php-wasix-context-probe/evidence/build.json',
        'toolchain.json': 'php-wasix-cgi-build/evidence/toolchain.json',
        'context-contract.json': f'php-wasix-host-probe/evidence/linux-{args.backend}-context-contract.json',
    }
    if args.backend == 'llvm':
        members['llvm-toolchain-verification.json'] = 'php-wasix-host-probe/evidence/llvm-toolchain-verification.json'
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        require(len(entries) == len(members) and {e.filename for e in entries} == set(members.values()),
                'unexpected or duplicate context artifact members')
        require(all(0 < e.file_size <= 256 * 1024 and not e.is_dir()
                    and (e.external_attr >> 16) & 0o170000 != 0o120000 for e in entries), 'context member type/size')
        raw = {name: archive.read(member) for name, member in members.items()}
    data = {name: parse_json(value) for name, value in raw.items()}
    summary = verify(data, raw, args.backend)
    destination = HERE / 'evidence' / f'ci-{RUN}-{args.backend}'
    destination.mkdir(parents=True, exist_ok=True)
    for name, value in raw.items():
        (destination / name).write_bytes(value)
    inputs = [Path(__file__).resolve(), HOST / 'import_checkpoint.py', HOST / 'import_ci_evidence.py',
              HOST / 'language_check.py']
    report = {'format_version': 1, 'source_commit': SOURCE, 'backend': args.backend,
              'run_url': f'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/{RUN}',
              'artifact_id': artifact, 'artifact_zip_bytes': size, 'artifact_zip_sha256': expected_sha,
              'ci_job_verdict': 'failure', 'original_git_source_hashes_verified': True,
              'php_or_guest_executed_by_importer': False, 'module_binaries_acquired_by_importer': False,
              'product_runtime_selected': False, 'imported_files': {name: sha(value) for name, value in raw.items()},
              'input_files': {p.relative_to(ROOT).as_posix(): sha(p.read_bytes()) for p in inputs},
              'observations': summary}
    (destination / 'provenance.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8', newline='\n')
    print('Imported raw-context stack corruption; original failure preserved: ' + args.backend)


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print('Context import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
