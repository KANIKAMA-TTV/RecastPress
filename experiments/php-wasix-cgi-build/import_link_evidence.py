"""Import fixed JSON-only CGI link audit receipts against their original Git tree.

This reads a separately acquired, pinned Actions ZIP. No PHP, linker, Wasm,
raw map/TSV, archive/tool binary or temporary download URL is acquired or run.
The original collector is NOT rerun: its complete-set hashes remain recorded
same-job observations; only embedded first proofs can be recomputed here.
"""
import argparse
from collections import Counter
from functools import lru_cache
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
sys.path.insert(0, str(HOST))
from language_check import equal, parse_json
import cgi_check

_spec = importlib.util.spec_from_file_location('recastpress_cgi_link_original_git_reader', HOST / 'import_ci_evidence.py')
reader = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(reader)
SOURCE = '6e3c66422e5bf4b92f20c231016849d816c95e6b'
RUN = '41323679290'
reader.COMMIT = SOURCE
require = reader.require
SPECS = {
    'llvm': {'artifact': 13090638558, 'bytes': 44982,
             'sha256': '53dee6e179f27c032c4702e1b9b6d462bb927422500f4d4e23fe1eb43995fa87'},
    'cranelift': {'artifact': 13090733158, 'bytes': 44494,
                 'sha256': 'f4974b9b8cd22c6c26dc26466c82d9e1a8f1a28a24246e6414bd937c1ae0cea4'},
}
CGI_PREFIX = 'experiments/php-wasix-cgi-build/'
CGI_WORK = 'target/php-wasix-cgi-build/'
DEPS = CGI_WORK + 'source/php-wasix-deps/'
SHA = re.compile('[0-9a-f]{64}')
EMPTY_IDENTITY = {'canonical_json_bytes': 2, 'canonical_json_sha256': hashlib.sha256(b'[]').hexdigest()}


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


@lru_cache(maxsize=512)
def original(name):
    return reader.original(name)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=True, allow_nan=False).encode('utf-8')


def canonical_identity(value):
    wire = canonical(value)
    return {'canonical_json_bytes': len(wire), 'canonical_json_sha256': sha(wire)}


def digest(value):
    require(type(value) is str and SHA.fullmatch(value), 'invalid SHA-256 observation')


def count(value, maximum=2**32):
    require(type(value) is int and 0 <= value <= maximum, 'invalid count observation')


def identity(value, minimum=0):
    equal(set(value), {'canonical_json_bytes', 'canonical_json_sha256'}, 'canonical identity fields')
    count(value['canonical_json_bytes'])
    require(value['canonical_json_bytes'] >= minimum, 'canonical identity is too small')
    digest(value['canonical_json_sha256'])


def verify_sources(value):
    hashes = value['input_files']
    require(type(hashes) is dict and hashes, 'missing original input bindings')
    for name, expected in hashes.items():
        digest(expected)
        equal(sha(original(name)), expected, 'original Git source ' + name)


def original_prepare_inputs():
    result = subprocess.run(['git', 'ls-tree', '-r', '--name-only', SOURCE, '--', 'experiments/php-wasix-cgi-build'],
        cwd=ROOT, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30, check=True)
    require(len(result.stdout) <= 256 * 1024 and not result.stderr, 'original source listing budget')
    names = {name for name in result.stdout.decode('utf-8').splitlines()
        if (name.startswith(CGI_PREFIX) and '/' not in name[len(CGI_PREFIX):] and name.endswith('.py'))
        or (name.startswith(CGI_PREFIX + 'patches/') and name.endswith('.patch'))}
    names.update([CGI_PREFIX + 'manifest.json', CGI_PREFIX + 'README.ja.md', '.github/workflows/php-wasix-cgi-build.yml'])
    return {name: sha(original(name)) for name in sorted(names)}


def members(backend):
    result = {name: 'php-wasix-cgi-build/evidence/' + name for name in
              ['build.json', 'link-audit.json', 'ci-stages.json', 'toolchain.json']}
    result.update({name: 'php-wasix-host-probe/evidence/' + name for name in
                   ['linux-' + backend + '-cgi-contract.json', 'native-cgi-contract.json']})
    if backend == 'llvm':
        result['llvm-toolchain-verification.json'] = 'php-wasix-host-probe/evidence/llvm-toolchain-verification.json'
    return result


def read_archive(backend):
    spec = SPECS[backend]
    path = ROOT / ('target/php-wasix-ci-evidence/run-' + RUN + '-' + backend + '-1.zip')
    require(path.resolve(strict=True) == path and path.is_file(), 'pinned ZIP missing or linked')
    require(path.stat().st_size == spec['bytes'], 'CI ZIP byte length changed')
    wire = path.read_bytes()
    require(len(wire) == spec['bytes'] and sha(wire) == spec['sha256'], 'CI ZIP identity changed')
    mapping = members(backend)
    with zipfile.ZipFile(path) as archive:
        entries = archive.infolist()
        equal(len(entries), len(mapping), 'exact ZIP member count')
        equal({row.filename for row in entries}, set(mapping.values()), 'exact ZIP member paths')
        for row in entries:
            require(not row.is_dir() and 0 < row.file_size <= 512 * 1024
                    and (row.external_attr >> 16) & 0o170000 != 0o120000
                    and row.flag_bits & 1 == 0, 'invalid evidence ZIP member')
        raw = {name: archive.read(member) for name, member in mapping.items()}
    return raw, {name: parse_json(wire) for name, wire in raw.items()}


def verify_build(data, raw):
    build, tools = data['build.json'], data['toolchain.json']
    manifest_wire = original(CGI_PREFIX + 'manifest.json')
    manifest = parse_json(manifest_wire)
    expected_inputs = original_prepare_inputs()
    for receipt in [build, tools]:
        verify_sources(receipt)
        equal(receipt['input_files'], expected_inputs, 'complete original preparation inputs')
    for name, expected in [('outcome', 'cgi_candidate_built_not_executed'), ('phase', 'build_complete'),
            ('guest_executed', False), ('product_runtime_selected', False), ('native_runtime_fallback_used', False),
            ('cgi_request_compatibility_verified', False), ('fiber_compatibility_verified', False),
            ('module_validation_performed', False), ('redistributed_binaries', False)]:
        equal(build[name], expected, 'build gate ' + name)
    for name, expected in [('outcome', 'verified_host_build_tools_prepared'), ('guest_executed', False),
            ('product_runtime_selected', False), ('redistributed_binaries', False)]:
        equal(tools[name], expected, 'tool gate ' + name)
    equal(build['toolchain_receipt_sha256'], sha(raw['toolchain.json']), 'same-job prepared tools')
    equal({row['name'] for row in tools['assets']}, {row['name'] for row in manifest['assets']}, 'tool asset set')
    equal(len(tools['assets']), 4, 'four fixed build assets')
    for asset in manifest['assets']:
        row = next(row for row in tools['assets'] if row['name'] == asset['name'])
        for key in ['name', 'repository', 'tag', 'asset_id', 'url', 'bytes', 'sha256']:
            equal(row[key], asset[key], 'tool pin ' + key)
        equal(row['verified'], True, 'tool archive verified')
        equal(row['observed_bytes'], asset['bytes'], 'observed archive bytes')
        equal(row['observed_sha256'], asset['sha256'], 'observed archive hash')
        equal(set(row['required_files']), set(asset['required_files']), 'extracted required-file set')
        for entry in row['required_files'].values():
            count(entry['bytes']); require(entry['bytes'] > 0, 'empty extracted tool')
            digest(entry['sha256'])
    equal(set(build['sources']), set(manifest['sources']), 'source checkout set')
    for name, pin in manifest['sources'].items():
        row = build['sources'][name]
        equal(row['repository'], pin['repository'], 'source repository')
        equal(row['commit'], pin['commit'], 'source commit')
        equal(row['tracked_worktree_clean_before_build'], True, 'clean source before build')
    equal(build['sources']['deps']['rebuilt_here'], False, 'prebuilt deps scope')
    inventory = build['sources']['deps']['prebuilt_archive_files']
    require(type(inventory) is dict and len(inventory) == 28, '28 dependency archives')
    for name, entry in inventory.items():
        require(name.endswith('.a') and not name.startswith('/') and '..' not in name.split('/'), 'archive inventory name')
        equal(set(entry), {'bytes', 'sha256'}, 'archive identity fields')
        count(entry['bytes'], 512 * 1024 * 1024); require(entry['bytes'] >= 8, 'archive byte size')
        digest(entry['sha256'])
    equal(build['configure_difference'], {'extra_flags': '--enable-cgi', 'php_source_patch': True,
        'cli_disabled': False, 'make_target': 'cgi', 'make_jobs': 2,
        'source_patch_scope': 'WASI FastCGI prefork exclusion only'}, 'source/configuration difference')
    equal(len(build['source_patches']), 1, 'one canonical CGI patch')
    patch, pin = build['source_patches'][0], manifest['source_patches'][0]
    for key in ['path', 'sha256', 'source_commit', 'target', 'before_bytes', 'before_sha256', 'after_sha256']:
        equal(patch[key], pin[key], 'CGI patch pin ' + key)
    equal(sha(original(CGI_PREFIX + pin['path'])), pin['sha256'], 'original patch bytes')
    for key, expected in [('clean_tracked_source_before_patch', True), ('applied', True),
            ('observed_patch_sha256', pin['sha256']), ('observed_before_bytes', pin['before_bytes']),
            ('observed_before_sha256', pin['before_sha256']), ('observed_before_git_blob_sha1', pin['before_git_blob_sha1']),
            ('git_apply_check_exit_code', 0), ('git_apply_apply_exit_code', 0), ('observed_after_sha256', pin['after_sha256'])]:
        equal(patch[key], expected, 'CGI applied patch ' + key)
    equal(build['wasi_prefork_support'], 'explicitly-disabled-by-source-patch', 'prefork exclusion')
    for name, entry in build['generated_configuration_files'].items():
        count(entry['bytes']); require(entry['bytes'] > 0, 'generated configuration size')
        digest(entry['sha256'])
    require({'config.log', 'config.status', 'Makefile', 'main/php_config.h', 'Zend/zend_config.h'}
            <= set(build['generated_configuration_files']), 'generated configuration presence')
    require(type(build['stub_headers_after']) is dict and len(build['stub_headers_after']) > 100, 'generated stub header set')
    for value in build['stub_headers_after'].values(): digest(value)
    for key in ['cgi_wasm', 'raw_cgi_wasm']:
        entry = build[key]
        count(entry['bytes'], 128 * 1024 * 1024); require(entry['bytes'] >= 8, 'generated module budget')
        digest(entry['sha256'])
    equal(build['cgi_wasm']['path'], manifest['build']['wasm_output'], 'CGI module path')
    equal(build['cgi_wasm']['wasm_header_verified'], True, 'CGI header gate')
    equal(build['cgi_wasm']['executed'], False, 'builder does not execute module')
    return manifest, inventory


def verify_captures(build):
    capture = build['link_capture']
    normal = '-Map=' + CGI_WORK + 'link-observations/cgi.link.map:--why-extract=' + CGI_WORK + 'link-observations/cgi.why-extract.tsv'
    for key, expected in [('scope', 'make_cgi_only'), ('normalized_flags', normal),
            ('configure_flags_absent', True), ('optimize_flags_absent', True), ('parsed_by_builder', False)]:
        equal(capture[key], expected, 'link capture policy ' + key)
    digest(capture['actual_value_sha256'])
    for name, filename, maximum in [('map', 'cgi.link.map', 67108864), ('why_extract', 'cgi.why-extract.tsv', 16777216)]:
        entry = capture[name]
        equal(set(entry), {'path', 'bytes', 'sha256', 'file_type', 'read_completed', 'byte_limit'}, 'capture fields')
        for key, expected in [('path', CGI_WORK + 'link-observations/' + filename), ('file_type', 'regular'),
                ('read_completed', True), ('byte_limit', maximum)]:
            equal(entry[key], expected, 'capture ' + name + ' ' + key)
        count(entry['bytes'], maximum); require(entry['bytes'] > 0, 'empty raw capture')
        digest(entry['sha256'])
    equal([row['name'] for row in build['build_steps']], ['configure', 'make_cgi', 'optimize_cgi'], 'build stage ordering')
    equal(build['build_steps'][0]['command'], ['bash', 'wasix-configure-eh-64.sh'], 'fixed configure command')
    make_command = build['build_steps'][1]['command']
    require(len(make_command) == 4 and make_command[:2] == ['make', '-j2'] and make_command[-1] == 'cgi'
            and re.fullmatch(r'PHP=/usr/bin/php(?:[0-9]+\.[0-9]+)?', make_command[2]), 'fixed CGI make command')
    optimizer = build['build_steps'][2]['command']
    suffix = '/' + CGI_WORK + 'tools/binaryen/bin/wasm-opt'
    require(type(optimizer[0]) is str and optimizer[0].endswith(suffix), 'fixed optimizer tool path')
    workspace = optimizer[0][:-len(suffix)]
    require(workspace.startswith('/') and len(workspace) > 1, 'fixed CI workspace prefix')
    manifest = parse_json(original(CGI_PREFIX + 'manifest.json'))
    equal(optimizer, [workspace + suffix, *manifest['build']['optimizer_flags'],
        workspace + '/' + CGI_WORK + 'source/php/' + manifest['build']['raw_output'], '-o',
        workspace + '/' + CGI_WORK + 'source/php/' + manifest['build']['wasm_output']], 'fixed CGI optimizer command')
    actual_flags = '-Map=' + workspace + '/' + CGI_WORK + 'link-observations/cgi.link.map:--why-extract=' + workspace + '/' + CGI_WORK + 'link-observations/cgi.why-extract.tsv'
    equal(capture['actual_value_sha256'], sha(actual_flags.encode('utf-8')), 'actual linker flags identity')
    for step in build['build_steps']:
        equal(step['completed'], True, 'completed build step')
        equal(step['exit_code'], 0, 'successful build step')
        count(step['elapsed_ms'])
        present = step['name'] == 'make_cgi'
        equal(step['linker_flags_environment'], {'present': present,
            'actual_value_sha256': capture['actual_value_sha256'] if present else None}, 'linker flags stage binding')
    return {'map': capture['map'], 'why_extract': capture['why_extract'], 'stage': 'make_cgi',
        'normalized_flags': normal, 'actual_value_sha256': capture['actual_value_sha256'], 'parsed_by_builder': False}


def verify_first_proof(proof, name, source, line_limit):
    equal(set(proof), {'complete_parsed_row_identity', 'observation', 'complete_parsed_row_embedded'}, 'first proof fields')
    equal(proof['complete_parsed_row_embedded'], True, 'fixed first proof fully embedded')
    observation = proof['observation']
    equal(proof['complete_parsed_row_identity'], canonical_identity(observation), 'embedded parsed first proof identity')
    require(len(canonical(proof)) <= 4096, 'embedded proof quota')
    line = observation['line']; count(line, line_limit); require(line >= 2, 'first proof line number')
    if source == 'map':
        equal(observation['kind'], 'input_chunk', 'map attribution kind')
        require(observation['section'] in ['CODE', 'DATA'], 'GLOBAL/header is not archive attribution')
        count(observation['file_offset']); count(observation['size'])
        if observation['address_or_index'] is not None: count(observation['address_or_index'])
        reference = observation
    else:
        reference = observation['extracted']
    equal(reference['inventory_archive'], name, 'exact archive-member attribution')
    equal(reference['file']['path'], DEPS + name, 'attributed archive path')
    equal(reference['file']['workspace_relative'], DEPS + name, 'normalized archive path')
    equal(reference['file']['external'], False, 'attribution is inside fixed dependency checkout')
    digest(reference['file']['source_utf8_sha256'])
    require(type(reference['member']) is dict and reference['member']['text'], 'archive member identity')
    digest(reference['member']['source_utf8_sha256'])
    equal(reference['possible_inventory_archives'], [], 'no basename-only attribution')
    equal(reference['possible_lto_generated_input'], False, 'no merged-LTO attribution')
    equal(reference['ambiguities'], [], 'exact first attribution is unambiguous')
    return line


def verify_summary(audit, inventory, bindings, raw_module, final_module):
    summary = audit['audit']
    equal(summary['schema'], 'recastpress.wasm-ld-link-map-observation.v0.5', 'collector schema')
    equal(summary['scope'], 'CODE/DATA raw-link map and pre-GC lazy extraction observations only', 'collector scope')
    equal(summary['reference'], {'llvm_release': '21.1.206', 'llvm_commit': '98c00cb849c94742b2b29918bb414d236c83ae97',
        'wasixcc_commit': 'aa934ded16300ab0d5b78de6647214b85a9a7fd7'}, 'collector source references')
    equal(summary['caller_supplied_raw_module_identity'], raw_module, 'collector raw module')
    equal(summary['caller_supplied_final_module_identity'], final_module, 'collector final module')
    equal(summary['caller_supplied_inventory_sha256'], sha(canonical(inventory)), 'collector inventory identity')
    for key, expected in [('full_parsed_observations_embedded', False), ('summary_is_not_raw_linker_output', True),
            ('same_link_invocation_attestation_verified', False), ('post_binaryen_archive_contribution_verified', False),
            ('all_archives_used_verified', False), ('full_license_inventory_verified', False), ('license_conclusions_made', False)]:
        equal(summary[key], expected, 'summary scope ' + key)
    equal(summary['path_normalization'], {'workspace_root_recorded': False,
        'link_cwd': CGI_WORK + 'source/php', 'deps_directory': DEPS[:-1],
        'external_directories_redacted': True, 'symlinks_not_resolved': True}, 'path normalization scope')
    captures = {}
    for source, field, maximum in [('map', 'map_capture', 33554432), ('why_extract', 'why_extract_capture', 8388608)]:
        row = summary[field]; equal(set(row), {'bytes', 'sha256', 'lines'}, 'parsed capture identity fields')
        equal({k: row[k] for k in ['bytes', 'sha256']}, {k: bindings[source][k] for k in ['bytes', 'sha256']}, 'raw/parser capture binding')
        count(row['bytes'], maximum); count(row['lines'], 200000); require(row['lines'] >= 1, 'parsed header line')
        captures[source] = row
    equal(set(summary['archives']), set(inventory), 'same 28 inventory archive names')
    status_counts, attributed_counts = Counter(), {'map': 0, 'why_extract': 0}
    first_lines = {'map': set(), 'why_extract': set()}
    for name, row in summary['archives'].items():
        equal({key: row[key] for key in ['bytes', 'sha256']}, inventory[name], 'archive inventory identity')
        for key in ['absence_proves_nonuse', 'post_binaryen_retention_verified']:
            equal(row[key], False, 'archive scope ' + key)
        live = row['map']['count'] > 0; why = row['why_extract']['count'] > 0
        equal(row['live_map_observed'], live, 'live CODE/DATA observation flag')
        equal(row['why_extract_observed'], why, 'pre-GC lazy extraction flag')
        equal(row['not_observed_or_ambiguous'], not live and not why, 'absence is not nonuse')
        status = 'live_map_observed' if live else 'why_extract_observed' if why else 'not_observed_or_ambiguous'
        equal(row['status'], status, 'archive observation status')
        status_counts[status] += 1
        for source in ['map', 'why_extract']:
            observed = row[source]; count(observed['count'], captures[source]['lines'] - 1)
            identity(observed['line_number_list_identity'], 2)
            equal(observed['additional_proofs_omitted'], max(0, observed['count'] - 1), 'proof omission count')
            attributed_counts[source] += observed['count']
            if observed['count']:
                line = verify_first_proof(observed['first_proof'], name, source, captures[source]['lines'])
                require(line not in first_lines[source], 'one parsed line attributed to multiple exact archives')
                first_lines[source].add(line)
                if observed['count'] == 1:
                    equal(observed['line_number_list_identity'], canonical_identity([line]), 'one complete line-number list')
            else:
                equal(observed['first_proof'], None, 'zero observations has no proof')
                equal(observed['line_number_list_identity'], EMPTY_IDENTITY, 'zero observations empty list identity')
        ambiguous = row['ambiguous_references']
        count(ambiguous['count'], 250000)
        identity({key: ambiguous[key] for key in ['canonical_json_bytes', 'canonical_json_sha256']}, 2)
        equal(ambiguous['additional_events_omitted'], max(0, ambiguous['count'] - 1), 'ambiguity omissions')
        if ambiguous['count'] == 0:
            equal(ambiguous['first'], None, 'empty ambiguity sample')
            equal({key: ambiguous[key] for key in EMPTY_IDENTITY}, EMPTY_IDENTITY, 'empty ambiguity identity')
    complete = summary['complete_parsed_sets']
    equal(set(complete), {'map_observations', 'why_extract_observations', 'ambiguous_observations', 'unattributed_possible_lto_inputs'}, 'complete parsed set identities')
    for source, field in [('map', 'map_observations'), ('why_extract', 'why_extract_observations')]:
        equal(complete[field]['count'], captures[source]['lines'] - 1, 'complete parsed row count')
        require(attributed_counts[source] <= complete[field]['count'], 'archive proofs exceed parsed rows')
    for row in complete.values():
        count(row['count'], 250000)
        identity({key: row[key] for key in ['canonical_json_bytes', 'canonical_json_sha256']}, 2)
    identity(summary['complete_collector_result_identity'], 2)
    samples = summary['global_observation_samples']
    for sample, field, omitted in [('ambiguities', 'ambiguous_observations', 'ambiguities_omitted'),
            ('possible_lto_inputs', 'unattributed_possible_lto_inputs', 'possible_lto_inputs_omitted')]:
        expected = min(8, complete[field]['count'])
        equal(len(samples[sample]), expected, 'global sample quota')
        equal(samples[omitted], complete[field]['count'] - expected, 'global sample omissions')
        for event in samples[sample]:
            require(event['source'] in ['map', 'why_extract'], 'global event capture source')
            count(event['line'], captures[event['source']]['lines']); require(event['line'] >= 2, 'global event line')
    equal(dict(status_counts), {'live_map_observed': 6, 'why_extract_observed': 17, 'not_observed_or_ambiguous': 5}, 'fixed observed archive classes')
    require(type(summary['limitations']) is list and len(summary['limitations']) >= 6, 'collector limitations retained')
    return {'archive_count': 28, 'status_counts': dict(status_counts), 'capture_lines': {k: v['lines'] for k, v in captures.items()},
        'embedded_first_proofs_verified': sum(len(v) for v in first_lines.values()),
        'complete_parsed_sets_recomputed_by_importer': False, 'raw_map_or_tsv_acquired_by_importer': False,
        'unobserved_archives': [name for name, row in summary['archives'].items() if row['not_observed_or_ambiguous']]}


def verify(raw, data, backend):
    stages, build, audit = [data[name] for name in ['ci-stages.json', 'build.json', 'link-audit.json']]
    for value in data.values():
        if 'input_files' in value: verify_sources(value)
    equal((stages['source_commit'], stages['run_id'], stages['run_attempt'], stages['backend']), (SOURCE, RUN, '1', backend), 'CI identity')
    expected = dict.fromkeys(['php_source', 'deps_source', 'parser_source', 'dependencies', 'definitions',
        'tools', 'build', 'rust', 'host_llvm', 'cgi_contract', 'link_audit'], 'success')
    expected['host_llvm'] = 'success' if backend == 'llvm' else 'skipped'
    equal(stages['stages'], expected, 'all required CGI/audit stages')
    equal(stages['product_runtime_selected'], False, 'no product adoption')
    equal(stages['redistributed_binaries'], False, 'JSON artifacts only')
    manifest, inventory = verify_build(data, raw)
    bindings = verify_captures(build)
    for key, expected in [('schema', 'recastpress.cgi-link-audit.v0.5'), ('outcome', 'cgi_link_observations_audited'),
            ('audit_passed', True), ('scope', 'separate post-build CGI raw-link archive observations'),
            ('build_receipt_modified', False), ('guest_or_native_php_executed', False), ('linker_executed_by_auditor', False),
            ('product_runtime_selected', False), ('full_license_inventory_verified', False), ('all_archives_used_verified', False),
            ('post_binaryen_archive_contribution_verified', False), ('sysroot_source_build_attestation_verified', False),
            ('public_json_limit_bytes', 524288), ('source_build_tool_module_captures_unchanged_after_audit', True)]:
        equal(audit[key], expected, 'audit gate ' + key)
    require('diagnostics' not in audit and 'failure_stage' not in audit, 'failed audit cannot become success')
    equal(audit['capture_bindings'], bindings, 'audit/build raw capture binding')
    provenance = {'build_receipt_sha256': sha(raw['build.json']), 'toolchain_receipt_sha256': sha(raw['toolchain.json']),
        'manifest_sha256': sha(original(CGI_PREFIX + 'manifest.json')), 'module_bytes': build['cgi_wasm']['bytes'],
        'module_sha256': build['cgi_wasm']['sha256'], 'source_commit': manifest['sources']['php']['commit'],
        'patch_sha256': manifest['source_patches'][0]['sha256'], 'link_capture': bindings,
        'verification_scope': 'same trusted job observations; not signed attestation or reproducible build'}
    equal(audit['build_provenance'], provenance, 'audit source/tool/patch/module chain')
    raw_module = {'path': CGI_WORK + 'source/php/' + manifest['build']['raw_output'], **build['raw_cgi_wasm']}
    final_module = {**build['cgi_wasm'], 'path': CGI_WORK + 'source/php/' + manifest['build']['wasm_output']}
    equal(audit['raw_module'], raw_module, 'audit raw module binding')
    equal(audit['final_module'], final_module, 'audit final module binding')
    expected_audit_inputs = dict(original_prepare_inputs())
    for name in [CGI_PREFIX + 'link_audit.py', CGI_PREFIX + 'link_map.py',
                 'experiments/php-wasix-host-probe/cgi_check.py', 'experiments/php-wasix-host-probe/check.py',
                 'experiments/php-wasix-host-probe/language_check.py', 'scripts/owned_probe.py']:
        expected_audit_inputs[name] = sha(original(name))
    equal(audit['input_files'], expected_audit_inputs, 'complete original audit input set')
    summary = verify_summary(audit, inventory, bindings, raw_module, final_module)
    native, contract = data['native-cgi-contract.json'], data['linux-' + backend + '-cgi-contract.json']
    equal(raw['native-cgi-contract.json'], original('experiments/php-wasix-host-probe/evidence/native-cgi-contract.json'), 'original native oracle unchanged')
    cgi_check.validate_native_baseline(native, original)
    for key, expected in [('outcome', 'cgi_contract_passed'), ('cgi_contract_passed', True),
            ('native_comparison_performed', True), ('native_comparison_passed', True), ('owned_host_reaped', True),
            ('exit_code', 0), ('build_exit_code', 0), ('product_runtime_selected', False), ('wordpress_executed', False),
            ('native_php_executed_by_this_helper', False), ('memory_limit_verified', False), ('redistributed_binaries', False)]:
        equal(contract[key], expected, 'CGI comparison gate ' + key)
    equal(contract['generated_provenance'], provenance, 'CGI guest/build/audit chain')
    require(type(contract['owned_process_pid']) is int and contract['owned_process_pid'] > 0, 'owned CGI host PID')
    cgi_check.validate_result(contract['native_result'], contract['owned_process_pid'], backend, provenance)
    comparisons = cgi_check.compare_native_requests(contract['native_result'], native)
    equal(comparisons, contract['native_comparison_results'], 'native CGI comparisons')
    require(all(row['equal_to_native'] is True for row in comparisons.values()), 'native CGI comparison failed')
    equal(contract['native_reference']['receipt_sha256'], sha(raw['native-cgi-contract.json']), 'same native oracle')
    if backend == 'llvm':
        llvm = data['llvm-toolchain-verification.json']; pin = parse_json(original('experiments/php-wasix-host-probe/llvm-toolchain.json'))
        for key, expected in [('llvm_toolchain_manifest_sha256', sha(original('experiments/php-wasix-host-probe/llvm-toolchain.json'))),
                ('archive_sha256', pin['archive_sha256']), ('archive_bytes', pin['archive_bytes']), ('llvm_config_version', '22.1.8')]:
            equal(llvm[key], expected, 'host LLVM pin ' + key)
    return {**summary, 'cgi_fixed_requests_passed': 3, 'native_cgi_comparison_passed': True,
        'build_module_bytes': build['cgi_wasm']['bytes'], 'build_module_sha256': build['cgi_wasm']['sha256'],
        'all_archives_used_verified': False, 'post_binaryen_archive_contribution_verified': False,
        'full_license_inventory_verified': False, 'license_conclusions_made': False,
        'source_build_attestation_verified': False, 'reproducible_build_verified': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', required=True, choices=sorted(SPECS))
    parser.add_argument('--verify-only', action='store_true', help='read-only verification; write no evidence')
    args = parser.parse_args()
    raw, data = read_archive(args.backend)
    observations = verify(raw, data, args.backend)
    if not args.verify_only:
        destination = HERE / ('evidence/ci-' + RUN + '-' + args.backend)
        require(destination.resolve() == destination and destination.is_relative_to(HERE), 'import destination escaped experiment')
        destination.mkdir(parents=True, exist_ok=True)
        for name, wire in raw.items():
            path = destination / name
            require(not path.is_symlink(), 'import destination is linked')
            if path.exists(): equal(path.read_bytes(), wire, 'original imported receipt immutable')
            else: path.write_bytes(wire)
        inputs = [Path(__file__).resolve(), HOST / 'import_ci_evidence.py', HOST / 'cgi_check.py', HOST / 'language_check.py']
        spec = SPECS[args.backend]
        provenance = {'format_version': 1, 'kind': 'source_verified_json_only_cgi_link_audit_import',
            'source_commit': SOURCE, 'backend': args.backend,
            'run_url': 'https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/' + RUN,
            'artifact_id': spec['artifact'], 'artifact_zip_bytes': spec['bytes'], 'artifact_zip_sha256': spec['sha256'],
            'original_git_source_hashes_verified': True, 'ci_verdict': 'success',
            'recorded_source_tool_module_capture_chain_cross_checked': True,
            'source_tool_or_module_binaries_reverified_by_importer': False,
            'php_or_linker_executed_by_importer': False, 'native_or_wasm_binaries_acquired_by_importer': False,
            'raw_map_or_tsv_acquired_by_importer': False, 'collector_rerun_by_importer': False,
            'product_runtime_selected': False, 'full_license_inventory_verified': False,
            'input_files': {path.relative_to(ROOT).as_posix(): sha(path.read_bytes()) for path in inputs},
            'archive_member_map': members(args.backend), 'imported_files': {name: sha(wire) for name, wire in raw.items()},
            'observations': observations}
        path = destination / 'provenance.json'
        require(not path.is_symlink(), 'derived provenance is linked')
        path.write_text(json.dumps(provenance, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('Verified source-bound CGI link audit (' + args.backend + '): 28 archives, 6 live-map / 17 why-only / 5 unobserved-or-ambiguous; absence is not nonuse.')


if __name__ == '__main__':
    try:
        main()
    except (Exception, KeyboardInterrupt) as error:
        print('CGI link evidence import failed: ' + str(error), file=sys.stderr)
        sys.exit(1)
