<?php
/** English, text-only isolated fixture. Not gettext or a title rendering pipeline. */
$texturize_counts = array_fill_keys(['calls','initialized','x_calls','run_hook','no_tags_hook','no_shortcodes_hook','spaces_hook','restored'], 0);

function texturize_load_reference_functions($root) {
    foreach (['wptexturize','wptexturize_primes','wp_spaces_regexp','_get_wptexturize_split_regex'] as $name) {
        load_reference_function($root . '/wp-includes/formatting.php', $name);
    }
}

function texturize_require($condition) {
    if (!$condition) { throw new RuntimeException('Texturize input/profile unsupported'); }
}

// Explicit English translations, restricted to this active fixture. No ambient
// gettext hooks, locale lookup or database access are modeled by this stub.
function _x(#[\SensitiveParameter] $text, #[\SensitiveParameter] $context, #[\SensitiveParameter] $domain = 'default') {
    $pairs = [
        'opening curly double quote'=>'&#8220;', 'closing curly double quote'=>'&#8221;',
        'apostrophe'=>'&#8217;', 'prime'=>'&#8242;', 'double prime'=>'&#8243;',
        'opening curly single quote'=>'&#8216;', 'closing curly single quote'=>'&#8217;',
        'en dash'=>'&#8211;', 'em dash'=>'&#8212;',
        'Comma-separated list of words to texturize in your language'=>"'tain't,'twere,'twas,'tis,'twill,'til,'bout,'nuff,'round,'cause,'em",
        'Comma-separated list of replacement words in your language'=>'&#8217;tain&#8217;t,&#8217;twere,&#8217;twas,&#8217;tis,&#8217;twill,&#8217;til,&#8217;bout,&#8217;nuff,&#8217;round,&#8217;cause,&#8217;em',
    ];
    texturize_require(($GLOBALS['texturize_scope']['active'] ?? false) === true
        && $domain === 'default' && is_string($context) && isset($pairs[$context]) && $text === $pairs[$context]);
    ++$GLOBALS['texturize_scope']['x_calls'];
    return $text;
}

function texturize_metadata() {
    global $root, $texturize_counts;
    return ['texturize_profile'=>[
        'php_version'=>PHP_VERSION, 'name'=>'valid_utf8_no_lt_default_english_empty_filters',
        'input_type'=>'bytes_whole_valid_utf8_no_literal_lt', 'reset'=>true,
        'translations'=>'explicit_default_english_x_identity', 'shortcode_registry'=>'empty',
        'cockney_override'=>'unset', 'filter_registry'=>'empty', 'ctype_locale'=>setlocale(LC_CTYPE,0),
        'native_filter_bookkeeping'=>false, 'spaces_cache'=>'one_initialization_per_php_process',
        'fixture_limits'=>['max_input_bytes'=>65536,'max_work_bytes'=>1048576,'max_output_bytes'=>65536],
        'source_sha256'=>hash_file('sha256',$root.'/wp-includes/formatting.php'),
    ], 'texturize_counts'=>$texturize_counts];
}

function oracle_texturize(#[\SensitiveParameter] $raw, #[\SensitiveParameter] $case) {
    global $texturize_counts;
    texturize_require(is_string($raw) && strlen($raw)<=65536 && !str_contains($raw,'<')
        && preg_match('//u',$raw)===1 && setlocale(LC_CTYPE,0)==='C'
        && ($case['texturize_profile']??null)==='valid_utf8_no_lt_default_english_empty_filters'
        && ($case['reset']??null)===true);
    $limits=$case['limits']??null;
    texturize_require(is_array($limits) && count($limits)===3);
    foreach (['max_input_bytes'=>65536,'max_work_bytes'=>1048576,'max_output_bytes'=>65536] as $key=>$value) {
        texturize_require(($limits[$key]??null)===$value);
    }
    // Reset=true cannot recover a previously poisoned run_texturize static:
    // the source checks it before entering its initialization branch.
    $statics=(new ReflectionFunction('wptexturize'))->getStaticVariables();
    $spaces=(new ReflectionFunction('wp_spaces_regexp'))->getStaticVariables()['spaces'];
    texturize_require(($statics['run_texturize']??null)===true
        && in_array($spaces, ['', '[\r\n\t ]|\xC2\xA0|&nbsp;'], true));
    $saved=[];
    foreach (['wp_filter','wp_filters','wp_current_filter','shortcode_tags','wp_cockneyreplace','texturize_scope'] as $name) {
        $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];
    }
    try {
        $GLOBALS['wp_filter']=[]; $GLOBALS['wp_filters']=[]; $GLOBALS['wp_current_filter']=[];
        $GLOBALS['shortcode_tags']=[]; unset($GLOBALS['wp_cockneyreplace']);
        $GLOBALS['texturize_scope']=['active'=>true,'x_calls'=>0];
        $initialized=$raw!=='' && $raw!=='0';
        $space_hook=$initialized && $spaces==='';
        $hooks=$initialized ? ['run_wptexturize'=>1] : [];
        if ($space_hook) { $hooks['wp_spaces_regexp']=1; }
        if ($initialized) { $hooks['no_texturize_tags']=1; $hooks['no_texturize_shortcodes']=1; }
        preg_match('//','');
        try { $result=wptexturize($raw,true); }
        catch (Throwable $unused) { throw new RuntimeException('Texturize reference failed'); }
        texturize_require(is_string($result) && strlen($result)<=65536 && preg_last_error()===PREG_NO_ERROR
            && $GLOBALS['wp_filters']===$hooks && $GLOBALS['wp_filter']===[] && $GLOBALS['wp_current_filter']===[]
            && $GLOBALS['shortcode_tags']===[] && !isset($GLOBALS['wp_cockneyreplace'])
            && $GLOBALS['texturize_scope']===['active'=>true,'x_calls'=>$initialized?11:0]);
        ++$texturize_counts['calls'];
        foreach (['initialized','run_hook','no_tags_hook','no_shortcodes_hook'] as $key) { $texturize_counts[$key]+=(int)$initialized; }
        $texturize_counts['x_calls']+=$GLOBALS['texturize_scope']['x_calls'];
        $texturize_counts['spaces_hook']+=(int)$space_hook;
        return $result;
    } finally {
        foreach ($saved as $name=>[$had,$old]) { if ($had) { $GLOBALS[$name]=$old; } else { unset($GLOBALS[$name]); } }
        foreach ($saved as $name=>[$had,$old]) {
            if (array_key_exists($name,$GLOBALS)!==$had || ($had && $GLOBALS[$name]!==$old)) {
                throw new RuntimeException('Texturize scope restoration failed');
            }
        }
        ++$texturize_counts['restored'];
    }
}
