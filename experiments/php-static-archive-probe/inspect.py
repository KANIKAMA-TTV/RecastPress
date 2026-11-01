"""Read three pinned ar files without extraction, execution or network access.

Member and registry-path strings are identity clues, not a complete dependency
graph, source attestation, final-Wasm retention proof or license approval.
"""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
INVENTORY = ROOT / 'experiments/php-wasix-cgi-build/evidence/dependency-source-inventory.json'
LOCAL = ROOT / '.reference/wasix-deps-b77-audit'
NAMES = ['libicrt.a', 'libwasix_rust_libs.a', 'libwasix_sendmail.a']
REGISTRY = re.compile(rb'registry[/\\]src[/\\][^/\\\x00]{1,100}[/\\]([A-Za-z0-9_-]+-[0-9]+\.[0-9]+\.[0-9]+(?:[-+][A-Za-z0-9_.-]+)?)[/\\]')


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def fixed(path, maximum):
    require(path.resolve(strict=True) == path and path.is_file(), 'linked/missing input')
    require(0 < path.stat().st_size <= maximum, 'input size budget')
    wire = path.read_bytes()
    require(0 < len(wire) <= maximum, 'input grew past budget')
    return wire


def members(wire):
    require(wire.startswith(b'!<arch>\n'), 'fixed regular ar header required')
    position, table, rows, table_seen, count = 8, b'', [], False, 0
    while position < len(wire):
        require(count < 4096, 'ar member count budget')
        count += 1
        header = wire[position:position + 60]
        require(len(header) == 60 and header[-2:] == b'`\n', 'ar member header')
        raw_size = header[48:58].strip()
        require(re.fullmatch(rb'[0-9]+', raw_size), 'ar size syntax')
        size = int(raw_size)
        payload = wire[position + 60:position + 60 + size]
        require(len(payload) == size, 'truncated ar member')
        position += 60 + size
        if size & 1:
            require(wire[position:position + 1] == b'\n', 'ar alignment byte')
            position += 1
        name = header[:16].decode('ascii').strip()
        if name == '//':
            require(not table_seen, 'duplicate GNU long-name table')
            table_seen = True
            table = payload
            continue
        if name in ['/', '/SYM64/']:
            continue
        if name.startswith('/') and name[1:].isdigit():
            offset = int(name[1:])
            require(offset < len(table), 'ar long-name offset')
            require(offset == 0 or table[offset - 1:offset] == b'\n', 'ar long-name entry start')
            end = table.find(b'/\n', offset)
            require(end >= offset, 'ar long-name terminator')
            name = table[offset:end].decode('utf-8')
        elif name.startswith('#1/'):
            require(name[3:].isdigit(), 'BSD name length syntax')
            length = int(name[3:])
            require(0 < length <= len(payload), 'BSD name length')
            name = payload[:length].rstrip(b'\0').decode('utf-8')
            payload = payload[length:]
        else:
            name = name.rstrip('/')
        require(0 < len(name.encode('utf-8')) <= 4096 and '\0' not in name, 'member name budget')
        # Names are metadata only. Nothing is extracted to an archive path.
        base = name.replace('\\', '/').rsplit('/', 1)[-1]
        require(base and not any(ord(c) < 32 or ord(c) == 127 for c in base), 'member name controls')
        kind = ('wasm-magic' if payload.startswith(b'\0asm') else
                'llvm-bitcode' if payload.startswith(b'BC\xc0\xde') else
                'wrapped-llvm-bitcode' if payload.startswith(b'\xde\xc0\x17\x0b') else 'other')
        rows.append({'name': base, 'format': kind, 'bytes': len(payload)})
    require(position == len(wire) and rows, 'ar completion')
    return rows


def inspect():
    source = fixed(INVENTORY, 128 * 1024)
    inventory = json.loads(source)
    require(inventory['commit'] == 'b77ecbede97aba2df32c86eaeb54ff9f5e44f543', 'fixed dependency commit')
    by_name = {Path(row['path']).name: row for row in inventory['selected_tree_entries']}
    observed = []
    for name in NAMES:
        pin = by_name[name]
        wire = fixed(LOCAL / name, 16 * 1024 * 1024)
        require(len(wire) == pin['bytes'] and sha(wire) == pin['sha256_recorded_by_both_ci_builders'], 'CI archive identity')
        blob = hashlib.sha1(b'blob ' + str(len(wire)).encode('ascii') + b'\0' + wire).hexdigest()
        require(blob == pin['git_blob_sha1'], 'Git blob identity')
        rows = members(wire)
        crates = sorted({match.group(1) for row in rows
            if (match := re.match(r'([A-Za-z0-9_]+)-[0-9a-f]{8,}', row['name']))})
        versions = sorted({value.decode('ascii') for value in REGISTRY.findall(wire)})
        require(len(crates) <= 256 and len(versions) <= 512, 'metadata count budget')
        observed.append({'path': pin['path'], 'bytes': len(wire), 'sha256': sha(wire),
            'git_blob_sha1': blob, 'verified': True, 'member_count': len(rows),
            'member_formats': dict(sorted(Counter(row['format'] for row in rows).items())),
            'normalized_member_inventory_sha256': sha(json.dumps(rows, sort_keys=True, separators=(',', ':')).encode()),
            'first_member_names': [row['name'] for row in rows[:4]],
            'rust_crate_stem_candidates': crates, 'registry_path_package_version_candidates': versions,
            'member_paths_basename_only': True, 'original_absolute_paths_recorded': False})
    return {'format_version': 1, 'schema': 'recastpress.php-static-archive-metadata.v0.5',
        'outcome': 'three_fixed_archive_metadata_inspected', 'dependency_commit': inventory['commit'],
        'input_files': {Path(__file__).resolve().relative_to(ROOT).as_posix(): sha(Path(__file__).read_bytes()),
            INVENTORY.relative_to(ROOT).as_posix(): sha(source)}, 'archives': observed,
        'archives_executed': False, 'archive_members_extracted': False,
        'network_used_by_inspector': False, 'binary_redistributed_by_inspector': False,
        'complete_dependency_graph_verified': False, 'exact_source_commits_verified': False,
        'final_wasm_retention_verified': False, 'full_license_inventory_verified': False,
        'redistribution_approved': False, 'product_runtime_selected': False}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    result = inspect()
    if args.record:
        directory = HERE / 'evidence'
        directory.mkdir(exist_ok=True)
        path = directory / 'archive-metadata.json'
        require(path.resolve() == path and not path.is_symlink(), 'linked evidence destination')
        text = json.dumps(result, indent=2) + '\n'
        require(len(text.encode('utf-8')) <= 128 * 1024, 'public JSON budget')
        path.write_text(text, encoding='utf-8', newline='\n')
    print(result['outcome'] + '; metadata candidates only, no license/source attestation')
