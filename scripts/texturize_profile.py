"""Fixed English text-only wptexturize corpus and strict actual metadata."""
from collections import Counter
import random

NAME = 'valid_utf8_no_lt_default_english_empty_filters'
OLD_COUNT = 486153
OLD_INPUT_SHA256 = '2b7f4aa626a6e858e2c495315d631a459e83011c1bf752e719ac63c3b2388401'
OLD_OUTPUT_SHA256 = '64e411af8e55dd2f3e39bbd341ed37014e9ca6ee7f1b93cdc2127f5262467735'
LIMITS = {'max_input_bytes': 65536, 'max_work_bytes': 1048576, 'max_output_bytes': 65536}
PROFILE = {
    'php_version': '8.5.12', 'name': NAME,
    'input_type': 'bytes_whole_valid_utf8_no_literal_lt', 'reset': True,
    'translations': 'explicit_default_english_x_identity', 'shortcode_registry': 'empty',
    'cockney_override': 'unset', 'filter_registry': 'empty', 'ctype_locale': 'C',
    'native_filter_bookkeeping': False, 'spaces_cache': 'one_initialization_per_php_process',
    'fixture_limits': LIMITS,
    'source_sha256': '350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32',
}
COUNT_KEYS = ('calls', 'initialized', 'x_calls', 'run_hook', 'no_tags_hook',
              'no_shortcodes_hook', 'spaces_hook', 'restored')


def _exact(value, expected):
    if type(value) is not type(expected):
        return False
    if type(expected) is dict:
        return set(value) == set(expected) and all(_exact(value[key], item) for key, item in expected.items())
    return value == expected


def validate_profile(value):
    if not _exact(value, PROFILE):
        raise ValueError('Texturize profile differs')


def validate_counts(value, expected):
    for item in (value, expected):
        if type(item) is not dict or set(item) != set(COUNT_KEYS) or any(type(item[key]) is not int or item[key] < 0 for key in COUNT_KEYS):
            raise ValueError('Texturize actual counts differ')
    if value != expected:
        raise ValueError('Texturize actual counts differ')


def validate_previous(count, input_sha, rust_sha, php_sha):
    if (type(count) is not int or count != OLD_COUNT or type(input_sha) is not str or input_sha != OLD_INPUT_SHA256
            or type(rust_sha) is not str or rust_sha != OLD_OUTPUT_SHA256
            or type(php_sha) is not str or php_sha != OLD_OUTPUT_SHA256):
        raise ValueError('Previous API73 exact corpus/results changed')


def expected_counts(cases):
    selected = [case for case in cases if case['api'] == 'wptexturize']
    active = sum(bytes.fromhex(case['input']['hex']) not in (b'', b'0') for case in selected)
    return {'calls': len(selected), 'initialized': active, 'x_calls': active * 11,
            'run_hook': active, 'no_tags_hook': active, 'no_shortcodes_hook': active,
            'spaces_hook': int(active > 0), 'restored': len(selected)}


def aggregate_counts(batches):
    total = Counter(dict.fromkeys(COUNT_KEYS, 0))
    for batch in batches:
        total.update(expected_counts(batch))
    return dict(total)


def cases():
    raw_values = [b'', b'0', b'00', b' ', b'\t\r\n\0\x0b', b'\0', b'\x0c', b'\r\n', b'\n\n']
    cockney = [b"'tain't", b"'twere", b"'twas", b"'tis", b"'twill", b"'til", b"'bout", b"'nuff", b"'round", b"'cause", b"'em"]
    for token in [b'...', b'....', b'.....', b'``', b"''", b' (tm)', b'(tm)', *cockney]:
        raw_values.extend([token, b'A'+token+b'Z', token.upper(), token+b'\n', b"'"+token+b"'", token*3])

    prefixes = [b'', b' ', b'\t', b'\n', b'(', b'[', b'{', b'-', b'&lt;', b'&nbsp;', '\u00a0'.encode(), b'A', b'9']
    suffixes = [b'', b' ', b'\n', b'.', b',', b'!', b'?', b')', b']', b';', b'&gt;', '\u00a0'.encode(), b'&nbsp;']
    contents = [b'A', b'0', b'1', b'01', b'99', b'999', b'0.42', b'1,234.5', b'two words', b"don't", '日本'.encode(), 'e\u0301'.encode()]
    for quote in (b"'", b'"'):
        for prefix in prefixes:
            for content in contents:
                for suffix in suffixes:
                    raw_values.append(prefix+quote+content+quote+suffix)

    for value in range(101):
        number = str(value).encode()
        for tail in (b'', b"'", b'"', b's', b"'s", b'%', b'.0', b',5', b'\n'):
            raw_values.append(b"'"+number+tail)
    ambiguities = [b"7'.", b"7' 8'.", b"7'. 8'.", b"a' 7' 8'", b"7' a' 8'.", b"7'. 8' a'", b"7' 8' 9'", b"1'2'3'", b"0'0'", b"9'\n", b"'9'", b"'99'", b"'99\"", b"'0.42'", b'"0.42"']
    for raw in ambiguities:
        for quote in (b"'", b'"'):
            for start, end in ((b'', b''), (quote, quote), (b'He said '+quote, b'.'+quote), (b'(', b')\n')):
                raw_values.append(start+raw+end)

    for left in (b'', b'A', b' ', b'\n', b'xn', b'XN', b'axn', b'0', '\u00a0'.encode(), b'&nbsp;'):
        for dash in (b'-', b'--', b'---', b'----', b'-----'):
            for right in (b'', b'Z', b' ', b'\n'):
                raw_values.append(left+dash+right)
    for left in (b'0', b'00', b'1', b'9', b'1,234', b'0.42', b'A9', b'9A', '日9'.encode()):
        for right in (b'0', b'1', b'9999', b'1.2', b'1,234', b'3A', b'3\n'):
            raw_values.append(left+b'x'+right)
    for entity in (b'&', b'&&', b'&copy;', b'&COPY;', b'&amp;', b'&lt;', b'&gt;', b'&nbsp;', b'&a1;', b'&a4;', b'&a5;', b'&123;', b'&#0;', b'&#9999999;', b'&#xAB;', b'&#Xff;', b'&#x;', b'&abcdefgh;', b'&abcdefghi;', b'&copy', b'&;'):
        raw_values.extend([entity, b"'"+entity+b"'", b'"'+entity+b'"', entity+b'\n'])
    for byte in range(128):
        if byte == ord('<'):
            continue
        raw = bytes([byte])
        raw_values.extend([raw, b"A'"+raw+b"'B", b'9"'+raw+b'8"', b'A'+raw+b'--'+raw+b'B'])
    for space in ('\u00a0', '\u2003', '\u2028', '\u202f', '\u3000', 'é', '日本語', '🦀'):
        raw = space.encode()
        raw_values.extend([raw+b"'A'", b'"A"'+raw, b'A'+raw+b'--'+raw+b'B', b"'99"+raw, b"9'"+raw+b'8"'])
    for token in (b'&', b"'", b'"', b'-', b'.', b' ', b'\0'):
        raw_values.extend([token*256, token*4096])

    generator = random.Random(20261030074)
    palette = [b'A', b'z', b'0', b'1', b'9', b'99', b'0.42', b"'", b'"', b'``', b"''", b'...', b' (tm)', b' ', b'\t', b'\r', b'\n', b'\0', b'--', b'---', b'xn--', b'x', b'(', b')', b'[', b']', b'&', b'&lt;', b'&gt;', b'&nbsp;', b'&#8217;', b"'tis", b"'em", 'é'.encode(), '\u00a0'.encode(), '日本'.encode(), '🦀'.encode()]
    for _ in range(1536):
        raw = b''.join(generator.choice(palette) for _ in range(generator.randrange(1, 49)))
        raw_values.extend([raw, raw+b'\n'])
    result = []
    for raw in raw_values:
        if b'<' in raw or len(raw) > LIMITS['max_input_bytes']:
            raise ValueError('Texturize corpus escaped its finite profile')
        raw.decode('utf-8', errors='strict')
        result.append({'api': 'wptexturize', 'input': {'type': 'string', 'hex': raw.hex()},
                       'texturize_profile': NAME, 'reset': True, 'limits': dict(LIMITS)})
    return result
