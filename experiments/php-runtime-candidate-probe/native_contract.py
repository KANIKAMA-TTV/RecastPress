"""Observe pinned native Windows PHP as a reference, never as a Wasm fallback.

Runs only the adjacent fixed PHP fixture. Capture size is checked after the
owned child exits; the helper does not impose an OS process memory limit.
"""
from datetime import datetime, timezone
from pathlib import Path, PurePosixPath
import base64
import hashlib
import json
import os
import platform
import stat
import sys
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

PHP_ROOT = ROOT / '.reference' / 'php-8.5.12'
WP_ROOT = ROOT / '.reference' / 'wordpress-7.1.3' / 'wp-includes'
PHP_ARCHIVE_SHA256 = '2ab242af38d76bdf417cde5ec048b1bcd76a5a40f39a3d15d5f046f38571c477'
WP_MANIFEST = ROOT / 'experiments/php-wasi-host-probe/wordpress-fixture.json'
FIXTURE = HERE / 'fixtures' / 'runtime_contract.php'
ORDINARY = b'ordinary\x00\xff\r\n'
READONLY = b'readonly fixture\n'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def json_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def parse_json(wire):
    def bad_number(_):
        raise ValueError('non-finite JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=json_object, parse_constant=bad_number)


def verify_references():
    manifest_path = PHP_ROOT / 'recastpress-tool.json'
    manifest = parse_json(manifest_path.read_bytes())
    require(manifest['php_version'] == '8.5.12' and manifest['archive_sha256'] == PHP_ARCHIVE_SHA256,
            'native PHP distribution identity changed')
    verified = {}
    for row in manifest['files']:
        name = row['path']
        relative = PurePosixPath(name)
        require(not relative.is_absolute() and relative.parts and '\\' not in name
                and all(part not in ('', '.', '..') and ':' not in part for part in name.split('/')),
                'unsafe native PHP manifest path')
        path = PHP_ROOT.joinpath(*relative.parts)
        require(path.resolve().is_relative_to(PHP_ROOT.resolve()) and path.is_file() and not path.is_symlink(),
                'native PHP file outside pinned directory')
        require(name.casefold() not in {existing.casefold() for existing in verified}, 'duplicate native PHP manifest path')
        require(digest(path) == row['sha256'], 'native PHP distribution file changed: ' + name)
        verified[name] = row['sha256']
    require({'php.exe', 'php8.dll'} <= set(verified), 'native PHP executable or core DLL is not verified')
    wordpress = parse_json(WP_MANIFEST.read_bytes())
    require(wordpress['wordpress_version'] == '7.1.3', 'WordPress reference version changed')
    require(set(wordpress['files']) == {'plugin.php', 'class-wp-hook.php', 'class-wp-filter-sentinel.php'},
            'unexpected WordPress reference files')
    for name, expected in wordpress['files'].items():
        path = WP_ROOT / name
        require(path.resolve().is_relative_to(WP_ROOT.resolve()) and path.is_file() and not path.is_symlink(),
                'WordPress reference path changed')
        require(path.stat().st_size == expected['bytes'] and digest(path) == expected['sha256'],
                'WordPress reference source changed: ' + name)
    return {
        'manifest': manifest_path.relative_to(ROOT).as_posix(), 'manifest_sha256': digest(manifest_path),
        'version': manifest['php_version'], 'archive_sha256': PHP_ARCHIVE_SHA256, 'verified_files': verified,
    }, wordpress


def equal(actual, expected, label):
    require(type(actual) is type(expected), 'native contract type mismatch: ' + label)
    if isinstance(expected, dict):
        require(set(actual) == set(expected), 'native contract fields mismatch: ' + label)
        for key, value in expected.items():
            equal(actual[key], value, label + '.' + key)
    elif isinstance(expected, list):
        require(len(actual) == len(expected), 'native contract list length mismatch: ' + label)
        for index, value in enumerate(expected):
            equal(actual[index], value, label + '[' + str(index) + ']')
    else:
        require(actual == expected, f'native contract mismatch: {label}; observed={actual!r}, expected={expected!r}')


def validate(observed):
    equal(observed['format_version'], 1, 'format version')
    equal(observed['runtime'], {'version': '8.5.12', 'version_id': 80512, 'int_size': 8, 'sapi': 'cli'}, 'runtime')
    equal(observed['unexpected_output_hex'], '', 'unexpected program output')
    cases = observed['cases']
    expected_names = {'basic', 'exceptions', 'fiber', 'wordpress_hooks', 'scratch_io', 'readonly_io'}
    require(set(cases) == expected_names, 'missing or unexpected contract cases')
    for name, result in cases.items():
        equal(result['status'], 'ok', name + ' execution')
        require(type(result['warnings']) is list, 'invalid warnings observation')
        if name != 'readonly_io':
            equal(result['warnings'], [], name + ' warnings')
    binary = b'\x00\xffRP\r\n' + '日本語'.encode('utf-8')
    equal(cases['basic']['value'], {
        'arithmetic': 17, 'binary_hex': binary.hex(), 'binary_bytes': len(binary),
        'unicode_bytes': 9, 'substring': '日',
        'json_roundtrip': {'text': '日本語', 'items': [False, None, 7], 'binary_base64': base64.b64encode(binary).decode('ascii')},
        'wide_integer': 2147483649, 'wide_integer_type': 'int', 'max_integer_decimal': '9223372036854775807',
    }, 'basic values')
    equal(cases['exceptions']['value'], {
        'trace': ['outer_try', 'inner_try', 'inner_catch', 'inner_finally', 'outer_catch', 'outer_finally'],
        'previous_class': 'RuntimeException', 'previous_message': 'inner failure',
    }, 'nested exceptions and finally')
    equal(cases['fiber']['value'], {
        'start_value': 'paused:seed', 'suspended': True, 'premature_return_error': 'FiberError',
        'resume_value': None, 'terminated': True, 'return_value': 'returned:next',
        'trace': ['start:seed', 'resume:next'],
    }, 'Fiber lifecycle')
    equal(cases['wordpress_hooks']['value'], {
        'initially_empty': True, 'filtered': 'post|plugin|theme', 'action_counter': 1,
        'action_calls': 1, 'null_result': True, 'current_filter_after': False,
    }, 'WordPress hooks')
    equal(cases['scratch_io']['value'], {'write_result': len(ORDINARY), 'read_hex': ORDINARY.hex(), 'readable': True, 'writable': True}, 'scratch IO')
    equal(cases['readonly_io']['value'], {
        'readable': True, 'writable': False, 'write_result': False,
        'before_hex': READONLY.hex(), 'after_hex': READONLY.hex(), 'unchanged': True,
    }, 'readonly metadata and actual IO')
    require(cases['readonly_io']['warnings'], 'readonly write denial was not observed')


def owned_path(path):
    resolved = path.resolve()
    require(resolved.is_relative_to(ROOT / 'target' / 'php-runtime-candidate-probe' / 'native-contract'),
            'scratch path is outside the owned native-contract area')
    return resolved


def run_reference():
    require(os.name == 'nt', 'this native reference helper pins Windows x64 PHP only')
    native, wordpress = verify_references()
    base = ROOT / 'target' / 'php-runtime-candidate-probe' / 'native-contract'
    require(base.resolve().is_relative_to(ROOT), 'scratch root escaped the workspace')
    base.mkdir(parents=True, exist_ok=True)
    directory = base / (str(os.getpid()) + '-' + uuid.uuid4().hex)
    owned_path(directory).mkdir(exist_ok=False)
    ordinary = directory / 'ordinary.txt'
    readonly = directory / 'readonly.txt'
    owned = []
    original_mode = None
    try:
        for path, data in [(ordinary, b''), (readonly, READONLY)]:
            with owned_path(path).open('xb') as destination:
                destination.write(data)
            owned.append(path)
        original_mode = stat.S_IMODE(readonly.stat().st_mode)
        os.chmod(owned_path(readonly), stat.S_IREAD)
        require(readonly.stat().st_file_attributes & stat.FILE_ATTRIBUTE_READONLY, 'Windows readonly attribute was not applied')
        environment = {key: value for key, value in os.environ.items() if key.upper() not in {'PHPRC', 'PHP_INI_SCAN_DIR', 'RP_WP_ROOT', 'RP_SCRATCH'}}
        environment.update({'RP_WP_ROOT': str(WP_ROOT), 'RP_SCRATCH': str(directory)})
        pid, completed = owned_probe.run_captured_probe([str(PHP_ROOT / 'php.exe'), '-n', str(FIXTURE)], environment, timeout=10)
        require(completed.returncode == 0, 'native PHP exited with failure')
        require(completed.stderr == b'', 'unexpected native PHP stderr')
        require(0 < len(completed.stdout) <= 64 * 1024 and completed.stdout.endswith(b'\n')
                and completed.stdout.count(b'\n') == 1, 'invalid native PHP output framing or size')
        observed = parse_json(completed.stdout)
        validate(observed)
        require(ordinary.read_bytes() == ORDINARY and readonly.read_bytes() == READONLY, 'scratch host file observations disagree')
        return native, wordpress, pid, observed
    finally:
        try:
            owned_probe.reap_owned_probes()
        finally:
            if original_mode is not None and readonly in owned and readonly.exists():
                os.chmod(owned_path(readonly), original_mode)
            for path in reversed(owned):
                if path.exists():
                    owned_path(path).unlink()
            owned_path(directory).rmdir()


def main():
    require(sys.argv[1:] in ([], ['--record']), 'usage: python native_contract.py [--record]')
    native, wordpress, pid, observed = run_reference()
    inputs = [Path(__file__).resolve(), FIXTURE, ROOT / 'scripts' / 'owned_probe.py', WP_MANIFEST]
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'role': 'native PHP comparison reference only; not a production fallback',
        'wasm_executed': False, 'platform': {'system': platform.system(), 'machine': platform.machine()},
        'input_files': {path.relative_to(ROOT).as_posix(): digest(path) for path in inputs},
        'native_reference': native, 'wordpress_reference': wordpress, 'owned_process_pid': pid,
        'observation': observed,
        'limitations': ['Native PHP only; no Wasm candidate has been executed by this helper.',
                        'Windows readonly file attributes are the tested permission boundary.',
                        'Output capture size is checked after child exit; total process RSS is not bounded here.'],
    }
    if sys.argv[1:] == ['--record']:
        destination = HERE / 'evidence' / 'native-contract.json'
        require(destination.resolve().is_relative_to(ROOT), 'evidence path escaped workspace')
        destination.parent.mkdir(exist_ok=True)
        destination.write_text(json.dumps(report, ensure_ascii=True, indent=2) + '\n', encoding='utf-8', newline='\n')
    print('Native PHP 8.5.12 reference passed: 6 observed cases; Wasm was not executed.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'Native contract failed: {error}', file=sys.stderr)
        sys.exit(1)
