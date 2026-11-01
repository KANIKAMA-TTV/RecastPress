<?php
/** Prepared raw WP_Post and four named default callbacks, not a WP bootstrap. */
$title_counts = array_fill_keys(['calls','the_title_hook','getter_x_calls','getter_run_hook',
    'getter_no_tags_hook','getter_no_shortcodes_hook','prime_calls','prime_x_calls',
    'prime_run_hook','prime_no_tags_hook','prime_no_shortcodes_hook','prime_spaces_hook',
    'registered_shape','db_calls','cache_calls','restored'], 0);

function title_require(#[\SensitiveParameter] $condition) {
    if (!$condition) { throw new RuntimeException('Prepared title input/profile unsupported'); }
}

function title_callbacks() {
    return [['function'=>'wptexturize','priority'=>10,'accepted_args'=>1],
        ['function'=>'convert_chars','priority'=>10,'accepted_args'=>1],
        ['function'=>'trim','priority'=>10,'accepted_args'=>1],
        ['function'=>'capital_P_dangit','priority'=>11,'accepted_args'=>1]];
}

function title_check_callbacks(#[\SensitiveParameter] $path = null) {
    global $root;
    try {
        $path ??= $root.'/wp-includes/default-filters.php';
        $handle = fopen($path, 'rb');
        try { $raw = stream_get_contents($handle, 1048577); }
        finally { fclose($handle); }
        title_require(is_string($raw) && strlen($raw)<=1048576
            && hash('sha256',$raw)==='55bbf47fb835a0f64f28423d475b51f7fa72479f319fca417b4f23990d44f897');
        $lines=preg_split('/\r?\n/',$raw);
        $direct=["add_filter( 'the_title', 'wptexturize' );", "add_filter( 'the_title', 'convert_chars' );", "add_filter( 'the_title', 'trim' );"];
        preg_match_all("/^add_filter\\( 'the_title', .*?\\);$/m",$raw,$matches);
        title_require(array_slice($lines,196,3)===$direct && $matches[0]===$direct
            && array_slice($lines,169,3)===["foreach ( array( 'the_content', 'the_title', 'wp_title', 'document_title' ) as \$filter ) {",
                "\tadd_filter( \$filter, 'capital_P_dangit', 11 );", '}']);
    } catch (Throwable $unused) { throw new RuntimeException('Prepared title callback source differs'); }
    return title_callbacks();
}

function title_load_reference_functions(#[\SensitiveParameter] $source_root) {
    title_check_callbacks();
    foreach (['OBJECT'=>'OBJECT','ARRAY_A'=>'ARRAY_A','ARRAY_N'=>'ARRAY_N'] as $name=>$value) {
        if (!defined($name)) { define($name,$value); }
        title_require(constant($name)===$value);
    }
    require_once $source_root.'/wp-includes/class-wp-post.php';
    title_require(realpath((new ReflectionClass('WP_Post'))->getFileName())===realpath($source_root.'/wp-includes/class-wp-post.php'));
    load_reference_function($source_root.'/wp-includes/post-template.php','get_the_title');
    load_reference_function($source_root.'/wp-includes/post.php','get_post');
    load_reference_function($source_root.'/wp-includes/load.php','is_admin');
    foreach (['convert_chars','capital_P_dangit'] as $name) {
        load_reference_function($source_root.'/wp-includes/formatting.php',$name);
    }
}

function title_metadata() {
    global $root,$title_counts;
    return ['title_profile'=>[
        'php_version'=>PHP_VERSION,'name'=>'prepared_raw_publish_frontend_default_english_no_html',
        'input_type'=>'bytes_whole_valid_utf8_no_literal_lt',
        'post_id_type'=>'positive_signed64_integer','post_password_type'=>'bytes_empty_or_ascii_zero',
        'post_status'=>'publish','post_filter'=>'raw','frontend'=>true,
        'translations'=>'explicit_default_english_x_identity','shortcode_registry'=>'empty',
        'cockney_override'=>'unset','ctype_locale'=>setlocale(LC_CTYPE,0),'native_filter_bookkeeping'=>false,
        'fixture_limits'=>['max_input_bytes'=>65536,'max_work_bytes'=>1048576,'max_output_bytes'=>65536],
        'callbacks'=>title_callbacks(),'all_hook'=>false,
        'priming'=>['raw'=>'prime','reset'=>true,'per_getter'=>true],
        'get_post_profile'=>'actual_wp_post_filter_raw_same_object','html_output'=>'entity_syntax_not_literal_text',
        'default_filters_source_sha256'=>hash_file('sha256',$root.'/wp-includes/default-filters.php'),
        'source_sha256'=>hash_file('sha256',$root.'/wp-includes/post-template.php'),
    ],'title_counts'=>$title_counts];
}

function title_bytes(#[\SensitiveParameter] $wire) {
    title_require(is_array($wire) && count($wire)===2 && ($wire['type']??null)==='string'
        && isset($wire['hex']) && is_string($wire['hex']) && strlen($wire['hex'])<=131072
        && strlen($wire['hex'])%2===0 && strspn($wire['hex'],'0123456789abcdefABCDEF')===strlen($wire['hex']));
    return hex2bin($wire['hex']);
}

function title_integer(#[\SensitiveParameter] $wire) {
    title_require(is_array($wire) && count($wire)===2 && ($wire['type']??null)==='int'
        && isset($wire['value']) && is_string($wire['value']));
    $value=decode_integer_wire($wire['value']);
    title_require(is_int($value) && $value>0 && PHP_INT_SIZE===8);
    return $value;
}

function title_input(#[\SensitiveParameter] $case) {
    try { return title_bytes($case['input']??null); }
    catch (Throwable $unused) { throw new RuntimeException('Prepared title input/profile unsupported'); }
}

function title_guard(#[\SensitiveParameter] $raw, #[\SensitiveParameter] $case) {
    title_require(is_string($raw) && $raw===title_input($case) && strlen($raw)<=65536
        && !str_contains($raw,'<') && preg_match('//u',$raw)===1
        && setlocale(LC_CTYPE,0)==='C' && !defined('WP_ADMIN')
        && ($case['title_profile']??null)==='prepared_raw_publish_frontend_default_english_no_html'
        && ($case['post_filter']??null)==='raw' && ($case['frontend']??null)===true);
    $owner=title_integer($case['post_id']??null);
    $password=title_bytes($case['post_password']??null);
    $status=title_bytes($case['post_status']??null);
    title_require(($password==='' || $password==='0') && $status==='publish');
    $limits=$case['limits']??null;
    title_require(is_array($limits) && count($limits)===3);
    foreach (['max_input_bytes'=>65536,'max_work_bytes'=>1048576,'max_output_bytes'=>65536] as $key=>$value) {
        title_require(($limits[$key]??null)===$value);
    }
    $statics=(new ReflectionFunction('wptexturize'))->getStaticVariables();
    $spaces=(new ReflectionFunction('wp_spaces_regexp'))->getStaticVariables()['spaces'];
    title_require(($statics['run_texturize']??null)===true
        && in_array($spaces,['','[\r\n\t ]|\xC2\xA0|&nbsp;'],true)
        && (new ReflectionFunction('capital_P_dangit'))->getStaticVariables()['dblq']===false);
    return [$owner,$password,$status,$spaces===''];
}

final class TitleSqlTrap {
    private int $count=0;
    public function calls(): int { return $this->count; }
    public function __get(#[\SensitiveParameter] $name) {
        ++$this->count; throw new RuntimeException('Prepared title database access forbidden');
    }
    public function __call(#[\SensitiveParameter] $name, #[\SensitiveParameter] $arguments) {
        ++$this->count; throw new RuntimeException('Prepared title database access forbidden');
    }
}

// The raw publish/frontend fixture must never request sanitization or translated
// protected/private prefixes. These are closed tripwires, not implementations.
if (function_exists('sanitize_post') || function_exists('__')) {
    throw new RuntimeException('Prepared title fixture function conflict');
} else {
    function sanitize_post(#[\SensitiveParameter] $post, #[\SensitiveParameter] $context='display') {
        if (($GLOBALS['title_scope']['active']??false)===true) { ++$GLOBALS['title_scope']['external_calls']; }
        throw new RuntimeException('Prepared title sanitization forbidden');
    }
    function __(#[\SensitiveParameter] $text, #[\SensitiveParameter] $domain='default') {
        if (($GLOBALS['title_scope']['active']??false)===true) { ++$GLOBALS['title_scope']['external_calls']; }
        throw new RuntimeException('Prepared title translation forbidden');
    }
}

function title_registry_shape() {
    title_require(array_keys($GLOBALS['wp_filter'])===['the_title']
        && $GLOBALS['wp_filter']['the_title'] instanceof WP_Hook);
    $result=[];
    foreach ($GLOBALS['wp_filter']['the_title']->callbacks as $priority=>$items) {
        foreach ($items as $item) {
            $result[]=['function'=>$item['function'],'priority'=>$priority,'accepted_args'=>$item['accepted_args']];
        }
    }
    title_require($result===title_callbacks());
}

function oracle_title(#[\SensitiveParameter] $raw, #[\SensitiveParameter] $case) {
    global $title_counts;
    try { [$owner,$password,$status,$first_spaces]=title_guard($raw,$case); }
    catch (Throwable $unused) { throw new RuntimeException('Prepared title input/profile unsupported'); }
    $names=['wp_filter','wp_filters','wp_current_filter','wp_actions','post','current_screen',
        'shortcode_tags','wp_cockneyreplace','texturize_scope','title_scope','post_meta_cached_scope',
        'wpdb','wp_hasher','wp_object_cache'];
    $saved=[];
    foreach ($names as $name) { $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null]; }
    try {
        $GLOBALS['wp_filter']=[];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];
        $GLOBALS['shortcode_tags']=[];unset($GLOBALS['wp_cockneyreplace'],$GLOBALS['current_screen']);
        $GLOBALS['post_meta_cached_scope']=null;
        $GLOBALS['title_scope']=['active'=>true,'cache_calls'=>0,'external_calls'=>0];
        $db=new TitleSqlTrap();$GLOBALS['wpdb']=$db;
        $GLOBALS['texturize_scope']=['active'=>true,'x_calls'=>0];
        $expected_prime=['run_wptexturize'=>1];
        if ($first_spaces) { $expected_prime['wp_spaces_regexp']=1; }
        $expected_prime['no_texturize_tags']=1;$expected_prime['no_texturize_shortcodes']=1;
        try { $primed=wptexturize('prime',true); }
        catch (Throwable $unused) { throw new RuntimeException('Prepared title reference failed'); }
        title_require($primed==='prime' && $GLOBALS['wp_filters']===$expected_prime
            && $GLOBALS['texturize_scope']===['active'=>true,'x_calls'=>11]
            && $GLOBALS['wp_filter']===[] && $GLOBALS['wp_current_filter']===[]);
        $GLOBALS['wp_filters']=[];$GLOBALS['texturize_scope']['x_calls']=0;
        // Named callbacks and default accepted_args=1: ID is NOT a reset flag.
        foreach (title_callbacks() as $callback) {
            add_filter('the_title',$callback['function'],$callback['priority'],$callback['accepted_args']);
        }
        title_registry_shape();
        $post=new WP_Post((object)['ID'=>$owner,'post_title'=>$raw,'post_password'=>$password,
            'post_status'=>$status,'filter'=>'raw']);
        $before=get_object_vars($post);
        title_require($post->filter==='raw' && $post->ID===$owner);
        $active=$raw!=='' && $raw!=='0';
        $expected_hooks=['the_title'=>1];
        if ($active) { $expected_hooks['no_texturize_tags']=1;$expected_hooks['no_texturize_shortcodes']=1; }
        preg_match('//','');
        try { $result=get_the_title($post); }
        catch (Throwable $unused) { throw new RuntimeException('Prepared title reference failed'); }
        title_require(is_string($result) && strlen($result)<=65536 && preg_last_error()===PREG_NO_ERROR
            && get_object_vars($post)===$before && $GLOBALS['wp_filters']===$expected_hooks
            && $GLOBALS['wp_current_filter']===[] && $GLOBALS['texturize_scope']===['active'=>true,'x_calls'=>0]
            && (new ReflectionFunction('capital_P_dangit'))->getStaticVariables()['dblq']===false
            && !isset($GLOBALS['current_screen']) && $GLOBALS['shortcode_tags']===[]
            && !isset($GLOBALS['wp_cockneyreplace']) && $GLOBALS['post_meta_cached_scope']===null
            && $db->calls()===0 && $GLOBALS['title_scope']===['active'=>true,'cache_calls'=>0,'external_calls'=>0]);
        title_registry_shape();
        foreach (['wp_actions','post','wp_hasher','wp_object_cache'] as $name) {
            [$had,$old]=$saved[$name];
            title_require(array_key_exists($name,$GLOBALS)===$had && (!$had || $GLOBALS[$name]===$old));
        }
        foreach (['calls','the_title_hook','prime_calls','prime_run_hook','prime_no_tags_hook',
            'prime_no_shortcodes_hook','registered_shape'] as $name) { ++$title_counts[$name]; }
        $title_counts['prime_x_calls']+=11;$title_counts['prime_spaces_hook']+=(int)$first_spaces;
        $title_counts['getter_no_tags_hook']+=(int)$active;$title_counts['getter_no_shortcodes_hook']+=(int)$active;
        $title_counts['db_calls']+=$db->calls();$title_counts['cache_calls']+=$GLOBALS['title_scope']['cache_calls'];
        return $result;
    } finally {
        foreach ($saved as $name=>[$had,$old]) { if ($had) { $GLOBALS[$name]=$old; } else { unset($GLOBALS[$name]); } }
        foreach ($saved as $name=>[$had,$old]) {
            if (array_key_exists($name,$GLOBALS)!==$had || ($had && $GLOBALS[$name]!==$old)) {
                throw new RuntimeException('Prepared title scope restoration failed');
            }
        }
        ++$title_counts['restored'];
    }
}
