"""Compare three unchanged CGI requests on the closed managed-access build.

Native Windows PHP is a source-bound recorded reference, never executed here.
This regression does not exercise the private permission-query endpoint or
establish an HTTP listener, WordPress bootstrap, database or product runtime.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
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


managed = load('rp_managed_three_request_build_gate', HERE / 'managed_check.py')
baseline = load('rp_managed_three_request_original_cgi_gate', HOST / 'cgi_check.py')
host, owned_probe = managed.host, managed.owned_probe
require, equal, digest, parse_json = managed.require, managed.equal, managed.digest, managed.parse_json
MODULE, RECEIPT, VARIANT = managed.MODULE, managed.RECEIPT, managed.VARIANT
SCHEMA = 'recastpress.php-wasix-managed-cgi-host.v0.5'
PHASE = 'managed_cgi_contract_verified'
SCOPE = 'three-patch-managed-candidate-fixed-CGI-request-regression-only'


def load_native_framing():
    # The original observer also exports its pure 200/404 framing parser. Bind
    # its two legacy imports explicitly, then restore the import namespace;
    # importing definitions neither verifies nor executes native PHP.
    bindings = {'cgi_check': baseline, 'native_contract': managed.runtime.native.native}
    saved = {name: sys.modules.get(name) for name in bindings}
    try:
        sys.modules.update(bindings)
        return load('rp_managed_three_request_native_framing', HOST / 'native_cgi_contract.py')
    finally:
        for name, previous in saved.items():
            if previous is None: sys.modules.pop(name, None)
            else: sys.modules[name] = previous


native_framing = load_native_framing()


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def source_hashes():
    hashes = managed.source_hashes()
    native, _ = baseline.load_native_baseline()  # Pure full 78-file recorded package/source gate.
    paths = [Path(__file__).resolve(), HERE / 'managed_check.py', HOST / 'cgi_check.py',
        HOST / 'native_cgi_contract.py', baseline.NATIVE_BASELINE]
    paths += [ROOT / name for name in native['input_files']]
    for path in paths:
        require(path.resolve(strict=True) == path and path.is_relative_to(ROOT), 'CGI input outside workspace/linked')
        name, value = path.relative_to(ROOT).as_posix(), digest(path)
        if name in hashes:
            equal(hashes[name], value, 'shared managed CGI source binding')
        hashes[name] = value
    return dict(sorted(hashes.items()))


def validate_result(value, pid, backend, provenance, *, read_source=None, build_inputs=None):
    """Pure validation of the actual report; never rewrite another schema's data."""
    require(backend in ['cranelift', 'llvm'], 'closed managed CGI backend')
    managed.integer(pid, 1)
    require('error' not in value, 'failed managed CGI host cannot pass')
    managed.validate_provenance(provenance, read_source, build_inputs)
    flat = {'build_receipt_sha256': provenance['build_receipt_sha256'],
        'manifest_sha256': provenance['builder_provenance']['manifest_sha256'],
        'module_sha256': provenance['module']['sha256'], 'module_bytes': provenance['module']['bytes']}
    baseline.validate_result(value, pid, backend, flat, schema=SCHEMA, phase=PHASE,
        module_path=MODULE.relative_to(ROOT).as_posix())
    for key, expected in [('scope', SCOPE), ('variant', VARIANT), ('wasmer_wasix', '0.702.0'),
            ('product_abi_selected', False), ('product_runtime_selected', False),
            ('managed_access_verified_for_fixed_cases', False)]:
        equal(value[key], expected, 'managed CGI host ' + key)
    equal(value['source'], {'repository': 'wasix-org/php',
        'commit': 'f4bfae79d5c19028349082f707930cf0951d23b9',
        'kind': 'same-job-patched-source-build', 'verified': True}, 'managed CGI source')
    equal(value['artifact']['identity_policy'], 'same-job-three-source-patch-build', 'managed CGI identity policy')
    for key, expected in [('path', RECEIPT.relative_to(ROOT).as_posix()),
            ('full_source_tool_chain_verified_by_host', False),
            ('chain_owner', 'managed_check.py verifies all source/tool/patch/module provenance')]:
        equal(value['build_receipt'][key], expected, 'managed CGI receipt ' + key)
    equal(value['source_files'], managed.sources([ROOT / name for name in managed.SOURCES], read_source),
        'fourteen compiled managed source/oracle identities')
    managed.fiber.validate_inspection(value['postopt_adapter'], provenance['module'])
    equal(value['fixture']['source'], baseline.FIXTURE.relative_to(ROOT).as_posix(), 'unchanged CGI source')
    equal(value['fixture']['guest_path'], '/fixture/cgi_contract.php', 'unchanged CGI guest path')
    equal(value['fixture']['expected_sha256'], baseline.FIXTURE_SHA, 'unchanged CGI embedded hash')
    equal(value['isolation']['stdin'], 'fixed-CGI-request-body-then-EOF', 'fixed request body closure')
    for request in value['requests']:
        require('error' not in request, 'retained CGI request failure')
        require('managed_access' not in request, 'CGI regression has no permission-query execution claim')
        response, _, _ = native_framing.parse_completed_cgi(bytes.fromhex(request['stdout_hex']))
        equal(response, request['response'], 'original CGI wire/decoded status/headers/body identity')


def native_reference(native, identity):
    return {'receipt': baseline.NATIVE_BASELINE.relative_to(ROOT).as_posix(), 'receipt_sha256': identity,
        'version': '8.5.12', 'archive_sha256': baseline.NATIVE_ARCHIVE_SHA,
        'cgi_sha256': baseline.NATIVE_CGI_SHA, 'verified_file_map_sha256': baseline.NATIVE_FILE_MAP_SHA,
        'comparison_normalization': native['comparison_normalization'], 'native_executed_by_this_helper': False,
        'json_serialization_bytes_compared': False, 'transport_framing_compared': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-managed-cgi-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend, 'variant': VARIANT,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'outcome': 'not_run',
        'scope': SCOPE, 'cgi_contract_passed': False, 'native_comparison_performed': False,
        'native_comparison_passed': False, 'owned_host_reaped': False, 'owned_cleanup_completed': False,
        'managed_access_verified_for_fixed_cases': False, 'product_runtime_selected': False,
        'product_abi_selected': False, 'native_runtime_fallback_used': False,
        'native_php_executed_by_this_helper': False, 'wordpress_executed': False,
        'real_db': False, 'listener_created': False, 'full_php_compatibility_verified': False,
        'all_resources_immediately_reclaimed': False, 'memory_limit_verified': False,
        'cpu_limit_verified': False, 'redistributed_binaries': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 600}
    passed, stage = False, 'source_verification'
    try:
        require(platform.system() == 'Linux', 'managed CGI regression executes only in its Linux source-build job')
        before = source_hashes(); report['input_files'] = before
        native, native_sha = baseline.load_native_baseline()
        report['native_reference'] = native_reference(native, native_sha)
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
        require(built.returncode == 0, 'managed CGI Rust host build failed')
        executable = host.select_artifact(built.stdout); binary_sha = digest(executable)
        report.update(native_artifact=executable.relative_to(ROOT).as_posix(), native_artifact_sha256=binary_sha)
        stage = 'execution'
        child_environment = {key: val for key, val in environment.items() if key.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, completed = owned_probe.run_captured_probe([str(executable), '--managed-cgi-contract'],
            child_environment, timeout=600)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=completed.returncode,
            host_stdout_sha256=sha(completed.stdout), host_stderr_sha256=sha(completed.stderr))
        require(len(completed.stdout) <= 512 * 1024 and len(completed.stderr) <= 65536, 'managed CGI host capture quota')
        phases = [line.split('=', 1)[1] for line in completed.stderr.decode('utf-8', errors='replace').splitlines()
            if line.startswith('PHP_WASIX_PHASE=')]
        report['observed_phases'] = phases
        if completed.stderr: report['host_stderr_excerpt'] = host.diagnostic(completed.stderr, 8192)
        require(completed.stdout.endswith(b'\n') and completed.stdout.count(b'\n') == 1, 'one terminated managed CGI host JSON line')
        value = parse_json(completed.stdout)
        report['native_result'] = value  # Keep even a nonzero host's actual failed report, without rewriting it.
        equal(source_hashes(), before, 'managed CGI sources/oracles unchanged after execution')
        equal(digest(executable), binary_sha, 'managed CGI host binary unchanged')
        equal(managed.verify_build(), provenance, 'managed CGI source/tool/three-patch/module chain unchanged')
        equal(baseline.load_native_baseline(), (native, native_sha), 'native CGI oracle unchanged')
        require(completed.returncode == 0, 'managed CGI host execution failed')
        stage = 'contract_verification'
        equal(phases, ['module_compile', 'cgi_request_1', 'cgi_request_2', 'cgi_request_3'], 'managed CGI execution phases')
        validate_result(value, pid, backend, provenance)
        comparisons = baseline.compare_native_requests(value, native)
        report['native_comparison_performed'] = True
        report['native_comparison_results'] = comparisons
        require(all(row['equal_to_native'] for row in comparisons.values()), 'unchanged native CGI comparison failed')
        passed = report['cgi_contract_passed'] = report['native_comparison_passed'] = True
        report['outcome'] = 'fixed_managed_cgi_contract_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr: report['host_stderr_excerpt'] = host.diagnostic(error.stderr, 8192)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try:
            owned_probe.reap_owned_probes()
            report['owned_cleanup_completed'] = True
        except (Exception, KeyboardInterrupt) as error:
            passed = report['cgi_contract_passed'] = report['native_comparison_passed'] = False
            report['outcome'] = 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / (platform.system().lower() + '-' + backend + '-managed-cgi-contract.json')
        require(path.resolve() == path and not path.is_symlink(), 'managed CGI evidence path linked')
        wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
        require(len(wire) <= 512 * 1024, 'managed CGI receipt exceeds public quota')
        path.write_bytes(wire)
    print('PHP WASIX managed CGI comparison: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
