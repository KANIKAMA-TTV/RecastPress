"""Prepared raw post / fixed default the_title callbacks, not an HTML sink."""
from collections import Counter
import hashlib
from pathlib import Path
import random
import re

import texturize_profile

ROOT = Path(__file__).resolve().parents[1]
NAME = 'prepared_raw_publish_frontend_default_english_no_html'
OLD_COUNT = 495336
OLD_INPUT_SHA256 = '3669eeb4f67b70e410fc92415b949882e941d6e5d6378d194412198ae285030c'
OLD_OUTPUT_SHA256 = '507fc386e149572f419b9d234112698ba0300e6d09ad6cba75f217aa3af54903'
LIMITS = dict(texturize_profile.LIMITS)
DEFAULT_FILTERS_SHA256 = '55bbf47fb835a0f64f28423d475b51f7fa72479f319fca417b4f23990d44f897'
CALLBACKS = [
    {'function': 'wptexturize', 'priority': 10, 'accepted_args': 1},
    {'function': 'convert_chars', 'priority': 10, 'accepted_args': 1},
    {'function': 'trim', 'priority': 10, 'accepted_args': 1},
    {'function': 'capital_P_dangit', 'priority': 11, 'accepted_args': 1},
]
PROFILE = {
    'php_version': '8.5.12', 'name': NAME,
    'input_type': 'bytes_whole_valid_utf8_no_literal_lt',
    'post_id_type': 'positive_signed64_integer', 'post_password_type': 'bytes_empty_or_ascii_zero',
    'post_status': 'publish', 'post_filter': 'raw', 'frontend': True,
    'translations': 'explicit_default_english_x_identity', 'shortcode_registry': 'empty',
    'cockney_override': 'unset', 'ctype_locale': 'C', 'native_filter_bookkeeping': False,
    'fixture_limits': LIMITS, 'callbacks': CALLBACKS, 'all_hook': False,
    'priming': {'raw': 'prime', 'reset': True, 'per_getter': True},
    'get_post_profile': 'actual_wp_post_filter_raw_same_object',
    'html_output': 'entity_syntax_not_literal_text',
    'default_filters_source_sha256': DEFAULT_FILTERS_SHA256,
    'source_sha256': '9cb15c0f7fc9bb52bd6d0211e544f84dd3b05e150b2a046c7cbdb3350e1eec70',
}
COUNT_KEYS = ('calls', 'the_title_hook', 'getter_x_calls', 'getter_run_hook',
              'getter_no_tags_hook', 'getter_no_shortcodes_hook', 'prime_calls',
              'prime_x_calls', 'prime_run_hook', 'prime_no_tags_hook',
              'prime_no_shortcodes_hook', 'prime_spaces_hook', 'registered_shape',
              'db_calls', 'cache_calls', 'restored')


def _exact(value, expected):
    if type(value) is not type(expected):
        return False
    if type(expected) is dict:
        return set(value) == set(expected) and all(_exact(value[key], item) for key, item in expected.items())
    if type(expected) is list:
        return len(value) == len(expected) and all(_exact(left, right) for left, right in zip(value, expected))
    return value == expected


def validate_profile(value):
    if not _exact(value, PROFILE):
        raise ValueError('Prepared title profile differs')


def validate_counts(value, expected):
    for item in (value, expected):
        if (type(item) is not dict or set(item) != set(COUNT_KEYS)
                or any(type(item[key]) is not int or item[key] < 0 for key in COUNT_KEYS)):
            raise ValueError('Prepared title actual counts differ')
    if value != expected:
        raise ValueError('Prepared title actual counts differ')


def validate_previous(count, input_sha, rust_sha, php_sha):
    if (type(count) is not int or count != OLD_COUNT or type(input_sha) is not str or input_sha != OLD_INPUT_SHA256
            or type(rust_sha) is not str or rust_sha != OLD_OUTPUT_SHA256
            or type(php_sha) is not str or php_sha != OLD_OUTPUT_SHA256):
        raise ValueError('Previous API74 exact corpus/results changed')


def check_callbacks(path=None):
    path = Path(path) if path is not None else ROOT / '.reference/wordpress-7.1.3/wp-includes/default-filters.php'
    with path.open('rb') as stream:
        wire = stream.read(1048577)
    if len(wire) > 1048576 or hashlib.sha256(wire).hexdigest() != DEFAULT_FILTERS_SHA256:
        raise ValueError('Pinned default title callback source differs')
    lines = wire.decode('utf-8').splitlines()
    direct = ["add_filter( 'the_title', 'wptexturize' );", "add_filter( 'the_title', 'convert_chars' );",
              "add_filter( 'the_title', 'trim' );"]
    if (lines[196:199] != direct
            or re.findall(r"^add_filter\( 'the_title', .*?\);$", wire.decode('utf-8'), re.M) != direct
            or lines[169:172] != ["foreach ( array( 'the_content', 'the_title', 'wp_title', 'document_title' ) as $filter ) {",
                                    "\tadd_filter( $filter, 'capital_P_dangit', 11 );", '}']):
        raise ValueError('Pinned default title callback tuples differ')
    return CALLBACKS


def expected_counts(cases):
    counts = dict.fromkeys(COUNT_KEYS, 0)
    spaces_initialized = False
    for case in cases:
        api = case['api']
        if api == 'wptexturize':
            if bytes.fromhex(case['input']['hex']) not in (b'', b'0'):
                spaces_initialized = True
            continue
        if api != 'get_the_title':
            continue
        for key in ('calls', 'the_title_hook', 'prime_calls', 'prime_run_hook',
                    'prime_no_tags_hook', 'prime_no_shortcodes_hook', 'registered_shape', 'restored'):
            counts[key] += 1
        counts['prime_x_calls'] += 11
        if not spaces_initialized:
            counts['prime_spaces_hook'] += 1
            spaces_initialized = True
        if bytes.fromhex(case['input']['hex']) not in (b'', b'0'):
            counts['getter_no_tags_hook'] += 1
            counts['getter_no_shortcodes_hook'] += 1
    return counts


def aggregate_counts(batches):
    total = Counter(dict.fromkeys(COUNT_KEYS, 0))
    for batch in batches:
        total.update(expected_counts(batch))
    return dict(total)


def fixture(raw, *, post_id=1, password=b''):
    return {'api': 'get_the_title', 'input': {'type': 'string', 'hex': raw.hex()},
            'post_id': {'type': 'int', 'value': str(post_id)},
            'post_password': {'type': 'string', 'hex': password.hex()},
            'post_status': {'type': 'string', 'hex': b'publish'.hex()},
            'post_filter': 'raw', 'frontend': True, 'title_profile': NAME,
            'limits': dict(LIMITS)}


def cases():
    # Reuse the old text-token corpus verbatim without changing API74's list.
    result = [fixture(bytes.fromhex(case['input']['hex']),
                      post_id=(1, 713, 2**63 - 1)[index % 3],
                      password=(b'', b'0')[index % 2])
              for index, case in enumerate(texturize_profile.cases())]
    boundaries = [b'', b'0', b'Wordpress', b'XWordpressY', b'WordpressWordpress',
                  b'WORDPRESS wordpress WordPress wordPress', b'&', b'&&', b'&&amp;',
                  b'&&a;', b'&a;&b;', b'&#;', b'&#x;', b'&amp;', b'&copy;',
                  b'\0\t\n\x0b\r Wordpress \0\t\n\x0b\r', b'\x0cWordpress\x0c',
                  '\u00a0Wordpress\u00a0'.encode(), b'&nbsp;Wordpress&nbsp;',
                  b"'Wordpress' -- \"Wordpress\" ... (tm)"]
    for length in (1, 2, 8, 9, 10):
        for letter in (b'a', b'A', b'1', b'4', b'5'):
            entity = b'&' + letter * length + b';'
            boundaries.extend([entity, b'&' + entity, entity + b'&', entity + entity])
    for byte in range(128):
        if byte != ord('<'):
            token = bytes([byte])
            boundaries.extend([token + b'Wordpress' + token, b'&' + token + b'amp;',
                               b'&a' + token + b';'])
    for raw in boundaries:
        for post_id in (1, 713, 2**63 - 1):
            for password in (b'', b'0'):
                result.append(fixture(raw, post_id=post_id, password=password))
    generator = random.Random(20261030075)
    palette = [b'Wordpress', b'XWordpressY', b'wordpress', b'WORDPRESS', b'&', b'&&',
               b'&a;', b'&abcdefghi;', b'&amp;', b'&#;', b' ', b'\0', b'\x0c', b'\x0b',
               b'\t', b'\n', b'\r', b'--', b'...', b'"', b"'", b'9', b'&nbsp;',
               '\u00a0'.encode(), '日本語'.encode(), '🦀'.encode()]
    for index in range(768):
        raw = b''.join(generator.choice(palette) for _ in range(generator.randrange(1, 49)))
        result.append(fixture(raw, post_id=(1, 713, 2**63 - 1)[index % 3],
                              password=(b'', b'0')[index % 2]))
    result.extend([fixture(b'A' * 65536), fixture(b' ' * 65536),
                   fixture(b'Wordpress' * 4096), fixture(b'&a;' * 1024)])
    for case in result:
        raw = bytes.fromhex(case['input']['hex'])
        if len(raw) > LIMITS['max_input_bytes'] or b'<' in raw:
            raise ValueError('Prepared title corpus escaped finite profile')
        raw.decode('utf-8', errors='strict')
    return result
