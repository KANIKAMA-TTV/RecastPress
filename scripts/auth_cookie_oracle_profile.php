<?php
/** Cookie contents only: prepared user/salt fixtures, no session or resolver. */
$auth_cookie_counts=array_fill_keys(['calls','user_lookup_calls','hash_salt_calls','auth_cookie_hook','session_calls','db_calls','cache_calls','restored'],0);
function auth_cookie_profile() {
    global $root;
    $profile=json_decode('{"php_version":"8.5.12","name":"prepared_existing_user_resolved_salt_truthy_token_empty_filters","user_lookup":"scoped_existing_record_get_userdata_fixture","user_id":"typed_positive_signed64_integer","user_login":"arbitrary_bytes","password_hash":"arbitrary_bytes_fragment_only_no_verification","expiration":"typed_signed64_decimal_no_clock_validation","token":"php_truthy_bytes_empty_and_ascii_zero_excluded","salt":"caller_resolved_nonempty_bytes_no_resolver","schemes":["auth","secure_auth","logged_in"],"scheme_scope":"oracle_salt_selection_witness_not_native_policy","pass_fragment":"literal_P_or_2y_offset8_len4_else_last4_bytes","inner_hmac":"md5_lowercase_hex_ascii_key","outer_hmac":"sha256_lowercase_hex","filter_registry":"one_empty_auth_cookie_wp_hook_no_all_callbacks","filter_arguments":5,"filter_arguments_basis":"pinned_source_without_observer_callback","hash_calls_basis":"actual_wp_hash_salt_fixture_entry_not_builtin_hmac_instrumentation","session_generation":"outside_profile_tripwire","native_filter_bookkeeping":false,"input_sum_fields":["user_login","password_hash","resolved_salt","token"],"fixture_limits":{"max_input_bytes":65536,"max_output_bytes":65536},"source_sha256":"3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777"}',true,32,JSON_THROW_ON_ERROR);
    $profile['php_version']=PHP_VERSION;
    $profile['source_sha256']=hash_file('sha256',$root.'/wp-includes/pluggable.php');
    return $profile;
}
function auth_cookie_metadata() {
    global $auth_cookie_counts;
    return ['auth_cookie_profile'=>auth_cookie_profile(),'auth_cookie_counts'=>$auth_cookie_counts];
}

function auth_cookie_require(#[\SensitiveParameter] $condition) {
    if (!$condition) { throw new RuntimeException('Auth cookie input/profile unsupported'); }
}

function auth_cookie_active() {
    return is_array($GLOBALS['auth_cookie_scope']??null) && ($GLOBALS['auth_cookie_scope']['active']??false)===true;
}

function auth_cookie_load_reference_functions(#[\SensitiveParameter] $root) {
    foreach (['wp_generate_auth_cookie','wp_hash'] as $name) {
        load_reference_function($root.'/wp-includes/pluggable.php',$name);
    }
    auth_cookie_require(in_array('md5',hash_hmac_algos(),true) && in_array('sha256',hash_hmac_algos(),true));
}

function auth_cookie_hex(#[\SensitiveParameter] $wire) {
    auth_cookie_require(is_array($wire) && count($wire)===2 && ($wire['type']??null)==='string'
        && isset($wire['hex']) && is_string($wire['hex']) && strlen($wire['hex'])<=131072
        && strlen($wire['hex'])%2===0 && strspn($wire['hex'],'0123456789abcdefABCDEF')===strlen($wire['hex']));
    return $wire['hex'];
}

function auth_cookie_integer(#[\SensitiveParameter] $wire) {
    auth_cookie_require(is_array($wire) && count($wire)===2 && ($wire['type']??null)==='int'
        && isset($wire['value']) && is_string($wire['value']) && PHP_INT_SIZE===8);
    return decode_integer_wire($wire['value']);
}

function auth_cookie_input(#[\SensitiveParameter] $case) {
    try { return hex2bin(auth_cookie_hex($case['input']??null)); }
    catch (Throwable $unused) { throw new RuntimeException('Auth cookie input/profile unsupported'); }
}

function auth_cookie_guard(#[\SensitiveParameter] $raw, #[\SensitiveParameter] $case) {
    auth_cookie_require(is_string($raw) && ($case['auth_cookie_profile']??null)==='prepared_existing_user_resolved_salt_truthy_token_empty_filters'
        && ($case['empty_filters']??null)===true && in_array($case['scheme']??null,['auth','secure_auth','logged_in'],true));
    $owner=auth_cookie_integer($case['user_id']??null);$expiration=auth_cookie_integer($case['expiration']??null);
    auth_cookie_require($owner>0);
    $hex=[];$sum=0;
    foreach (['input','password_hash','resolved_salt','token'] as $field) {
        $hex[$field]=auth_cookie_hex($case[$field]??null);$length=intdiv(strlen($hex[$field]),2);
        auth_cookie_require($length<=65536-$sum);$sum+=$length;
    }
    $token=hex2bin($hex['token']);$salt=hex2bin($hex['resolved_salt']);
    auth_cookie_require($token!=='' && $token!=='0' && $salt!=='');
    auth_cookie_require($raw===hex2bin($hex['input']));
    $limits=$case['limits']??null;
    auth_cookie_require(is_array($limits) && count($limits)===2
        && ($limits['max_input_bytes']??null)===65536 && ($limits['max_output_bytes']??null)===65536);
    $final=strlen($raw)+strlen((string)$expiration)+strlen($token)+67;
    auth_cookie_require($final<=65536);
    return [$owner,$expiration,hex2bin($hex['password_hash']),$token,$salt,$case['scheme']];
}

// These fixture names are intentionally closed outside the active scope. They
// do not implement user lookup, configuration, salt resolution, RNG or storage.
if (function_exists('get_userdata') || function_exists('wp_salt') || class_exists('WP_Session_Tokens',false)) {
    throw new RuntimeException('Auth cookie fixture function conflict');
} else {
    function get_userdata(#[\SensitiveParameter] $user_id) {
        auth_cookie_require(auth_cookie_active());
        ++$GLOBALS['auth_cookie_scope']['user_lookup_calls'];
        auth_cookie_require(func_num_args()===1 && $user_id===$GLOBALS['auth_cookie_scope']['user_id']);
        return $GLOBALS['auth_cookie_scope']['user'];
    }
    function wp_salt(#[\SensitiveParameter] $scheme='auth') {
        auth_cookie_require(auth_cookie_active());
        ++$GLOBALS['auth_cookie_scope']['hash_salt_calls'];
        auth_cookie_require(func_num_args()===1 && $scheme===$GLOBALS['auth_cookie_scope']['scheme']);
        return $GLOBALS['auth_cookie_scope']['salt'];
    }
    final class WP_Session_Tokens {
        public static function get_instance(#[\SensitiveParameter] $user_id) {
            auth_cookie_require(auth_cookie_active());
            ++$GLOBALS['auth_cookie_scope']['session_calls'];
            throw new RuntimeException('Auth cookie session generation forbidden');
        }
    }
}

final class AuthCookieSqlTrap {
    private int $count=0;
    public function calls(): int { return $this->count; }
    public function __get(#[\SensitiveParameter] $name) {
        ++$this->count;throw new RuntimeException('Auth cookie database access forbidden');
    }
    public function __call(#[\SensitiveParameter] $name, #[\SensitiveParameter] $arguments) {
        ++$this->count;throw new RuntimeException('Auth cookie database access forbidden');
    }
}

function oracle_auth_cookie(#[\SensitiveParameter] $raw, #[\SensitiveParameter] $case) {
    global $auth_cookie_counts;
    try { [$owner,$expiration,$password,$token,$salt,$scheme]=auth_cookie_guard($raw,$case); }
    catch (Throwable $unused) { throw new RuntimeException('Auth cookie input/profile unsupported'); }
    $names=['wp_filter','wp_filters','wp_current_filter','auth_cookie_scope','wpdb','wp_actions',
        'wp_hasher','wp_object_cache','wp_meta_keys','post_meta_cached_scope','title_scope'];
    $saved=[];foreach ($names as $name) { $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null]; }
    $db=new AuthCookieSqlTrap();
    try {
        $hook=new WP_Hook();
        $GLOBALS['wp_filter']=['auth_cookie'=>$hook];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];
        $GLOBALS['wpdb']=$db;$GLOBALS['post_meta_cached_scope']=null;$GLOBALS['title_scope']=null;
        $user=(object)['ID'=>$owner,'user_login'=>$raw,'user_pass'=>$password];$user_before=get_object_vars($user);
        $scope=['active'=>true,'user_id'=>$owner,'user'=>$user,'scheme'=>$scheme,'salt'=>$salt,
            'user_lookup_calls'=>0,'hash_salt_calls'=>0,'session_calls'=>0,'cache_calls'=>0];
        $GLOBALS['auth_cookie_scope']=$scope;
        ++$auth_cookie_counts['calls'];
        try { $result=wp_generate_auth_cookie($owner,$expiration,$scheme,$token); }
        catch (Throwable $unused) { throw new RuntimeException('Auth cookie reference failed'); }
        $expected=$scope;$expected['user_lookup_calls']=1;$expected['hash_salt_calls']=1;
        auth_cookie_require(is_string($result) && strlen($result)<=65536
            && $GLOBALS['wp_filters']===['auth_cookie'=>1] && $GLOBALS['wp_current_filter']===[]
            && array_keys($GLOBALS['wp_filter'])===['auth_cookie'] && $GLOBALS['wp_filter']['auth_cookie']===$hook
            && $hook->callbacks===[] && $GLOBALS['auth_cookie_scope']===$expected
            && get_object_vars($user)===$user_before && $GLOBALS['wpdb']===$db && $db->calls()===0
            && $GLOBALS['post_meta_cached_scope']===null && $GLOBALS['title_scope']===null);
        foreach (['wp_actions','wp_hasher','wp_object_cache','wp_meta_keys'] as $name) {
            [$had,$old]=$saved[$name];auth_cookie_require(array_key_exists($name,$GLOBALS)===$had && (!$had || $GLOBALS[$name]===$old));
        }
        return $result;
    } finally {
        // Failure counters are observations too, never successful comparison
        // outcomes. The full batch aborts on any reference/profile refusal.
        try {
            if (auth_cookie_active()) {
                foreach (['user_lookup_calls','hash_salt_calls','session_calls','cache_calls'] as $key) {
                    $value=$GLOBALS['auth_cookie_scope'][$key]??null;
                    if (is_int($value) && $value>=0) { $auth_cookie_counts[$key]+=$value; }
                }
            }
            $hook_calls=$GLOBALS['wp_filters']['auth_cookie']??null;
            if (is_int($hook_calls) && $hook_calls>=0) { $auth_cookie_counts['auth_cookie_hook']+=$hook_calls; }
            $auth_cookie_counts['db_calls']+=$db->calls();
        } finally {
            foreach ($saved as $name=>[$had,$old]) { if ($had) { $GLOBALS[$name]=$old; } else { unset($GLOBALS[$name]); } }
            foreach ($saved as $name=>[$had,$old]) {
                if (array_key_exists($name,$GLOBALS)!==$had || ($had && $GLOBALS[$name]!==$old)) {
                    throw new RuntimeException('Auth cookie scope restoration failed');
                }
            }
            ++$auth_cookie_counts['restored'];
        }
    }
}
