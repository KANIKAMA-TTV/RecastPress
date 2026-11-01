"""Synthetic wrapper contracts, distinct from actual-WP differential comparisons."""
import json
from pathlib import Path
import subprocess
import unittest

import wp_bcrypt_profile
import wp_rehash_profile

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / '.reference/php-8.5.12/php.exe'

@unittest.skipUnless(PHP.is_file(), 'pinned development PHP unavailable')
class WpRehashOracleTests(unittest.TestCase):
    def scope(self, fail):
        code = r'''
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_wp_rehash');
$fail=$argv[2]==='true';$count=0;
$hooks=['wp_hash_password_algorithm','wp_hash_password_options','password_needs_rehash'];
$wp_rehash_hook_counts=array_fill_keys($hooks,0);
function wp_password_needs_rehash(#[\SensitiveParameter]$hash,$uid){
 global$fail,$hooks;
 if($uid!==0||$GLOBALS['wp_filter']!==[]||$GLOBALS['wp_filters']!==[]||$GLOBALS['wp_current_filter']!==[]||$GLOBALS['wp_hasher']!==null){throw new LogicException('wrong scope');}
 $GLOBALS['wp_filters']=array_fill_keys($hooks,1);
 if($fail){throw new RuntimeException('injected failure');}
 return !str_starts_with($hash,'$wp');
}
foreach([0,1,2,3]as$state){foreach(['',"\xff\0",$argv[3],str_repeat('x',8192)]as$hash){
 $saved=[];foreach(['wp_filter','wp_filters','wp_current_filter','wp_hasher']as$name){
  if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>['sentinel'=>new stdClass()],default=>[]};}
  $saved[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];
 }
 if($state===0){unset($GLOBALS['wp_actions']);}else{$GLOBALS['wp_actions']=$state===1?null:['untouched'=>5];}
 $had_actions=array_key_exists('wp_actions',$GLOBALS);$actions=$GLOBALS['wp_actions']??null;
 $caught=false;try{$result=oracle_wp_rehash($hash,'default_bcrypt_empty');}
 catch(RuntimeException$e){if(!$fail||$e->getMessage()!=='injected failure'){throw$e;}$caught=true;}
 if($caught!==$fail||(!$fail&&$result!==!str_starts_with($hash,'$wp'))){throw new LogicException('result differs');}
 if(array_key_exists('wp_actions',$GLOBALS)!==$had_actions||($had_actions&&$GLOBALS['wp_actions']!==$actions)){throw new LogicException('actions changed');}
 foreach($saved as$name=>[$had,$value]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$value)){throw new LogicException('scope leaked');}}
 $before=$wp_rehash_hook_counts;
 foreach([[null,'default_bcrypt_empty'],[true,'default_bcrypt_empty'],[1,'default_bcrypt_empty'],[[],'default_bcrypt_empty'],['',null],['',true],['$wpSECRET_REHASH_INPUT','default_bcrypt_empty'],[$argv[3]."\n",'default_bcrypt_empty']]as$bad){
  $refused=false;try{oracle_wp_rehash(...$bad);}catch(RuntimeException$e){$refused=true;}
  if(!$refused||$wp_rehash_hook_counts!==$before){throw new LogicException('bad admission called reference');}
  foreach($saved as$name=>[$had,$value]){if(array_key_exists($name,$GLOBALS)!==$had||($had&&$GLOBALS[$name]!==$value)){throw new LogicException('bad admission scope leaked');}}
 }
 ++$count;
}}
echo json_encode(['restored'=>$count,'hooks'=>$wp_rehash_hook_counts,'failed'=>$fail]),"\n";
'''
        result = subprocess.run([str(PHP), '-n', '-r', code, str(ROOT), 'true' if fail else 'false',
            wp_bcrypt_profile.VECTORS['cost12']], stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=15)
        self.assertEqual(result.stderr, b'')
        self.assertEqual(json.loads(result.stdout), {'restored': 16,
            'hooks': dict.fromkeys(wp_rehash_profile.HOOKS, 0 if fail else 16), 'failed': fail})

    def test_success_restores_four_global_states_and_actions_with_exact_three_hooks(self):
        self.scope(False)

    def test_injected_inner_failure_and_bad_domain_restore_without_counting_success(self):
        self.scope(True)

    def test_exception_trace_does_not_disclose_hash_or_profile_arguments(self):
        code = r'''
ini_set('display_errors','stderr');ini_set('log_errors','0');ini_set('zend.exception_ignore_args','0');
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_wp_rehash');
oracle_wp_rehash('$wpSECRET_REHASH_INPUT','SECRET_REHASH_PROFILE');
'''
        result = subprocess.run([str(PHP), '-n', '-r', code, str(ROOT)], stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, timeout=15)
        self.assertNotEqual(result.returncode, 0); self.assertEqual(result.stdout, b'')
        for marker in (b'SECRET_REHASH_INPUT', b'SECRET_REHASH_PROFILE'):
            self.assertNotIn(marker, result.stderr)

if __name__ == '__main__': unittest.main()
