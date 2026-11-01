"""Pinned literal-map profile, no ambient locale/Intl/charset policy."""
import export_remove_accents as data

NAME = 'valid_utf8_explicit_locale_no_intl'
PROFILE = {'php_version':'8.5.12','text_type':'bytes_valid_utf8','locale_type':'bytes_php_truthy',
    'intl_loaded':False,'normalizer_is_normalized_exists':False,'normalizer_normalize_exists':False,
    'mbstring_loaded':False,'locale_rules':'de_prefix/da_DK_exact/ca_exact/sr_RS_exact/bs_BA_exact/other_base',
    'nfc_normalization':False,'get_locale_calls':0,'latin1_fallback_supported':False,**data.profile()}
BRANCHES = ('base','german','danish','catalan','serbian','bosnian')

def branch(locale):
    if locale.startswith(b'de'): return 'german'
    return {b'da_DK':'danish',b'ca':'catalan',b'sr_RS':'serbian',b'bs_BA':'bosnian'}.get(locale,'base')
