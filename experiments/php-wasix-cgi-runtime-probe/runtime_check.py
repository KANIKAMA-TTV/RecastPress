"""Supervise three fixed buffered CGI requests using the opt-in PHP Fiber backend.

Strict final-Wasm assembly inspection precedes compilation and guest execution.
Build, parser, guest, semantic, deadline and cleanup failures remain failures.
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
sys.path.insert(0, str(HOST))
import check as host
from language_check import equal, parse_json
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


COMBINED = ROOT / 'experiments/php-wasix-fiber-cgi-probe'
combined = load('rp_cgi_runtime_combined_gate', COMBINED / 'fiber_cgi_check.py')
preparer = load('rp_cgi_runtime_source_gate', HERE / 'prepare.py')
native = load('rp_cgi_runtime_native_gate', HERE / 'native_contract.py')
require, digest = host.require, host.digest
ORACLE = HERE / 'evidence/native-cgi-runtime-contract.json'
FIXTURE = HERE / 'fixtures/runtime_cgi_contract.php'
VARIANT = combined.VARIANT
read_fixed = combined.read_fixed
verify_build = combined.verify_build


def runtime_sources():
    return [HERE / 'manifest.json', FIXTURE, ORACLE,
        ROOT / 'experiments/php-wasi-host-probe/wordpress-fixture.json',
        ROOT / 'experiments/php-runtime-candidate-probe/fixtures/runtime_contract.php']


def verify_native_reference(read_source=None):
    oracle = parse_json(combined.validation_source(ORACLE, read_source))
    for name, expected in oracle['input_files'].items():
        equal(hashlib.sha256(combined.validation_source(ROOT / name, read_source)).hexdigest(), expected,
            'original runtime native source binding')
    # Current pure validator must still match these original native inputs.
    # A changed native corpus therefore fails closed rather than being relabeled.
    native.validate_receipt(oracle)
    return oracle


def source_hashes():
    oracle = verify_native_reference()
    paths = host.compiled_inputs() + [Path(__file__).resolve(), HERE / 'native_contract.py',
        HOST / 'check.py', HOST / 'language_check.py', COMBINED / 'fiber_cgi_check.py', ROOT / 'scripts/owned_probe.py']
    hashes = {p.relative_to(ROOT).as_posix(): digest(p) for p in paths}
    for group in [combined.builder.input_hashes(), preparer.input_hashes(), oracle['input_files']]:
        for name, value in group.items():
            path = ROOT / name
            require(path.resolve(strict=True) == path and path.is_relative_to(ROOT), 'runtime source outside workspace')
            equal(digest(path), value, 'runtime source binding')
            if name in hashes:
                equal(hashes[name], value, 'shared runtime input')
            hashes[name] = value
    return hashes


def sources(paths, read_source=None):
    return [{'path':p.relative_to(ROOT).as_posix(),'bytes':len(combined.validation_source(p, read_source)),
        'sha256':hashlib.sha256(combined.validation_source(p, read_source)).hexdigest(),'verified':True} for p in paths]


def validate_result(value, pid, backend, provenance, prepared, read_source=None, build_inputs=None):
    combined.validate_provenance(provenance, read_source, build_inputs)
    require('error' not in value, 'failed runtime host result')
    for key, expected in [('schema','recastpress.php-wasix-cgi-runtime-host.v0.5'), ('process_pid',pid),
            ('engine','wasmer-7.2.0/'+backend), ('wasmer_wasix','0.702.0'), ('ok',True),
            ('phase','cgi_runtime_contract_verified'), ('variant',VARIANT), ('engine_supports_async',True),
            ('module_compile_invocations',1), ('runtime_contract_verified_for_fixed_cases',True),
            ('php_metadata_matches_actual_readonly_denial',True), ('wordpress_pure_hooks_executed',True),
            ('wordpress_executed',True), ('exceptions_executed',True), ('integer_width_matches_native',True)]:
        equal(value[key],expected,'runtime host '+key)
    for key in ['php_fiber_compatibility_verified_for_fixed_cases','full_php_compatibility_verified',
            'product_runtime_selected','native_runtime_fallback_used','memory_limit_verified','cpu_limit_verified',
            'all_resources_immediately_reclaimed','sysroot_source_build_attestation_verified',
            'physical_stack_free_verified','physical_stack_reuse_verified','stack_guards_verified',
            'persistent_requests_supported','zts_supported','real_db','full_wordpress_bootstrap_executed']:
        equal(value[key],False,'unclaimed runtime gate '+key)
    equal(value['source'],{'repository':'wasix-org/php','commit':'f4bfae79d5c19028349082f707930cf0951d23b9',
        'kind':'same-job-patched-source-build','verified':True},'fixed PHP source')
    for key, expected in [('path',combined.MODULE.relative_to(ROOT).as_posix()),('verified',True),
            ('observed_bytes',provenance['module']['bytes']),('observed_sha256',provenance['module']['sha256'])]:
        equal(value['artifact'][key],expected,'runtime artifact '+key)
    equal(value['build_receipt']['path'],combined.RECEIPT.relative_to(ROOT).as_posix(),'runtime build receipt')
    equal(value['build_receipt']['observed_sha256'],provenance['build_receipt_sha256'],'runtime receipt digest')
    equal(value['build_receipt']['verified'],True,'runtime receipt verified')
    equal(value['build_receipt']['full_source_tool_chain_verified_by_host'],False,'external source chain ownership')
    equal(value['source_files'],sources(combined.embedded_sources(), read_source),'combined compiled sources')
    equal(value['runtime_source_files'],sources(runtime_sources(), read_source),'runtime compiled sources')
    equal(value['wordpress_source_files'],{name:{**row,'verified':True} for name,row in prepared['files'].items()},'actual WP sources')
    equal(value['isolation'],{'asynchronous_threading':True,'deep_sleep':False,'max_guest_threads':1,
        'host_environment_forwarded':False,'home_mapped':False,'guest_environment_supplied':native.expected_environment(),
        'host_directories_mapped':False,'guest_mounts':['/fixture','/wp-core','/scratch'],
        'networking':'UnsupportedVirtualNetworking','http_client':False,'stdin':'closed-pipe',
        'stdout_limit_bytes':65536,'stderr_limit_bytes':65536,'other_virtual_root_writes_forbidden':False,
        'writable_fixture_entries':['/scratch/ordinary.txt'],'scratch_file_limit_bytes':4096},'runtime capabilities')
    combined.validate_inspection(value['postopt_adapter'],provenance['module'])
    oracle = verify_native_reference(read_source)
    equal(value['native_oracle'],{'path':ORACLE.relative_to(ROOT).as_posix(),'sha256':hashlib.sha256(combined.validation_source(ORACLE, read_source)).hexdigest(),
        'expected_case_count':6,'expected_values_weakened':False},'runtime native oracle')
    equal(value['exception_support']['execution_verdict'],'fixed-cgi-runtime-corpus-passed','runtime exception scope')
    require(type(value['compile_elapsed_ms']) is int and value['compile_elapsed_ms']>=0,'runtime compile span')
    require(type(value['cases']) is list and len(value['cases'])==3,'three runtime requests')
    keys=['ordinary_write_calls','ordinary_bytes_written','ordinary_truncate_opens','ordinary_set_len_calls',
        'readonly_mutating_open_rejections','readonly_handle_mutation_rejections','namespace_mutation_rejections',
        'open_handles','closed_handles']
    for index,name in enumerate(['normal_first','normal_repeat','normal_final']):
        case=value['cases'][index]
        require(not any(key in case for key in ['host_execution_error','expectation_error','guest_json_error']), 'retained runtime failure')
        for key,expected in [('name',name),('request_number',index+1),('module_delivery','Module.clone'),
                ('module_compile_invocations',0),('fresh_wasi_runtime',True),('fresh_scratch',True),('ok',True),
                ('native_semantics_match',True),('expectation_met',True),('guest_json_decoded',True),
                ('guest_arguments',native.command_arguments()),('guest_environment_supplied',native.expected_environment()),
                ('program_sha256',hashlib.sha256(combined.validation_source(FIXTURE, read_source)).hexdigest()),('stdout_eof',True),('stderr_eof',True),('stdout_overflow',False),
                ('stderr_overflow',False),('stderr_bytes',0),('stderr_hex',''),('runtime_shutdown_returned',True),
                ('runtime_shutdown_wait_limit_ms',2000)]:
            equal(case[key],expected,'runtime request '+key)
        require(type(case['guest_execution_elapsed_ms']) is int and case['guest_execution_elapsed_ms']>=0,'runtime guest span')
        wire=bytes.fromhex(case['stdout_hex']);require(0<len(wire)<=65536,'runtime stdout quota')
        equal(case['stdout_bytes'],len(wire),'runtime stdout bytes')
        equal(case['stdout_sha256'],hashlib.sha256(wire).hexdigest(),'runtime stdout digest')
        response,body,_=native.parse_completed_cgi(wire)
        equal(response,case['response'],'runtime raw CGI parsing');equal(response,oracle['response'],'runtime native CGI response')
        observation=parse_json(body);native.validate_observation(observation)
        equal(observation,case['guest_json'],'runtime raw JSON');equal(observation,oracle['observation'],'runtime native semantics')
        initial={'ordinary_hex':'','readonly_hex':b'readonly fixture\n'.hex(),**{key:0 for key in keys}}
        equal(case['scratch_initial'],initial,'fresh scratch state')
        final=case['scratch_final'];equal(set(final),set(initial),'complete scratch snapshot')
        equal(final['ordinary_hex'],b'ordinary\0\xff\r\n'.hex(),'ordinary physical bytes')
        equal(final['readonly_hex'],initial['readonly_hex'],'readonly physical bytes')
        require(all(type(final[key]) is int and 0<=final[key]<=65536 for key in keys),'actual bounded scratch counter types')
        require(final['ordinary_write_calls']>=1 and final['ordinary_bytes_written']==12
            and final['readonly_mutating_open_rejections']>=1 and final['open_handles']==0
            and final['closed_handles']>=4,'actual IO/denial and scratch handle close evidence')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-cgi-runtime-comparison.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'backend': backend,
        'platform': {'system': platform.system(), 'machine': platform.machine()}, 'variant': VARIANT,
        'outcome': 'not_run', 'runtime_contract_passed': False, 'owned_host_reaped': False,
        'product_runtime_selected': False, 'full_php_compatibility_verified': False,
        'native_runtime_fallback_used': False, 'all_resources_immediately_reclaimed': False,
        'memory_limit_verified': False, 'cpu_limit_verified': False, 'stack_free_or_reuse_verified': False,
        'outer_build_timeout_seconds': 1200, 'outer_execution_timeout_seconds': 600}
    passed, stage = False, 'build_provenance'
    try:
        require(platform.system() == 'Linux', 'CGI runtime corpus requires its Linux build job')
        before = source_hashes()
        report['input_files'] = before
        provenance = verify_build()
        prepared = preparer.verify_prepared()
        report['wordpress_preparation'] = prepared
        report['generated_provenance'] = provenance
        stage = 'host_build'
        environment = os.environ.copy()
        command = ['cargo', 'build', '--release', '--manifest-path', str(HOST / 'Cargo.toml'),
            '--target-dir', str(host.TARGET), '--locked', '--offline', '--message-format=json']
        if args.llvm:
            command += ['--no-default-features', '--features', 'llvm']
        _, built = owned_probe.run_captured_probe(command, environment, timeout=1200)
        report['build_exit_code'] = built.returncode
        if built.returncode:
            report['diagnostics'] = host.diagnostic(built.stderr, 8192)
        require(built.returncode == 0, 'CGI runtime Rust host build failed')
        executable = host.select_artifact(built.stdout)
        binary_sha = digest(executable)
        report['native_artifact'] = executable.relative_to(ROOT).as_posix()
        report['native_artifact_sha256'] = binary_sha
        stage = 'execution'
        child_environment = {k: v for k, v in environment.items() if k.upper() not in {
            'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE'}}
        pid, result = owned_probe.run_captured_probe([str(executable), '--cgi-runtime-contract'], child_environment, timeout=600)
        report.update(owned_process_pid=pid, owned_host_reaped=True, exit_code=result.returncode)
        require(len(result.stdout) <= 512 * 1024 and len(result.stderr) <= 65536, 'CGI runtime host capture budget')
        report['host_stderr_sha256'] = hashlib.sha256(result.stderr).hexdigest()
        if result.stderr:
            report['host_stderr_excerpt'] = host.diagnostic(result.stderr, 8192)
        require(result.stdout.endswith(b'\n') and result.stdout.count(b'\n') == 1, 'one host JSON line')
        value = parse_json(result.stdout)
        report['native_result'] = value
        equal(source_hashes(), before, 'CGI runtime sources unchanged after execution')
        equal(digest(executable), binary_sha, 'CGI runtime native host unchanged')
        equal(verify_build(), provenance, 'runtime generated source/tool/module chain unchanged')
        equal(preparer.verify_prepared(), prepared, 'runtime WP preparation unchanged')
        require(result.returncode == 0, 'patched PHP CGI runtime host execution failed')
        stage = 'contract_verification'
        validate_result(value, pid, backend, provenance, prepared)
        passed, report['runtime_contract_passed'] = True, True
        report['outcome'] = 'fixed_php_cgi_runtime_contract_passed'
    except subprocess.TimeoutExpired as error:
        report['outcome'] = stage + '_timeout'
        report['diagnostics'] = 'owned child exceeded finite deadline'
        if error.stderr:
            report['host_stderr_excerpt'] = host.diagnostic(error.stderr, 8192)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = stage + '_failed'
        report['diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    finally:
        try:
            owned_probe.reap_owned_probes()
        except (Exception, KeyboardInterrupt) as error:
            passed, report['runtime_contract_passed'], report['outcome'] = False, False, 'cleanup_failed'
            report['cleanup_diagnostics'] = host.diagnostic(str(error).encode('utf-8', errors='replace'), 8192)
    if args.record:
        (HERE / 'evidence').mkdir(exist_ok=True)
        (HERE / ('evidence/' + platform.system().lower() + '-' + backend + '-cgi-runtime-contract.json')).write_text(
            json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('PHP WASIX CGI runtime corpus: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
