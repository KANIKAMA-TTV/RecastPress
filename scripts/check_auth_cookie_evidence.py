"""Prepared GET cookie/session relations; never production authentication proof."""
from datetime import datetime, timedelta
import hashlib
import json
from pathlib import Path
import re

import auth_cookie_cases as corpus
import check_password_generation_evidence as common

ROOT = common.ROOT
OUTPUT = ROOT / 'compatibility/evidence/auth-cookie-relations.json'
MAX_JSON = 512 * 1024
SCOPE = 'prepared_auth_cookie_get_public_dummy_stable_clock_relations'
REFERENCE_FILES = ('pluggable.php', 'load.php', 'plugin.php', 'class-wp-hook.php',
                   'class-wp-filter-sentinel.php', 'class-wp-session-tokens.php',
                   'class-wp-user-meta-session-tokens.php')
RESULT_KEYS = ('id', 'outcome', 'stage', 'prepared_user_id')
CONTROL_NAMES = ('unexpected_argument', 'duplicate_envelope_field', 'unknown_envelope_field',
                 'valid_then_refused_batch_has_no_partial_output', 'global_input_cap_plus_one')
LIMITATIONS = [
    'Finite public dummies and prepared record witnesses; no principal, permissions or static WP API count added.',
    'Actual unchanged PHP builtin time is bracketed; stable host wall clock inside endpoints is a trusted assumption.',
    'All cookie/session expiry comparisons stay stable at both endpoints; exact equality/extrema are native-only contracts.',
    'Caller-prepared record lookup/resolved salt/complete typed session projection replace DB/cache/resolver policy.',
    'Actual default session classes and named fixture/action counters; internal time/HMAC calls are uninstrumented.',
    'CLI captures and logical byte limits are not OS/RSS/process-tree sandbox or whole-function constant timing guarantees.',
    'No real credentials, cookie headers, session creation/writes, key storage/rotation, HTTP auth or native Linux execution.',
]


def profile(source_hashes):
    return {
        'name': 'prepared_get_nonajax_user_resolved_salt_complete_sessions_empty_hooks',
        'php_version': '8.5.12', 'php_int_size': 8, 'request_method': 'GET', 'doing_ajax_defined': False,
        'builtin_clock': True, 'max_bracket_seconds': 2, 'all_expiries_endpoint_stable': True,
        'user_lookup': 'record_only_stdclass_fixture_not_db_or_WP_User_policy',
        'salt': 'caller_resolved_fixture_no_resolver', 'session_manager': 'actual_base_and_default_user_meta_classes',
        'actions': 'actual_empty_WP_Hook_did_action_counters', 'internal_time_and_hmac_calls': 'uninstrumented',
        'fixture_limits': {'max_input_bytes': 65536, 'max_sessions': 64, 'max_session_key_bytes': 4096},
        'source_sha256': source_hashes,
    }


def read_json(path):
    with Path(path).open('rb') as source:
        wire = source.read(MAX_JSON + 1)
    if len(wire) > MAX_JSON:
        raise ValueError('cookie evidence exceeds byte bound')
    return json.loads(wire, object_pairs_hook=common.unique_object,
                      parse_constant=lambda _: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def tracked_files():
    return {
        'Cargo.toml', 'Cargo.lock', 'crates/recastpress-wp/Cargo.toml',
        'scripts/auth_cookie_cases.py', 'scripts/auth_cookie_oracle.php',
        'scripts/check_auth_cookie_evidence.py', 'scripts/compare_auth_cookies.py',
        'scripts/test_auth_cookie_relation.py', 'scripts/test_auth_cookie_evidence.py',
        'scripts/reference_functions.php', 'scripts/owned_probe.py', 'scripts/setup_php.py', 'scripts/wordpress_reference.py',
        'scripts/check_password_generation_evidence.py',
        '.reference/php-8.5.12/recastpress-tool.json',
        '.reference/wordpress-7.1.3/recastpress-reference.json',
        *('.reference/wordpress-7.1.3/wp-includes/' + file for file in REFERENCE_FILES),
        *(path.relative_to(ROOT).as_posix() for folder in ('src', 'tests', 'examples')
          for path in (ROOT / 'crates/recastpress-wp' / folder).rglob('*.rs')),
    }


def binary_paths():
    import os
    return {'rust': 'target/debug/examples/auth_cookie_verify_probe' + ('.exe' if os.name == 'nt' else ''),
            'php': '.reference/php-8.5.12/php.exe'}


def verify_pinned():
    common.verify_pinned()
    source = ROOT / '.reference/wordpress-7.1.3'
    with (source / 'recastpress-reference.json').open('rb') as handle:
        wire = handle.read(2 * 1024 * 1024 + 1)
    if len(wire) > 2 * 1024 * 1024:
        raise ValueError('cookie reference manifest bound')
    manifest = json.loads(wire, object_pairs_hook=common.unique_object)
    listed = {row['path']: row['sha256'] for row in manifest['files']}
    for file in REFERENCE_FILES:
        relative = 'wp-includes/' + file
        if common.digest(source / relative) != listed[relative]:
            raise ValueError('pinned cookie source differs')


def corpus_hash(cases):
    return hashlib.sha256(common.canonical(cases)).hexdigest()


def expected_result(case):
    stage = case['expected_stage']
    request = case['request_without_now']
    return {'id': request['id'], 'outcome': 'match' if stage == 'valid' else 'no_match',
            'stage': stage, 'prepared_user_id': request['user']['id'] if stage == 'valid' else None}


def validate_php_cases(rows, cases, anchor):
    if type(rows) is not list or len(rows) != len(cases):
        raise ValueError('cookie PHP coverage differs')
    for row, case in zip(rows, cases, strict=True):
        common.fields(row, [*RESULT_KEYS, 'clock_before', 'clock_after', 'clock_stable', 'counts', 'actions'])
        if not common.exact({key: row[key] for key in RESULT_KEYS}, expected_result(case)):
            raise ValueError('cookie PHP expected stage differs')
        before, after = row['clock_before'], row['clock_after']
        if (type(before) is not int or type(after) is not int or not anchor - 2 <= before <= anchor + 120
                or not 0 <= after - before <= 2 or row['clock_stable'] is not True):
            raise ValueError('cookie clock bracket differs')
        request = case['request_without_now']
        pieces = bytes.fromhex(request['cookie_hex']).split(b'|')
        if len(pieces) == 4:
            expiry = int(pieces[1].decode('ascii'))
            if (expiry < before) != (expiry < after):
                raise ValueError('cookie expiry endpoint unstable')
        for session in request['sessions']:
            expiry = int(session['expiration'])
            if (expiry >= before) != (expiry >= after):
                raise ValueError('session expiry endpoint unstable')
        if (not common.exact(row['counts'], corpus.expected_counts(case['expected_stage']))
                or not common.exact(row['actions'], corpus.expected_actions(case['expected_stage']))):
            raise ValueError('cookie per-case counters/actions differ')


def validate_native_cases(rows, cases):
    if type(rows) is not list or len(rows) != len(cases):
        raise ValueError('cookie native coverage differs')
    for row, case in zip(rows, cases, strict=True):
        common.fields(row, RESULT_KEYS)
        if not common.exact(row, expected_result(case)):
            raise ValueError('cookie native expected stage differs')


def validate_run(run):
    common.fields(run, ('owned_pid', 'reported_pid', 'exit_code', 'stderr_empty'))
    common.positive_pid(run['owned_pid'])
    common.positive_pid(run['reported_pid'])
    if (not common.exact(run['owned_pid'], run['reported_pid'])
            or not common.exact(run['exit_code'], 0) or run['stderr_empty'] is not True):
        raise ValueError('cookie observed process identity differs')


def validate_record(record, verify_files=True):
    common.fields(record, ('format_version', 'scope', 'generated_at_utc', 'fixture_anchor', 'corpus_sha256',
                           'profile', 'runs', 'php', 'native_before', 'native_after', 'closed_controls',
                           'tracked_files', 'binaries', 'limitations'))
    if (not common.exact(record['format_version'], 1) or record['scope'] != SCOPE
            or not common.exact(record['limitations'], LIMITATIONS)):
        raise ValueError('cookie evidence scope differs')
    stamp = record['generated_at_utc']
    if type(stamp) is not str or 'T' not in stamp or datetime.fromisoformat(stamp).utcoffset() != timedelta(0):
        raise ValueError('cookie timestamp must be UTC')
    anchor = record['fixture_anchor']
    if type(anchor) is not int or not 3600 < anchor < (1 << 63) - 7200:
        raise ValueError('cookie fixture anchor differs')
    if not anchor - 2 <= datetime.fromisoformat(stamp).timestamp() <= anchor + 180:
        raise ValueError('cookie observation timestamp differs from fixture anchor')
    cases = corpus.cases(anchor)
    if record['corpus_sha256'] != corpus_hash(cases):
        raise ValueError('cookie corpus differs')
    common.fields(record['tracked_files'], tracked_files())
    for path, digest in record['tracked_files'].items():
        if type(digest) is not str or not re.fullmatch('[0-9a-f]{64}', digest):
            raise ValueError('cookie source identity malformed')
        if verify_files and digest != common.digest(ROOT / path):
            raise ValueError('stale cookie source: ' + path)
    source_hashes = {'wp-includes/' + file: record['tracked_files']['.reference/wordpress-7.1.3/wp-includes/' + file]
                     for file in REFERENCE_FILES}
    if not common.exact(record['profile'], profile(source_hashes)):
        raise ValueError('cookie actual PHP profile differs')
    common.fields(record['runs'], ('php', 'native_before', 'native_after'))
    for run in record['runs'].values():
        validate_run(run)
    validate_php_cases(record['php'], cases, anchor)
    validate_native_cases(record['native_before'], cases)
    validate_native_cases(record['native_after'], cases)
    if type(record['closed_controls']) is not list or len(record['closed_controls']) != len(CONTROL_NAMES):
        raise ValueError('cookie refusal coverage differs')
    for row, name in zip(record['closed_controls'], CONTROL_NAMES, strict=True):
        common.fields(row, ('id', 'owned_pid', 'exit_code', 'stdout_empty', 'fixed_opaque_stderr'))
        common.positive_pid(row['owned_pid'])
        if (row['id'] != name or not common.exact(row['exit_code'], 2)
                or row['stdout_empty'] is not True or row['fixed_opaque_stderr'] is not True):
            raise ValueError('cookie refusal observation differs')
    common.fields(record['binaries'], binary_paths())
    for name, path in binary_paths().items():
        row = record['binaries'][name]
        common.fields(row, ('path', 'sha256'))
        if row['path'] != path or type(row['sha256']) is not str or not re.fullmatch('[0-9a-f]{64}', row['sha256']):
            raise ValueError('cookie binary identity differs')
        if verify_files and row['sha256'] != common.digest(ROOT / path):
            raise ValueError('stale cookie binary')
    return record


def main():
    verify_pinned()
    validate_record(read_json(OUTPUT))
    print('Prepared cookie relations30/current sources and binaries; no principal or static WP API count')


if __name__ == '__main__':
    main()
