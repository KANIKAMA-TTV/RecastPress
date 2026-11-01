"""Record identity and ABI/EH structure of a fixed candidate, without executing PHP."""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import os
import platform
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

NAME = 'recastpress-php-runtime-candidate-probe'
TARGET = ROOT / 'target/php-runtime-candidate-probe'
PREFIX = b'PHP_RUNTIME_ARTIFACT_PROFILE='


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    require(sys.argv[1:] in ([], ['--record']), 'usage: python check.py [--record]')
    try:
        _, built = owned_probe.run_captured_probe([
            'cargo', 'build', '--manifest-path', str(HERE / 'Cargo.toml'), '--release',
            '--target-dir', str(TARGET), '--offline', '--locked', '--message-format=json',
        ], os.environ.copy(), timeout=600)
        require(built.returncode == 0 and len(built.stdout) <= 16 * 1024 * 1024, 'inspector build failed')
        artifacts = []
        finished = False
        for line in built.stdout.splitlines():
            item = json.loads(line)
            require(not finished, 'unexpected Cargo message after completion')
            if item.get('reason') == 'build-finished':
                require(item.get('success') is True, 'Cargo build did not succeed')
                finished = True
            if item.get('reason') == 'compiler-artifact' and item.get('target', {}).get('name') == NAME:
                require(item['target']['kind'] == ['bin'] and item['profile']['test'] is False, 'wrong inspector target')
                require(Path(item['manifest_path']).resolve() == HERE / 'Cargo.toml', 'wrong inspector manifest')
                executable = Path(item['executable']).resolve(strict=True)
                require(executable.is_relative_to(TARGET.resolve()), 'inspector outside target')
                artifacts.append(executable)
        require(finished and len(artifacts) == 1, 'ambiguous inspector executable')
        pid, result = owned_probe.run_captured_probe([str(artifacts[0])], os.environ.copy(), timeout=60)
        require(result.returncode == 0 and result.stderr == b'', 'artifact inspection failed')
        require(result.stdout.startswith(PREFIX) and result.stdout.endswith(b'\n')
                and result.stdout.count(b'\n') == 1 and len(result.stdout) <= 64 * 1024, 'invalid inspector framing')
        value = json.loads(result.stdout[len(PREFIX):])
        require(type(value['process_pid']) is int and value['process_pid'] == pid, 'wrong inspector process')
        identity = json.loads((HERE / 'artifact.json').read_text(encoding='utf-8'))
        require(value['artifact_sha256'] == identity['wasm_sha256'] and value['artifact_bytes'] == identity['wasm_bytes'], 'artifact identity mismatch')
        require(value['source_commit'] == identity['source_commit'] and value['workflow_run_id'] == identity['workflow_run_id'], 'upstream identity mismatch')
        require(value['legacy_exception_operators'] == 0 and value['exception_operators']['try_table'] == 8331
                and value['exception_operators']['throw_ref'] == 6176, 'exception format changed')
        require(value['wasix_imports'] is True and value['start_export'] is True and value['memory_export'] is False, 'unexpected ABI')
        require(value['memories'] == [{'import_module': 'env', 'import_name': 'memory', 'initial_pages': 403,
                                     'maximum_pages': 65536, 'shared': True, 'memory64': False}], 'unexpected memory ABI')
        require(value['php_executed'] is False and value['engine_validated'] is False and value['product_ready'] is False,
                'inspection became an execution claim')
    finally:
        owned_probe.reap_owned_probes()
    inputs = [HERE / name for name in ['Cargo.toml', 'Cargo.lock', 'artifact.json', 'prepare_fixture.py', 'check.py']]
    inputs += sorted((HERE / 'src').glob('*.rs')) + [ROOT / 'scripts/owned_probe.py']
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'scope': 'artifact identity and structure only; no engine or PHP execution',
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'input_files': {path.relative_to(ROOT).as_posix(): digest(path) for path in inputs},
        'inspector_artifact': artifacts[0].relative_to(ROOT).as_posix(),
        'inspector_artifact_sha256': digest(artifacts[0]), 'observation': value,
    }
    if sys.argv[1:] == ['--record']:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / 'artifact-profile.json').write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('Pinned PHP artifact inspected: standard EH, WASIX imports, shared imported memory; PHP not executed.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'Candidate inspection failed: {error}', file=sys.stderr)
        sys.exit(1)
