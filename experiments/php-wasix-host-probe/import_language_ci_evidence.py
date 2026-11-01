"""Verify and import one failed language run, then compare captured cases.

Case comparison is offline analysis of the original bytes, not another PHP run
or a change to the failed CI verdict. The reference still requires all 3 cases.
"""
import json
from pathlib import Path
import sys
import zipfile

import import_ci_evidence as reader
import language_check as contract

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SOURCE = '64c004fd90f8c9bbb3afa9991fe0c55387e2e69c'
RUN = 41295645988
ARTIFACT = 13080055217
ZIP_SHA = '5fc96a733785993d8124b1a001e27162c80f7cea2c041f80eee4e9cdc211b1bc'
ARCHIVE = ROOT / 'target/php-wasix-ci-evidence/run-41295645988-1.zip'
CURRENT = {'ci-stages.json', 'linux-llvm.json', 'linux-llvm-language-contract.json',
           'llvm-toolchain-verification.json'}
OLD = {f'linux-llvm-41293870063/{name}' for name in
       ['ci-stages.json', 'linux-llvm.json', 'llvm-toolchain-verification.json', 'provenance.json']}
NAMES = CURRENT | OLD | {'windows-cranelift.json'}
require = reader.require
sha = reader.sha


def main():
    require(len(sys.argv) == 1, 'this importer accepts no arguments')
    # The shared reader checks canonical repository paths before git show.
    # It is used only for this pinned original execution tree in this process.
    reader.COMMIT = SOURCE
    wire = ARCHIVE.read_bytes()
    require(len(wire) == 12046 and sha(wire) == ZIP_SHA, 'CI ZIP identity changed')
    with zipfile.ZipFile(ARCHIVE) as archive:
        entries = archive.infolist()
        require(len(entries) == len(NAMES) and {entry.filename for entry in entries} == NAMES,
                'unexpected or duplicate artifact entries')
        for entry in entries:
            require(0 < entry.file_size <= 256 * 1024 and not entry.is_dir()
                    and (entry.external_attr >> 16) & 0o170000 != 0o120000,
                    'invalid evidence member')
        raw = {name: archive.read(name) for name in NAMES}
    for name in OLD | {'windows-cranelift.json'}:
        require(raw[name] == reader.original('experiments/php-wasix-host-probe/evidence/' + name),
                'historical receipt changed in CI')
    data = {name: contract.parse_json(raw[name]) for name in CURRENT}
    stages = data['ci-stages.json']
    require(stages['source_commit'] == SOURCE and stages['run_id'] == str(RUN)
            and stages['run_attempt'] == '1', 'wrong CI source or run')
    expected = dict.fromkeys(['dependencies', 'rust', 'php', 'cargo', 'llvm', 'fixture_fs', 'probe'], 'success')
    expected['language'] = 'failure'
    require(stages['stages'] == expected, 'unexpected CI stage results')
    for name in ['ci-stages.json', 'linux-llvm.json', 'linux-llvm-language-contract.json']:
        for path, expected in data[name]['input_files'].items():
            require(sha(reader.original(path)) == expected, 'original source mismatch: ' + path)
    native_wire = reader.original('experiments/php-runtime-candidate-probe/evidence/native-contract.json')
    native = contract.parse_json(native_wire)
    for path, expected in native['input_files'].items():
        require(sha(reader.original(path)) == expected, 'native source mismatch: ' + path)
    version = data['linux-llvm.json']
    require(version['outcome'] == 'version_only_passed' and version['php_version_verified'] is True
            and version['exit_code'] == 0 and version['production_compatible'] is False,
            'version-only result changed')
    language = data['linux-llvm-language-contract.json']
    require(language['comparison_passed'] is False and language['outcome'] == 'execution_failed'
            and language['exit_code'] == 1 and language['production_compatible'] is False,
            'failed language comparison became success')
    require(language['native_reference']['receipt_sha256'] == sha(native_wire), 'wrong native reference')
    result = language['native_result']
    require(result['process_pid'] == language['owned_process_pid']
            and result['schema'] == 'recastpress.php-wasix-language-contract.v0.5'
            and result['engine'] == 'wasmer-7.2.0/llvm'
            and result['phase'] == 'language_observation_verification'
            and result['ok'] is False and result['fixture']['verified'] is True,
            'invalid language observation source')
    fixture = json.loads(reader.original('experiments/php-runtime-candidate-probe/artifact.json'))
    require(result['artifact']['observed_sha256'] == fixture['wasm_sha256']
            and result['artifact']['verified'] is True, 'wrong PHP Wasm artifact')
    require(result['fixture']['observed_sha256'] == contract.FIXTURE_SHA256, 'wrong fixed PHP source')
    observed = language['observation']
    contract.equal(observed, result['php'], 'captured observation')
    contract.equal(observed, contract.parse_json(result['stdout'].encode('utf-8')), 'original PHP stdout')
    contract.equal(observed['runtime'], native['observation']['runtime'], 'runtime identity')
    contract.equal(observed['format_version'], 1, 'fixture format')
    contract.equal(observed['unexpected_output_hex'], '', 'unexpected output')
    cases = {}
    for name in contract.CASES:
        try:
            contract.equal(observed['cases'][name], native['observation']['cases'][name], name)
        except ValueError as error:
            cases[name] = {'equal_to_native': False, 'difference': str(error)}
        else:
            cases[name] = {'equal_to_native': True}
    require(cases['basic']['equal_to_native'] and cases['exceptions']['equal_to_native']
            and cases['fiber']['equal_to_native'] is False, 'recorded case observations changed')
    require(observed['cases']['fiber']['error'] == {
        'class': 'FiberError', 'message': 'Cannot resume a fiber that is not suspended'},
        'unexpected Fiber failure')
    destination = HERE / 'evidence' / ('linux-llvm-' + str(RUN))
    destination.mkdir(parents=True, exist_ok=True)
    for name in sorted(CURRENT):
        (destination / name).write_bytes(raw[name])
    inputs = [Path(__file__).resolve(), HERE / 'import_ci_evidence.py', HERE / 'language_check.py', HERE / 'check.py']
    analysis = {
        'format_version': 1, 'kind': 'offline_comparison_of_captured_php_observations',
        'source_commit': SOURCE,
        'run_url': f'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/{RUN}',
        'artifact_id': ARTIFACT, 'artifact_zip_bytes': len(wire), 'artifact_zip_sha256': ZIP_SHA,
        'original_git_source_hashes_verified': True,
        'input_files': {path.relative_to(ROOT).as_posix(): sha(path.read_bytes()) for path in inputs},
        'imported_files': {name: sha(raw[name]) for name in sorted(CURRENT)},
        'native_reference_sha256': sha(native_wire), 'case_results': cases,
        'all_three_cases_equal': False, 'ci_verdict': 'failure',
        'php_executed_by_this_analysis': False, 'wordpress_executed': False,
        'product_runtime_selected': False,
        'timing_observed_in_ci_ms': {key: result[key] for key in ['compile_elapsed_ms', 'php_execution_elapsed_ms']},
        'timing_scope': 'cold module compile versus run_wasm startup/execution/shutdown; one CI runner, not a product benchmark',
    }
    (destination / 'case-comparison.json').write_text(json.dumps(analysis, indent=2) + '\n',
                                                     encoding='utf-8', newline='\n')
    print('Imported failed CI unchanged: basic/exception observations equal native; Fiber differs; overall verdict remains failure.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'Language CI evidence import failed: {error}', file=sys.stderr)
        sys.exit(1)
