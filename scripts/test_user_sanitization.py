"""Wrapper/scope/privacy contracts; actual-WP corpus comparison is a separate step."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
PHP=ROOT/'.reference/php-8.5.12/php.exe'
WP=ROOT/'.reference/wordpress-7.1.3'

@unittest.skipUnless(PHP.is_file() and (WP/'wp-includes/l10n.php').is_file(),'pinned PHP/WP unavailable')
class UserSanitizationTests(unittest.TestCase):
    def php(self,code,*args,success=True):
        result=subprocess.run([str(PHP),'-n','-r',code,str(ROOT),*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
        if success:
            self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(result.stderr,b'');return json.loads(result.stdout)
        self.assertNotEqual(result.returncode,0);self.assertEqual(result.stdout,b'');return result

    def test_strip_forwards_raw_bytes_and_bool_and_rejects_tags_and_bad_types_before_reference(self):
        code=r'''
require $argv[1].'/scripts/reference_functions.php';load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_strip_all_tags');
$strip_all_tags_calls=0;$called=0;
function wp_strip_all_tags($raw,$flag){global$called;if(!is_bool($flag)){throw new LogicException('flag changed');}++$called;return"\xff";}
foreach(['',"\xff","\xc3\0\xa9"]as$raw){foreach([false,true]as$flag){if(oracle_strip_all_tags($raw,$flag,'no_literal_lt')!=="\xff"){throw new LogicException('raw result changed');}}}
$before=$called;
foreach([[null,false],[false,false],[[],false],['',null],['',0],['','yes'],['<',false]]as[$raw,$flag]){
 $refused=false;try{oracle_strip_all_tags($raw,$flag,'no_literal_lt');}catch(RuntimeException$e){$refused=true;}
 if(!$refused||$before!==$called){throw new LogicException('bad domain called reference');}
}
echo json_encode(['calls'=>$called,'success'=>$strip_all_tags_calls]),"\n";
'''
        self.assertEqual(self.php(code),{'calls':6,'success':6})

    def scope(self,fail):
        code=r'''
setlocale(LC_CTYPE,'C');
require $argv[1].'/.reference/wordpress-7.1.3/wp-includes/plugin.php';
require $argv[1].'/.reference/wordpress-7.1.3/wp-includes/compat-utf8.php';require $argv[1].'/.reference/wordpress-7.1.3/wp-includes/utf8.php';
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_sanitize_user');
load_reference_function($argv[1].'/.reference/wordpress-7.1.3/wp-includes/formatting.php','wp_strip_all_tags');
load_reference_function($argv[1].'/.reference/wordpress-7.1.3/wp-includes/l10n.php','get_locale');
$sanitize_user_calls=0;$sanitize_user_filter_calls=0;$sanitize_user_locale_calls=['zero'=>0,'one'=>0];$locale_db_branch_calls=0;
$called=0;$fail=$argv[2]==='true';$expected_raw='';$expected_strict=false;$expected_locale='';
function is_multisite(){throw new LogicException('DB reached');}
function sanitize_user($raw,$strict){global$called,$fail,$expected_raw,$expected_strict,$expected_locale;
 if($raw!==$expected_raw||$strict!==$expected_strict){throw new LogicException('args changed');}++$called;
 if(preg_match('/[\x80-\xff]/',wp_strip_all_tags($raw,false))===1&&get_locale()!==$expected_locale){throw new LogicException('resolved locale changed');}
 $result=apply_filters('sanitize_user','fixture',$raw,$strict);
 if($fail){throw new RuntimeException('SECRET_INNER_FAILURE');}return$result;
}
$restored=0;
foreach([0,1,2,3]as$state){foreach(['en_US',"de\0\xff",'ca',"0\0"]as$candidate_locale){foreach(['A',"\0","\xc3\0\xa9","\xf0\0\x9f\xa6\x80"]as$raw){foreach([false,true]as$strict){
 $saved=[];foreach(['wp_filter','wp_filters','wp_current_filter','locale','wp_local_package','wp_actions','wp_hasher']as$name){
  if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>['sentinel'=>new stdClass()],default=>[]};}
  $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];
 }
 $expected_raw=$raw;$expected_strict=$strict;$expected_locale=$candidate_locale;$caught=false;
 try{$result=oracle_sanitize_user($raw,$strict,$candidate_locale,'no_lt_utf8_resolved_locale_c_empty_filters');}
 catch(RuntimeException$e){if(!$fail||$e->getMessage()!=='User reference failed'){throw$e;}$caught=true;}
 if($caught!==$fail||(!$fail&&$result!=='fixture')){throw new LogicException('result mismatch');}
 foreach($saved as$name=>[$had,$value]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$value)){throw new LogicException('five globals/actions/hasher leaked');}}
 ++$restored;
}}}}
echo json_encode(['restored'=>$restored,'called'=>$called,'success'=>$sanitize_user_calls,'filters'=>$sanitize_user_filter_calls,'locale'=>$sanitize_user_locale_calls,'db'=>$locale_db_branch_calls]),"\n";
'''
        self.assertEqual(self.php(code,'true'if fail else'false'),{'restored':128,'called':128,
            'success':0 if fail else 128,'filters':0 if fail else 128,
            'locale':{'zero':0 if fail else 64,'one':0 if fail else 64},'db':0})

    def test_success_restores_five_globals_absence_null_and_values_with_actions_and_hasher_untouched(self):self.scope(False)

    def test_inner_failure_restores_all_scope_and_does_not_count_success(self):self.scope(True)

    def test_api66_explicit_locale_never_calls_actual_getter_or_db_branch_with_verified_traps(self):
        code=r'''
require $argv[1].'/.reference/wordpress-7.1.3/wp-includes/plugin.php';
require $argv[1].'/.reference/wordpress-7.1.3/wp-includes/compat-utf8.php';require $argv[1].'/.reference/wordpress-7.1.3/wp-includes/utf8.php';
require $argv[1].'/scripts/reference_functions.php';load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_remove_accents');
load_reference_function($argv[1].'/.reference/wordpress-7.1.3/wp-includes/formatting.php','remove_accents');
load_reference_function($argv[1].'/.reference/wordpress-7.1.3/wp-includes/l10n.php','get_locale');
$remove_accents_calls=0;$remove_accents_get_locale_calls=0;
function is_multisite(){throw new RuntimeException('DB trap');}
add_filter('locale',static function($value){throw new RuntimeException('locale trap');});
$GLOBALS['locale']='fixture';$caught=false;try{get_locale();}catch(RuntimeException$e){$caught=$e->getMessage()==='locale trap';}if(!$caught){throw new LogicException('ineffective locale trap');}
unset($GLOBALS['locale'],$GLOBALS['wp_local_package']);$caught=false;try{get_locale();}catch(RuntimeException$e){$caught=$e->getMessage()==='DB trap';}if(!$caught){throw new LogicException('ineffective DB trap');}
unset($GLOBALS['locale'],$GLOBALS['wp_local_package']);$GLOBALS['wp_filters']=[];
foreach(['en_US','de','ca',"\xff","\0"]as$explicit_locale){foreach(['',"\xc3\xa9","l\xc2\xb7l"]as$value){oracle_remove_accents($value,$explicit_locale,'valid_utf8_explicit_locale_no_intl');}}
echo json_encode(['calls'=>$remove_accents_calls,'ambient'=>$remove_accents_get_locale_calls,'localehook'=>did_filter('locale'),'package_exists'=>array_key_exists('wp_local_package',$GLOBALS)]),"\n";
'''
        self.assertEqual(self.php(code),{'calls':15,'ambient':0,'localehook':0,'package_exists':False})

    def test_inner_failure_trace_redacts_original_username_and_exception_message(self):
        code=r'''
ini_set('display_errors','stderr');ini_set('log_errors','0');ini_set('zend.exception_ignore_args','0');setlocale(LC_CTYPE,'C');
require $argv[1].'/scripts/reference_functions.php';load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_sanitize_user');
$sanitize_user_calls=0;$sanitize_user_filter_calls=0;$sanitize_user_locale_calls=['zero'=>0,'one'=>0];$locale_db_branch_calls=0;
function wp_strip_all_tags($value,$flag){return$value;}function wp_is_valid_utf8($value){return true;}
function sanitize_user($raw,$strict){throw new RuntimeException('SECRET_INNER_DETAILS');}
oracle_sanitize_user('SECRET_USERNAME',false,'SECRET_RESOLVED_LOCALE','no_lt_utf8_resolved_locale_c_empty_filters');
'''
        result=self.php(code,success=False);self.assertNotIn(b'SECRET_',result.stderr)

if __name__=='__main__':unittest.main()
