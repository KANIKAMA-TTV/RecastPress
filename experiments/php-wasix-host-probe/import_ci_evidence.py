"""Import one pinned successful CI receipt, verifying its original Git sources.

The ZIP is obtained separately through GitHub's artifact API. No credential,
temporary download URL, PHP binary or LLVM binary is read or saved here.
"""
import hashlib
import json
from pathlib import Path, PurePosixPath
import subprocess
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
COMMIT = '5fd037742b27f0b3b7b9096f92ea58ef091a4f2e'
RUN = 41293870063
ARTIFACT = 13078243713
ZIP_SHA = '3ec26e180a51007a88fcb55e79cb73ef0e7c21a3c49e8d8330b3c6549791d65d'
ARCHIVE = ROOT / 'target/php-wasix-ci-evidence/run-41293870063-1.zip'
NAMES = {'ci-stages.json', 'linux-llvm.json', 'llvm-toolchain-verification.json', 'windows-cranelift.json'}


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def original(path):
    name = PurePosixPath(path)
    require(type(path) is str and path == name.as_posix() and not name.is_absolute()
            and '..' not in name.parts and '\\' not in path and ':' not in path
            and '\x00' not in path and path.startswith(('experiments/', 'scripts/', '.github/')),
            'unexpected receipt source path')
    return subprocess.check_output(['git', 'show', COMMIT + ':' + path], cwd=ROOT, timeout=10)


def main():
    require(len(sys.argv) == 1, 'this importer accepts no arguments')
    wire = ARCHIVE.read_bytes()
    require(len(wire) == 4064 and sha(wire) == ZIP_SHA, 'CI artifact ZIP identity changed')
    with zipfile.ZipFile(ARCHIVE) as archive:
        entries = archive.infolist()
        require(len(entries) == len(NAMES) and {entry.filename for entry in entries} == NAMES,
                'unexpected or duplicate artifact members')
        for entry in entries:
            require(0 < entry.file_size <= 65536 and not entry.is_dir()
                    and (entry.external_attr >> 16) & 0o170000 != 0o120000,
                    'invalid evidence member')
        raw = {name: archive.read(name) for name in NAMES}
    data = {name: json.loads(value) for name, value in raw.items()}
    stages = data['ci-stages.json']
    require(stages['source_commit'] == COMMIT and stages['run_id'] == str(RUN)
            and stages['run_attempt'] == '1', 'wrong CI run')
    require(stages['stages'] == dict.fromkeys(['dependencies', 'rust', 'php', 'cargo', 'llvm', 'probe'], 'success'),
            'CI did not complete every required stage')
    require(stages['product_runtime_selected'] is False and stages['wordpress_bootstrapped'] is False,
            'version-only scope changed')
    for name in ['ci-stages.json', 'linux-llvm.json', 'windows-cranelift.json']:
        for path, expected in data[name]['input_files'].items():
            require(sha(original(path)) == expected, 'original Git source mismatch: ' + path)
    require(raw['windows-cranelift.json'] == original('experiments/php-wasix-host-probe/evidence/windows-cranelift.json'),
            'the checked-out Windows receipt changed in CI')
    result = data['linux-llvm.json']
    require(result['outcome'] == 'version_only_passed' and result['exit_code'] == 0
            and result['backend'] == 'llvm' and result['platform']['system'] == 'Linux'
            and result['php_version_verified'] is True and result['production_compatible'] is False,
            'missing successful Linux version-only result')
    native = result['native_result']
    require(native['ok'] is True and native['phase'] == 'version_only_verified'
            and native['engine'] == 'wasmer-7.2.0/llvm'
            and native['php'] == {'php_version': '8.5.12', 'php_int_size': 8,
                                  'echo': 'recastpress-v0.5-wasix-version-only'}
            and native['process_pid'] == result['owned_process_pid'], 'invalid fixed PHP result')
    require(all(native[key] is False for key in ['exceptions_executed', 'wordpress_executed',
                                               'full_php_compatibility_verified', 'real_db']),
            'version-only result became wider compatibility')
    fixture = json.loads(original('experiments/php-runtime-candidate-probe/artifact.json'))
    require(native['artifact']['verified'] is True
            and native['artifact']['observed_sha256'] == fixture['wasm_sha256'], 'PHP artifact mismatch')
    reference = original('experiments/php-runtime-candidate-probe/evidence/native-contract.json')
    comparison = result['integer_width_comparison']
    require(comparison == {'native_bytes': 8, 'wasm_bytes': 8, 'equal': True,
                           'reference_sha256': sha(reference)}, 'native integer comparison changed')
    llvm = data['llvm-toolchain-verification.json']
    manifest = original('experiments/php-wasix-host-probe/llvm-toolchain.json')
    toolchain = json.loads(manifest)
    require(llvm['llvm_toolchain_manifest_sha256'] == sha(manifest)
            and llvm['archive_sha256'] == toolchain['archive_sha256']
            and llvm['archive_bytes'] == toolchain['archive_bytes']
            and llvm['llvm_config_version'] == '22.1.8', 'LLVM identity mismatch')
    destination = HERE / 'evidence' / ('linux-llvm-' + str(RUN))
    destination.mkdir(parents=True, exist_ok=True)
    for name in sorted(NAMES - {'windows-cranelift.json'}):
        (destination / name).write_bytes(raw[name])
    provenance = {
        'format_version': 1,
        'source_commit': COMMIT,
        'run_url': f'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/{RUN}',
        'artifact_id': ARTIFACT, 'artifact_zip_bytes': len(wire), 'artifact_zip_sha256': ZIP_SHA,
        'original_git_source_hashes_verified': True,
        'native_binary_acquired_by_importer': False,
        'product_runtime_selected': False,
        'input_files': {str(Path(__file__).resolve().relative_to(ROOT).as_posix()): sha(Path(__file__).read_bytes())},
        'imported_files': {name: sha(raw[name]) for name in sorted(NAMES - {'windows-cranelift.json'})},
    }
    (destination / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n',
                                                encoding='utf-8', newline='\n')
    print('Imported source-verified Linux LLVM PHP 8.5.12 version-only result; PHP_INT_SIZE=8.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'CI evidence import failed: {error}', file=sys.stderr)
        sys.exit(1)
