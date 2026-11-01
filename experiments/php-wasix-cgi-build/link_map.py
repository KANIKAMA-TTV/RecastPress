"""Pure, bounded wasm-ld collector used by the separate link_audit step.

The CGI builder captures raw files but does not run this parser in its success
predicate. Actual linker-output parsing is not established by synthetic tests.

Reference: wasix-org/llvm-project 21.1.206, commit
98c00cb849c94742b2b29918bb414d236c83ae97: lld/wasm/MapFile.cpp,
InputFiles.cpp, InputChunks.cpp, Driver.cpp and lld/test/wasm/why-extract.s.
https://github.com/wasix-org/llvm-project/blob/98c00cb849c94742b2b29918bb414d236c83ae97/lld/wasm/MapFile.cpp
wasixcc v0.4.4 source aa934ded16300ab0d5b78de6647214b85a9a7fd7
forwards raw WASIXCC_LINKER_FLAGS (colon separated) directly to wasm-ld.

collect() accepts captured bytes and caller-supplied file identities only. It
does no filesystem access, subprocess execution, download or evidence writing.
Paths in module identities, when supplied, are workspace-relative metadata.
Map input references are resolved lexically relative to link_cwd. The caller
must separately verify the actual files, command and same-link provenance.

Map CODE/DATA chunks describe raw-link observations BEFORE Binaryen. GLOBAL
names and other output-section headers have no member attribution here. Lazy
extraction reasons precede GC, omit whole-archive's eager path, and do not prove
retention. LTO/merged inputs lose origin; map absence proves neither non-use nor
license conclusions. A member name need not uniquely identify an ar occurrence.
"""
import hashlib
import json
import posixpath
import re
import unicodedata

LLVM_COMMIT = '98c00cb849c94742b2b29918bb414d236c83ae97'
WASIXCC_COMMIT = 'aa934ded16300ab0d5b78de6647214b85a9a7fd7'
MAX_MAP_BYTES = 32 * 1024 * 1024
MAX_WHY_BYTES = 8 * 1024 * 1024
MAX_LINES = 200000
MAX_TOTAL_ROWS = 250000
MAX_LINE_BYTES = 16384
MAX_MODULE_BYTES = 128 * 1024 * 1024
MAP_HEADER = '    Addr      Off     Size Out     In      Symbol'
WHY_HEADER = 'reference\textracted\tsymbol'
_ROW = re.compile(r'^ *(-|[0-9a-f]{1,16}) +([0-9a-f]{1,16}) +([0-9a-f]{1,16}) (.*)$')
_DIGEST = re.compile('[0-9a-f]{64}')
_DRIVE = re.compile(r'^[A-Za-z]:/')
_SECTIONS = {'TYPE', 'IMPORT', 'FUNCTION', 'TABLE', 'MEMORY', 'GLOBAL', 'EXPORT',
             'START', 'ELEM', 'CODE', 'DATA', 'DATA_COUNT', 'DATACOUNT', 'TAG'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def text(value, empty=False):
    require(type(value) is str and (empty or bool(value)), 'empty/non-string map field')
    require(len(value.encode('utf-8')) <= MAX_LINE_BYTES
            and not any(unicodedata.category(char) in {'Cc', 'Cf', 'Cs'} for char in value),
            'map field has a control/format character or exceeds its budget')
    return value


def relative(value):
    text(value)
    require('\\' not in value and not value.startswith('/') and not _DRIVE.match(value)
            and all(part not in {'', '.', '..'} and ':' not in part for part in value.split('/')),
            'expected a canonical workspace-relative path')
    return value


def lines(wire, limit, header):
    require(type(wire) is bytes and 0 < len(wire) <= limit, 'map/TSV byte type or budget')
    require(wire.endswith(b'\n') and b'\r' not in wire and b'\0' not in wire, 'map/TSV must be complete LF-only text')
    rows = wire[:-1].split(b'\n')
    require(len(rows) <= MAX_LINES and all(0 < len(row) <= MAX_LINE_BYTES for row in rows), 'map/TSV line budget or blank line')
    rows = [row.decode('utf-8', errors='strict') for row in rows]
    require(rows[0] == header and header not in rows[1:], 'missing/duplicate map/TSV header')
    return rows


def label(value):
    text(value, empty=True)
    # Names are opaque, never paths to open. Redact embedded absolute-looking
    # paths too, while retaining their exact original field hash for auditing.
    normalized = value.replace('\\', '/')
    sanitized = re.sub(r'(?<![A-Za-z0-9_])(?:[A-Za-z]:/|/)[^\s]*', '<path>', normalized)
    return {'text': sanitized, 'source_utf8_sha256': sha(value.encode('utf-8')),
            'path_redacted': sanitized != normalized}


def environment(workspace_root, link_cwd, deps_directory):
    text(workspace_root)
    root = workspace_root.replace('\\', '/').rstrip('/')
    require((root.startswith('/') or _DRIVE.match(root)) and not root.startswith('//')
            and posixpath.normpath(root) == root and len(root.split('/')) >= 3
            and not any(part in {'.', '..'} for part in root.split('/')), 'workspace root must be a specific canonical absolute directory')
    return {'root': root, 'cwd': relative(link_cwd), 'deps': relative(deps_directory)}


def path_reference(value, env):
    text(value)
    slash = value.replace('\\', '/')
    if slash == '<internal>':
        return {'path': '<internal>', 'workspace_relative': None, 'external': False,
                'source_utf8_sha256': sha(value.encode('utf-8'))}
    absolute = slash.startswith('/') or bool(_DRIVE.match(slash))
    normalized = posixpath.normpath(slash if absolute else posixpath.join(env['cwd'], slash))
    if absolute:
        prefix = env['root'] + '/'
        inside = normalized.startswith(prefix)
        local = normalized[len(prefix):] if inside else None
    else:
        inside = normalized != '..' and not normalized.startswith('../') and not normalized.startswith('/')
        local = normalized if inside else None
    if inside:
        relative(local)
        display = local
    else:
        # No outside absolute path, directory names or host root is published.
        display = '<external>/' + label(posixpath.basename(normalized))['text']
    return {'path': display, 'workspace_relative': local, 'external': not inside,
            'source_utf8_sha256': sha(value.encode('utf-8'))}


def inventory_rows(inventory, env):
    require(type(inventory) is dict and len(inventory) == 28, 'exactly 28 supplied dependency archives required')
    rows, lookup = {}, {}
    for name, identity in inventory.items():
        relative(name)
        require(name.endswith('.a') and type(identity) is dict and set(identity) == {'bytes', 'sha256'}
                and type(identity['bytes']) is int and 8 <= identity['bytes'] <= 512 * 1024 * 1024
                and type(identity['sha256']) is str and _DIGEST.fullmatch(identity['sha256']), 'invalid archive identity')
        require(name.casefold() not in {key.casefold() for key in rows}, 'case-ambiguous archive identity')
        rows[name] = {'bytes': identity['bytes'], 'sha256': identity['sha256'],
                      'live_map_observed': False, 'why_extract_observed': False,
                      'map_lines': [], 'why_extract_lines': [], 'ambiguous_references': []}
        lookup[env['deps'] + '/' + name] = name
    return rows, lookup


def module_identity(identity):
    require(type(identity) is dict and {'bytes', 'sha256'} <= set(identity)
            and set(identity) <= {'path', 'bytes', 'sha256', 'wasm_header_verified', 'executed'}, 'module identity fields')
    require(type(identity['bytes']) is int and 8 <= identity['bytes'] <= MAX_MODULE_BYTES
            and type(identity['sha256']) is str and _DIGEST.fullmatch(identity['sha256']), 'module identity value/type')
    result = dict(identity)
    if 'path' in result:
        relative(result['path'])
    if 'wasm_header_verified' in result:
        require(type(result['wasm_header_verified']) is bool, 'module header flag type')
    if 'executed' in result:
        require(result['executed'] is False, 'collector requires pre-execution identities')
    return result


def file_reference(value, env, lookup):
    text(value)
    if value == '--entry':
        # Driver.cpp uses this marker as a reason, not an InputFile pathname.
        return {'file': {'path': '<linker-option:--entry>', 'workspace_relative': None, 'external': False,
                         'source_utf8_sha256': sha(value.encode('utf-8'))},
                'member': None, 'inventory_archive': None, 'possible_inventory_archives': [],
                'possible_lto_generated_input': False, 'ambiguities': ['linker-control-reason-not-a-file']}
    archive_member = '.a(' in value and value.endswith(')')
    leaf = posixpath.basename(value.replace('\\', '/'))
    lto = not archive_member and ('.lto.' in leaf or leaf == 'ld-temp.o')
    member = None
    archive = value
    if '.a(' in value and not lto:
        archive, _, member = value.partition('.a(')
        archive += '.a'
        require(member.endswith(')') and len(member) > 1, 'malformed archive/member notation')
        member = member[:-1]
        text(member)
    reference = path_reference(archive if member is not None else value, env)
    known = lookup.get(reference['workspace_relative']) if member is not None and not lto else None
    candidates = []
    if member is not None and known is None:
        leaf = posixpath.basename(archive.replace('\\', '/'))
        candidates = sorted(name for path, name in lookup.items() if posixpath.basename(path) == leaf)
    reasons = []
    if lto:
        reasons.append('possible-LTO-generated-input-origin-not-attributed')
    if reference['external']:
        reasons.append('outside-workspace-path-redacted')
    if candidates:
        reasons.append('archive-basename-match-is-not-path-identity')
    if known is None:
        reasons.append('no-exact-inventory-archive-member-identity')
    return {'file': reference, 'member': label(member) if member is not None else None,
            'inventory_archive': known, 'possible_inventory_archives': candidates,
            'possible_lto_generated_input': lto, 'ambiguities': reasons}


def collect(map_bytes, why_extract_bytes, inventory, raw_identity, final_identity, *, workspace_root,
            link_cwd='target/php-wasix-cgi-build/source/php',
            deps_directory='target/php-wasix-cgi-build/source/php-wasix-deps'):
    """Return observations; module/archive hashes are supplied, not remeasured.

    Relative references in the linker text use link_cwd; inventory paths use
    deps_directory. No basename-only match is promoted to archive-use evidence.
    The identities' optional path fields are canonical workspace-relative names.
    """
    env = environment(workspace_root, link_cwd, deps_directory)
    archives, lookup = inventory_rows(inventory, env)
    raw, final = module_identity(raw_identity), module_identity(final_identity)
    mapping = lines(map_bytes, MAX_MAP_BYTES, MAP_HEADER)
    extracted = lines(why_extract_bytes, MAX_WHY_BYTES, WHY_HEADER)
    require(len(mapping) + len(extracted) <= MAX_TOTAL_ROWS, 'combined linker observation row budget')
    observations, why, ambiguous, lto_inputs = [], [], [], []
    section, parent_chunk = None, None

    def register(reference, source, number, positive):
        known = reference['inventory_archive']
        if known is not None and positive:
            row = archives[known]
            row['live_map_observed' if source == 'map' else 'why_extract_observed'] = True
            row['map_lines' if source == 'map' else 'why_extract_lines'].append(number)
        if reference['ambiguities']:
            event = {'source': source, 'line': number, 'reasons': reference['ambiguities']}
            ambiguous.append(event)
            for candidate in reference['possible_inventory_archives']:
                archives[candidate]['ambiguous_references'].append(event)
        if reference['possible_lto_generated_input']:
            lto_inputs.append({'source': source, 'line': number})

    for number, line in enumerate(mapping[1:], 2):
        match = _ROW.fullmatch(line)
        require(match is not None, 'invalid wasm-ld map row at line ' + str(number))
        address, offset, size, tail = match.groups()
        indent = len(tail) - len(tail.lstrip(' '))
        require(indent in {0, 8, 16}, 'unsupported map row indentation')
        payload = text(tail[indent:])
        row = {'line': number, 'address_or_index': None if address == '-' else int(address, 16),
               'file_offset': int(offset, 16), 'size': int(size, 16)}
        if indent == 0:
            parent_chunk = None
            if payload in _SECTIONS or (payload.startswith('CUSTOM(') and payload.endswith(')')):
                require(row['address_or_index'] is None and row['file_offset'] + row['size'] <= raw['bytes'],
                        'output section exceeds supplied raw-module size')
                section = payload
                row.update(kind='output_section', section=label(payload))
            elif section == 'DATA':
                row.update(kind='data_segment', section='DATA', name=label(payload))
            else:
                raise ValueError('unknown top-level wasm-ld map layout')
        elif indent == 8 and section in {'CODE', 'DATA'}:
            owner, separator, chunk = payload.rpartition(':(')
            require(separator and chunk.endswith(')') and bool(owner), 'input chunk lacks file:(chunk) framing')
            reference = file_reference(owner, env, lookup)
            row.update(kind='input_chunk', section=section, chunk=label(chunk[:-1]), **reference)
            register(reference, 'map', number, True)
            parent_chunk = number
        elif indent == 8 and section == 'GLOBAL':
            # Global rows are names/indices, not file/member source evidence.
            row.update(kind='global_name', section='GLOBAL', name=label(payload))
            parent_chunk = None
        elif indent == 16 and section in {'CODE', 'DATA'} and parent_chunk is not None:
            row.update(kind='symbol', section=section, parent_chunk_line=parent_chunk, name=label(payload))
        else:
            raise ValueError('orphan or unsupported wasm-ld map row')
        observations.append(row)
    require(any(row['kind'] == 'output_section' for row in observations), 'empty output-section map')

    for number, line in enumerate(extracted[1:], 2):
        columns = line.split('\t')
        require(len(columns) == 3 and all(columns), 'why-extract requires exactly three nonempty fields')
        source, target, symbol = columns
        reference = file_reference(target, env, lookup)
        origin = file_reference(source, env, lookup)
        why.append({'line': number, 'reference': origin,
                    'extracted': reference, 'symbol': label(symbol)})
        register(reference, 'why_extract', number, True)
        if origin['possible_lto_generated_input']:
            lto_inputs.append({'source': 'why_extract_reference', 'line': number})

    for row in archives.values():
        row['not_observed_or_ambiguous'] = not (row['live_map_observed'] or row['why_extract_observed']) or bool(row['ambiguous_references'])
        row['status'] = ('live_map_observed' if row['live_map_observed'] else
                         'why_extract_observed' if row['why_extract_observed'] else 'not_observed_or_ambiguous')
        row['absence_proves_nonuse'] = False
        row['post_binaryen_retention_verified'] = False
    return {
        'format_version': 1, 'schema': 'recastpress.wasm-ld-link-map-observation.v0.5',
        'scope': 'CODE/DATA raw-link map and pre-GC lazy extraction observations only',
        'reference': {'llvm_release': '21.1.206', 'llvm_commit': LLVM_COMMIT, 'wasixcc_commit': WASIXCC_COMMIT},
        'collector_integrated_into_builder': False, 'linker_executed_by_collector': False,
        'linker_version_verified_by_collector': False,
        'archive_files_hashed_here': False, 'module_files_hashed_here': False,
        'same_link_invocation_verified': False, 'post_binaryen_archive_contribution_verified': False,
        'license_conclusions_made': False, 'source_utf8': 'strict',
        'map_capture': {'bytes': len(map_bytes), 'sha256': sha(map_bytes), 'lines': len(mapping)},
        'why_extract_capture': {'bytes': len(why_extract_bytes), 'sha256': sha(why_extract_bytes), 'lines': len(extracted)},
        'caller_supplied_raw_module_identity': raw, 'caller_supplied_final_module_identity': final,
        'caller_supplied_inventory_sha256': sha(json.dumps(inventory, sort_keys=True, separators=(',', ':')).encode('utf-8')),
        'path_normalization': {'workspace_root_recorded': False, 'link_cwd': env['cwd'], 'deps_directory': env['deps'],
                               'external_directories_redacted': True, 'symlinks_not_resolved': True},
        'archives': archives, 'map_observations': observations, 'why_extract_observations': why,
        'ambiguous_observations': ambiguous, 'unattributed_possible_lto_inputs': lto_inputs,
        'limitations': ['Inventory bytes/SHA and module identities are caller assertions; bind actual files and command separately.',
            'Map rows describe the raw CGI link before Binaryen optimization, not final deployed code provenance.',
            'Only CODE/DATA chunks carry source evidence here; GLOBAL names and other section headers do not.',
            'why-extract records lazy extraction before GC and is not a complete eager/whole-archive input ledger.',
            'LTO, merged chunks, copied objects, aliases and duplicate archive-member names can hide original attribution.',
            'Not observed does not mean unused, and no source/license or legal conclusion is inferred.'],
    }
