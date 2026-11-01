"""Observe three fixed requests with pinned Windows PHP CGI as an oracle.

This is a native comparison reference only, never a product or Wasm fallback.
The same PHP fixture is copied into a fresh owned target directory. No listener,
arbitrary script, request, environment mapping or native CLI substitute is used.
"""
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import sys
import time
import uuid

import cgi_check as contract

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
sys.path.insert(0, str(ROOT / 'experiments/php-runtime-candidate-probe'))
import native_contract as native
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

BASE = ROOT / 'target/php-wasix-host-probe/native-cgi-contract'
DESTINATION = HERE / 'evidence/native-cgi-contract.json'
require, equal, digest = native.require, native.equal, native.digest


def owned_path(path):
    require(BASE.resolve() == BASE and BASE.is_relative_to(ROOT / 'target'), 'owned native CGI base escaped workspace')
    require(path.is_relative_to(BASE) and path.resolve() == path, 'owned native CGI path escaped fixed target')
    return path


def parse_completed_cgi(wire):
    """Closed native-oracle framing checks; not a reusable HTTP/CGI gateway.

    Only the expected 200/404 fixtures are accepted. Original value bytes and
    duplicate fields survive. CGI status/length stay out of transport headers.
    """
    require(0 < len(wire) <= 65536, 'native CGI output capture budget')
    boundary = wire.find(b'\r\n\r\n')
    separator = b'\r\n'
    if boundary == -1:
        boundary = wire.find(b'\n\n')
        separator = b'\n'
    require(boundary >= 0 and boundary + 2 * len(separator) <= 16384, 'CGI header terminator/budget')
    head, body = wire[:boundary], wire[boundary + 2 * len(separator):]
    rows = head.split(separator)
    require(len(rows) <= 64, 'CGI header field budget')
    fields = []
    for row in rows:
        require(row and row[:1] not in (b' ', b'\t') and b':' in row, 'invalid/folded CGI header')
        name, value = row.split(b':', 1)
        require(re.fullmatch(rb"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name) is not None, 'invalid CGI header name')
        value = value.strip(b' \t')
        require(all(x == 9 or 32 <= x != 127 for x in value), 'CGI header control/mixed line ending')
        name = name.decode('ascii').lower()
        require(name not in {'connection', 'keep-alive', 'proxy-connection', 'te', 'trailer',
                             'transfer-encoding', 'upgrade'}, 'hop-by-hop CGI field')
        fields.append((name, value))
    values = lambda name: [value for key, value in fields if key == name]
    statuses, lengths = values('status'), values('content-length')
    require(len(statuses) <= 1 and len(lengths) == 1, 'duplicate/missing CGI status/length')
    status = 200
    if statuses:
        require(re.fullmatch(rb'[2-5][0-9]{2}(?: [^\r\n]*)?', statuses[0]) is not None, 'invalid CGI status')
        status = int(statuses[0][:3])
    require(status in (200, 404), 'unexpected fixed CGI status')
    require(re.fullmatch(rb'[0-9]+', lengths[0]) is not None and int(lengths[0]) == len(body),
            'CGI Content-Length differs from completed body')
    require(len(values('content-type')) == 1, 'missing/duplicate CGI content type')
    headers = [{'name': name, 'value_hex': value.hex()} for name, value in fields
               if name not in {'status', 'content-length'}]
    response = {'status': status, 'headers': headers, 'body_bytes': len(body),
                'body_hex': body.hex(), 'body_sha256': hashlib.sha256(body).hexdigest(),
                'representation_length': None}
    return response, body, fields


def validate_response(wire, index, script_filename):
    response, body, fields = parse_completed_cgi(wire)
    values = lambda name: [value for key, value in fields if key == name]
    equal(response['status'], 404 if index == 2 else 200, 'native CGI status')
    equal(values('content-type'), [b'application/octet-stream' if index == 2 else b'application/json'], 'native content type')
    cookies = [b'wp_a=one; path=/; HttpOnly', b'wp_b=two; path=/; SameSite=Lax'] if index == 2 else []
    equal(values('set-cookie'), cookies, 'native separate cookie fields')
    observed = {'response': response,
                'wire_headers': [{'name': name, 'value_hex': value.hex()} for name, value in fields]}
    if index == 2:
        equal(body, b'\0\xffCGI\r\n', 'native binary body')
    else:
        php = native.parse_json(body)
        expected = contract.expected_php(index)
        expected['script_filename'] = script_filename
        equal(php, expected, 'native CGI request/runtime observation')
        normalized = dict(php)
        normalized['script_filename'] = '/fixture/cgi_contract.php'
        equal(normalized, contract.expected_php(index), 'normalized CGI comparison')
        observed['php'] = php
        observed['comparison_php'] = normalized
    return observed


def observe(report):
    require(os.name == 'nt', 'native CGI oracle pins Windows x64 only')
    reference, wordpress = native.verify_references()
    require('php-cgi.exe' in reference['verified_files'], 'native distribution has no verified CGI executable')
    executable = native.PHP_ROOT / 'php-cgi.exe'
    equal(digest(executable), reference['verified_files']['php-cgi.exe'], 'native CGI executable')
    report['native_reference'] = reference
    report['wordpress_reference'] = {'manifest_sha256': digest(native.WP_MANIFEST),
                                      'version': wordpress['wordpress_version'], 'executed': False}
    equal(digest(contract.FIXTURE), contract.FIXTURE_SHA, 'original CGI fixture')
    fixture_bytes = contract.FIXTURE.read_bytes()
    equal(hashlib.sha256(fixture_bytes).hexdigest(), contract.FIXTURE_SHA, 'fixture snapshot')
    owned_path(BASE).mkdir(parents=True, exist_ok=True)
    directory = BASE / (str(os.getpid()) + '-' + uuid.uuid4().hex)
    owned_path(directory).mkdir(exist_ok=False)
    fixture = directory / 'cgi_contract.php'
    created = False
    try:
        with owned_path(fixture).open('xb') as destination:
            created = True
            destination.write(fixture_bytes)
        equal(digest(owned_path(fixture)), contract.FIXTURE_SHA, 'owned fixture copy')
        # Windows needs its system location for process activation; all CGI
        # variables are constructed below. No PATH, PHP config or credentials
        # are inherited. TEMP/TMP, if consulted, stay in the owned directory.
        system_root = os.environ.get('SystemRoot') or os.environ.get('SYSTEMROOT')
        require(system_root and Path(system_root).is_absolute(), 'Windows SystemRoot missing')
        scaffold = {'SystemRoot': system_root, 'WINDIR': system_root,
                    'TEMP': str(directory), 'TMP': str(directory)}
        script_filename = fixture.as_posix()
        report['fixture_copy'] = {'path': fixture.relative_to(ROOT).as_posix(),
                                  'sha256': contract.FIXTURE_SHA, 'bytes': len(fixture_bytes)}
        report['comparison_normalization'] = {
            'field': 'script_filename', 'native_public_value': '<owned-fixture>',
            'guest_exact_value': '/fixture/cgi_contract.php',
            'original_value_stored': False,
            'reason': 'identical verified source bytes reside at different native and virtual paths; all other PHP fields compare exactly'}
        report['native_environment_scaffold'] = {
            'provided_keys': sorted(scaffold), 'absolute_values_recorded': False,
            'temp_scope': 'fresh owned target directory'}
        for index, name in enumerate(contract.NAMES):
            env_rows = contract.expected_environment(index)
            env_rows = [[key, script_filename if key == 'SCRIPT_FILENAME' else value] for key, value in env_rows]
            environment = scaffold | dict(env_rows)
            body = contract.POST if index == 1 else b''
            row = {'name': name, 'request_number': index + 1, 'completed': False,
                   'normalized_environment': [[k, '<owned-fixture>' if k == 'SCRIPT_FILENAME' else v] for k, v in env_rows],
                   'environment_record_kind': 'SCRIPT_FILENAME-redacted; other CGI values unchanged', 'stdin_hex': body.hex(),
                   'stdin_bytes': len(body), 'command_arguments': ['-n']}
            report['requests'].append(row)
            started = time.monotonic()
            pid, completed = owned_probe.run_captured_probe([str(executable), '-n'], environment,
                                                           timeout=10, input_bytes=body)
            row.update({'owned_process_pid': pid, 'exit_code': completed.returncode,
                        'elapsed_ms': round((time.monotonic() - started) * 1000),
                        'owned_process_reaped': True, 'stdout_eof': True, 'stderr_eof': True,
                        'stdout_bytes': len(completed.stdout), 'stderr_bytes': len(completed.stderr)})
            require(len(completed.stdout) <= 65536 and len(completed.stderr) <= 65536, 'native CGI capture budget')
            row.update({'stdout_sha256': hashlib.sha256(completed.stdout).hexdigest(),
                        'stderr_sha256': hashlib.sha256(completed.stderr).hexdigest()})
            require(completed.returncode == 0, 'native CGI exited unsuccessfully')
            require(completed.stderr == b'', 'native CGI emitted stderr')
            observed = validate_response(completed.stdout, index, script_filename)
            if index == 2:
                # The validated binary response contains no native file path.
                row['stdout_hex'] = completed.stdout.hex()
                row['stdout_record_kind'] = 'original-bytes'
                observed['response']['body_record_kind'] = 'original-bytes'
            else:
                # Raw CGI JSON contains the private absolute SCRIPT_FILENAME.
                # Retain its original digests/lengths without publishing bytes.
                # The public PHP value is explicitly labelled as normalized.
                observed['normalized_native_php'] = observed.pop('php')
                observed['normalized_native_php']['script_filename'] = '<owned-fixture>'
                observed['response'].pop('body_hex')
                observed['response']['body_record_kind'] = 'original-digest-and-length-only'
                row['stdout_record_kind'] = 'original-digest-and-length-only'
            row.update(observed)
            equal(digest(owned_path(fixture)), contract.FIXTURE_SHA, 'fixture unchanged after native CGI')
            row['completed'] = True
    finally:
        # Never recursively remove this directory or delete unknown entries.
        # Stop/reap the captured child before removing its known fixture file.
        owned_probe.reap_owned_probes()
        report['owned_process_cleanup_completed'] = True
        if created and fixture.exists():
            owned_path(fixture).unlink()
        owned_path(directory).rmdir()
        report['owned_fixture_cleanup_completed'] = True


def main():
    require(sys.argv[1:] in ([], ['--record']), 'usage: native_cgi_contract.py [--record]')
    report = {'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'role': 'native CGI comparison oracle only; never a Wasm or product fallback',
              'outcome': 'not_run', 'wasm_executed': False, 'wordpress_executed': False,
              'native_cli_substitute_used': False, 'listener_created': False,
              'native_reference_passed': False, 'owned_process_cleanup_completed': False,
              'owned_fixture_cleanup_completed': False, 'requests': [],
              'platform': {'system': platform.system(), 'machine': platform.machine()},
              'request_timeout_seconds': 10,
              'limitations': ['Output limits are checked after the owned process exits; no OS RSS limit is established.',
                              'Native Windows CGI only; this oracle does not establish Wasm compatibility.',
                              'Only SCRIPT_FILENAME is normalized for comparison; JSON captures publish original SHA/length and labelled normalized values.',
                              'Original JSON capture bytes and absolute native filesystem/environment values are not retained in this public receipt.']}
    passed = False
    try:
        observe(report)
        equal(len(report['requests']), 3, 'native request count')
        require(all(row['completed'] for row in report['requests']), 'native CGI request incomplete')
        report['outcome'], report['native_reference_passed'], passed = 'native_cgi_contract_passed', True, True
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = 'native_cgi_contract_failed'
        report['diagnostics'] = str(error).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            passed = False
            report['native_reference_passed'] = False
            report['outcome'] = 'owned_cleanup_failed'
            report['cleanup_diagnostics'] = str(error).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')[:8192]
    inputs = [Path(__file__).resolve(), contract.FIXTURE, Path(contract.__file__).resolve(),
              HERE / 'check.py', HERE / 'language_check.py', Path(native.__file__).resolve(),
              ROOT / 'scripts/owned_probe.py', native.WP_MANIFEST,
              contract.BUILD / 'build.py', contract.BUILD / 'prepare.py', contract.BUILD / 'patch_source.py']
    report['input_files'] = {path.relative_to(ROOT).as_posix(): digest(path) for path in inputs}
    if sys.argv[1:] == ['--record']:
        require(DESTINATION.resolve().is_relative_to(ROOT), 'native evidence destination escaped workspace')
        DESTINATION.parent.mkdir(exist_ok=True)
        DESTINATION.write_text(json.dumps(report, ensure_ascii=True, indent=2) + '\n', encoding='utf-8', newline='\n')
    print('Native PHP CGI oracle: ' + report['outcome'] + '; Wasm was not executed.')
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
