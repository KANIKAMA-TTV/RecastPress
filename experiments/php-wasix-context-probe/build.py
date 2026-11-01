"""Build one fixed small C/WASIX context module, without running guest code.

Reuses the four verified CGI tool archives and their preparation receipt only.
PHP/dependency checkouts and PHP build steps are neither needed nor executed.
verify_build() is the read-only, same-job provenance API for the Rust checker.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
WORK = ROOT / 'target/php-wasix-context-probe'
MANIFEST = HERE / 'manifest.json'
RECEIPT = HERE / 'evidence/build.json'
CGI = ROOT / 'experiments/php-wasix-cgi-build'
sys.path.insert(0, str(CGI))
import prepare as tool_prepare

# The caller may itself import this file as "build". Avoid importing that name
# again when loading the existing CGI module's read-only tool verifier.
_spec = importlib.util.spec_from_file_location('_rp_context_cgi_tools', CGI / 'build.py')
_cgi_tools = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(_cgi_tools)
require, digest = tool_prepare.require, tool_prepare.digest

COMPILE_FLAGS = ['-std=c11', '-O1', '-g0', '-fno-optimize-sibling-calls', '-fno-lto']
ASSEMBLY_FLAGS = ['-g0', '-fno-lto']
EXPORTS = ['rp_stack_switch_raw', 'rp_stack_entry_a', 'rp_stack_entry_b',
           'rp_stack_dispatch_a', 'rp_stack_dispatch_b']
LINK_FLAGS = ['-sMODULE_KIND=static-main', '-fno-lto'] + ['-Wl,--export=' + name for name in [*EXPORTS, '__stack_pointer']]
OPT_FLAGS = ['--emit-exnref', '--all-features', '--strip-debug', '--asyncify',
             '--pass-arg=asyncify-imports@wasix_32v1.proc_snapshot',
             '--pass-arg=asyncify-ignore-indirect', '--pass-arg=max-func-params@32']
ENVIRONMENT = {'WASIXCC_WASM_EXCEPTIONS': 'legacy', 'WASIXCC_INCLUDE_CPP_SYMBOLS': 'yes',
               'WASIXCC_RUN_WASM_OPT': 'no', 'WASIXCC_PIC': 'no', 'WASIXCC_GENERATE_SHELL_SCRIPT': 'no'}
FIXTURE = 'experiments/php-wasix-context-probe/fixtures/context_contract.c'
ADAPTER = ['experiments/php-wasix-context-probe/fixtures/stack_adapter.' + suffix for suffix in ['c', 'h', 'S']]
OBJECTS = ['target/php-wasix-context-probe/' + name for name in
           ['context.fixture.o', 'context.adapter-c.o', 'context.adapter-asm.o']]
RAW = 'target/php-wasix-context-probe/context.managed.raw.wasm'
WASM = 'target/php-wasix-context-probe/context.managed.wasm'
STACK_ADAPTER = {
    'slots': 2, 'stack_bytes_per_context': 65536, 'alignment_bytes': 16,
    'buffers_freed': False, 'buffers_reused': False, 'allocation_lifetime': 'owned guest process exit',
    'forced_include_scope': 'original fixture translation unit only',
    'switch_policy': 'Wasm local saves current SP; raw import returns before SP restoration; no target-SP prechange',
    'postopt_exports': EXPORTS, 'postopt_stack_global': '__stack_pointer',
    'postopt_stackless_verified_by_builder': False,
}


def json_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def parse_json(wire):
    def bad_number(_):
        raise ValueError('non-finite JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=json_object, parse_constant=bad_number)


def fixed_file(name, limit):
    path = ROOT.joinpath(*tool_prepare.relative_parts(name))
    require(path.resolve(strict=True) == path and path.is_file(), 'fixed input escaped through a link: ' + name)
    require(0 < path.stat().st_size <= limit, 'fixed input byte budget: ' + name)
    return path


def load_manifest():
    manifest = parse_json(fixed_file(MANIFEST.relative_to(ROOT).as_posix(), 1024 * 1024).read_bytes())
    expected = {
        'format_version': 1, 'schema': 'recastpress.wasix-context-build.v0.5',
        'scope': 'fixed small C context candidate; no PHP backend or product adoption',
        'context_api_reference': {
            'repository': 'wasix-org/wasix-libc', 'commit': '09503b230e8acc721c1e013ca28a41bf149e4ee2',
            'header': 'libc-bottom-half/headers/public/wasix/context.h',
            'role': 'consulted public ABI source; not a verified source-build identity of the downloaded sysroot'},
        'sysroot_source_build_attestation_verified': False,
        'toolchain_manifest': 'experiments/php-wasix-cgi-build/manifest.json',
        'toolchain_receipt': 'experiments/php-wasix-cgi-build/evidence/toolchain.json',
        'fixture': FIXTURE, 'rounds': 8, 'depth': 4, 'b_extra_rounds': 2,
        'stack_lifetime_case': 'A unwinds while B remains suspended; main clobbers before B resumes',
        'variant': 'managed-guest-linear-stacks-v1', 'adapter_sources': ADAPTER,
        'stack_adapter': STACK_ADAPTER, 'object_outputs': OBJECTS,
        'raw_output': RAW, 'wasm_output': WASM,
        'compile_flags': COMPILE_FLAGS, 'assembly_flags': ASSEMBLY_FLAGS, 'link_flags': LINK_FLAGS,
        'optimizer_flags': OPT_FLAGS, 'tool_environment': ENVIRONMENT,
        'command_timeout_seconds': 120, 'command_cleanup_timeout_seconds': 5,
        'command_log_limit_bytes': 65536, 'module_limit_bytes': 67108864,
        'guest_executed': False, 'php_executed': False, 'product_runtime_selected': False,
        'redistributed_binaries': False,
    }
    require(type(manifest) is dict and set(manifest) == set(expected)
            and all(type(manifest[k]) is type(v) for k, v in expected.items()), 'context build definition types changed')
    require(manifest == expected, 'fixed context build definition changed')
    tool_prepare.load_manifest()
    fixed_file(FIXTURE, 1024 * 1024)
    for source in ADAPTER:
        fixed_file(source, 1024 * 1024)
    require(WORK.resolve() == WORK and WORK.is_relative_to(ROOT / 'target'), 'owned build directory escaped workspace')
    return manifest


def input_hashes():
    result = tool_prepare.input_hashes()
    for path in [Path(__file__).resolve(), MANIFEST, ROOT / FIXTURE, *[ROOT / source for source in ADAPTER]]:
        result[path.relative_to(ROOT).as_posix()] = digest(path)
    return dict(sorted(result.items()))


def verify_toolchain(manifest):
    tool_manifest = tool_prepare.load_manifest()
    path = fixed_file(manifest['toolchain_receipt'], 16 * 1024 * 1024)
    receipt = parse_json(path.read_bytes())
    require(receipt['input_files'] == tool_prepare.input_hashes(), 'tool preparation source set changed')
    receipt_sha = _cgi_tools.verify_tools(tool_manifest)
    require(digest(path) == receipt_sha, 'tool preparation receipt changed during verification')
    return {
        'manifest': manifest['toolchain_manifest'], 'manifest_sha256': digest(ROOT / manifest['toolchain_manifest']),
        'receipt': manifest['toolchain_receipt'], 'receipt_sha256': receipt_sha,
        'verification_scope': 'pinned archive size/SHA and extracted tool identities; no source-build attestation',
        'assets': [{'name': a['name'], 'asset_id': a['asset_id'], 'bytes': a['bytes'], 'sha256': a['sha256']}
                   for a in tool_manifest['assets']],
    }


def commands():
    compiler = 'target/php-wasix-cgi-build/tools/bin/wasixcc'
    return [
        ('compile_fixture', [compiler, *COMPILE_FLAGS, '-include', ADAPTER[1], '-c', FIXTURE, '-o', OBJECTS[0]]),
        ('compile_adapter_c', [compiler, *COMPILE_FLAGS, '-c', ADAPTER[0], '-o', OBJECTS[1]]),
        ('compile_adapter_asm', [compiler, *ASSEMBLY_FLAGS, '-c', ADAPTER[2], '-o', OBJECTS[2]]),
        ('link_context', [compiler, *LINK_FLAGS, *OBJECTS, '-o', RAW]),
        ('optimize_context', ['target/php-wasix-cgi-build/tools/binaryen/bin/wasm-opt', RAW, *OPT_FLAGS, '-o', WASM]),
    ]


def write_report(report):
    require(RECEIPT.resolve().is_relative_to(HERE) and not RECEIPT.is_symlink(), 'context receipt escaped experiment')
    RECEIPT.parent.mkdir(exist_ok=True)
    temporary = RECEIPT.with_suffix('.partial')
    require(not temporary.is_symlink(), 'context receipt temporary is a link')
    temporary.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    temporary.replace(RECEIPT)


def run_step(name, command, environment, report, record):
    step = {'name': name, 'command': command, 'working_directory': '<workspace>',
            'completed': False, 'timeout_seconds': 120,
            'process_group_timeout_cleanup': False, 'log_budget_checked_after_exit': True}
    report['phase'] = name
    report['build_steps'].append(step)
    if record:
        write_report(report)
    started = time.monotonic()
    # No inherited PIPE writers can block timeout cleanup. An unnamed owned
    # temporary log is checked after exit; this is not a live filesystem quota.
    with tempfile.TemporaryFile(dir=WORK) as log:
        child = subprocess.Popen(command, cwd=ROOT, env=environment, shell=False,
                                 stdin=subprocess.DEVNULL, stdout=log, stderr=log,
                                 start_new_session=True)
        try:
            exit_code = child.wait(timeout=120)
        except BaseException as error:
            # A still-unreaped captured child reserves this session/group ID.
            # Do not poll/reap it before killing its owned compiler process tree.
            # Popen.wait may already have reaped on KeyboardInterrupt; in that
            # case returncode is set and the old numeric group is not reused.
            if child.returncode is None:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=5)
                step['process_group_timeout_cleanup'] = True
            if isinstance(error, subprocess.TimeoutExpired):
                raise RuntimeError('fixed build command exceeded 120 seconds: ' + name) from error
            raise
        finally:
            step['elapsed_ms'] = round((time.monotonic() - started) * 1000)
        size = log.seek(0, os.SEEK_END)
        step['log_bytes'] = size
        require(size <= 65536, 'build command log exceeded post-exit budget')
        log.seek(0)
        wire = log.read(65537)
        step['log_sha256'] = hashlib.sha256(wire).hexdigest()
        step['exit_code'], step['completed'] = exit_code, True
        if exit_code:
            report['diagnostics'] = wire.decode('utf-8', errors='replace').replace(str(ROOT), '<workspace>')[-8192:]
        require(exit_code == 0, 'fixed C build command failed: ' + name)


def wasm_identity(name):
    path = fixed_file(name, 67108864)
    with path.open('rb') as source:
        require(source.read(8) == b'\0asm\x01\0\0\0', 'generated C candidate is not WebAssembly v1')
    return {'path': name, 'bytes': path.stat().st_size, 'sha256': digest(path),
            'wasm_header_verified': True, 'executed': False}


def verify_build():
    """Read-only same-job verification; no download, compiler or guest call."""
    manifest = load_manifest()
    path = fixed_file(RECEIPT.relative_to(ROOT).as_posix(), 16 * 1024 * 1024)
    wire = path.read_bytes()
    report = parse_json(wire)
    require(report['format_version'] == 1 and report['schema'] == manifest['schema']
            and report['outcome'] == 'context_candidate_built_not_executed'
            and report['phase'] == 'build_complete', 'C candidate build did not complete')
    for key in ['guest_executed', 'php_executed', 'php_backend_modified', 'product_runtime_selected',
                'redistributed_binaries', 'sysroot_source_build_attestation_verified']:
        require(report[key] is False, 'context build scope changed: ' + key)
    require(report['input_files'] == input_hashes(), 'context build sources changed')
    require(report['toolchain'] == verify_toolchain(manifest), 'context build tool provenance changed')
    require(report['build_config'] == manifest, 'context build configuration differs')
    require(report['source'] == {'path': FIXTURE, 'bytes': (ROOT / FIXTURE).stat().st_size,
                                 'sha256': digest(ROOT / FIXTURE)}, 'context C source changed')
    require(report['variant'] == 'managed-guest-linear-stacks-v1' and report['stack_adapter'] == STACK_ADAPTER,
            'context managed adapter scope changed')
    expected_adapter = {name: {'bytes': (ROOT / name).stat().st_size, 'sha256': digest(ROOT / name)} for name in ADAPTER}
    require(report['adapter_sources'] == expected_adapter, 'context adapter sources changed')
    require(report['object_files'] == {name: wasm_identity(name) for name in OBJECTS}, 'context object bytes changed')
    require(len(report['build_steps']) == 5, 'context build step count')
    for step, (name, command) in zip(report['build_steps'], commands()):
        require(step['name'] == name and step['command'] == command and step['working_directory'] == '<workspace>'
                and step['completed'] is True and type(step['exit_code']) is int and step['exit_code'] == 0
                and step['process_group_timeout_cleanup'] is False, 'context build command incomplete')
    require(report['raw_context_wasm'] == wasm_identity(RAW) and report['context_wasm'] == wasm_identity(WASM),
            'context generated bytes changed')
    require(report['binaryen_validation_enabled'] is True, 'Binaryen validation must remain enabled')
    require(digest(path) == hashlib.sha256(wire).hexdigest(), 'context build receipt changed')
    return {'build_receipt_sha256': hashlib.sha256(wire).hexdigest(),
            'manifest_sha256': digest(MANIFEST), 'source_sha256': digest(ROOT / FIXTURE),
            'toolchain_receipt_sha256': report['toolchain']['receipt_sha256'],
            'toolchain_manifest_sha256': report['toolchain']['manifest_sha256'],
            'module_bytes': report['context_wasm']['bytes'], 'module_sha256': report['context_wasm']['sha256'],
            'variant': report['variant'], 'adapter_sources': expected_adapter,
            'verification_scope': 'same-job generated small C candidate; not reproducible-build or signed attestation'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true')
    parser.add_argument('--validate-only', action='store_true')
    args = parser.parse_args()
    report = {'format_version': 1, 'schema': 'recastpress.wasix-context-build.v0.5',
              'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'outcome': 'not_run', 'phase': 'verification', 'build_steps': [],
              'guest_executed': False, 'php_executed': False, 'php_backend_modified': False,
              'sysroot_source_build_attestation_verified': False,
              'variant': 'managed-guest-linear-stacks-v1', 'stack_adapter': STACK_ADAPTER,
              'adapter_sources': {}, 'object_files': {},
              'product_runtime_selected': False, 'redistributed_binaries': False,
              'binaryen_validation_enabled': True, 'runtime_module_validation_performed': False,
              'platform': {'system': platform.system(), 'machine': platform.machine()}}
    passed = False
    try:
        manifest = load_manifest()
        if args.validate_only:
            print('Fixed context C/driver definitions validated; no tools, build or guest executed.')
            return 0
        require(platform.system() == 'Linux' and platform.machine() in ('x86_64', 'AMD64'), 'fixed build tools require Linux x86-64')
        for name in [*OBJECTS, RAW, WASM]:
            output = ROOT / name
            require(output.resolve() == output and not output.is_symlink() and not output.exists(),
                    'context output is linked or already exists; refusing stale reuse')
        WORK.mkdir(parents=True, exist_ok=True)
        report['input_files'] = input_hashes()
        report['build_config'] = manifest
        report['toolchain'] = verify_toolchain(manifest)
        source = fixed_file(FIXTURE, 1024 * 1024)
        report['source'] = {'path': FIXTURE, 'bytes': source.stat().st_size, 'sha256': digest(source)}
        report['adapter_sources'] = {name: {'bytes': (ROOT / name).stat().st_size, 'sha256': digest(ROOT / name)} for name in ADAPTER}
        environment = tool_prepare.tool_environment()
        environment.update(ENVIRONMENT)
        for name, command in commands():
            run_step(name, command, environment, report, args.record)
            if name in ['compile_fixture', 'compile_adapter_c', 'compile_adapter_asm']:
                output = command[-1]
                report['object_files'][output] = wasm_identity(output)
            if name == 'link_context':
                report['raw_context_wasm'] = wasm_identity(RAW)
        report['context_wasm'] = wasm_identity(WASM)
        require(input_hashes() == report['input_files'], 'context source changed during build')
        require(verify_toolchain(manifest) == report['toolchain'], 'context toolchain changed during build')
        report['outcome'], report['phase'], passed = 'context_candidate_built_not_executed', 'build_complete', True
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = 'context_build_failed'
        report.setdefault('diagnostics', str(error).replace(str(ROOT), '<workspace>')[:8192])
    finally:
        if args.record and not args.validate_only:
            report.setdefault('input_files', input_hashes())
            write_report(report)
    print('Fixed WASIX context C build: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
