"""Supervise the unchanged 100M-operation diagnostic through the CGI entry.

Typed setup/recycle Global observations remain owned by the shared Rust probe.
The exhausted loop keeps raw READY and its Runner error; it is not a completed
CGI response, CPU-time limit, cancellation test or product-runtime claim.
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


cgi = load('rp_managed_operation_cgi_provenance', HERE / 'cgi_check.py')
managed = cgi.managed
original = load('rp_managed_operation_original_observer_validation', HOST / 'operation_check.py')
host, owned_probe = managed.host, managed.owned_probe
require, equal, digest, parse_json = managed.require, managed.equal, managed.digest, managed.parse_json
MODULE, RECEIPT, VARIANT = managed.MODULE, managed.RECEIPT, managed.VARIANT
FIXTURE = HERE / 'fixtures/operation_cgi_contract.php'
CANONICAL = HOST / 'src/php_operation_budget.rs'
GUEST = '/fixture/operation_cgi_contract.php'
FIXTURE_SHA = '455204fd5af80680dca3b32de236bb22db2a212ab91f124fc2fb02b8007a1f88'
SCHEMA = 'recastpress.php-wasix-managed-operation-host.v0.5'
SCOPE = 'three-patch-managed-candidate-three-CGI-operation-budget-cases'
PHASE = 'managed_operation_budget_verified'
HEAD = b"<?php\n// Closed CGI entry; original operation-budget program bodies are unchanged.\nif (PHP_SAPI !== 'cgi-fcgi'\n    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'\n    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_operation_cgi=1'\n    || ($_SERVER['argc'] ?? 0) !== 0) {\n    throw new RuntimeException('Fixed operation-budget CGI request required');\n}\n$rp_case = getenv('RP_OPERATION_CASE');\nif ($rp_case !== 'normal' && $rp_case !== 'loop') {\n    throw new RuntimeException('Fixed operation-budget case required');\n}\nif ($rp_case === 'normal') {\nheader('Content-Type: application/json');"
MIDDLE = b"} else {\n$rp_stdout = fopen('php://stdout', 'wb');\nif ($rp_stdout === false || !define('STDOUT', $rp_stdout)) {\n    throw new RuntimeException('Fixed CGI STDOUT alias required');\n}"
END = b'}\n'


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def verify_fixture(read_source=None):
    source = managed.source_bytes(CANONICAL, read_source)
    bodies = {}
    for name, size, identity in [('NORMAL', 405, original.NORMAL_SHA), ('LOOP', 84, original.LOOP_SHA)]:
        matches = re.findall(rb'const ' + name.encode('ascii') + rb': &str = r#"(.*?)"#;', source, re.S)
        require(len(matches) == 1, 'one fixed original operation program ' + name)
        bodies[name] = matches[0]
        equal((len(matches[0]), sha(matches[0])), (size, identity), 'byte-exact original operation body ' + name)
    actual = managed.source_bytes(FIXTURE, read_source)
    equal(actual, HEAD + bodies['NORMAL'] + MIDDLE + bodies['LOOP'] + END, 'closed CGI wrapper and unchanged original bodies')
    equal(sha(actual), FIXTURE_SHA, 'fixed CGI operation fixture identity')
    return {'source': FIXTURE.relative_to(ROOT).as_posix(), 'guest_path': GUEST,
        'expected_sha256': FIXTURE_SHA, 'verified': True,
        'canonical_source': CANONICAL.relative_to(ROOT).as_posix(),
        'case_bodies_byte_identical': True, 'observed_sha256': FIXTURE_SHA}


def source_hashes():
    verify_fixture()
    hashes = cgi.source_hashes()
    for path in [Path(__file__).resolve(), FIXTURE, CANONICAL, HOST / 'operation_check.py']:
        require(path.resolve(strict=True) == path and path.is_relative_to(ROOT), 'operation source outside workspace/linked')
        name, identity = path.relative_to(ROOT).as_posix(), digest(path)
        if name in hashes: equal(hashes[name], identity, 'shared operation source identity')
        hashes[name] = identity
    return dict(sorted(hashes.items()))


def validate_result(value, pid, backend, provenance, *, read_source=None, build_inputs=None):
    require(backend in ['cranelift', 'llvm'] and 'error' not in value, 'closed successful managed operation host')
    managed.integer(pid, 1)
    managed.validate_provenance(provenance, read_source, build_inputs)
    # Only schema/path/CGI input/body/SAPI differ. The original validator checks
    # every 100M point, start charge, same-store typed handle, recycle and budget
    # observation directly on the actual report, without a normalized copy.
    original.validate_result(value, pid, backend, profile='managed-cgi', provenance=provenance)
    for key, expected in [('schema', SCHEMA), ('scope', SCOPE), ('phase', PHASE), ('variant', VARIANT),
            ('wasmer_wasix', '0.702.0'), ('product_runtime_selected', False), ('product_abi_selected', False),
            ('managed_access_verified_for_fixed_cases', False)]: equal(value[key], expected, 'managed operation host ' + key)
    equal(value['source'], {'repository': 'wasix-org/php', 'commit': 'f4bfae79d5c19028349082f707930cf0951d23b9',
        'kind': 'same-job-patched-source-build', 'verified': True}, 'managed operation PHP source')
    equal(value['artifact']['identity_policy'], 'same-job-three-source-patch-build', 'three-patch operation candidate')
    for key, expected in [('path', RECEIPT.relative_to(ROOT).as_posix()), ('verified', True),
            ('observed_sha256', provenance['build_receipt_sha256']),
            ('manifest_sha256', provenance['builder_provenance']['manifest_sha256']),
            ('full_source_tool_chain_verified_by_host', False),
            ('chain_owner', 'managed_check.py verifies all source/tool/patch/module provenance')]:
        equal(value['build_receipt'][key], expected, 'managed operation build receipt ' + key)
    equal(value['source_files'], managed.sources([ROOT / name for name in managed.SOURCES], read_source), 'fourteen embedded managed source identities')
    managed.fiber.validate_inspection(value['postopt_adapter'], provenance['module'])
    equal(value['fixture'], verify_fixture(read_source), 'actual byte-exact operation CGI fixture')
    for case in value['cases']:
        require('managed_access' not in case, 'operation-only trial does not prove permission queries')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    timeout = original.HOST_TIMEOUT_SECONDS[backend]
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-managed-operation-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend, 'variant': VARIANT,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'scope': SCOPE,
        'outcome': 'not_run', 'diagnostic_passed': False,
        'php_wasm_operation_budget_verified_for_fixed_cases': False,
        'owned_host_reaped': False, 'owned_process_cleanup_completed': False,
        'normal_host_result_received': False, 'native_comparison_performed': False,
        'native_php_executed_by_this_helper': False, 'native_runtime_fallback_used': False,
        'product_runtime_selected': False, 'product_abi_selected': False, 'product_adopted': False,
        'wordpress_executed': False, 'real_db': False, 'listener_created': False,
        'managed_access_verified_for_fixed_cases': False, 'full_php_compatibility_verified': False,
        'production_operation_budget_verified': False, 'cpu_time_budget_verified': False,
        'php_opcode_budget_verified': False, 'hostcall_timeout_verified': False,
        'all_resources_immediately_reclaimed': False, 'memory_limit_verified': False, 'cpu_limit_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': timeout}
    passed, stage = False, 'source_verification'
    try:
        require(platform.system() == 'Linux', 'managed operation execution is Linux-only')
        before = source_hashes(); report['input_files'] = before
        report['fixture_derivation'] = verify_fixture()
        stage = 'build_provenance'
        provenance = managed.verify_build(); report['generated_provenance'] = provenance
        stage = 'host_build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HOST / 'Cargo.toml'),
            '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm: command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode: report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'managed operation Rust host build failed')
        executable = host.select_artifact(built.stdout); binary_sha = digest(executable)
        report.update(native_artifact=executable.relative_to(ROOT).as_posix(), native_artifact_sha256=binary_sha)
        equal(source_hashes(), before, 'operation sources unchanged during compilation')
        stage = 'execution'
        child_environment = {key: val for key, val in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--managed-php-operation-budget'], child_environment, timeout=timeout)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=completed.returncode,
            normal_host_result_received=True, owned_host_reap_kind='normal-host-return',
            host_stdout_sha256=sha(completed.stdout), host_stderr_sha256=sha(completed.stderr))
        require(len(completed.stdout) <= 1024 * 1024 and len(completed.stderr) <= 65536, 'managed operation host capture quota')
        phases = original.observed_phases(completed.stderr); report['observed_phases'] = phases
        if completed.stderr: report['host_stderr_excerpt'] = host.diagnostic(completed.stderr, 8192)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated operation host JSON line')
        value = parse_json(completed.stdout); report['native_result'] = value
        equal(source_hashes(), before, 'operation sources/oracles unchanged after execution')
        equal(digest(executable), binary_sha, 'operation host binary unchanged')
        equal(managed.verify_build(), provenance, 'operation full source/tool/three-patch/module chain unchanged')
        require(completed.returncode == 0, 'managed operation host failed')
        stage = 'contract_verification'
        equal(phases, ['module_compile'] + original.CASES, 'one compilation/normal-loop-normal cases')
        validate_result(value, pid, backend, provenance)
        passed = report['diagnostic_passed'] = report['php_wasm_operation_budget_verified_for_fixed_cases'] = True
        report['outcome'] = 'fixed_managed_operation_budget_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline; timeout is not proof of exhaustion'
        original.record_timeout_capture(report, error)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try:
            owned_probe.reap_owned_probes(); report['owned_process_cleanup_completed'] = True
            if report['outcome'] == 'execution_timeout':
                report['owned_host_reaped'] = True
                report['owned_host_reap_kind'] = 'forced-timeout-cleanup-without-normal-result'
        except (Exception, KeyboardInterrupt) as error:
            passed = report['diagnostic_passed'] = report['php_wasm_operation_budget_verified_for_fixed_cases'] = False
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / (platform.system().lower() + '-' + backend + '-managed-operation-budget.json')
        require(path.resolve() == path and not path.is_symlink(), 'managed operation evidence linked')
        wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 1024 * 1024, 'managed operation receipt public quota')
        path.write_bytes(wire)
    print('PHP WASIX managed operation budget: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
