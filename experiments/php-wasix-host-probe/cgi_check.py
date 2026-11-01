"""Verify this job's generated CGI candidate and supervise three fixed requests.

No CLI artifact, native PHP, arbitrary path/code/query or binary upload is used.
The receipt is same-job provenance, not a signed or reproducible-build claim.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys

import check as host_probe
from language_check import equal, parse_json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
BUILD = ROOT / 'experiments/php-wasix-cgi-build'
WORK = ROOT / 'target/php-wasix-cgi-build'
MODULE = WORK / 'source/php/sapi/cgi/php-cgi.wasm'
RECEIPT = BUILD / 'evidence/build.json'
FIXTURE = HERE / 'fixtures/cgi_contract.php'
FIXTURE_SHA = 'ac0e2fb549a7fb0c8620effcbe7cc40dd84f171f0b6dfad639f1fafb65e51bc0'
NATIVE_BASELINE = HERE / 'evidence/native-cgi-contract.json'
NATIVE_ARCHIVE_SHA = '2ab242af38d76bdf417cde5ec048b1bcd76a5a40f39a3d15d5f046f38571c477'
NATIVE_MANIFEST_SHA = '159a065582fef2e837b2a67f02b9b4cd5c3b7397f644b95fd718a3d038696137'
NATIVE_FILE_MAP_SHA = '1711770af256947abd7aded6d89b7f32426e0a3a0f80632dcf5dbc8973892d3b'
NATIVE_CGI_SHA = 'f6007e19a06eddaa6c275539f8c8a4b34991bd8b3db061eb965b40e372dfb4a3'
POST = b'title=RP%20CGI&count=2&empty='
QUERIES = ['case=get&value=hello%20wasix', 'case=post&source=fixed', 'case=metadata']
NAMES = ['get_cookie', 'post_raw_body', 'status_cookies_binary']
require, digest = host_probe.require, host_probe.digest
sys.path.insert(0, str(BUILD))
import build as cgi_build
import prepare as cgi_prepare
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe


def fixed_file(path, limit):
    require(path.resolve(strict=True) == ROOT.resolve() / path.relative_to(ROOT),
            'fixed input resolves through symlink')
    require(path.is_file() and 0 < path.stat().st_size <= limit, 'fixed file size/type: ' + path.name)
    return path


def read_json(path, limit=16 * 1024 * 1024):
    wire = fixed_file(path, limit).read_bytes()
    require(0 < len(wire) <= limit, 'JSON input grew past budget')
    value = parse_json(wire)
    require(type(value) is dict, 'JSON input is not an object')
    return value, hashlib.sha256(wire).hexdigest()


def verify_hash_files(base, rows):
    require(type(rows) is dict and rows, 'missing recorded file hashes')
    for name, identity in rows.items():
        path = base.joinpath(*cgi_prepare.relative_parts(name))
        fixed_file(path, 512 * 1024 * 1024)
        expected = identity['sha256'] if type(identity) is dict else identity
        require(digest(path) == expected, 'recorded file changed: ' + name)
        if type(identity) is dict:
            equal(path.stat().st_size, identity['bytes'], 'recorded byte count ' + name)


def verify_build():
    """Do not reapply patches or use post-build clean as a pre-build assertion."""
    manifest = cgi_prepare.load_manifest()
    receipt, receipt_sha = read_json(RECEIPT)
    toolchain, tool_sha = read_json(BUILD / 'evidence/toolchain.json')
    for key, expected in [('format_version', 1), ('outcome', 'cgi_candidate_built_not_executed'),
                          ('phase', 'build_complete'), ('guest_executed', False),
                          ('product_runtime_selected', False), ('native_runtime_fallback_used', False)]:
        equal(receipt[key], expected, 'build ' + key)
    equal(receipt['input_files'], cgi_prepare.input_hashes(), 'build definitions')
    equal(receipt['toolchain_receipt_sha256'], tool_sha, 'toolchain receipt identity')
    equal(toolchain['input_files'], cgi_prepare.input_hashes(), 'tool definitions')
    # Existing verifier rechecks the four pinned asset identities and required
    # extracted binaries, including each wrapper's verified executable target.
    equal(cgi_build.verify_tools(manifest), tool_sha, 'verified toolchain snapshot')
    environment = cgi_prepare.tool_environment()
    equal(set(receipt['sources']), set(manifest['sources']), 'source set')
    for name, identity in manifest['sources'].items():
        directory = cgi_prepare.owned_path(identity['directory'])
        actual = cgi_build.checked_output(['git', 'rev-parse', 'HEAD'], directory, environment)
        equal(actual, identity['commit'], name + ' checkout')
        for field in ['repository', 'commit']:
            equal(receipt['sources'][name][field], identity[field], name + ' ' + field)
        equal(receipt['sources'][name]['tracked_worktree_clean_before_build'], True, name + ' original clean observation')
    php = cgi_prepare.owned_path(manifest['sources']['php']['directory'])
    patches = receipt['source_patches']
    require(type(patches) is list and len(patches) == len(manifest['source_patches']) == 1,
            'unexpected patch set')
    for observed, pin in zip(patches, manifest['source_patches']):
        for field in ['path', 'sha256', 'source_commit', 'target', 'before_bytes', 'before_sha256', 'after_sha256']:
            equal(observed[field], pin[field], 'patch ' + field)
        for field, expected in [('applied', True), ('clean_tracked_source_before_patch', True),
                                ('git_apply_check_exit_code', 0), ('git_apply_apply_exit_code', 0),
                                ('observed_patch_sha256', pin['sha256']),
                                ('observed_before_bytes', pin['before_bytes']),
                                ('observed_before_git_blob_sha1', pin['before_git_blob_sha1']),
                                ('observed_before_sha256', pin['before_sha256']),
                                ('observed_after_sha256', pin['after_sha256'])]:
            equal(observed[field], expected, 'patch observation ' + field)
        target = fixed_file(php.joinpath(*cgi_prepare.relative_parts(pin['target'])), 1024 * 1024)
        equal(digest(target), pin['after_sha256'], 'current patched source')
    expected_difference = {'extra_flags': '--enable-cgi', 'php_source_patch': True,
                          'cli_disabled': False, 'make_target': 'cgi', 'make_jobs': 2,
                          'source_patch_scope': 'WASI FastCGI prefork exclusion only'}
    equal(receipt['configure_difference'], expected_difference, 'configure/source differences')
    equal(receipt['wasi_prefork_support'], 'explicitly-disabled-by-source-patch', 'prefork support')
    controls = receipt['source_control_files'].copy()
    equal(controls.pop('sapi/cgi/cgi_main.c'), manifest['source_patches'][0]['before_sha256'], 'original CGI control source')
    expected_controls = {'wasix-configure-eh-64.sh', 'wasix-configure-eh.sh', 'wasix-build-eh.sh',
                         'sapi/cgi/config9.m4', 'sapi/cgi/Makefile.frag', 'build/gen_stub.php', 'main/php_version.h'}
    equal(set(controls), expected_controls, 'source control files')
    verify_hash_files(php, controls)
    expected_config = {'config.log', 'config.status', 'config.nice', 'Makefile', 'main/php_config.h', 'Zend/zend_config.h'}
    equal(set(receipt['generated_configuration_files']), expected_config, 'generated config set')
    verify_hash_files(php, receipt['generated_configuration_files'])
    verify_hash_files(php, receipt['stub_headers_after'])
    deps = cgi_prepare.owned_path(manifest['sources']['deps']['directory'])
    actual_archives = {p.relative_to(deps).as_posix() for p in (deps / 'lib-eh').glob('*.a')}
    actual_archives |= {p.relative_to(deps).as_posix() for p in (deps / 'pgsql-eh/lib').glob('*.a')}
    archive_rows = receipt['sources']['deps']['prebuilt_archive_files']
    equal(set(archive_rows), actual_archives, 'prebuilt dependency archive set')
    verify_hash_files(deps, archive_rows)
    equal(receipt['sources']['deps']['rebuilt_here'], False, 'dependency rebuild scope')
    steps = receipt['build_steps']
    require(type(steps) is list and len(steps) == 3, 'CGI build step count')
    host_php = Path('/usr/bin/php').resolve(strict=True)
    equal(str(host_php), receipt['host_stub_generator_php']['binary'], 'fixed stub-generator executable')
    equal(digest(host_php), receipt['host_stub_generator_php']['executable_sha256'], 'stub-generator executable bytes')
    equal(receipt['host_stub_generator_php']['sapi'], 'cli', 'build-time stub-generator SAPI')
    equal(receipt['host_stub_generator_php']['tokenizer'], True, 'build-time tokenizer')
    require(type(receipt['host_stub_generator_php']['version_id']) is int
            and receipt['host_stub_generator_php']['version_id'] >= 70400, 'build-time PHP version')
    expected_commands = [
        ['bash', manifest['build']['configure_script']],
        ['make', '-j2', 'PHP=' + receipt['host_stub_generator_php']['binary'], 'cgi'],
        [str(cgi_prepare.owned_path('tools/binaryen/bin/wasm-opt'))]
        + manifest['build']['optimizer_flags']
        + [str(php / manifest['build']['raw_output']), '-o', str(php / manifest['build']['wasm_output'])],
    ]
    for step, name, command in zip(steps, ['configure', 'make_cgi', 'optimize_cgi'], expected_commands):
        equal(step['name'], name, 'build step ordering')
        equal(step['completed'], True, name + ' completion')
        equal(step['exit_code'], 0, name + ' exit')
        equal(step['command'], command, name + ' command')
    equal(receipt['cgi_wasm']['path'], 'sapi/cgi/php-cgi.wasm', 'fixed generated path')
    equal(manifest['build']['wasm_output'], 'sapi/cgi/php-cgi.wasm', 'manifest generated path')
    equal(receipt['cgi_wasm']['wasm_header_verified'], True, 'Wasm header observation')
    equal(receipt['cgi_wasm']['executed'], False, 'builder execution scope')
    fixed_file(MODULE, 256 * 1024 * 1024)
    equal(MODULE.stat().st_size, receipt['cgi_wasm']['bytes'], 'generated module size')
    equal(digest(MODULE), receipt['cgi_wasm']['sha256'], 'generated module SHA')
    with MODULE.open('rb') as source:
        require(source.read(8) == b'\0asm\x01\0\0\0', 'generated module header')
    raw = fixed_file(php / manifest['build']['raw_output'], 256 * 1024 * 1024)
    equal(raw.stat().st_size, receipt['raw_cgi_wasm']['bytes'], 'raw module size')
    equal(digest(raw), receipt['raw_cgi_wasm']['sha256'], 'raw module SHA')
    equal(digest(FIXTURE), FIXTURE_SHA, 'fixed CGI fixture')
    captures = cgi_build.verify_link_capture(receipt, manifest)
    return {'build_receipt_sha256': receipt_sha, 'toolchain_receipt_sha256': tool_sha,
            'manifest_sha256': digest(BUILD / 'manifest.json'),
            'module_bytes': receipt['cgi_wasm']['bytes'], 'module_sha256': receipt['cgi_wasm']['sha256'],
            'source_commit': manifest['sources']['php']['commit'],
            'patch_sha256': manifest['source_patches'][0]['sha256'],
            'link_capture': captures,
            'verification_scope': 'same trusted job observations; not signed attestation or reproducible build'}


def expected_php(index):
    return {
        'runtime': {'version': '8.5.12', 'version_id': 80512, 'int_size': 8, 'sapi': 'cgi-fcgi'},
        'method': 'POST' if index == 1 else 'GET', 'query_string': QUERIES[index],
        'script_filename': '/fixture/cgi_contract.php',
        'request_uri': '/cgi_contract.php?' + QUERIES[index],
        'get': {'case': 'post', 'source': 'fixed'} if index == 1 else {'case': 'get', 'value': 'hello wasix'},
        'post': {'title': 'RP CGI', 'count': '2', 'empty': ''} if index == 1 else [],
        'cookie': {'wp_logged_in': 'fixture user', 'wp_test': '1'},
        'raw_body_hex': POST.hex() if index == 1 else '',
    }


def expected_environment(index):
    values = [
        ['GATEWAY_INTERFACE', 'CGI/1.1'], ['SERVER_PROTOCOL', 'HTTP/1.1'],
        ['SERVER_SOFTWARE', 'recastpress-v0.5-fixed-cgi-probe'],
        ['SERVER_NAME', 'fixture.invalid'], ['SERVER_PORT', '80'], ['REDIRECT_STATUS', '1'],
        ['REQUEST_METHOD', 'POST' if index == 1 else 'GET'],
        ['SCRIPT_FILENAME', '/fixture/cgi_contract.php'], ['SCRIPT_NAME', '/cgi_contract.php'],
        ['DOCUMENT_ROOT', '/fixture'], ['QUERY_STRING', QUERIES[index]],
        ['HTTP_COOKIE', 'wp_logged_in=fixture%20user; wp_test=1'],
        ['REQUEST_URI', '/cgi_contract.php?' + QUERIES[index]],
        ['CONTENT_LENGTH', str(len(POST)) if index == 1 else '0'],
    ]
    if index == 1:
        values.append(['CONTENT_TYPE', 'application/x-www-form-urlencoded'])
    return values


def validate_native_baseline(native, read_source=None):
    """Verify a public native oracle without executing or requiring native PHP.

    The pinned map digest binds all 78 package-file names and SHA values. Its
    bytes are not available on the Linux CI host; this remains a source-bound
    recorded Windows observation, not a fresh package execution or attestation.
    """
    for key, expected in [('format_version', 1), ('outcome', 'native_cgi_contract_passed'),
                          ('native_reference_passed', True), ('wasm_executed', False),
                          ('wordpress_executed', False), ('native_cli_substitute_used', False),
                          ('listener_created', False), ('owned_process_cleanup_completed', True),
                          ('owned_fixture_cleanup_completed', True), ('request_timeout_seconds', 10)]:
        equal(native[key], expected, 'native baseline ' + key)
    equal(native['platform'], {'system': 'Windows', 'machine': 'AMD64'}, 'native platform')
    expected_inputs = {
        'experiments/php-wasix-host-probe/native_cgi_contract.py',
        'experiments/php-wasix-host-probe/fixtures/cgi_contract.php',
        'experiments/php-wasix-host-probe/cgi_check.py',
        'experiments/php-wasix-host-probe/check.py',
        'experiments/php-wasix-host-probe/language_check.py',
        'experiments/php-runtime-candidate-probe/native_contract.py',
        'scripts/owned_probe.py', 'experiments/php-wasi-host-probe/wordpress-fixture.json',
        'experiments/php-wasix-cgi-build/build.py', 'experiments/php-wasix-cgi-build/prepare.py',
        'experiments/php-wasix-cgi-build/patch_source.py',
    }
    equal(set(native['input_files']), expected_inputs, 'native baseline source set')
    if read_source is None:
        verify_hash_files(ROOT, native['input_files'])
    else:
        # Historical CI importers supply bytes from their pinned original Git
        # tree. They must not reinterpret an old receipt as a current execution.
        for path, expected in native['input_files'].items():
            equal(hashlib.sha256(read_source(path)).hexdigest(), expected, 'original native source ' + path)
    reference = native['native_reference']
    for key, expected in [('version', '8.5.12'), ('archive_sha256', NATIVE_ARCHIVE_SHA),
                          ('manifest', '.reference/php-8.5.12/recastpress-tool.json'),
                          ('manifest_sha256', NATIVE_MANIFEST_SHA)]:
        equal(reference[key], expected, 'native package ' + key)
    files = reference['verified_files']
    require(type(files) is dict and len(files) == 78, 'native package file set')
    map_sha = hashlib.sha256(json.dumps(files, sort_keys=True, separators=(',', ':')).encode('utf-8')).hexdigest()
    equal(map_sha, NATIVE_FILE_MAP_SHA, 'complete native package file identities')
    equal(files['php-cgi.exe'], NATIVE_CGI_SHA, 'native CGI executable')
    equal(native['fixture_copy']['sha256'], FIXTURE_SHA, 'native copied fixture')
    equal(native['fixture_copy']['bytes'], FIXTURE.stat().st_size, 'native copied fixture size')
    equal(digest(FIXTURE), FIXTURE_SHA, 'current CGI fixture')
    normalization = native['comparison_normalization']
    equal(normalization['field'], 'script_filename', 'native normalization field')
    equal(normalization['native_public_value'], '<owned-fixture>', 'native public path marker')
    equal(normalization['guest_exact_value'], '/fixture/cgi_contract.php', 'normalized guest path')
    equal(normalization['original_value_stored'], False, 'private original path omitted')
    equal(native['native_environment_scaffold'], {
        'provided_keys': ['SystemRoot', 'TEMP', 'TMP', 'WINDIR'],
        'absolute_values_recorded': False, 'temp_scope': 'fresh owned target directory'}, 'native environment scaffold')
    require(type(native['requests']) is list and len(native['requests']) == 3, 'native baseline request set')
    for index, row in enumerate(native['requests']):
        equal(row['name'], NAMES[index], 'native request name')
        equal(row['request_number'], index + 1, 'native request order')
        equal(row['exit_code'], 0, 'native exit')
        for key in ['completed', 'owned_process_reaped', 'stdout_eof', 'stderr_eof']:
            equal(row[key], True, 'native ' + key)
        require(type(row['owned_process_pid']) is int and row['owned_process_pid'] > 0, 'native owned PID')
        equal(row['command_arguments'], ['-n'], 'native fixed arguments')
        env = [[k, '<owned-fixture>' if k == 'SCRIPT_FILENAME' else v] for k, v in expected_environment(index)]
        equal(row['normalized_environment'], env, 'native fixed environment')
        equal(row['environment_record_kind'], 'SCRIPT_FILENAME-redacted; other CGI values unchanged', 'native environment labelling')
        equal(row['stdin_hex'], POST.hex() if index == 1 else '', 'native stdin')
        equal(row['stdin_bytes'], len(POST) if index == 1 else 0, 'native stdin size')
        equal(row['stderr_bytes'], 0, 'native stderr size')
        equal(row['stderr_sha256'], hashlib.sha256(b'').hexdigest(), 'native stderr digest')
        require(type(row['stdout_bytes']) is int and 0 < row['stdout_bytes'] <= 65536, 'native capture size')
        require(type(row['stdout_sha256']) is str and len(row['stdout_sha256']) == 64
                and all(c in '0123456789abcdef' for c in row['stdout_sha256']), 'native capture digest')
        response = row['response']
        equal(response['status'], 404 if index == 2 else 200, 'native response status')
        equal(response['representation_length'], None, 'native representation length')
        headers = [(x['name'], bytes.fromhex(x['value_hex'])) for x in response['headers']]
        equal([v for k, v in headers if k == 'content-type'],
              [b'application/octet-stream' if index == 2 else b'application/json'], 'native content type')
        equal([v for k, v in headers if k == 'set-cookie'],
              [b'wp_a=one; path=/; HttpOnly', b'wp_b=two; path=/; SameSite=Lax'] if index == 2 else [], 'native cookie fields')
        if index == 2:
            equal(row['stdout_record_kind'], 'original-bytes', 'native binary capture kind')
            equal(response['body_record_kind'], 'original-bytes', 'native binary body kind')
            body = bytes.fromhex(response['body_hex'])
            equal(body, b'\0\xffCGI\r\n', 'native binary bytes')
            equal(response['body_bytes'], len(body), 'native binary size')
            equal(response['body_sha256'], hashlib.sha256(body).hexdigest(), 'native binary digest')
            wire = bytes.fromhex(row['stdout_hex'])
            equal(len(wire), row['stdout_bytes'], 'native binary CGI capture size')
            equal(hashlib.sha256(wire).hexdigest(), row['stdout_sha256'], 'native binary CGI capture digest')
            require(wire.endswith(body), 'native binary body/capture consistency')
        else:
            equal(row['stdout_record_kind'], 'original-digest-and-length-only', 'native private JSON capture kind')
            equal(response['body_record_kind'], 'original-digest-and-length-only', 'native private JSON body kind')
            require('stdout_hex' not in row and 'php' not in row and 'body_hex' not in response,
                    'private original JSON capture must not be published')
            public_php = dict(expected_php(index))
            public_php['script_filename'] = '<owned-fixture>'
            equal(row['normalized_native_php'], public_php, 'labelled native JSON observation')
            normalized = dict(row['normalized_native_php'])
            normalized['script_filename'] = '/fixture/cgi_contract.php'
            equal(row['comparison_php'], normalized, 'native single-field comparison normalization')
            equal(row['comparison_php'], expected_php(index), 'native CGI JSON contract')
            require(type(response['body_bytes']) is int and 0 < response['body_bytes'] < row['stdout_bytes'], 'native original JSON body size')
            require(type(response['body_sha256']) is str and len(response['body_sha256']) == 64
                    and all(c in '0123456789abcdef' for c in response['body_sha256']), 'native original JSON body digest')


def load_native_baseline():
    native, receipt_sha = read_json(NATIVE_BASELINE, 256 * 1024)
    validate_native_baseline(native)
    return native, receipt_sha


def compare_native_requests(value, native):
    """Compare semantic CGI fields separately from framing and JSON encoding.

    Header-name case is immaterial. Cookie values/order and binary bytes remain
    exact. JSON formatting and Content-Length naturally differ when the native
    script path has a different length; only the parsed SCRIPT_FILENAME value
    was normalized by the native oracle, and every other PHP type/value matches.
    """
    require(type(value['requests']) is list and len(value['requests']) == 3, 'actual request count for native comparison')
    results = {}
    for index, (actual, reference) in enumerate(zip(value['requests'], native['requests'])):
        try:
            equal(actual['name'], reference['name'], 'native case ordering')
            equal(actual['response']['status'], reference['response']['status'], 'native response status')
            for name in ['content-type', 'set-cookie']:
                def values(row):
                    return [bytes.fromhex(x['value_hex']) for x in row['response']['headers'] if x['name'].lower() == name]
                equal(values(actual), values(reference), 'native ' + name + ' fields')
            if index == 2:
                equal(bytes.fromhex(actual['response']['body_hex']), bytes.fromhex(reference['response']['body_hex']), 'native binary body bytes')
            else:
                equal(actual['php'], reference['comparison_php'], 'native PHP JSON with only SCRIPT_FILENAME normalized')
        except (ValueError, TypeError, KeyError) as error:
            results[NAMES[index]] = {'equal_to_native': False, 'difference': str(error)}
        else:
            results[NAMES[index]] = {'equal_to_native': True,
                                    'body_comparison': 'exact-binary-bytes' if index == 2 else 'exact-JSON-types-and-values-after-declared-path-normalization'}
    return results


def validate_result(value, pid, backend, provenance, *,
                    schema='recastpress.php-wasix-cgi-contract.v0.5',
                    phase='cgi_contract_verified', module_path=None):
    """Validate actual host output; callers select their closed build profile.

    The default is the original one-patch CGI candidate. Shared request/body
    expectations are unchanged when a caller supplies another fixed profile.
    """
    if module_path is None:
        module_path = MODULE.relative_to(ROOT).as_posix()
    require(type(schema) is str and type(phase) is str and type(module_path) is str,
            'explicit CGI validation profile strings required')
    for field, expected in [('schema', schema),
                            ('process_pid', pid), ('engine', 'wasmer-7.2.0/' + backend),
                            ('phase', phase), ('ok', True), ('cgi_contract_verified', True),
                            ('real_db', False), ('wordpress_executed', False),
                            ('native_comparison_performed', False), ('full_php_compatibility_verified', False)]:
        equal(value[field], expected, 'host ' + field)
    equal(value['build_receipt']['observed_sha256'], provenance['build_receipt_sha256'], 'host receipt snapshot')
    equal(value['build_receipt']['manifest_sha256'], provenance['manifest_sha256'], 'host manifest snapshot')
    equal(value['build_receipt']['verified'], True, 'host build receipt')
    equal(value['build_receipt']['full_source_tool_config_chain_verified_by_host'], False, 'verification owner')
    equal(value['artifact']['path'], module_path, 'host fixed generated path')
    equal(value['artifact']['observed_sha256'], provenance['module_sha256'], 'host exact compiled bytes')
    equal(value['artifact']['observed_bytes'], provenance['module_bytes'], 'host compiled size')
    equal(value['artifact']['verified'], True, 'host module identity')
    equal(value['fixture']['observed_sha256'], FIXTURE_SHA, 'host fixture snapshot')
    equal(value['fixture']['verified'], True, 'host fixture verification')
    equal(value['guest_arguments'], ['-n'], 'fixed CGI arguments')
    isolation = value['isolation']
    for field in ['host_environment_forwarded', 'home_mapped', 'host_directories_mapped', 'http_client',
                  'memory_limit_enforced', 'in_process_deadline_enforced', 'other_virtual_root_writes_forbidden']:
        equal(isolation[field], False, 'isolation ' + field)
    equal(isolation['networking'], 'UnsupportedVirtualNetworking', 'no guest network')
    equal(isolation['guest_fixture_files'], ['/fixture/cgi_contract.php'], 'only fixed fixture supplied')
    equal(isolation['guest_readonly_mounts'], ['/fixture'], 'immutable fixture mount')
    for field, expected in [('requested_requests', 3), ('php_module_compile_invocations', 1),
                            ('module_clone_count', 3), ('completed_requests', 3), ('request_path_module_compile_calls', 0),
                            ('module_delivery', 'Module.clone')]:
        equal(value['reuse'][field], expected, 'reuse ' + field)
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms'] >= 0, 'compile duration')
    require(type(value['requests']) is list and len(value['requests']) == 3, 'fixed request count')
    for index, request in enumerate(value['requests']):
        equal(request['name'], NAMES[index], 'request name')
        equal(request['request_number'], index + 1, 'request ordering')
        equal(request['module_compile_invocations'], 0, 'no request compiler call')
        equal(request['module_delivery'], 'Module.clone', 'request module reuse')
        equal(request['guest_environment_supplied'], expected_environment(index), 'fixed CGI environment')
        for field in ['php_completed', 'contract_verified', 'stdout_eof', 'stderr_eof', 'runtime_shutdown_returned']:
            equal(request[field], True, 'request ' + field)
        for field in ['stdout_overflow', 'stderr_overflow']:
            equal(request[field], False, 'request ' + field)
        equal(request['stderr_bytes'], 0, 'guest stderr count')
        equal(request['stderr_hex'], '', 'guest stderr')
        equal(request['stdin_hex'], POST.hex() if index == 1 else '', 'fixed stdin')
        equal(request['stdin_bytes'], len(POST) if index == 1 else 0, 'fixed stdin length')
        require(type(request['php_execution_elapsed_ms']) is int and request['php_execution_elapsed_ms'] >= 0,
                'execution duration')
        wire = bytes.fromhex(request['stdout_hex'])
        equal(len(wire), request['stdout_bytes'], 'raw CGI capture length')
        require(0 < len(wire) <= 65536, 'raw CGI capture limit')
        equal(hashlib.sha256(wire).hexdigest(), request['stdout_sha256'], 'raw CGI capture hash')
        response = request['response']
        body = bytes.fromhex(response['body_hex'])
        equal(response['body_bytes'], len(body), 'response body count')
        equal(response['body_sha256'], hashlib.sha256(body).hexdigest(), 'response body hash')
        require(wire.endswith(body), 'response body differs from CGI capture')
        equal(response['representation_length'], None, 'representation-only length')
        equal(response['status'], 404 if index == 2 else 200, 'response status')
        headers = [(row['name'], bytes.fromhex(row['value_hex'])) for row in response['headers']]
        expected_type = b'application/octet-stream' if index == 2 else b'application/json'
        equal([v for k, v in headers if k == 'content-type'], [expected_type], 'response content type')
        cookies = [v for k, v in headers if k == 'set-cookie']
        equal(cookies, [b'wp_a=one; path=/; HttpOnly', b'wp_b=two; path=/; SameSite=Lax'] if index == 2 else [],
              'separate cookie fields')
        require(not any(k in {'status', 'content-length', 'connection', 'transfer-encoding'} for k, _ in headers),
                'CGI/transport control header leaked')
        if index == 2:
            equal(body, b'\0\xffCGI\r\n', 'binary response bytes')
        else:
            php = parse_json(body)
            equal(php, expected_php(index), 'CGI request observation')
            equal(request['php'], php, 'decoded PHP body consistency')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'scope': 'same-job-generated-CGI-three-fixed-requests', 'outcome': 'not_run',
              'platform': {'system': platform.system(), 'machine': platform.machine()}, 'backend': backend,
              'cgi_contract_passed': False, 'owned_host_reaped': False, 'product_runtime_selected': False,
              'wordpress_executed': False, 'native_php_executed_by_this_helper': False,
              'native_comparison_performed': False, 'native_comparison_passed': False,
              'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 300,
              'memory_limit_verified': False, 'redistributed_binaries': False}
    passed, stage = False, 'native_reference_verification'
    try:
        require(platform.system() == 'Linux', 'generated CGI contract requires its Linux build job')
        native, native_sha = load_native_baseline()
        report['native_reference'] = {'receipt': NATIVE_BASELINE.relative_to(ROOT).as_posix(),
                                      'receipt_sha256': native_sha, 'version': '8.5.12',
                                      'archive_sha256': NATIVE_ARCHIVE_SHA, 'cgi_sha256': NATIVE_CGI_SHA,
                                      'verified_file_map_sha256': NATIVE_FILE_MAP_SHA,
                                      'comparison_normalization': native['comparison_normalization'],
                                      'native_executed_by_this_helper': False,
                                      'json_serialization_bytes_compared': False,
                                      'transport_framing_compared': False}
        stage = 'build_provenance'
        provenance = verify_build()
        report['generated_provenance'] = provenance
        stage = 'host_build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HERE / 'Cargo.toml'),
                   '--target-dir', str(host_probe.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode != 0:
            report['diagnostics'] = host_probe.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'CGI host build failed')
        executable = host_probe.select_artifact(built.stdout)
        report['native_artifact_sha256'] = digest(executable)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        stage = 'execution'
        child_environment = {k: v for k, v in environment.items() if k.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--cgi-contract'], child_environment, timeout=300)
        report['owned_host_reaped'] = True
        report['owned_process_pid'] = pid
        report['exit_code'] = completed.returncode
        require(len(completed.stdout) <= 512 * 1024 and len(completed.stderr) <= 64 * 1024, 'host capture budget')
        report['stderr_sha256'] = hashlib.sha256(completed.stderr).hexdigest()
        phases = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
                  if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr:
            report['stderr_excerpt'] = host_probe.diagnostic(completed.stderr, 4096)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated host JSON result required')
        value = parse_json(completed.stdout)
        report['native_result'] = value
        require(completed.returncode == 0, 'fixed CGI host execution failed')
        stage = 'contract_verification'
        equal(phases, ['module_compile', 'cgi_request_1', 'cgi_request_2', 'cgi_request_3'], 'execution phases')
        validate_result(value, pid, backend, provenance)
        results = compare_native_requests(value, native)
        report['native_comparison_performed'] = True
        report['native_comparison_results'] = results
        require(all(row['equal_to_native'] for row in results.values()), 'CGI observations differ from native CGI oracle')
        equal(digest(NATIVE_BASELINE), native_sha, 'native oracle unchanged during execution')
        report['native_comparison_passed'] = True
        # Recheck after execution: neither the receipt nor generated bytes may
        # have changed between source-chain verification and host observation.
        equal(verify_build(), provenance, 'post-execution provenance snapshot')
        report['cgi_contract_passed'], passed = True, True
        report['outcome'] = 'cgi_contract_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr:
            report['stderr_excerpt'] = host_probe.diagnostic(error.stderr, 4096)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        try:
            owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = str(error)[:8192]
            report['cgi_contract_passed'], passed = False, False
    inputs = host_probe.compiled_inputs()
    inputs += [HERE / n for n in ['check.py', 'language_check.py', 'cgi_check.py']]
    inputs += [ROOT / 'scripts/owned_probe.py', BUILD / 'manifest.json',
               BUILD / 'prepare.py', BUILD / 'build.py', BUILD / 'patch_source.py',
               NATIVE_BASELINE, HERE / 'native_cgi_contract.py']
    report['input_files'] = {p.relative_to(ROOT).as_posix(): digest(p) for p in inputs}
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        (directory / (platform.system().lower() + '-' + backend + '-cgi-contract.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX fixed CGI contract: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
