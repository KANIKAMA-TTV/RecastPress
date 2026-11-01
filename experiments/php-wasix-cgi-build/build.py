"""Build fixed PHP CGI source with the declared WASI prefork exclusion patch.

The target Wasm is inspected for its header and hashed, never executed here.
Host native PHP is a recorded stub-generation tool, not a runtime fallback.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import subprocess
import time

import prepare
import patch_source

HERE, ROOT, WORK = prepare.HERE, prepare.ROOT, prepare.WORK
require, digest = prepare.require, prepare.digest
LINK_CAPTURES = {
    'map': ('link-observations/cgi.link.map', 64 * 1024 * 1024),
    'why_extract': ('link-observations/cgi.why-extract.tsv', 16 * 1024 * 1024),
}


def exact(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(exact(actual[key], value) for key, value in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(exact(left, right) for left, right in zip(actual, expected))
    return actual == expected


def capture_path(name):
    path = WORK.joinpath(*prepare.relative_parts(LINK_CAPTURES[name][0]))
    require(WORK.resolve() == WORK and path.resolve() == path and path.is_relative_to(WORK),
            'link capture resolves outside the fixed owned directory')
    return path


def link_flags():
    return '-Map=' + str(capture_path('map')) + ':--why-extract=' + str(capture_path('why_extract'))


def public_link_flags():
    return '-Map=' + capture_path('map').relative_to(ROOT).as_posix() + ':--why-extract=' + capture_path('why_extract').relative_to(ROOT).as_posix()


def observe_link_capture(name):
    """Hash a bounded regular file without parsing or making audit judgments."""
    path = capture_path(name)
    limit = LINK_CAPTURES[name][1]
    metadata = path.stat()
    require(not path.is_symlink() and stat.S_ISREG(metadata.st_mode) and 0 < metadata.st_size <= limit,
            'link capture type/byte budget: ' + name)
    value, total = hashlib.sha256(), 0
    with path.open('rb') as source:
        require(stat.S_ISREG(os.fstat(source.fileno()).st_mode), 'link capture is not a regular file')
        while True:
            block = source.read(min(1024 * 1024, limit + 1 - total))
            if not block:
                break
            total += len(block)
            require(total <= limit, 'link capture grew past hard limit')
            value.update(block)
    require(total == metadata.st_size and path.stat().st_size == total, 'link capture changed while hashing')
    return {'path': path.relative_to(ROOT).as_posix(), 'bytes': total, 'sha256': value.hexdigest(),
            'file_type': 'regular', 'read_completed': True, 'byte_limit': limit}


def verify_link_capture(receipt, manifest):
    """Read-only capture binding, complementary to the existing build checker.

    No collector or syntax/semantic audit is run here. Caller must verify the
    normal source/tool/configuration/module chain independently as before.
    """
    expected = {'scope': 'make_cgi_only', 'normalized_flags': public_link_flags(),
        'actual_value_sha256': hashlib.sha256(link_flags().encode('utf-8')).hexdigest(),
        'configure_flags_absent': True, 'optimize_flags_absent': True, 'parsed_by_builder': False}
    observed = receipt['link_capture']
    require(set(observed) == set(expected) | {'map', 'why_extract'}, 'unexpected link capture fields')
    require(all(type(observed[key]) is type(value) and observed[key] == value for key, value in expected.items()),
            'link capture stage/flags/scope binding changed')
    for name in LINK_CAPTURES:
        require(exact(observed[name], observe_link_capture(name)), 'captured raw linker text changed: ' + name)
    steps = receipt['build_steps']
    require([row['name'] for row in steps] == ['configure', 'make_cgi', 'optimize_cgi'], 'link capture build stage ordering')
    for row in steps:
        present = row['name'] == 'make_cgi'
        flags = {'present': present, 'actual_value_sha256': expected['actual_value_sha256'] if present else None}
        require(exact(row['linker_flags_environment'], flags), 'link flags escaped the final CGI make stage')
    require(manifest['build']['make_target'] == 'cgi', 'link capture is limited to the fixed CGI target')
    return {'map': observed['map'], 'why_extract': observed['why_extract'],
            'stage': 'make_cgi', 'normalized_flags': expected['normalized_flags'],
            'actual_value_sha256': expected['actual_value_sha256'], 'parsed_by_builder': False}


def checked_output(command, cwd, environment):
    result = subprocess.run(command, cwd=cwd, env=environment, stdout=subprocess.PIPE,
                            stderr=subprocess.PIPE, timeout=30)
    require(result.returncode == 0 and len(result.stdout) + len(result.stderr) <= 1024 * 1024,
            'source/tool observation failed: ' + command[0])
    return result.stdout.decode('utf-8', errors='strict').strip()


def verify_sources(manifest, environment):
    sources = {}
    for name, identity in manifest['sources'].items():
        directory = prepare.owned_path(identity['directory'])
        observed = checked_output(['git', 'rev-parse', 'HEAD'], directory, environment)
        require(observed == identity['commit'], 'source checkout commit mismatch: ' + name)
        tracked = checked_output(['git', 'status', '--porcelain', '--untracked-files=no'], directory, environment)
        require(not tracked, 'source checkout already has tracked changes: ' + name)
        sources[name] = {'repository': identity['repository'], 'commit': observed,
                         'tracked_worktree_clean_before_build': True}
    php = prepare.owned_path(manifest['sources']['php']['directory'])
    parser = prepare.owned_path(manifest['sources']['parser']['directory'])
    require(parser == php / 'build/PHP-Parser-5.6.1' and (parser / 'lib/PhpParser/ParserFactory.php').is_file(),
            'the pinned parser is not preseeded at gen_stub.php\'s expected path')
    generator = php / 'build/gen_stub.php'
    source = generator.read_text(encoding='utf-8')
    require('$version = "5.6.1";' in source and '$phpParserDir = __DIR__ . "/PHP-Parser-$version";' in source,
            'unexpected stub generator parser lookup')
    deps = prepare.owned_path(manifest['sources']['deps']['directory'])
    archives = {}
    for path in sorted((deps / 'lib-eh').glob('*.a')) + sorted((deps / 'pgsql-eh/lib').glob('*.a')):
        require(path.resolve().is_relative_to(deps.resolve()), 'dependency archive escaped fixed checkout')
        with path.open('rb') as archive:
            require(archive.read(8) == b'!<arch>\n', 'dependency is not an archive (possible unmaterialized LFS pointer)')
        archives[path.relative_to(deps).as_posix()] = {'bytes': path.stat().st_size, 'sha256': digest(path)}
    require('pgsql-eh/lib/libpq.a' in archives and 'lib-eh/libcurl.a' in archives,
            'pinned prebuilt dependency archives are missing')
    sources['deps']['prebuilt_archive_files'] = archives
    sources['deps']['rebuilt_here'] = False
    return sources


def verify_tools(manifest):
    receipt_path = HERE / 'evidence/toolchain.json'
    receipt = json.loads(receipt_path.read_text(encoding='utf-8'))
    require(receipt['outcome'] == 'verified_host_build_tools_prepared'
            and receipt['guest_executed'] is False, 'toolchain was not successfully verified')
    for name, expected in receipt['input_files'].items():
        path = ROOT.joinpath(*prepare.relative_parts(name))
        require(path.resolve().is_relative_to(ROOT.resolve()) and digest(path) == expected,
                'tool preparation source changed: ' + name)
    rows = {row['name']: row for row in receipt['assets']}
    require(set(rows) == {asset['name'] for asset in manifest['assets']}, 'unexpected prepared tool set')
    for asset in manifest['assets']:
        row = rows[asset['name']]
        require(row['verified'] is True and row['sha256'] == asset['sha256']
                and row['bytes'] == asset['bytes'], 'prepared archive identity mismatch')
        for name, expected in row['required_files'].items():
            path = prepare.owned_path(asset['directory']).joinpath(*prepare.relative_parts(name))
            require(path.resolve().is_relative_to(prepare.owned_path(asset['directory']).resolve())
                    and path.is_file() and path.stat().st_size == expected['bytes']
                    and digest(path) == expected['sha256'], 'prepared tool file changed')
    binary = prepare.owned_path('tools/wasixcc/wasixccenv')
    # v0.4.4 install_executables symlinks canonical current_exe directly;
    # COMMANDS includes cc, ++, cc++, ar, nm, ranlib, ld (each prefixed wasix).
    for name in ['wasixcc', 'wasixcc++', 'wasixar', 'wasixranlib', 'wasixnm']:
        wrapper = prepare.owned_path('tools/bin/' + name)
        require(wrapper.is_file() and os.path.samefile(wrapper, binary), 'compiler wrapper is not the verified wasixccenv')
    return digest(receipt_path)


def apply_source_patches(php, manifest, report, environment, record=False):
    report['phase'] = 'source_patch'
    head = checked_output(['git', 'rev-parse', 'HEAD'], php, environment)
    require(head == manifest['sources']['php']['commit'], 'source commit changed before patch application')
    dirty = checked_output(['git', 'status', '--porcelain', '--untracked-files=no'], php, environment)
    require(not dirty, 'tracked PHP source is not clean immediately before patch application')
    report['source_patches'] = []
    for identity in manifest['source_patches']:
        row = {name: identity[name] for name in ('path', 'sha256', 'source_commit', 'target',
                                                'before_bytes', 'before_sha256', 'after_sha256')}
        row.update({'clean_tracked_source_before_patch': True, 'applied': False})
        report['source_patches'].append(row)
        if record:
            prepare.write_report('build.json', report)
        patch_source.apply_to_checkout(php, identity, row)
    report['configure_difference']['php_source_patch'] = True
    report['configure_difference']['source_patch_scope'] = 'WASI FastCGI prefork exclusion only'
    report['wasi_prefork_support'] = 'explicitly-disabled-by-source-patch'


def stub_headers(php):
    result = {}
    for path in sorted(php.rglob('*_arginfo.h')):
        if '.git' not in path.relative_to(php).parts:
            require(path.resolve().is_relative_to(php.resolve()), 'stub header escaped source')
            result[path.relative_to(php).as_posix()] = digest(path)
    return result


def configuration_files(php):
    names = ['config.log', 'config.status', 'config.nice', 'Makefile', 'main/php_config.h', 'Zend/zend_config.h']
    return {name: {'bytes': (php / name).stat().st_size, 'sha256': digest(php / name)}
            for name in names if (php / name).is_file()}


def run_step(report, name, command, cwd, environment, record=False):
    report['phase'] = name
    report['outcome'] = 'build_in_progress'
    started = time.monotonic()
    step = {'name': name, 'command': command, 'completed': False}
    supplied = environment.get('WASIXCC_LINKER_FLAGS')
    expected = link_flags() if name == 'make_cgi' else None
    require(supplied == expected, 'linker diagnostic flags must be absent except during make_cgi')
    step['linker_flags_environment'] = {'present': supplied is not None,
        'actual_value_sha256': hashlib.sha256(supplied.encode('utf-8')).hexdigest() if supplied is not None else None}
    report['build_steps'].append(step)
    if record:
        report['generated_configuration_files'] = configuration_files(cwd)
        prepare.write_report('build.json', report)
    # CI's 45-minute job timeout owns the complete process tree. Do not claim a
    # local subprocess timeout would also stop make's compiler grandchildren.
    result = subprocess.run(command, cwd=cwd, env=environment)
    step.update({'exit_code': result.returncode, 'elapsed_ms': round((time.monotonic() - started) * 1000),
                 'completed': True})
    require(result.returncode == 0, 'fixed build step failed: ' + name)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true')
    parser.add_argument('--validate-only', action='store_true')
    args = parser.parse_args()
    report = {
        'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
        'scope': 'official CGI source with explicit WASI prefork exclusion; guest not executed',
        'outcome': 'not_run', 'phase': 'verification',
        'guest_executed': False, 'product_runtime_selected': False, 'native_runtime_fallback_used': False,
        'cgi_request_compatibility_verified': False, 'fiber_compatibility_verified': False,
        'module_validation_performed': False, 'redistributed_binaries': False, 'build_steps': [],
    }
    report['input_files'] = prepare.input_hashes()
    php = None
    passed = False
    before = {}
    try:
        manifest = prepare.load_manifest()
        if args.validate_only:
            print('CGI build procedure validated; no source checkout, download, native build or guest execution.')
            return 0
        require(platform.system() == 'Linux' and platform.machine() in ('x86_64', 'AMD64'), 'build pins Linux x86-64 only')
        environment = prepare.tool_environment()
        require('WASIXCC_LINKER_FLAGS' not in environment, 'configure must not inherit link-map flags')
        for name in LINK_CAPTURES:
            path = capture_path(name)
            require(not path.exists() and not path.is_symlink(), 'stale or dangling link capture refused')
        capture_path('map').parent.mkdir(parents=True, exist_ok=True)
        report['toolchain_receipt_sha256'] = verify_tools(manifest)
        report['sources'] = verify_sources(manifest, environment)
        php = prepare.owned_path(manifest['sources']['php']['directory'])
        deps = prepare.owned_path(manifest['sources']['deps']['directory'])
        before = stub_headers(php)
        report['stub_headers_before'] = before
        host_php = Path('/usr/bin/php').resolve(strict=True)
        query = ('echo json_encode(["version"=>PHP_VERSION,"version_id"=>PHP_VERSION_ID,'
                 '"sapi"=>PHP_SAPI,"tokenizer"=>extension_loaded("tokenizer"),'
                 '"binary"=>PHP_BINARY], JSON_THROW_ON_ERROR);')
        native = json.loads(checked_output([str(host_php), '-r', query], php, environment))
        require(type(native['version_id']) is int and native['version_id'] >= 70400
                and native['sapi'] == 'cli' and native['tokenizer'] is True,
                'host stub generator needs native PHP >=7.4 with tokenizer')
        native.update({'role': 'build-time gen_stub.php tool only; never a product runtime fallback',
                       'executable_sha256': digest(host_php)})
        report['host_stub_generator_php'] = native
        report['apt_packages'] = checked_output(
            ['dpkg-query', '-W', '-f=${Package}=${Version}\n', 'autoconf', 'automake', 'bison',
             'build-essential', 're2c', 'pkg-config', 'php-cli', 'php-common'], php, environment).splitlines()
        environment.update({'PHP_WASIX_DEPS': str(deps),
                            'WASIX_EXTRA_CONFIGURE_FLAGS': manifest['build']['extra_configure_flags']})
        scripts = ['wasix-configure-eh-64.sh', 'wasix-configure-eh.sh', 'wasix-build-eh.sh',
                   'sapi/cgi/config9.m4', 'sapi/cgi/Makefile.frag', 'sapi/cgi/cgi_main.c',
                   'build/gen_stub.php', 'main/php_version.h']
        report['source_control_files'] = {name: digest(php / name) for name in scripts}
        require('#define PHP_VERSION "8.5.12"' in (php / 'main/php_version.h').read_text(encoding='utf-8'),
                'unexpected declared PHP source version')
        report['configure_difference'] = {'extra_flags': '--enable-cgi', 'php_source_patch': False,
                                           'cli_disabled': False, 'make_target': 'cgi', 'make_jobs': 2}
        apply_source_patches(php, manifest, report, environment, args.record)
        run_step(report, 'configure', ['bash', manifest['build']['configure_script']], php, environment, args.record)
        # Keep autoconf probe links and the existing optimizer environment
        # unchanged. Only the final CGI make sees raw wasm-ld diagnostics flags.
        make_environment = environment.copy()
        make_environment['WASIXCC_LINKER_FLAGS'] = link_flags()
        report['link_capture'] = {'scope': 'make_cgi_only', 'normalized_flags': public_link_flags(),
            'actual_value_sha256': hashlib.sha256(link_flags().encode('utf-8')).hexdigest(),
            'configure_flags_absent': True, 'optimize_flags_absent': True, 'parsed_by_builder': False}
        run_step(report, 'make_cgi', ['make', '-j2', 'PHP=' + str(host_php), 'cgi'], php, make_environment, args.record)
        report['phase'] = 'link_capture'
        for name in LINK_CAPTURES:
            report['link_capture'][name] = observe_link_capture(name)
        if args.record:
            # Preserve actual captures even if the later optimizer/audit fails.
            prepare.write_report('build.json', report)
        raw = php / manifest['build']['raw_output']
        with raw.open('rb') as source:
            require(source.read(8) == b'\0asm\x01\0\0\0', 'CGI link did not produce a WebAssembly v1 module')
        output = php / manifest['build']['wasm_output']
        command = [str(prepare.owned_path('tools/binaryen/bin/wasm-opt'))]
        command += manifest['build']['optimizer_flags'] + [str(raw), '-o', str(output)]
        run_step(report, 'optimize_cgi', command, php, environment, args.record)
        with output.open('rb') as source:
            require(source.read(8) == b'\0asm\x01\0\0\0', 'optimized output is not a WebAssembly v1 module')
        report['cgi_wasm'] = {'path': manifest['build']['wasm_output'], 'bytes': output.stat().st_size,
                              'sha256': digest(output), 'wasm_header_verified': True, 'executed': False}
        report['raw_cgi_wasm'] = {'bytes': raw.stat().st_size, 'sha256': digest(raw)}
        report['outcome'] = 'cgi_candidate_built_not_executed'
        report['phase'] = 'build_complete'
        passed = True
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = 'link_capture_failed' if report['phase'] == 'link_capture' else 'build_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        if php is not None:
            try:
                report['generated_configuration_files'] = configuration_files(php)
                after = stub_headers(php)
                report['stub_headers_after'] = after
                report['changed_stub_headers'] = sorted(name for name in set(before) | set(after)
                                                         if before.get(name) != after.get(name))
            except (Exception, KeyboardInterrupt) as error:
                passed = False
                report['outcome'] = 'evidence_collection_failed'
                report['evidence_diagnostics'] = str(error)[:8192]
        report['input_files'] = prepare.input_hashes()
        if args.record and not args.validate_only:
            prepare.write_report('build.json', report)
    print('PHP WASIX CGI build: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
