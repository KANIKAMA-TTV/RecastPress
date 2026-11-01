"""Import pinned P0 checkpoint ZIPs without changing their execution verdicts."""
import argparse
import json
from pathlib import Path
import sys
import zipfile

import import_ci_evidence as reader
import language_check
import reuse_check

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = '071c9443ba56b2b5dd27032567ed3e7a9c8d0045'
SPECS = {
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


def verify_reuse(data):
    stages = data['ci-stages.json']['stages']
    expected = dict.fromkeys(['dependencies', 'rust', 'php', 'cargo', 'llvm', 'fixture_fs', 'probe', 'reuse'], 'success')
    expected['language'] = 'failure'
    require(stages == expected, 'unexpected reuse CI stage outcomes')
    for name in ['linux-llvm.json', 'linux-llvm-module-reuse.json', 'linux-llvm-language-contract.json']:
        verify_sources(data[name])
    version = data['linux-llvm.json']
    reuse = data['linux-llvm-module-reuse.json']
    language = data['linux-llvm-language-contract.json']
    require(version['outcome'] == 'version_only_passed' and version['php_version_verified'] is True,
            'version result changed')
    require(reuse['outcome'] == 'module_reuse_passed' and reuse['reuse_passed'] is True
            and reuse['exit_code'] == 0 and reuse['production_compatible'] is False,
            'missing successful module reuse result')
    reuse_check.equal(reuse['observed_phases'], reuse_check.PHASES, 'observed reuse phases')
    reuse_check.validate_result(reuse['native_result'], reuse['owned_process_pid'], 'llvm')
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


def verify_cgi(data):
    expected = dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions', 'tools'], 'success')
    expected['build'] = 'failure'
    require(data['ci-stages.json']['stages'] == expected, 'unexpected CGI CI stage outcomes')
    build, tools = data['build.json'], data['toolchain.json']
    verify_sources(build)
    verify_sources(tools)
    require(build['outcome'] == 'build_failed' and build['phase'] == 'make_cgi'
            and build['guest_executed'] is False and build['product_runtime_selected'] is False
            and build['module_validation_performed'] is False and 'cgi_wasm' not in build,
            'failed CGI build became a module or execution claim')
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
    require(build['configure_difference'] == {'extra_flags': '--enable-cgi', 'php_source_patch': False,
                                             'cli_disabled': False, 'make_target': 'cgi', 'make_jobs': 2},
            'initial build source/configure difference changed')
    require([(step['name'], step['exit_code']) for step in build['build_steps']] == [('configure', 0), ('make_cgi', 2)],
            'initial build failure phase changed')
    return {'tool_preparation_passed': True, 'configure_passed': True, 'cgi_build_passed': False,
            'cgi_guest_executed': False, 'failure_phase': 'make_cgi',
            'failure_diagnostics': build['diagnostics']}


def raw_toolchain(data):
    return data['_raw_toolchain']


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--run', required=True, choices=sorted(SPECS))
    args = parser.parse_args()
    spec = SPECS[args.run]
    reader.COMMIT = SOURCE
    path = ROOT / 'target/php-wasix-ci-evidence' / ('run-' + args.run + '-1.zip')
    wire = path.read_bytes()
    require(len(wire) == spec['bytes'] and sha(wire) == spec['sha256'], 'CI ZIP identity changed')
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        names = {entry.filename for entry in entries}
        require(len(names) == len(entries) and len(entries) <= 64 and set(spec['current']) <= names,
                'unexpected or duplicate artifact entries')
        for entry in entries:
            require(0 < entry.file_size <= 512 * 1024 and not entry.is_dir()
                    and (entry.external_attr >> 16) & 0o170000 != 0o120000, 'invalid evidence member')
        raw = {name: archive.read(name) for name in names}
    for name in names - set(spec['current']):
        require(spec['kind'] == 'module-reuse', 'unexpected CGI artifact member')
        require(raw[name] == reader.original(spec['directory'] + '/evidence/' + name),
                'historical evidence changed in CI')
    data = {name: language_check.parse_json(raw[name]) for name in spec['current']}
    stages = data['ci-stages.json']
    require(stages['source_commit'] == SOURCE and stages['run_id'] == args.run
            and stages['run_attempt'] == '1', 'wrong source or CI run')
    verify_sources(stages)
    if spec['kind'] == 'module-reuse':
        summary = verify_reuse(data)
    else:
        data['_raw_toolchain'] = raw['toolchain.json']
        summary = verify_cgi(data)
    destination = ROOT / spec['directory'] / 'evidence' / ('ci-' + args.run)
    destination.mkdir(parents=True, exist_ok=True)
    for name in spec['current']:
        (destination / name).write_bytes(raw[name])
    sources = [Path(__file__).resolve(), HERE / 'import_ci_evidence.py', HERE / 'language_check.py',
               HERE / 'reuse_check.py', HERE / 'check.py']
    report = {
        'format_version': 1, 'kind': 'source_verified_import_of_pinned_ci_checkpoint',
        'source_commit': SOURCE,
        'run_url': f'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/{args.run}',
        'artifact_id': spec['artifact'], 'artifact_zip_bytes': len(wire), 'artifact_zip_sha256': spec['sha256'],
        'original_git_source_hashes_verified': True, 'ci_verdict': 'failure',
        'php_executed_by_importer': False, 'product_runtime_selected': False,
        'native_or_wasm_binaries_acquired_by_importer': False,
        'input_files': {source.relative_to(ROOT).as_posix(): sha(source.read_bytes()) for source in sources},
        'imported_files': {name: sha(raw[name]) for name in spec['current']},
        'observations': summary,
    }
    (destination / 'provenance.json').write_text(json.dumps(report, indent=2) + '\n', encoding='utf-8', newline='\n')
    print('Imported ' + spec['kind'] + ' checkpoint; original failure verdict preserved.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'Checkpoint import failed: {error}', file=sys.stderr)
        sys.exit(1)
