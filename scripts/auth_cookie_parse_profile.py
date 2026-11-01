"""Explicit byte-cookie parser only; no verification or environment selection."""
from collections import Counter
import random

NAME = 'explicit_php_truthy_cookie_arbitrary_byte_scheme'
LIMITS = {'max_input_bytes': 65536}
OLD_COUNT = 510813
OLD_INPUT_SHA256 = '0c38646ec3bd6639180c3bf9e067be352c96fe0fe3c7d00c4b7e0ba1a5a570b6'
OLD_OUTPUT_SHA256 = 'bc0fe2a7de0a0712812cc10ac213b714dc03427f86315d4a38283700dd8a1dd6'
FIELDS = ('username', 'expiration', 'token', 'hmac', 'scheme')
PROFILE = {
    'php_version': '8.5.12', 'name': NAME,
    'cookie': 'explicit_php_truthy_bytes_empty_and_ascii_zero_excluded',
    'scheme': 'arbitrary_bytes_preserved_even_empty_unknown_or_binary',
    'input_cap': 'checked_sum_cookie_and_raw_scheme', 'fixture_limits': LIMITS,
    'parse': 'exactly_three_literal_pipe_bytes_or_false',
    'fields': list(FIELDS), 'field_values': 'raw_bytes_in_source_order_without_validation',
    'environment_cookie_selection': 'outside_profile_ssl_and_cookie_access_tripwires',
    'hooks': 'none_in_source', 'session_or_hmac_verification': False,
    'source_sha256': '3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777',
}
COUNT_KEYS = ('calls', 'parsed', 'false_results', 'ssl_calls', 'cookie_accesses', 'restored')


def string(raw): return {'type': 'string', 'hex': raw.hex()}


def fixture(cookie=b'user|raw-expiry|token|not-a-valid-hmac', scheme=b'auth'):
    return {'api': 'wp_parse_auth_cookie', 'input': string(cookie), 'scheme': string(scheme),
            'explicit_cookie': True, 'auth_cookie_parse_profile': NAME, 'limits': dict(LIMITS)}


def expected_counts(cases):
    values = [case for case in cases if case['api'] == 'wp_parse_auth_cookie']
    parsed = sum(bytes.fromhex(case['input']['hex']).count(b'|') == 3 for case in values)
    return {'calls': len(values), 'parsed': parsed, 'false_results': len(values)-parsed,
            'ssl_calls': 0, 'cookie_accesses': 0, 'restored': len(values)}


def exact(value, expected):
    if type(value) is not type(expected): return False
    if type(expected) is dict:
        return set(value) == set(expected) and all(exact(value[key], item) for key, item in expected.items())
    if type(expected) is list:
        return len(value) == len(expected) and all(exact(a,b) for a,b in zip(value,expected))
    return value == expected


def validate_profile(value):
    if not exact(value, PROFILE): raise ValueError('Explicit cookie parser profile differs')


def validate_counts(value, expected):
    for item in (value, expected):
        if (type(item) is not dict or set(item) != set(COUNT_KEYS)
                or any(type(item[key]) is not int or item[key] < 0 for key in COUNT_KEYS)):
            raise ValueError('Explicit cookie parser counts differ')
    if value != expected: raise ValueError('Explicit cookie parser counts differ')


def validate_previous(count, input_sha, rust_sha, php_sha):
    if (type(count) is not int or count != OLD_COUNT or type(input_sha) is not str or input_sha != OLD_INPUT_SHA256
            or type(rust_sha) is not str or rust_sha != OLD_OUTPUT_SHA256
            or type(php_sha) is not str or php_sha != OLD_OUTPUT_SHA256):
        raise ValueError('Previous API76 exact corpus/results changed')


def aggregate_counts(batches):
    total=Counter(dict.fromkeys(COUNT_KEYS,0))
    for batch in batches: total.update(expected_counts(batch))
    return dict(total)


def cases():
    result=[]
    for cookie in [b'|||',b'a|b|c|d',b'0|0|0|0',b'user|not-an-integer||',
                   b'|+001||not-a-hmac',b'u|-9223372036854775809|t|h',
                   b'\0\xff|\r\n|\xff\0|\0',b'public',b'|',b'||',b'||||',b'a|b|c|d|e']:
        for scheme in [b'',b'auth',b'secure_auth',b'logged_in',b'unknown',b'0',b'a|b',b'\0\xff',b'\r\n']:
            result.append(fixture(cookie,scheme))
    for field in range(4):
        for byte in range(256):
            pieces=[b'u',b'e',b't',b'h'];pieces[field]=bytes([byte])
            result.append(fixture(b'|'.join(pieces),bytes([255-byte])))
    for byte in range(256): result.append(fixture(b'|||',bytes([byte])))
    for count in range(1,65): result.append(fixture(b'|'*count,b'unknown'))
    generator=random.Random(20261030077)
    palette=[b'u',b'0',b'|',b'||',b'\0',b'\xff',b'\r\n',b' ',b'999999999999999999999',b'&amp;', '日本'.encode()]
    for _ in range(1024):
        cookie=b''.join(generator.choice(palette)for _ in range(generator.randrange(1,24)))
        if cookie in (b'',b'0'): cookie=b'no-pipes'
        scheme=b''.join(generator.choice(palette)for _ in range(generator.randrange(5)))
        result.append(fixture(cookie,scheme))
    result.extend([fixture(b'u'*65533+b'|||',b''),fixture(b'no-pipes',b's'*65528),fixture(b'|'*65536,b'')])
    for case in result:
        raw=bytes.fromhex(case['input']['hex']);scheme=bytes.fromhex(case['scheme']['hex'])
        if raw in (b'',b'0') or len(raw)+len(scheme)>LIMITS['max_input_bytes']:
            raise ValueError('Explicit cookie parser corpus escaped profile')
    return result
