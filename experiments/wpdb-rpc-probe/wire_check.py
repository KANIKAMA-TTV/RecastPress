"""Build the closed wire probe offline and bind its executable to checked inputs.

This step never starts or connects to a database. database_check.py separately
owns each disposable database and verifies this build receipt before execution.
"""
import argparse
from datetime import datetime, timezone
import importlib.util
import json
import os
from pathlib import Path
import platform
import secrets
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
TARGET = ROOT / 'target/wpdb-rpc-wire-probe'
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe


def load_preparer():
    spec = importlib.util.spec_from_file_location('rp_wire_preparer', HERE / 'prepare_driver.py')
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


preparer = load_preparer()
require, sha = preparer.require, preparer.sha


def input_files():
    paths = [HERE / p for p in ('Cargo.toml', 'Cargo.lock', 'src/main.rs',
        'wire_check.py', 'prepare_driver.py', 'driver-plan.json',
        'patches/0001-mysql-async-wpdb-handshake.patch')]
    paths += [ROOT / p for p in ('scripts/owned_probe.py', 'Cargo.lock',
        'experiments/binlog-probe/Cargo.lock', 'experiments/binlog-probe/Cargo.toml')]
    return {p.relative_to(ROOT).as_posix(): sha(preparer.read_fixed(p, 2 * 1024 * 1024)) for p in paths}


def executable_from_cargo(wire):
    require(len(wire) <= 16 * 1024 * 1024, 'Cargo stdout quota')
    selected, finished = [], False
    for line in wire.splitlines():
        value = json.loads(line)
        require(not finished, 'Cargo output after build-finished')
        if value.get('reason') == 'build-finished':
            require(value.get('success') is True, 'Cargo build failed')
            finished = True
        if value.get('reason') != 'compiler-artifact':
            continue
        if value.get('target', {}).get('name') != 'recastpress-wpdb-wire-probe':
            continue
        require(value['target']['kind'] == ['bin'] and value['profile']['test'] is False,
            'wire artifact must be a normal executable')
        require(Path(value['manifest_path']).resolve() == HERE / 'Cargo.toml'
            and Path(value['target']['src_path']).resolve() == HERE / 'src/main.rs', 'wire artifact source')
        artifact = Path(value['executable'])
        require(artifact.resolve(strict=True) == artifact and artifact.is_relative_to(TARGET), 'wire artifact escaped target')
        selected.append(artifact)
    require(finished and len(selected) == 1, 'exactly one wire executable required')
    return selected[0]


def verify_receipt(binary):
    """Reject stale source, staged driver or substituted executable before DB use."""
    path = HERE / 'evidence/windows-wire-build.json'
    record = json.loads(preparer.read_fixed(path, 128 * 1024))
    require(record['schema'] == 'recastpress.wpdb-wire-build.v0.5'
        and record['outcome'] == 'locked_offline_wire_build_passed'
        and record['passed'] is True and record['owned_cleanup_completed'] is True
        and record['original_inputs_unchanged_after'] is True, 'wire build receipt did not pass')
    require(input_files() == record['input_files'], 'wire build source changed')
    require(preparer.prepare() == record['preparation_after'], 'wire staged sources changed')
    binary = Path(binary).absolute()
    require(binary.resolve(strict=True) == binary and binary.is_relative_to(TARGET)
        and binary.relative_to(ROOT).as_posix() == record['executable'], 'wire binary path differs from build')
    require(preparer.identity(preparer.read_fixed(binary, 256 * 1024 * 1024)) == record['executable_identity'],
        'wire executable differs from build')
    return record


def invalid_destination_cases(binary, environment):
    value = {'schema': 'recastpress.wpdb-wire-input.v0.5', 'host': '192.0.2.1',
        'port': 1, 'database': 'rp_wpdb_probe', 'user': 'rp_wpdb_probe',
        'password': secrets.token_hex(24), 'tls': 'disabled', 'tls_ca_pem': None}
    records = []
    for name, reason in [('non_loopback_destination', 'input_closed_destination'),
            ('invalid_secret_shape', 'input_secret_shape')]:
        if name == 'invalid_secret_shape':
            value.update(host='127.0.0.1', password='invalid')
        pid, result = owned_probe.run_captured_probe([str(binary)], environment,
            timeout=10, input_bytes=json.dumps(value).encode())
        require(len(result.stdout) <= 16384 and not result.stderr and result.returncode == 1,
            'invalid destination rejection failed')
        observation = json.loads(result.stdout)
        require(observation['schema'] == 'recastpress.wpdb-wire-observation.v0.5'
            and observation['passed'] is False and observation['failure'] == reason
            and observation['handshakes'] == [] and observation['observations'] == [],
            'invalid input must fail before connecting')
        records.append({'case': name, 'owned_pid': pid, 'exit_code': result.returncode,
            'reaped': True, 'observation': observation})
    return records


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    passed = False
    record = {'schema': 'recastpress.wpdb-wire-build.v0.5', 'outcome': 'not_run', 'passed': False,
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'platform': platform.system(),
        'locked_offline': True, 'outer_build_timeout_seconds': 1200,
        'owned_cleanup_completed': False, 'original_inputs_unchanged_after': False,
        'database_started': False, 'database_connected': False, 'product_driver_selected': False}
    try:
        require(os.name == 'nt', 'this receipt is the Windows owned validation build')
        before = input_files()
        record['input_files'] = before
        record['preparation_before'] = preparer.prepare()
        require(TARGET.resolve() == TARGET, 'wire target linked')
        environment = {k: v for k, v in os.environ.items() if not k.upper().startswith('MYSQL_')
            and k.upper() not in {'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        environment.update(CARGO_NET_OFFLINE='true', CARGO_BUILD_JOBS='2', CARGO_INCREMENTAL='0')
        command = ['cargo', 'build', '--manifest-path', str(HERE / 'Cargo.toml'), '--target-dir', str(TARGET),
            '--locked', '--offline', '--message-format=json']
        pid, result = owned_probe.run_captured_probe(command, environment, timeout=1200)
        record.update(owned_build_pid=pid, build_exit_code=result.returncode, owned_build_reaped=True,
            build_stdout_sha256=sha(result.stdout), build_stderr_sha256=sha(result.stderr))
        require(result.returncode == 0 and len(result.stderr) <= 65536, 'wire compilation failed')
        binary = executable_from_cargo(result.stdout)
        record.update(executable=binary.relative_to(ROOT).as_posix(),
            executable_identity=preparer.identity(preparer.read_fixed(binary, 256 * 1024 * 1024)))
        record['input_rejections'] = invalid_destination_cases(binary, environment)
        require(preparer.identity(preparer.read_fixed(binary, 256 * 1024 * 1024)) == record['executable_identity'],
            'wire executable changed during input checks')
        record['preparation_after'] = preparer.prepare()
        require(record['preparation_after'] == record['preparation_before'], 'staged source changed during build')
        require(input_files() == before, 'wire inputs changed during build')
        record['original_inputs_unchanged_after'] = True
        passed = True
        record['outcome'] = 'locked_offline_wire_build_passed'
    except (Exception, KeyboardInterrupt) as error:
        record['outcome'] = 'wire_build_failed'
        record['failure_class'] = type(error).__name__
        # Never expose Cargo environment or arbitrary child diagnostics.
    finally:
        try:
            owned_probe.reap_owned_probes()
            record['owned_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt):
            passed = False
            record['outcome'] = 'wire_build_cleanup_failed'
    record['passed'] = passed
    if args.record:
        path = HERE / 'evidence/windows-wire-build.json'
        require(path.resolve() == path, 'wire receipt linked')
        wire = (json.dumps(record, indent=2, ensure_ascii=True) + '\n').encode()
        require(len(wire) <= 128 * 1024, 'wire receipt quota')
        path.write_bytes(wire)
    print('WPDB wire build: ' + record['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
