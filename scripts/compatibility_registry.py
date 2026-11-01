"""Validate source provenance and print the tracked API subset, without coverage claims."""

import argparse
from functools import lru_cache
import json
from pathlib import Path
import re

import wordpress_reference as reference
import wordpress_entity_names as entity_names
import wordpress_xml_entity_mapping as xml_entities
import wp_bcrypt_profile as wp_bcrypt
import wp_rehash_profile as wp_rehash
import export_remove_accents as accent_data
import remove_accents_profile as accent_profile
import user_sanitization_profile as user_profile
import slug_byte_profile as slug_profile
import sanitize_title_profile as default_title_profile
import truncate_slug_profile as truncate_profile
import post_meta_cached_profile as post_meta_profile
import texturize_profile
import title_profile
import auth_cookie_profile
import auth_cookie_parse_profile
import sanitize_text_profile

REGISTRY = reference.ROOT / "compatibility" / "registry.json"


PRIMITIVE_SCOPE = "isolated_function_subsets_with_real_wp_filter_dispatch"
HOOK_SCOPE = "request_local_WP_Hook_subset"
DISPATCHER_SCOPE = "named_dispatcher_subset"
UNIQUE_ID_STATE_SCOPE = "one_php_process_and_one_native_context_per_input_batch"
JSON_ENCODE_FLAGS = {"JSON_HEX_TAG": 1, "JSON_HEX_AMP": 2, "JSON_HEX_APOS": 4,
                     "JSON_HEX_QUOT": 8, "JSON_FORCE_OBJECT": 16, "JSON_PRETTY_PRINT": 128,
                     "JSON_UNESCAPED_SLASHES": 64, "JSON_UNESCAPED_UNICODE": 256,
                     "JSON_UNESCAPED_LINE_TERMINATORS": 2048}
JSON_UTF8_FALLBACK_PROFILE = {"blog_charset": "UTF-8",
                              "mb_convert_encoding_exists": False,
                              "is_utf8_charset": True}
HR_BYTE_UNITS = {"KB_IN_BYTES": 1024, "MB_IN_BYTES": 1048576, "GB_IN_BYTES": 1073741824}
HR_BYTE_MAX_SIGNIFICANT_DIGITS = 256
ENTITY_DECODE_PROFILE = {"input_type": "bytes", "max_input_bytes": 1048576,
                         "max_reference_value": 255, "order": "decimal_then_hex"}
PROTOCOL_ONCE_PROFILE = {"input_type": "bytes", "allowed_values": "bytes_only", "ctype_locale": "C",
                         "default_count": 1, "top_input_max_bytes": 1048576, "max_frames": 3}
PROTOCOL_OUTER_PROFILE = {"input_type": "bytes", "allowed_values": "bytes_only", "ctype_locale": "C",
                          "top_input_max_bytes": 1048576, "global_no_null_passes": 1, "max_once_passes": 6,
                          "once_default_count": 1, "once_max_frames": 3,
                          "fastpath": "literal_lower_http_https_strict_values", "internal_top_guard": False}
SPECIALCHARS_STYLES = {"noquotes": 0, "compat": 2, "quotes": 3,
                      "single": "single", "double": "double", "xml1": 16}
SPECIALCHARS_PROFILE = {"input_type": "bytes", "charset": "UTF-8", "default_charset_during_call": "UTF-8",
                       "php_version": "8.5.12", "quote_noquotes": 0, "quote_compat": 2, "quote_quotes": 3,
                       "quote_single": "single", "quote_double": "double", "quote_xml1": 16,
                       "double_encode_type": "explicit_boolean", "utf8_check": "original_special_guard_then_empty",
                       "source_sha256": "350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32",
                       "entity_source_sha256": entity_names.SOURCE_SHA256,
                       "ordered_names_sha256": entity_names.ORDERED_NAMES_SHA256,
                       "xml_mapping_sha256": xml_entities.MAPPING_SHA256}
EMPTY_FILTER_ESCAPING_PROFILE = {"input_type": "bytes", "blog_charset": "UTF-8",
    "option_profile": "immutable_UTF-8_for_process", "mb_convert_encoding_exists": False,
    "filter_registry": "empty", "native_filter_bookkeeping": False,
    "html_hook": "esc_html", "attr_hook": "attribute_escape", "quote_style": 3,
    "double_encode": False, "utf8_check": "whole_input_strip_false_before_special_guard",
    "default_charset_during_call": "UTF-8", "source_sha256": SPECIALCHARS_PROFILE["source_sha256"],
    "entity_source_sha256": entity_names.SOURCE_SHA256, "ordered_names_sha256": entity_names.ORDERED_NAMES_SHA256}
HOOK_SOURCES = {"wp-includes/plugin.php", "wp-includes/class-wp-hook.php", "wp-includes/class-wp-filter-sentinel.php"}


def validate_json_encode_flags(flags):
    if (not isinstance(flags, dict) or set(flags) != set(JSON_ENCODE_FLAGS)
            or any(type(flags[key]) is not int or flags[key] != value
                   for key, value in JSON_ENCODE_FLAGS.items())):
        raise ValueError("JSON encoding flag constants differ in identity, type or value")


def validate_hr_byte_units(units):
    if (not isinstance(units, dict) or set(units) != set(HR_BYTE_UNITS)
            or any(type(units[key]) is not int or units[key] != value
                   for key, value in HR_BYTE_UNITS.items())):
        raise ValueError("Byte conversion default unit profile differs in identity, type or value")


def validate_hr_byte_domain(max_significant_digits):
    if type(max_significant_digits) is not int or max_significant_digits != HR_BYTE_MAX_SIGNIFICANT_DIGITS:
        raise ValueError("Byte conversion significant digit domain differs in type or value")


def validate_default_html_entity_profile(profile):
    expected = entity_names.profile()
    if (not isinstance(profile, dict) or set(profile) != set(expected)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in expected.items())):
        raise ValueError("Default HTML entity profile differs in identity, type or value")


def validate_xml_entity_profile(profile):
    expected = xml_entities.profile()
    if (not isinstance(profile, dict) or set(profile) != set(expected)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in expected.items())):
        raise ValueError("XML UTF-8 entity profile differs in identity, type or value")


def validate_entity_profile_counts(counts, total):
    if (not isinstance(counts, dict) or set(counts) != {"html", "xml"}
            or type(total) is not int or total <= 0
            or any(type(value) is not int or value <= 0 for value in counts.values())
            or sum(counts.values()) != total):
        raise ValueError("Entity context profile counts are incomplete or inconsistent")


def validate_specialchars_profile(profile):
    if (not isinstance(profile, dict) or set(profile) != set(SPECIALCHARS_PROFILE)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in SPECIALCHARS_PROFILE.items())):
        raise ValueError("Specialchars UTF-8 profile differs in identity, type or value")


def validate_specialchars_matrix(counts, total):
    expected = {style + ":" + double for style in SPECIALCHARS_STYLES for double in ("false", "true")}
    if (not isinstance(counts, dict) or set(counts) != expected
            or type(total) is not int or total <= 0
            or any(type(value) is not int or value <= 0 for value in counts.values())
            or sum(counts.values()) != total):
        raise ValueError("Specialchars style/double-encoding matrix is incomplete or inconsistent")


def validate_empty_filter_escaping_profile(profile):
    if (not isinstance(profile, dict) or set(profile) != set(EMPTY_FILTER_ESCAPING_PROFILE)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in EMPTY_FILTER_ESCAPING_PROFILE.items())):
        raise ValueError("Empty-filter escaping profile differs in identity, type or value")


def validate_empty_filter_hook_counts(observed, counts):
    if not isinstance(counts, dict):
        raise ValueError("Empty-filter escaping API counts are missing")
    expected = {"esc_html": counts.get("esc_html", 0), "attribute_escape": counts.get("esc_attr", 0)}
    if (not isinstance(observed, dict) or set(observed) != set(expected)
            or any(type(value) is not int or value < 0 for value in expected.values())
            or any(type(observed[key]) is not int or observed[key] != value for key, value in expected.items())):
        raise ValueError("Empty-filter escaping hook counts differ from API case counts")


def validate_strip_all_tags_profile(profile):
    expected=user_profile.STRIP_PROFILE
    if (not isinstance(profile,dict) or set(profile)!=set(expected)
        or any(type(profile[key]) is not type(value) or profile[key]!=value for key,value in expected.items())):
        raise ValueError('NoLiteral< strip profile differs')

def validate_slug_profile(profile,title=False):
    expected=slug_profile.TITLE_PROFILE if title else slug_profile.URI_PROFILE
    if (not isinstance(profile,dict)or set(profile)!=set(expected)
        or any(type(profile[key])is not type(value)or profile[key]!=value for key,value in expected.items())):
        raise ValueError('Explicit URI/title noMbstring profile differs')

def validate_slug_coverage(calls,cells,total,title=False):
    validate_strip_calls(calls,total)
    keys={'display','save'}if title else {'zero:false','zero:true','positive:false','positive:true'}
    if (type(total)is not int or total<=0 or not isinstance(cells,dict)or set(cells)!=keys
        or any(type(value)is not int or value<=0 for value in cells.values())or sum(cells.values())!=total):
        raise ValueError('URI/title option coverage differs')

def validate_previous68(input_sha,output_sha):
    if input_sha!=slug_profile.OLD_INPUT_SHA256 or output_sha!=slug_profile.OLD_OUTPUT_SHA256:
        raise ValueError('Previous API68 corpus/result identity differs')

def validate_default_title_profile(profile):
    expected=default_title_profile.PROFILE
    if(not isinstance(profile,dict)or set(profile)!=set(expected)
        or any(type(profile[key])is not type(value)or profile[key]!=value for key,value in expected.items())):
        raise ValueError('Default sanitize_title explicit callback/locale/UTF8 profile differs')

def validate_truncate_slug_profile(profile):
    expected=truncate_profile.PROFILE
    if(not isinstance(profile,dict)or set(profile)!=set(expected)
        or any(type(profile[key])is not type(value)or profile[key]!=value for key,value in expected.items())):
        raise ValueError('Conditional truncate slug byte/UTF8/noMbstring profile differs')

def validate_truncate_slug_calls(calls,branches,total):
    validate_strip_calls(calls,total)
    if(not isinstance(branches,dict)or set(branches)!=set(truncate_profile.BRANCHES)
        or any(type(value)is not int or value<0 for value in branches.values())or sum(branches.values())!=total):
        raise ValueError('Truncate slug actual branch counts differ')

def validate_truncate_slug_coverage(calls,branches,expected_branches,input_sha,output_sha,total):
    validate_truncate_slug_calls(calls,branches,total)
    if(type(total)is not int or total<=0 or any(value<=0 for value in branches.values())
        or not isinstance(expected_branches,dict)or set(expected_branches)!=set(truncate_profile.BRANCHES)
        or any(type(value)is not int or value<=0 for value in expected_branches.values())or expected_branches!=branches
        or input_sha!=truncate_profile.OLD_INPUT_SHA256 or output_sha!=truncate_profile.OLD_OUTPUT_SHA256):
        raise ValueError('Truncate slug three-branch coverage/old71 identity differs')

def validate_post_meta_cached_coverage(profile, counts, expected_counts, total,
                                      previous_count, input_sha, rust_sha, php_sha):
    post_meta_profile.validate_profile(profile)
    cases = post_meta_profile.cases()
    expected = post_meta_profile.expected_counts(cases)
    if type(total) is not int or total != len(cases) or total <= 0:
        raise ValueError("Cached post-meta finite coverage differs")
    post_meta_profile.validate_counts(counts, expected)
    post_meta_profile.validate_counts(expected_counts, expected)
    if (type(previous_count) is not int or previous_count != post_meta_profile.OLD_COUNT
            or type(input_sha) is not str or input_sha != post_meta_profile.OLD_INPUT_SHA256
            or type(rust_sha) is not str or rust_sha != post_meta_profile.OLD_OUTPUT_SHA256
            or type(php_sha) is not str or php_sha != post_meta_profile.OLD_OUTPUT_SHA256):
        raise ValueError("Previous 72 API exact input/results differ")


@lru_cache(maxsize=1)
def _texturize_expected_coverage():
    # Regenerate mixed-API byte-bounded batches: spaces cache is process-local.
    import compare_primitives
    from collections import Counter
    counts = Counter()
    for batch in compare_primitives.batches(compare_primitives.cases()):
        counts.update(texturize_profile.expected_counts(batch))
    return dict(counts)


def validate_texturize_coverage(profile, counts, expected_counts, total,
                                previous_count, input_sha, rust_sha, php_sha):
    texturize_profile.validate_profile(profile)
    expected = _texturize_expected_coverage()
    if type(total) is not int or total != len(texturize_profile.cases()) or total <= 0:
        raise ValueError('Texturize finite coverage differs')
    texturize_profile.validate_counts(counts, expected)
    texturize_profile.validate_counts(expected_counts, expected)
    if (type(previous_count) is not int or previous_count != texturize_profile.OLD_COUNT
            or type(input_sha) is not str or input_sha != texturize_profile.OLD_INPUT_SHA256
            or type(rust_sha) is not str or rust_sha != texturize_profile.OLD_OUTPUT_SHA256
            or type(php_sha) is not str or php_sha != texturize_profile.OLD_OUTPUT_SHA256):
        raise ValueError('Previous 73 API exact input/results differ')


@lru_cache(maxsize=1)
def _title_expected_coverage():
    import compare_primitives
    from collections import Counter
    counts = Counter()
    for batch in compare_primitives.batches(compare_primitives.cases()):
        counts.update(title_profile.expected_counts(batch))
    return dict(counts)


def validate_title_coverage(profile, counts, expected_counts, total,
                            previous_count, input_sha, rust_sha, php_sha):
    title_profile.validate_profile(profile)
    expected = _title_expected_coverage()
    if type(total) is not int or total != len(title_profile.cases()) or total <= 0:
        raise ValueError('Prepared title finite coverage differs')
    title_profile.validate_counts(counts, expected)
    title_profile.validate_counts(expected_counts, expected)
    title_profile.validate_previous(previous_count, input_sha, rust_sha, php_sha)


def validate_auth_cookie_coverage(profile, counts, expected_counts, total,
                                  previous_count, input_sha, rust_sha, php_sha):
    auth_cookie_profile.validate_profile(profile)
    cases = auth_cookie_profile.cases()
    if type(total) is not int or total != len(cases) or total <= 0:
        raise ValueError('Auth cookie finite coverage differs')
    # These counters are additive per case; unlike texturize there is no shared
    # process initialization. Never trust an artifact's expected counters alone.
    expected = auth_cookie_profile.expected_counts(cases)
    auth_cookie_profile.validate_counts(counts, expected)
    auth_cookie_profile.validate_counts(expected_counts, expected)
    auth_cookie_profile.validate_previous(previous_count, input_sha, rust_sha, php_sha)


def validate_auth_cookie_parse_coverage(profile, counts, expected_counts, total,
                                        previous_count, input_sha, rust_sha, php_sha):
    auth_cookie_parse_profile.validate_profile(profile)
    cases = auth_cookie_parse_profile.cases()
    if type(total) is not int or total != len(cases) or total <= 0:
        raise ValueError('Explicit cookie parser finite coverage differs')
    expected = auth_cookie_parse_profile.expected_counts(cases)
    auth_cookie_parse_profile.validate_counts(counts, expected)
    auth_cookie_parse_profile.validate_counts(expected_counts, expected)
    auth_cookie_parse_profile.validate_previous(previous_count, input_sha, rust_sha, php_sha)


def validate_sanitize_text_coverage(profile, counts, expected_counts, total,
                                    previous_count, input_sha, rust_sha, php_sha):
    sanitize_text_profile.validate_profile(profile)
    cases = sanitize_text_profile.cases()
    if type(total) is not int or total != len(cases) or total <= 0:
        raise ValueError('Text sanitizer finite coverage differs')
    expected = sanitize_text_profile.expected_counts(cases)
    sanitize_text_profile.validate_counts(counts, expected)
    sanitize_text_profile.validate_counts(expected_counts, expected)
    sanitize_text_profile.validate_previous(previous_count, input_sha, rust_sha, php_sha)


def validate_default_title_calls(calls,filters,locale_calls,restored,total):
    validate_strip_calls(calls,total)
    if(type(filters)is not int or filters!=total or type(restored)is not int or restored!=total
        or not isinstance(locale_calls,dict)or set(locale_calls)!={'zero','one'}
        or any(type(value)is not int or value<0 for value in locale_calls.values())or sum(locale_calls.values())!=total):
        raise ValueError('Default title actual filter/locale/restoration counts differ')

def validate_default_title_coverage(calls,filters,locale_calls,expected_locale,restored,contexts,input_sha,output_sha,total):
    validate_default_title_calls(calls,filters,locale_calls,restored,total)
    if(not isinstance(contexts,dict)or set(contexts)!={'save','display','query'}or any(type(value)is not int or value<=0 for value in contexts.values())or sum(contexts.values())!=total
        or not isinstance(expected_locale,dict)or set(expected_locale)!={'zero','one'}
        or any(type(value)is not int or value<=0 for value in expected_locale.values())or expected_locale!=locale_calls
        or input_sha!=default_title_profile.OLD_INPUT_SHA256 or output_sha!=default_title_profile.OLD_OUTPUT_SHA256):
        raise ValueError('Default title coverage/conditional locale/old70 identity differs')

def validate_sanitize_user_profile(profile):
    expected=user_profile.USER_PROFILE
    if (not isinstance(profile,dict) or set(profile)!=set(expected)
        or any(type(profile[key]) is not type(value) or profile[key]!=value for key,value in expected.items())):
        raise ValueError('NoLiteral< user resolved locale/UTF8/C/empty-filter profile differs')

def validate_strip_calls(calls,total):
    if type(total) is not int or total<0 or type(calls) is not int or calls!=total:raise ValueError('Strip actual calls differ')

def validate_boolean_coverage(counts,total):
    if (type(total) is not int or total<=0 or not isinstance(counts,dict) or set(counts)!={'true','false'}
        or any(type(value) is not int or value<=0 for value in counts.values()) or sum(counts.values())!=total):
        raise ValueError('Boolean option coverage differs')

def validate_sanitize_user_calls(calls,filters,histogram,db_calls,total):
    validate_strip_calls(calls,total)
    if (type(filters) is not int or filters!=total or type(db_calls) is not int or db_calls!=0
        or not isinstance(histogram,dict) or set(histogram)!={'zero','one'}
        or any(type(value) is not int or value<0 for value in histogram.values()) or sum(histogram.values())!=total):
        raise ValueError('User actual locale/filter calls or forbidden DB branch differ')

def validate_sanitize_user_coverage(calls,filters,histogram,expected_histogram,db_calls,flags,previous_hash,total):
    validate_sanitize_user_calls(calls,filters,histogram,db_calls,total)
    validate_boolean_coverage(flags,total)
    if (not isinstance(expected_histogram,dict) or set(expected_histogram)!={'zero','one'}
        or any(type(value) is not int or value<=0 for value in expected_histogram.values())
        or expected_histogram!=histogram or previous_hash!=user_profile.ACCENT_INPUT_SHA256):
        raise ValueError('User conditional locale coverage or old accent corpus identity differs')

def validate_remove_accents_profile(profile):
    expected=accent_profile.PROFILE
    if (not isinstance(profile,dict) or set(profile)!=set(expected)
        or any(type(profile[key]) is not type(value) or profile[key]!=value for key,value in expected.items())):
        raise ValueError('Accent explicit locale/no-Intl profile differs')

def validate_remove_accents_calls(calls,locale_calls,total):
    if type(total) is not int or total<0 or type(calls) is not int or calls!=total or type(locale_calls) is not int or locale_calls!=0:
        raise ValueError('Accent actual calls/get_locale counters differ')

def validate_remove_accents_coverage(branches,calls,locale_calls,total):
    validate_remove_accents_calls(calls,locale_calls,total)
    if (total<=0 or not isinstance(branches,dict) or set(branches)!=set(accent_profile.BRANCHES)
        or any(type(value) is not int or value<=0 for value in branches.values()) or sum(branches.values())!=total):
        raise ValueError('Accent explicit locale branch coverage differs')

def validate_wp_rehash_profile(profile):
    expected = wp_rehash.PROFILE
    if (not isinstance(profile, dict) or set(profile)!=set(expected)
        or any(type(profile[key]) is not type(value) or profile[key]!=value for key,value in expected.items())):
        raise ValueError('WP rehash profile differs in identity, type or value')

def validate_wp_rehash_hooks(observed, total):
    if (type(total) is not int or total<0 or not isinstance(observed, dict) or set(observed)!=set(wp_rehash.HOOKS)
        or any(type(value) is not int or value!=total for value in observed.values())):
        raise ValueError('WP rehash three actual hook counts differ from case count')

def validate_wp_rehash_coverage(kinds, costs, hooks, total):
    validate_wp_rehash_hooks(hooks, total)
    if (total<=0 or not isinstance(kinds, dict) or set(kinds)!={'nonprefixed','canonical_prefixed'}
        or any(type(value) is not int or value<=0 for value in kinds.values()) or sum(kinds.values())!=total
        or not isinstance(costs, dict) or set(costs)!={str(cost) for cost in range(4,32)}
        or any(type(value) is not int or value<=0 for value in costs.values())
        or sum(costs.values())!=kinds['canonical_prefixed']):
        raise ValueError('WP rehash conditional domain/cost coverage differs')

def validate_wp_bcrypt_profile(profile):
    if wp_bcrypt.vectors_hash() != wp_bcrypt.VECTORS_SHA256:
        raise ValueError("WP bcrypt dummy vectors differ")
    expected = wp_bcrypt.PROFILE
    if (not isinstance(profile, dict) or set(profile) != set(expected)
            or any(type(profile[key]) is not type(value) or profile[key] != value for key, value in expected.items())):
        raise ValueError("WP bcrypt verify profile differs in identity, type or value")


def validate_wp_bcrypt_coverage(costs, maximums, lengths, calls, total):
    if type(total) is not int or total <= 0 or type(calls) is not int or calls != total:
        raise ValueError("WP bcrypt hook calls differ from case count")
    for counts, keys in ((costs, {"4", "12"}), (maximums, {"4", "12"}), (lengths, {"within_4096", "over_4096"})):
        if (not isinstance(counts, dict) or set(counts) != keys
                or any(type(value) is not int or value <= 0 for value in counts.values()) or sum(counts.values()) != total):
            raise ValueError("WP bcrypt cost/length coverage is incomplete or inconsistent")


def validate_entity_decode_profile(profile):
    if (not isinstance(profile, dict) or set(profile) != set(ENTITY_DECODE_PROFILE)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in ENTITY_DECODE_PROFILE.items())):
        raise ValueError("Byte entity decoder profile differs in identity, type or value")


def validate_protocol_once_profile(profile):
    if (not isinstance(profile, dict) or set(profile) != set(PROTOCOL_ONCE_PROFILE)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in PROTOCOL_ONCE_PROFILE.items())):
        raise ValueError("Protocol-once profile differs in identity, type or value")


def validate_protocol_outer_profile(profile):
    if (not isinstance(profile, dict) or set(profile) != set(PROTOCOL_OUTER_PROFILE)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in PROTOCOL_OUTER_PROFILE.items())):
        raise ValueError("Protocol-outer profile differs in identity, type or value")


def validate_json_utf8_fallback_profile(profile):
    if (not isinstance(profile, dict) or set(profile) != set(JSON_UTF8_FALLBACK_PROFILE)
            or any(type(profile[key]) is not type(value) or profile[key] != value
                   for key, value in JSON_UTF8_FALLBACK_PROFILE.items())):
        raise ValueError("JSON UTF-8 fallback profile was not verified")


def validate_json_profile_cases(counts, total):
    if (not isinstance(counts, dict) or set(counts) != {"first_encode_only", "utf8_no_mbstring"}
            or type(total) is not int or total <= 0
            or any(type(value) is not int or value <= 0 for value in counts.values())
            or sum(counts.values()) != total):
        raise ValueError("JSON profile counts are incomplete or inconsistent")


def validate_json_flag_cases(counts, total):
    bits = list(JSON_ENCODE_FLAGS.values())
    expected = {str(sum(flag for offset, flag in enumerate(bits) if choice & (1 << offset)))
                for choice in range(1 << len(bits))}
    if (not isinstance(counts, dict) or set(counts) != expected
            or type(total) is not int or total <= 0
            or any(type(value) is not int or value <= 0 for value in counts.values())
            or sum(counts.values()) != total):
        raise ValueError("JSON encoding flag combination counts are incomplete or inconsistent")


def evidence_files(scope):
    """Required identities come from the checker, never from the evidence itself."""
    probes = {
        PRIMITIVE_SCOPE: ("compat_probe", "wp_oracle", "compare_primitives"),
        HOOK_SCOPE: ("hook_probe", "wp_hook_oracle", "compare_hooks"),
        DISPATCHER_SCOPE: ("dispatcher_probe", "wp_dispatcher_oracle", "compare_dispatcher"),
    }
    if scope not in probes:
        raise ValueError("comparison evidence scope/limitations missing")
    probe, oracle, comparison = probes[scope]
    implementation = {path.relative_to(reference.ROOT).as_posix()
                      for folder in ("src", "tests")
                      for path in (reference.ROOT / "crates/recastpress-wp" / folder).rglob("*.rs")}
    implementation.update({f"crates/recastpress-wp/examples/{probe}.rs", "crates/recastpress-wp/examples/common/mod.rs"})
    harness = {"Cargo.toml", "Cargo.lock", "crates/recastpress-wp/Cargo.toml",
               "scripts/php_value_codec.php", "scripts/reference_functions.php",
               "scripts/wordpress_reference.py", "scripts/compare_primitives.py",
               "scripts/compatibility_registry.py", "compatibility/registry.json",
               "scripts/wordpress_entity_names.py", entity_names.ARTIFACT,
               "scripts/wordpress_xml_entity_mapping.py", "scripts/wordpress_xml_entity_mapping.php", xml_entities.ARTIFACT,
               "scripts/test_xml_entity_oracle.py",
               "scripts/test_specialchars_oracle.py",
               "scripts/test_empty_filter_escaping_oracle.py",
               "scripts/wp_bcrypt_profile.py", "scripts/test_wp_bcrypt_oracle.py",
               "scripts/wp_rehash_profile.py", "scripts/test_wp_rehash_oracle.py",
               "scripts/export_remove_accents.py", "scripts/remove_accents_profile.py", "scripts/test_remove_accents.py", accent_data.ARTIFACT,
               "scripts/user_sanitization_profile.py", "scripts/test_user_sanitization.py",
               "scripts/slug_byte_profile.py", "scripts/test_slug_bytes.py",
               "scripts/sanitize_title_profile.py", "scripts/test_sanitize_title_profile.py",
               "scripts/truncate_slug_profile.py", "scripts/test_truncate_slug_profile.py",
               "scripts/title_profile.py", "scripts/title_oracle_profile.php",
               "scripts/auth_cookie_profile.py", "scripts/auth_cookie_oracle_profile.php", "scripts/test_auth_cookie_oracle.py",
               "scripts/auth_cookie_parse_profile.py", "scripts/auth_cookie_parse_oracle_profile.php", "scripts/test_auth_cookie_parse_oracle.py",
               "scripts/sanitize_text_profile.py", "scripts/sanitize_text_oracle_profile.php", "scripts/test_sanitize_text_oracle.py",
               "scripts/test_title_profile.py", "scripts/test_title_oracle.py",
               "scripts/test_title_publication.py", "crates/recastpress-wp/examples/title_publication_fixture.rs",
               "scripts/texturize_profile.py", "scripts/texturize_oracle_profile.php",
               "scripts/test_texturize_profile.py", "scripts/test_texturize_oracle.py",
               "scripts/post_meta_cached_profile.py", "scripts/post_meta_oracle_profile.php",
               "scripts/test_post_meta_cached_profile.py", "scripts/test_post_meta_oracle.py",
               f"scripts/{oracle}.php", f"scripts/{comparison}.py"}
    if scope == PRIMITIVE_SCOPE:
        return {"implementation_files": implementation, "harness_files": harness}
    return {"tracked_files": implementation | harness}


def evidence_sources(scope, registry, relative):
    sources = set(HOOK_SOURCES)
    if scope == PRIMITIVE_SCOPE:
        for api in registry["apis"]:
            if api.get("differential_evidence") == relative:
                sources.add(api["source_path"])
                sources.update(api.get("source_dependencies", {}))
    return sources


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate evidence JSON key: {key}")
        result[key] = value
    return result


def validate(registry, check_evidence=True):
    if any(api.get('wordpress_symbol')=='remove_accents'for api in registry['apis']): accent_data.check()
    if any(api.get("wordpress_symbol") == "wp_kses_normalize_entities" for api in registry["apis"]):
        entity_names.check()
        xml_entities.check()
    if registry["format_version"] != 1 or registry["wordpress_version"] != reference.VERSION:
        raise ValueError("registry format/baseline mismatch")
    archive = reference.REFERENCE / f"wordpress-{reference.VERSION}.tar.gz"
    if reference.digest_file(archive, "sha1") != reference.EXPECTED_SHA1:
        raise ValueError("reference archive differs from the pinned baseline")
    if reference.digest_file(archive) != registry["archive_sha256"]:
        raise ValueError("registry archive SHA-256 mismatch")
    source_root = reference.REFERENCE / f"wordpress-{reference.VERSION}"
    identifiers = set()
    for api in registry["apis"]:
        name = api["wordpress_symbol"]
        if name in identifiers:
            raise ValueError(f"duplicate registry symbol: {name}")
        identifiers.add(name)
        if api["status"] != "source_derived":
            raise ValueError(f"unsupported evidence status: {api['status']}; differential evidence workflow is not implemented")
        if not api["limitations"]:
            raise ValueError(f"source-derived API must disclose limitations: {name}")
        source = source_root / reference.safe_relative("wordpress/" + api["source_path"])
        if reference.digest_file(source) != api["source_sha256"]:
            raise ValueError(f"reference source changed: {api['source_path']}")
        for dependency, expected in api.get("source_dependencies", {}).items():
            path = source_root / reference.safe_relative("wordpress/" + dependency)
            if reference.digest_file(path) != expected:
                raise ValueError(f"reference dependency changed: {dependency}")
        lines = source.read_text(encoding="utf-8").splitlines()
        line = api["source_line"]
        if line < 1 or line > len(lines) or not re.search(r"\bfunction\s+" + re.escape(name) + r"\s*\(", lines[line - 1]):
            raise ValueError(f"source symbol/line mismatch: {name}")
        for field in ["rust_source", "test_source"]:
            path = (reference.ROOT / api[field]).resolve()
            if not path.is_relative_to(reference.ROOT.resolve()) or not path.is_file():
                raise ValueError(f"invalid implementation path: {api[field]}")
        rust = (reference.ROOT / api["rust_source"]).read_text(encoding="utf-8")
        if not re.search(r"\bpub fn\s+" + re.escape(api["rust_symbol"]) + r"\b", rust):
            raise ValueError(f"Rust symbol missing: {api['rust_symbol']}")
        tests = (reference.ROOT / api["test_source"]).read_text(encoding="utf-8")
        if not api["tests"] or any(not re.search(r"\bfn\s+" + re.escape(test) + r"\s*\(", tests) for test in api["tests"]):
            raise ValueError(f"registered contract test missing: {name}")
        for contract in api.get("profile_contracts", []):
            for field in ("rust_source", "test_source"):
                path = (reference.ROOT / contract[field]).resolve()
                if not path.is_relative_to(reference.ROOT.resolve()) or not path.is_file():
                    raise ValueError("invalid profile implementation path")
            rust = (reference.ROOT / contract["rust_source"]).read_text(encoding="utf-8")
            tests = (reference.ROOT / contract["test_source"]).read_text(encoding="utf-8")
            if (not re.search(r"\bpub fn\s+" + re.escape(contract["rust_symbol"]) + r"\b", rust)
                    or not contract["tests"] or any(not re.search(r"\bfn\s+" + re.escape(test) + r"\s*\(", tests) for test in contract["tests"])):
                raise ValueError("profile implementation or contract test missing")
    for component in registry.get("components", []):
        source = source_root / reference.safe_relative("wordpress/" + component["source_path"])
        if reference.digest_file(source) != component["source_sha256"]:
            raise ValueError(f"reference component changed: {component['source_path']}")
        lines = source.read_text(encoding="utf-8").splitlines()
        class_name = component.get("wordpress_class")
        class_line = component["source_line"]
        if class_line < 1 or class_line > len(lines):
            raise ValueError("reference component line is invalid")
        if class_name and not re.search(r"\bclass\s+" + re.escape(class_name) + r"\b", lines[class_line - 1]):
            raise ValueError("reference class declaration mismatch")
        for name, line in component["source_methods"].items():
            if line < 1 or line > len(lines) or not re.search(r"\bfunction\s+" + re.escape(name) + r"\s*\(", lines[line - 1]):
                raise ValueError(f"reference method declaration mismatch: {name}")
        for field in ["rust_source", "test_source"]:
            path = (reference.ROOT / component[field]).resolve()
            if not path.is_relative_to(reference.ROOT.resolve()) or not path.is_file():
                raise ValueError("invalid component implementation path")
        rust = (reference.ROOT / component["rust_source"]).read_text(encoding="utf-8")
        tests = (reference.ROOT / component["test_source"]).read_text(encoding="utf-8")
        if component["status"] != "source_derived" or not component["limitations"]:
            raise ValueError("component scope/limitations are missing")
        if any(not re.search(r"\bpub fn\s+" + re.escape(name) + r"\b", rust) for name in component["rust_methods"]):
            raise ValueError("component Rust method missing")
        if not component["tests"] or any(not re.search(r"\bfn\s+" + re.escape(name) + r"\s*\(", tests) for name in component["tests"]):
            raise ValueError("component contract test missing")
    if check_evidence:
        records = [*registry["apis"], *registry.get("components", [])]
        for path in {record["differential_evidence"] for record in records if "differential_evidence" in record}:
            validate_evidence(path, registry)
    return registry["apis"]


def validate_evidence(relative, registry):
    path = (reference.ROOT / relative).resolve()
    if not path.is_relative_to(reference.ROOT.resolve()):
        raise ValueError("evidence path escaped workspace")
    evidence = json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_object)
    if type(evidence["format_version"]) is not int or evidence["format_version"] != 1 or evidence["wordpress_version"] != registry["wordpress_version"] or evidence["archive_sha256"] != registry["archive_sha256"]:
        raise ValueError("comparison evidence baseline mismatch")
    scope = evidence["scope"]
    if type(evidence["mismatches"]) is not int or evidence["mismatches"] != 0 or type(evidence["case_count"]) is not int or evidence["case_count"] <= 0:
        raise ValueError("comparison evidence does not record a complete successful run")
    if scope == "isolated_function_subsets_with_real_wp_filter_dispatch":
        if evidence["jsonp_ctype_locale"] != "C":
            raise ValueError("JSONP oracle C locale was not verified")
        if evidence["php_mbstring_loaded"] is not False or evidence["utf8_oracle_path"] != "no_mbstring_fallback":
            raise ValueError("UTF-8 oracle path was not verified as the no-mbstring fallback")
        if evidence["php_parse_str_settings"] != {"arg_separator.input": "&", "max_input_vars": "1000", "max_input_nesting_level": "64"}:
            raise ValueError("query oracle settings were not verified")
        counts = evidence["case_counts_by_api"]
        expected = {api["wordpress_symbol"] for api in registry["apis"] if api.get("differential_evidence") == relative}
        if "wp_kses_normalize_entities" in expected:
            validate_default_html_entity_profile(evidence.get("php_default_html_entity_profile"))
            validate_xml_entity_profile(evidence.get("php_xml_utf8_entity_profile"))
            validate_entity_profile_counts(evidence.get("entity_context_case_counts"), counts.get("wp_kses_normalize_entities"))
        if "_wp_specialchars" in expected:
            validate_specialchars_profile(evidence.get("php_specialchars_utf8_profile"))
            validate_specialchars_matrix(evidence.get("specialchars_style_double_case_counts"), counts.get("_wp_specialchars"))
        if expected & {"esc_html", "esc_attr"}:
            validate_empty_filter_escaping_profile(evidence.get("php_empty_filter_escaping_profile"))
            validate_empty_filter_hook_counts(evidence.get("empty_filter_escaping_hook_counts"), counts)
        if "wp_check_password" in expected:
            validate_wp_bcrypt_profile(evidence.get("php_wp_bcrypt_profile"))
            validate_wp_bcrypt_coverage(evidence.get("wp_bcrypt_hash_cost_case_counts"), evidence.get("wp_bcrypt_host_max_cost_case_counts"),
                evidence.get("wp_bcrypt_length_case_counts"), evidence.get("wp_bcrypt_check_password_calls"), counts.get("wp_check_password"))
        if 'wp_password_needs_rehash' in expected:
            validate_wp_rehash_profile(evidence.get('php_wp_rehash_profile'))
            validate_wp_rehash_coverage(evidence.get('wp_rehash_case_kind_counts'), evidence.get('wp_rehash_cost_case_counts'),
                evidence.get('wp_rehash_hook_counts'), counts.get('wp_password_needs_rehash'))
        if 'remove_accents' in expected:
            validate_remove_accents_profile(evidence.get('php_remove_accents_profile'))
            validate_remove_accents_coverage(evidence.get('remove_accents_locale_branch_case_counts'),evidence.get('remove_accents_reference_calls'),
                evidence.get('remove_accents_get_locale_calls'),counts.get('remove_accents'))
        if 'wp_strip_all_tags' in expected:
            validate_strip_all_tags_profile(evidence.get('php_strip_all_tags_profile'))
            validate_strip_calls(evidence.get('strip_all_tags_reference_calls'),counts.get('wp_strip_all_tags'))
            validate_boolean_coverage(evidence.get('strip_all_tags_flag_case_counts'),counts.get('wp_strip_all_tags'))
        if 'sanitize_user' in expected:
            validate_sanitize_user_profile(evidence.get('php_sanitize_user_profile'))
            validate_sanitize_user_coverage(evidence.get('sanitize_user_reference_calls'),evidence.get('sanitize_user_filter_calls'),
                evidence.get('sanitize_user_locale_hook_counts'),evidence.get('sanitize_user_expected_locale_hook_counts'),
                evidence.get('locale_db_branch_calls'),evidence.get('sanitize_user_strict_case_counts'),evidence.get('previous_api66_input_sha256'),counts.get('sanitize_user'))
        if 'utf8_uri_encode'in expected or 'sanitize_title_with_dashes'in expected:
            validate_previous68(evidence.get('previous_api68_input_sha256'),evidence.get('previous_api68_results_sha256'))
        if 'utf8_uri_encode'in expected:
            validate_slug_profile(evidence.get('php_utf8_uri_profile'))
            validate_slug_coverage(evidence.get('utf8_uri_reference_calls'),evidence.get('utf8_uri_option_case_counts'),counts.get('utf8_uri_encode'))
        if 'sanitize_title_with_dashes'in expected:
            validate_slug_profile(evidence.get('php_title_dash_profile'),True)
            validate_slug_coverage(evidence.get('title_dash_reference_calls'),evidence.get('title_dash_context_case_counts'),counts.get('sanitize_title_with_dashes'),True)
        if 'sanitize_title'in expected:
            validate_default_title_profile(evidence.get('php_default_sanitize_title_profile'))
            validate_default_title_coverage(evidence.get('default_title_reference_calls'),evidence.get('default_title_filter_calls'),
                evidence.get('default_title_locale_hook_counts'),evidence.get('default_title_expected_locale_hook_counts'),
                evidence.get('default_title_global_restorations'),evidence.get('default_title_context_case_counts'),
                evidence.get('previous_api70_input_sha256'),evidence.get('previous_api70_results_sha256'),counts.get('sanitize_title'))
        if '_truncate_post_slug'in expected:
            validate_truncate_slug_profile(evidence.get('php_truncate_slug_profile'))
            validate_truncate_slug_coverage(evidence.get('truncate_slug_reference_calls'),evidence.get('truncate_slug_branch_counts'),
                evidence.get('truncate_slug_expected_branch_counts'),evidence.get('previous_api71_input_sha256'),
                evidence.get('previous_api71_results_sha256'),counts.get('_truncate_post_slug'))
        if 'get_post_meta' in expected:
            validate_post_meta_cached_coverage(evidence.get('php_post_meta_cached_profile'),
                evidence.get('post_meta_cached_counts'), evidence.get('post_meta_cached_expected_counts'),
                counts.get('get_post_meta'), evidence.get('previous_api72_case_count'),
                evidence.get('previous_api72_input_sha256'), evidence.get('previous_api72_rust_results_sha256'),
                evidence.get('previous_api72_wordpress_results_sha256'))
        if 'wptexturize' in expected:
            validate_texturize_coverage(evidence.get('php_texturize_profile'),
                evidence.get('texturize_counts'), evidence.get('texturize_expected_counts'),
                counts.get('wptexturize'), evidence.get('previous_api73_case_count'),
                evidence.get('previous_api73_input_sha256'), evidence.get('previous_api73_rust_results_sha256'),
                evidence.get('previous_api73_wordpress_results_sha256'))
        if 'get_the_title' in expected:
            validate_title_coverage(evidence.get('php_title_profile'),
                evidence.get('title_counts'), evidence.get('title_expected_counts'),
                counts.get('get_the_title'), evidence.get('previous_api74_case_count'),
                evidence.get('previous_api74_input_sha256'), evidence.get('previous_api74_rust_results_sha256'),
                evidence.get('previous_api74_wordpress_results_sha256'))
        if 'wp_generate_auth_cookie' in expected:
            validate_auth_cookie_coverage(evidence.get('php_auth_cookie_profile'),
                evidence.get('auth_cookie_counts'), evidence.get('auth_cookie_expected_counts'),
                counts.get('wp_generate_auth_cookie'), evidence.get('previous_api75_case_count'),
                evidence.get('previous_api75_input_sha256'), evidence.get('previous_api75_rust_results_sha256'),
                evidence.get('previous_api75_wordpress_results_sha256'))
        if 'wp_parse_auth_cookie' in expected:
            validate_auth_cookie_parse_coverage(evidence.get('php_auth_cookie_parse_profile'),
                evidence.get('auth_cookie_parse_counts'), evidence.get('auth_cookie_parse_expected_counts'),
                counts.get('wp_parse_auth_cookie'), evidence.get('previous_api76_case_count'),
                evidence.get('previous_api76_input_sha256'), evidence.get('previous_api76_rust_results_sha256'),
                evidence.get('previous_api76_wordpress_results_sha256'))
        if expected & set(sanitize_text_profile.APIS):
            field_count = counts.get('sanitize_text_field')
            area_count = counts.get('sanitize_textarea_field')
            expected_by_api = {name: sum(case['api'] == name for case in sanitize_text_profile.cases())
                               for name in sanitize_text_profile.APIS}
            if (type(field_count) is not int or field_count != expected_by_api['sanitize_text_field']
                    or type(area_count) is not int or area_count != expected_by_api['sanitize_textarea_field']):
                raise ValueError('Text sanitizer per-API finite coverage differs')
            validate_sanitize_text_coverage(evidence.get('php_sanitize_text_profile'),
                evidence.get('sanitize_text_counts'), evidence.get('sanitize_text_expected_counts'),
                field_count + area_count, evidence.get('previous_api77_case_count'),
                evidence.get('previous_api77_input_sha256'), evidence.get('previous_api77_rust_results_sha256'),
                evidence.get('previous_api77_wordpress_results_sha256'))
        if expected & {"wp_kses_decode_entities", "wp_kses_bad_protocol_once2", "wp_kses_bad_protocol_once", "wp_kses_bad_protocol"}:
            validate_entity_decode_profile(evidence.get("php_entity_decode_profile"))
        if "wp_kses_bad_protocol" in expected:
            validate_protocol_outer_profile(evidence.get("php_protocol_outer_profile"))
        if "wp_kses_bad_protocol_once" in expected:
            validate_protocol_once_profile(evidence.get("php_protocol_once_profile"))
        if "wp_convert_hr_to_bytes" in expected:
            validate_hr_byte_units(evidence.get("php_hr_byte_units"))
            validate_hr_byte_domain(evidence.get("php_hr_byte_max_significant_digits"))
        if "wp_json_encode" in expected:
            validate_json_encode_flags(evidence.get("php_json_encode_flags"))
            validate_json_utf8_fallback_profile(evidence.get("php_json_utf8_fallback_profile"))
        if expected & {"wp_unique_id", "wp_unique_prefixed_id"} and evidence.get("unique_id_state_scope") != UNIQUE_ID_STATE_SCOPE:
            raise ValueError("unique ID comparison context lifetime is not recorded")
        if not isinstance(counts, dict) or set(counts) != expected or any(type(count) is not int or count <= 0 for count in counts.values()) or sum(counts.values()) != evidence["case_count"]:
            raise ValueError("comparison evidence API identities/counts are inconsistent")
        if "wp_json_encode" in expected:
            validate_json_flag_cases(evidence.get("json_encode_flag_case_counts"), counts["wp_json_encode"])
            validate_json_profile_cases(evidence.get("json_encode_profile_case_counts"), counts["wp_json_encode"])
            validate_json_flag_cases(evidence.get("json_encode_utf8_fallback_flag_case_counts"),
                                     evidence["json_encode_profile_case_counts"]["utf8_no_mbstring"])
        for field in ["comparison_batches", "batch_case_limit", "batch_input_limit_bytes", "max_observed_batch_bytes"]:
            if type(evidence[field]) is not int or evidence[field] <= 0:
                raise ValueError("comparison batch observations must be positive integers")
        if evidence["comparison_batches"] > evidence["case_count"] or evidence["max_observed_batch_bytes"] > evidence["batch_input_limit_bytes"]:
            raise ValueError("comparison batch observations exceed their limits")
    if scope in ["request_local_WP_Hook_subset", "named_dispatcher_subset"]:
        fields=["typed_value_case_count","legacy_string_case_count"]
        if any(type(evidence[field]) is not int or evidence[field] <= 0 for field in fields) or sum(evidence[field] for field in fields) != evidence["case_count"]:
            raise ValueError("hook value-domain scenario counts are incomplete or inconsistent")
    if evidence["rust_results_sha256"] != evidence["wordpress_results_sha256"]:
        raise ValueError("comparison evidence output hashes differ")
    if scope not in ["isolated_function_subsets_with_real_wp_filter_dispatch", "request_local_WP_Hook_subset", "named_dispatcher_subset"] or not evidence["limitations"]:
        raise ValueError("comparison evidence scope/limitations missing")
    groups = evidence_files(scope)
    for group, required in groups.items():
        if not isinstance(evidence[group], dict) or set(evidence[group]) != required:
            raise ValueError(f"evidence file identities incomplete or unexpected: {group}")
        for name, expected in evidence[group].items():
            source = (reference.ROOT / name).resolve()
            if not source.is_relative_to(reference.ROOT.resolve()) or reference.digest_file(source) != expected:
                raise ValueError(f"comparison evidence is stale: {name}; rerun scripts/compare_primitives.py")
    if not isinstance(evidence["source_files"], dict) or set(evidence["source_files"]) != evidence_sources(scope, registry, relative):
        raise ValueError("evidence reference identities incomplete or unexpected")
    for name, expected in evidence["source_files"].items():
        relative = reference.safe_relative("wordpress/" + name)
        source = reference.REFERENCE / f"wordpress-{reference.VERSION}" / relative
        if reference.digest_file(source) != expected:
            raise ValueError(f"comparison reference source changed: {name}")
    return evidence


def report(apis, components=()):
    print(f"WordPress {reference.VERSION}: {len(apis)} tracked API records; complete API-domain verification is not claimed")
    print("The complete API universe has not been enumerated; this is not a coverage percentage.")
    for api in apis:
        print(f"  {api['wordpress_symbol']} -> {api['rust_symbol']}: {api['status']}")
        if "differential_evidence" in api:
            print(f"    supported-subset comparison: {api['differential_evidence']}")
        for limitation in api["limitations"]:
            print(f"    pending: {limitation}")
    for component in components:
        label = component.get("wordpress_class") or component["wordpress_component"]
        print(f"  {label} -> {component['rust_type']}: {component['status']} subset")
        print(f"    supported-subset comparison: {component['differential_evidence']}")
        for limitation in component["limitations"]:
            print(f"    pending: {limitation}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    try:
        registry = json.loads(REGISTRY.read_text(encoding="utf-8"))
        report(validate(registry), registry.get("components", []))
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"Compatibility registry check failed: {error}\n")
