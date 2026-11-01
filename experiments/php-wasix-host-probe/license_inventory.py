"""Record declared Cargo licenses for one fixed offline Linux-target graph.

This is a manifest inventory, not a legal, proc-macro, compile-unit or linker
analysis. PHP's static archives and the native LLVM toolchain have separate
provenance gates and are deliberately outside this report.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import tarfile
import tomllib

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
import owned_probe

INPUTS = [HERE / 'Cargo.toml', HERE / 'Cargo.lock', Path(__file__).resolve(), ROOT / 'scripts/owned_probe.py']
TARGET = 'x86_64-unknown-linux-gnu'
LICENSE_FILES = {
    ('wasmer-compiler-llvm', '7.2.0'): (
        '0f1dadc1e6d195aa32396840230dd99cfce7920d8a9393434f9ffd25bee5e7b6',
        'wasmer_llvm_special_business_source_license_file'),
    ('saffron', '0.1.0'): (
        '8cb4ca05e2f9c0e1765e06b8fdb199cbd19d8afde523fa9f2985683c52a84373',
        'saffron_license_file_without_spdx_declaration'),
}


class InventoryError(Exception):
    """Only fixed, path-free diagnostic codes may leave the helper."""


def require(condition, code):
    if not condition:
        raise InventoryError(code)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def input_hashes():
    return {path.relative_to(ROOT).as_posix(): sha(path.read_bytes()) for path in INPUTS}


def identity(package):
    # Cargo package IDs and package/manifests' absolute paths stay internal.
    return {key: package[key] for key in ['name', 'version', 'source']}


def license_file_name(package):
    name = package.get('license_file')
    if name is None:
        return None
    path = Path(name)
    base = Path(package['manifest_path']).parent
    if path.is_absolute():
        require(path.is_relative_to(base), 'license_file_outside_package')
        path = path.relative_to(base)
    require(not path.is_absolute() and '..' not in path.parts and ':' not in path.as_posix(),
            'nonrelative_license_file')
    return path.as_posix()


def license_file_check(package, locked):
    """Read cached bytes only; never extract, fetch or print a license body."""
    key = package['name'], package['version']
    expected_sha, flag = LICENSE_FILES[key]
    name = license_file_name(package)
    require(name == 'LICENSE', 'unexpected_known_license_filename')
    base = Path(package['manifest_path']).parent
    local_path = base / name
    require(local_path.is_file() and local_path.stat().st_size <= 65536, 'license_file_budget')
    local = local_path.read_bytes()
    require(0 < len(local) <= 65536 and sha(local) == expected_sha, 'reviewed_license_file_changed')
    source = package['source']
    require(isinstance(source, str) and source.startswith('registry+'), 'known_license_not_registry_source')
    row = locked.get((key[0], key[1], source))
    require(row is not None and re.fullmatch('[0-9a-f]{64}', row.get('checksum', '')),
            'known_license_missing_lock_checksum')
    # Cargo's registry/src/<registry>/<name-version> and matching cache layout.
    archive = base.parents[2] / 'cache' / base.parent.name / (key[0] + '-' + key[1] + '.crate')
    require(archive.is_file() and 0 < archive.stat().st_size <= 16 * 1024 * 1024,
            'cached_crate_missing_or_over_budget')
    archive_sha = sha(archive.read_bytes())
    require(archive_sha == row['checksum'], 'cached_crate_lock_checksum_mismatch')
    with tarfile.open(archive, 'r:gz') as source_archive:
        entry = source_archive.getmember(key[0] + '-' + key[1] + '/' + name)
        require(entry.isfile() and entry.size == len(local), 'cached_license_member_type_or_size')
        reader = source_archive.extractfile(entry)
        require(reader is not None, 'cached_license_member_missing')
        with reader:
            require(reader.read(65537) == local, 'cached_license_differs_from_local_file')
    return {**identity(package), 'license_file': name, 'bytes': len(local), 'sha256': sha(local),
            'review_flag': flag, 'cached_crate_sha256': archive_sha,
            'cached_crate_matches_lock_checksum': True, 'local_file_matches_cached_crate': True}


def inventory(metadata, locked, backend):
    packages = {package['id']: package for package in metadata['packages']}
    nodes = {node['id']: node for node in metadata['resolve']['nodes']}
    root = metadata['resolve']['root']
    require(root in packages and root in nodes, 'metadata_root_missing')
    require(packages[root]['name'] == 'recastpress-php-wasix-host-probe'
            and packages[root]['version'] == '0.5.0', 'unexpected_workspace_root')

    def dependencies(node, kinds):
        return {edge['pkg'] for edge in nodes[node]['deps']
                if any(kind['kind'] in kinds for kind in edge['dep_kinds'])}

    def closure(seeds, kinds):
        seen, pending = set(), list(seeds)
        while pending:
            node = pending.pop()
            if node in seen:
                continue
            require(node in nodes and node in packages, 'metadata_dependency_node_missing')
            seen.add(node)
            pending.extend(dependencies(node, kinds))
        return seen

    runtime = closure([root], {None})
    dev = closure(dependencies(root, {'dev'}), {None})
    build_seeds = set()
    for node in runtime | dev:
        build_seeds.update(dependencies(node, {'build'}))
    build = closure(build_seeds, {None, 'build'})
    graphs = {'runtime': runtime, 'build': build, 'dev': dev}
    reachable = runtime | build | dev
    rows, flags, file_checks = [], [], []
    for node in sorted(reachable, key=lambda item: (packages[item]['name'], packages[item]['version'],
                                                   packages[item]['source'] or '')):
        package = packages[node]
        license_name = package.get('license')
        license_file = license_file_name(package)
        labels = [kind for kind, graph in graphs.items() if node in graph]
        rows.append({**identity(package), 'license': license_name, 'license_file': license_file,
                     'reachable_from': labels})
        codes = []
        if license_name is None:
            codes.append('declared_license_string_missing')
        if license_file is not None:
            codes.append('separate_license_file')
        if license_name:
            for token, code in [(' AND ', 'declared_and_expression'), (' WITH ', 'declared_exception_expression'),
                                ('/', 'legacy_slash_expression')]:
                if token in license_name:
                    codes.append(code)
            if '(' in license_name or license_name.count(' OR ') > 1:
                codes.append('compound_or_multiple_choice_expression')
        if (package['name'], package['version']) in LICENSE_FILES:
            checked = license_file_check(package, locked)
            file_checks.append(checked)
            codes.append(checked['review_flag'])
        if codes:
            flags.append({**identity(package), 'codes': codes})

    compiler_reachability = {}
    for name in ['wasmer-compiler-llvm', 'wasmer-compiler-cranelift']:
        compiler_reachability[name] = {
            kind: any(packages[node]['name'] == name for node in graph) for kind, graph in graphs.items()}
    llvm_reachable = any(compiler_reachability['wasmer-compiler-llvm'].values())
    cranelift_reachable = any(compiler_reachability['wasmer-compiler-cranelift'].values())
    require(llvm_reachable == (backend == 'llvm') and cranelift_reachable == (backend == 'cranelift'),
            'selected_compiler_graph_mismatch')
    root_edges = {}
    for label, kind in [('runtime', None), ('build', 'build'), ('dev', 'dev')]:
        root_edges[label] = [identity(packages[node]) for node in sorted(dependencies(root, {kind}),
                                                                      key=lambda node: (packages[node]['name'],
                                                                                        packages[node]['version']))]
    return {'root_package': identity(packages[root]), 'root_dependency_edges': root_edges,
            'graph_package_counts': {kind: len(graph) for kind, graph in graphs.items()},
            'reachable_package_count': len(reachable), 'locked_package_count': len(locked),
            'packages': rows, 'flags': flags, 'license_file_checks': file_checks,
            'compiler_reachability': compiler_reachability,
            'cranelift_graph_excludes_llvm_compiler': not llvm_reachable if backend == 'cranelift' else None,
            'missing_license_and_license_file_count': sum(row['license'] is None and row['license_file'] is None
                                                        for row in rows)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--llvm', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    backend = 'llvm' if args.llvm else 'cranelift'
    command = ['cargo', 'metadata', '--manifest-path', 'experiments/php-wasix-host-probe/Cargo.toml',
               '--format-version', '1', '--offline', '--locked', '--filter-platform', TARGET,
               '--no-default-features', '--features', backend]
    report = {'format_version': 1, 'observed_at_utc': datetime.now(timezone.utc).isoformat(),
              'schema': 'recastpress.php-wasix-dependency-inventory.v0.5',
              'outcome': 'not_run', 'inventory_verified': False, 'backend': backend,
              'host_platform': {'system': platform.system(), 'machine': platform.machine()},
              'metadata_filter_platform': TARGET, 'metadata_command': command,
              'outer_metadata_timeout_seconds': 60, 'owned_metadata_reaped': False,
              'owned_cleanup_confirmed': False, 'legal_compatibility_evaluated': False,
              'php_static_archive_inventory_included': False,
              'php_static_archive_license_provenance_verified': False,
              'native_llvm_toolchain_inventory_included': False,
              'native_llvm_toolchain_license_provenance_verified': False,
              'graph_policy': {
                  'runtime': 'workspace root plus normal dependency edges only',
                  'dev': 'root dev dependency edges, then normal dependency edges only',
                  'build': 'build edges from runtime/dev closures, then normal/build dependency edges',
                  'overlap': 'a package may appear in multiple closures; counts must not be added',
              },
              'limitations': [
                  'Declared dep_kind reachability only; proc-macro host units are not reclassified.',
                  'Linux target filtering in metadata is not an actual Linux compile or linker observation.',
                  'Metadata can unify features across dev and normal dependencies; this is the current manifest/lock snapshot.',
                  'Licenses are publisher declarations; no acceptance, compatibility or redistribution verdict is inferred.',
                  'PHP static archives and the native LLVM toolchain need independent provenance and license gates.',
                  'Raw metadata, diagnostics, Cargo package IDs and absolute host paths are never published.',
              ]}
    passed = False
    try:
        os.chdir(ROOT)
        before = input_hashes()
        report['input_files'] = before
        lock = tomllib.loads((HERE / 'Cargo.lock').read_text(encoding='utf-8'))
        locked = {(row['name'], row['version'], row.get('source')): row for row in lock['package']}
        pid, result = owned_probe.run_captured_probe(command, os.environ.copy(), timeout=60)
        report.update(owned_process_pid=pid, owned_metadata_reaped=True, metadata_exit_code=result.returncode,
                      metadata_stdout_bytes=len(result.stdout), metadata_stdout_sha256=sha(result.stdout),
                      metadata_stderr_bytes=len(result.stderr), metadata_stderr_sha256=sha(result.stderr))
        require(result.returncode == 0, 'offline_locked_metadata_failed')
        require(0 < len(result.stdout) <= 32 * 1024 * 1024 and len(result.stderr) <= 65536,
                'metadata_capture_budget')
        data = json.loads(result.stdout)
        report.update(inventory(data, locked, backend))
        require(input_hashes() == before, 'inventory_inputs_changed_during_read')
        report['input_files_unchanged'] = True
        report['outcome'] = 'declared_dependency_inventory_recorded'
        report['inventory_verified'], passed = True, True
    except subprocess.TimeoutExpired:
        report['outcome'] = 'metadata_timeout'
        report['diagnostic_code'] = 'owned_metadata_exceeded_60_seconds'
    except InventoryError as error:
        report['outcome'] = 'inventory_failed'
        report['diagnostic_code'] = str(error)
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = 'inventory_failed'
        report['diagnostic_code'] = 'unpublished_' + type(error).__name__
    finally:
        if report['outcome'] != 'declared_dependency_inventory_recorded':
            report['inventory_verified'], passed = False, False
        try:
            owned_probe.reap_owned_probes()
            report['owned_cleanup_confirmed'] = True
        except (Exception, KeyboardInterrupt):
            report['outcome'] = 'cleanup_failed'
            report['diagnostic_code'] = 'owned_metadata_cleanup_unconfirmed'
            report['inventory_verified'], passed = False, False
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        filename = platform.system().lower() + '-' + backend + '-dependency-inventory.json'
        (directory / filename).write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n',
                                           encoding='utf-8', newline='\n')
    print('PHP WASIX declared dependency inventory (' + backend + '): ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
