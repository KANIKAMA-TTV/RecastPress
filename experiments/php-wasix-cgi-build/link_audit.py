"""Audit captured CGI linker metadata after a successful same-job build.

This separate read-only phase never relinks, executes PHP, changes build.json or
turns a build/runtime failure into success. Only its bounded JSON audit receipt
is written with --record. Raw map/TSV files stay in ignored target storage.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import platform
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
HOST = ROOT / 'experiments/php-wasix-host-probe'
BUILD_RECEIPT = HERE / 'evidence/build.json'
DESTINATION = HERE / 'evidence/link-audit.json'
PUBLIC_LIMIT = 512 * 1024
PROOF_LIMIT = 4096
LABEL_LIMIT = 384


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


collector = load('recastpress_cgi_link_map_collector', HERE / 'link_map.py')
require = collector.require


def load_gate():
    # Existing CGI verification has generic local imports. Scope their aliases
    # only while loading, so a prior Fiber/context/native helper cannot replace
    # its dependencies and this audit cannot poison later helper imports.
    names = [('prepare', HERE / 'prepare.py'), ('patch_source', HERE / 'patch_source.py'),
             ('build', HERE / 'build.py'), ('check', HOST / 'check.py'),
             ('language_check', HOST / 'language_check.py'), ('cgi_check', HOST / 'cgi_check.py')]
    missing = object()
    previous = {name: sys.modules.get(name, missing) for name, _ in names}
    search_path = list(sys.path)
    try:
        for name, path in names:
            sys.modules[name] = load('recastpress_link_audit_' + name, path)
        return sys.modules['cgi_check']
    finally:
        sys.path[:] = search_path
        for name, module in previous.items():
            if module is missing:
                sys.modules.pop(name, None)
            else:
                sys.modules[name] = module


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def canonical_identity(value):
    checksum, count = hashlib.sha256(), 0
    encoder = json.JSONEncoder(sort_keys=True, separators=(',', ':'), ensure_ascii=True, allow_nan=False)
    for part in encoder.iterencode(value):
        wire = part.encode('utf-8')
        count += len(wire)
        checksum.update(wire)
    return {'canonical_json_bytes': count, 'canonical_json_sha256': checksum.hexdigest()}


def parse_json(wire):
    def pairs(rows):
        result = {}
        for key, value in rows:
            require(key not in result, 'duplicate audit JSON key')
            result[key] = value
        return result
    def bad_number(_):
        raise ValueError('non-finite audit JSON number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=pairs, parse_constant=bad_number)


def read_fixed(path, limit):
    require(path.resolve(strict=True) == path and path.is_file(), 'fixed audit file symlink/type')
    size = path.stat().st_size
    require(0 < size <= limit, 'fixed audit file byte budget')
    with path.open('rb') as source:
        wire = source.read(limit + 1)
    require(len(wire) == size and len(wire) <= limit, 'fixed audit file changed while reading')
    return wire


def source_hashes(gate):
    hashes = gate.cgi_prepare.input_hashes()
    for path in [Path(__file__).resolve(), HERE / 'link_map.py', HOST / 'cgi_check.py',
                 HOST / 'check.py', HOST / 'language_check.py', ROOT / 'scripts/owned_probe.py']:
        require(path.resolve(strict=True) == path and path.is_file(), 'audit source resolves through a link')
        hashes[path.relative_to(ROOT).as_posix()] = digest(path)
    return dict(sorted(hashes.items()))


def verified_snapshot(gate):
    provenance = gate.verify_build()  # Source/tool/patch/config/raw/final hash checks, no guest execution.
    wire = read_fixed(BUILD_RECEIPT, 16 * 1024 * 1024)
    require(hashlib.sha256(wire).hexdigest() == provenance['build_receipt_sha256'], 'CGI receipt changed between verifiers')
    receipt = parse_json(wire)
    manifest = gate.cgi_prepare.load_manifest()
    binding = gate.cgi_build.verify_link_capture(receipt, manifest)
    inventory = receipt['sources']['deps']['prebuilt_archive_files']
    require(type(inventory) is dict and len(inventory) == 28, 'CGI audit requires its exact 28 archive inventory')
    php = gate.cgi_prepare.owned_path(manifest['sources']['php']['directory'])
    raw = {'path': (php / manifest['build']['raw_output']).relative_to(ROOT).as_posix(), **receipt['raw_cgi_wasm']}
    final = dict(receipt['cgi_wasm'])
    final['path'] = (php / manifest['build']['wasm_output']).relative_to(ROOT).as_posix()
    return {'provenance': provenance, 'captures': binding, 'inventory': inventory,
            'raw_module': raw, 'final_module': final, 'input_files': source_hashes(gate)}


def clipped(value):
    """Bound public proof strings without pretending omitted text was raw."""
    if type(value) is dict:
        result = {key: clipped(item) for key, item in value.items()}
        if 'text' in value and type(value['text']) is str and len(value['text'].encode('utf-8')) > LABEL_LIMIT:
            result['text'] = None
            result['text_omitted'] = True
            result['sanitized_text_sha256'] = hashlib.sha256(value['text'].encode('utf-8')).hexdigest()
        return result
    if type(value) is list:
        return [clipped(item) for item in value]
    if type(value) is str and len(value.encode('utf-8')) > LABEL_LIMIT:
        return {'text_omitted': True, 'utf8_sha256': hashlib.sha256(value.encode('utf-8')).hexdigest()}
    return value


def first_proof(row):
    proof = {'complete_parsed_row_identity': canonical_identity(row), 'observation': row,
             'complete_parsed_row_embedded': True}
    if canonical_identity(proof)['canonical_json_bytes'] > PROOF_LIMIT:
        proof['observation'] = clipped(row)
        proof['complete_parsed_row_embedded'] = False
        proof['omission_reason'] = 'long field text omitted; original field hashes and complete parsed-row hash retained'
    require(canonical_identity(proof)['canonical_json_bytes'] <= PROOF_LIMIT, 'first proof exceeds public quota')
    return proof


def summarize(parsed):
    """Keep first exact facts plus complete-set hashes/counts; never claim absence."""
    mapping = {row['line']: row for row in parsed['map_observations']}
    why = {row['line']: row for row in parsed['why_extract_observations']}
    archives = {}
    for name, row in parsed['archives'].items():
        summary = {key: row[key] for key in ['bytes', 'sha256', 'status', 'live_map_observed',
                    'why_extract_observed', 'not_observed_or_ambiguous', 'absence_proves_nonuse', 'post_binaryen_retention_verified']}
        for source, field, records in [('map', 'map_lines', mapping), ('why_extract', 'why_extract_lines', why)]:
            numbers = row[field]
            summary[source] = {'count': len(numbers), 'line_number_list_identity': canonical_identity(numbers),
                'first_proof': first_proof(records[numbers[0]]) if numbers else None,
                'additional_proofs_omitted': max(0, len(numbers) - 1)}
        events = row['ambiguous_references']
        summary['ambiguous_references'] = {'count': len(events), **canonical_identity(events),
            'first': events[0] if events else None, 'additional_events_omitted': max(0, len(events) - 1)}
        archives[name] = summary
    complete = {}
    for key in ['map_observations', 'why_extract_observations', 'ambiguous_observations', 'unattributed_possible_lto_inputs']:
        complete[key] = {'count': len(parsed[key]), **canonical_identity(parsed[key])}
    result = {'schema': parsed['schema'], 'scope': parsed['scope'], 'reference': parsed['reference'],
        'map_capture': parsed['map_capture'], 'why_extract_capture': parsed['why_extract_capture'],
        'caller_supplied_raw_module_identity': parsed['caller_supplied_raw_module_identity'],
        'caller_supplied_final_module_identity': parsed['caller_supplied_final_module_identity'],
        'caller_supplied_inventory_sha256': parsed['caller_supplied_inventory_sha256'],
        'path_normalization': parsed['path_normalization'], 'archives': archives,
        'complete_parsed_sets': complete, 'complete_collector_result_identity': canonical_identity(parsed),
        'global_observation_samples': {'ambiguities': parsed['ambiguous_observations'][:8],
            'possible_lto_inputs': parsed['unattributed_possible_lto_inputs'][:8],
            'ambiguities_omitted': max(0, len(parsed['ambiguous_observations']) - 8),
            'possible_lto_inputs_omitted': max(0, len(parsed['unattributed_possible_lto_inputs']) - 8)},
        'full_parsed_observations_embedded': False, 'summary_is_not_raw_linker_output': True,
        'same_link_invocation_attestation_verified': False, 'post_binaryen_archive_contribution_verified': False,
        'all_archives_used_verified': False, 'full_license_inventory_verified': False,
        'license_conclusions_made': False, 'limitations': parsed['limitations']}
    require(canonical_identity(result)['canonical_json_bytes'] <= PUBLIC_LIMIT, 'public link summary exceeds 512KiB')
    return result


def public_error(failure):
    message = str(failure).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')
    message = ''.join(char if ord(char) >= 32 and ord(char) != 127 else ' ' for char in message)[:2048]
    try:
        return collector.label(message)['text']
    except ValueError:
        return 'unprintable error message omitted'


def report_bytes(report):
    wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('utf-8')
    require(len(wire) <= PUBLIC_LIMIT, 'entire public link audit exceeds 512KiB')
    return wire


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    report = {'format_version': 1, 'schema': 'recastpress.cgi-link-audit.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'outcome': 'not_run', 'audit_passed': False,
        'scope': 'separate post-build CGI raw-link archive observations', 'build_receipt_modified': False,
        'guest_or_native_php_executed': False, 'linker_executed_by_auditor': False,
        'product_runtime_selected': False, 'full_license_inventory_verified': False,
        'all_archives_used_verified': False, 'post_binaryen_archive_contribution_verified': False,
        'sysroot_source_build_attestation_verified': False, 'public_json_limit_bytes': PUBLIC_LIMIT}
    passed, before, gate, stage = False, None, None, 'build_capture_verification'
    try:
        require(platform.system() == 'Linux', 'link audit requires its actual Linux build job')
        gate = load_gate()
        before = verified_snapshot(gate)
        report['input_files'] = before['input_files']
        report['build_provenance'] = before['provenance']
        report['capture_bindings'] = before['captures']
        report['raw_module'], report['final_module'] = before['raw_module'], before['final_module']
        stage = 'raw_capture_read'
        captures = {}
        for name in ['map', 'why_extract']:
            identity = before['captures'][name]
            wire = read_fixed(ROOT / identity['path'], identity['byte_limit'])
            require(len(wire) == identity['bytes'] and hashlib.sha256(wire).hexdigest() == identity['sha256'], 'raw linker capture changed')
            captures[name] = wire
        stage = 'complete_capture_parse'
        parsed = collector.collect(captures['map'], captures['why_extract'], before['inventory'],
            before['raw_module'], before['final_module'], workspace_root=ROOT.as_posix())
        stage = 'bounded_public_summary'
        summary = summarize(parsed)
        candidate = {**report, 'audit': summary}
        # Reserve space for the final provenance verdict or a bounded failure
        # diagnostic, so failure reporting cannot itself overflow the receipt.
        require(len(report_bytes(candidate)) <= PUBLIC_LIMIT - 8192, 'public summary leaves no verdict/diagnostic budget')
        report = candidate
        passed, report['audit_passed'], report['outcome'] = True, True, 'cgi_link_observations_audited'
    except (Exception, KeyboardInterrupt) as failure:
        report['outcome'], report['failure_stage'] = 'cgi_link_audit_failed', stage
        report['diagnostics'] = public_error(failure)
    finally:
        if before is not None:
            try:
                require(verified_snapshot(gate) == before, 'source/build/tool/module/capture provenance changed during audit')
                report['source_build_tool_module_captures_unchanged_after_audit'] = True
            except (Exception, KeyboardInterrupt) as failure:
                passed, report['audit_passed'], report['outcome'] = False, False, 'audit_provenance_changed'
                report['diagnostics'] = public_error(failure)
    wire = report_bytes(report)
    if args.record:
        require(DESTINATION.resolve().is_relative_to(HERE) and not DESTINATION.is_symlink(), 'link audit destination escaped experiment')
        DESTINATION.parent.mkdir(exist_ok=True)
        temporary = DESTINATION.with_suffix('.partial')
        require(not temporary.is_symlink(), 'link audit temporary is a link')
        temporary.write_bytes(wire)
        temporary.replace(DESTINATION)
    print('CGI link audit: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    raise SystemExit(main())
