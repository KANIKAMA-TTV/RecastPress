"""Finite, caller-prepared cached single-meta fixture; no DB/cache-miss policy."""
import random

NAME = 'complete_cached_single_empty_filters'
OLD_COUNT = 482951
OLD_INPUT_SHA256 = '710f479a08eabf482709af5c57552786c21d17ffb27e5d2c3ff4dce6a7d8a293'
OLD_OUTPUT_SHA256 = 'ed5f18b5f0ac9b710fae3187271a573cdd0d6a76dad093f1c680f01cc5aa765a'
LIMITS = {'max_input_bytes': 65536, 'max_array_entries': 1024,
          'max_owned_string_bytes': 65536, 'max_array_depth': 64}
PROFILE = {
    'php_version': '8.5.12', 'name': NAME,
    'object_id': 'typed_positive_signed_i64', 'meta_key': 'php_truthy_arbitrary_bytes',
    'cache': 'caller_prepared_complete_nonempty_php_array',
    'cache_values': 'null_or_nonempty_dense_lists_of_raw_null_or_bytes',
    'single': True, 'complete_cache': True, 'empty_filters': True,
    'selected_decode': 'first_value_only_one_layer_canonical_modeled_subset',
    'canonical_admission': 'php_serialize_typed_witness_equals_php_trim_selected_raw',
    'missing_or_null': 'source_default_empty_bytes',
    'native_filter_bookkeeping': False,
    'cache_miss': 'outside_profile', 'database_calls': 0,
    'fixture_limits': LIMITS,
    'meta_source_sha256': 'b1b49d1d3ddbf10d0f17286f53c30ce9582c636a4679f9a0ef92ee5ebb020a3e',
    'post_source_sha256': '34678856a4326b7e76ef120ba80a9e46c6903193e058b988555ec5c1f5cb8f26',
}
COUNT_KEYS = ('calls', 'get_hook', 'default_hook', 'cache_get', 'cache_update', 'db_calls', 'restored')


def _exact(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(_exact(actual[key], value) for key, value in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(_exact(a, b) for a, b in zip(actual, expected))
    return actual == expected


def validate_profile(value):
    if not _exact(value, PROFILE):
        raise ValueError('Cached post-meta profile differs')


def validate_counts(value, expected):
    if any(type(item) is not dict or set(item) != set(COUNT_KEYS) or any(type(item[key]) is not int or item[key] < 0 for key in COUNT_KEYS) for item in (value, expected)) or value != expected:
        raise ValueError('Cached post-meta actual counts differ')


def string(raw):
    return {'type': 'string', 'hex': raw.hex()}


def integer(value):
    return {'type': 'int', 'value': str(value)}


def key(raw):
    try:
        value = int(raw.decode('ascii'))
        if -(2**63) <= value < 2**63 and str(value).encode() == raw:
            return integer(value)
    except (ValueError, UnicodeError):
        pass
    return string(raw)


def array(entries):
    return {'type': 'array', 'entries': [{'key': k, 'value': v} for k, v in entries]}


def dense(values):
    return array([(integer(index), value) for index, value in enumerate(values)])


def serialize_witness(value):
    """Build canonical public fixture bytes, never an oracle return value."""
    kind = value['type']
    if kind == 'null':
        return b'N;'
    if kind == 'bool':
        return b'b:1;' if value['value'] else b'b:0;'
    if kind == 'int':
        return b'i:' + value['value'].encode('ascii') + b';'
    if kind == 'string':
        raw = bytes.fromhex(value['hex'])
        return b's:' + str(len(raw)).encode() + b':"' + raw + b'";'
    if kind == 'array':
        return b'a:' + str(len(value['entries'])).encode() + b':{' + b''.join(serialize_witness(entry['key']) + serialize_witness(entry['value']) for entry in value['entries']) + b'}'
    raise ValueError('Unsupported witness fixture type')


_MISSING = object()


def fixture(meta_key, first=_MISSING, *, witness=_MISSING, owner=1, later=True, outer_null=False):
    if not meta_key or meta_key == b'0':
        raise ValueError('Falsey key is outside cached single fixture')
    entries = []
    if outer_null:
        entries.append((key(meta_key), {'type': 'null'}))
    elif first is not _MISSING:
        values = [{'type': 'null'} if first is None else string(first)]
        if later:
            values.append(string(b'O:1:"X":0:{}'))  # Unselected: no decode/witness.
        entries.append((key(meta_key), dense(values)))
    unused = b'__unselected_public_fixture__'
    if unused == meta_key:
        unused += b'2'
    entries.append((key(unused), dense([string(b'O:1:"X":0:{}')])))
    result = {'api': 'get_post_meta', 'input': integer(owner), 'meta_key': string(meta_key),
              'cache': array(entries), 'single': True, 'complete_cache': True,
              'empty_filters': True, 'post_meta_profile': NAME, 'limits': dict(LIMITS)}
    if witness is not _MISSING:
        result['witness'] = witness
    return result


def default_calls(case):
    requested = key(bytes.fromhex(case['meta_key']['hex']))
    for entry in case['cache']['entries']:
        if entry['key'] == requested:
            if entry['value']['type'] == 'null':
                return 1
            first = entry['value']['entries'][0]['value']
            return int(first['type'] == 'null' or case.get('witness', {}).get('type') == 'null')
    return 1


def expected_counts(cases):
    selected = [case for case in cases if case['api'] == 'get_post_meta']
    total = len(selected)
    return {'calls': total, 'get_hook': total, 'default_hook': sum(default_calls(case) for case in selected),
            'cache_get': total, 'cache_update': 0, 'db_calls': 0, 'restored': total}


def cases():
    result = []
    named_keys = [b'_wp_attachment_image_alt', b'_thumbnail_id', b'_wp_page_template',
                  b'subtitle', b'Case', b'case', b'key\0tail', b'\xffkey']
    plain = [b'', b'0', b'plain', b' \tvalue\r\n', b'\0', b'\xff\0', '日本語 🦀'.encode(),
             b'<script>public_fixture()</script>', b'&copy;', b'R:1;']
    for requested in named_keys:
        for owner in (1, 713, 2**63 - 1):
            result.extend([fixture(requested, owner=owner), fixture(requested, None, owner=owner), fixture(requested, owner=owner, outer_null=True)])
            result.extend(fixture(requested, raw, owner=owner) for raw in plain)
    for byte in range(256):
        for requested in named_keys[:4]:
            result.append(fixture(requested, bytes([byte])))
        if byte != ord('0'):
            for first in (_MISSING, None, b'', b'public'):
                result.append(fixture(bytes([byte]), first))
    numeric = [b'-9223372036854775808', b'9223372036854775807', b'9223372036854775808',
               b'01', b'+1', b'-0', b'00', b'1', b' 1', b'1 ']
    for requested in numeric:
        result.extend([fixture(requested), fixture(requested, None), fixture(requested, b'number-key')])
    witnesses = [{'type': 'null'}, {'type': 'bool', 'value': False}, {'type': 'bool', 'value': True},
                 *[integer(value) for value in (-(2**63), -1, 0, 1, 2**63 - 1)],
                 *[string(raw) for raw in (b'', b'0', b'N;', b'b:0;', b'a:0:{}', b'a\0b\xff', '日本 🦀'.encode())],
                 array([]), array([(integer(0), {'type': 'null'})]),
                 array([(key(b'01'), string(b'a\0b')), (integer(-(2**63)), {'type': 'bool', 'value': False}), (key(b'\0\xff'), integer(2**63 - 1))])]
    for witness in witnesses:
        for requested in named_keys[:4] + numeric[:2]:
            for leading, trailing in ((b'', b''), (b' \t', b'\r\n'), (b'\0\x0b', b'\0'), (b'\n', b' ')):
                result.append(fixture(requested, leading + serialize_witness(witness) + trailing, witness=witness))
    generator = random.Random(20261029073)
    for index in range(192):
        value = generator.choice(witnesses[:15])
        for depth in range(generator.randrange(1, 9)):
            value = array([(integer(0), value), (key(f'public-{depth}'.encode()), string(bytes([index % 256])))])
        for requested in named_keys[:2]:
            result.append(fixture(requested, serialize_witness(value), witness=value))
    return result
