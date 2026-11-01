"""Pinned data-only default callback, explicit locale/UTF8 profile and old70 identity."""
import hashlib
from pathlib import Path
import re
import export_remove_accents as accent_data

OLD_COUNT=461522
OLD_INPUT_SHA256='271c455b22c11690d8c57799be7701a52ca6cefe3295190298885176a1fcca2e'
OLD_OUTPUT_SHA256='61745186306527a8839d50a473c42caa1519117ddc23760aa4da05ab89d38a10'
DEFAULT_FILTERS_SHA256='55bbf47fb835a0f64f28423d475b51f7fa72479f319fca417b4f23990d44f897'
NAME='valid_utf8_no_lt_explicit_locale_default_title_callback'
ROOT=Path(__file__).resolve().parents[1]
CALLBACK=('sanitize_title','sanitize_title_with_dashes',10,3)
PROFILE={'php_version':'8.5.12','input_type':'bytes_no_literal_lt_whole_valid_utf8','fallback_type':'arbitrary_bytes_unmodified',
    'contexts':['save','display','query'],'locale_type':'bytes_php_truthy_all_contexts',
    'locale_resolution':'scoped_global_first_isset_branch','ctype_locale':'C','mbstring_loaded':False,
    'mb_internal_encoding_exists':False,'mb_strtolower_exists':False,'intl_loaded':False,
    'normalizer_is_normalized_exists':False,'normalizer_normalize_exists':False,'fixture_final_host_cap_bytes':65536,
    'default_filters_source_sha256':DEFAULT_FILTERS_SHA256,'callback_hook':CALLBACK[0],'callback_function':CALLBACK[1],
    'callback_priority':10,'callback_accepted_args':3,'callback_count':1,'all_hook':False,
    'fallback_condition':'empty_string_or_false_not_php_empty','native_filter_bookkeeping':False,**accent_data.profile()}

def check_callback(path=None):
    path=path or ROOT/'.reference/wordpress-7.1.3/wp-includes/default-filters.php'
    with Path(path).open('rb')as stream:wire=stream.read(1048577)
    if len(wire)>1048576 or hashlib.sha256(wire).hexdigest()!=DEFAULT_FILTERS_SHA256:
        raise ValueError('Pinned default callback source differs')
    lines=wire.decode('utf-8').splitlines()
    expected="add_filter( 'sanitize_title', 'sanitize_title_with_dashes', 10, 3 );"
    matches=re.findall(r"^add_filter\( 'sanitize_title', .*?\);$",wire.decode('utf-8'),re.M)
    if len(lines)<309 or lines[308]!=expected or matches!=[expected]:
        raise ValueError('Pinned default sanitize_title callback tuple differs')
    return CALLBACK

def locale_calls(raw,context):return int(context=='save'and any(byte>=128 for byte in raw))
