"""Explicit prepared-user/resolved-salt cookie contents, no session adoption."""
from collections import Counter
import random

NAME = 'prepared_existing_user_resolved_salt_truthy_token_empty_filters'
LIMITS = {'max_input_bytes': 65536, 'max_output_bytes': 65536}
SCHEMES = ('auth', 'secure_auth', 'logged_in')
OLD_COUNT = 508297
OLD_INPUT_SHA256 = '0c917b6c57f06f48e6ab9e79567b3c95cb26fffdff8a162c8abf222a371cda5b'
OLD_OUTPUT_SHA256 = '00aceb3a9d50ed9b140065203ba4bdc20dcadc76e3aadd5ecffa37ef52641089'
PROFILE = {
    'php_version': '8.5.12', 'name': NAME,
    'user_lookup': 'scoped_existing_record_get_userdata_fixture',
    'user_id': 'typed_positive_signed64_integer', 'user_login': 'arbitrary_bytes',
    'password_hash': 'arbitrary_bytes_fragment_only_no_verification',
    'expiration': 'typed_signed64_decimal_no_clock_validation',
    'token': 'php_truthy_bytes_empty_and_ascii_zero_excluded',
    'salt': 'caller_resolved_nonempty_bytes_no_resolver', 'schemes': list(SCHEMES),
    'scheme_scope': 'oracle_salt_selection_witness_not_native_policy',
    'pass_fragment': 'literal_P_or_2y_offset8_len4_else_last4_bytes',
    'inner_hmac': 'md5_lowercase_hex_ascii_key', 'outer_hmac': 'sha256_lowercase_hex',
    'filter_registry': 'one_empty_auth_cookie_wp_hook_no_all_callbacks',
    'filter_arguments': 5, 'filter_arguments_basis': 'pinned_source_without_observer_callback',
    'hash_calls_basis': 'actual_wp_hash_salt_fixture_entry_not_builtin_hmac_instrumentation',
    'session_generation': 'outside_profile_tripwire', 'native_filter_bookkeeping': False,
    'input_sum_fields': ['user_login', 'password_hash', 'resolved_salt', 'token'],
    'fixture_limits': LIMITS,
    'source_sha256': '3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777',
}
COUNT_KEYS = ('calls', 'user_lookup_calls', 'hash_salt_calls', 'auth_cookie_hook',
              'session_calls', 'db_calls', 'cache_calls', 'restored')


def exact(value, expected):
    if type(value) is not type(expected):
        return False
    if type(expected) is dict:
        return set(value) == set(expected) and all(exact(value[key], item) for key, item in expected.items())
    if type(expected) is list:
        return len(value) == len(expected) and all(exact(left, right) for left, right in zip(value, expected))
    return value == expected


def validate_profile(value):
    if not exact(value, PROFILE):
        raise ValueError('Auth cookie profile differs')


def validate_counts(value, expected):
    for item in (value, expected):
        if (type(item) is not dict or set(item) != set(COUNT_KEYS)
                or any(type(item[key]) is not int or item[key] < 0 for key in COUNT_KEYS)):
            raise ValueError('Auth cookie actual counts differ')
    if value != expected:
        raise ValueError('Auth cookie actual counts differ')


def validate_previous(count, input_sha, rust_sha, php_sha):
    if (type(count) is not int or count != OLD_COUNT or type(input_sha) is not str or input_sha != OLD_INPUT_SHA256
            or type(rust_sha) is not str or rust_sha != OLD_OUTPUT_SHA256
            or type(php_sha) is not str or php_sha != OLD_OUTPUT_SHA256):
        raise ValueError('Previous API75 exact corpus/results changed')


def expected_counts(cases):
    count = sum(case['api'] == 'wp_generate_auth_cookie' for case in cases)
    return {key: count if key in ('calls', 'user_lookup_calls', 'hash_salt_calls',
                                 'auth_cookie_hook', 'restored') else 0 for key in COUNT_KEYS}


def aggregate_counts(batches):
    total = Counter(dict.fromkeys(COUNT_KEYS, 0))
    for batch in batches:
        total.update(expected_counts(batch))
    return dict(total)


def string(raw):
    return {'type': 'string', 'hex': raw.hex()}


def integer(value):
    return {'type': 'int', 'value': str(value)}


def fixture(login=b'public-user', password_hash=b'public-hash', token=b'public-token',
            salt=b'public-resolved-salt', *, user_id=1, expiration=1700000000, scheme='auth'):
    return {'api': 'wp_generate_auth_cookie', 'input': string(login),
            'user_id': integer(user_id), 'password_hash': string(password_hash),
            'expiration': integer(expiration), 'token': string(token), 'resolved_salt': string(salt),
            'scheme': scheme, 'empty_filters': True, 'auth_cookie_profile': NAME,
            'limits': dict(LIMITS)}


def cases():
    result = []
    hashes = [b'', b'0', b'abc', b'abcd', b'abcde', b'0123456789abcdef',
              b'$P$', b'$P$0123', b'$P$01234', b'$P$012345', b'$P$01234567',
              b'$P$0123456789abcdef', b'$2y$', b'$2y$0123456789abcdef',
              b'$wp$2y$12$publicnotcanonical', b'$p$0123456789abcdef',
              b'$2Y$0123456789abcdef', b'$2a$0123456789abcdef', b'\0\xff|abc',
              '日本語hash🦀'.encode()]
    for raw_hash in hashes:
        for expiration in (-(2**63), -1, 0, 1, 1700000000, 2**63-1):
            for scheme in SCHEMES:
                result.append(fixture(password_hash=raw_hash, expiration=expiration, scheme=scheme,
                                      user_id=(1, 713, 2**63-1)[len(result) % 3]))
    for field in ('login', 'password_hash', 'token', 'salt'):
        for byte in range(256):
            raw = bytes([byte])
            if field == 'token' and raw == b'0':
                raw = b'00'
            result.append(fixture(**{field: raw}, expiration=byte-128,
                                  scheme=SCHEMES[byte % 3], user_id=(1, 713, 2**63-1)[byte % 3]))
    for length in (1, 2, 3, 4, 7, 8, 9, 11, 12, 13, 15, 16, 31, 32, 63, 64, 65, 127, 128, 129, 1024, 4096):
        for field in ('login', 'password_hash', 'token', 'salt'):
            result.append(fixture(**{field: b'x'*length}))
    for login, token in [(b'', b't'), (b'0', b'00'), (b' a\t\r\n\v\f\0', b'|'),
                         (b'a|b', b'c|d'), (b'\xff\xc0\0', b'\0\xff'),
                         ('日本語🦀'.encode(), 'é🦀'.encode())]:
        for scheme in SCHEMES:
            result.append(fixture(login, token=token, salt=b'\0\xffsalt', scheme=scheme))
    generator = random.Random(20261030076)
    palette = [b'public', b'0', b'00', b'|', b'\0', b'\xff', b'\xc0', b' ', b'\t', b'\r\n',
               b'$P$', b'$2y$', b'$wp$2y$', '日本'.encode(), '🦀'.encode()]
    for index in range(1024):
        values = [b''.join(generator.choice(palette) for _ in range(generator.randrange(12))) for _ in range(4)]
        login, raw_hash, token, salt = values
        if token in (b'', b'0'): token = b'public-token'
        if not salt: salt = b'public-salt'
        result.append(fixture(login, raw_hash, token, salt,
                              expiration=generator.randrange(-(2**63), 2**63),
                              user_id=(1, 713, 2**63-1)[index % 3], scheme=SCHEMES[index % 3]))
    # Exact final cap and exact combined input cap are different contracts.
    result.append(fixture(b'x'*(65536-69), b'h', b't', b's', expiration=0))
    result.append(fixture(b'', b'h'*65534, b't', b's', expiration=0))
    for case in result:
        values = [bytes.fromhex(case[field]['hex']) for field in ('input','password_hash','token','resolved_salt')]
        if sum(map(len, values)) > LIMITS['max_input_bytes'] or values[2] in (b'',b'0') or not values[3]:
            raise ValueError('Auth cookie corpus escaped its explicit input profile')
        output = len(values[0]) + len(str(int(case['expiration']['value']))) + len(values[2]) + 67
        if output > LIMITS['max_output_bytes']:
            raise ValueError('Auth cookie corpus exceeds fixture final output bound')
    return result
