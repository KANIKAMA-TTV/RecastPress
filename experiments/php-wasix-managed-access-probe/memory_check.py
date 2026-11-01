"""Verify the unchanged four-case memory diagnostic on the managed CGI build.

The 256MiB cap and 128MiB allocation are fixed experiment values. The expected
OOM remains a failed execution. No product, RSS, compiler or aggregate limit is
established, and failed/partial CGI output is retained without normalization.
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

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


cgi = load('rp_managed_memory_generated_gate', HERE / 'cgi_check.py')
managed = cgi.managed
original = load('rp_managed_memory_original_expectations', HOST / 'memory_check.py')
host, owned_probe = managed.host, managed.owned_probe
require, equal, digest, parse_json = managed.require, managed.equal, managed.digest, managed.parse_json
MODULE, RECEIPT, VARIANT = managed.MODULE, managed.RECEIPT, managed.VARIANT
FIXTURE = HERE / 'fixtures/memory_cgi_contract.php'
CANONICAL = HOST / 'src/php_memory_cap.rs'
GUEST = '/fixture/memory_cgi_contract.php'
FIXTURE_SHA = '9b9c066b75628b04569df02288b60c1d4f392f7af773bf09fc0255b191e427b7'
SCHEMA = 'recastpress.php-wasix-managed-memory-host.v0.5'
PHASE = 'managed_memory_cap_verified'
SCOPE = 'three-patch-managed-candidate-four-CGI-memory-cases'
ALIAS = 'STDERR=php://stderr (wb), fixed CGI wrapper only'
RESPONSE_SCOPE = 'response is accepted only after successful guest exit; failed parse observations are separate'
BODY_PINS = {'NORMAL': (258, '09c2be37fa073cafd97c1244ca802780cc650e4cdd0d82ced8fead95dac80242'),
    'ALLOCATE': (459, 'c3191d33dee9e91b50215895d25ecc11692b5faee392940fc0bae9a3d6374c73')}
ARGUMENTS = ['-n', '-d', 'memory_limit=-1', '-d', 'display_errors=stderr', '-d', 'log_errors=0',
    '-d', 'fiber.stack_size=2097152', '-d', 'register_argc_argv=0']


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def verify_fixture(read_source=None):
    source = managed.source_bytes(CANONICAL, read_source)
    actual = managed.source_bytes(FIXTURE, read_source)
    equal(sha(actual), FIXTURE_SHA, 'fixed CGI entry/STDERR alias and original memory programs')
    positions = []
    for name, (size, identity) in BODY_PINS.items():
        bodies = re.findall(rb'const ' + name.encode('ascii') + rb': &str = r#"(.*?)"#;', source, re.S)
        require(len(bodies) == 1, 'one original memory program ' + name)
        body = bodies[0]
        equal(len(body), size, 'original memory program bytes ' + name)
        equal(sha(body), identity, 'original memory program hash ' + name)
        equal(actual.count(body), 1, 'byte-identical original body in fixed CGI wrapper ' + name)
        positions.append(actual.index(body))
    require(positions[0] < positions[1], 'fixed normal/allocate branch order')
    return {'source': FIXTURE.relative_to(ROOT).as_posix(), 'guest_path': GUEST,
        'expected_sha256': FIXTURE_SHA, 'verified': True,
        'canonical_source': CANONICAL.relative_to(ROOT).as_posix(), 'case_bodies_byte_identical': True,
        'stderr_alias': ALIAS, 'observed_sha256': FIXTURE_SHA}


def source_hashes():
    verify_fixture()
    hashes = cgi.source_hashes()
    for path in [Path(__file__).resolve(), FIXTURE, CANONICAL, HOST / 'memory_check.py', HOST / 'reuse_check.py']:
        require(path.resolve(strict=True) == path and path.is_relative_to(ROOT), 'memory input outside workspace/linked')
        name, identity = path.relative_to(ROOT).as_posix(), digest(path)
        if name in hashes: equal(hashes[name], identity, 'shared memory source identity')
        hashes[name] = identity
    return dict(sorted(hashes.items()))


def expected_environment(index):
    return [['GATEWAY_INTERFACE', 'CGI/1.1'], ['SERVER_PROTOCOL', 'HTTP/1.1'],
        ['SERVER_SOFTWARE', 'recastpress-v0.5-fixed-memory-cgi-probe'], ['SERVER_NAME', 'fixture.invalid'],
        ['SERVER_PORT', '80'], ['REDIRECT_STATUS', '200'], ['REQUEST_METHOD', 'GET'],
        ['SCRIPT_FILENAME', GUEST], ['SCRIPT_NAME', '/__rp_memory.php'], ['DOCUMENT_ROOT', '/fixture'],
        ['QUERY_STRING', 'rp_memory_cgi=1'], ['REQUEST_URI', '/__rp_memory.php?rp_memory_cgi=1'],
        ['CONTENT_LENGTH', '0'], ['RP_MEMORY_CASE', 'allocate' if index in [1, 3] else 'normal']]


def parse_cgi(wire):
    """Pure counterpart of the bounded host complete-response parser.

    Parsing a failed run only records an observation; it never accepts the
    failed execution or requires OOM stdout to have any particular framing.
    """
    require(type(wire) is bytes and len(wire) <= 65536, 'memory CGI wire budget')
    position, style, count = 0, None, 0
    status, length, location, content_type = None, None, False, False
    headers = []
    while True:
        end = min(len(wire), 16384)
        require(position < end, 'missing memory CGI header terminator')
        newline = wire.find(b'\n', position, end)
        require(newline >= 0, 'memory CGI header newline/budget')
        raw = wire[position:newline]; crlf = raw.endswith(b'\r')
        require(style is None or style is crlf, 'mixed memory CGI header endings')
        style = crlf; line = raw[:-1] if crlf else raw; position = newline + 1
        if not line: break
        count += 1
        require(count <= 64 and line[:1] not in (b' ', b'\t') and b':' in line, 'memory CGI field budget/folding')
        name, value = line.split(b':', 1)
        require(re.fullmatch(rb"[!#$%&'*+.^_`|~0-9A-Za-z-]+", name) is not None, 'memory CGI header name')
        name, value = name.decode('ascii').lower(), value.strip(b' \t')
        require(all(byte == 9 or 32 <= byte != 127 for byte in value), 'memory CGI header control')
        require(name not in {'connection', 'keep-alive', 'proxy-connection', 'te', 'trailer', 'transfer-encoding', 'upgrade'},
            'memory CGI hop-by-hop field')
        if name == 'status':
            require(status is None and re.fullmatch(rb'[2-5][0-9]{2}(?: [^\r\n]*)?', value) is not None, 'memory CGI status')
            status = int(value[:3])
        elif name == 'content-length':
            require(length is None and re.fullmatch(rb'[0-9]+', value) is not None, 'memory CGI length')
            length = int(value); require(length <= 2**64 - 1, 'memory CGI length overflow')
        else:
            if name == 'location':
                require(not location, 'duplicate memory CGI location'); location = True
            if name == 'content-type':
                require(not content_type, 'duplicate memory CGI content type'); content_type = True
            headers.append({'name': name, 'value_hex': value.hex()})
    status = status if status is not None else (302 if location else 200)
    body = wire[position:]
    representation_length = None
    if status == 204:
        require(not body and length is None, 'memory CGI 204 body/length')
    elif status == 304:
        require(not body, 'memory CGI 304 body'); representation_length = length
    else:
        if status == 205: require(not body, 'memory CGI 205 body')
        require(length is None or length == len(body), 'memory CGI completed length mismatch')
    return {'status': status, 'headers': headers, 'body_bytes': len(body), 'body_hex': body.hex(),
        'body_sha256': sha(body), 'representation_length': representation_length}, body


def failed_json_observation(wire):
    candidates = [wire]
    for separator in [b'\r\n\r\n', b'\n\n']:
        start = wire.find(separator)
        if start >= 0: candidates.append(wire[start + len(separator):])
    for candidate in candidates:
        # serde_json's duplicate-key behavior is relevant only to this negative
        # probe: even completed JSON with duplicate keys must reject an OOM.
        def bad_number(_): raise ValueError('non-finite failed JSON')
        try: return True, json.loads(candidate.decode('utf-8'), parse_constant=bad_number)
        except (ValueError, UnicodeError): pass
    return False, None


def validate_failed_json(case, wire):
    found, observed = failed_json_observation(wire)
    equal(case['failed_stdout_completed_json_observed'], found, 'failed stdout completed JSON observation retained')
    equal(case['php'], observed, 'failed stdout JSON value retained')
    equal(found, False, 'failed allocation did not emit completed JSON, including JSON null')


def validate_result(value, pid, backend, provenance, *, read_source=None, build_inputs=None):
    require(backend in ['cranelift', 'llvm'] and 'error' not in value, 'closed successful managed memory diagnostic')
    managed.integer(pid, 1); managed.validate_provenance(provenance, read_source, build_inputs)
    for key, expected in [('schema', SCHEMA), ('scope', SCOPE), ('phase', PHASE), ('variant', VARIANT),
            ('process_pid', pid), ('engine', 'wasmer-7.2.0/' + backend), ('wasmer_wasix', '0.702.0'),
            ('ok', True), ('diagnostic_verified', True), ('php_linear_memory_cap_verified_for_fixed_cases', True),
            ('observed_php_int_size', 8), ('test_cap_pages', 4096), ('test_cap_bytes', 256 * 1024 * 1024),
            ('allocation_bytes', 128 * 1024 * 1024)]:
        equal(value[key], expected, 'managed memory host ' + key)
    for key in ['production_memory_limit_verified', 'rss_limit_verified', 'compiler_memory_limit_verified',
            'aggregate_memory_limit_verified', 'native_comparison_performed', 'product_adopted', 'real_db',
            'wordpress_executed', 'full_php_compatibility_verified', 'product_abi_selected', 'product_runtime_selected',
            'managed_access_verified_for_fixed_cases']:
        equal(value[key], False, 'unclaimed memory scope ' + key)
    equal(value['source'], {'repository': 'wasix-org/php', 'commit': 'f4bfae79d5c19028349082f707930cf0951d23b9',
        'kind': 'same-job-patched-source-build', 'verified': True}, 'memory candidate source')
    for key, expected in [('path', MODULE.relative_to(ROOT).as_posix()), ('verified', True),
            ('identity_policy', 'same-job-three-source-patch-build'), ('observed_bytes', provenance['module']['bytes']),
            ('observed_sha256', provenance['module']['sha256'])]:
        equal(value['artifact'][key], expected, 'memory candidate module ' + key)
    for key, expected in [('path', RECEIPT.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_sha256', provenance['build_receipt_sha256']),
            ('manifest_sha256', provenance['builder_provenance']['manifest_sha256']),
            ('full_source_tool_chain_verified_by_host', False),
            ('chain_owner', 'managed_check.py verifies all source/tool/patch/module provenance')]:
        equal(value['build_receipt'][key], expected, 'memory candidate receipt ' + key)
    equal(value['source_files'], managed.sources([ROOT / name for name in managed.SOURCES], read_source), 'compiled managed sources')
    managed.fiber.validate_inspection(value['postopt_adapter'], provenance['module'])
    equal(value['fixture'], verify_fixture(read_source), 'compiled fixed memory CGI fixture')
    isolation = value['isolation']
    for key in ['host_environment_forwarded', 'host_directories_mapped', 'home_mapped', 'http_client',
            'memory_limit_enforced', 'in_process_deadline_enforced', 'deep_sleep']:
        equal(isolation[key], False, 'memory isolation ' + key)
    for key, expected in [('networking', 'UnsupportedVirtualNetworking'), ('guest_environment_supplied', None),
            ('guest_environment_policy', 'fixed CGI GET scaffold and closed normal/allocate case'),
            ('guest_fixture_files', [GUEST]), ('guest_readonly_mounts', ['/fixture']), ('asynchronous_threading', True),
            ('max_guest_threads', 1), ('stdin', 'closed-pipe'), ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536)]:
        equal(isolation[key], expected, 'memory isolation ' + key)
    for key, expected in [('php_module_compile_invocations', 1), ('module_clone_count', 4),
            ('request_path_module_compile_calls', 0), ('observed_cases', 4)]:
        equal(value['reuse'][key], expected, 'memory module reuse ' + key)
    identity = value['reuse']['configured_engine_identity']; require(type(identity) is str and identity, 'actual engine identity')
    managed.integer(value['compile_elapsed_ms'])
    require(type(value['cases']) is list and len(value['cases']) == 4, 'four unchanged memory cases')
    for index, case in enumerate(value['cases']):
        success = index != 1
        for key, expected in [('name', original.CASES[index]), ('configured_engine_identity', identity),
                ('module_delivery', 'Module.clone'), ('module_compile_invocations', 0), ('cap_pages', 4096 if index < 3 else None),
                ('success_expected', success), ('ok', success), ('php_completed', success), ('expectation_met', True),
                ('fresh_wasi_runtime', True), ('fresh_memory_policy', True), ('program_sha256', FIXTURE_SHA),
                ('guest_arguments', ARGUMENTS), ('guest_environment_supplied', expected_environment(index)),
                ('host_environment_forwarded', False), ('host_directories_mapped', False), ('home_mapped', False),
                ('http_client', False), ('networking', 'UnsupportedVirtualNetworking'), ('stdin', 'closed-pipe'),
                ('stdout_limit_bytes', 65536), ('stderr_limit_bytes', 65536), ('stdout_eof', True), ('stderr_eof', True),
                ('stdout_overflow', False), ('stderr_overflow', False), ('runtime_shutdown_returned', True),
                ('runtime_shutdown_wait_limit_ms', 2000), ('memory_observation_count', 1), ('response_scope', RESPONSE_SCOPE),
                ('cgi_response_accepted', success), ('oom_observed', not success)]:
            equal(case[key], expected, 'memory case ' + key)
        require('expectation_error' not in case, 'retained memory expectation failure')
        require('managed_access' not in case, 'memory-only corpus has no permission-query execution claim')
        managed.integer(case['php_execution_elapsed_ms'])
        streams = {}
        for stream in ['stdout', 'stderr']:
            wire = bytes.fromhex(case[stream + '_hex']); managed.integer(len(wire), 0, 65536)
            equal(case[stream + '_bytes'], len(wire), 'actual memory ' + stream + ' bytes')
            equal(case[stream + '_sha256'], sha(wire), 'actual memory ' + stream + ' hash')
            equal(case[stream], wire.decode('utf-8', errors='replace'), 'actual memory ' + stream + ' display')
            streams[stream] = wire
        maximum = 4096 if index < 3 else 65536
        equal(set(case['memory']), {'maximum_pages', 'size_pages', 'shared'}, 'actual memory snapshot fields')
        equal(case['memory']['maximum_pages'], maximum, 'actual imported memory maximum')
        equal(case['memory']['shared'], True, 'actual imported shared memory')
        managed.integer(case['memory']['size_pages'], 403, maximum)
        if not success:
            require(type(case['execution_error']) is str and case['execution_error'], 'actual failed Runner error retained')
            require(streams['stderr'].startswith(original.READY.encode()) and b'out of memory' in streams['stderr'].lower(),
                'actual allocation OOM after READY memory_limit=-1')
            equal(case['response'], None, 'failed response is never accepted')
            equal(case['failed_stdout_json_probe_performed'], True, 'completed JSON failure probe performed')
            try: response, _ = parse_cgi(streams['stdout'])
            except ValueError: response = None
            equal(case['cgi_response_parsed'], response is not None, 'failed CGI parse observation is factual')
            equal(case['response_observation'], response, 'failed CGI parsed observation retained')
            validate_failed_json(case, streams['stdout'])
            continue
        require('execution_error' not in case, 'successful memory case retained Runner failure')
        equal(case['cgi_response_parsed'], True, 'completed successful CGI parsed')
        response, body = parse_cgi(streams['stdout'])
        equal(response['status'], 200, 'successful memory CGI status')
        equal(response['headers'], [{'name': 'x-powered-by', 'value_hex': b'PHP/8.5.12'.hex()},
            {'name': 'content-type', 'value_hex': b'application/json'.hex()}], 'fixed memory CGI headers')
        equal(response['representation_length'], None, 'successful memory representation length')
        equal(case['response'], response, 'actual complete CGI raw/body identity')
        equal(case['response_observation'], response, 'actual accepted response observation')
        allocation = index == 3
        php = parse_json(body)
        equal(php, {'php_version': '8.5.12', 'php_int_size': 8, 'kind': 'allocation' if allocation else 'normal',
            'memory_limit': '-1', 'allocated_bytes': 128 * 1024 * 1024 if allocation else 0,
            'first_byte': 120 if allocation else None, 'last_byte': 120 if allocation else None,
            'echo': 'recastpress-v0.5-php-memory-cap'}, 'unchanged normal/allocation PHP observation')
        equal(case['php'], php, 'actual CGI body/decoded memory PHP consistency')
        equal(streams['stderr'], original.READY.encode() if allocation else b'', 'unchanged memory stderr')
        if allocation: require(case['memory']['size_pages'] > 4096, 'uncapped control actually exceeded fixed cap')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true'); parser.add_argument('--record', action='store_true')
    args = parser.parse_args(); backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-managed-memory-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend, 'variant': VARIANT,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'scope': SCOPE,
        'outcome': 'not_run', 'diagnostic_passed': False, 'expected_execution_results': [True, False, True, True],
        'owned_host_reaped': False, 'owned_cleanup_completed': False, 'product_runtime_selected': False,
        'product_abi_selected': False, 'product_adopted': False, 'production_memory_limit_verified': False,
        'rss_limit_verified': False, 'compiler_memory_limit_verified': False, 'aggregate_memory_limit_verified': False,
        'cpu_limit_verified': False, 'all_resources_immediately_reclaimed': False, 'native_comparison_performed': False,
        'native_php_executed_by_this_helper': False, 'native_runtime_fallback_used': False, 'real_db': False,
        'standard_wpdb': False, 'wordpress_executed': False, 'listener_created': False,
        'managed_access_verified_for_fixed_cases': False, 'full_php_compatibility_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 600}
    passed, stage = False, 'source_verification'
    try:
        require(platform.system() == 'Linux', 'managed memory CGI executes only in its Linux source-build job')
        before = source_hashes(); report['input_files'] = before; report['fixture_derivation'] = verify_fixture()
        stage = 'build_provenance'; provenance = managed.verify_build(); report['generated_provenance'] = provenance
        stage = 'host_build'; environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HOST / 'Cargo.toml'),
            '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm: command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode: report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'managed memory Rust host build failed')
        executable = host.select_artifact(built.stdout); binary_sha = digest(executable)
        report.update(native_artifact=executable.relative_to(ROOT).as_posix(), native_artifact_sha256=binary_sha)
        stage = 'execution'
        child_environment = {key: val for key, val in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--managed-php-memory-cap'], child_environment, timeout=600)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=completed.returncode,
            host_stdout_sha256=sha(completed.stdout), host_stderr_sha256=sha(completed.stderr))
        require(len(completed.stdout) <= 512 * 1024 and len(completed.stderr) <= 65536, 'managed memory host capture quota')
        phases = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
            if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr: report['host_stderr_excerpt'] = host.diagnostic(completed.stderr, 8192)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated managed memory JSON result')
        value = parse_json(completed.stdout); report['native_result'] = value
        equal(source_hashes(), before, 'memory sources/oracles unchanged')
        equal(digest(executable), binary_sha, 'memory host binary unchanged')
        equal(managed.verify_build(), provenance, 'memory source/tool/three-patch/module chain unchanged')
        require(completed.returncode == 0, 'managed memory host diagnostic failed')
        stage = 'contract_verification'
        equal(phases, ['module_compile'] + original.CASES, 'one compile/four unchanged memory cases')
        validate_result(value, pid, backend, provenance)
        report['diagnostic_passed'], passed = True, True; report['outcome'] = 'fixed_managed_memory_cap_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'; report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr: report['host_stderr_excerpt'] = host.diagnostic(error.stderr, 8192)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try: owned_probe.reap_owned_probes(); report['owned_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as error:
            passed = report['diagnostic_passed'] = False; report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / (platform.system().lower() + '-' + backend + '-managed-memory-cap.json')
        require(path.resolve() == path and not path.is_symlink(), 'managed memory evidence path linked')
        wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'managed memory receipt public quota'); path.write_bytes(wire)
    print('PHP WASIX managed memory cap: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
