"""Prepare only the four pinned Linux build tools, never execute guest PHP.

--validate-only performs manifest checks without download, extraction or tools.
Normal preparation requires a fresh, owned target directory on Linux x86-64.
"""
import argparse
import copy
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import subprocess
import tarfile
import time
import urllib.request

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
WORK = ROOT / 'target/php-wasix-cgi-build'
EVIDENCE = HERE / 'evidence'
MANIFEST = HERE / 'manifest.json'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    result = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            result.update(block)
    return result.hexdigest()


def relative_parts(name):
    require(type(name) is str and name and '\\' not in name, 'invalid relative path')
    path = PurePosixPath(name)
    require(not path.is_absolute() and all(part not in ('', '.', '..') and ':' not in part
                                         for part in name.split('/')), 'unsafe relative path: ' + name)
    return path.parts


def owned_path(name):
    path = WORK.joinpath(*relative_parts(name))
    require(path.resolve().is_relative_to(WORK.resolve()), 'path escaped owned build area')
    require(WORK.resolve().is_relative_to(ROOT.resolve() / 'target'), 'build root escaped workspace target')
    return path


def load_manifest():
    manifest = json.loads(MANIFEST.read_text(encoding='utf-8'))
    require(manifest['format_version'] == 1 and manifest['php_version'] == '8.5.12', 'wrong build manifest')
    require(manifest['guest_executed'] is False and manifest['product_runtime_selected'] is False
            and manifest['redistribute_binaries'] is False, 'build-only scope changed')
    require(set(manifest['sources']) == {'php', 'deps', 'parser'}, 'unexpected source set')
    for source in manifest['sources'].values():
        require(re.fullmatch('[0-9a-f]{40}', source['commit']), 'unfixed source commit')
        owned_path(source['directory'])
    assets = manifest['assets']
    require(len(assets) == 4 and {asset['name'] for asset in assets} == {'wasixcc', 'sysroot', 'llvm', 'binaryen'},
            'exactly four fixed tool archives are required')
    for asset in assets:
        prefix = f"https://github.com/{asset['repository']}/releases/download/{asset['tag']}/"
        require(asset['url'].startswith(prefix) and '?' not in asset['url'] and '#' not in asset['url'],
                'unexpected tool download endpoint')
        require(type(asset['asset_id']) is int and asset['asset_id'] > 0, 'invalid asset identity')
        require(type(asset['bytes']) is int and 0 < asset['bytes'] <= 512 * 1024 * 1024, 'invalid archive budget')
        require(re.fullmatch('[0-9a-f]{64}', asset['sha256']), 'missing fixed archive SHA-256')
        require(type(asset['strip_components']) is int and asset['strip_components'] >= 0, 'invalid strip count')
        prefix_parts = relative_parts(asset['archive_prefix']) if asset['archive_prefix'] else ()
        require(len(prefix_parts) == asset['strip_components'], 'unfixed archive root layout')
        require(type(asset['unpacked_bytes_limit']) is int and asset['unpacked_bytes_limit'] > 0, 'invalid extraction budget')
        owned_path(asset['directory'])
        require(asset['required_files'], 'missing required archive files')
        for name in asset['required_files']:
            relative_parts(name)
    build = manifest['build']
    require(build['configure_script'] == 'wasix-configure-eh-64.sh'
            and build['extra_configure_flags'] == '--enable-cgi'
            and build['make_target'] == 'cgi' and build['make_jobs'] == 2,
            'fixed CGI build procedure changed')
    return manifest


class HttpsOnlyRedirects(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, file_pointer, code, message, headers, new_url):
        require(new_url.startswith('https://'), 'tool download redirected away from HTTPS')
        return super().redirect_request(request, file_pointer, code, message, headers, new_url)


def verify_archive(path, asset):
    require(path.stat().st_size == asset['bytes'], asset['name'] + ' archive byte length mismatch')
    require(digest(path) == asset['sha256'], asset['name'] + ' archive SHA-256 mismatch')


def download(asset):
    archive = owned_path('downloads/' + asset['name'] + '.tar.gz')
    archive.parent.mkdir(parents=True, exist_ok=True)
    if archive.exists():
        verify_archive(archive, asset)
        return archive
    partial = archive.with_suffix(archive.suffix + '.partial')
    deadline = time.monotonic() + 600
    opener = urllib.request.build_opener(HttpsOnlyRedirects())
    request = urllib.request.Request(asset['url'], headers={'User-Agent': 'recastpress-closed-cgi-build'})
    with opener.open(request, timeout=45) as source, partial.open('xb') as output:
        declared = source.headers.get('Content-Length')
        require(declared is None or int(declared) == asset['bytes'], 'unexpected archive Content-Length')
        total = 0
        while True:
            require(time.monotonic() < deadline, 'tool download exceeded its deadline')
            block = source.read(1024 * 1024)
            if not block:
                break
            total += len(block)
            require(total <= asset['bytes'], 'tool download exceeded pinned byte length')
            output.write(block)
    verify_archive(partial, asset)
    partial.rename(archive)
    return archive


def transformed_members(source, asset):
    """Strip only the documented root, then let tarfile's data filter confine links."""
    prefix = tuple(asset['archive_prefix'].split('/')) if asset['archive_prefix'] else ()
    count, unpacked = 0, 0
    for member in source:
        count += 1
        require(count <= 100000, 'archive contains too many entries')
        require(not member.name.startswith('/') and '\\' not in member.name, 'unsafe archive entry')
        parts = PurePosixPath(member.name).parts
        require(all(part not in ('..', '') and ':' not in part for part in parts), 'archive path traversal')
        if len(parts) <= len(prefix):
            require(member.isdir() and parts == prefix[:len(parts)], 'archive root layout mismatch')
            continue
        require(parts[:len(prefix)] == prefix, 'archive prefix mismatch')
        require(member.isfile() or member.isdir() or member.issym() or member.islnk(), 'archive contains a special file')
        clone = copy.copy(member)
        clone.name = '/'.join(parts[len(prefix):])
        if clone.islnk():
            target = PurePosixPath(clone.linkname)
            require(not target.is_absolute() and '\\' not in clone.linkname
                    and '..' not in target.parts and target.parts[:len(prefix)] == prefix,
                    'unsafe archive hard link')
            clone.linkname = '/'.join(target.parts[len(prefix):])
            require(clone.linkname, 'archive hard link points at root')
        if clone.isfile():
            require(clone.size >= 0, 'negative archive file length')
            unpacked += clone.size
            require(unpacked <= asset['unpacked_bytes_limit'], 'archive exceeded extraction budget')
        yield clone


def extract_verified(archive, asset):
    verify_archive(archive, asset)
    directory = owned_path(asset['directory'])
    directory.parent.mkdir(parents=True, exist_ok=True)
    directory.mkdir(exist_ok=False)
    with tarfile.open(archive, 'r:gz') as source:
        source.extractall(directory, members=transformed_members(source, asset), filter='data')
    files = {}
    for name in asset['required_files']:
        path = directory.joinpath(*relative_parts(name))
        require(path.resolve().is_relative_to(directory.resolve()) and path.is_file(), 'required tool file missing or escaped archive')
        files[name] = {'bytes': path.stat().st_size, 'sha256': digest(path)}
    return files


def tool_environment():
    environment = {key: value for key, value in os.environ.items()
                   if not key.startswith('WASIXCC_') and key not in {'GH_TOKEN', 'GITHUB_TOKEN', 'PHPRC', 'PHP_INI_SCAN_DIR'}}
    environment.update({
        'WASIXCC_SYSROOT': str(owned_path('tools/sysroot-eh')),
        'WASIXCC_LLVM_LOCATION': str(owned_path('tools/llvm')),
        'WASIXCC_BINARYEN_LOCATION': str(owned_path('tools/binaryen')),
        'WASIXCC_WASM_EXCEPTIONS': 'legacy',
        'WASIXCC_INCLUDE_CPP_SYMBOLS': 'yes',
    })
    environment['PATH'] = os.pathsep.join([str(owned_path('tools/bin')),
                                          str(owned_path('tools/binaryen/bin')), environment.get('PATH', '')])
    return environment


def observe_tool(command, environment):
    result = subprocess.run(command, env=environment, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
    require(result.returncode == 0, 'pinned host build tool exited with failure: ' + Path(command[0]).name)
    require(len(result.stdout) + len(result.stderr) <= 65536, 'host build tool output exceeded budget')
    return (result.stdout + result.stderr).decode('utf-8', errors='strict').strip()


def write_report(name, report):
    require(name in {'toolchain.json', 'build.json'}, 'unexpected evidence filename')
    require(EVIDENCE.resolve().is_relative_to(ROOT.resolve()), 'evidence path escaped workspace')
    EVIDENCE.mkdir(exist_ok=True)
    destination = EVIDENCE / name
    partial = destination.with_suffix('.json.partial')
    require(not destination.is_symlink() and not partial.is_symlink(), 'evidence file is a symlink')
    partial.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    partial.replace(destination)


def input_hashes():
    paths = sorted(HERE.glob('*.py')) + [MANIFEST, HERE / 'README.ja.md', ROOT / '.github/workflows/php-wasix-cgi-build.yml']
    return {path.relative_to(ROOT).as_posix(): digest(path) for path in paths if path.is_file()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validate-only', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    report = {'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'outcome': 'not_run', 'guest_executed': False, 'product_runtime_selected': False,
              'redistributed_binaries': False, 'assets': []}
    report['input_files'] = input_hashes()
    passed = False
    try:
        manifest = load_manifest()
        if args.validate_only:
            print('CGI build manifest validated; no download, extraction, build tool or guest executed.')
            return 0
        require(platform.system() == 'Linux' and platform.machine() in ('x86_64', 'AMD64'), 'tools pin Linux x86-64 only')
        for asset in manifest['assets']:
            row = {key: asset[key] for key in ('name', 'repository', 'tag', 'asset_id', 'url', 'bytes', 'sha256')}
            row['verified'] = False
            report['assets'].append(row)
            report['outcome'] = 'preparation_in_progress'
            if args.record:
                write_report('toolchain.json', report)
            archive = download(asset)
            verify_archive(archive, asset)
            row['observed_bytes'] = archive.stat().st_size
            row['observed_sha256'] = digest(archive)
            row['verified'] = True
            row['required_files'] = extract_verified(archive, asset)
            if args.record:
                write_report('toolchain.json', report)
        environment = tool_environment()
        binary = owned_path('tools/wasixcc/wasixccenv')
        require(os.access(binary, os.X_OK), 'verified wasixccenv is not executable')
        install = subprocess.run([str(binary), 'install-executables', str(owned_path('tools/bin'))],
                                 env=environment, timeout=30)
        require(install.returncode == 0, 'install-executables failed')
        report['tool_versions'] = {
            'wasixccenv': observe_tool([str(binary), '--version'], environment),
            'clang': observe_tool([str(owned_path('tools/llvm/bin/clang')), '--version'], environment),
            'wasm_opt': observe_tool([str(owned_path('tools/binaryen/bin/wasm-opt')), '--version'], environment),
        }
        sysroot = observe_tool([str(binary), 'print-sysroot'], environment)
        require(Path(sysroot).resolve() == owned_path('tools/sysroot-eh').resolve(), 'wasixcc selected an unexpected sysroot')
        report['selected_sysroot'] = 'tools/sysroot-eh'
        report['environment'] = {key: value.replace(str(WORK), '<owned-build>') for key, value in environment.items()
                                 if key.startswith('WASIXCC_')}
        report['outcome'] = 'verified_host_build_tools_prepared'
        passed = True
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = 'preparation_failed'
        report['diagnostics'] = str(error)[:8192]
    finally:
        report['input_files'] = input_hashes()
        if args.record and not args.validate_only:
            write_report('toolchain.json', report)
    print('PHP WASIX CGI tools: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
