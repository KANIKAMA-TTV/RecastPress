"""Pinned native CGI reference for the guard-adapted six-case PHP Fiber fixture.

No Wasm, patched Zend backend, listener or product fallback is executed. The
native executable and all 78 distribution files are checked against the same
pinned archive before and after this one owned, ten-second CGI process.
"""
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
import uuid

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
CLI_HELPER = ROOT / 'experiments/php-wasix-fiber-probe/native_contract.py'
_spec = importlib.util.spec_from_file_location('recastpress_fiber_cgi_native_cli_reference', CLI_HELPER)
fiber = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(fiber)
native = fiber.native
require, equal, digest = fiber.require, fiber.equal, fiber.digest
CANONICAL_FIXTURE = fiber.FIXTURE
CANONICAL_SHA256 = '9da4b5ffb45677eaa0e81ddbac8c101c692663bf685bbc1157a4689c61c17831'
FIXTURE = HERE / 'fixtures/fiber_cgi_contract.php'
CANONICAL_GUARD = b"if (($_SERVER['argc'] ?? 0) !== 1) {"
CGI_GUARD = (b"if (PHP_SAPI !== 'cgi-fcgi'\n"
             b"    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'\n"
             b"    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_fiber_cgi=1'\n"
             b"    || ($_SERVER['argc'] ?? 0) !== 0) {")
CASES = fiber.CASES
STACK_BYTES = fiber.STACK_BYTES
BASE = ROOT / 'target/php-wasix-fiber-cgi-probe/native-contract'
DESTINATION = HERE / 'evidence/native-cgi-fiber-contract.json'
CGI_EXE_SHA256 = 'f6007e19a06eddaa6c275539f8c8a4b34991bd8b3db061eb965b40e372dfb4a3'
PACKAGE_MAP_SHA256 = '1711770af256947abd7aded6d89b7f32426e0a3a0f80632dcf5dbc8973892d3b'
MANIFEST_SHA256 = '159a065582fef2e837b2a67f02b9b4cd5c3b7397f644b95fd718a3d038696137'


def command_arguments():
    return ['-n', '-d', 'fiber.stack_size=' + str(STACK_BYTES), '-d', 'register_argc_argv=0']


def limitations():
    return [
        'Native Windows CGI PHP 8.5.12 only; no Wasm or patched Zend backend is executed.',
        'Only the fixed request guard differs from the canonical CLI corpus; all six Fiber cases are byte-identical.',
        'The fixed query contains an equals sign so Windows CGI processes -n/-d; register_argc_argv=0 avoids deprecated query-to-argv derivation.',
        'GC count, header defaults and error messages are exact observations of this closed build/config.',
        'Capture size is checked after exit; no native process memory or CPU budget is established.',
        'No exception stack traces, object IDs or absolute source locations are published.',
    ]


def fixture_derivation():
    return {'canonical_path': CANONICAL_FIXTURE.relative_to(ROOT).as_posix(),
            'canonical_bytes': CANONICAL_FIXTURE.stat().st_size, 'canonical_sha256': CANONICAL_SHA256,
            'change': 'fixed CGI request entry guard only', 'all_six_case_bodies_byte_identical': True,
            'verified_before_and_after_execution': True}


def verify_fixture():
    """The request guard is the sole change to the pinned six-case corpus."""
    canonical = CANONICAL_FIXTURE.read_bytes()
    equal(hashlib.sha256(canonical).hexdigest(), CANONICAL_SHA256, 'fixed original CLI Fiber corpus')
    require(canonical.count(CANONICAL_GUARD) == 1, 'canonical Fiber entry guard count')
    equal(FIXTURE.read_bytes(), canonical.replace(CANONICAL_GUARD, CGI_GUARD, 1),
          'CGI Fiber corpus differs outside the sole fixed request guard')


def expected_observation():
    return {
        'schema': 'recastpress.php-fiber-contract.v0.5', 'ok': True,
        'runtime': {'version': '8.5.12', 'version_id': 80512, 'int_size': 8,
                    'sapi': 'cgi-fcgi', 'fiber_stack_size': str(STACK_BYTES)},
        'cases': {name: {'status': 'ok', 'value': value, 'warnings': []}
                  for name, value in fiber.expected_values().items()},
        'unexpected_output_hex': '',
    }


def expected_headers():
    return [{'name': 'x-powered-by', 'value_hex': b'PHP/8.5.12'.hex()},
            {'name': 'content-type', 'value_hex': b'text/html; charset=UTF-8'.hex()}]


def validate_observation(observation):
    equal(observation, expected_observation(), 'fixed native CGI Fiber semantics')


def expected_environment(script_filename='/fixture/fiber_cgi_contract.php'):
    return [
        ['REDIRECT_STATUS', '200'], ['GATEWAY_INTERFACE', 'CGI/1.1'],
        ['SERVER_PROTOCOL', 'HTTP/1.1'], ['SERVER_SOFTWARE', 'recastpress-p0-fiber-cgi'],
        ['SERVER_NAME', 'localhost'], ['SERVER_PORT', '80'], ['HTTP_HOST', 'localhost'],
        ['REMOTE_ADDR', '127.0.0.1'], ['REQUEST_METHOD', 'GET'],
        ['REQUEST_URI', '/__rp_fiber.php?rp_fiber_cgi=1'], ['SCRIPT_NAME', '/__rp_fiber.php'],
        ['SCRIPT_FILENAME', script_filename], ['DOCUMENT_ROOT', '/fixture'],
        ['QUERY_STRING', 'rp_fiber_cgi=1'], ['CONTENT_LENGTH', '0'],
    ]


def parse_completed_cgi(wire):
    """Parse only this closed, completed 200 response; never an HTTP gateway."""
    require(type(wire) is bytes and 0 < len(wire) <= 65536, 'native CGI Fiber wire budget')
    boundary = wire.find(b'\r\n\r\n')
    require(0 < boundary <= 16380, 'native CGI Fiber CRLF header terminator/budget')
    rows, body = wire[:boundary].split(b'\r\n'), wire[boundary + 4:]
    require(len(rows) <= 64, 'native CGI Fiber header field budget')
    fields = []
    for row in rows:
        require(row and row[:1] not in (b' ', b'\t') and b':' in row, 'invalid/folded native CGI header')
        name, value = row.split(b':', 1)
        require(re.fullmatch(rb"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name) is not None,
                'invalid native CGI header name')
        value = value.strip(b' \t')
        require(all(x == 9 or 32 <= x != 127 for x in value), 'native CGI header control byte')
        fields.append({'name': name.decode('ascii').lower(), 'value_hex': value.hex()})
    # The unchanged fixture emits no explicit Status or Content-Length. This
    # exact header list is an observation of the fixed PHP CGI build/config.
    equal(fields, expected_headers(), 'native CGI Fiber fixed response headers')
    response = {'status': 200, 'headers': fields, 'body_bytes': len(body),
                'body_hex': body.hex(), 'body_sha256': hashlib.sha256(body).hexdigest(),
                'representation_length': None}
    return response, body, fields


def source_hashes():
    verify_fixture()
    paths = [Path(__file__).resolve(), CLI_HELPER, CANONICAL_FIXTURE, FIXTURE, fiber.COMMON,
             ROOT / 'scripts/owned_probe.py', native.WP_MANIFEST]
    for path in paths:
        require(path.resolve(strict=True) == path and path.is_file(), 'linked or missing fixed CGI Fiber input')
    return {path.relative_to(ROOT).as_posix(): digest(path) for path in paths}


def validate_receipt(oracle):
    """Read-only Linux-safe receipt gate; never verifies or executes native PHP.

    The historical full-package check is bound by fixed manifest, archive and
    canonical 78-file-map hashes. It is a source-bound observation, not a new
    claim that the native Windows files exist on the validating Linux host.
    """
    require(type(oracle) is dict, 'native CGI Fiber receipt object required')
    fixed = {
        'format_version': 1, 'schema': 'recastpress.php-wasix-fiber-cgi-native.v0.5',
        'role': 'native CGI PHP comparison oracle only; never a Wasm or production fallback',
        'platform': {'system': 'Windows', 'machine': 'AMD64'},
        'outcome': 'native_cgi_fiber_contract_passed', 'native_contract_passed': True,
        'owned_host_reaped': True, 'owned_process_cleanup_completed': True,
        'owned_scratch_reclaimed': True, 'stdout_eof': True, 'stderr_eof': True,
        'wasm_executed': False, 'patched_fiber_backend_executed': False,
        'php_wasm_fiber_compatibility_verified': False, 'product_runtime_selected': False,
        'wordpress_executed': False, 'native_cli_substitute_used': False, 'listener_created': False,
        'native_oom_executed': False, 'memory_limit_verified': False,
        'fiber_stack_bytes': STACK_BYTES,
        'fiber_stack_setting_scope': 'explicit experiment setting, not product default',
        'outer_execution_timeout_seconds': 10, 'stdout_limit_bytes': 65536, 'stderr_limit_bytes': 65536,
        'stdin': 'closed empty CGI request body', 'expected_case_names': CASES,
        'exit_code': 0, 'stderr_bytes': 0,
        'stderr_sha256': hashlib.sha256(b'').hexdigest(),
        'sources_unchanged_after_execution': True, 'package_unchanged_after_execution': True,
        'limitations': limitations(), 'fixture_derivation': fixture_derivation(),
        'input_files': source_hashes(),
        'fixture': {'path': FIXTURE.relative_to(ROOT).as_posix(), 'bytes': FIXTURE.stat().st_size,
                    'sha256': digest(FIXTURE), 'modified': False},
        'native_executable': {'path': '.reference/php-8.5.12/php-cgi.exe', 'sha256': CGI_EXE_SHA256},
        'command': ['.reference/php-8.5.12/php-cgi.exe'] + command_arguments(),
    }
    for name, value in fixed.items():
        require(name in oracle, 'native CGI Fiber receipt lacks ' + name)
        equal(oracle[name], value, 'native CGI Fiber receipt.' + name)
    equal(set(oracle), set(fixed) | {
        'observed_at_utc', 'native_reference', 'wordpress_verifier_side_check', 'environment',
        'owned_process_pid', 'stdout_bytes', 'stdout_sha256', 'observation', 'response',
        'cgi_wire_headers', 'stdout_hex'}, 'native CGI Fiber receipt fields')
    require(type(oracle.get('owned_process_pid')) is int and oracle['owned_process_pid'] > 0,
            'native CGI Fiber positive owned PID required')
    timestamp = oracle.get('observed_at_utc')
    require(type(timestamp) is str and len(timestamp) <= 128, 'native CGI Fiber UTC timestamp')
    require(datetime.fromisoformat(timestamp).utcoffset().total_seconds() == 0, 'native CGI Fiber non-UTC timestamp')
    package = oracle.get('native_reference')
    require(type(package) is dict, 'native CGI Fiber package reference')
    equal(set(package), {'manifest', 'manifest_sha256', 'version', 'archive_sha256', 'verified_files',
                         'distribution_gate'}, 'native CGI Fiber package fields')
    for name, value in {
        'manifest': '.reference/php-8.5.12/recastpress-tool.json', 'manifest_sha256': MANIFEST_SHA256,
        'version': '8.5.12', 'archive_sha256': native.PHP_ARCHIVE_SHA256,
        'distribution_gate': {
            'manifest_files': 78, 'extracted_file_set_verified': True,
            'all_78_extracted_hashes_verified': True, 'archive_bytes': 36306925,
            'archive_sha256_verified': True, 'archive_member_file_set_verified': True,
            'all_78_archive_member_hashes_match_extracted_files': True,
        },
    }.items():
        equal(package.get(name), value, 'native CGI Fiber package.' + name)
    files = package['verified_files']
    require(type(files) is dict and len(files) == 78, 'native CGI Fiber full package file map')
    canonical = json.dumps(files, sort_keys=True, separators=(',', ':')).encode('utf-8')
    equal(hashlib.sha256(canonical).hexdigest(), PACKAGE_MAP_SHA256, 'native CGI Fiber exact 78-file package map')
    equal(files['php-cgi.exe'], CGI_EXE_SHA256, 'native CGI Fiber mapped executable')
    wordpress = native.parse_json(native.WP_MANIFEST.read_bytes())
    equal(oracle.get('wordpress_verifier_side_check'), {
        'source_side_check_only': True, 'executed': False, 'version': '7.1.3',
        'manifest_sha256': digest(native.WP_MANIFEST), 'verified_source_files': wordpress['files'],
    }, 'native CGI Fiber WordPress verifier side check only')
    keys = sorted({'SystemRoot', 'WINDIR', 'TEMP', 'TMP'} | {key for key, _ in expected_environment()})
    equal(oracle.get('environment'), {
        'provided_keys': keys, 'absolute_values_recorded': False,
        'php_configuration_environment_forwarded': False, 'credentials_forwarded': False,
        'normalized_cgi_rows': expected_environment('<owned-fixture>'),
        'script_filename_record_kind': 'owned verified source copy; absolute native value omitted',
    }, 'native CGI Fiber fixed environment')
    wire_hex = oracle.get('stdout_hex')
    require(type(wire_hex) is str and 0 < len(wire_hex) <= 131072 and len(wire_hex) % 2 == 0
            and re.fullmatch('[0-9a-f]+', wire_hex) is not None, 'native CGI Fiber hex capture')
    wire = bytes.fromhex(wire_hex)
    equal(oracle.get('stdout_bytes'), len(wire), 'native CGI Fiber stdout length')
    equal(oracle.get('stdout_sha256'), hashlib.sha256(wire).hexdigest(), 'native CGI Fiber stdout hash')
    response, body, fields = parse_completed_cgi(wire)
    equal(oracle.get('response'), response, 'native CGI Fiber response/wire identity')
    equal(oracle.get('cgi_wire_headers'), fields, 'native CGI Fiber header/wire identity')
    require(body.endswith(b'\n') and body.count(b'\n') == 1, 'native CGI Fiber terminated single JSON body')
    observed = native.parse_json(body)
    validate_observation(observed)
    equal(oracle.get('observation'), observed, 'native CGI Fiber decoded body identity')
    return {'schema': oracle['schema'], 'native_contract_passed': True,
            'expected_case_names': CASES, 'source_input_count': len(fixed['input_files']),
            'native_execution_performed_by_validator': False}


def owned_path(path):
    require(BASE.resolve() == BASE and BASE.is_relative_to(ROOT / 'target'), 'owned native CGI Fiber base escaped target')
    require(path.is_relative_to(BASE) and path.resolve() == path, 'owned native CGI Fiber path escaped target')
    return path


def public_message(value):
    return str(value).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')[:8192]


def observe(report):
    require(os.name == 'nt', 'native CGI Fiber reference pins Windows x64 PHP only')
    before = source_hashes()
    report['input_files'] = before
    package, wordpress = fiber.verify_package()
    report['native_reference'], report['wordpress_verifier_side_check'] = package, wordpress
    executable = native.PHP_ROOT / 'php-cgi.exe'
    equal(digest(executable), package['verified_files']['php-cgi.exe'], 'native CGI executable')
    report['native_executable'] = {'path': executable.relative_to(ROOT).as_posix(),
                                   'sha256': package['verified_files']['php-cgi.exe']}
    fixture_bytes = FIXTURE.read_bytes()
    report['fixture'] = {'path': FIXTURE.relative_to(ROOT).as_posix(), 'bytes': len(fixture_bytes),
                         'sha256': hashlib.sha256(fixture_bytes).hexdigest(), 'modified': False}
    owned_path(BASE).mkdir(parents=True, exist_ok=True)
    directory = BASE / (str(os.getpid()) + '-' + uuid.uuid4().hex)
    owned_path(directory).mkdir(exist_ok=False)
    fixture = directory / 'fiber_cgi_contract.php'
    created = False
    report['owned_scratch_reclaimed'] = False
    try:
        with owned_path(fixture).open('xb') as destination:
            created = True
            destination.write(fixture_bytes)
        equal(digest(owned_path(fixture)), report['fixture']['sha256'], 'owned Fiber fixture copy')
        system_root = os.environ.get('SystemRoot') or os.environ.get('SYSTEMROOT')
        require(system_root and Path(system_root).is_absolute(), 'Windows SystemRoot is missing')
        scaffold = {'SystemRoot': system_root, 'WINDIR': system_root,
                    'TEMP': str(directory), 'TMP': str(directory)}
        environment = scaffold | dict(expected_environment(fixture.as_posix()))
        report['environment'] = {
            'provided_keys': sorted(environment), 'absolute_values_recorded': False,
            'php_configuration_environment_forwarded': False, 'credentials_forwarded': False,
            'normalized_cgi_rows': expected_environment('<owned-fixture>'),
            'script_filename_record_kind': 'owned verified source copy; absolute native value omitted',
        }
        arguments = command_arguments()
        report['command'] = [executable.relative_to(ROOT).as_posix()] + arguments
        pid, result = native.owned_probe.run_captured_probe([str(executable)] + arguments,
                                                           environment, timeout=10, input_bytes=b'')
        report.update(owned_process_pid=pid, exit_code=result.returncode, owned_host_reaped=True,
                      stdout_eof=True, stderr_eof=True)
        for name, wire in [('stdout', result.stdout), ('stderr', result.stderr)]:
            report[name + '_bytes'], report[name + '_sha256'] = len(wire), hashlib.sha256(wire).hexdigest()
        require(0 < len(result.stdout) <= 65536 and len(result.stderr) <= 65536, 'native CGI Fiber capture budget')
        require(result.returncode == 0 and result.stderr == b'', 'native CGI Fiber exit or stderr failure')
        response, body, fields = parse_completed_cgi(result.stdout)
        require(body.endswith(b'\n') and body.count(b'\n') == 1, 'one terminated native CGI Fiber JSON required')
        observed = native.parse_json(body)
        validate_observation(observed)
        require(str(ROOT) not in json.dumps(observed) and ROOT.as_posix() not in json.dumps(observed),
                'absolute native source path in CGI Fiber observation')
        report['observation'], report['response'], report['cgi_wire_headers'] = observed, response, fields
        report['stdout_hex'] = result.stdout.hex()
        equal(source_hashes(), before, 'CGI Fiber sources before/after execution')
        equal(fiber.verify_package(), (package, wordpress), 'native CGI Fiber package before/after execution')
        equal(digest(owned_path(fixture)), report['fixture']['sha256'], 'owned Fiber fixture unchanged')
        report['sources_unchanged_after_execution'] = report['package_unchanged_after_execution'] = True
        report['fixture_derivation'] = fixture_derivation()
    finally:
        native.owned_probe.reap_owned_probes()
        if created and fixture.exists():
            owned_path(fixture).unlink()
        owned_path(directory).rmdir()
        report['owned_scratch_reclaimed'] = True


def main():
    require(sys.argv[1:] in ([], ['--record']), 'usage: python native_contract.py [--record]')
    report = {
        'format_version': 1, 'schema': 'recastpress.php-wasix-fiber-cgi-native.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'role': 'native CGI PHP comparison oracle only; never a Wasm or production fallback',
        'platform': {'system': platform.system(), 'machine': platform.machine()},
        'outcome': 'not_run', 'native_contract_passed': False, 'owned_host_reaped': False,
        'owned_process_cleanup_completed': False, 'wasm_executed': False,
        'patched_fiber_backend_executed': False, 'php_wasm_fiber_compatibility_verified': False,
        'product_runtime_selected': False, 'wordpress_executed': False,
        'native_cli_substitute_used': False, 'listener_created': False,
        'native_oom_executed': False, 'memory_limit_verified': False,
        'fiber_stack_bytes': STACK_BYTES,
        'fiber_stack_setting_scope': 'explicit experiment setting, not product default',
        'outer_execution_timeout_seconds': 10, 'stdout_limit_bytes': 65536, 'stderr_limit_bytes': 65536,
        'stdin': 'closed empty CGI request body', 'expected_case_names': CASES,
        'limitations': limitations(),
    }
    passed = False
    try:
        observe(report)
        report['outcome'], report['native_contract_passed'], passed = 'native_cgi_fiber_contract_passed', True, True
    except subprocess.TimeoutExpired:
        report['outcome'], report['diagnostics'] = 'native_execution_timeout', 'owned native CGI exceeded ten seconds'
    except (Exception, KeyboardInterrupt) as failure:
        report['outcome'], report['diagnostics'] = 'native_cgi_fiber_contract_failed', public_message(failure)
    finally:
        try:
            native.owned_probe.reap_owned_probes()
            report['owned_process_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as failure:
            report['outcome'], report['diagnostics'] = 'cleanup_failed', public_message(failure)
            report['native_contract_passed'], passed = False, False
    if passed:
        try:
            validate_receipt(report)
        except (Exception, KeyboardInterrupt) as failure:
            report['outcome'], report['diagnostics'] = 'native_receipt_validation_failed', public_message(failure)
            report['native_contract_passed'], passed = False, False
    if sys.argv[1:] == ['--record']:
        require(DESTINATION.resolve().is_relative_to(HERE) and not DESTINATION.is_symlink(), 'native CGI receipt escaped experiment')
        DESTINATION.parent.mkdir(exist_ok=True)
        DESTINATION.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('Native PHP CGI Fiber reference: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
