<?php
/** Dedicated actual WP cookie/session relation; no production auth API. */
function relation_require(#[\SensitiveParameter] $condition) {
    if (!$condition) { throw new RuntimeException('Prepared cookie relation failed'); }
}
function relation_keys(#[\SensitiveParameter] $value, #[\SensitiveParameter] $keys) {
    relation_require(is_array($value)); $actual=array_keys($value); sort($actual); sort($keys);
    relation_require($actual===$keys);
}
function relation_i64(#[\SensitiveParameter] $wire) {
    relation_require(PHP_INT_SIZE===8 && is_string($wire) && strlen($wire)<=20
        && preg_match('/\A(?:0|[1-9][0-9]*|-[1-9][0-9]*)\z/D',$wire)===1);
    $negative=str_starts_with($wire,'-'); $digits=$negative?substr($wire,1):$wire;
    $bound=$negative?'9223372036854775808':'9223372036854775807';
    relation_require(strlen($digits)<19 || (strlen($digits)===19 && strcmp($digits,$bound)<=0));
    return (int)$wire;
}
function relation_hex(#[\SensitiveParameter] $wire) {
    relation_require(is_string($wire) && strlen($wire)<=131072 && strlen($wire)%2===0
        && strspn($wire,'0123456789abcdefABCDEF')===strlen($wire));
    return $wire;
}
function relation_prepare(#[\SensitiveParameter] $case) {
    relation_keys($case,['id','cookie_hex','scheme_hex','salt_hex','user','sessions','limits']);
    relation_require(is_int($case['id']) && $case['id']>=0 && $case['id']<=4294967295);
    relation_keys($case['limits'],['max_input_bytes','max_sessions','max_session_key_bytes']);
    foreach (['max_input_bytes'=>65536,'max_sessions'=>64,'max_session_key_bytes'=>4096] as $key=>$expected) {
        relation_require($case['limits'][$key]===$expected);
    }
    $cookie=relation_hex($case['cookie_hex']); $scheme=relation_hex($case['scheme_hex']); $salt=relation_hex($case['salt_hex']);
    relation_require($salt!=='');
    $total=intdiv(strlen($cookie)+strlen($scheme)+strlen($salt),2);
    $user=null;
    if ($case['user']!==null) {
        relation_keys($case['user'],['id','login_hex','hash_hex']);
        $id=relation_i64($case['user']['id']); relation_require($id>0);
        $login=relation_hex($case['user']['login_hex']); $hash=relation_hex($case['user']['hash_hex']);
        $total+=intdiv(strlen($login)+strlen($hash),2);
        $user=['id'=>$id,'login'=>$login,'hash'=>$hash];
    }
    relation_require(is_array($case['sessions']) && array_is_list($case['sessions']) && count($case['sessions'])<=64);
    $sessions=[]; $expiries=[]; $keys=0;
    foreach ($case['sessions'] as $row) {
        relation_keys($row,['verifier','expiration','legacy']);
        relation_require(is_string($row['verifier']) && strlen($row['verifier'])===64
            && strspn($row['verifier'],'0123456789abcdef')===64 && is_bool($row['legacy']));
        relation_require(!array_key_exists($row['verifier'],$sessions));
        $expiry=relation_i64($row['expiration']); $expiries[]=$expiry; $keys+=64;
        $sessions[$row['verifier']]=$row['legacy']?$expiry:['expiration'=>$expiry,'fixture_hint'=>'public-only'];
    }
    relation_require($keys<=4096 && $total+$keys<=65536);
    // Decode only after the complete fixture input allowance has passed.
    $cookie=hex2bin($cookie); $scheme=hex2bin($scheme); $salt=hex2bin($salt);
    relation_require($cookie!=='' && $cookie!=='0');
    $parts=explode('|',$cookie); $expiry=null; $username=null;
    if (count($parts)===4) { $expiry=relation_i64($parts[1]); $username=$parts[0]; }
    if ($user!==null) { $user['login']=hex2bin($user['login']); $user['hash']=hex2bin($user['hash']); }
    return ['id'=>$case['id'],'cookie'=>$cookie,'scheme'=>$scheme,'salt'=>$salt,'user'=>$user,
            'sessions'=>$sessions,'expiries'=>$expiries,'expiry'=>$expiry,'username'=>$username];
}
function relation_scope() {
    $scope=$GLOBALS['prepared_cookie_relation_scope']??null;
    relation_require(is_object($scope) && ($scope->active??false)===true); return $scope;
}
function relation_forbid(#[\SensitiveParameter] $kind) {
    $scope=relation_scope(); ++$scope->counts[$kind]; throw new RuntimeException('Prepared cookie relation failed');
}
function get_user_by(#[\SensitiveParameter] $field, #[\SensitiveParameter] $key) {
    $scope=relation_scope(); ++$scope->counts['user_lookup'];
    relation_require($field==='login' && $key===$scope->case['username']);
    $row=$scope->case['user']; if ($row===null) { return false; }
    // Explicit record-only replacement for lookup, not a WP_User/DB policy.
    return (object)['ID'=>$row['id'],'user_login'=>$row['login'],'user_pass'=>$row['hash']];
}
function get_user_meta(#[\SensitiveParameter] $id, #[\SensitiveParameter] $key, #[\SensitiveParameter] $single=false) {
    $scope=relation_scope(); ++$scope->counts['user_meta'];
    relation_require($scope->case['user']!==null && $id===$scope->case['user']['id'] && $key==='session_tokens' && $single===true);
    return $scope->case['sessions'];
}
function wp_salt(#[\SensitiveParameter] $scheme='auth') {
    $scope=relation_scope(); ++$scope->counts['hash_salt'];
    relation_require($scheme===$scope->case['scheme']); return $scope->case['salt'];
}
function get_option(#[\SensitiveParameter] ...$args) { relation_forbid('options'); }
function wp_cache_get(#[\SensitiveParameter] ...$args) { relation_forbid('cache'); }
function wp_cache_set(#[\SensitiveParameter] ...$args) { relation_forbid('cache'); }
function wp_cache_delete(#[\SensitiveParameter] ...$args) { relation_forbid('cache'); }
function update_user_meta(#[\SensitiveParameter] ...$args) { relation_forbid('writes'); }
function delete_user_meta(#[\SensitiveParameter] ...$args) { relation_forbid('writes'); }
function delete_metadata(#[\SensitiveParameter] ...$args) { relation_forbid('writes'); }
function wp_generate_password(#[\SensitiveParameter] ...$args) { relation_forbid('rng'); }
function wp_rand(#[\SensitiveParameter] ...$args) { relation_forbid('rng'); }
function is_ssl() { relation_forbid('ssl'); }
final class RelationSqlTrap {
    public function __get(#[\SensitiveParameter] $name) { relation_forbid('db'); }
    public function __isset(#[\SensitiveParameter] $name): bool { relation_forbid('db'); }
    public function __set(#[\SensitiveParameter] $name, #[\SensitiveParameter] $value): void { relation_forbid('db'); }
    public function __call(#[\SensitiveParameter] $name, #[\SensitiveParameter] $args) { relation_forbid('db'); }
}
final class RelationCookieTrap implements ArrayAccess {
    public function offsetExists(#[\SensitiveParameter] mixed $offset): bool { relation_forbid('cookie_accesses'); }
    public function offsetGet(#[\SensitiveParameter] mixed $offset): mixed { relation_forbid('cookie_accesses'); }
    public function offsetSet(#[\SensitiveParameter] mixed $offset, #[\SensitiveParameter] mixed $value): void { relation_forbid('cookie_accesses'); }
    public function offsetUnset(#[\SensitiveParameter] mixed $offset): void { relation_forbid('cookie_accesses'); }
}
function relation_globals() {
    return ['wp_filter','wp_filters','wp_actions','wp_current_filter','wp_hasher','wp_meta_keys',
            'wp_object_cache','wpdb','login_grace_period','_SERVER','_COOKIE','prepared_cookie_relation_scope'];
}
function relation_seed(#[\SensitiveParameter] $index) {
    foreach (relation_globals() as $name) {
        switch ($index%4) {
            case 0: unset($GLOBALS[$name]); break;
            case 1: $GLOBALS[$name]=null; break;
            case 2: $GLOBALS[$name]=['public_fixture'=>new stdClass()]; break;
            default: $GLOBALS[$name]=[];
        }
    }
}
function relation_case(#[\SensitiveParameter] $case) {
    $names=relation_globals(); $saved=[];
    foreach ($names as $name) { $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null]; }
    $counter_names=['calls','ajax_filter','user_lookup','hash_salt','manager_filter','user_meta','writes','db','cache','options','rng','ssl','cookie_accesses','restored'];
    $scope=(object)['active'=>true,'case'=>$case,'counts'=>array_fill_keys($counter_names,0)];
    $action_map=['malformed'=>'auth_cookie_malformed','expired'=>'auth_cookie_expired','unknown_user'=>'auth_cookie_bad_username',
                 'bad_mac'=>'auth_cookie_bad_hash','bad_session'=>'auth_cookie_bad_session_token','valid'=>'auth_cookie_valid'];
    try {
        $GLOBALS['prepared_cookie_relation_scope']=$scope;
        $GLOBALS['wp_filter']=[]; $GLOBALS['wp_filters']=[]; $GLOBALS['wp_actions']=[]; $GLOBALS['wp_current_filter']=[];
        foreach (['wp_doing_ajax','session_token_manager',...array_values($action_map)] as $hook) {
            $GLOBALS['wp_filter'][$hook]=new WP_Hook();
        }
        $GLOBALS['wpdb']=new RelationSqlTrap(); $GLOBALS['_SERVER']=['REQUEST_METHOD'=>'GET']; $GLOBALS['_COOKIE']=new RelationCookieTrap();
        $before=time(); ++$scope->counts['calls'];
        $actual=wp_validate_auth_cookie($case['cookie'],$case['scheme']);
        $after=time();
        relation_require($after>=$before && $after-$before<=2);
        if ($case['expiry']!==null) { relation_require(($case['expiry']<$before)===($case['expiry']<$after)); }
        foreach ($case['expiries'] as $expiry) { relation_require(($expiry>=$before)===($expiry>=$after)); }
        $actions=[]; $stage=null;
        foreach ($action_map as $kind=>$hook) {
            $actions[$hook]=$GLOBALS['wp_actions'][$hook]??0;
            relation_require(is_int($actions[$hook]) && in_array($actions[$hook],[0,1],true));
            if ($actions[$hook]===1) { relation_require($stage===null); $stage=$kind; }
        }
        relation_require($stage!==null && count($GLOBALS['wp_actions'])===1);
        relation_require($stage==='valid' ? (is_int($actual) && $case['user']!==null && $actual===$case['user']['id']) : $actual===false);
        $scope->counts['ajax_filter']=$GLOBALS['wp_filters']['wp_doing_ajax']??0;
        $scope->counts['manager_filter']=$GLOBALS['wp_filters']['session_token_manager']??0;
        $lookup=!in_array($stage,['malformed','expired'],true)?1:0;
        $salt=in_array($stage,['bad_mac','bad_session','valid'],true)?1:0;
        $session=in_array($stage,['bad_session','valid'],true)?1:0;
        $expected=array_fill_keys($counter_names,0);
        foreach (['calls'=>1,'ajax_filter'=>$stage==='malformed'?0:1,'user_lookup'=>$lookup,'hash_salt'=>$salt,'manager_filter'=>$session,'user_meta'=>$session] as $key=>$value) { $expected[$key]=$value; }
        relation_require($scope->counts===$expected && count($GLOBALS['wp_filters'])===($stage==='malformed'?0:1+$session));
        relation_require($GLOBALS['wp_current_filter']===[] && !isset($GLOBALS['wp_filter']['all']));
        foreach ($GLOBALS['wp_filter'] as $hook) { relation_require($hook instanceof WP_Hook && $hook->callbacks===[]); }
        foreach (['wp_hasher','wp_meta_keys','wp_object_cache','login_grace_period'] as $name) {
            relation_require(array_key_exists($name,$GLOBALS)===$saved[$name][0]
                && (!$saved[$name][0] || $GLOBALS[$name]===$saved[$name][1]));
        }
        $result=['id'=>$case['id'],'outcome'=>$stage==='valid'?'match':'no_match','stage'=>$stage,
                 'prepared_user_id'=>$stage==='valid'?(string)$actual:null,'clock_before'=>$before,'clock_after'=>$after,
                 'clock_stable'=>true,'counts'=>$scope->counts,'actions'=>$actions];
    } finally {
        foreach ($names as $name) {
            if ($saved[$name][0]) { $GLOBALS[$name]=$saved[$name][1]; } else { unset($GLOBALS[$name]); }
        }
    }
    foreach ($names as $name) {
        relation_require(array_key_exists($name,$GLOBALS)===$saved[$name][0]
            && (!$saved[$name][0] || $GLOBALS[$name]===$saved[$name][1]));
    }
    $result['counts']['restored']=1;
    return $result;
}
function relation_main(#[\SensitiveParameter] $argv) {
    relation_require(count($argv)===2 && PHP_VERSION==='8.5.12' && PHP_INT_SIZE===8 && !defined('DOING_AJAX'));
    $repo=realpath($argv[1]); relation_require($repo!==false);
    $root=$repo.'/.reference/wordpress-7.1.3';
    $manifest_wire=file_get_contents($root.'/recastpress-reference.json',false,null,0,2097153);
    relation_require(is_string($manifest_wire) && strlen($manifest_wire)<=2097152);
    $manifest=json_decode($manifest_wire,true,32,JSON_THROW_ON_ERROR);
    relation_require(($manifest['wordpress_version']??null)==='7.1.3');
    $pinned=[]; foreach ($manifest['files'] as $entry) { $pinned[$entry['path']]=$entry['sha256']; }
    $files=['pluggable.php','load.php','plugin.php','class-wp-hook.php','class-wp-filter-sentinel.php',
            'class-wp-session-tokens.php','class-wp-user-meta-session-tokens.php'];
    $hashes=[];
    foreach ($files as $file) {
        $relative='wp-includes/'.$file; $observed=hash_file('sha256',$root.'/'.$relative);
        relation_require(isset($pinned[$relative]) && $observed===$pinned[$relative]); $hashes[$relative]=$observed;
    }
    relation_require(in_array('md5',hash_hmac_algos(),true) && in_array('sha256',hash_hmac_algos(),true));
    define('ABSPATH',$root.'/'); define('WPINC','wp-includes');
    require $root.'/wp-includes/plugin.php';
    require $repo.'/scripts/reference_functions.php';
    foreach (['wp_validate_auth_cookie','wp_parse_auth_cookie','wp_hash'] as $name) { load_reference_function($root.'/wp-includes/pluggable.php',$name); }
    load_reference_function($root.'/wp-includes/load.php','wp_doing_ajax');
    require $root.'/wp-includes/class-wp-session-tokens.php';
    require $root.'/wp-includes/class-wp-user-meta-session-tokens.php';
    $raw=stream_get_contents(STDIN,8388609); relation_require(strlen($raw)<=8388608);
    $wire=json_decode($raw,true,32,JSON_THROW_ON_ERROR);
    // Canonical ASCII fixture wire also rejects duplicate decoded object keys.
    relation_require(json_encode($wire,JSON_UNESCAPED_SLASHES|JSON_THROW_ON_ERROR)."\n"===$raw);
    relation_keys($wire,['cases']); relation_require(is_array($wire['cases']) && array_is_list($wire['cases']) && count($wire['cases'])>=1 && count($wire['cases'])<=256);
    $prepared=[]; $seen=[];
    foreach ($wire['cases'] as $case) {
        $value=relation_prepare($case); relation_require(!isset($seen[$value['id']]));
        $seen[$value['id']]=true; $prepared[]=$value;
    }
    $results=[];
    foreach ($prepared as $index=>$case) { relation_seed($index); $results[]=relation_case($case); }
    $profile=['name'=>'prepared_get_nonajax_user_resolved_salt_complete_sessions_empty_hooks',
              'php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,'request_method'=>'GET','doing_ajax_defined'=>false,
              'builtin_clock'=>true,'max_bracket_seconds'=>2,'all_expiries_endpoint_stable'=>true,
              'user_lookup'=>'record_only_stdclass_fixture_not_db_or_WP_User_policy',
              'salt'=>'caller_resolved_fixture_no_resolver','session_manager'=>'actual_base_and_default_user_meta_classes',
              'actions'=>'actual_empty_WP_Hook_did_action_counters','internal_time_and_hmac_calls'=>'uninstrumented',
              'fixture_limits'=>['max_input_bytes'=>65536,'max_sessions'=>64,'max_session_key_bytes'=>4096],
              'source_sha256'=>$hashes];
    $output=json_encode(['format_version'=>1,'process_pid'=>getmypid(),'profile'=>$profile,'cases'=>$results],JSON_UNESCAPED_SLASHES|JSON_THROW_ON_ERROR)."\n";
    relation_require(strlen($output)<=1048576);
    relation_require(fwrite(STDOUT,$output)===strlen($output) && fflush(STDOUT));
}
try {
    error_reporting(E_ALL); ini_set('display_errors','0'); ini_set('log_errors','0'); ini_set('zend.exception_ignore_args','1');
    relation_require(ini_get('zend.exception_ignore_args')==='1');
    set_error_handler(static function ($level, #[\SensitiveParameter] $message, #[\SensitiveParameter] $file, $line) {
        throw new ErrorException('Prepared cookie relation diagnostic',0,$level);
    });
    relation_main($argv);
} catch (Throwable $unused) {
    // Discard inner exceptions/args, emit neither partial cases nor raw details.
    fwrite(STDERR,"Prepared cookie relation failed\n"); exit(2);
}
