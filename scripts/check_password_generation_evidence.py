"""Strict finite relation evidence, separate from exact WP primitive comparisons."""
from datetime import datetime, timedelta
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / 'compatibility/evidence/wp-password-generation-crosscheck.json'
MAX_JSON = 128 * 1024
PROFILE = {'wordpress': '7.1.3', 'php': '8.5.12', 'custom_hasher': 'empty',
           'filters': 'empty', 'algorithm': '2y', 'cost': 12,
           'maximum': 12, 'raw_password_max_bytes': 4096,
           'trim_bytes_hex': '00090a0b0d20', 'comparison': 'cross_verification_not_hash_equality'}
LIMITATIONS = [
    'Finite public dummy inputs; random hash byte equality and salt uniqueness are not asserted.',
    'Separate generation relation experiment; no compatibility registry API or comparison counts are added.',
    'No custom hasher, filters, coercions, login/session policy, rehash or production credentials.',
    'OS RNG failure is covered only by native injected-provider tests, not an observed OS failure.',
    'No whole-memory erasure, wall-clock bound, RSS budget or concurrency guarantee.',
]

def cases():
    return [('empty', b''), ('plain', b'public-generation-dummy'),
            ('trim6', b'\x00\t\n\r\x0b public-generation-dummy \x0b\r\n\t\x00'),
            ('internal_nul', b'public\x00dummy'), ('form_feed', b'\x0cpublic-dummy\x0c'),
            ('binary', b'\xff\xfepublic\x80dummy'), ('exact4096', b'x' * 4096),
            ('exact4096_trimempty', b' ' * 4096),
            ('over4097', b'x' * 4097), ('over4097_trimempty', b' ' * 4097)]

def trim6(value):
    return value.strip(b'\x00\x09\x0a\x0b\x0d\x20')

def canonical(value):
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(',', ':')).encode('ascii')

def corpus_hash():
    return hashlib.sha256(canonical([[name, value.hex()] for name, value in cases()])).hexdigest()

def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key')
        result[key] = value
    return result

def read_json(path):
    with Path(path).open('rb') as stream:
        wire = stream.read(MAX_JSON + 1)
    if len(wire) > MAX_JSON:
        raise ValueError('evidence exceeds byte bound')
    return json.loads(wire.decode('utf-8'), object_pairs_hook=unique_object)

def digest(path):
    size = Path(path).stat().st_size
    if size > 128 * 1024 * 1024:
        raise ValueError('tracked file exceeds byte bound')
    result = hashlib.sha256()
    total = 0
    with Path(path).open('rb') as source:
        while chunk := source.read(min(65536, 128 * 1024 * 1024 + 1 - total)):
            total += len(chunk)
            if total > 128 * 1024 * 1024:
                raise ValueError('tracked file grew beyond byte bound')
            result.update(chunk)
    return result.hexdigest()

def tracked_files():
    return {'Cargo.lock', 'Cargo.toml', 'crates/recastpress-wp/Cargo.toml',
            'scripts/compare_password_generation.py', 'scripts/check_password_generation_evidence.py',
            'scripts/test_password_generation_evidence.py', 'scripts/password_generation_oracle.php',
            'scripts/reference_functions.php', 'scripts/owned_probe.py', 'scripts/setup_php.py',
            '.reference/php-8.5.12/recastpress-tool.json',
            '.reference/wordpress-7.1.3/recastpress-reference.json',
            '.reference/wordpress-7.1.3/wp-includes/pluggable.php',
            '.reference/wordpress-7.1.3/wp-includes/plugin.php',
            '.reference/wordpress-7.1.3/wp-includes/class-wp-hook.php',
            '.reference/wordpress-7.1.3/wp-includes/class-wp-filter-sentinel.php',
            *[path.relative_to(ROOT).as_posix() for folder in ('src', 'tests', 'examples')
              for path in (ROOT / 'crates/recastpress-wp' / folder).rglob('*.rs')]}

def binary_paths():
    import os
    return {'rust': 'target/debug/examples/password_generation_probe' + ('.exe' if os.name == 'nt' else ''),
            'php': '.reference/php-8.5.12/php.exe'}

def verify_pinned():
    """Selected immutable WP source and installed tool identities, no downloads."""
    import setup_php
    import wordpress_reference as reference
    source = ROOT / '.reference/wordpress-7.1.3'
    manifest = read_json(source / reference.MANIFEST_NAME) if (source / reference.MANIFEST_NAME).stat().st_size <= MAX_JSON else None
    # The complete WordPress manifest is larger than our evidence JSON bound.
    if manifest is None:
        with (source / reference.MANIFEST_NAME).open('rb') as stream:
            wire = stream.read(8 * 1024 * 1024 + 1)
        if len(wire) > 8 * 1024 * 1024: raise ValueError('reference manifest bound')
        manifest = json.loads(wire, object_pairs_hook=unique_object)
    if manifest['wordpress_version'] != reference.VERSION or manifest['archive_sha1'] != reference.EXPECTED_SHA1:
        raise ValueError('WordPress baseline differs')
    files = {entry['path']: entry['sha256'] for entry in manifest['files']}
    for name in ('pluggable.php', 'plugin.php', 'class-wp-hook.php', 'class-wp-filter-sentinel.php'):
        relative = 'wp-includes/' + name
        if digest(source / relative) != files[relative]: raise ValueError('pinned WordPress source differs')
    tool = read_json(ROOT / '.reference/php-8.5.12/recastpress-tool.json')
    if tool['archive_sha256'] != setup_php.SHA256 or tool['php_version'] != setup_php.VERSION:
        raise ValueError('PHP tool baseline differs')
    seen = set()
    for entry in tool['files']:
        relative = reference.safe_relative('wordpress/' + entry['path'])
        if relative.as_posix() in seen: raise ValueError('duplicate PHP tool file')
        seen.add(relative.as_posix())
        if digest(ROOT / '.reference/php-8.5.12' / relative) != entry['sha256']:
            raise ValueError('pinned PHP tool differs')
    if 'php.exe' not in seen: raise ValueError('PHP binary identity missing')

def exact(actual, expected):
    return canonical(actual) == canonical(expected)

def fields(value, expected):
    if type(value) is not dict or set(value) != set(expected):
        raise ValueError('evidence fields differ')

def positive_pid(value):
    if type(value) is not int or value <= 0 or value > 0xffffffff:
        raise ValueError('invalid process PID')

def hash_shape(value):
    if type(value) is not str or not re.fullmatch(r'[0-9a-f]{126}', value):
        raise ValueError('invalid generated hash wire')
    raw = bytes.fromhex(value)
    alphabet = b'./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
    if (not raw.startswith(b'$wp$2y$12$') or any(byte not in alphabet for byte in raw[10:])
            or alphabet.index(raw[31]) % 16 != 0 or alphabet.index(raw[62]) % 4 != 0):
        raise ValueError('noncanonical generated bcrypt')

def validate_record(record, verify_files=True):
    fields(record, ['format_version', 'scope', 'generated_at_utc', 'profile', 'corpus_sha256',
                    'runs', 'cases', 'host_refusals', 'tracked_files', 'binaries', 'limitations'])
    if not exact(record['format_version'], 1) or record['scope'] != 'public_dummy_wp_password_generation_cross_verification':
        raise ValueError('wrong evidence scope')
    if not exact(record['profile'], PROFILE) or record['corpus_sha256'] != corpus_hash() or not exact(record['limitations'], LIMITATIONS):
        raise ValueError('generation profile differs')
    stamp = record['generated_at_utc']
    if type(stamp) is not str or 'T' not in stamp:
        raise ValueError('invalid timestamp')
    if datetime.fromisoformat(stamp).utcoffset() != timedelta(0):
        raise ValueError('timestamp must be UTC')
    run_names = ['rust_generate', 'php_generate', 'php_verify', 'rust_verify']
    fields(record['runs'], run_names)
    for name in run_names:
        run = record['runs'][name]
        fields(run, ['owned_pid', 'reported_pid', 'exit_code', 'stderr_empty'])
        positive_pid(run['owned_pid']); positive_pid(run['reported_pid'])
        if not exact(run['owned_pid'], run['reported_pid']) or not exact(run['exit_code'], 0) or run['stderr_empty'] is not True:
            raise ValueError('child execution did not succeed')
    if type(record['cases']) is not list or len(record['cases']) != len(cases()):
        raise ValueError('case coverage differs')
    for item, (name, password) in zip(record['cases'], cases(), strict=True):
        fields(item, ['name', 'native_hash_hex', 'php_hash_hex', 'algorithm_calls', 'options_calls',
                      'globals_restored', 'actions_unchanged', 'php_checks', 'native_checks', 'check_calls'])
        if item['name'] != name or item['globals_restored'] is not True or item['actions_unchanged'] is not True:
            raise ValueError('case context differs')
        over = len(password) > 4096
        for key in ('native_hash_hex', 'php_hash_hex'):
            if over:
                if item[key] != '2a': raise ValueError('overlength marker differs')
            else: hash_shape(item[key])
        count = 0 if over else 1
        if not exact(item['algorithm_calls'], count) or not exact(item['options_calls'], count):
            raise ValueError('actual generation filter counts differ')
        # Native verify intentionally does not admit the '*' legacy marker.
        checks = [] if over else [True, password == trim6(password), False]
        if not exact(item['php_checks'], checks) or not exact(item['native_checks'], checks):
            raise ValueError('cross-verification relation failed')
        if not exact(item['check_calls'], [] if over else [1, 1, 1]):
            raise ValueError('actual verification hooks differ')
    if not exact(record['host_refusals'], {'within_max4': 'cost_limit', 'over_max4': '2a'}):
        raise ValueError('host refusal order differs')
    hashes = record['tracked_files']
    fields(hashes, tracked_files())
    for path, value in hashes.items():
        if type(value) is not str or not re.fullmatch('[0-9a-f]{64}', value): raise ValueError('bad source hash')
        if verify_files and value != digest(ROOT / path): raise ValueError('stale generation source: ' + path)
    fields(record['binaries'], binary_paths())
    for name, path in binary_paths().items():
        value = record['binaries'][name]
        fields(value, ['path', 'sha256'])
        if value['path'] != path or type(value['sha256']) is not str or not re.fullmatch('[0-9a-f]{64}', value['sha256']):
            raise ValueError('binary identity differs')
        if verify_files and value['sha256'] != digest(ROOT / path): raise ValueError('stale generation binary')
    return record

def main():
    verify_pinned()
    validate_record(read_json(OUTPUT))
    print('Password generation cross-verification evidence matches current sources and binaries; finite public dummies only')

if __name__ == '__main__':
    main()
