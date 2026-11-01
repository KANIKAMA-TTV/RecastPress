"""Explicit wrapper profile/domain/privacy checks; actual corpus is separate."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT=Path(__file__).resolve().parents[1]
PHP=ROOT/'.reference/php-8.5.12/php.exe'
WP=ROOT/'.reference/wordpress-7.1.3'

@unittest.skipUnless(PHP.is_file()and(WP/'wp-includes/formatting.php').is_file(),'pinned PHP/WP unavailable')
class SlugByteTests(unittest.TestCase):
    def php(self,code,*args):
        result=subprocess.run([str(PHP),'-n','-d','memory_limit=128M','-r',code,str(ROOT),*args],stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=20)
        self.assertEqual(result.returncode,0,result.stderr);self.assertEqual(result.stderr,b'')
        return json.loads(result.stdout)

    def test_actual_source_keeps_marker_collisions_precleanup_limit_controls_and_hex_cases(self):
        code=r'''
$root=$argv[1].'/.reference/wordpress-7.1.3';setlocale(LC_CTYPE,'C');require$argv[1].'/scripts/reference_functions.php';
require$root.'/wp-includes/compat-utf8.php';require$root.'/wp-includes/utf8.php';
foreach(['utf8_uri_encode','sanitize_title_with_dashes']as$n){load_reference_function($root.'/wp-includes/formatting.php',$n);}
foreach(['mbstring_binary_safe_encoding','reset_mbstring_encoding']as$n){load_reference_function($root.'/wp-includes/functions.php',$n);}
$rows=[];
foreach([['---41---','display'],['a'."\t".'b','save'],['a b','save'],[str_repeat('&a;',67),'display'],[str_repeat('a',199).'%41','save'],["\xc3\0\x84",'save'],['A/B','display'],['A/B','save']]as[$raw,$context]){$rows[]=bin2hex(sanitize_title_with_dashes($raw,'',$context));}
echo json_encode(['title'=>$rows,'uri'=>[utf8_uri_encode("\0\n/\xc3\xa9",0,true),utf8_uri_encode("A\xc3\xa9Z",6,false),utf8_uri_encode("A\xc3\xa9Z",7,false)]]);
'''
        self.assertEqual(self.php(code),{'title':[b'%41'.hex(),b'ab'.hex(),b'a-b'.hex(),b'a'.hex(),(b'a'*199+b'%').hex(),b'%c3%84'.hex(),b'ab'.hex(),b'a-b'.hex()],
            'uri':['%00%0A%2F%c3%a9','A','A%c3%a9']})

    def test_all_nonbyte_length_bool_context_and_raw_title_guards_run_before_reference(self):
        code=r'''
setlocale(LC_CTYPE,'C');require$argv[1].'/scripts/reference_functions.php';
foreach(['oracle_utf8_uri','oracle_title_dash','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
function wp_is_valid_utf8($raw){return preg_match('//u',$raw)===1;}
$calls=0;function utf8_uri_encode($raw,$length,$ascii){global$calls;++$calls;return$raw;}
function sanitize_title_with_dashes($raw,$unused,$context){global$calls;++$calls;return$raw;}
$utf8_uri_calls=0;$title_dash_calls=0;$uri='valid_utf8_explicit_length_bool_no_mbstring';$title='no_lt_post_nul_utf8_explicit_context_no_mbstring';
foreach([[null,0,false],[false,0,false],[[],0,false],['',-1,false],['',true,false],['',1.0,false],['','1',false],['',0,0],["\xff",0,false]]as[$raw,$length,$ascii]){try{oracle_utf8_uri($raw,$length,$ascii,$uri);throw new LogicException('admitted');}catch(RuntimeException$e){}}
foreach([[null,'','save'],['',null,'save'],['','unused','save'],['','',true],['','','query'],['<','','save'],["\xff",'','save']]as[$raw,$unused,$context]){try{oracle_title_dash($raw,$unused,$context,$title);throw new LogicException('admitted');}catch(RuntimeException$e){}}
if($calls!==0){throw new LogicException('invalid input called reference');}
oracle_utf8_uri("\0",0,false,$uri);oracle_title_dash("\xc3\0\xa9",'','display',$title);
echo json_encode(['calls'=>$calls,'uri'=>$utf8_uri_calls,'title'=>$title_dash_calls]);
'''
        self.assertEqual(self.php(code),{'calls':2,'uri':1,'title':1})

    def test_each_mbstring_function_presence_is_refused_without_reference_call(self):
        for name in ('mb_strtolower','mb_internal_encoding'):
            code=r'''
setlocale(LC_CTYPE,'C');require$argv[1].'/scripts/reference_functions.php';
foreach(['oracle_utf8_uri','oracle_title_dash','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
eval('function '.$argv[2].'(...$args){return "fixture";}');function wp_is_valid_utf8($raw){return true;}
function utf8_uri_encode(){throw new LogicException('reference called');}function sanitize_title_with_dashes(){throw new LogicException('reference called');}
$refused=0;try{oracle_utf8_uri('',0,false,'valid_utf8_explicit_length_bool_no_mbstring');}catch(RuntimeException$e){++$refused;}
try{oracle_title_dash('','','save','no_lt_post_nul_utf8_explicit_context_no_mbstring');}catch(RuntimeException$e){++$refused;}
echo json_encode(['refused'=>$refused]);
'''
            self.assertEqual(self.php(code,name),{'refused':2})

    def test_inner_reference_failure_has_no_input_or_previous_exception_trace(self):
        code=r'''
setlocale(LC_CTYPE,'C');ini_set('zend.exception_ignore_args','0');require$argv[1].'/scripts/reference_functions.php';
foreach(['oracle_utf8_uri','oracle_title_dash','slug_no_mb_guard']as$n){load_reference_function($argv[1].'/scripts/wp_oracle.php',$n);}
function wp_is_valid_utf8($raw){return true;}function utf8_uri_encode($raw,$l,$a){throw new RuntimeException('SECRET_INNER_URI');}function sanitize_title_with_dashes($raw,$unused,$context){throw new RuntimeException('SECRET_INNER_TITLE');}
$errors=[];foreach([false,true]as$title){try{if($title){oracle_title_dash('SECRET_TITLE','','save','no_lt_post_nul_utf8_explicit_context_no_mbstring');}else{oracle_utf8_uri('SECRET_URI',0,false,'valid_utf8_explicit_length_bool_no_mbstring');}}catch(RuntimeException$e){if(str_contains((string)$e,'SECRET_')||$e->getPrevious()!==null){throw new LogicException('secret trace');}$errors[]=$e->getMessage();}}
echo json_encode($errors);
'''
        self.assertEqual(self.php(code),['URI reference failed','Title reference failed'])

if __name__=='__main__':unittest.main()
