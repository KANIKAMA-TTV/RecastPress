"""Compile and run only two driver policy units, with explicit dependency refill.

Offline resolution starts from the unchanged binlog lock in an ignored isolated
workspace. This never starts a DB or tests real handshake/SQL/wpdb compatibility.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tomllib

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe


def load():
    spec = importlib.util.spec_from_file_location('rp_wpdb_pinned_driver_preparation', HERE / 'prepare_driver.py')
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


preparer = load()
require, sha = preparer.require, preparer.sha
FEATURES = ['minimal-rust', 'rustls-tls', 'ring', 'tls12', 'binlog']
TESTS = ['opts::test::rp_legacy_handshake_capability_defaults_are_preserved',
    'opts::test::rp_wpdb_handshake_clears_only_selected_optional_capabilities']


def diagnostic(wire):
    text = wire.decode('utf-8', errors='replace')
    for path, label in [(ROOT, '<workspace>'), (preparer.CARGO_HOME, '<cargo-cache>'), (Path.home(), '<user-home>')]:
        text = text.replace(str(path), label).replace(path.as_posix(), label)
    return text[:8192]


def sources():
    paths = [HERE / name for name in ['driver-plan.json', 'patches/0001-mysql-async-wpdb-handshake.patch',
        'prepare_driver.py', 'driver_check.py']]
    paths += [ROOT / 'scripts/owned_probe.py', ROOT / 'Cargo.lock', ROOT / 'experiments/binlog-probe/Cargo.lock',
        ROOT / 'experiments/binlog-probe/Cargo.toml']
    return {p.relative_to(ROOT).as_posix(): sha(preparer.read_fixed(p, 2 * 1024 * 1024)) for p in paths}


def select_test(wire):
    require(0 < len(wire) <= 16 * 1024 * 1024, 'Cargo capture quota')
    selected = []; finished = False
    crate = preparer.WORKSPACE / 'source/mysql_async-0.37.1'
    for line in wire.splitlines():
        value = json.loads(line); require(not finished, 'Cargo output after build-finished')
        if value.get('reason') == 'build-finished':
            require(value.get('success') is True, 'Cargo build-finished failure'); finished = True
        if value.get('reason') != 'compiler-artifact' or value.get('target', {}).get('name') != 'mysql_async': continue
        if value.get('profile', {}).get('test') is not True: continue
        require(value['target']['kind'] == ['lib'], 'wrong driver test kind')
        require(Path(value['manifest_path']).resolve() == crate / 'Cargo.toml', 'wrong driver test manifest')
        require(Path(value['target']['src_path']).resolve() == crate / 'src/lib.rs', 'wrong driver test source')
        executable = Path(value['executable']).resolve(strict=True)
        require(executable.is_relative_to(preparer.TARGET) and not executable.is_symlink(), 'test artifact outside owned target')
        selected.append(executable)
    require(finished and len(selected) == 1, 'one actual driver unit executable required')
    return selected[0]


def registry_lock_rows(wire):
    return {(row['name'], row['version']): row.get('checksum') for row in tomllib.loads(wire.decode())['package']
        if row.get('source', '').startswith('registry+')}


def main():
    parser = argparse.ArgumentParser(description=__doc__); parser.add_argument('--record', action='store_true')
    parser.add_argument('--resolve-with-network', action='store_true', help='explicitly refill public registry validation dependencies; build/tests stay locked offline')
    args = parser.parse_args(); passed = False; stage = 'preparation'
    report = {'format_version': 1, 'schema': 'recastpress.wpdb-rpc-driver-units.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'platform': {'system': platform.system(), 'machine': platform.machine()},
        'outcome': 'not_run', 'tests_passed': False, 'patch_compiled': False,
        'owned_tests_reaped': False, 'owned_cleanup_completed': False, 'compile_and_tests_locked_offline': True,
        'dependency_network_access_enabled': args.resolve_with_network,
        'database_started': False, 'database_connected': False, 'php_executed': False,
        'handshake_wire_verified': False, 'sql_bytes_transport_verified': False, 'native_wpdb_comparison_passed': False,
        'standard_wpdb_compatibility_verified': False, 'product_driver_selected': False,
        'product_abi_selected': False, 'existing_editing_sqlx_changed': False, 'binary_redistributed': False,
        'original_inputs_unchanged_after': False, 'staged_sources_reverified_after': False,
        'outer_resolution_timeout_seconds': 120, 'outer_network_bootstrap_timeout_seconds': 300,
        'outer_network_pin_timeout_seconds': 300, 'outer_build_timeout_seconds': 1200,
        'outer_list_timeout_seconds': 10, 'outer_test_timeout_seconds': 30,
        'outer_cleanup_timeout_seconds': owned_probe.CLEANUP_TIMEOUT, 'features': FEATURES}
    try:
        before = sources(); report['input_files'] = before
        preparation = preparer.prepare(); report['preparation_before'] = preparation
        plan = preparer.load_plan(); require(plan['patch']['included_unit_tests'] == TESTS, 'fixed two-unit plan')
        manifest = preparer.owned(preparer.WORKSPACE / 'Cargo.toml')
        environment = {k: v for k, v in os.environ.items() if k.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        environment.update(CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0')
        state = preparer.owned(preparer.WORK / 'resolved-lock.json')
        if args.resolve_with_network:
            upstream = tomllib.loads(preparer.read_fixed(preparer.WORKSPACE / 'source/mysql_async-0.37.1/Cargo.lock', 1024 * 1024).decode())
            pins = [p for p in upstream['package'] if p['name'] == 'hdrhistogram']
            require(len(pins) == 1, 'one hdrhistogram pin in fixed original crate lock')
            report['preferred_original_crate_lock_package'] = pins[0]
            network_environment = {**environment, 'CARGO_NET_OFFLINE': 'false'}
            if not state.exists():
                stage = 'network_lock_resolution'
                # The optional package is absent from the original binlog lock.
                # First let Cargo add missing entries in this isolated lock;
                # then require the exact original-crate version before build.
                _, bootstrap = owned_probe.run_captured_probe(['cargo', 'metadata', '--format-version', '1',
                    '--manifest-path', str(manifest), '--no-default-features',
                    '--features', ','.join('mysql_async/' + name for name in FEATURES)], network_environment, timeout=300)
                report['network_bootstrap_exit_code'] = bootstrap.returncode
                require(len(bootstrap.stdout) <= 16 * 1024 * 1024 and len(bootstrap.stderr) <= 65536, 'network bootstrap capture budget')
                if bootstrap.stderr: report['network_bootstrap_diagnostics'] = diagnostic(bootstrap.stderr)
                require(bootstrap.returncode == 0, 'isolated validation lock bootstrap failed')
                _, updated = owned_probe.run_captured_probe(['cargo', 'update', '--manifest-path', str(manifest),
                    '--package', 'hdrhistogram', '--precise', pins[0]['version']], network_environment, timeout=300)
                report['network_resolution_exit_code'] = updated.returncode
                require(len(updated.stdout) <= 65536 and len(updated.stderr) <= 65536, 'network resolution capture budget')
                if updated.stderr: report['network_resolution_diagnostics'] = diagnostic(updated.stderr)
                require(updated.returncode == 0, 'pinned validation dependency resolution failed')
                lock_wire = preparer.read_fixed(preparer.WORKSPACE / 'Cargo.lock', 1024 * 1024)
                rows = registry_lock_rows(lock_wire)
                require(rows.get(('hdrhistogram', pins[0]['version'])) == pins[0]['checksum'], 'preferred hdrhistogram version/checksum differs')
                record = {'lock': preparer.identity(lock_wire), 'seed_sha256': preparation['seed_lock_sha256'],
                    'manifest_sha256': sha(preparer.MANIFEST)}
                with state.open('xb') as stream: stream.write((json.dumps(record, indent=2) + '\n').encode())
            environment_for_resolution = network_environment
        else: environment_for_resolution = environment
        stage = 'offline_resolution'
        if args.resolve_with_network: stage = 'locked_dependency_refill'
        command = ['cargo', 'metadata', '--format-version', '1', '--manifest-path', str(manifest),
            '--no-default-features', '--features', ','.join('mysql_async/' + name for name in FEATURES)]
        if not args.resolve_with_network: command.append('--offline')
        if state.exists(): command.append('--locked')
        _, resolved = owned_probe.run_captured_probe(command, environment_for_resolution, timeout=120)
        report['resolution_exit_code'] = resolved.returncode
        require(len(resolved.stdout) <= 16 * 1024 * 1024 and len(resolved.stderr) <= 65536, 'resolution capture budget')
        if resolved.stderr: report['resolution_diagnostics'] = diagnostic(resolved.stderr)
        require(resolved.returncode == 0, 'isolated dependency resolution failed')
        metadata = json.loads(resolved.stdout)
        for name, version in [('mysql_async', '0.37.1'), ('mysql_common', '0.37.3')]:
            packages = [p for p in metadata['packages'] if p['name'] == name and p['version'] == version]
            require(len(packages) == 1 and packages[0]['source'] is None, 'fixed staged path dependency required')
            require(Path(packages[0]['manifest_path']).resolve() == preparer.WORKSPACE / 'source' / (name + '-' + version) / 'Cargo.toml', 'resolved crate source escaped owned stage')
        lock = preparer.read_fixed(preparer.owned(preparer.WORKSPACE / 'Cargo.lock'), 1024 * 1024)
        seed = preparer.read_fixed(preparer.owned(preparer.WORKSPACE / 'seed-binlog.Cargo.lock'), 1024 * 1024)
        old_rows, new_rows = registry_lock_rows(seed), registry_lock_rows(lock)
        for key in old_rows.keys() & new_rows.keys(): require(old_rows[key] == new_rows[key], 'existing registry checksum changed')
        report['resolved_lock'] = preparer.identity(lock)
        report['additional_resolved_registry_packages'] = [{'name': k[0], 'version': k[1], 'checksum': new_rows[k]} for k in sorted(new_rows.keys() - old_rows.keys())]
        report['resolved_registry_package_count'] = len(new_rows)
        record = {'lock': preparer.identity(lock), 'seed_sha256': sha(seed), 'manifest_sha256': sha(preparer.MANIFEST)}
        if state.exists(): require(json.loads(preparer.read_fixed(state, 65536)) == record, 'recorded resolved lock changed')
        else:
            with state.open('xb') as stream: stream.write((json.dumps(record, indent=2) + '\n').encode())
        preparer.prepare()  # Verify full staged source before compilation.
        stage = 'build'
        command = ['cargo', 'test', '--release', '--lib', '--no-run', '-p', 'mysql_async',
            '--manifest-path', str(manifest), '--target-dir', str(preparer.owned(preparer.TARGET)),
            '--no-default-features', '--features', ','.join(FEATURES), '--locked', '--offline', '--message-format=json']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        require(len(built.stdout) <= 16 * 1024 * 1024 and len(built.stderr) <= 65536, 'driver build capture quota')
        if built.stderr: report['build_diagnostics'] = diagnostic(built.stderr)
        if built.returncode:
            messages = [item['message']['rendered'] for line in built.stdout.splitlines()
                if (item := json.loads(line)).get('reason') == 'compiler-message'
                and item.get('message', {}).get('level') == 'error' and type(item['message'].get('rendered')) is str]
            report['compiler_errors'] = diagnostic('\n'.join(messages).encode())
        require(built.returncode == 0, 'fixed driver unit compilation failed')
        executable = select_test(built.stdout); binary_sha = sha(preparer.read_fixed(executable, 256 * 1024 * 1024))
        report.update(patch_compiled=True, test_artifact=executable.relative_to(ROOT).as_posix(), test_artifact_sha256=binary_sha)
        stage = 'list'
        pid, listed = owned_probe.run_captured_probe([str(executable), 'rp_', '--list', '--format', 'terse'], environment, timeout=10)
        report.update(owned_list_pid=pid, list_exit_code=listed.returncode, owned_list_reaped=True)
        require(listed.returncode == 0 and len(listed.stdout) <= 65536 and not listed.stderr, 'unit listing failed/capture budget')
        names = [line[:-6] for line in listed.stdout.decode('utf-8').splitlines() if line.endswith(': test')]
        require(sorted(names) == sorted(TESTS) and len(names) == len(set(names)) == 2, 'exact two new tests must be listed')
        report['listed_tests'] = names
        stage = 'execution'
        pid, completed = owned_probe.run_captured_probe([str(executable), 'rp_', '--test-threads=1', '--show-output'], environment, timeout=30)
        report.update(owned_test_pid=pid, exit_code=completed.returncode, owned_tests_reaped=True)
        require(len(completed.stdout) <= 65536 and len(completed.stderr) <= 65536, 'driver unit capture quota')
        report.update(stdout_sha256=sha(completed.stdout), stderr_sha256=sha(completed.stderr), stdout_excerpt=diagnostic(completed.stdout))
        if completed.stderr: report['stderr_excerpt'] = diagnostic(completed.stderr)
        require(completed.returncode == 0, 'driver pure tests failed')
        output = completed.stdout.decode('utf-8').replace('\r\n', '\n')
        require(sorted(re.findall(r'^test (\S+) \.\.\. ok$', output, re.M)) == sorted(TESTS), 'not exactly both planned tests passed')
        require(re.search(r'test result: ok\. 2 passed; 0 failed; 0 ignored;', output), 'zero/ignored/failed tests cannot pass')
        report['preparation_after'] = preparer.prepare()
        require(sources() == before, 'helper/plan/original locks changed during run')
        require(sha(preparer.read_fixed(executable, 256 * 1024 * 1024)) == binary_sha, 'test executable changed')
        require(preparer.identity(preparer.read_fixed(preparer.WORKSPACE / 'Cargo.lock', 1024 * 1024)) == report['resolved_lock'], 'resolved lock changed during compilation')
        passed = report['tests_passed'] = True; report['outcome'] = 'fixed_driver_patch_two_units_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'; report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr: report['stderr_excerpt'] = diagnostic(error.stderr)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'; report['diagnostics'] = diagnostic(str(error).encode())
    finally:
        try: owned_probe.reap_owned_probes(); report['owned_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as error:
            passed = report['tests_passed'] = False; report['outcome'] = 'cleanup_failed'; report['cleanup_diagnostics'] = diagnostic(str(error).encode())
        if 'input_files' in report:
            try:
                require(sources() == report['input_files'], 'original helper/plan/locks changed')
                report['original_inputs_unchanged_after'] = True
                report['preparation_after'] = preparer.prepare()
                report['staged_sources_reverified_after'] = True
            except (Exception, KeyboardInterrupt) as error:
                passed = report['tests_passed'] = False
                report['after_verification_error'] = diagnostic(str(error).encode())
                if report['outcome'] == 'fixed_driver_patch_two_units_passed': report['outcome'] = 'after_verification_failed'
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / (platform.system().lower() + '-driver-units.json')
        require(path.resolve() == path and not path.is_symlink(), 'driver receipt path linked')
        wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode()
        require(len(wire) <= 128 * 1024, 'driver public metadata quota'); path.write_bytes(wire)
    print('WPDB driver patch units: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__': sys.exit(main())
