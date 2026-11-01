"""Build, execute, and optionally record the fixed P0 PHP WASI experiment.

Cargo/tool configuration is trusted. Child capture limits are checked after exit;
this helper is not an OS process RSS limit or a production worker supervisor.
"""
from datetime import datetime, timezone
from pathlib import Path
import hashlib
import json
import os
import platform
import re
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe
import dependency_inventory

TARGET = ROOT / 'target' / 'php-wasi-host-probe'
NAME = 'recastpress-php-wasi-host-probe'
PREFIX = b'PHP_WASI_HOST_PROBE_RESULT='
CHECKS = [
    'real_php_wasi', 'binary_input', 'cgi_output', 'include_read', 'shared_request_state',
    'live_rust_cache_callback', 'live_rust_db_fixture_callback', 'fresh_request_state',
    'read_only_preopen', 'outside_file_denied', 'no_inherited_environment',
    'wp_filter_chain', 'wp_actions_shared_object', 'wp_null_sentinel', 'wp_request_hook_cleanup',
    'rpc_output_buffer_independent', 'literal_stdout_preserved', 'fragmented_rpc', 'rpc_descriptor_reopen',
    'cgi_status_and_cookies', 'cgi_redirect', 'cgi_no_content', 'cgi_not_modified',
    'fuel', 'epoch', 'fatal', 'output', 'memory', 'bad_rpc',
    'rpc_oversize', 'rpc_incomplete',
    'cgi_wrong_length', 'cgi_hop_header', 'cgi_header_limit',
    'parallel_request_isolation', 'independent_request_deadlines', 'normal_request_during_sibling_timeout',
]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            value.update(chunk)
    return value.hexdigest()


def json_object(pairs):
    value = {}
    for key, item in pairs:
        require(key not in value, 'duplicate JSON key')
        value[key] = item
    return value


def parse_json(wire):
    def bad_constant(_):
        raise ValueError('non-finite JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=json_object, parse_constant=bad_constant)


def select_artifact(wire):
    require(type(wire) is bytes and 0 < len(wire) <= 16 * 1024 * 1024 and wire.endswith(b'\n'), 'invalid Cargo capture')
    candidates = []
    finished = False
    for line in wire.splitlines():
        message = parse_json(line)
        require(isinstance(message, dict) and not finished, 'invalid Cargo message sequence')
        if message.get('reason') == 'build-finished':
            require(message.get('success') is True, 'Cargo build failed')
            finished = True
        elif message.get('reason') == 'compiler-artifact' and message.get('target', {}).get('name') == NAME:
            require(message['target']['kind'] == ['bin'] and message['profile']['test'] is False, 'wrong artifact kind')
            require(Path(message['manifest_path']).resolve() == HERE / 'Cargo.toml', 'wrong artifact manifest')
            require(Path(message['target']['src_path']).resolve() == HERE / 'src' / 'main.rs', 'wrong artifact source')
            path = Path(message['executable']).resolve(strict=True)
            require(path.is_file() and path.is_relative_to(TARGET.resolve()), 'artifact outside probe target')
            candidates.append(path)
    require(finished and len(candidates) == 1, 'missing or ambiguous Cargo-selected executable')
    return candidates[0]


def validate_result(pid, completed):
    require(completed.returncode == 0 and completed.stderr == b'', 'native probe failed')
    wire = completed.stdout
    require(type(wire) is bytes and len(wire) <= 4096 and wire.endswith(b'\n') and wire.count(b'\n') == 1 and wire.startswith(PREFIX), 'invalid native probe response')
    value = parse_json(wire[len(PREFIX):])
    expected = {
        'format_version': 3, 'milestone': 'v0.5-P0-experiment', 'php_version': '8.2.6',
        'process_pid': pid,
        'artifact_sha256': '4fd2e8c42ae529ba72f88a0f1e46de1fc69a4b4f01e01fedd65ca966b8ffe6fa',
        'wasmtime_version': '36.0.19', 'checks': CHECKS, 'recovery_requests': 11,
        'parallel_requests': 4, 'shared_epoch_tick_ms': 10,
        'cranelift_opt_level': 'none',
        'memory_limit_bytes': 256 * 1024 * 1024, 'stdout_limit_bytes': 64 * 1024,
        'fuel_per_request': 200_000_000, 'product_ready': False,
        'wordpress_bootstrapped': False, 'real_database_connected': False,
        'wordpress_hook_version': '7.1.3', 'wordpress_hooks_executed': True,
        'bridge': 'fixed-fixture dedicated WASI descriptor channel; not the product ABI',
    }
    require(type(value) is dict and set(value) == set(expected), 'unexpected result fields')
    for key, item in expected.items():
        require(type(value[key]) is type(item) and value[key] == item, f'result mismatch: {key}')
    return value


def main():
    require(sys.argv[1:] in ([], ['--record']), 'usage: python experiments/php-wasi-host-probe/check.py [--record]')
    environment = os.environ.copy()
    try:
        _, built = owned_probe.run_captured_probe([
            'cargo', 'build', '--manifest-path', str(HERE / 'Cargo.toml'),
            '--bin', NAME, '--release', '--target-dir', str(TARGET), '--offline', '--locked', '--message-format=json',
        ], environment, timeout=600)
        require(built.returncode == 0 and len(built.stderr) <= 1024 * 1024, 'Cargo build failed')
        artifact = select_artifact(built.stdout)
        pid, completed = owned_probe.run_captured_probe([str(artifact)], environment, timeout=300)
        if completed.returncode != 0:
            # Only fixed fixture output; no application data or inherited guest environment.
            print(completed.stderr.decode('utf-8', errors='replace')[:16384], file=sys.stderr)
        native = validate_result(pid, completed)
        _, tested = owned_probe.run_captured_probe([
            'cargo', 'test', '--manifest-path', str(HERE / 'Cargo.toml'),
            '--release', '--target-dir', str(TARGET), '--offline', '--locked',
            'cgi_response::tests', '--', '--color=never',
        ], environment, timeout=600)
        require(tested.returncode == 0 and len(tested.stdout) <= 65536 and len(tested.stderr) <= 1024 * 1024, 'CGI unit tests failed')
        test_names = re.findall(rb'^test (cgi_response::tests::[a-z_]+) \.\.\. ok$', tested.stdout, re.MULTILINE)
        require(len(test_names) == len(set(test_names)) == 9 and b'test result: ok. 9 passed; 0 failed;' in tested.stdout, 'CGI unit test scope mismatch')
    finally:
        owned_probe.reap_owned_probes()
    inputs = [HERE / name for name in ['Cargo.toml', 'Cargo.lock', 'fixture.json', 'wordpress-fixture.json', 'prepare_fixture.py', 'check.py', 'outside.txt']]
    inputs += sorted((HERE / 'src').glob('*.rs')) + sorted((HERE / 'fixtures').glob('*.php'))
    inputs += sorted((HERE / 'fixtures').glob('*.wat'))
    inputs += [ROOT / 'scripts' / 'owned_probe.py', ROOT / 'scripts' / 'dependency_inventory.py']
    observation = {
        'format_version': 1,
        'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'input_files': {path.relative_to(ROOT).as_posix(): digest(path) for path in sorted(inputs)},
        'native_artifact': artifact.relative_to(ROOT).as_posix(),
        'native_artifact_sha256': digest(artifact),
        'result': native,
        'cgi_unit_tests': [name.decode('ascii') for name in sorted(test_names)],
        'wordpress_reference': json.loads((HERE / 'wordpress-fixture.json').read_text(encoding='utf-8')),
        'limitations': [
            'P0 experiment, not P0 product acceptance or an engine/license adoption decision.',
            'Old PHP 8.2.6 upstream WASI fixture; not the planned production PHP build.',
            'DB callbacks use fixed Rust data; no MySQL/MariaDB or standard wpdb execution.',
            'Fresh stores are disposed; this does not prove process RSS reclamation or database rollback.',
            'Wasm linear memory is limited; compiler, Rust, WASI and total process RSS are not capped here.',
            'Deadline interrupts guest execution; blocking host calls need separate production supervision.',
            'No WordPress bootstrap, real HTTP, theme/plugin compatibility, or three-platform claim.',
        ],
    }
    if sys.argv[1:] == ['--record']:
        dependency_inventory.PROJECTS = {'v0_5_php_wasi_host_probe': (HERE / 'Cargo.toml').relative_to(ROOT).as_posix()}
        declarations = dependency_inventory.inventory()
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / 'dependencies.json').write_text(json.dumps(declarations, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
        (directory / 'native.json').write_text(json.dumps(observation, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print(f'PHP WASI P0 experiment passed: {len(CHECKS)} checks, 11 post-failure recovery requests, 4 parallel requests, 9 CGI unit tests.')
    print('Scope: experimental Rust-hosted PHP; production runtime, WordPress and real DB remain open.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print(f'PHP WASI experiment check failed: {error}', file=sys.stderr)
        sys.exit(1)
