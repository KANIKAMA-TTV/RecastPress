"""Conditional decoded-UTF8 admission and preceding corpus identity, not slug policy."""
from urllib.parse import unquote_to_bytes

OLD_COUNT=466887
OLD_INPUT_SHA256='398fa05860787a8671ec6b65334cfcae29a9ea21d163573fe5e54b165f51e033'
OLD_OUTPUT_SHA256='29ec1e7f530d41bb18d7b114dac9e52b6a1e75038bf2ded35b9aa1b619cd290a'
SOURCE_SHA256='34678856a4326b7e76ef120ba80a9e46c6903193e058b988555ec5c1f5cb8f26'
NAME='conditional_decoded_utf8_no_mbstring'
BRANCHES=('raw_no_truncate','raw_substr','decoded_uri')
PROFILE={'php_version':'8.5.12','input_type':'arbitrary_bytes_with_conditional_decoded_whole_utf8',
    'length_type':'integer_0_to_i64_max','decoded_utf8_guard':'only_strlen_gt_length_and_urldecode_changed',
    'decode':'php_urldecode_plus_space_single_percent_pass','uri_encode_ascii':True,'source_length_zero':'uri_unlimited_or_raw_substr_empty',
    'rtrim_bytes_hex':'2d','mbstring_loaded':False,'mb_internal_encoding_exists':False,'mb_strtolower_exists':False,
    'fixture_final_host_cap_bytes':65536,'source_sha256':SOURCE_SHA256,
    'uri_source_sha256':'350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32'}

def decoded(raw):return unquote_to_bytes(raw.replace(b'+',b' '))

def branch(raw,length):
    if len(raw)<=length:return'raw_no_truncate'
    return'raw_substr'if decoded(raw)==raw else'decoded_uri'

def admitted(raw,length):
    if branch(raw,length)!='decoded_uri':return True
    try:decoded(raw).decode('utf-8')
    except UnicodeDecodeError:return False
    return True
