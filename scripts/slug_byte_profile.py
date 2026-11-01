"""Explicit URI/title byte kernels, preserving the preceding complete corpus."""
SOURCE_SHA256='350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32'
OLD_COUNT=453064
OLD_INPUT_SHA256='77a9129b8aa98f0823d423b138c30328a0bf690b392e4cd79b157fe861d4664a'
OLD_OUTPUT_SHA256='787fa2917806bee17af805f8bf7f3b9b06d8eb460bb4f786e88ebe1d52f8428f'
URI_NAME='valid_utf8_explicit_length_bool_no_mbstring'
TITLE_NAME='no_lt_post_nul_utf8_explicit_context_no_mbstring'
COMMON={'php_version':'8.5.12','mbstring_loaded':False,'mb_internal_encoding_exists':False,'mb_strtolower_exists':False,
        'source_sha256':SOURCE_SHA256,'fixture_final_host_cap_bytes':65536}
URI_PROFILE={**COMMON,'input_type':'bytes_whole_valid_utf8','length_type':'integer_0_to_i64_max',
    'encode_ascii_type':'bool','length_zero':'unlimited','length_unit':'encoded_bytes_whole_scalar',
    'ascii_hex_case':'upper','non_ascii_hex_case':'lower'}
TITLE_PROFILE={**COMMON,'input_type':'bytes_no_literal_lt_post_nul_valid_utf8','raw_title':'explicit_empty_bytes',
    'contexts':['display','save'],'uri_encoded_limit':200,'uri_limit_before_cleanup':True,'accents_removed':False,
    'sentinel_restoration':'original_and_generated_literal_triple_dash_hex_pair',
    'outer_sanitize_title':False,'locale_resolution':False,'ctype_locale':'C'}
