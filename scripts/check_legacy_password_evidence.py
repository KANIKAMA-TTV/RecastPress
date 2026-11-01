"""Separate fixed-dummy legacy verification evidence, never registry compatibility counts."""
from datetime import datetime, timedelta
import hashlib
import json
import re

import check_password_generation_evidence as common
import wp_bcrypt_profile as bcrypt_vectors

ROOT = common.ROOT
OUTPUT = ROOT / 'compatibility/evidence/wp-legacy-password-crosscheck.json'
MAX_JSON = 128 * 1024
PROFILE = {'wordpress': '7.1.3', 'php': '8.5.12', 'hasher': 'empty', 'filters': 'empty', 'user_id': 0,
           'md5_hash_max_bytes': 32, 'md5_password_4096_guard': False, 'trim': False,
           'phpass_prefix': '$P$', 'phpass_hash_bytes': 34, 'phpass_salt': 'crypt64_ascii_only',
           'phpass_cost_min': 7, 'phpass_cost_max': 30, 'phpass_tested_cpu_costs': [7, 13],
           'phpass_order': 'envelope_raw4096_cost_host_canonical_cpu', 'fixture_input_max_bytes': 8192,
           'md5_provider': 'md-5@0.11.0/zeroize', 'comparison': 'actual_wp_boolean_and_explicit_host_refusals',
           'combined': {'order': 'hash_le32_md5_then_raw_gt4096_false_then_wp_then_P_then_unsupported',
                        'limits': 'required_bcrypt_4_to_31_and_portable_7_to_30',
                        'provider_failure': 'fatal_no_evidence', 'bcrypt_cpu_costs': [4, 12],
                        'legacy64_sha256': '4c19c37feeac6367141e79a6a8e2a9264895d3aee9ffe2469937447055c99e6f',
                        'bcrypt_vectors_sha256': bcrypt_vectors.VECTORS_SHA256}}
LIMITATIONS = [
    'Finite fixed public dummies only; no new legacy hashing, login, rehash, credential or session policy.',
    'Separate experiment; no compatibility registry APIs or exact primitive comparison counts added.',
    'Portable phpass is exact ASCII $P$ canonical subset, not arbitrary salt bytes or crypt fallback.',
    'Host/domain refusals are not actual WP false results and do not execute the reference function.',
    'Only public costs 7 and 13 are computed; higher admitted costs require an explicit host allowance.',
    'No whole-memory erasure, whole-operation constant time, RSS budget or timing guarantee.',
    'Current local pinned source/tool observations assume a trusted stable workspace, not continuous attestation.',
    'Combined mode adds selected raw dispatch, not all password_verify formats; its raw >4096 guard admits other-format false results.',
]
# Actual pinned class-phpass.php::crypt_private public-dummy observations.
# These fixed salts are test fixtures, never a product salt-generation API.
VECTORS = [
    ('empty', b'', '$P$512345678YsQn8dJMnXIqYTKyUqL7q1'),
    ('public', b'public-legacy-dummy', '$P$512345678X6j1oU8AetC4zjJyp/36r1'),
    ('binary', b'\x00\xffpublic\x80dummy', '$P$5./09AZaz2gimYXfzYGLpJD2anG/Zf0'),
    ('spaced', b' \tpublic-dummy\x00 ', '$P$5salt1234V9bkU6TOOK2Vdi9Elmjy81'),
    ('default13', b'public-legacy-dummy', '$P$B12345678ymVq6hVWkVcT1b2.4Yw6B.'),
    ('max4096', b'x'*4096, '$P$512345678PeS8FEIfNJzWuqAFIL7wz/'),
]

def legacy_cases():
    rows = []
    def add(name, mode, password, stored, expected, actual=True, maximum=13):
        request = {'mode': mode, 'password_hex': password.hex(), 'hash_hex': stored.hex()}
        if mode == 'phpass': request['maximum'] = maximum
        rows.append({'id': name, 'request': request, 'expected': expected, 'actual': actual})
    values = [('empty', b''), ('plain', b'public-legacy-dummy'), ('binary', b'\x00\xffpublic\x80dummy'),
              ('spaced', b' \tpublic-dummy\x00 '), ('ff', b'\x0cdummy\x0c'), ('utf8', '公開ダミー'.encode()),
              ('max4096', b'x'*4096), ('over4097', b'x'*4097), ('max_fixture8192', b'x'*8192)]
    for name, password in values:
        stored = hashlib.md5(password, usedforsecurity=False).hexdigest().encode('ascii')
        add('md5_' + name, 'md5', password, stored, True)
        add('md5_wrong_' + name, 'md5', b'wrong-public-dummy', stored, False)
        add('md5_upper_' + name, 'md5', password, stored.upper(), False)
    spaced = values[3][1]
    add('md5_no_trim', 'md5', common.trim6(spaced), hashlib.md5(spaced, usedforsecurity=False).hexdigest().encode(), False)
    for name, stored in [('empty', b''), ('short31', b'a'*31), ('nonhex', b'g'*32),
                         ('nul', b'\0'*32), ('badutf8', b'\xff'*32)]:
        add('md5_hash_' + name, 'md5', b'public-dummy', stored, False)
    add('md5_hash33', 'md5', b'', b'a'*33, 'unsupported_hash', False)
    for name, password, stored in VECTORS:
        add('phpass_' + name, 'phpass', password, stored.encode(), True)
        add('phpass_wrong_' + name, 'phpass', b'wrong-public-dummy', stored.encode(), False)
    add('phpass_no_trim', 'phpass', common.trim6(VECTORS[3][1]), VECTORS[3][2].encode(), False)
    good = VECTORS[1][2].encode()
    for name, stored in [('valid', good), ('badcost', good[:3]+b'!'+good[4:]),
                         ('binarybody', good[:4]+b'\xff'*30), ('highcost', good[:3]+b'S'+good[4:])]:
        add('phpass_over4097_' + name, 'phpass', b'x'*4097, stored, False, maximum=7)
    for name, stored, maximum, error in [
        ('host13', VECTORS[4][2].encode(), 7, 'cost_limit'),
        ('host30', good[:3]+b'S'+good[4:], 7, 'cost_limit'),
        ('host30_badbody', good[:3]+b'S'+b'\xff'*30, 7, 'cost_limit'),
        ('below7', good[:3]+b'4'+good[4:], 13, 'unsupported_hash'),
        ('cost31', good[:3]+b'T'+good[4:], 13, 'unsupported_hash'),
        ('cost_invalid', good[:3]+b'!'+good[4:], 13, 'unsupported_hash'),
        ('checksum_bits', good[:-1]+b'2', 13, 'unsupported_hash'),
        ('salt_punctuation', good[:4]+b'@'+good[5:], 13, 'unsupported_hash'),
        ('salt_binary', good[:4]+b'\xff'+good[5:], 13, 'unsupported_hash'),
        ('checksum_binary', good[:12]+b'\xff'+good[13:], 13, 'unsupported_hash'),
        ('prefix_h', b'$H$'+good[3:], 13, 'unsupported_hash'),
        ('short33', good[:-1], 13, 'unsupported_hash'),
        ('long35', good+b'.', 13, 'unsupported_hash')]:
        add('phpass_refuse_' + name, 'phpass', b'public-dummy', stored, error, False, maximum)
    return rows

def combined_cases():
    rows = []
    def add(name, password, stored, expected, actual=True, bcrypt=4, portable=7):
        rows.append({'id': 'combined_'+name, 'request': {'mode': 'combined', 'password_hex': password.hex(),
            'hash_hex': stored.hex(), 'bcrypt_maximum': bcrypt, 'portable_maximum': portable},
            'expected': expected, 'actual': actual})
    old = {case['id']: case for case in legacy_cases()}
    for name in ('md5_empty', 'md5_plain', 'md5_binary', 'md5_spaced', 'md5_max4096', 'md5_over4097',
                 'md5_max_fixture8192', 'md5_no_trim', 'md5_hash_short31', 'md5_hash_nul', 'md5_hash_badutf8', 'md5_hash33'):
        case = old[name]
        add(name, bytes.fromhex(case['request']['password_hex']), bytes.fromhex(case['request']['hash_hex']),
            case['expected'], case['actual'])
    for name, password in [('empty', b''), ('dummy', b'dummy-pass'), ('binary', b'dummy\0\xffpass'),
                           ('spaced', b' spaced dummy '), ('max4096', b'x'*4096)]:
        add('bcrypt_'+name, password, bcrypt_vectors.VECTORS[name].encode(), True)
    for name in ('dummy', 'binary'):
        add('bcrypt_wrong_'+name, b'wrong-public-dummy', bcrypt_vectors.VECTORS[name].encode(), False)
    add('bcrypt_no_trim', b'spaced dummy', bcrypt_vectors.VECTORS['spaced'].encode(), False)
    add('bcrypt_over4097', b'x'*4097, bcrypt_vectors.VECTORS['max4096'].encode(), False)
    for name, password, expected in [('cost12', b'dummy-pass', True), ('cost12_wrong', b'wrong-public-dummy', False)]:
        add('bcrypt_'+name, password, bcrypt_vectors.VECTORS['cost12'].encode(), expected, bcrypt=12)
    cost12 = bcrypt_vectors.VECTORS['cost12'].encode()
    add('bcrypt_cost_limit', b'dummy-pass', cost12, 'bcrypt_cost_limit', False)
    add('bcrypt_host_before_body', b'dummy-pass', cost12[:10]+b'!'*53, 'bcrypt_cost_limit', False)
    bad_bits = bcrypt_vectors.VECTORS['dummy'].encode()
    bad_bits = bad_bits[:31]+b'/'+bad_bits[32:]
    add('bcrypt_noncanonical', b'dummy-pass', bad_bits, 'unsupported_hash', False)
    for name, password, stored in VECTORS:
        add('portable_'+name, password, stored.encode(), True, portable=13 if name == 'default13' else 7)
    add('portable_wrong', b'wrong-public-dummy', VECTORS[3][2].encode(), False)
    add('portable_no_trim', common.trim6(VECTORS[3][1]), VECTORS[3][2].encode(), False)
    add('portable_cost_limit', b'public-dummy', VECTORS[4][2].encode(), 'portable_cost_limit', False)
    add('portable_host_before_body', b'public-dummy', b'$P$S'+b'\xff'*30, 'portable_cost_limit', False)
    for name, stored in [('unknown33', b'?'*33), ('binary33', b'\xff'*33),
                         ('wp_bad_envelope', b'$wp'+b'?'*30),
                         ('P_noncanonical', b'$P$!'+b'\xff'*30), ('H_unsupported', b'$H$'+b'.'*31)]:
        add(name+'_4096', b'x'*4096, stored, 'unsupported_hash', False)
        add(name+'_4097', b'x'*4097, stored, False)
    add('short_P_32_long', b'x'*4097, b'$P$'+b'.'*29, False)
    add('unknown32_4096', b'x'*4096, b'?'*32, False)
    add('unknown32_4097', b'x'*4097, b'?'*32, False)
    return rows

def cases():
    original = legacy_cases()
    if len(original) != 64 or hashlib.sha256(common.canonical(original)).hexdigest() != PROFILE['combined']['legacy64_sha256']:
        raise ValueError('original legacy64 corpus changed')
    if bcrypt_vectors.vectors_hash() != bcrypt_vectors.VECTORS_SHA256:
        raise ValueError('fixed public bcrypt vectors changed')
    return original + combined_cases()

def corpus_hash():
    return hashlib.sha256(common.canonical(cases())).hexdigest()

def tracked_files():
    return {*common.tracked_files(), 'scripts/wordpress_reference.py',
            'scripts/compare_legacy_passwords.py', 'scripts/check_legacy_password_evidence.py',
            'scripts/test_legacy_password_evidence.py', 'scripts/legacy_password_oracle.php', 'scripts/wp_bcrypt_profile.py',
            '.reference/wordpress-7.1.3/wp-includes/class-phpass.php'}

def binary_paths():
    return {'rust': common.binary_paths()['rust'].replace('password_generation_probe', 'legacy_password_probe'),
            'php': common.binary_paths()['php']}

def verify_pinned():
    common.verify_pinned()
    source = ROOT / '.reference/wordpress-7.1.3'
    with (source / 'recastpress-reference.json').open('rb') as stream:
        wire = stream.read(8*1024*1024+1)
    if len(wire) > 8*1024*1024: raise ValueError('reference manifest bound')
    manifest = json.loads(wire, object_pairs_hook=common.unique_object)
    hashes = {row['path']: row['sha256'] for row in manifest['files']}
    if common.digest(source/'wp-includes/class-phpass.php') != hashes['wp-includes/class-phpass.php']:
        raise ValueError('pinned phpass class differs')

def validate_record(record, verify_files=True):
    common.fields(record, ['format_version', 'scope', 'generated_at_utc', 'profile', 'corpus_sha256',
                          'runs', 'oracle', 'cases', 'tracked_files', 'binaries', 'limitations'])
    if not common.exact(record['format_version'], 1) or record['scope'] != 'public_dummy_legacy_password_verification':
        raise ValueError('legacy evidence scope differs')
    if not common.exact(record['profile'], PROFILE) or not common.exact(record['limitations'], LIMITATIONS) or record['corpus_sha256'] != corpus_hash():
        raise ValueError('legacy profile/corpus differs')
    stamp = record['generated_at_utc']
    if type(stamp) is not str or 'T' not in stamp or datetime.fromisoformat(stamp).utcoffset() != timedelta(0):
        raise ValueError('legacy timestamp is not UTC')
    common.fields(record['runs'], ['rust', 'php'])
    for run in record['runs'].values():
        common.fields(run, ['owned_pid', 'reported_pid', 'exit_code', 'stderr_empty'])
        common.positive_pid(run['owned_pid']); common.positive_pid(run['reported_pid'])
        if (not common.exact(run['owned_pid'], run['reported_pid']) or not common.exact(run['exit_code'], 0)
                or run['stderr_empty'] is not True): raise ValueError('legacy child execution failed')
    if not common.exact(record['oracle'], {'php_version': '8.5.12', 'globals_restored': True,
        'actions_unchanged': True, 'restoration_states': ['absent', 'null', 'sentinel', 'empty']}):
        raise ValueError('legacy actual context differs')
    expected = cases()
    if type(record['cases']) is not list or len(record['cases']) != len(expected): raise ValueError('legacy case coverage differs')
    for row, case in zip(record['cases'], expected, strict=True):
        common.fields(row, ['id', 'native', 'php'])
        if row['id'] != case['id']: raise ValueError('legacy case order differs')
        result = {'verified': case['expected']} if type(case['expected']) is bool else {'error': case['expected']}
        actual = {**result, 'actual_called': case['actual'], 'check_calls': int(case['actual'])}
        if not common.exact(row['native'], result) or not common.exact(row['php'], actual):
            raise ValueError('legacy result/refusal classification differs')
    common.fields(record['tracked_files'], tracked_files())
    for path, value in record['tracked_files'].items():
        if type(value) is not str or not re.fullmatch('[0-9a-f]{64}', value): raise ValueError('invalid source hash')
        if verify_files and value != common.digest(ROOT/path): raise ValueError('stale legacy source: '+path)
    common.fields(record['binaries'], binary_paths())
    for name, path in binary_paths().items():
        item = record['binaries'][name]; common.fields(item, ['path', 'sha256'])
        if item['path'] != path or type(item['sha256']) is not str or not re.fullmatch('[0-9a-f]{64}', item['sha256']):
            raise ValueError('legacy binary identity differs')
        if verify_files and item['sha256'] != common.digest(ROOT/path): raise ValueError('stale legacy binary')
    return record

def main():
    verify_pinned()
    validate_record(common.read_json(OUTPUT))
    print('Legacy password evidence matches current sources/binaries; finite public dummy subset only')

if __name__ == '__main__': main()
