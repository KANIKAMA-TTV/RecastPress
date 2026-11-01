"""Conditional domain, real branch examples and wrapper privacy, not slug policy."""
import json
from pathlib import Path
import subprocess
import unittest

import truncate_slug_profile as profile

ROOT=Path(__file__).resolve().parents[1];PHP=ROOT/'.reference/php-8.5.12/php.exe'

class TruncateSlugDomainTests(unittest.TestCase):
    def test_branching_and_nonrecursive_decode_guard_only_the_changed_path(self):
        self.assertEqual(profile.decoded(b'+%2B%252B%25%GG'),b' +%2B%%GG')
        for raw,length,branch,allowed in [(b'%FF',3,'raw_no_truncate',True),(b'%FF',2,'decoded_uri',False),
            (b'\xff',0,'raw_substr',True),(b'\xff+',3,'raw_no_truncate',True),(b'\xff+',1,'decoded_uri',False),
            (b'\xff%GG',2,'raw_substr',True),(b'A%41\xff',1,'decoded_uri',False)]:
            self.assertEqual(profile.branch(raw,length),branch);self.assertEqual(profile.admitted(raw,length),allowed)

@unittest.skipUnless(PHP.is_file(),'pinned PHP unavailable')
class TruncateSlugOracleTests(unittest.TestCase):
    def php(self,code,*args):
        result=subprocess.run([str(PHP),'-n','-d','memory_limit=128M','-r',code,str(ROOT),*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
        self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(result.stderr,b'');return json.loads(result.stdout)

    def test_actual_named_source_preserves_binary_cuts_and_zero_length_uri_unlimited(self):
        code=r'''
$root=$argv[1].'/.reference/wordpress-7.1.3';require$argv[1].'/scripts/reference_functions.php';
require$root.'/wp-includes/compat-utf8.php';require$root.'/wp-includes/utf8.php';
foreach(['oracle_truncate_slug','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
load_reference_function($root.'/wp-includes/post.php','_truncate_post_slug');load_reference_function($root.'/wp-includes/formatting.php','utf8_uri_encode');
foreach(['mbstring_binary_safe_encoding','reset_mbstring_encoding']as$n){load_reference_function($root.'/wp-includes/functions.php',$n);}
$truncate_slug_calls=0;$truncate_slug_branches=['raw_no_truncate'=>0,'raw_substr'=>0,'decoded_uri'=>0];$out=[];
foreach([['%FF',3],["\xc3\xa9Z",1],['+',0],['%2B',0],['%252B',0],['%C3%A9--',6],['ab---',4],["\0\xff--",4],['A/B',2]]as[$raw,$length]){$out[]=bin2hex(oracle_truncate_slug($raw,$length,'conditional_decoded_utf8_no_mbstring'));}
echo json_encode(['out'=>$out,'calls'=>$truncate_slug_calls,'branches'=>$truncate_slug_branches]);
'''
        values=[b'%FF',b'\xc3',b'%20',b'%2B',b'%252B',b'%c3%a9',b'ab',b'\0\xff',b'A/']
        self.assertEqual(self.php(code),{'out':[v.hex()for v in values],'calls':9,'branches':{'raw_no_truncate':2,'raw_substr':3,'decoded_uri':4}})

    def test_types_and_changed_invalid_utf8_are_rejected_before_source_with_no_raw_guard(self):
        code=r'''
require$argv[1].'/scripts/reference_functions.php';foreach(['oracle_truncate_slug','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
$calls=0;$checked=0;function _truncate_post_slug($raw,$length){global$calls;++$calls;return$raw;}
function wp_is_valid_utf8($raw){global$checked;++$checked;return preg_match('//u',$raw)===1;}
$truncate_slug_calls=0;$truncate_slug_branches=['raw_no_truncate'=>0,'raw_substr'=>0,'decoded_uri'=>0];$p='conditional_decoded_utf8_no_mbstring';$rejected=0;
foreach([[null,0],[false,0],[[],0],['',null],['',true],['',1.0],['',-1],['','1'],['%FF',0],["A%41\xff",1]]as[$raw,$length]){
 try{oracle_truncate_slug($raw,$length,$p);throw new LogicException('admitted');}catch(RuntimeException$e){++$rejected;}
}
oracle_truncate_slug("\xff",1,$p);oracle_truncate_slug("\xff",0,$p);oracle_truncate_slug('%GG',1,$p);
echo json_encode(['calls'=>$calls,'checked'=>$checked,'rejected'=>$rejected,'branches'=>$truncate_slug_branches]);
'''
        self.assertEqual(self.php(code),{'calls':3,'checked':2,'rejected':10,'branches':{'raw_no_truncate':1,'raw_substr':2,'decoded_uri':0}})

    def test_no_mb_guard_and_fixed_inner_exception_never_expose_payload_or_succeed(self):
        code=r'''
ini_set('zend.exception_ignore_args','0');require$argv[1].'/scripts/reference_functions.php';foreach(['oracle_truncate_slug','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
function wp_is_valid_utf8($raw){return true;}function _truncate_post_slug($raw,$length){throw new RuntimeException('SECRET_INNER_TRUNCATE');}
$truncate_slug_calls=0;$truncate_slug_branches=['raw_no_truncate'=>0,'raw_substr'=>0,'decoded_uri'=>0];$p='conditional_decoded_utf8_no_mbstring';
try{oracle_truncate_slug('SECRET_TRUNCATE_INPUT',100,$p);throw new LogicException('no exception');}catch(RuntimeException$e){if($e->getMessage()!=='Truncate slug reference failed'||$e->getPrevious()!==null||str_contains((string)$e,'SECRET_')){throw new LogicException('secret diagnostic');}}
eval('function '.$argv[2].'(...$args){return "fixture";}');
$refused=false;try{oracle_truncate_slug('',0,$p);}catch(RuntimeException$e){$refused=true;}
echo json_encode(['refused'=>$refused,'calls'=>$truncate_slug_calls,'branches'=>$truncate_slug_branches]);
'''
        for name in ('mb_strtolower','mb_internal_encoding'):
            self.assertEqual(self.php(code,name),{'refused':True,'calls':0,'branches':{'raw_no_truncate':0,'raw_substr':0,'decoded_uri':0}})

if __name__=='__main__':unittest.main()
