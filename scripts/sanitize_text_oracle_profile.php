<?php
/** Fixed UTF8-site / no literal less-than / empty own filter profile. */
$sanitize_text_counts=array_fill_keys(['calls','field_hook','area_hook','restored'],0);
function sanitize_text_metadata(){
 global$root,$sanitize_text_counts;
 $profile=json_decode('{"php_version":"8.5.12","name":"raw_no_lt_fixed_utf8_no_mbstring_empty_filters","input_type":"arbitrary_bytes_without_literal_lt","blog_charset":"UTF-8","option_fixture":"immutable_utf8_not_core_option_resolution","mbstring_loaded":false,"mb_check_encoding_exists":false,"mb_convert_encoding_exists":false,"ctype_locale":"C","invalid_utf8":"whole_input_empty_before_percent_processing","utf8_static":"reflection_null_or_true_before_empty_keeps_state_nonempty_after_true_no_prime","utf8_internal_calls":"uninstrumented_actual_source_chain","html_branch":"source_unreachable_raw_no_lt_and_strip_false_not_measured_calls","filter_registry":"one_empty_own_wp_hook_no_all_callbacks","filter_arguments":2,"filter_arguments_basis":"pinned_source_without_observer_callback","native_filter_bookkeeping":false,"percent_order":"first_case_insensitive_match_exact_selected_three_byte_global_nonrecursive_delete_repeat","percent_rounds":"successful_selected_triplet_deletion_passes_only","round_budget_basis":"raw_input_4096_bytes_floor_div_three_is_sufficient","fixture_limits":{"max_input_bytes":4096,"max_output_bytes":4096,"max_percent_rounds":1365},"source_sha256":"350f6b32303915df970bb0e3ea360fcd54bb0b8a7ecc3d504c3d860ab9468d32"}',true,32,JSON_THROW_ON_ERROR);
 $profile['php_version']=PHP_VERSION;$profile['mbstring_loaded']=extension_loaded('mbstring');
 $profile['mb_check_encoding_exists']=function_exists('mb_check_encoding');
 $profile['mb_convert_encoding_exists']=function_exists('mb_convert_encoding');$profile['ctype_locale']=setlocale(LC_CTYPE,0);
 $profile['source_sha256']=hash_file('sha256',$root.'/wp-includes/formatting.php');
 return['sanitize_text_profile'=>$profile,'sanitize_text_counts'=>$sanitize_text_counts];
}

function sanitize_text_require(#[\SensitiveParameter]$condition){
 if(!$condition){throw new RuntimeException('Text sanitizer input/profile unsupported');}
}
function sanitize_text_load_reference_functions(#[\SensitiveParameter]$root){
 foreach(['sanitize_text_field','sanitize_textarea_field','_sanitize_text_fields']as$name){
  load_reference_function($root.'/wp-includes/formatting.php',$name);
 }
}
function sanitize_text_hex(#[\SensitiveParameter]$wire){
 sanitize_text_require(is_array($wire)&&count($wire)===2&&($wire['type']??null)==='string'
  &&isset($wire['hex'])&&is_string($wire['hex'])&&strlen($wire['hex'])<=8192
  &&strlen($wire['hex'])%2===0&&strspn($wire['hex'],'0123456789abcdefABCDEF')===strlen($wire['hex']));
 return$wire['hex'];
}
function sanitize_text_input(#[\SensitiveParameter]$case){
 try{return hex2bin(sanitize_text_hex($case['input']??null));}
 catch(Throwable$unused){throw new RuntimeException('Text sanitizer input/profile unsupported');}
}
function sanitize_text_guard(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$case){
 sanitize_text_require(is_string($raw)&&strlen($raw)<=4096&&!str_contains($raw,'<')
  &&in_array($case['api']??null,['sanitize_text_field','sanitize_textarea_field'],true)
  &&($case['text_profile']??null)==='raw_no_lt_fixed_utf8_no_mbstring_empty_filters'
  &&($case['empty_filters']??null)===true&&$raw===sanitize_text_input($case)
  &&!extension_loaded('mbstring')&&!function_exists('mb_check_encoding')&&!function_exists('mb_convert_encoding')
  &&setlocale(LC_CTYPE,0)==='C');
 $limits=$case['limits']??null;
 sanitize_text_require(is_array($limits)&&count($limits)===3
  &&($limits['max_input_bytes']??null)===4096&&($limits['max_output_bytes']??null)===4096
  &&($limits['max_percent_rounds']??null)===1365);
 $statics=(new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables();
 sanitize_text_require(array_key_exists('is_utf8',$statics)&&($statics['is_utf8']===null||$statics['is_utf8']===true));
 return[$case['api'],$statics['is_utf8']];
}
function oracle_sanitize_text(#[\SensitiveParameter]$raw,#[\SensitiveParameter]$case){
 global$sanitize_text_counts;
 try{[$api,$before]=sanitize_text_guard($raw,$case);}
 catch(Throwable$unused){throw new RuntimeException('Text sanitizer input/profile unsupported');}
 $names=['wp_filter','wp_filters','wp_current_filter','wp_actions','wp_hasher','wpdb','wp_object_cache','auth_cookie_scope','title_scope'];
 $saved=[];foreach($names as$name){$saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
 try{
  $hook=new WP_Hook();$GLOBALS['wp_filter']=[$api=>$hook];$GLOBALS['wp_filters']=[];$GLOBALS['wp_current_filter']=[];
  // Existing main fixture get_option guards these unrelated active scopes.
  // Clear/restore them; never prime UTF8 statics or add option read counters.
  $GLOBALS['auth_cookie_scope']=null;$GLOBALS['title_scope']=null;
  preg_match('//','');++$sanitize_text_counts['calls'];
  try{$result=$api($raw);}catch(Throwable$unused){throw new RuntimeException('Text sanitizer reference failed');}
  $after=(new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables()['is_utf8'];
  $expected_after=$raw===''?$before:true;
  sanitize_text_require(is_string($result)&&strlen($result)<=4096&&preg_last_error()===PREG_NO_ERROR
   &&$after===$expected_after&&$GLOBALS['wp_filters']===[$api=>1]&&$GLOBALS['wp_current_filter']===[]
   &&array_keys($GLOBALS['wp_filter'])===[$api]&&$GLOBALS['wp_filter'][$api]===$hook&&$hook->callbacks===[]
   &&$GLOBALS['auth_cookie_scope']===null&&$GLOBALS['title_scope']===null);
  foreach(['wp_actions','wp_hasher','wpdb','wp_object_cache']as$name){[$had,$old]=$saved[$name];
   sanitize_text_require(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}
  $sanitize_text_counts[$api==='sanitize_text_field'?'field_hook':'area_hook']+=$GLOBALS['wp_filters'][$api];
  return$result;
 }finally{
  foreach($saved as$name=>[$had,$old]){if($had){$GLOBALS[$name]=$old;}else{unset($GLOBALS[$name]);}}
  foreach($saved as$name=>[$had,$old]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$old)){
   throw new RuntimeException('Text sanitizer restoration failed');}}
  ++$sanitize_text_counts['restored'];
 }
}
