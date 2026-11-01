"""No real secrets: synthetic exact-wrapper scope/admission process checks."""
import json
from pathlib import Path
import subprocess
import unittest
import wp_bcrypt_profile as profile

ROOT=Path(__file__).resolve().parents[1]
PHP=ROOT/".reference/php-8.5.12/php.exe"

@unittest.skipUnless(PHP.is_file(),"pinned development PHP unavailable")
class WpBcryptOracleTests(unittest.TestCase):
    def run_scope(self, fail):
        code=r'''
require $argv[1].'/scripts/reference_functions.php';
load_reference_function($argv[1].'/scripts/wp_oracle.php','oracle_wp_bcrypt_verify');
$fail=$argv[2]==='true';$wp_bcrypt_calls=0;$hash=$argv[3];
function wp_check_password(#[\SensitiveParameter]$p,#[\SensitiveParameter]$h,$uid){
 global$fail;
 if($uid!==0||$GLOBALS['wp_hasher']!==null||$GLOBALS['wp_filter']!==[]||$GLOBALS['wp_filters']!==[]||$GLOBALS['wp_current_filter']!==[]){throw new LogicException('wrong bcrypt callback scope');}
 $GLOBALS['wp_filters']=['check_password'=>1];
 if($fail){throw new RuntimeException('injected provider failure');}
 return strlen($p)<=4096;
}
$names=['wp_filter','wp_filters','wp_current_filter','wp_hasher'];$restored=0;
foreach([0,1,2]as$state){foreach([4,12]as$max){foreach(['',"dummy\0\xff",str_repeat('x',4097)]as$password){
 $saved=[];foreach($names as$name){if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=$state===1?null:($name==='wp_hasher'?(object)['sentinel'=>true]:['sentinel']);}$saved[$name]=['had'=>array_key_exists($name,$GLOBALS),'value'=>$GLOBALS[$name]??null];}
 if($state===0){unset($GLOBALS['wp_actions']);}else{$GLOBALS['wp_actions']=$state===1?null:['untouched'=>13];}
 $had_actions=array_key_exists('wp_actions',$GLOBALS);$actions=$GLOBALS['wp_actions']??null;$caught=false;
 try{$out=oracle_wp_bcrypt_verify($password,$hash,$max,'wp_bcrypt_empty');}
 catch(RuntimeException$e){if(!$fail||$e->getMessage()!=='injected provider failure'){throw$e;}$caught=true;}
 if($caught!==$fail||(!$fail&&$out!==(strlen($password)<=4096))||array_key_exists('wp_actions',$GLOBALS)!==$had_actions||($had_actions&&$GLOBALS['wp_actions']!==$actions)){throw new LogicException('bcrypt result/actions changed');}
 foreach($saved as$name=>$old){if(array_key_exists($name,$GLOBALS)!==$old['had']||($old['had']&&$GLOBALS[$name]!==$old['value'])){throw new LogicException('bcrypt scope leaked');}}
 $before=$wp_bcrypt_calls;
 foreach([[null,$hash,4,'wp_bcrypt_empty'],['',null,4,'wp_bcrypt_empty'],['',$hash,false,'wp_bcrypt_empty'],['',$hash,4.0,'wp_bcrypt_empty'],['',$hash,3,'wp_bcrypt_empty'],['',$hash,32,'wp_bcrypt_empty'],['',$hash,4,true],['','legacy',4,'wp_bcrypt_empty']]as$bad){
  $rejected=false;try{oracle_wp_bcrypt_verify(...$bad);}catch(RuntimeException$e){$rejected=true;}
  if(!$rejected||$wp_bcrypt_calls!==$before){throw new LogicException('bad bcrypt admission changed calls');}
  foreach($saved as$name=>$old){if(array_key_exists($name,$GLOBALS)!==$old['had']||($old['had']&&$GLOBALS[$name]!==$old['value'])){throw new LogicException('bad bcrypt admission changed globals');}}
 }
 ++$restored;
}}}
echo json_encode(['restored'=>$restored,'calls'=>$wp_bcrypt_calls,'inner_failure'=>$fail]),"\n";
'''
        run=subprocess.run([str(PHP),"-n","-r",code,str(ROOT),"true"if fail else"false",profile.VECTORS["dummy"]],stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True,timeout=10)
        self.assertEqual(run.stderr,b"")
        self.assertEqual(json.loads(run.stdout),{"restored":18,"calls":0 if fail else 18,"inner_failure":fail})
    def test_empty_callback_scope_restores_absence_null_values_and_prior_hasher_object(self):
        self.run_scope(False)
    def test_inner_failure_restores_scope_and_bad_admission_never_calls_provider_or_hook(self):
        self.run_scope(True)
