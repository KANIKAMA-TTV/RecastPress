<?php
/** Development-only oracle for isolated functions in pinned WordPress source. */
error_reporting(E_ALL);
set_error_handler(static function ($level, $message, $file, $line) {
    throw new ErrorException($message, 0, $level, $file, $line);
});

$root = $argv[1] ?? throw new RuntimeException('missing pinned source root');
require $root . '/wp-includes/plugin.php';
require $root . '/wp-includes/compat-utf8.php';
require $root . '/wp-includes/utf8.php';
require $root . '/wp-includes/class-wp-list-util.php';

require __DIR__ . '/reference_functions.php';

// Extract only the predicate: never initialize KSES/entity globals here.
load_reference_function($root . '/wp-includes/kses.php', 'valid_unicode');
load_reference_function($root . '/wp-includes/formatting.php', '_wp_specialchars');
load_reference_function($root . '/wp-includes/formatting.php', 'esc_html');
load_reference_function($root . '/wp-includes/formatting.php', 'esc_attr');
load_reference_function($root . '/wp-includes/pluggable.php', 'wp_check_password');
load_reference_function($root . '/wp-includes/pluggable.php', 'wp_password_needs_rehash');
load_reference_function($root . '/wp-includes/formatting.php', 'remove_accents');
load_reference_function($root . '/wp-includes/formatting.php', 'wp_strip_all_tags');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_user');
load_reference_function($root . '/wp-includes/formatting.php', 'utf8_uri_encode');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_title_with_dashes');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_title');
load_reference_function($root . '/wp-includes/post.php', '_truncate_post_slug');
load_reference_function($root . '/wp-includes/functions.php', 'mbstring_binary_safe_encoding');
load_reference_function($root . '/wp-includes/functions.php', 'reset_mbstring_encoding');
load_reference_function($root . '/wp-includes/l10n.php', 'get_locale');
load_reference_function($root . '/wp-includes/functions.php', '_canonical_charset');
foreach (['wp_kses_normalize_entities', 'wp_kses_normalize_entities2', 'wp_kses_normalize_entities3', 'wp_kses_named_entities', 'wp_kses_xml_named_entities'] as $symbol) {
    load_reference_function($root . '/wp-includes/kses.php', $symbol);
}
foreach (['wp_kses_decode_entities', '_wp_kses_decode_entities_chr', '_wp_kses_decode_entities_chr_hexdec'] as $symbol) {
    load_reference_function($root . '/wp-includes/kses.php', $symbol);
}
load_reference_function($root . '/wp-includes/kses.php', 'wp_kses_no_null');
load_reference_function($root . '/wp-includes/kses.php', 'wp_kses_stripslashes');
load_reference_function($root . '/wp-includes/kses.php', 'wp_kses_bad_protocol_once2');
load_reference_function($root . '/wp-includes/kses.php', 'wp_kses_bad_protocol_once');
load_reference_function($root . '/wp-includes/kses.php', 'wp_kses_bad_protocol');
function protocol_outer_profile() {
    return ['input_type' => 'bytes', 'allowed_values' => 'bytes_only', 'ctype_locale' => setlocale(LC_CTYPE, 0),
        'top_input_max_bytes' => 1048576, 'global_no_null_passes' => 1, 'max_once_passes' => 6,
        'once_default_count' => 1, 'once_max_frames' => 3,
        'fastpath' => 'literal_lower_http_https_strict_values', 'internal_top_guard' => false];
}
function slug_common_profile(){global$root;return['php_version'=>PHP_VERSION,'mbstring_loaded'=>extension_loaded('mbstring'),
    'mb_internal_encoding_exists'=>function_exists('mb_internal_encoding'),'mb_strtolower_exists'=>function_exists('mb_strtolower'),
    'source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php'),'fixture_final_host_cap_bytes'=>65536];}
function utf8_uri_profile(){return[...slug_common_profile(),'input_type'=>'bytes_whole_valid_utf8','length_type'=>'integer_0_to_i64_max',
    'encode_ascii_type'=>'bool','length_zero'=>'unlimited','length_unit'=>'encoded_bytes_whole_scalar','ascii_hex_case'=>'upper','non_ascii_hex_case'=>'lower'];}
function title_dash_profile(){return[...slug_common_profile(),'input_type'=>'bytes_no_literal_lt_post_nul_valid_utf8','raw_title'=>'explicit_empty_bytes',
    'contexts'=>['display','save'],'uri_encoded_limit'=>200,'uri_limit_before_cleanup'=>true,'accents_removed'=>false,
    'sentinel_restoration'=>'original_and_generated_literal_triple_dash_hex_pair','outer_sanitize_title'=>false,'locale_resolution'=>false,'ctype_locale'=>setlocale(LC_CTYPE,0)];}
function slug_no_mb_guard(){if(extension_loaded('mbstring')||function_exists('mb_internal_encoding')||function_exists('mb_strtolower')){throw new RuntimeException('Slug fixture requires no mbstring functions');}}
$utf8_uri_calls=0;$title_dash_calls=0;
$truncate_slug_calls=0;$truncate_slug_branches=['raw_no_truncate'=>0,'raw_substr'=>0,'decoded_uri'=>0];
function truncate_slug_profile(){global$root;return['php_version'=>PHP_VERSION,'input_type'=>'arbitrary_bytes_with_conditional_decoded_whole_utf8',
    'length_type'=>'integer_0_to_i64_max','decoded_utf8_guard'=>'only_strlen_gt_length_and_urldecode_changed',
    'decode'=>'php_urldecode_plus_space_single_percent_pass','uri_encode_ascii'=>true,'source_length_zero'=>'uri_unlimited_or_raw_substr_empty',
    'rtrim_bytes_hex'=>'2d','mbstring_loaded'=>extension_loaded('mbstring'),'mb_internal_encoding_exists'=>function_exists('mb_internal_encoding'),
    'mb_strtolower_exists'=>function_exists('mb_strtolower'),'fixture_final_host_cap_bytes'=>65536,
    'source_sha256'=>hash_file('sha256',$root.'/wp-includes/post.php'),'uri_source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php')];}
function oracle_truncate_slug(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$length,#[\SensitiveParameter]$profile){
    global$truncate_slug_calls,$truncate_slug_branches;
    if(!is_string($raw)||!is_int($length)||$length<0||$profile!=='conditional_decoded_utf8_no_mbstring'){throw new RuntimeException('Truncate slug input/profile unsupported');}
    slug_no_mb_guard();$branch='raw_no_truncate';
    if(strlen($raw)>$length){$decoded=urldecode($raw);$branch=$decoded===$raw?'raw_substr':'decoded_uri';
        if($branch==='decoded_uri'&&!wp_is_valid_utf8($decoded)){throw new RuntimeException('Changed decoded UTF8 outside truncate subset');}}
    preg_match('//','');
    try{$result=_truncate_post_slug($raw,$length);}catch(Throwable$unused){throw new RuntimeException('Truncate slug reference failed');}
    if(!is_string($result)||strlen($result)>65536||preg_last_error()!==PREG_NO_ERROR){throw new RuntimeException('Truncate slug reference result unsupported');}
    ++$truncate_slug_calls;++$truncate_slug_branches[$branch];return$result;
}
$default_title_calls=0;$default_title_filter_calls=0;$default_title_locale_calls=['zero'=>0,'one'=>0];$default_title_restored=0;
function default_title_callback(){
    global$root;static$checked=null;if($checked!==null){return$checked;}
    $wire=file_get_contents($root.'/wp-includes/default-filters.php',false,null,0,1048577);
    if(!is_string($wire)||strlen($wire)>1048576||hash('sha256',$wire)!=='55bbf47fb835a0f64f28423d475b51f7fa72479f319fca417b4f23990d44f897'){throw new RuntimeException('Default title callback source differs');}
    $lines=explode("\n",$wire);$expected="add_filter( 'sanitize_title', 'sanitize_title_with_dashes', 10, 3 );";
    $count=preg_match_all("/^add_filter\\( 'sanitize_title', .*?\\);$/m",$wire,$matches);
    if($count!==1||($lines[308]??null)!==$expected||$matches[0]!==[$expected]){throw new RuntimeException('Default title callback tuple differs');}
    return$checked=['sanitize_title','sanitize_title_with_dashes',10,3];
}
function default_sanitize_title_profile(){
    global$root;$tuple=default_title_callback();
    return['php_version'=>PHP_VERSION,'input_type'=>'bytes_no_literal_lt_whole_valid_utf8','fallback_type'=>'arbitrary_bytes_unmodified',
        'contexts'=>['save','display','query'],'locale_type'=>'bytes_php_truthy_all_contexts','locale_resolution'=>'scoped_global_first_isset_branch',
        'ctype_locale'=>setlocale(LC_CTYPE,0),'mbstring_loaded'=>extension_loaded('mbstring'),'mb_internal_encoding_exists'=>function_exists('mb_internal_encoding'),
        'mb_strtolower_exists'=>function_exists('mb_strtolower'),'intl_loaded'=>extension_loaded('intl'),
        'normalizer_is_normalized_exists'=>function_exists('normalizer_is_normalized'),'normalizer_normalize_exists'=>function_exists('normalizer_normalize'),
        'fixture_final_host_cap_bytes'=>65536,'default_filters_source_sha256'=>hash_file('sha256',$root.'/wp-includes/default-filters.php'),
        'callback_hook'=>$tuple[0],'callback_function'=>$tuple[1],'callback_priority'=>$tuple[2],'callback_accepted_args'=>$tuple[3],
        'callback_count'=>1,'all_hook'=>false,'fallback_condition'=>'empty_string_or_false_not_php_empty','native_filter_bookkeeping'=>false,
        'source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php'),'base_count'=>311,
        'base_ordered_pairs_sha256'=>'69b78d9596b1e927213e72b0698a40a85daa76e8846a4d86d7a13c78d4594477',
        'locale_override_ordered_pairs_sha256'=>'07c40351a07697a814d6a97ec45c2549b7f08071f3ff3cc33768fa0285e6932c'];
}
function oracle_default_sanitize_title(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$fallback,#[\SensitiveParameter]$context,#[\SensitiveParameter]$resolved_locale,#[\SensitiveParameter]$profile){
    global$default_title_calls,$default_title_filter_calls,$default_title_locale_calls,$default_title_restored,$locale_db_branch_calls;
    if(!is_string($raw)||!is_string($fallback)||!is_string($resolved_locale)||$resolved_locale===''||$resolved_locale==='0'
        ||!in_array($context,['save','display','query'],true)||$profile!=='valid_utf8_no_lt_explicit_locale_default_title_callback'
        ||str_contains($raw,'<')||!wp_is_valid_utf8($raw)||setlocale(LC_CTYPE,0)!=='C'
        ||extension_loaded('intl')||function_exists('normalizer_is_normalized')||function_exists('normalizer_normalize')){throw new RuntimeException('Default title input/profile unsupported');}
    slug_no_mb_guard();$tuple=default_title_callback();
    $expected_locale=$context==='save'&&preg_match('/[\x80-\xff]/',$raw)===1?1:0;
    if(preg_last_error()!==PREG_NO_ERROR){throw new RuntimeException('Title locale preflight failed');}
    $saved=[];foreach(['wp_filter','wp_filters','wp_current_filter','locale','wp_local_package','wp_actions','wp_hasher']as$name){$saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
    $db_before=$locale_db_branch_calls;
    try{
        $GLOBALS['wp_filter']=[];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];$GLOBALS['locale']=$resolved_locale;$GLOBALS['wp_actions']=[];$GLOBALS['wp_hasher']=null;
        add_filter(...$tuple);
        $callbacks=[10=>['sanitize_title_with_dashes'=>['function'=>'sanitize_title_with_dashes','accepted_args'=>3]]];
        if(array_keys($GLOBALS['wp_filter'])!==['sanitize_title']||!($GLOBALS['wp_filter']['sanitize_title']instanceof WP_Hook)||$GLOBALS['wp_filter']['sanitize_title']->callbacks!==$callbacks){throw new RuntimeException('Default title callback registration differs');}
        preg_match('//','');
        try{$result=sanitize_title($raw,$fallback,$context);}catch(Throwable$unused){throw new RuntimeException('Default title reference failed');}
        $expected_hooks=$expected_locale===1?['locale'=>1,'sanitize_title'=>1]:['sanitize_title'=>1];
        if(!is_string($result)||strlen($result)>65536||preg_last_error()!==PREG_NO_ERROR||$GLOBALS['wp_current_filter']!==[]
            ||$GLOBALS['wp_filters']!==$expected_hooks||array_keys($GLOBALS['wp_filter'])!==['sanitize_title']
            ||$GLOBALS['wp_filter']['sanitize_title']->callbacks!==$callbacks||$GLOBALS['locale']!==$resolved_locale
            ||$GLOBALS['wp_actions']!==[]||$GLOBALS['wp_hasher']!==null||$locale_db_branch_calls!==$db_before){throw new RuntimeException('Default title actual scope differs');}
        ++$default_title_calls;++$default_title_filter_calls;++$default_title_locale_calls[$expected_locale?'one':'zero'];
        return$result;
    }finally{
        foreach($saved as$name=>[$had,$old]){if($had){$GLOBALS[$name]=$old;}else{unset($GLOBALS[$name]);}}
        foreach($saved as$name=>[$had,$old]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$old)){throw new RuntimeException('Default title scope restoration failed');}}
        ++$default_title_restored;
    }
}
function oracle_utf8_uri(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$length,#[\SensitiveParameter]$ascii,#[\SensitiveParameter]$profile){
    global$utf8_uri_calls;
    if(!is_string($raw)||!is_int($length)||$length<0||!is_bool($ascii)||$profile!=='valid_utf8_explicit_length_bool_no_mbstring'||!wp_is_valid_utf8($raw)){throw new RuntimeException('URI fixture input/profile unsupported');}
    slug_no_mb_guard();
    try{$result=utf8_uri_encode($raw,$length,$ascii);}catch(Throwable$e){throw new RuntimeException('URI reference failed');}
    if(!is_string($result)||preg_last_error()!==PREG_NO_ERROR||strlen($result)>65536){throw new RuntimeException('URI reference result unsupported');}
    ++$utf8_uri_calls;return$result;
}
function oracle_title_dash(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$raw_title,#[\SensitiveParameter]$context,#[\SensitiveParameter]$profile){
    global$title_dash_calls;
    if(!is_string($raw)||!is_string($raw_title)||$raw_title!==''||!in_array($context,['display','save'],true)
        ||$profile!=='no_lt_post_nul_utf8_explicit_context_no_mbstring'||str_contains($raw,'<')||!wp_is_valid_utf8(str_replace("\0",'',$raw))){throw new RuntimeException('Title fixture input/profile unsupported');}
    slug_no_mb_guard();if(setlocale(LC_CTYPE,0)!=='C'){throw new RuntimeException('Title fixture requires C locale');}
    try{$result=sanitize_title_with_dashes($raw,$raw_title,$context);}catch(Throwable$e){throw new RuntimeException('Title reference failed');}
    if(!is_string($result)||preg_last_error()!==PREG_NO_ERROR||strlen($result)>200){throw new RuntimeException('Title reference result unsupported');}
    ++$title_dash_calls;return$result;
}
function oracle_bad_protocol_outer_c($input, $protocols) {
    if (!is_string($input) || !is_array($protocols)) {
        throw new RuntimeException('Protocol-outer fixture requires byte content and an array of byte-string protocols');
    }
    foreach ($protocols as $protocol) {
        if (!is_string($protocol)) { throw new RuntimeException('Protocol-outer fixture requires byte content and an array of byte-string protocols'); }
    }
    if (strlen($input) > 1048576) { throw new RuntimeException('protocol outer input exceeds the top-level host byte bound'); }
    if (setlocale(LC_CTYPE, 0) !== 'C') { throw new RuntimeException('Protocol-outer fixture requires LC_CTYPE=C'); }
    // Admission follows only visited prefixes. Never apply the public once
    // entry guard to expanded internal content, or admit an unvisited seventh pass.
    $current = wp_kses_no_null($input);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($current)) { throw new RuntimeException('Protocol-outer no-null PCRE error'); }
    $fast = (str_starts_with($current, 'https://') && in_array('https', $protocols, true)) ||
        (str_starts_with($current, 'http://') && in_array('http', $protocols, true));
    if (!$fast) {
        for ($pass = 0; $pass < 6; ++$pass) {
            admit_protocol_once_prefixes($current, $protocols);
            if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) { throw new RuntimeException('Protocol-outer reset failed'); }
            $next = wp_kses_bad_protocol_once($current, $protocols);
            if (preg_last_error() !== PREG_NO_ERROR || !is_string($next)) { throw new RuntimeException('Protocol-outer once PCRE error'); }
            if ($next === $current) { break; }
            $current = $next;
        }
    }
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) { throw new RuntimeException('Protocol-outer reset failed'); }
    $result = wp_kses_bad_protocol($input, $protocols);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) { throw new RuntimeException('Protocol-outer PCRE error or nonstring result'); }
    return $result;
}
function protocol_once_profile() {
    return ['input_type' => 'bytes', 'allowed_values' => 'bytes_only',
        'ctype_locale' => setlocale(LC_CTYPE, 0), 'default_count' => 1,
        'top_input_max_bytes' => 1048576, 'max_frames' => 3];
}
function admit_protocol_once_prefixes($content, $protocols, $count = 1) {
    // Admission only: mirror the source's branch/recursion conditions, not a
    // whole-body decoder. The compared result is the public function below.
    $content = preg_replace('/(&#0*58(?![;0-9])|&#x0*3a(?![;a-f0-9]))/i', '$1;', $content);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($content)) {
        throw new RuntimeException('Protocol-once fixture encountered a repair PCRE error');
    }
    $parts = preg_split('/:|&#0*58;|&#x0*3a;|&colon;/i', $content, 2);
    if (preg_last_error() !== PREG_NO_ERROR || !is_array($parts)) {
        throw new RuntimeException('Protocol-once fixture encountered a split PCRE error');
    }
    if (!isset($parts[1])) { return; }
    $guard = preg_match('%/\?%', $parts[0]);
    if (preg_last_error() !== PREG_NO_ERROR || !is_int($guard)) {
        throw new RuntimeException('Protocol-once fixture encountered a guard PCRE error');
    }
    if ($guard !== 0) { return; }
    $tail = trim($parts[1]);
    $protocol = oracle_bad_protocol_once2_c($parts[0], $protocols);
    // once2 must run even in frame three, before this depth branch.
    if ($protocol === 'feed:' && $count <= 2) {
        admit_protocol_once_prefixes($tail, $protocols, $count + 1);
    }
}
function oracle_bad_protocol_once_c($input, $protocols, $has_count) {
    if (!is_string($input) || !is_array($protocols) || $has_count) {
        throw new RuntimeException('Protocol-once fixture requires byte content, byte protocol array, and default count');
    }
    foreach ($protocols as $protocol) {
        if (!is_string($protocol)) {
            throw new RuntimeException('Protocol-once fixture requires byte content, byte protocol array, and default count');
        }
    }
    if (strlen($input) > 1048576) {
        throw new RuntimeException('protocol once input exceeds the top-level host byte bound');
    }
    if (setlocale(LC_CTYPE, 0) !== 'C') { throw new RuntimeException('Protocol-once fixture requires LC_CTYPE=C'); }
    admit_protocol_once_prefixes($input, $protocols);
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('Protocol-once fixture could not reset PCRE state');
    }
    $result = wp_kses_bad_protocol_once($input, $protocols);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) {
        throw new RuntimeException('Protocol-once fixture encountered a PCRE error or nonstring result');
    }
    return $result;
}
function oracle_bad_protocol_once2_c($input, $protocols) {
    if (!is_string($input) || !is_array($protocols)) {
        throw new RuntimeException('Protocol fixture requires byte scheme and an array of byte-string protocols');
    }
    foreach ($protocols as $protocol) {
        if (!is_string($protocol)) {
            throw new RuntimeException('Protocol fixture requires byte scheme and an array of byte-string protocols');
        }
    }
    if (setlocale(LC_CTYPE, 0) !== 'C') {
        throw new RuntimeException('Protocol fixture requires LC_CTYPE=C');
    }
    // Admission only: preserve the inherited whole-input and both-pass byte
    // value domain even when the allowlist is empty. Compare the actual helper.
    oracle_decode_byte_entities($input);
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('Protocol fixture could not reset PCRE state');
    }
    $result = wp_kses_bad_protocol_once2($input, $protocols);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) {
        throw new RuntimeException('Protocol fixture encountered a PCRE error or nonstring result');
    }
    return $result;
}
function oracle_kses_stripslashes_bytes($input) {
    if (!is_string($input)) {
        throw new RuntimeException('KSES slash fixture requires byte-string input');
    }
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('KSES slash fixture could not reset PCRE state');
    }
    $result = wp_kses_stripslashes($input);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) {
        throw new RuntimeException('KSES slash fixture encountered a PCRE error or nonstring result');
    }
    return $result;
}
function oracle_no_null_bytes($input, $mode) {
    if (!is_string($input) || !is_string($mode) || !in_array($mode, ['remove', 'keep'], true)) {
        throw new RuntimeException('Null-removal fixture requires byte-string input and explicit remove/keep option');
    }
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('Null-removal fixture could not reset PCRE state');
    }
    $result = wp_kses_no_null($input, ['slash_zero' => $mode]);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) {
        throw new RuntimeException('Null-removal fixture encountered a PCRE error or nonstring result');
    }
    return $result;
}
function entity_decode_profile() {
    return ['input_type' => 'bytes', 'max_input_bytes' => 1048576,
        'max_reference_value' => 255, 'order' => 'decimal_then_hex'];
}
function check_entity_decode_digits($digits, $hex) {
    $significant = ltrim($digits, '0');
    if (strlen($significant) > ($hex ? 2 : 3)
        || (!$hex && strlen($significant) === 3 && strcmp($significant, '255') > 0)) {
        throw new RuntimeException('entity decoder recognizes only values from 0 through 255');
    }
}
function oracle_decode_byte_entities($input) {
    if (!is_string($input)) {
        throw new RuntimeException('Entity decoder fixture requires byte-string input');
    }
    if (strlen($input) > 1048576) {
        throw new RuntimeException('entity decoder input exceeds the host input byte bound');
    }
    // Admission only: run the same two patterns with real source callbacks to
    // check generated hex references too. This is not the compared result.
    $intermediate = preg_replace_callback('/&#([0-9]+);/', static function ($matches) {
        check_entity_decode_digits($matches[1], false);
        return _wp_kses_decode_entities_chr($matches);
    }, $input);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($intermediate)) {
        throw new RuntimeException('Entity decoder fixture encountered a decimal preflight PCRE error');
    }
    $preflight = preg_replace_callback('/&#[Xx]([0-9A-Fa-f]+);/', static function ($matches) {
        check_entity_decode_digits($matches[1], true);
        return _wp_kses_decode_entities_chr_hexdec($matches);
    }, $intermediate);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($preflight)) {
        throw new RuntimeException('Entity decoder fixture encountered a hex preflight PCRE error');
    }
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('Entity decoder fixture could not reset PCRE state');
    }
    $result = wp_kses_decode_entities($input);
    if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) {
        throw new RuntimeException('Entity decoder fixture encountered a PCRE error or nonstring result');
    }
    return $result;
}
// Data only: never execute KSES bootstrap, define CUSTOM_TAGS or initialize tag globals.
$entity_data = json_decode(file_get_contents(__DIR__ . '/../compatibility/wordpress-default-html-entities.json'), true, 512, JSON_THROW_ON_ERROR);
$entity_keys = array_keys($entity_data);
sort($entity_keys);
$expected_entity_keys = ['format_version', 'wordpress_version', 'source_path', 'source_line', 'context', 'names_count', 'source_sha256', 'ordered_names_sha256', 'names'];
sort($expected_entity_keys);
if ($entity_keys !== $expected_entity_keys || $entity_data['format_version'] !== 1
    || $entity_data['wordpress_version'] !== '7.1.3' || $entity_data['source_path'] !== 'wp-includes/kses.php'
    || $entity_data['source_line'] !== 639 || !is_array($entity_data['names']) || !array_is_list($entity_data['names'])) {
    throw new RuntimeException('Entity fixture data identity differs');
}
$entity_source_sha256 = hash_file('sha256', $root . '/wp-includes/kses.php');
$default_html_entity_names = $entity_data['names'];
function default_html_entity_profile($names = null) {
    global $default_html_entity_names, $entity_source_sha256;
    $names ??= $default_html_entity_names;
    if (!is_array($names) || !array_is_list($names)
        || count($names) !== 253
        || count(array_unique($names, SORT_STRING)) !== 253) {
        throw new RuntimeException('Entity fixture default HTML name set differs');
    }
    foreach ($names as $name) {
        if (!is_string($name) || preg_match('/\A[A-Za-z]{2,8}[0-9]{0,2}\z/D', $name) !== 1) {
            throw new RuntimeException('Entity fixture data contains an invalid name');
        }
    }
    $profile = ['context' => 'html', 'names_count' => count($names),
        'source_sha256' => $entity_source_sha256,
        'ordered_names_sha256' => hash('sha256', json_encode($names, JSON_THROW_ON_ERROR))];
    if ($profile !== ['context' => 'html', 'names_count' => 253,
        'source_sha256' => 'ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d',
        'ordered_names_sha256' => '60c1fcc052465d98ef468f0a6080aa422a457d5e77c9b7d8d72d40179c21c512']) {
        throw new RuntimeException('Entity fixture default HTML profile differs');
    }
    return $profile;
}
if (array_intersect_key($entity_data, default_html_entity_profile()) !== default_html_entity_profile()) {
    throw new RuntimeException('Entity fixture artifact profile differs');
}
function oracle_normalize_default_html_entities($input, $context) {
    global $default_html_entity_names;
    if (!is_string($input) || $context !== 'html') {
        throw new RuntimeException('Entity fixture requires byte-string input and explicit HTML context');
    }
    default_html_entity_profile();
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('Entity fixture could not reset PCRE state');
    }
    $had_names = array_key_exists('allowedentitynames', $GLOBALS);
    $saved_names = $GLOBALS['allowedentitynames'] ?? null;
    try {
        $GLOBALS['allowedentitynames'] = $default_html_entity_names;
        default_html_entity_profile($GLOBALS['allowedentitynames']);
        $result = wp_kses_normalize_entities($input, 'html');
        if (preg_last_error() !== PREG_NO_ERROR || !is_string($result)) {
            throw new RuntimeException('Entity fixture encountered a PCRE error or nonstring result');
        }
        return $result;
    } finally {
        if ($had_names) {
            $GLOBALS['allowedentitynames'] = $saved_names;
        } else {
            unset($GLOBALS['allowedentitynames']);
        }
    }
}
function oracle_normalize_xml_utf8_entities($input, $context) {
    global $default_html_entity_names;
    // Type/context admission precedes any charset/global mutation.
    if (!is_string($input) || !is_string($context) || $context !== 'xml') {
        throw new RuntimeException('XML entity fixture requires bytes and literal XML context');
    }
    default_html_entity_profile();
    $charset = ini_get('default_charset');
    if (!is_string($charset)) { throw new RuntimeException('missing default charset'); }
    $had_html = array_key_exists('allowedentitynames', $GLOBALS);
    $saved_html = $GLOBALS['allowedentitynames'] ?? null;
    $had_xml = array_key_exists('allowedxmlentitynames', $GLOBALS);
    $saved_xml = $GLOBALS['allowedxmlentitynames'] ?? null;
    try {
        if (ini_set('default_charset', 'UTF-8') === false || ini_get('default_charset') !== 'UTF-8') {
            throw new RuntimeException('XML entity UTF-8 setup failed');
        }
        $GLOBALS['allowedentitynames'] = $default_html_entity_names;
        $GLOBALS['allowedxmlentitynames'] = ['amp', 'lt', 'gt', 'apos', 'quot'];
        if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('XML entity PCRE reset failed');
        }
        $output = wp_kses_normalize_entities($input, 'xml');
        if (!is_string($output) || preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('XML entity PCRE error or nonstring output');
        }
        return $output;
    } finally {
        if ($had_html) { $GLOBALS['allowedentitynames'] = $saved_html; } else { unset($GLOBALS['allowedentitynames']); }
        if ($had_xml) { $GLOBALS['allowedxmlentitynames'] = $saved_xml; } else { unset($GLOBALS['allowedxmlentitynames']); }
        if (ini_set('default_charset', $charset) === false || ini_get('default_charset') !== $charset) {
            throw new RuntimeException('XML entity charset restore failed');
        }
    }
}
function xml_utf8_entity_profile() {
    global $default_html_entity_names, $entity_source_sha256;
    $pairs = [];
    foreach ($default_html_entity_names as $name) {
        $pairs[] = [$name, bin2hex(oracle_normalize_xml_utf8_entities('&' . $name . ';', 'xml'))];
    }
    $profile = ['context' => 'xml', 'charset' => 'UTF-8', 'php_version' => PHP_VERSION,
        'html5_flag' => ENT_HTML5, 'html_names_count' => 253, 'xml_names_count' => 5,
        'source_sha256' => $entity_source_sha256,
        'ordered_names_sha256' => default_html_entity_profile()['ordered_names_sha256'],
        'mapping_sha256' => hash('sha256', json_encode($pairs, JSON_THROW_ON_ERROR)),
        'php_binary_sha256' => hash_file('sha256', PHP_BINARY)];
    if ($profile !== ['context' => 'xml', 'charset' => 'UTF-8', 'php_version' => '8.5.12',
        'html5_flag' => 48, 'html_names_count' => 253, 'xml_names_count' => 5,
        'source_sha256' => 'ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d',
        'ordered_names_sha256' => '60c1fcc052465d98ef468f0a6080aa422a457d5e77c9b7d8d72d40179c21c512',
        'mapping_sha256' => '6810e6d66a468cf2a7b75a0969b932fd727553dc7141090bd837d6023331c25b',
        'php_binary_sha256' => '02273af2cb60375c95d1a4d9244fd1bb8abad898e63055235a06ee36af0380ef']) {
        throw new RuntimeException('XML UTF-8 actual observation profile differs');
    }
    return $profile;
}
function oracle_specialchars_utf8($input, $style, $double_encode) {
    global $default_html_entity_names;
    $styles = ['noquotes' => ENT_NOQUOTES, 'compat' => ENT_COMPAT, 'quotes' => ENT_QUOTES,
        'single' => 'single', 'double' => 'double', 'xml1' => ENT_XML1];
    if (!is_string($input) || !is_string($style) || !array_key_exists($style, $styles) || !is_bool($double_encode)) {
        throw new RuntimeException('Specialchars fixture requires bytes, selected style and explicit boolean double_encode');
    }
    default_html_entity_profile();
    $charset = ini_get('default_charset');
    if (!is_string($charset)) { throw new RuntimeException('missing default charset'); }
    $had_html = array_key_exists('allowedentitynames', $GLOBALS);
    $saved_html = $GLOBALS['allowedentitynames'] ?? null;
    $had_xml = array_key_exists('allowedxmlentitynames', $GLOBALS);
    $saved_xml = $GLOBALS['allowedxmlentitynames'] ?? null;
    try {
        if (ini_set('default_charset', 'UTF-8') === false || ini_get('default_charset') !== 'UTF-8') {
            throw new RuntimeException('Specialchars UTF-8 setup failed');
        }
        $GLOBALS['allowedentitynames'] = $default_html_entity_names;
        $GLOBALS['allowedxmlentitynames'] = ['amp', 'lt', 'gt', 'apos', 'quot'];
        if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('Specialchars PCRE reset failed');
        }
        // Explicit nonempty UTF-8: _canonical_charset never needs get_option.
        $output = _wp_specialchars($input, $styles[$style], 'UTF-8', $double_encode);
        if (!is_string($output) || preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('Specialchars PCRE error or nonstring output');
        }
        return $output;
    } finally {
        if ($had_html) { $GLOBALS['allowedentitynames'] = $saved_html; } else { unset($GLOBALS['allowedentitynames']); }
        if ($had_xml) { $GLOBALS['allowedxmlentitynames'] = $saved_xml; } else { unset($GLOBALS['allowedxmlentitynames']); }
        if (ini_set('default_charset', $charset) === false || ini_get('default_charset') !== $charset) {
            throw new RuntimeException('Specialchars charset restore failed');
        }
    }
}
function specialchars_utf8_profile() {
    global $root;
    return ['input_type' => 'bytes', 'charset' => 'UTF-8', 'default_charset_during_call' => 'UTF-8',
        'php_version' => PHP_VERSION, 'quote_noquotes' => ENT_NOQUOTES, 'quote_compat' => ENT_COMPAT,
        'quote_quotes' => ENT_QUOTES, 'quote_single' => 'single', 'quote_double' => 'double', 'quote_xml1' => ENT_XML1,
        'double_encode_type' => 'explicit_boolean', 'utf8_check' => 'original_special_guard_then_empty',
        'source_sha256' => hash_file('sha256', $root . '/wp-includes/formatting.php'),
        'entity_source_sha256' => default_html_entity_profile()['source_sha256'],
        'ordered_names_sha256' => default_html_entity_profile()['ordered_names_sha256'],
        'xml_mapping_sha256' => xml_utf8_entity_profile()['mapping_sha256']];
}
$empty_filter_hook_counts = ['esc_html' => 0, 'attribute_escape' => 0];
function oracle_escape_utf8_empty_filters($api, $input, $profile) {
    global $default_html_entity_names, $empty_filter_hook_counts;
    if (!is_string($input) || !in_array($api, ['esc_html', 'esc_attr'], true) || $profile !== 'empty') {
        throw new RuntimeException('Escaping fixture requires bytes and explicit empty filter profile');
    }
    default_html_entity_profile();
    $saved = [];
    foreach (['wp_filter', 'wp_filters', 'wp_current_filter', 'allowedentitynames', 'allowedxmlentitynames'] as $name) {
        $saved[$name] = ['had' => array_key_exists($name, $GLOBALS), 'value' => $GLOBALS[$name] ?? null];
    }
    $had_actions = array_key_exists('wp_actions', $GLOBALS);
    $saved_actions = $GLOBALS['wp_actions'] ?? null;
    $charset = ini_get('default_charset');
    if (!is_string($charset)) { throw new RuntimeException('missing default charset'); }
    try {
        if (ini_set('default_charset', 'UTF-8') === false || ini_get('default_charset') !== 'UTF-8') {
            throw new RuntimeException('Escaping UTF-8 setup failed');
        }
        foreach (['wp_filter', 'wp_filters', 'wp_current_filter'] as $name) { $GLOBALS[$name] = []; }
        $GLOBALS['allowedentitynames'] = $default_html_entity_names;
        $GLOBALS['allowedxmlentitynames'] = ['amp', 'lt', 'gt', 'apos', 'quot'];
        if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) { throw new RuntimeException('Escaping PCRE reset failed'); }
        $output = $api($input);
        $hook = $api === 'esc_html' ? 'esc_html' : 'attribute_escape';
        if (!is_string($output) || preg_last_error() !== PREG_NO_ERROR
            || $GLOBALS['wp_filter'] !== [] || $GLOBALS['wp_current_filter'] !== []
            || $GLOBALS['wp_filters'] !== [$hook => 1]
            || array_key_exists('wp_actions', $GLOBALS) !== $had_actions
            || ($had_actions && $GLOBALS['wp_actions'] !== $saved_actions)) {
            throw new RuntimeException('Escaping result or empty-hook bookkeeping differs');
        }
        ++$empty_filter_hook_counts[$hook];
        return $output;
    } finally {
        foreach ($saved as $name => $old) {
            if ($old['had']) { $GLOBALS[$name] = $old['value']; } else { unset($GLOBALS[$name]); }
        }
        if (ini_set('default_charset', $charset) === false || ini_get('default_charset') !== $charset) {
            throw new RuntimeException('Escaping charset restore failed');
        }
    }
}
function empty_filter_escaping_profile() {
    global $root;
    if (get_option('blog_charset') !== 'UTF-8') { throw new RuntimeException('Escaping requires fixed site UTF-8 profile'); }
    return ['input_type' => 'bytes', 'blog_charset' => get_option('blog_charset'),
        'option_profile' => 'immutable_UTF-8_for_process', 'mb_convert_encoding_exists' => function_exists('mb_convert_encoding'),
        'filter_registry' => 'empty', 'native_filter_bookkeeping' => false,
        'html_hook' => 'esc_html', 'attr_hook' => 'attribute_escape', 'quote_style' => ENT_QUOTES,
        'double_encode' => false, 'utf8_check' => 'whole_input_strip_false_before_special_guard',
        'default_charset_during_call' => 'UTF-8', 'source_sha256' => hash_file('sha256', $root . '/wp-includes/formatting.php'),
        'entity_source_sha256' => default_html_entity_profile()['source_sha256'],
        'ordered_names_sha256' => default_html_entity_profile()['ordered_names_sha256']];
}
$remove_accents_calls=0; $remove_accents_get_locale_calls=0;
$locale_db_branch_calls=0;
function is_multisite(){global$locale_db_branch_calls;++$locale_db_branch_calls;throw new RuntimeException('Locale DB branch is outside the fixture');}
function wp_installing(){global$locale_db_branch_calls;++$locale_db_branch_calls;throw new RuntimeException('Locale installation branch is outside the fixture');}
function get_site_option(...$unused){global$locale_db_branch_calls;++$locale_db_branch_calls;throw new RuntimeException('Locale site option is outside the fixture');}
function oracle_remove_accents(#[\SensitiveParameter] $text, #[\SensitiveParameter] $locale, #[\SensitiveParameter] $profile) {
    global $remove_accents_calls,$remove_accents_get_locale_calls;
    if(!is_string($text)||!is_string($locale)||$locale===''||$locale==='0'||$profile!=='valid_utf8_explicit_locale_no_intl'
        ||!wp_is_valid_utf8($text)||extension_loaded('mbstring')||extension_loaded('intl')
        ||function_exists('normalizer_is_normalized')||function_exists('normalizer_normalize')) {
        throw new RuntimeException('Unsupported explicit accent profile/input');
    }
    $before=did_filter('locale');
    preg_match('//','');
    $result=remove_accents($text,$locale);
    $delta=did_filter('locale')-$before;
    $remove_accents_get_locale_calls+=$delta;
    if(!is_string($result)||preg_last_error()!==PREG_NO_ERROR||$delta!==0
        ||strlen($result)>65536){throw new RuntimeException('Accent reference diagnostic/host bound');}
    ++$remove_accents_calls;
    return $result;
}
function remove_accents_profile() {
    global $root;
    return ['php_version'=>PHP_VERSION,'text_type'=>'bytes_valid_utf8','locale_type'=>'bytes_php_truthy',
        'intl_loaded'=>extension_loaded('intl'),'normalizer_is_normalized_exists'=>function_exists('normalizer_is_normalized'),
        'normalizer_normalize_exists'=>function_exists('normalizer_normalize'),'mbstring_loaded'=>extension_loaded('mbstring'),
        'locale_rules'=>'de_prefix/da_DK_exact/ca_exact/sr_RS_exact/bs_BA_exact/other_base','nfc_normalization'=>false,
        'get_locale_calls'=>0,'latin1_fallback_supported'=>false,'source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php'),
        'base_count'=>311,'base_ordered_pairs_sha256'=>'69b78d9596b1e927213e72b0698a40a85daa76e8846a4d86d7a13c78d4594477',
        'locale_override_ordered_pairs_sha256'=>'07c40351a07697a814d6a97ec45c2549b7f08071f3ff3cc33768fa0285e6932c'];
}
$strip_all_tags_calls=0;$sanitize_user_calls=0;$sanitize_user_filter_calls=0;
$sanitize_user_locale_calls=['zero'=>0,'one'=>0];
function oracle_strip_all_tags(#[\SensitiveParameter] $raw,#[\SensitiveParameter] $remove_breaks,#[\SensitiveParameter] $profile){
    global$strip_all_tags_calls;
    if(!is_string($raw)||!is_bool($remove_breaks)||$profile!=='no_literal_lt'||str_contains($raw,'<')){
        throw new RuntimeException('Unsupported NoLiteral< strip profile/input');
    }
    preg_match('//','');
    try{$result=wp_strip_all_tags($raw,$remove_breaks);}catch(Throwable$unused){throw new RuntimeException('Strip reference failed');}
    if(!is_string($result)||preg_last_error()!==PREG_NO_ERROR||strlen($result)>65536){throw new RuntimeException('Strip reference diagnostic/host bound');}
    ++$strip_all_tags_calls;return$result;
}
function oracle_sanitize_user(#[\SensitiveParameter] $raw,#[\SensitiveParameter] $strict,#[\SensitiveParameter] $resolved_locale,#[\SensitiveParameter] $profile){
    global$sanitize_user_calls,$sanitize_user_filter_calls,$sanitize_user_locale_calls,$locale_db_branch_calls;
    if(!is_string($raw)||!is_bool($strict)||!is_string($resolved_locale)||$resolved_locale===''||$resolved_locale==='0'
        ||$profile!=='no_lt_utf8_resolved_locale_c_empty_filters'||str_contains($raw,'<')||setlocale(LC_CTYPE,0)!=='C'
        ||extension_loaded('mbstring')||extension_loaded('intl')||function_exists('normalizer_is_normalized')||function_exists('normalizer_normalize')){
        throw new RuntimeException('Unsupported NoLiteral< user profile/input');
    }
    preg_match('//','');
    try{$stripped=wp_strip_all_tags($raw,false);}catch(Throwable$unused){throw new RuntimeException('User strip preflight failed');}
    if(!is_string($stripped)||preg_last_error()!==PREG_NO_ERROR||!wp_is_valid_utf8($stripped)){
        throw new RuntimeException('Post-strip UTF8 is outside the user subset');
    }
    $expected_locale=preg_match('/[\x80-\xff]/',$stripped);
    if($expected_locale===false||preg_last_error()!==PREG_NO_ERROR){throw new RuntimeException('User locale classification failed');}
    $saved=[];foreach(['wp_filter','wp_filters','wp_current_filter','locale','wp_local_package']as$name){
        $saved[$name]=['had'=>array_key_exists($name,$GLOBALS),'value'=>$GLOBALS[$name]??null];
    }
    $untouched=[];foreach(['wp_actions','wp_hasher']as$name){$untouched[$name]=['had'=>array_key_exists($name,$GLOBALS),'value'=>$GLOBALS[$name]??null];}
    $db_before=$locale_db_branch_calls;
    try{
        $GLOBALS['wp_filter']=[];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];$GLOBALS['locale']=$resolved_locale;
        preg_match('//','');
        try{$result=sanitize_user($raw,$strict);}catch(Throwable$unused){throw new RuntimeException('User reference failed');}
        $hooks=$expected_locale===1?['locale'=>1,'sanitize_user'=>1]:['sanitize_user'=>1];
        if(!is_string($result)||preg_last_error()!==PREG_NO_ERROR||strlen($result)>65536||$GLOBALS['wp_filter']!==[]
            ||$GLOBALS['wp_filters']!==$hooks||$GLOBALS['wp_current_filter']!==[]||$GLOBALS['locale']!==$resolved_locale
            ||$locale_db_branch_calls!==$db_before){throw new RuntimeException('User actual empty-filter scope differs');}
        foreach($untouched as$name=>$old){if(array_key_exists($name,$GLOBALS)!==$old['had']||($old['had']&&$GLOBALS[$name]!==$old['value'])){throw new RuntimeException('User actions/hasher changed');}}
        ++$sanitize_user_calls;++$sanitize_user_filter_calls;++$sanitize_user_locale_calls[$expected_locale===1?'one':'zero'];
        return$result;
    }finally{
        foreach($saved as$name=>$old){if($old['had']){$GLOBALS[$name]=$old['value'];}else{unset($GLOBALS[$name]);}}
    }
}
function strip_all_tags_profile(){global$root;return['php_version'=>PHP_VERSION,'input_type'=>'bytes_no_literal_lt','remove_breaks_type'=>'bool',
    'utf8_validation'=>false,'stages'=>'delete_nul/optional_cr_lf_tab_space_collapse/php_trim6','source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php')];}
function sanitize_user_profile(){global$root;return['php_version'=>PHP_VERSION,'input_type'=>'bytes_no_literal_lt_post_strip_valid_utf8','strict_type'=>'bool',
    'locale_type'=>'bytes_php_truthy','locale_resolution'=>'scoped_global_first_isset_branch','filter_registry'=>'empty','native_filter_bookkeeping'=>false,
    'ctype_locale'=>setlocale(LC_CTYPE,0),'intl_loaded'=>extension_loaded('intl'),'normalizer_is_normalized_exists'=>function_exists('normalizer_is_normalized'),
    'normalizer_normalize_exists'=>function_exists('normalizer_normalize'),'mbstring_loaded'=>extension_loaded('mbstring'),
    'l10n_source_sha256'=>hash_file('sha256',$root.'/wp-includes/l10n.php'),'latin1_fallback_supported'=>false,
    'source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php'),'base_count'=>311,
    'base_ordered_pairs_sha256'=>'69b78d9596b1e927213e72b0698a40a85daa76e8846a4d86d7a13c78d4594477',
    'locale_override_ordered_pairs_sha256'=>'07c40351a07697a814d6a97ec45c2549b7f08071f3ff3cc33768fa0285e6932c'];}
$wp_rehash_hook_counts = array_fill_keys(['wp_hash_password_algorithm','wp_hash_password_options','password_needs_rehash'], 0);
function oracle_wp_rehash(#[\SensitiveParameter] $hash, #[\SensitiveParameter] $profile) {
    global $wp_rehash_hook_counts;
    if (!is_string($hash) || $profile !== 'default_bcrypt_empty') { throw new RuntimeException('Rehash requires Bytes and explicit default empty profile'); }
    if (str_starts_with($hash, '$wp')) {
        if (strlen($hash) !== 63 || !str_starts_with($hash, '$wp$2y$') || $hash[9] !== '$') { throw new RuntimeException('Unsupported prefixed rehash input'); }
        for ($i=0; $i<63; ++$i) { if (ord($hash[$i]) > 127) { throw new RuntimeException('Unsupported prefixed rehash input'); } }
        $a=ord($hash[7])-48; $b=ord($hash[8])-48;
        if ($a<0 || $a>9 || $b<0 || $b>9 || 10*$a+$b<4 || 10*$a+$b>31) { throw new RuntimeException('Unsupported prefixed rehash cost'); }
        $alphabet='./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
        if (strspn(substr($hash,10),$alphabet)!==53 || strpos($alphabet,$hash[31])%16!==0 || strpos($alphabet,$hash[62])%4!==0) {
            throw new RuntimeException('Unsupported noncanonical prefixed rehash input');
        }
    }
    $names=['wp_filter','wp_filters','wp_current_filter','wp_hasher']; $saved=[];
    foreach($names as$name){$saved[$name]=['had'=>array_key_exists($name,$GLOBALS),'value'=>$GLOBALS[$name]??null];}
    $had_actions=array_key_exists('wp_actions',$GLOBALS);$actions=$GLOBALS['wp_actions']??null;
    $hooks=['wp_hash_password_algorithm','wp_hash_password_options','password_needs_rehash'];
    try {
        $GLOBALS['wp_filter']=[];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];$GLOBALS['wp_hasher']=null;
        $result=wp_password_needs_rehash($hash,0);
        if(!is_bool($result)||$GLOBALS['wp_filter']!==[]||$GLOBALS['wp_filters']!==array_fill_keys($hooks,1)
            ||$GLOBALS['wp_current_filter']!==[]||$GLOBALS['wp_hasher']!==null
            ||array_key_exists('wp_actions',$GLOBALS)!==$had_actions||($had_actions&&$GLOBALS['wp_actions']!==$actions)) {
            throw new RuntimeException('Rehash empty callback scope differs');
        }
        foreach($hooks as$hook){++$wp_rehash_hook_counts[$hook];}
        return $result;
    } finally {
        foreach($saved as$name=>$old){if($old['had']){$GLOBALS[$name]=$old['value'];}else{unset($GLOBALS[$name]);}}
    }
}
function wp_rehash_profile() {
    global $root;
    $raw='$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C';
    $info=password_get_info($raw);
    $confirmed=PASSWORD_BCRYPT==='2y' && $info['algo']==='2y' && $info['options']['cost']===12
        && password_needs_rehash($raw,PASSWORD_BCRYPT,[])===false
        && password_needs_rehash(substr_replace($raw,'11',4,2),PASSWORD_BCRYPT,[])===true
        && password_needs_rehash(substr_replace($raw,'13',4,2),PASSWORD_BCRYPT,[])===true;
    if(!$confirmed){throw new RuntimeException('Default bcrypt metadata differs');}
    return ['php_version'=>PHP_VERSION,'hash_type'=>'bytes','custom_hasher'=>'empty','filter_registry'=>'empty',
        'user_id'=>0,'algorithm'=>PASSWORD_BCRYPT,'options'=>'empty','default_cost'=>$info['options']['cost'],
        'default_cost_confirmed'=>$confirmed,'nonprefixed'=>'arbitrary_bytes_true','prefixed'=>'canonical_wp_2y_63_cost4_to31',
        'native_filter_bookkeeping'=>false,'bcrypt_version'=>'0.19.3','source_sha256'=>hash_file('sha256',$root.'/wp-includes/pluggable.php')];
}
$wp_bcrypt_calls = 0;
function oracle_wp_bcrypt_verify(#[\SensitiveParameter] $password, #[\SensitiveParameter] $hash, $maximum, $profile) {
    global $wp_bcrypt_calls;
    if (!is_string($password) || !is_string($hash) || !is_int($maximum) || $maximum < 4 || $maximum > 31 || $profile !== 'wp_bcrypt_empty') {
        throw new RuntimeException('WP bcrypt fixture requires Bytes, typed host cost and explicit empty profile');
    }
    if (strlen($hash) !== 63 || preg_match('/[^\x00-\x7f]/', $hash) !== 0 || !str_starts_with($hash, '$wp$2y$') || $hash[9] !== '$') {
        throw new RuntimeException('Unsupported WP bcrypt hash');
    }
    if (strlen($password) <= 4096) {
        if (preg_match('/\A[0-9]{2}\z/D', substr($hash, 7, 2)) !== 1) { throw new RuntimeException('Unsupported WP bcrypt cost'); }
        $cost=(int)substr($hash,7,2);
        if ($cost < 4 || $cost > 31) { throw new RuntimeException('Unsupported WP bcrypt cost'); }
        if ($cost > $maximum) { throw new RuntimeException('WP bcrypt host CostLimit'); }
        $alphabet='./ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
        if (preg_match('/\A[.\/A-Za-z0-9]{53}\z/D', substr($hash,10)) !== 1
            || strpos($alphabet,$hash[31]) % 16 !== 0 || strpos($alphabet,$hash[62]) % 4 !== 0) {
            throw new RuntimeException('Unsupported noncanonical WP bcrypt hash');
        }
    }
    $saved=[];
    foreach (['wp_filter','wp_filters','wp_current_filter','wp_hasher'] as $name) { $saved[$name]=['had'=>array_key_exists($name,$GLOBALS),'value'=>$GLOBALS[$name]??null]; }
    $had_actions=array_key_exists('wp_actions',$GLOBALS);$actions=$GLOBALS['wp_actions']??null;
    try {
        $GLOBALS['wp_filter']=[];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];$GLOBALS['wp_hasher']=null;
        $result=wp_check_password($password,$hash,0);
        if (!is_bool($result) || $GLOBALS['wp_filter']!==[] || $GLOBALS['wp_filters']!==['check_password'=>1]
            || $GLOBALS['wp_current_filter']!==[] || $GLOBALS['wp_hasher']!==null
            || array_key_exists('wp_actions',$GLOBALS)!==$had_actions || ($had_actions&&$GLOBALS['wp_actions']!==$actions)) {
            throw new RuntimeException('WP bcrypt empty callback scope differs');
        }
        ++$wp_bcrypt_calls;
        return $result;
    } finally {
        foreach ($saved as $name=>$old) { if ($old['had']) { $GLOBALS[$name]=$old['value']; } else { unset($GLOBALS[$name]); } }
    }
}
function wp_bcrypt_profile() {
    global $root;
    $vectors=[['empty','$wp$2y$04$......................95Tr8E4rK4vqpGi1aEzSvD8w4ihtXuy'],['dummy','$wp$2y$04$......................p4I2i5y/qZ1mBr4uEKqha7UHXK0Clmq'],['binary','$wp$2y$04$......................S3bG01U.z0mtO6foDhFLmZTx5V6GN5K'],['spaced','$wp$2y$04$......................hl8fSCoR6uBYSLfV/VnjGvO8VRxZaWa'],['max4096','$wp$2y$04$......................PJ0SHzxZ5XVAIaQ.o5XGE7.jlbWF7K2'],['cost12','$wp$2y$12$abcdefghijklmnopqrstuuMG6jJyGou4KCYl4.uqa/dV0lxY1o35C']];
    return ['php_version'=>PHP_VERSION,'password_type'=>'bytes','hash_type'=>'bytes','hash_format'=>'$wp$2y$',
        'hash_bytes'=>63,'password_max_bytes'=>4096,'prehash'=>'HMAC-SHA384/base64_standard','custom_hasher'=>'empty','filter_registry'=>'empty',
        'hook'=>'check_password','user_id'=>0,'native_filter_bookkeeping'=>false,'host_max_cost'=>'required_4_to_31',
        'source_sha256'=>hash_file('sha256',$root.'/wp-includes/pluggable.php'),
        'bcrypt_version'=>'0.19.3','hmac_version'=>'0.12.1','sha2_version'=>'0.10.9','base64_version'=>'0.22.1','zeroize_version'=>'1.9.1',
        'dummy_vectors_sha256'=>hash('sha256',json_encode($vectors,JSON_UNESCAPED_SLASHES|JSON_THROW_ON_ERROR))];
}
load_reference_function($root . '/wp-includes/functions.php', 'wp_validate_boolean');
load_reference_function($root . '/wp-includes/functions.php', 'wp_check_jsonp_callback');
load_reference_function($root . '/wp-includes/functions.php', 'wp_is_uuid');
load_reference_function($root . '/wp-includes/functions.php', 'wp_unique_id');
load_reference_function($root . '/wp-includes/functions.php', 'wp_unique_prefixed_id');
load_reference_function($root . '/wp-includes/functions.php', 'wp_json_encode');
foreach (['_wp_json_sanity_check', '_wp_json_convert_string', 'is_utf8_charset'] as $symbol) {
    load_reference_function($root . '/wp-includes/functions.php', $symbol);
}
load_reference_function($root . '/wp-includes/compat.php', '_is_utf8_charset');
load_reference_function($root . '/wp-includes/formatting.php', 'wp_check_invalid_utf8');
load_reference_function($root . '/wp-includes/functions.php', 'wp_is_numeric_array');
load_reference_function($root . '/wp-includes/functions.php', 'wp_parse_list');
load_reference_function($root . '/wp-includes/functions.php', 'wp_array_slice_assoc');
load_reference_function($root . '/wp-includes/functions.php', 'wp_parse_args');
load_reference_function($root . '/wp-includes/formatting.php', 'wp_parse_str');
load_reference_function($root . '/wp-includes/functions.php', 'wp_list_pluck');
load_reference_function($root . '/wp-includes/functions.php', 'wp_is_stream');
load_reference_function($root . '/wp-includes/functions.php', 'wp_normalize_path');
load_reference_function($root . '/wp-includes/functions.php', 'wp_checkdate');
load_reference_function($root . '/wp-includes/functions.php', 'is_serialized');
load_reference_function($root . '/wp-includes/functions.php', 'is_serialized_string');
load_reference_function($root . '/wp-includes/functions.php', 'maybe_serialize');
load_reference_function($root . '/wp-includes/functions.php', 'maybe_unserialize');
load_reference_function($root . '/wp-includes/load.php', 'absint');
load_reference_function($root . '/wp-includes/load.php', 'wp_is_json_media_type');
load_reference_function($root . '/wp-includes/load.php', 'wp_is_json_request');
load_reference_function($root . '/wp-includes/load.php', 'wp_is_xml_request');
load_reference_function($root . '/wp-includes/load.php', 'wp_convert_hr_to_bytes');
load_reference_function($root . '/wp-includes/functions.php', 'wp_parse_id_list');
load_reference_function($root . '/wp-includes/formatting.php', 'trailingslashit');
load_reference_function($root . '/wp-includes/formatting.php', 'untrailingslashit');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_key');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_html_class');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_locale_name');
load_reference_function($root . '/wp-includes/formatting.php', 'sanitize_mime_type');
load_reference_function($root . '/wp-includes/formatting.php', 'is_email');
foreach (['__checked_selected_helper', 'checked', 'selected', 'disabled', 'wp_readonly'] as $symbol) {
    load_reference_function($root . '/wp-includes/general-template.php', $symbol);
}
load_reference_function($root . '/wp-includes/formatting.php', 'wp_basename');
load_reference_function($root . '/wp-includes/formatting.php', 'normalize_whitespace');
load_reference_function($root . '/wp-includes/formatting.php', 'get_html_split_regex');
load_reference_function($root . '/wp-includes/formatting.php', 'wp_html_split');
load_reference_function($root . '/wp-includes/formatting.php', 'wp_replace_in_html_tags');
load_reference_function($root . '/wp-includes/formatting.php', 'wp_make_link_relative');
load_reference_function($root . '/wp-includes/formatting.php', 'url_shorten');
load_reference_function($root . '/wp-includes/formatting.php', 'zeroise');
foreach (['sanitize_hex_color', 'sanitize_hex_color_no_hash', 'maybe_hash_hex_color'] as $symbol) {
    load_reference_function($root . '/wp-includes/formatting.php', $symbol);
}
foreach (['map_deep', 'stripslashes_deep', 'stripslashes_from_strings_only', 'wp_slash', 'wp_unslash'] as $symbol) {
    load_reference_function($root . '/wp-includes/formatting.php', $symbol);
}

require __DIR__ . '/php_value_codec.php';
require __DIR__ . '/post_meta_oracle_profile.php';
post_meta_cached_load_reference_functions($root);
require __DIR__ . '/texturize_oracle_profile.php';
texturize_load_reference_functions($root);
require __DIR__ . '/title_oracle_profile.php';
title_load_reference_functions($root);
require __DIR__ . '/auth_cookie_oracle_profile.php';
auth_cookie_load_reference_functions($root);
require __DIR__ . '/auth_cookie_parse_oracle_profile.php';
auth_cookie_parse_load_reference_functions($root);
require __DIR__ . '/sanitize_text_oracle_profile.php';
sanitize_text_load_reference_functions($root);

// Explicit isolated fixture option, not a WordPress database/bootstrap.
function get_option(#[\SensitiveParameter] $name) {
    if (auth_cookie_active()) {
        throw new RuntimeException('Auth cookie option resolution forbidden');
    }
    if (($GLOBALS['title_scope']['active'] ?? false) === true) {
        ++$GLOBALS['title_scope']['external_calls'];
        throw new RuntimeException('Prepared title option access forbidden');
    }
    if ($name !== 'blog_charset') {
        global$locale_db_branch_calls;++$locale_db_branch_calls;
        throw new RuntimeException('JSON fixture requested an unmodeled option');
    }
    return 'UTF-8';
}

// Fixed default-unit value profile only. Do not call wp_initial_constants():
// its other work reads/selects ini and memory settings outside this oracle.
foreach (['KB_IN_BYTES' => 1024, 'MB_IN_BYTES' => 1048576, 'GB_IN_BYTES' => 1073741824] as $name => $value) {
    if (!defined($name)) {
        define($name, $value);
    }
}
function hr_byte_units_profile() {
    $units = ['KB_IN_BYTES' => KB_IN_BYTES, 'MB_IN_BYTES' => MB_IN_BYTES, 'GB_IN_BYTES' => GB_IN_BYTES];
    if ($units !== ['KB_IN_BYTES' => 1024, 'MB_IN_BYTES' => 1048576, 'GB_IN_BYTES' => 1073741824]) {
        throw new RuntimeException('Byte conversion fixture requires exact integer default units');
    }
    return $units;
}
hr_byte_units_profile();

function oracle_convert_hr_bytes($input) {
    if (!is_string($input) || preg_match('/\A[0-9]+[kKmMgG]?\z/D', $input) !== 1) {
        throw new RuntimeException('Byte conversion fixture requires ASCII digits and an optional single K M G suffix, at most 256 significant digits');
    }
    $digits = str_contains('kKmMgG', $input[strlen($input) - 1]) ? substr($input, 0, -1) : $input;
    if (strlen(ltrim($digits, '0')) > 256) {
        throw new RuntimeException('Byte conversion fixture requires ASCII digits and an optional single K M G suffix, at most 256 significant digits');
    }
    $result = wp_convert_hr_to_bytes($input);
    if (!is_int($result)) {
        throw new RuntimeException('Byte conversion fixture expected an integer result');
    }
    return $result;
}

function json_utf8_fallback_profile() {
    return ['blog_charset' => get_option('blog_charset'),
        'mb_convert_encoding_exists' => function_exists('mb_convert_encoding'),
        'is_utf8_charset' => is_utf8_charset()];
}

function oracle_parse_str($input) {
    wp_parse_str($input, $result);
    return $result;
}

function oracle_is_json_media_type($input) {
    // The real function memoizes bools. Reset PCRE's process error state even
    // for cache hits, and never count a failed match operation as false success.
    if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('JSON media type fixture could not reset PCRE state');
    }
    $result = wp_is_json_media_type($input);
    if (preg_last_error() !== PREG_NO_ERROR) {
        throw new RuntimeException('JSON media type fixture encountered a PCRE error');
    }
    return $result;
}

function oracle_media_request($case, $input, $json) {
    if ($input !== null) {
        throw new RuntimeException('Media request fixture requires a null input sentinel');
    }
    $mapping = ['accept' => 'HTTP_ACCEPT', 'content_type' => 'CONTENT_TYPE'];
    $headers = [];
    // Validate BOTH fields before any server mutation or short-circuit matching.
    foreach ($mapping as $field => $name) {
        if (array_key_exists($field, $case)) {
            $header = decode_value($case[$field]);
            if ($header !== null && !is_string($header)) {
                throw new RuntimeException('Media request fixture requires byte-string or null headers');
            }
            $headers[$field] = $header;
        }
    }
    $saved = [];
    foreach ($mapping as $name) {
        $saved[$name] = [array_key_exists($name, $_SERVER), $_SERVER[$name] ?? null];
    }
    try {
        foreach ($mapping as $field => $name) {
            if (array_key_exists($field, $headers)) {
                $_SERVER[$name] = $headers[$field];
            } else {
                unset($_SERVER[$name]);
            }
        }
        if (!$json) {
            return wp_is_xml_request();
        }
        if (setlocale(LC_CTYPE, 0) !== 'C') {
            throw new RuntimeException('JSON request fixture requires LC_CTYPE=C');
        }
        // Follow the actual function's short-circuit order. Otherwise a failed
        // Accept regex could be masked by a successful Content-Type regex.
        $accepts_json = isset($_SERVER['HTTP_ACCEPT']) && oracle_is_json_media_type($_SERVER['HTTP_ACCEPT']);
        if (!$accepts_json && isset($_SERVER['CONTENT_TYPE'])) {
            oracle_is_json_media_type($_SERVER['CONTENT_TYPE']);
        }
        if (preg_match('//', '') !== 1 || preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('JSON request fixture could not reset PCRE state');
        }
        $result = wp_is_json_request();
        if (preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('JSON request fixture encountered a PCRE error');
        }
        return $result;
    } finally {
        foreach ($saved as $name => [$exists, $value]) {
            if ($exists) {
                $_SERVER[$name] = $value;
            } else {
                unset($_SERVER[$name]);
            }
        }
    }
}

$cases = json_decode(stream_get_contents(STDIN), true, 128, JSON_THROW_ON_ERROR);
$results = [];
foreach ($cases as $case) {
    $value = match ($case['api'] ?? null) {
        'get_post_meta' => post_meta_cached_decode_owner($case),
        'get_the_title' => title_input($case),
        'wp_generate_auth_cookie' => auth_cookie_input($case),
        'wp_parse_auth_cookie' => auth_cookie_parse_input($case),
        'sanitize_text_field', 'sanitize_textarea_field' => sanitize_text_input($case),
        default => decode_value($case['input']),
    };
    if ($case['api'] === 'valid_unicode' && !is_int($value)) {
        throw new RuntimeException('Unicode character fixture requires signed 64-bit integer input');
    }
    if ($case['api'] === 'zeroise') {
        if (!is_int($value) && !is_string($value)) {
            throw new RuntimeException('Zeroise fixture requires integer or byte-string input');
        }
        if (!array_key_exists('width', $case) || !is_int($case['width']) || $case['width'] < 0 || $case['width'] > 4096) {
            throw new RuntimeException('Zeroise fixture requires integer width from 0 through 4096');
        }
    }
    if (in_array($case['api'], ['is_utf8_charset', 'wp_check_invalid_utf8'], true)
        && !is_string($value)) {
        throw new RuntimeException('UTF-8 fixture requires explicit byte-string input');
    }
    if ($case['api'] === 'wp_is_json_media_type' && !is_string($value)) {
        throw new RuntimeException('JSON media type fixture requires explicit byte-string input');
    }
    if ($case['api'] === 'wp_is_json_media_type' && setlocale(LC_CTYPE, 0) !== 'C') {
        throw new RuntimeException('JSON media type fixture requires LC_CTYPE=C');
    }
    if (array_key_exists('utf8_profile', $case)
        && ($case['api'] !== 'wp_json_encode' || $case['utf8_profile'] !== 'utf8_no_mbstring')) {
        throw new RuntimeException('unsupported explicit JSON UTF-8 profile');
    }
    if ($case['api'] === 'wp_json_encode' && array_key_exists('flags', $case)
        && (!is_int($case['flags']) || ($case['flags'] & ~2527) !== 0)) {
        throw new RuntimeException('JSON fixture requires supported integer flags');
    }
    if ($case['api'] === 'wp_check_invalid_utf8' && (!array_key_exists('strip', $case) || !is_bool($case['strip']))) {
        throw new RuntimeException('UTF-8 checking fixture requires explicit boolean strip');
    }
    if (in_array($case['api'], ['checked', 'selected', 'disabled', 'wp_readonly'], true) && ($case['display'] ?? null) !== false) {
        throw new RuntimeException('form attribute comparison requires display=false');
    }
    if ($case['api'] === 'wp_check_jsonp_callback' && setlocale(LC_CTYPE, 0) !== 'C') {
        throw new RuntimeException('JSONP callback fixture requires LC_CTYPE=C');
    }
    if (($case['filter'] ?? '') === 'append_raw') {
        $filter_name = in_array($case['api'], ['wp_checkdate', 'sanitize_html_class', 'sanitize_locale_name', 'sanitize_mime_type'], true) ? $case['api'] : 'sanitize_key';
        add_filter($filter_name, static fn ($clean, $raw) => (is_bool($clean) ? ($clean ? '1' : '0') : $clean) . "\0" . $raw, 10, 2);
    }
    if (($case['filter'] ?? '') === 'constant') {
        $constant = decode_value($case['filter_return']);
        add_filter($case['api'] === 'wp_parse_args' ? 'wp_parse_str' : $case['api'], static fn () => $constant, 10, 0);
    }
    if (($case['filter'] ?? '') === 'class_arguments') {
        add_filter('sanitize_html_class', static fn ($clean, $raw, $fallback) => [$clean, $raw, $fallback], 10, 3);
    }
    if (($case['filter'] ?? '') === 'email_arguments') {
        add_filter('is_email', static fn ($result, $raw, $context) => [$result, $raw, $context], 10, 3);
    }
    $output = match ($case['api']) {
        'get_the_title' => encode_value(oracle_title($value, $case)),
        'wp_generate_auth_cookie' => encode_value(oracle_auth_cookie($value, $case)),
        'wp_parse_auth_cookie' => encode_value(oracle_auth_cookie_parse($value, $case)),
        'sanitize_text_field', 'sanitize_textarea_field' => encode_value(oracle_sanitize_text($value, $case)),
        'wptexturize' => encode_value(oracle_texturize($value, $case)),
        'get_post_meta' => encode_value(oracle_post_meta_cached_single($value, $case)),
        'wp_kses_bad_protocol' => encode_value(oracle_bad_protocol_outer_c($value, array_key_exists('allowed_protocols', $case) ? decode_value($case['allowed_protocols']) : null)),
        'wp_kses_bad_protocol_once' => encode_value(oracle_bad_protocol_once_c($value, array_key_exists('allowed_protocols', $case) ? decode_value($case['allowed_protocols']) : null, array_key_exists('count', $case))),
        'wp_kses_bad_protocol_once2' => encode_value(oracle_bad_protocol_once2_c($value, array_key_exists('allowed_protocols', $case) ? decode_value($case['allowed_protocols']) : null)),
        'wp_kses_stripslashes' => encode_value(oracle_kses_stripslashes_bytes($value)),
        'wp_kses_no_null' => encode_value(oracle_no_null_bytes($value, $case['slash_zero'] ?? null)),
        'wp_kses_decode_entities' => encode_value(oracle_decode_byte_entities($value)),
        'wp_strip_all_tags' => encode_value(oracle_strip_all_tags($value,$case['remove_breaks']??null,$case['strip_profile']??null)),
        'sanitize_user' => encode_value(oracle_sanitize_user($value,$case['strict']??null,decode_value($case['locale']??['type'=>'null']),$case['user_profile']??null)),
        'utf8_uri_encode' => encode_value(oracle_utf8_uri($value,$case['length']??null,$case['encode_ascii']??null,$case['uri_profile']??null)),
        'sanitize_title_with_dashes' => encode_value(oracle_title_dash($value,decode_value($case['raw_title']??['type'=>'null']),$case['context']??null,$case['title_profile']??null)),
        'sanitize_title' => encode_value(oracle_default_sanitize_title($value,decode_value($case['fallback']??['type'=>'null']),$case['context']??null,decode_value($case['locale']??['type'=>'null']),$case['default_title_profile']??null)),
        '_truncate_post_slug' => encode_value(oracle_truncate_slug($value,$case['length']??null,$case['truncate_profile']??null)),
        'remove_accents' => encode_value(oracle_remove_accents($value, decode_value($case['locale'] ?? ['type'=>'null']), $case['accent_profile'] ?? null)),
        'wp_password_needs_rehash' => oracle_wp_rehash($value, $case['rehash_profile'] ?? null),
        'wp_check_password' => oracle_wp_bcrypt_verify($value, decode_value($case['password_hash'] ?? ['type'=>'null']), $case['max_cost'] ?? null, $case['password_profile'] ?? null),
        'esc_html', 'esc_attr' => encode_value(oracle_escape_utf8_empty_filters($case['api'], $value, $case['filter_profile'] ?? null)),
        '_wp_specialchars' => encode_value(oracle_specialchars_utf8($value, $case['quote_style'] ?? null, $case['double_encode'] ?? null)),
        'wp_kses_normalize_entities' => encode_value(($case['context'] ?? null) === 'xml'
            ? oracle_normalize_xml_utf8_entities($value, $case['context'])
            : oracle_normalize_default_html_entities($value, $case['context'] ?? null)),
        'valid_unicode' => valid_unicode($value),
        'wp_convert_hr_to_bytes' => encode_value(oracle_convert_hr_bytes($value)),
        'zeroise' => encode_value(zeroise($value, $case['width'])),
        'wp_is_json_request' => oracle_media_request($case, $value, true),
        'wp_is_xml_request' => oracle_media_request($case, $value, false),
        'wp_is_json_media_type' => oracle_is_json_media_type($value),
        'is_utf8_charset' => is_utf8_charset($value),
        'wp_check_invalid_utf8' => encode_value(wp_check_invalid_utf8($value, $case['strip'])),
        'wp_check_jsonp_callback' => wp_check_jsonp_callback($value),
        'normalize_whitespace' => encode_value(normalize_whitespace($value)),
        'wp_html_split' => encode_value(wp_html_split($value)),
        'wp_replace_in_html_tags' => encode_value(wp_replace_in_html_tags($value, decode_value($case['replacements']))),
        'wp_make_link_relative' => encode_value(wp_make_link_relative($value)),
        'url_shorten' => encode_value(url_shorten($value, (int) $case['length'])),
        'sanitize_hex_color' => encode_value(sanitize_hex_color($value)),
        'sanitize_hex_color_no_hash' => encode_value(sanitize_hex_color_no_hash($value)),
        'maybe_hash_hex_color' => encode_value(maybe_hash_hex_color($value)),
        'wp_is_uuid' => wp_is_uuid($value, $case['version'] === null ? null : (int) $case['version']),
        'wp_validate_boolean' => wp_validate_boolean($value),
        'wp_is_numeric_array' => wp_is_numeric_array($value),
        'maybe_unserialize' => encode_value(maybe_unserialize($value)),
        'maybe_serialize' => encode_value(maybe_serialize($value)),
        'absint' => encode_value(absint($value)),
        'wp_parse_id_list' => encode_value(wp_parse_id_list($value)),
        'is_serialized' => is_serialized($value, $case['strict']),
        'is_serialized_string' => is_serialized_string($value),
        'trailingslashit' => encode_value(trailingslashit($value)),
        'untrailingslashit' => encode_value(untrailingslashit($value)),
        'wp_normalize_path' => encode_value(wp_normalize_path($value)),
        'wp_checkdate' => encode_value(wp_checkdate((int) $case['month'], (int) $case['day'], (int) $case['year'], $value)),
        'wp_is_valid_utf8' => wp_is_valid_utf8($value),
        'wp_has_noncharacters' => wp_has_noncharacters($value),
        'wp_scrub_utf8' => encode_value(wp_scrub_utf8($value)),
        'wp_basename' => encode_value(wp_basename($value, decode_value($case['suffix']))),
        'wp_list_pluck' => encode_value(wp_list_pluck($value, decode_value($case['field']), $case['index'] === null ? null : decode_value($case['index']))),
        'wp_slash' => encode_value(wp_slash($value)),
        'wp_unslash' => encode_value(wp_unslash($value)),
        'wp_parse_list' => encode_value(wp_parse_list($value)),
        'wp_parse_str' => encode_value(oracle_parse_str($value)),
        'wp_parse_args' => encode_value(wp_parse_args($value, decode_value($case['defaults']))),
        'wp_array_slice_assoc' => encode_value(wp_array_slice_assoc($value, decode_value($case['keys']))),
        'sanitize_key' => encode_value(sanitize_key($value)),
        'sanitize_html_class' => encode_value(sanitize_html_class($value, decode_value($case['fallback']))),
        'sanitize_locale_name' => encode_value(sanitize_locale_name($value)),
        'sanitize_mime_type' => encode_value(sanitize_mime_type($value)),
        'is_email' => encode_value(is_email($value)),
        'wp_unique_id' => encode_value(wp_unique_id($value)),
        'wp_unique_prefixed_id' => encode_value(wp_unique_prefixed_id($value)),
        'wp_json_encode' => encode_value(wp_json_encode($value, $case['flags'] ?? 0)),
        'checked' => encode_value(checked($value, decode_value($case['current']), false)),
        'selected' => encode_value(selected($value, decode_value($case['current']), false)),
        'disabled' => encode_value(disabled($value, decode_value($case['current']), false)),
        'wp_readonly' => encode_value(wp_readonly($value, decode_value($case['current']), false)),
        default => throw new RuntimeException('unknown oracle API'),
    };
    remove_all_filters('sanitize_key');
    remove_all_filters('sanitize_html_class');
    remove_all_filters('sanitize_locale_name');
    remove_all_filters('sanitize_mime_type');
    remove_all_filters('is_email');
    remove_all_filters('wp_checkdate');
    remove_all_filters('wp_parse_str');
    $results[] = ['id' => $case['id'], 'value' => $output];
}
$json_flags = [];
foreach (['JSON_HEX_TAG','JSON_HEX_AMP','JSON_HEX_APOS','JSON_HEX_QUOT','JSON_FORCE_OBJECT','JSON_PRETTY_PRINT','JSON_UNESCAPED_SLASHES','JSON_UNESCAPED_UNICODE','JSON_UNESCAPED_LINE_TERMINATORS'] as $name) {
    $json_flags[$name] = constant($name);
}
$truncate_metadata=['truncate_slug_profile'=>truncate_slug_profile(),'truncate_slug_calls'=>$truncate_slug_calls,'truncate_slug_branches'=>$truncate_slug_branches];
$default_title_metadata=['default_sanitize_title_profile'=>default_sanitize_title_profile(),'default_title_calls'=>$default_title_calls,'default_title_filter_calls'=>$default_title_filter_calls,'default_title_locale_calls'=>$default_title_locale_calls,'default_title_restored'=>$default_title_restored];
$slug_metadata=['utf8_uri_profile'=>utf8_uri_profile(),'title_dash_profile'=>title_dash_profile(),'utf8_uri_calls'=>$utf8_uri_calls,'title_dash_calls'=>$title_dash_calls];
$post_meta_cached_metadata=post_meta_cached_metadata();
$texturize_metadata=texturize_metadata();
$title_metadata=title_metadata();
$auth_cookie_metadata=auth_cookie_metadata();
$auth_cookie_parse_metadata=auth_cookie_parse_metadata();
$sanitize_text_metadata=sanitize_text_metadata();
echo json_encode([...$sanitize_text_metadata,...$auth_cookie_parse_metadata,...$auth_cookie_metadata,...$title_metadata,...$texturize_metadata,...$post_meta_cached_metadata,...$truncate_metadata,...$default_title_metadata,...$slug_metadata,'php_version' => PHP_VERSION, 'php_int_size' => PHP_INT_SIZE, 'mbstring_loaded' => extension_loaded('mbstring'), 'ctype_locale' => setlocale(LC_CTYPE, 0), 'json_encode_flags' => $json_flags, 'json_utf8_fallback_profile' => json_utf8_fallback_profile(), 'hr_byte_units' => hr_byte_units_profile(), 'hr_byte_max_significant_digits' => 256, 'default_html_entity_profile' => default_html_entity_profile(), 'xml_utf8_entity_profile' => xml_utf8_entity_profile(), 'specialchars_utf8_profile' => specialchars_utf8_profile(), 'empty_filter_escaping_profile' => empty_filter_escaping_profile(), 'empty_filter_escaping_hook_counts' => $empty_filter_hook_counts, 'strip_all_tags_profile' => strip_all_tags_profile(), 'strip_all_tags_calls' => $strip_all_tags_calls, 'sanitize_user_profile' => sanitize_user_profile(), 'sanitize_user_calls' => $sanitize_user_calls, 'sanitize_user_filter_calls' => $sanitize_user_filter_calls, 'sanitize_user_locale_hook_counts' => $sanitize_user_locale_calls, 'locale_db_branch_calls' => $locale_db_branch_calls, 'remove_accents_profile' => remove_accents_profile(), 'remove_accents_calls' => $remove_accents_calls, 'remove_accents_get_locale_calls' => $remove_accents_get_locale_calls, 'wp_rehash_profile' => wp_rehash_profile(), 'wp_rehash_hook_counts' => $wp_rehash_hook_counts, 'wp_bcrypt_profile' => wp_bcrypt_profile(), 'wp_bcrypt_check_password_calls' => $wp_bcrypt_calls, 'entity_decode_profile' => entity_decode_profile(), 'protocol_once_profile' => protocol_once_profile(), 'protocol_outer_profile' => protocol_outer_profile(), 'parse_str_settings' => ['arg_separator.input' => ini_get('arg_separator.input'), 'max_input_vars' => ini_get('max_input_vars'), 'max_input_nesting_level' => ini_get('max_input_nesting_level')], 'results' => $results], JSON_THROW_ON_ERROR), "\n";
