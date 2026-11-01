<?php
/** Explicit parser observations, never environment cookie selection. */
$auth_cookie_parse_counts=array_fill_keys(['calls','parsed','false_results','ssl_calls','cookie_accesses','restored'],0);
function auth_cookie_parse_metadata() {
 global $root,$auth_cookie_parse_counts;
 $profile=json_decode('{"php_version":"8.5.12","name":"explicit_php_truthy_cookie_arbitrary_byte_scheme","cookie":"explicit_php_truthy_bytes_empty_and_ascii_zero_excluded","scheme":"arbitrary_bytes_preserved_even_empty_unknown_or_binary","input_cap":"checked_sum_cookie_and_raw_scheme","fixture_limits":{"max_input_bytes":65536},"parse":"exactly_three_literal_pipe_bytes_or_false","fields":["username","expiration","token","hmac","scheme"],"field_values":"raw_bytes_in_source_order_without_validation","environment_cookie_selection":"outside_profile_ssl_and_cookie_access_tripwires","hooks":"none_in_source","session_or_hmac_verification":false,"source_sha256":"3a2482a65b50d62ebae75a728be98b78985d3a3f5a3e007cb2d0c1338bf05777"}',true,32,JSON_THROW_ON_ERROR);
 $profile['php_version']=PHP_VERSION;
 $profile['source_sha256']=hash_file('sha256',$root.'/wp-includes/pluggable.php');
 return ['auth_cookie_parse_profile'=>$profile,'auth_cookie_parse_counts'=>$auth_cookie_parse_counts];
}

function auth_cookie_parse_require(#[\SensitiveParameter]$condition) {
 if(!$condition){throw new RuntimeException('Explicit cookie parser input/profile unsupported');}
}
function auth_cookie_parse_active() {
 return is_array($GLOBALS['auth_cookie_parse_scope']??null)&&($GLOBALS['auth_cookie_parse_scope']['active']??false)===true;
}
function auth_cookie_parse_load_reference_functions(#[\SensitiveParameter]$root) {
 load_reference_function($root.'/wp-includes/pluggable.php','wp_parse_auth_cookie');
}
function auth_cookie_parse_hex(#[\SensitiveParameter]$wire) {
 auth_cookie_parse_require(is_array($wire)&&count($wire)===2&&($wire['type']??null)==='string'
  &&isset($wire['hex'])&&is_string($wire['hex'])&&strlen($wire['hex'])<=131072
  &&strlen($wire['hex'])%2===0&&strspn($wire['hex'],'0123456789abcdefABCDEF')===strlen($wire['hex']));
 return$wire['hex'];
}
function auth_cookie_parse_input(#[\SensitiveParameter]$case) {
 try{return hex2bin(auth_cookie_parse_hex($case['input']??null));}
 catch(Throwable$unused){throw new RuntimeException('Explicit cookie parser input/profile unsupported');}
}
function auth_cookie_parse_guard(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$case) {
 auth_cookie_parse_require(is_string($raw)&&$raw!==''&&$raw!=='0'
  &&($case['explicit_cookie']??null)===true
  &&($case['auth_cookie_parse_profile']??null)==='explicit_php_truthy_cookie_arbitrary_byte_scheme');
 $cookie=auth_cookie_parse_hex($case['input']??null);$scheme=auth_cookie_parse_hex($case['scheme']??null);
 auth_cookie_parse_require(strlen($cookie)+strlen($scheme)<=131072&&hex2bin($cookie)===$raw);
 $limits=$case['limits']??null;
 auth_cookie_parse_require(is_array($limits)&&count($limits)===1&&($limits['max_input_bytes']??null)===65536);
 return hex2bin($scheme);
}

// These tripwires are fixture-local definitions, not an implementation of SSL
// detection or PHP/WP cookie lookup. The accepted source path calls neither.
if(function_exists('is_ssl')){throw new RuntimeException('Explicit parser fixture function conflict');}
else{
 function is_ssl(){
  auth_cookie_parse_require(auth_cookie_parse_active());
  ++$GLOBALS['auth_cookie_parse_scope']['ssl_calls'];
  throw new RuntimeException('Explicit cookie parser SSL selection forbidden');
 }
}
final class AuthCookieParseCookieTrap implements ArrayAccess {
 private int$count=0;
 public function calls():int{return$this->count;}
 public function offsetExists(#[\SensitiveParameter]mixed$offset):bool{
  ++$this->count;throw new RuntimeException('Explicit cookie parser environment access forbidden');
 }
 public function offsetGet(#[\SensitiveParameter]mixed$offset):mixed{
  ++$this->count;throw new RuntimeException('Explicit cookie parser environment access forbidden');
 }
 public function offsetSet(#[\SensitiveParameter]mixed$offset,#[\SensitiveParameter]mixed$value):void{
  ++$this->count;throw new RuntimeException('Explicit cookie parser environment access forbidden');
 }
 public function offsetUnset(#[\SensitiveParameter]mixed$offset):void{
  ++$this->count;throw new RuntimeException('Explicit cookie parser environment access forbidden');
 }
}

function oracle_auth_cookie_parse(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$case) {
 global$auth_cookie_parse_counts;
 try{$scheme=auth_cookie_parse_guard($raw,$case);}
 catch(Throwable$unused){throw new RuntimeException('Explicit cookie parser input/profile unsupported');}
 $names=['_COOKIE','_SERVER','auth_cookie_parse_scope','wp_filter','wp_filters','wp_current_filter','wp_actions','wpdb','wp_object_cache'];
 $saved=[];foreach($names as$name){$saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
 $cookie_trap=new AuthCookieParseCookieTrap();
 try{
  $GLOBALS['_COOKIE']=$cookie_trap;$GLOBALS['auth_cookie_parse_scope']=['active'=>true,'ssl_calls'=>0];
  ++$auth_cookie_parse_counts['calls'];
  try{$result=wp_parse_auth_cookie($raw,$scheme);}
  catch(Throwable$unused){throw new RuntimeException('Explicit cookie parser reference failed');}
  $should_parse=substr_count($raw,'|')===3;
  if($should_parse){
   auth_cookie_parse_require(is_array($result)&&array_keys($result)===['username','expiration','token','hmac','scheme']);
   foreach($result as$value){auth_cookie_parse_require(is_string($value));}
   auth_cookie_parse_require(implode('|',array_slice($result,0,4))===$raw&&$result['scheme']===$scheme);
  }else{auth_cookie_parse_require($result===false);}
  auth_cookie_parse_require($GLOBALS['_COOKIE']===$cookie_trap&&$cookie_trap->calls()===0
   &&$GLOBALS['auth_cookie_parse_scope']===['active'=>true,'ssl_calls'=>0]);
  foreach($names as$name){if(in_array($name,['_COOKIE','auth_cookie_parse_scope'],true)){continue;}
   [$had,$old]=$saved[$name];auth_cookie_parse_require(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}
  ++$auth_cookie_parse_counts[$result===false?'false_results':'parsed'];
  return$result;
 }finally{
  try{
   if(auth_cookie_parse_active()){$value=$GLOBALS['auth_cookie_parse_scope']['ssl_calls']??null;
    if(is_int($value)&&$value>=0){$auth_cookie_parse_counts['ssl_calls']+=$value;}}
   $auth_cookie_parse_counts['cookie_accesses']+=$cookie_trap->calls();
  }finally{
   foreach($saved as$name=>[$had,$old]){if($had){$GLOBALS[$name]=$old;}else{unset($GLOBALS[$name]);}}
   foreach($saved as$name=>[$had,$old]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$old)){
    throw new RuntimeException('Explicit cookie parser restoration failed');}}
   ++$auth_cookie_parse_counts['restored'];
  }
 }
}
