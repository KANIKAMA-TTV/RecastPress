"""Pinned native PHP Fiber oracle only; no Wasm, patched backend or fallback.

Runs one fixed fixture with a 2MiB Fiber stack trial setting and a ten-second
owned-child deadline. Exact GC counts/messages belong to this closed PHP build;
they are observations, not a general guarantee for other PHP configurations.
"""
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import uuid
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
COMMON = ROOT / 'experiments/php-runtime-candidate-probe/native_contract.py'
_spec = importlib.util.spec_from_file_location('recastpress_fiber_native_reference_gate', COMMON)
native = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(native)
require, equal, digest = native.require, native.equal, native.digest
FIXTURE = HERE / 'fixtures/fiber_contract.php'
ARCHIVE = ROOT / '.reference/php-8.5.12-nts-Win32-vs17-x64.zip'
BASE = ROOT / 'target/php-wasix-fiber-probe/native-contract'
DESTINATION = HERE / 'evidence/native-fiber-contract.json'
CASES = ['multi_lifetime', 'nested', 'values', 'throw_catch_finally', 'suspended_gc', 'api_errors']
STACK_BYTES = 2097152


def error(class_name, message):
    return {'threw': True, 'class': class_name, 'message': message}


def expected_values():
    not_suspended = error('FiberError', 'Cannot resume a fiber that is not suspended')
    return {
        'multi_lifetime': {
            'fiber_count': 4, 'starts': {owner: 'pause:' + owner for owner in 'abcd'},
            'resumes': dict.fromkeys('abcd'), 'a_done_while_others_suspended': True,
            'returns': {owner: {'owner': owner, 'received': {'sequence': index + 1, 'text': owner.upper()}, 'frames': 4}
                        for index, owner in enumerate('abcd')},
            'trace': [owner + ':start' for owner in 'abcd'] + [owner + ':return' for owner in 'abcd'],
            'main_current_is_null': True,
        },
        'nested': {
            'start': {'outer': 'paused', 'inner_start': 'inner-paused', 'inner_suspended': True},
            'resume': None, 'return': {'outer_received': 'outer-next', 'inner_resume': None, 'inner_return': 'inner-return:inner-next'},
            'terminated': True, 'trace': ['outer:start', 'inner:start', 'outer:inner-paused', 'outer:resume:outer-next',
                                        'inner:resume:inner-next', 'inner:finally', 'outer:inner-done', 'outer:finally'],
        },
        'values': {
            'before': {'started': False, 'suspended': False, 'terminated': False},
            'inside': {'current_is_fiber': True, 'running': True, 'started': True, 'suspended': False},
            'start': {'integer': 17, 'float': 1.5, 'false': False, 'null': None, 'text': '日本語', 'binary_hex': '00ff'},
            'suspended': True,
            'premature_return': error('FiberError', 'Cannot get fiber return value: The fiber has not returned'),
            'resume': None, 'return': {'seed': {'seed': 7}, 'reply': {'answer': 23, 'ok': True}}, 'terminated': True,
        },
        'throw_catch_finally': {
            'start': 'armed', 'throw_value': {'class': 'RuntimeException', 'message': 'injected'},
            'suspended_after_throw': True, 'resume': None, 'return': {'reply': 'finish', 'caught_class': 'RuntimeException'},
            'terminated': True, 'uncaught_start': 'uncaught-armed',
            'uncaught_error': error('LogicException', 'uncaught'), 'uncaught_terminated': True,
            'uncaught_return_error': error('FiberError', 'Cannot get fiber return value: The fiber threw an exception'),
            'trace': ['caught:start', 'caught:catch:injected', 'caught:resume:finish', 'caught:finally', 'uncaught:start', 'uncaught:finally'],
        },
        'suspended_gc': {
            'start': 'gc-paused', 'alive_before_collection': True, 'collected_cycles': 1,
            'weak_is_null': True, 'trace': ['gc:start', 'gc:finally'],
        },
        'api_errors': {
            'before_start': {'resume': not_suspended, 'throw': not_suspended,
                             'get_return': error('FiberError', 'Cannot get fiber return value: The fiber has not been started')},
            'start': None,
            'after_termination': {'start': error('FiberError', 'Cannot start a fiber that has already been started'),
                                  'resume': not_suspended, 'throw': not_suspended},
            'terminated': True, 'return': 42,
            'suspend_from_main': error('FiberError', 'Cannot suspend outside of a fiber'), 'main_current_is_null': True,
        },
    }


def validate(observed):
    equal(observed, {
        'schema': 'recastpress.php-fiber-contract.v0.5', 'ok': True,
        'runtime': {'version': '8.5.12', 'version_id': 80512, 'int_size': 8, 'sapi': 'cli', 'fiber_stack_size': str(STACK_BYTES)},
        'cases': {name: {'status': 'ok', 'value': value, 'warnings': []} for name, value in expected_values().items()},
        'unexpected_output_hex': '',
    }, 'fixed native Fiber contract')


def source_hashes():
    # Keep Git-verifiable source inputs separate from the ignored prepared
    # distributions, which verify_package binds in its explicit reference data.
    paths = [Path(__file__).resolve(), FIXTURE, COMMON, ROOT / 'scripts/owned_probe.py',
             native.WP_MANIFEST]
    for path in paths:
        require(path.resolve(strict=True) == path and path.is_file(), 'linked or missing fixed source input')
    return {path.relative_to(ROOT).as_posix(): digest(path) for path in paths}


def verify_package():
    package, wordpress = native.verify_references()
    files = package['verified_files']
    require(len(files) == 78 and native.PHP_ROOT.resolve() == native.PHP_ROOT, 'exact pinned 78-file package required')
    actual = set()
    for path in native.PHP_ROOT.rglob('*'):
        require(path.resolve() == path and not path.is_symlink(), 'native package contains a linked path')
        if path.is_file():
            actual.add(path.relative_to(native.PHP_ROOT).as_posix())
    require(actual == set(files) | {'recastpress-tool.json'}, 'native package file set changed')
    require(ARCHIVE.resolve(strict=True) == ARCHIVE and digest(ARCHIVE) == native.PHP_ARCHIVE_SHA256,
            'native package archive SHA changed')
    with zipfile.ZipFile(ARCHIVE) as archive:
        entries = [entry for entry in archive.infolist() if not entry.is_dir()]
        require(len(entries) == 78 and {entry.filename for entry in entries} == set(files), 'archive/package file sets differ')
        for entry in entries:
            value = hashlib.sha256()
            with archive.open(entry) as source:
                for block in iter(lambda: source.read(1024 * 1024), b''):
                    value.update(block)
            equal(value.hexdigest(), files[entry.filename], 'archive member/extracted native file hash')
    package['distribution_gate'] = {
        'manifest_files': 78, 'extracted_file_set_verified': True, 'all_78_extracted_hashes_verified': True,
        'archive_bytes': ARCHIVE.stat().st_size, 'archive_sha256_verified': True,
        'archive_member_file_set_verified': True, 'all_78_archive_member_hashes_match_extracted_files': True,
    }
    return package, {'source_side_check_only': True, 'executed': False,
                     'version': wordpress['wordpress_version'], 'manifest_sha256': digest(native.WP_MANIFEST),
                     'verified_source_files': wordpress['files']}


def owned_path(path):
    require(BASE.resolve() == BASE and BASE.is_relative_to(ROOT / 'target'), 'owned Fiber scratch base escaped workspace')
    require(path.is_relative_to(BASE) and path.resolve() == path, 'owned Fiber scratch path escaped fixed target')
    return path


def public_message(value):
    return str(value).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')[:8192]


def observe(report):
    require(os.name == 'nt', 'native reference pins Windows x64 PHP only')
    before = source_hashes()
    report['input_files'] = before
    package, wordpress = verify_package()
    report['native_reference'], report['wordpress_verifier_side_check'] = package, wordpress
    executable = native.PHP_ROOT / 'php.exe'
    report['native_executable'] = {'path': executable.relative_to(ROOT).as_posix(), 'sha256': package['verified_files']['php.exe']}
    report['fixture'] = {'path': FIXTURE.relative_to(ROOT).as_posix(), 'bytes': FIXTURE.stat().st_size, 'sha256': digest(FIXTURE)}
    owned_path(BASE).mkdir(parents=True, exist_ok=True)
    directory = BASE / (str(os.getpid()) + '-' + uuid.uuid4().hex)
    owned_path(directory).mkdir(exist_ok=False)
    report['owned_scratch_reclaimed'] = False
    try:
        system_root = os.environ.get('SystemRoot') or os.environ.get('SYSTEMROOT')
        require(system_root and Path(system_root).is_absolute(), 'Windows SystemRoot is missing')
        environment = {'SystemRoot': system_root, 'WINDIR': system_root, 'TEMP': str(directory), 'TMP': str(directory)}
        report['environment'] = {'provided_keys': sorted(environment), 'absolute_values_recorded': False,
                                 'php_configuration_environment_forwarded': False, 'credentials_forwarded': False}
        command = [str(executable), '-n', '-d', 'fiber.stack_size=' + str(STACK_BYTES), str(FIXTURE)]
        report['command'] = [executable.relative_to(ROOT).as_posix(), '-n', '-d', 'fiber.stack_size=' + str(STACK_BYTES),
                             FIXTURE.relative_to(ROOT).as_posix()]
        pid, result = native.owned_probe.run_captured_probe(command, environment, timeout=10, input_bytes=b'')
        report.update(owned_process_pid=pid, exit_code=result.returncode, owned_host_reaped=True)
        for name, wire in [('stdout', result.stdout), ('stderr', result.stderr)]:
            report[name + '_bytes'], report[name + '_sha256'] = len(wire), hashlib.sha256(wire).hexdigest()
        require(0 < len(result.stdout) <= 65536 and len(result.stderr) <= 65536, 'native Fiber capture budget')
        require(result.stdout.endswith(b'\n') and result.stdout.count(b'\n') == 1, 'one terminated native Fiber JSON required')
        observed = native.parse_json(result.stdout)
        report['observation'] = observed  # Preserve actual failed cases before any success checks.
        require(result.returncode == 0 and result.stderr == b'', 'native PHP execution or stderr failure')
        validate(observed)
        equal(source_hashes(), before, 'sources before/after native execution')
        equal(verify_package(), (package, wordpress), 'native package before/after execution')
        require(str(ROOT) not in json.dumps(observed) and ROOT.as_posix() not in json.dumps(observed),
                'absolute workspace path in public semantic observation')
        report['sources_unchanged_after_execution'] = report['package_unchanged_after_execution'] = True
    finally:
        native.owned_probe.reap_owned_probes()
        owned_path(directory).rmdir()  # Empty owned directory only; no recursive deletion or unknown files.
        report['owned_scratch_reclaimed'] = True


def main():
    require(sys.argv[1:] in ([], ['--record']), 'usage: python native_contract.py [--record]')
    report = {
        'format_version': 1, 'schema': 'recastpress.native-fiber-reference.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'role': 'native PHP comparison oracle only; never a Wasm or production fallback',
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'outcome': 'not_run', 'native_contract_passed': False, 'owned_host_reaped': False,
        'owned_process_cleanup_completed': False, 'wasm_executed': False, 'patched_fiber_backend_executed': False,
        'php_wasm_fiber_compatibility_verified': False, 'product_runtime_selected': False,
        'wordpress_executed': False, 'native_oom_executed': False, 'memory_limit_verified': False,
        'fiber_stack_bytes': STACK_BYTES, 'fiber_stack_setting_scope': 'explicit experiment setting, not product default',
        'outer_execution_timeout_seconds': 10, 'stdout_limit_bytes': 65536, 'stderr_limit_bytes': 65536,
        'stdin': 'closed', 'expected_case_names': CASES,
        'limitations': ['Native Windows PHP 8.5.12 only; no Wasm or patched Zend backend is executed.',
                        'No exception stack traces, object IDs or absolute source locations are recorded.',
                        'GC count and error messages are exact observations of this closed fixture/build.',
                        'Capture size is checked after exit; no native process memory or CPU budget is established.'],
    }
    passed = False
    try:
        observe(report)
        report['outcome'], report['native_contract_passed'], passed = 'native_fiber_contract_passed', True, True
    except subprocess.TimeoutExpired:
        report['outcome'], report['diagnostics'] = 'native_execution_timeout', 'owned native PHP exceeded ten seconds'
    except (Exception, KeyboardInterrupt) as failure:
        report['outcome'], report['diagnostics'] = 'native_fiber_contract_failed', public_message(failure)
    finally:
        try:
            native.owned_probe.reap_owned_probes()
            report['owned_process_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as failure:
            report['outcome'], report['diagnostics'] = 'cleanup_failed', public_message(failure)
            report['native_contract_passed'], passed = False, False
    if sys.argv[1:] == ['--record']:
        require(DESTINATION.resolve().is_relative_to(HERE) and not DESTINATION.is_symlink(), 'receipt escaped experiment')
        DESTINATION.parent.mkdir(exist_ok=True)
        DESTINATION.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('Native PHP Fiber reference: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
