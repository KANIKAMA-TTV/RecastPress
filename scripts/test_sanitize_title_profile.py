"""Default callback data/scope/privacy contracts, separate from finite PHP corpus."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import sanitize_title_profile as profile

ROOT=Path(__file__).resolve().parents[1];PHP=ROOT/'.reference/php-8.5.12/php.exe'

class DefaultTitleDataTests(unittest.TestCase):
    def test_callback_is_one_pinned_data_tuple_without_loading_default_filters(self):
        path=ROOT/'.reference/wordpress-7.1.3/wp-includes/default-filters.php'
        if not path.is_file():self.skipTest('pinned WP unavailable')
        self.assertEqual(profile.check_callback(),('sanitize_title','sanitize_title_with_dashes',10,3))
        with tempfile.TemporaryDirectory(dir=ROOT/'target')as directory:
            fake=Path(directory)/'callbacks.php'
            for wire in (path.read_bytes()+b'\n',b'<?php add_filter(\'sanitize_title\',\'other\',10,3);',b'x'*1048577):
                fake.write_bytes(wire)
                with self.assertRaises(ValueError):profile.check_callback(fake)

@unittest.skipUnless(PHP.is_file(),'pinned PHP unavailable')
class DefaultTitleScopeTests(unittest.TestCase):
    def php(self,code,*args):
        run=subprocess.run([str(PHP),'-n','-d','memory_limit=128M','-r',code,str(ROOT),*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
        self.assertEqual(run.returncode,0,run.stderr);self.assertEqual(run.stderr,b'');return json.loads(run.stdout)

    def scope(self,fail):
        code=r'''
$root=$argv[1].'/.reference/wordpress-7.1.3';setlocale(LC_CTYPE,'C');ini_set('zend.exception_ignore_args','0');
require$argv[1].'/scripts/reference_functions.php';require$root.'/wp-includes/plugin.php';require$root.'/wp-includes/compat-utf8.php';require$root.'/wp-includes/utf8.php';
foreach(['oracle_default_sanitize_title','default_title_callback','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
foreach(['remove_accents','sanitize_title_with_dashes','utf8_uri_encode']as$n){load_reference_function($root.'/wp-includes/formatting.php',$n);}
foreach(['mbstring_binary_safe_encoding','reset_mbstring_encoding']as$n){load_reference_function($root.'/wp-includes/functions.php',$n);}
load_reference_function($root.'/wp-includes/l10n.php','get_locale');
$fail=$argv[2]==='true';
if($fail){function sanitize_title($raw,$fallback,$context){throw new RuntimeException('SECRET_INNER_TITLE');}}
else{load_reference_function($root.'/wp-includes/formatting.php','sanitize_title');}
function is_multisite(){throw new LogicException('DB branch reached');}function get_option(){throw new LogicException('DB branch reached');}
$default_title_calls=0;$default_title_filter_calls=0;$default_title_locale_calls=['zero'=>0,'one'=>0];$default_title_restored=0;$locale_db_branch_calls=0;$count=0;
foreach([0,1,2,3]as$state){foreach(['en_US',"de\0\xff",'ca','00']as$requested){foreach(['save','display','query']as$context){foreach(['','0',"\xc3\x84","\xc2\xa3"]as$raw){
 $saved=[];foreach(['wp_filter','wp_filters','wp_current_filter','locale','wp_local_package','wp_actions','wp_hasher']as$name){
  if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>['sentinel'=>new stdClass()],default=>[]};}
  $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];
 }
 $fallback="\xff\0<RAW>";$caught=false;
 try{$value=oracle_default_sanitize_title($raw,$fallback,$context,$requested,'valid_utf8_no_lt_explicit_locale_default_title_callback');
  $expected=match($raw){''=>$fallback,'0'=>'0',"\xc3\x84"=>$context==='save'?(str_starts_with($requested,'de')?'ae':'a'):'%c3%84',default=>$context==='save'?$fallback:'%c2%a3'};
  if($value!==$expected){throw new LogicException('source result mismatch');}
 }catch(RuntimeException$e){
  if(!$fail||$e->getMessage()!=='Default title reference failed'||$e->getPrevious()!==null||str_contains((string)$e,'SECRET_')){throw$e;}$caught=true;
 }
 if($caught!==$fail){throw new LogicException('error converted to fallback');}
 foreach($saved as$name=>[$had,$old]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$old)){throw new LogicException('source globals leaked');}}
 ++$count;
}}}}
echo json_encode(['restored'=>$default_title_restored,'count'=>$count,'calls'=>$default_title_calls,'filters'=>$default_title_filter_calls,'locale'=>$default_title_locale_calls,'db'=>$locale_db_branch_calls]);
'''
        self.assertEqual(self.php(code,'true'if fail else'false'),{'restored':192,'count':192,'calls':0 if fail else 192,'filters':0 if fail else 192,'locale':{'zero':0 if fail else 160,'one':0 if fail else 32},'db':0})

    def test_actual_default_callback_restores_seven_global_states_and_uses_fallback_only_for_empty(self):self.scope(False)

    def test_callback_exception_is_fixed_private_restored_and_never_fallback(self):self.scope(True)

    def test_domain_and_each_normalizer_presence_are_refused_before_actual_callback(self):
        code=r'''
setlocale(LC_CTYPE,'C');require$argv[1].'/scripts/reference_functions.php';
foreach(['oracle_default_sanitize_title','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
function wp_is_valid_utf8($raw){return preg_match('//u',$raw)===1;}
function sanitize_title(){throw new LogicException('reference reached');}function default_title_callback(){throw new LogicException('registration reached');}
$p='valid_utf8_no_lt_explicit_locale_default_title_callback';$n=0;
foreach([[null,'','save','en_US'],['',null,'save','en_US'],['','','SAVE','en_US'],['','','save',''],['','','display','0'],['','','query',null],['<','','save','en_US'],["\xc3\0\xa9",'','save','en_US']]as[$raw,$fallback,$context,$locale]){
 try{oracle_default_sanitize_title($raw,$fallback,$context,$locale,$p);throw new LogicException('admitted');}catch(RuntimeException$e){++$n;}
}
eval('function '.$argv[2].'(...$args){return true;}');
try{oracle_default_sanitize_title('','','save','en_US',$p);throw new LogicException('normalizer admitted');}catch(RuntimeException$e){++$n;}
echo json_encode(['refused'=>$n]);
'''
        for name in ('normalizer_normalize','normalizer_is_normalized'):
            self.assertEqual(self.php(code,name),{'refused':9})

    def test_callback_registration_shape_is_checked_before_call_and_scope_restored(self):
        code=r'''
setlocale(LC_CTYPE,'C');require$argv[1].'/scripts/reference_functions.php';require$argv[1].'/.reference/wordpress-7.1.3/wp-includes/plugin.php';
foreach(['oracle_default_sanitize_title','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
function wp_is_valid_utf8($raw){return true;}function default_title_callback(){return['sanitize_title','sanitize_title_with_dashes',10,2];}
function sanitize_title_with_dashes($raw){return$raw;}function sanitize_title(){throw new LogicException('actual called');}
$locale_db_branch_calls=0;$default_title_restored=0;$saved=[];foreach(['wp_filter','wp_filters','wp_current_filter','locale','wp_local_package','wp_actions','wp_hasher']as$n){unset($GLOBALS[$n]);}
try{oracle_default_sanitize_title('','','save','en_US','valid_utf8_no_lt_explicit_locale_default_title_callback');throw new LogicException('bad registration admitted');}catch(RuntimeException$e){if($e->getMessage()!=='Default title callback registration differs'){throw$e;}}
foreach(['wp_filter','wp_filters','wp_current_filter','locale','wp_local_package','wp_actions','wp_hasher']as$n){if(array_key_exists($n,$GLOBALS)){throw new LogicException('registration failure leaked');}}
echo json_encode(['restored'=>$default_title_restored]);
'''
        self.assertEqual(self.php(code),{'restored':1})

if __name__=='__main__':unittest.main()
