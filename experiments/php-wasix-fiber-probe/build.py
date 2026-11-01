"""Build the fixed opt-in NTS fresh-WASI PHP CLI candidate; never run it.

Linux CI supplies independent pinned source checkouts and the existing four
verified CGI tool assets. --validate-only reads definitions on any platform;
it never installs, downloads, patches, compiles or executes PHP.
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
import selectors
import signal
import subprocess
import time

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
WORK = ROOT / 'target/php-wasix-fiber-probe'
MANIFEST = HERE / 'manifest.json'
RECEIPT = HERE / 'evidence/build.json'
CGI = ROOT / 'experiments/php-wasix-cgi-build'


def load_unique(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


prepare = load_unique('recastpress_fiber_pinned_tool_prepare', CGI / 'prepare.py')
patch_source = load_unique('recastpress_fiber_source_patch', HERE / 'patch_source.py')
require, digest = prepare.require, prepare.digest
VARIANT = 'nts-fresh-wasi-zend-fibers-v1'
SOURCE_PINS = {
    'php': {'repository': 'wasix-org/php', 'commit': 'f4bfae79d5c19028349082f707930cf0951d23b9', 'directory': 'source/php'},
    'deps': {'repository': 'wasix-org/php-wasix-deps', 'commit': 'b77ecbede97aba2df32c86eaeb54ff9f5e44f543', 'directory': 'source/php-wasix-deps'},
    'parser': {'repository': 'nikic/PHP-Parser', 'commit': 'f103601b29efebd7ff4a1ca7b3eeea9e3336a2a2', 'directory': 'source/php/build/PHP-Parser-5.6.1'},
}
EXPORTS = ['rp_zend_wasix_switch_raw', 'rp_zend_wasix_entry', 'rp_zend_wasix_dispatch',
           'rp_zend_wasix_bootstrap_sp', '__stack_pointer']
OPT_FLAGS = ['--emit-exnref', '--all-features', '--strip-debug', '--asyncify',
             '--pass-arg=asyncify-imports@wasix_32v1.proc_snapshot',
             '--pass-arg=asyncify-ignore-indirect', '--pass-arg=max-func-params@32']
FLAGS = ['--enable-rp-wasix-fibers', '--disable-zts']
LINK_FLAGS = ['--export=' + name for name in EXPORTS]
EXTRA_ENV = {'WASIXCC_COMPILER_POST_FLAGS': '-fno-lto', 'WASIXCC_LINKER_FLAGS': ':'.join(LINK_FLAGS),
             'WASIXCC_RUN_WASM_OPT': 'no', 'WASIXCC_PIC': 'no', 'WASIXCC_GENERATE_SHELL_SCRIPT': 'no'}
RAW = 'target/php-wasix-fiber-probe/php-fiber.raw.wasm'
WASM = 'target/php-wasix-fiber-probe/php-fiber.wasm'
LINKED = 'target/php-wasix-fiber-probe/source/php/sapi/cli/php'
MODULE_LIMIT = 128 * 1024 * 1024


def parse_json(wire):
    def pairs(rows):
        result = {}
        for key, value in rows:
            require(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def bad_number(_):
        raise ValueError('non-finite JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=pairs, parse_constant=bad_number)


def owned(name):
    path = WORK.joinpath(*prepare.relative_parts(name))
    require(WORK.resolve() == WORK and WORK.is_relative_to(ROOT / 'target')
            and path.resolve() == path and path.is_relative_to(WORK), 'Fiber build path escaped fixed target')
    return path


def fixed(path, limit=16 * 1024 * 1024):
    require(path.resolve(strict=True) == path and path.is_file() and 0 < path.stat().st_size <= limit,
            'fixed file missing/linked/oversized: ' + path.name)
    return path


def public(value):
    return str(value).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')


def load_manifest():
    manifest = parse_json(fixed(MANIFEST, 1024 * 1024).read_bytes())
    require(manifest['format_version'] == 1 and manifest['schema'] == 'recastpress.wasix-php-fiber-draft.v0.5'
            and manifest['scope'] == 'NTS wasm32 PHP 8.5.12, exactly one request per fresh WASI instance', 'unexpected Fiber design')
    require(manifest['upstream']['repository'] == SOURCE_PINS['php']['repository'], 'Fiber source repository changed')
    patch_source.validate_plan(manifest)
    design = manifest['build_design']
    require(design['toolchain_manifest'] == 'experiments/php-wasix-cgi-build/manifest.json'
            and digest(CGI / 'manifest.json') == design['toolchain_manifest_sha256'], 'pinned tool definition changed')
    tool_manifest = prepare.load_manifest()
    for name, pin in SOURCE_PINS.items():
        require(all(tool_manifest['sources'][name][key] == pin[key] for key in pin), 'inherited source pin changed')
        owned(pin['directory'])
    require(design['configure_script'] == 'wasix-configure-eh-64.sh' and design['extra_configure_flags'] == FLAGS
            and design['make_target'] == 'cli' and design['regenerate_configure_after_patch'] is True
            and design['reuse_cgi_source_checkout'] is False and design['inherit_pinned_assets_only'] is True,
            'fixed one-shot NTS CLI procedure changed')
    require(design['required_extra_c_cxx_flags'] == ['-fno-lto'] and design['assembly_extra_flags'] == ['-fno-lto']
            and design['required_link_flags'] == ['-Wl,' + flag for flag in LINK_FLAGS]
            and design['backend_macro'] == 'ZEND_FIBER_WASIX' and design['default_enabled'] is False,
            'Fiber backend compiler/link policy changed')
    fixture = manifest['first_comparison_plan']['fixed_fixture']
    require(fixture['path'] == 'fixtures/fiber_contract.php' and digest(fixed(HERE / fixture['path'])) == fixture['sha256'],
            'fixed comparison fixture changed')
    require(manifest['sysroot_source_build_attestation_verified'] is False
            and all(value is False for value in manifest['claims'].values()), 'draft must not become an execution claim')
    return manifest


def input_hashes():
    result = prepare.input_hashes()
    paths = [Path(__file__).resolve(), HERE / 'patch_source.py', MANIFEST, HERE / patch_source.PATCH,
             patch_source.ASSEMBLY, HERE / 'fixtures/fiber_contract.php']
    for path in paths:
        result[path.relative_to(ROOT).as_posix()] = digest(fixed(path))
    return dict(sorted(result.items()))


def write_report(report):
    require(RECEIPT.resolve().is_relative_to(HERE) and not RECEIPT.is_symlink(), 'Fiber build receipt escaped experiment')
    RECEIPT.parent.mkdir(exist_ok=True)
    temporary = RECEIPT.with_suffix('.partial')
    require(not temporary.is_symlink(), 'Fiber build receipt temporary is a link')
    temporary.write_text(json.dumps(report, ensure_ascii=True, indent=2) + '\n', encoding='utf-8', newline='\n')
    temporary.replace(RECEIPT)


def run(command, cwd, environment, timeout=30, limit=1024 * 1024, step=None):
    """Linux only: live output quota, finite session/group kill and leader reap.

    Do not poll/reap the leader before timeout cleanup: its unreaped PID reserves
    the owned process-group identifier even when a grandchild keeps a pipe open.
    """
    require(platform.system() == 'Linux', 'build commands require Linux; validate-only executes none')
    started = time.monotonic()
    child = subprocess.Popen(command, cwd=cwd, env=environment, shell=False, stdin=subprocess.DEVNULL,
                             stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
    captured = {'stdout': bytearray(), 'stderr': bytearray()}
    cleanup = False
    capture_complete = False
    try:
        with selectors.DefaultSelector() as selector:
            for name, pipe in [('stdout', child.stdout), ('stderr', child.stderr)]:
                os.set_blocking(pipe.fileno(), False)
                selector.register(pipe, selectors.EVENT_READ, name)
            while selector.get_map():
                remaining = timeout - (time.monotonic() - started)
                require(remaining > 0, 'owned command deadline exceeded')
                for key, _ in selector.select(min(remaining, 0.25)):
                    block = os.read(key.fileobj.fileno(), 65536)
                    if not block:
                        selector.unregister(key.fileobj)
                    else:
                        require(sum(len(value) for value in captured.values()) + len(block) <= limit,
                                'owned command live output quota exceeded')
                        captured[key.data].extend(block)
            capture_complete = True
            child.wait(timeout=max(0.001, timeout - (time.monotonic() - started)))
        require(child.returncode == 0, 'fixed command failed: ' + Path(command[0]).name + '; ' +
                public(captured['stderr'].decode('utf-8', errors='replace'))[-4096:])
        return bytes(captured['stdout'])
    except BaseException:
        if child.returncode is None:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=5)
            cleanup = True
        raise
    finally:
        child.stdout.close()
        child.stderr.close()
        if step is not None:
            step.update(completed=child.returncode is not None, exit_code=child.returncode,
                        elapsed_ms=round((time.monotonic() - started) * 1000), timeout_seconds=timeout,
                        output_quota_bytes=limit, live_output_quota_enforced=True,
                        output_capture_complete=capture_complete,
                        process_group_failure_cleanup=cleanup, owned_leader_reaped=child.returncode is not None,
                        stdout_bytes=len(captured['stdout']), stderr_bytes=len(captured['stderr']),
                        stdout_sha256=hashlib.sha256(captured['stdout']).hexdigest(),
                        stderr_sha256=hashlib.sha256(captured['stderr']).hexdigest())


def verify_tools(manifest):
    tool_manifest = prepare.load_manifest()
    path = fixed(CGI / 'evidence/toolchain.json')
    wire = path.read_bytes()
    receipt = parse_json(wire)
    require(receipt['outcome'] == 'verified_host_build_tools_prepared' and receipt['guest_executed'] is False,
            'four host tools have not been prepared successfully')
    require(receipt['input_files'] == prepare.input_hashes(), 'tool preparation definitions changed')
    rows = receipt['assets']
    require(len(rows) == 4 and {row['name'] for row in rows} == {asset['name'] for asset in tool_manifest['assets']},
            'prepared tool set changed')
    for asset in tool_manifest['assets']:
        prepare.verify_archive(prepare.owned_path('downloads/' + asset['name'] + '.tar.gz'), asset)
        row = next(row for row in rows if row['name'] == asset['name'])
        require(row['verified'] is True and row['observed_bytes'] == asset['bytes']
                and row['observed_sha256'] == asset['sha256'] and row['bytes'] == asset['bytes']
                and row['sha256'] == asset['sha256'], 'prepared tool archive identity changed')
        require(set(row['required_files']) == set(asset['required_files']), 'prepared tool required-file set changed')
        directory = prepare.owned_path(asset['directory'])
        for name, identity in row['required_files'].items():
            item = directory.joinpath(*prepare.relative_parts(name))
            require(item.resolve().is_relative_to(directory.resolve()) and item.is_file()
                    and item.stat().st_size == identity['bytes'] and digest(item) == identity['sha256'], 'extracted tool changed')
    binary = prepare.owned_path('tools/wasixcc/wasixccenv')
    for name in ['wasixcc', 'wasixcc++', 'wasixar', 'wasixranlib', 'wasixnm']:
        wrapper = prepare.owned_path('tools/bin/' + name)
        require(wrapper.is_file() and os.path.samefile(wrapper, binary), 'compiler wrapper is not the verified executable')
    require(digest(path) == hashlib.sha256(wire).hexdigest(), 'tool receipt changed during verification')
    return {'manifest': 'experiments/php-wasix-cgi-build/manifest.json', 'manifest_sha256': digest(CGI / 'manifest.json'),
            'receipt': 'experiments/php-wasix-cgi-build/evidence/toolchain.json', 'receipt_sha256': hashlib.sha256(wire).hexdigest(),
            'assets': [{key: asset[key] for key in ['name', 'asset_id', 'bytes', 'sha256']} for asset in tool_manifest['assets']],
            'scope': 'pinned archives and extracted tools, not source-build attestation'}


def git_output(args, directory, environment):
    return run(args, directory, environment)


def command_recipe():
    php = 'target/php-wasix-fiber-probe/source/php'
    return [
        ('configure', ['bash', 'wasix-configure-eh-64.sh'], php, 600),
        ('make_cli', ['make', '-j2', 'PHP=<host-stub-php>', 'cli'], php, 1200),
        ('optimize_cli', ['target/php-wasix-cgi-build/tools/binaryen/bin/wasm-opt', *OPT_FLAGS, RAW, '-o', WASM], '<workspace>', 300),
    ]


def normalized_command(command, host_php=None):
    values = []
    for value in command:
        if host_php is not None and value == 'PHP=' + str(host_php):
            values.append('PHP=<host-stub-php>')
        elif value.startswith(str(ROOT) + os.sep):
            values.append(Path(value).relative_to(ROOT).as_posix())
        else:
            values.append(value)
    return values


def tracked_changes(environment):
    php = owned('source/php')
    names = git_output(['git', 'diff', '--name-only', '-z'], php, environment).decode('utf-8').split('\0')
    return {name: {'bytes': fixed(php.joinpath(*prepare.relative_parts(name)), MODULE_LIMIT).stat().st_size,
                   'sha256': digest(php.joinpath(*prepare.relative_parts(name)))} for name in sorted(set(names) - {''})}


def source_snapshot(environment, require_clean):
    sources = {}
    for name, pin in SOURCE_PINS.items():
        directory = owned(pin['directory'])
        head = git_output(['git', 'rev-parse', 'HEAD'], directory, environment).decode('ascii').strip()
        require(head == pin['commit'], 'independent source HEAD changed: ' + name)
        if require_clean or name != 'php':
            require(not git_output(['git', 'status', '--porcelain', '--untracked-files=no'], directory, environment).strip(),
                    'source is not tracked-clean: ' + name)
        sources[name] = dict(pin)
        sources[name]['directory'] = directory.relative_to(ROOT).as_posix()
    parser = owned(SOURCE_PINS['parser']['directory'])
    require(fixed(parser / 'lib/PhpParser/ParserFactory.php').is_file(), 'pinned parser was not preseeded')
    generator = fixed(owned('source/php/build/gen_stub.php')).read_text(encoding='utf-8')
    require('$version = "5.6.1";' in generator and '$phpParserDir = __DIR__ . "/PHP-Parser-$version";' in generator,
            'stub generator would use an unexpected parser path')
    deps = owned('source/php-wasix-deps')
    archives = {}
    for path in sorted((deps / 'lib-eh').glob('*.a')) + sorted((deps / 'pgsql-eh/lib').glob('*.a')):
        fixed(path, 512 * 1024 * 1024)
        with path.open('rb') as source:
            require(source.read(8) == b'!<arch>\n', 'unmaterialized/invalid dependency archive')
        archives[path.relative_to(deps).as_posix()] = {'bytes': path.stat().st_size, 'sha256': digest(path)}
    require(len(archives) == 28 and 'pgsql-eh/lib/libpq.a' in archives and 'lib-eh/libcurl.a' in archives,
            'pinned 28-archive dependency inventory changed')
    sources['deps'].update(prebuilt_archive_files=archives, rebuilt_here=False, all_archives_linked_verified=False)
    return sources


def configuration_files():
    php = owned('source/php')
    names = ['config.log', 'config.status', 'config.nice', 'configure', 'Makefile', 'main/php_config.h', 'Zend/zend_config.h']
    return {name: {'bytes': fixed(php / name).stat().st_size, 'sha256': digest(php / name)} for name in names if (php / name).is_file()}


def configuration_policy():
    php = owned('source/php')
    headers = '\n'.join((php / name).read_text(encoding='utf-8') for name in ['main/php_config.h', 'Zend/zend_config.h'])
    require(re.search(r'^#define\s+ZEND_FIBER_WASIX\s+1\b', headers, re.M), 'opt-in WASIX Fiber macro is not enabled')
    require(not re.search(r'^#define\s+ZTS\b', headers, re.M), 'ZTS is forbidden for this candidate')
    require(not re.search(r'^#define\s+ZEND_FIBER_UCONTEXT\b', headers, re.M), 'ucontext cannot accompany the WASIX backend')
    makefile = fixed(php / 'Makefile').read_text(encoding='utf-8')
    require('recastpress_wasix_fiber.S' in makefile and '-fno-lto' in makefile, 'fixed assembly build rule is missing')
    return {'backend_enabled': True, 'zts_enabled': False, 'ucontext_enabled': False, 'assembly_rule_observed': True,
            'compiler_post_flags': '-fno-lto', 'source_script_flto_preserved': True,
            'effective_non_lto_basis': 'verified wasixcc appends compiler post flags after original script -flto args'}


def stub_headers():
    php = owned('source/php')
    return {path.relative_to(php).as_posix(): digest(fixed(path)) for path in sorted(php.rglob('*_arginfo.h'))
            if '.git' not in path.relative_to(php).parts}


def wasm_identity(name):
    path = fixed(ROOT / name, MODULE_LIMIT)
    with path.open('rb') as source:
        require(source.read(8) == b'\0asm\x01\0\0\0', 'candidate is not WebAssembly v1')
    return {'path': name, 'bytes': path.stat().st_size, 'sha256': digest(path), 'wasm_header_verified': True, 'executed': False}


def build_config():
    return {'variant': VARIANT, 'sources': SOURCE_PINS, 'configure_command': ['bash', 'wasix-configure-eh-64.sh'],
            'extra_configure_flags': FLAGS, 'make_target': 'cli', 'make_jobs': 2,
            'tool_environment_additions': EXTRA_ENV, 'required_exports': EXPORTS,
            'raw_output': RAW, 'wasm_output': WASM, 'linked_output': LINKED,
            'optimizer_flags': OPT_FLAGS, 'module_limit_bytes': MODULE_LIMIT,
            'command_recipe': [{'name': name, 'command': command, 'working_directory': cwd, 'timeout_seconds': timeout}
                               for name, command, cwd, timeout in command_recipe()],
            'configure_timeout_seconds': 600, 'make_timeout_seconds': 1200, 'optimizer_timeout_seconds': 300,
            'live_command_output_limit_bytes': 16 * 1024 * 1024, 'failure_cleanup_timeout_seconds': 5,
            'configure_difference': 'opt-in --enable-rp-wasix-fibers --disable-zts only; CGI remains disabled',
            'lto_difference': '64bit script unconditionally sets -flto; WASIXCC_COMPILER_POST_FLAGS=-fno-lto overrides it last for C/CXX/assembly',
            'optimizer_difference': 'validation enabled and no -O3; final stackless-shape inspection is required before execution',
            'postopt_inspected_by_builder': False, 'sysroot_source_build_attestation_verified': False}


def verify_build():
    """Read-only same-job receipt/source/tool/generated-byte verification."""
    manifest = load_manifest()
    wire = fixed(RECEIPT).read_bytes()
    report = parse_json(wire)
    require(report['schema'] == 'recastpress.php-wasix-fiber-build.v0.5' and report['variant'] == VARIANT
            and report['outcome'] == 'fiber_candidate_built_not_executed' and report['phase'] == 'build_complete', 'Fiber CLI build did not complete')
    for key in ['guest_executed', 'php_guest_executed', 'patched_php_executed', 'product_runtime_selected',
                'native_runtime_fallback_used', 'redistributed_binaries', 'php_wasm_fiber_compatibility_verified',
                'postopt_module_inspected', 'sysroot_source_build_attestation_verified']:
        require(report[key] is False, 'Fiber builder execution/adoption scope changed')
    require(report['tracked_sources_clean_before_build'] is True and report['binaryen_validation_enabled'] is True,
            'clean source or validation requirement changed')
    require(report['input_files'] == input_hashes() and report['build_config'] == build_config(), 'Fiber build inputs/configuration changed')
    require(report['toolchain'] == verify_tools(manifest), 'Fiber tool provenance changed')
    environment = prepare.tool_environment()
    require(report['sources'] == source_snapshot(environment, False), 'Fiber source/dependency provenance changed')
    after = patch_source.verify_applied(owned('source/php'), manifest)
    require(report['source_patch'] == {
        'path': patch_source.PATCH, 'bytes': patch_source.PATCH_BYTES, 'sha256': patch_source.PATCH_SHA,
        'source_commit': patch_source.COMMIT, 'clean_tracked_source_immediately_before': True,
        'applied': True, 'git_apply_check_passed': True, 'git_apply_passed': True, 'index_updated': False,
        'before': {row['path']: row['before'] for row in manifest['source_patches'][0]['files']},
        'after': after, 'added_assembly_matches_fixed_fixture': True}, 'Fiber patch receipt changed')
    require(report['generated_configuration_files'] == configuration_files() and report['configuration_policy'] == configuration_policy(),
            'Fiber generated configuration changed')
    require(report['tracked_changes_after_build'] == tracked_changes(environment), 'tracked PHP source/generated changes differ')
    require(report['stub_headers_after'] == stub_headers(), 'Fiber generated stub headers changed')
    require(report['raw_wasm'] == wasm_identity(RAW) and report['php_wasm'] == wasm_identity(WASM)
            and report['linked_php_output'] == wasm_identity(LINKED), 'Fiber module bytes changed')
    require(report['raw_wasm']['sha256'] == report['linked_php_output']['sha256'], 'raw candidate copy differs from link output')
    require(len(report['build_steps']) == 3
            and all(step['completed'] is True and step['exit_code'] == 0 and step['owned_leader_reaped'] is True
                    and step['live_output_quota_enforced'] is True and step['output_capture_complete'] is True
                    and step['process_group_failure_cleanup'] is False
                    for step in report['build_steps']), 'Fiber build commands did not complete normally')
    for step, (name, command, cwd, timeout) in zip(report['build_steps'], command_recipe()):
        require(step['name'] == name and step['command'] == command and step['working_directory'] == cwd
                and step['timeout_seconds'] == timeout and step['output_quota_bytes'] == 16 * 1024 * 1024,
                'fixed Fiber build command/deadline/quota changed')
    require(digest(RECEIPT) == hashlib.sha256(wire).hexdigest(), 'Fiber receipt changed during verification')
    return {'build_receipt_sha256': hashlib.sha256(wire).hexdigest(), 'manifest_sha256': digest(MANIFEST),
            'patch_sha256': patch_source.PATCH_SHA, 'assembly_sha256': digest(patch_source.ASSEMBLY),
            'toolchain_receipt_sha256': report['toolchain']['receipt_sha256'], 'input_files': report['input_files'],
            'module_bytes': report['php_wasm']['bytes'], 'module_sha256': report['php_wasm']['sha256'],
            'raw_module_sha256': report['raw_wasm']['sha256'], 'variant': VARIANT,
            'scope': 'same-job generated NTS fresh-WASI CLI candidate; no signed/reproducible-build attestation'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true')
    parser.add_argument('--validate-only', action='store_true')
    args = parser.parse_args()
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-fiber-build.v0.5', 'variant': VARIANT,
              'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'outcome': 'not_run', 'phase': 'verification',
              'guest_executed': False, 'php_guest_executed': False, 'patched_php_executed': False,
              'product_runtime_selected': False, 'native_runtime_fallback_used': False,
              'redistributed_binaries': False, 'php_wasm_fiber_compatibility_verified': False,
              'postopt_module_inspected': False, 'sysroot_source_build_attestation_verified': False,
              'binaryen_validation_enabled': True, 'build_steps': []}
    passed = False
    environment = None
    try:
        manifest = load_manifest()
        if args.validate_only:
            print('Fixed Fiber CLI build definitions validated; no checkout patch, tools, build or PHP executed.')
            return 0
        require(platform.system() == 'Linux' and platform.machine() in ['x86_64', 'AMD64'], 'fixed tools require Linux x86-64')
        for name in ['php-fiber.raw.wasm', 'php-fiber.wasm', 'source/php/Makefile', 'source/php/sapi/cli/php']:
            require(not owned(name).exists() and not owned(name).is_symlink(), 'stale Fiber output/configuration refused')
        WORK.mkdir(parents=True, exist_ok=True)
        environment = prepare.tool_environment()
        report['input_files'], report['build_config'] = input_hashes(), build_config()
        report['toolchain'] = verify_tools(manifest)
        report['sources'] = source_snapshot(environment, True)
        report['tracked_sources_clean_before_build'] = True
        php = owned('source/php')
        report['stub_headers_before'] = stub_headers()
        report['phase'] = 'source_patch'
        report['source_patch'] = patch_source.apply_to_checkout(php, manifest, lambda command, cwd: git_output(command, cwd, environment))
        host_php = Path('/usr/bin/php').resolve(strict=True)
        require(host_php.is_file() and host_php.is_relative_to(Path('/usr/bin')), 'fixed host stub PHP is missing')
        query = 'echo json_encode(["version"=>PHP_VERSION,"version_id"=>PHP_VERSION_ID,"sapi"=>PHP_SAPI,"tokenizer"=>extension_loaded("tokenizer")],JSON_THROW_ON_ERROR);'
        native = parse_json(run([str(host_php), '-r', query], php, environment))
        require(type(native['version_id']) is int and native['version_id'] >= 70400
                and native['sapi'] == 'cli' and native['tokenizer'] is True, 'host stub PHP requires >=7.4 and tokenizer')
        native.update(executable_name=host_php.name, executable_sha256=digest(host_php), role='gen_stub.php build tool only, never product fallback')
        report['host_stub_generator_php'] = native
        report['apt_packages'] = run(['dpkg-query', '-W', '-f=${Package}=${Version}\n', 'autoconf', 'automake', 'bison',
                                     'build-essential', 're2c', 'pkg-config', 'php-cli', 'php-common', 'php-xml'], php, environment).decode('utf-8').splitlines()
        require('export WASIX_EXTRA_FLAGS="-flto"' in (php / 'wasix-configure-eh-64.sh').read_text(encoding='utf-8'), '64bit configure script changed')
        environment.update(EXTRA_ENV)
        environment.update(PHP_WASIX_DEPS=str(owned('source/php-wasix-deps')), WASIX_EXTRA_CONFIGURE_FLAGS=' '.join(FLAGS))
        for name, command, timeout in [
            ('configure', ['bash', 'wasix-configure-eh-64.sh'], 600),
            ('make_cli', ['make', '-j2', 'PHP=' + str(host_php), 'cli'], 1200),
        ]:
            step = {'name': name, 'command': normalized_command(command, host_php), 'working_directory': 'target/php-wasix-fiber-probe/source/php'}
            recipe = command_recipe()[len(report['build_steps'])]
            require((step['name'], step['command'], step['working_directory'], timeout) == recipe, 'build command differs from fixed recipe')
            report['phase'] = name
            report['build_steps'].append(step)
            if args.record:
                write_report(report)
            run(command, php, environment, timeout, 16 * 1024 * 1024, step)
            if name == 'configure':
                report['configuration_policy'] = configuration_policy()
        report['linked_php_output'] = wasm_identity(LINKED)
        with fixed(ROOT / LINKED, MODULE_LIMIT).open('rb') as source, owned('php-fiber.raw.wasm').open('xb') as destination:
            total = 0
            for block in iter(lambda: source.read(1024 * 1024), b''):
                total += len(block)
                require(total <= MODULE_LIMIT, 'linked raw module exceeded copy budget')
                destination.write(block)
        report['raw_wasm'] = wasm_identity(RAW)
        require(report['raw_wasm']['sha256'] == report['linked_php_output']['sha256'], 'linked output changed while copying')
        command = [str(prepare.owned_path('tools/binaryen/bin/wasm-opt')), *OPT_FLAGS, str(ROOT / RAW), '-o', str(ROOT / WASM)]
        step = {'name': 'optimize_cli', 'command': normalized_command(command), 'working_directory': '<workspace>'}
        require((step['name'], step['command'], step['working_directory'], 300) == command_recipe()[2], 'optimizer command differs from fixed recipe')
        report['phase'] = 'optimize_cli'
        report['build_steps'].append(step)
        if args.record:
            write_report(report)
        run(command, ROOT, environment, 300, 16 * 1024 * 1024, step)
        report['php_wasm'] = wasm_identity(WASM)
        require(input_hashes() == report['input_files'] and verify_tools(manifest) == report['toolchain'], 'build inputs/tools changed')
        require(source_snapshot(environment, False) == report['sources'], 'source/dependency identity changed during build')
        require(patch_source.verify_applied(php, manifest) == report['source_patch']['after'], 'patched source changed during build')
        report['generated_configuration_files'], report['stub_headers_after'] = configuration_files(), stub_headers()
        report['tracked_changes_after_build'] = tracked_changes(environment)
        report['changed_stub_headers'] = sorted(name for name in set(report['stub_headers_before']) | set(report['stub_headers_after'])
                                                if report['stub_headers_before'].get(name) != report['stub_headers_after'].get(name))
        report['outcome'], report['phase'], passed = 'fiber_candidate_built_not_executed', 'build_complete', True
    except (Exception, KeyboardInterrupt) as failure:
        report['outcome'], report['diagnostics'] = 'fiber_build_failed', public(failure)[:8192]
    finally:
        if args.record and not args.validate_only:
            if 'input_files' not in report:
                try:
                    report['input_files'] = input_hashes()
                except (Exception, KeyboardInterrupt) as failure:
                    report['source_evidence_error'] = public(failure)[:8192]
            write_report(report)
    print('PHP WASIX Fiber CLI build: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
