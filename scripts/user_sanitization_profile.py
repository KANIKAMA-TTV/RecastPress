"""Explicit NoLiteral< byte subsets and frozen preceding corpus identity."""
import export_remove_accents as accents

OLD_COUNT=443078
OLD_INPUT_SHA256='26145278d40d4c0915218fd2e0d18b426a115ec7d5dcaa20b003a69f71100310'
ACCENT_COUNT=3698
ACCENT_INPUT_SHA256='10a0742bd427aaddbf856add8533b5b427f132d3d433ee0dbb4f8368cef59f1e'
L10N_SHA256='43c9af6b687ee3eb92608fea4697ebe7b62b3d45490a7657efce71033ac82ed7'
STRIP_NAME='no_literal_lt'
USER_NAME='no_lt_utf8_resolved_locale_c_empty_filters'
STRIP_PROFILE={'php_version':'8.5.12','input_type':'bytes_no_literal_lt','remove_breaks_type':'bool',
    'utf8_validation':False,'stages':'delete_nul/optional_cr_lf_tab_space_collapse/php_trim6',
    'source_sha256':accents.SOURCE_SHA256}
USER_PROFILE={'php_version':'8.5.12','input_type':'bytes_no_literal_lt_post_strip_valid_utf8','strict_type':'bool',
    'locale_type':'bytes_php_truthy','locale_resolution':'scoped_global_first_isset_branch',
    'filter_registry':'empty','native_filter_bookkeeping':False,'ctype_locale':'C',
    'intl_loaded':False,'normalizer_is_normalized_exists':False,'normalizer_normalize_exists':False,
    'mbstring_loaded':False,'l10n_source_sha256':L10N_SHA256,'latin1_fallback_supported':False,**accents.profile()}

def stripped_input(raw):
    return raw.replace(b'\0',b'').strip(b'\0\t\n\r\v ')

def locale_calls(raw):
    return int(any(byte>=128 for byte in stripped_input(raw)))
