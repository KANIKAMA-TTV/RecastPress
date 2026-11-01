<?php
/** Public-dummy relation fixture, not a DB-backed authentication service. */
error_reporting(E_ALL);ini_set('display_errors','stderr');ini_set('log_errors','0');ini_set('zend.exception_ignore_args','1');
set_exception_handler(static function($error){fwrite(STDERR,"Credential stage oracle rejected\n");exit(2);});
set_error_handler(static function($level){throw new ErrorException('fixture diagnostic',0,$level);});
if(ini_get('zend.exception_ignore_args')!=='1'||setlocale(LC_CTYPE,'C')!=='C'){throw new RuntimeException('profile unavailable');}
if(PHP_VERSION!=='8.5.12'||extension_loaded('intl')||extension_loaded('mbstring')||function_exists('normalizer_normalize')||function_exists('normalizer_is_normalized')){throw new RuntimeException('tool profile differs');}
$root=$argv[1]??throw new RuntimeException('pinned root required');
define('ABSPATH',$root.'/');define('WPINC','wp-includes');
require __DIR__.'/reference_functions.php';
require $root.'/wp-includes/plugin.php';require $root.'/wp-includes/compat-utf8.php';require $root.'/wp-includes/utf8.php';
require $root.'/wp-includes/class-wp-user.php';require $root.'/wp-includes/class-wp-error.php';
foreach(['wp_authenticate_username_password','wp_authenticate_email_password']as$name){load_reference_function($root.'/wp-includes/user.php',$name);}
foreach(['wp_check_password','wp_password_needs_rehash']as$name){load_reference_function($root.'/wp-includes/pluggable.php',$name);}
foreach(['wp_strip_all_tags','sanitize_user','remove_accents','is_email']as$name){load_reference_function($root.'/wp-includes/formatting.php',$name);}
load_reference_function($root.'/wp-includes/l10n.php','get_locale');load_reference_function($root.'/wp-includes/load.php','is_wp_error');
load_reference_function(__DIR__.'/legacy_password_oracle.php','refusal');
function fields($value,$expected){if(!is_array($value)){throw new RuntimeException('object required');}$actual=array_keys($value);sort($actual);sort($expected);if($actual!==$expected){throw new RuntimeException('fields differ');}}
function unhex($value){if(!is_string($value)||strlen($value)>16384||strlen($value)%2!==0||preg_match('/\A[0-9a-f]*\z/D',$value)!==1){throw new RuntimeException('byte wire differs');}return hex2bin($value);}
function __($text){return'fixture translated error';}
function esc_html($text){return'fixture rendered error';}
function wp_lostpassword_url(){return'https://example.test/fixture-reset';}
function get_user_meta(){throw new RuntimeException('metadata access forbidden');}
function is_multisite(){throw new RuntimeException('locale DB branch forbidden');}
function get_option(){throw new RuntimeException('option access forbidden');}
function update_user_caches($row){/* Explicit fixture no-op; no real cache mutation. */}
function wp_set_password($password,$id){global$intents;$intents[]=$id;}
function wp_cache_get($key,$group){
 global$lookups;
 $kind=match($group){'userlogins'=>'login','useremail'=>'email',default=>throw new RuntimeException('unexpected cache group')};
 $lookups[]=['kind'=>$kind,'key_hex'=>bin2hex((string)$key)];
 // Every source cache lookup is a miss; fake SQL below only selects public rows.
 return false;
}
function get_user_by($field,$value){
 $data=WP_User::get_data_by($field,$value);if(!$data){return false;}
 $user=(new ReflectionClass(WP_User::class))->newInstanceWithoutConstructor();$user->data=$data;$user->ID=$data->ID;return$user;
}
$wire=stream_get_contents(STDIN,1048577);if(strlen($wire)>1048576){throw new RuntimeException('input bound');}
$request=json_decode($wire,true,32,JSON_THROW_ON_ERROR);
if($wire!==json_encode($request,JSON_THROW_ON_ERROR|JSON_UNESCAPED_SLASHES)){throw new RuntimeException('noncanonical wire');}
fields($request,['items']);if(!is_array($request['items'])||!array_is_list($request['items'])||count($request['items'])>128){throw new RuntimeException('batch bound');}
$results=[];$states=[];
$wpdb=new class {
 public $users='fixture_users';
 public function prepare($sql,$value){
  $field=match($sql){'SELECT * FROM fixture_users WHERE user_login = %s LIMIT 1'=>'user_login','SELECT * FROM fixture_users WHERE user_email = %s LIMIT 1'=>'user_email',default=>throw new RuntimeException('unexpected fake query')};
  return[$field,$value];
 }
 public function get_row($query){global$rows;foreach($rows as$row){if($row->{$query[0]}===$query[1]){return$row;}}return false;}
};
foreach($request['items']as$item){
 fields($item,['username_hex','password_hex','locale_hex','login','email','bcrypt_maximum','portable_maximum','lookup_error']);
 $raw=unhex($item['username_hex']);$password=unhex($item['password_hex']);$resolved_locale=unhex($item['locale_hex']);
 if(str_contains($raw,'<')||!$resolved_locale||!wp_is_valid_utf8(wp_strip_all_tags($raw,false))){throw new RuntimeException('input profile differs');}
 if(!is_int($item['bcrypt_maximum'])||$item['bcrypt_maximum']<4||$item['bcrypt_maximum']>31||!is_int($item['portable_maximum'])||$item['portable_maximum']<7||$item['portable_maximum']>30||!in_array($item['lookup_error'],['none','login','email'],true)){throw new RuntimeException('limits/profile differ');}
 $records=[];
 foreach(['login','email']as$kind){
  $record=$item[$kind];if($record===null){$records[$kind]=null;continue;}
  fields($record,['key_hex','hash_hex','tag']);
  if(!is_string($record['tag'])||!in_array($record['tag'],['login-record','email-record'],true)){throw new RuntimeException('public tag required');}
  $records[$kind]=['key'=>unhex($record['key_hex']),'hash'=>unhex($record['hash_hex']),'tag'=>$record['tag']];
 }
 $state=['absent','null','sentinel','empty'][count($results)%4];$states[$state]=true;
 $names=['wp_filter','wp_filters','wp_current_filter','wp_hasher','locale','wp_local_package','wp_actions'];$saved=[];
 foreach($names as$name){if($state==='absent'){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){'null'=>null,'sentinel'=>['fixture'=>new stdClass()],default=>[]};}$saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
 try{
  $wp_filter=[];$wp_filters=[];$wp_current_filter=[];$wp_actions=[];$wp_hasher=null;$locale=$resolved_locale;
  $s1=sanitize_user($raw);$initial_hooks=$wp_filters;$p1=trim($password);$rows=[];$lookups=[];$intents=[];
  foreach(['login'=>101,'email'=>202]as$kind=>$id){if($records[$kind]!==null){$record=$records[$kind];$rows[$id]=(object)['ID'=>$id,'user_login'=>$kind==='login'?$record['key']:'fixture-email-row','user_email'=>$kind==='email'?$record['key']:'fixture-login-row@example.test','user_pass'=>$record['hash'],'fixture_tag'=>$record['tag']];}}
  // Deliberate native host/backend refusals are a separate relation category.
  // Never convert these into actual WP false, nor invoke costly PHP at host-refused cost.
  $refused=$item['lookup_error']!=='none'?'lookup':null;
  if($refused===null&&!empty($s1)&&!empty($p1)&&$records['login']!==null){
   $query=trim($s1);$query=$query?sanitize_user($query):null;
   if($query!==null&&$query===$records['login']['key']){$refused=refusal('combined',$p1,$records['login']['hash'],$item['portable_maximum'],$item['bcrypt_maximum']);}
  }
  if($refused===null&&!empty($s1)&&!empty($p1)&&$records['email']!==null){
   // This deliberate refusal fixture only follows an absent login or a matched
   // short hash, whose MD5 comparison is definitively false (digest has 32 bytes).
   $login_key=trim($s1);$login_key=$login_key?sanitize_user($login_key):null;
   $login_missing=$records['login']===null||$login_key!==$records['login']['key'];
   $login_short_false=!$login_missing&&strlen($records['login']['hash'])<32;
   if(($login_missing||$login_short_false)&&is_email($s1)&&trim($s1)===$records['email']['key']){
    $refused=refusal('combined',$p1,$records['email']['hash'],$item['portable_maximum'],$item['bcrypt_maximum']);
   }
  }
  // Preflight probes must not inflate actual callback bookkeeping.
  $wp_filters=$initial_hooks;
  if($refused!==null){$results[]=['actual_called'=>false,'username_calls'=>0,'email_calls'=>0,'relation'=>null,'refusal'=>$refused,'source_error_codes'=>[],'preserved_username_error'=>null,'hook_counts'=>null];continue;}
  $before=wp_authenticate_username_password(null,$s1,$p1);
  $after=wp_authenticate_email_password($before,$s1,$p1);
  if(preg_last_error()!==PREG_NO_ERROR||$wp_current_filter!==[]||$wp_filter!==[]){throw new RuntimeException('reference result context differs');}
  $accepted=$after instanceof WP_User;$no_match=$after instanceof WP_Error;
  if(!$accepted&&!$no_match){throw new RuntimeException('outside result subset');}
  $via=$accepted?($after->ID===101?'login':'email'):null;
  $hint=$accepted?in_array($after->ID,$intents,true):null;
  if(count($intents)!==(int)($hint===true)){throw new RuntimeException('unexpected rehash intent');}
  $relation=['outcome'=>$accepted?'user':'no_match','record_tag'=>$accepted?$after->data->fixture_tag:null,
      'lookups'=>$lookups,'rehash_hint'=>$hint,'unresolved_reason'=>null,'matched_via'=>$via,'unresolved_phase'=>null,
      'username_php_empty'=>empty($s1),'password_php_empty'=>empty($p1)];
  $hooks=[];foreach(['sanitize_user','is_email','wp_authenticate_user','check_password','wp_hash_password_algorithm','wp_hash_password_options','password_needs_rehash']as$name){$hooks[$name]=did_filter($name);}
  $results[]=['actual_called'=>true,'username_calls'=>1,'email_calls'=>1,'relation'=>$relation,'refusal'=>null,
      'source_error_codes'=>$no_match?$after->get_error_codes():[],'preserved_username_error'=>$no_match&&$after===$before,'hook_counts'=>$hooks];
 }finally{
  foreach($saved as$name=>[$exists,$old]){if($exists){$GLOBALS[$name]=$old;}else{unset($GLOBALS[$name]);}}
  foreach($saved as$name=>[$exists,$old]){if(array_key_exists($name,$GLOBALS)!==$exists||($exists&&$GLOBALS[$name]!==$old)){throw new RuntimeException('global restoration failed');}}
 }
}
$output=json_encode(['process_pid'=>getmypid(),'results'=>$results,'php_version'=>PHP_VERSION,'globals_restored'=>true,'actions_restored'=>true,'restoration_states'=>array_keys($states),'db_writes'=>0],JSON_THROW_ON_ERROR|JSON_UNESCAPED_SLASHES);
if(strlen($output)>131072){throw new RuntimeException('output bound');}echo$output,"\n";
