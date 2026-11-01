"""Pinned isolated PHP contracts, source rules, static state and private errors."""
import copy
import hashlib
import hmac
import json
import os
from pathlib import Path
import re
import sys
import unittest

HERE=Path(__file__).resolve().parent
ROOT=HERE.parent
sys.path.insert(0,str(ROOT/'scripts'))
import owned_probe
import sanitize_text_profile as profile
PHP=ROOT/'.reference/php-8.5.12/php.exe';WP=ROOT/'.reference/wordpress-7.1.3'
BOOT=r'''
$repo=$argv[1];$root=$repo.'/.reference/wordpress-7.1.3';$fixture=$argv[2];
error_reporting(E_ALL);ini_set('zend.exception_ignore_args','0');setlocale(LC_CTYPE,'C');
set_error_handler(static function($level,$message,$file,$line){throw new ErrorException($message,0,$level,$file,$line);});
require$repo.'/scripts/reference_functions.php';require$repo.'/scripts/php_value_codec.php';
require$root.'/wp-includes/plugin.php';require$root.'/wp-includes/compat-utf8.php';require$root.'/wp-includes/utf8.php';
load_reference_function($root.'/wp-includes/functions.php','is_utf8_charset');
load_reference_function($root.'/wp-includes/compat.php','_is_utf8_charset');
load_reference_function($root.'/wp-includes/formatting.php','wp_check_invalid_utf8');
function get_option(#[\SensitiveParameter]$name){
 foreach(['auth_cookie_scope','title_scope']as$key){if(is_array($GLOBALS[$key]??null)&&($GLOBALS[$key]['active']??false)===true){throw new RuntimeException('Unrelated active scope option forbidden');}}
 if($name!=='blog_charset'){throw new RuntimeException('Unexpected fixture option');}
 return$GLOBALS['charset_fixture']??'UTF-8';
}
require$fixture.'/sanitize_text_oracle_profile.php';
function check($condition){if(!$condition){throw new LogicException('Text sanitizer contract failed');}}
$cases=json_decode(stream_get_contents(STDIN),true,128,JSON_THROW_ON_ERROR);
'''


def expected(raw,keep_newlines):
    try:raw.decode('utf-8',errors='strict')
    except UnicodeDecodeError:return b'',0
    value=raw if keep_newlines else re.sub(rb'[\r\n\t ]+',b' ',raw)
    value=value.strip(b' \t\n\r\0\x0b');rounds=0
    while match:=re.search(rb'%[a-f0-9]{2}',value,re.I):
        value=value.replace(match[0],b'');rounds+=1
        if rounds>1365:raise ValueError('source-derived round bound violated')
    if rounds:value=re.sub(rb' +',b' ',value).strip(b' \t\n\r\0\x0b')
    return value,rounds


class SanitizeTextProfileTests(unittest.TestCase):
    def test_types_tight_round_family_and_completed_old77_identity(self):
        profile.validate_profile(copy.deepcopy(profile.PROFILE))
        for key,value in [('native_filter_bookkeeping',0),('filter_arguments',True)]:
            bad=copy.deepcopy(profile.PROFILE);bad[key]=value
            with self.assertRaises(ValueError):profile.validate_profile(bad)
        tight=b'%a'*1364+b'%41'+b'b'*1364
        self.assertEqual(len(tight),4095)
        self.assertEqual(expected(tight,False),(b'',1365))
        self.assertEqual(expected(b'x'+tight,True),(b'x',1365))
        cases=profile.cases();self.assertEqual(cases,profile.cases());self.assertGreater(len(cases),5000)
        counts=profile.expected_counts(cases);profile.validate_counts(counts,counts)
        with self.assertRaises(ValueError):profile.validate_counts(dict(counts,field_hook=True),counts)
        profile.validate_previous(profile.OLD_COUNT,profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256)
        with self.assertRaises(ValueError):profile.validate_previous(float(profile.OLD_COUNT),profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256)
        with self.assertRaises(ValueError):profile.validate_previous(profile.OLD_COUNT,profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,'0'*64)

    def test_registry_recomputes_both_api_counts_instead_of_trusting_forged_artifact_pairs(self):
        import compatibility_registry as registry
        cases=profile.cases();counts=profile.expected_counts(cases)
        args=[profile.PROFILE,counts,counts,len(cases),profile.OLD_COUNT,
              profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256]
        registry.validate_sanitize_text_coverage(*args)
        forged=dict(counts,field_hook=0,area_hook=len(cases))
        bad=list(args);bad[1]=forged;bad[2]=forged
        with self.assertRaises(ValueError):registry.validate_sanitize_text_coverage(*bad)
        bad=list(args);bad[3]=float(len(cases))
        with self.assertRaises(ValueError):registry.validate_sanitize_text_coverage(*bad)
        bad=list(args);bad[7]='0'*64
        with self.assertRaises(ValueError):registry.validate_sanitize_text_coverage(*bad)


@unittest.skipUnless(PHP.is_file()and(WP/'wp-includes/formatting.php').is_file(),'pinned PHP/WordPress unavailable')
class SanitizeTextOracleTests(unittest.TestCase):
    def setUp(self):self.addCleanup(owned_probe.reap_owned_probes)

    def php(self,body,cases,*,actual=True):
        load='sanitize_text_load_reference_functions($root);\n'if actual else''
        pid,result=owned_probe.run_captured_probe([str(PHP),'-n','-d','memory_limit=128M','-r',BOOT+load+body,str(ROOT),str(HERE)],
            os.environ.copy(),timeout=20,input_bytes=json.dumps(cases,ensure_ascii=True,separators=(',',':')).encode())
        self.assertGreater(pid,0);self.assertEqual(result.returncode,0,result.stdout or result.stderr)
        self.assertEqual(result.stderr,b'');self.assertLessEqual(len(result.stdout),8*1024*1024)
        return json.loads(result.stdout)

    def test_exact_newline_percent_cascade_nul_and_utf8_source_boundaries(self):
        raws=[b'a  b',b'a  b %41',b'a  b %GG',b'a\t%41\tb',b'%a%41b',b'%4%411',
              b'\x0c A\tB \x0c',b'a\0b',b'\0%41\0A\0',b'\xc3%41\xa9',b'\xff',b'%c3%a9']
        cases=[profile.fixture(raw,api)for raw in raws for api in profile.APIS]
        observed=self.php(r'''
$results=[];foreach($cases as$case){$results[]=encode_value(oracle_sanitize_text(sanitize_text_input($case),$case));}
echo json_encode(['results'=>$results,...sanitize_text_metadata()],JSON_THROW_ON_ERROR);
''',cases)
        wants=[]
        for raw in raws:
            for keep in(False,True):wants.append({'type':'string','hex':expected(raw,keep)[0].hex()})
        self.assertEqual(observed['results'],wants)
        self.assertEqual(expected(b'a  b',True)[0],b'a  b')
        self.assertEqual(expected(b'a  b %41',True)[0],b'a b')
        self.assertEqual(expected(b'a\t%41\tb',True)[0],b'a\t\tb')
        profile.validate_profile(observed['sanitize_text_profile'])
        profile.validate_counts(observed['sanitize_text_counts'],profile.expected_counts(cases))

    def test_all_finite_input_pairs_match_actual_sources_and_have_only_own_hooks(self):
        cases=profile.cases()
        observed=self.php(r'''
$results=[];foreach($cases as$case){$results[]=encode_value(oracle_sanitize_text(sanitize_text_input($case),$case));}
echo json_encode(['results'=>$results,...sanitize_text_metadata()],JSON_THROW_ON_ERROR);
''',cases)
        wants=[{'type':'string','hex':expected(bytes.fromhex(case['input']['hex']),case['api']=='sanitize_textarea_field')[0].hex()}for case in cases]
        self.assertEqual(observed['results'],wants)
        profile.validate_counts(observed['sanitize_text_counts'],profile.expected_counts(cases))

    def test_first_empty_preserves_null_cache_and_nonempty_bad_utf8_initializes_true(self):
        cases=[profile.fixture(b''),profile.fixture(b'',profile.APIS[1]),profile.fixture(b'\xff'),profile.fixture(b'0')]
        observed=self.php(r'''
$states=[(new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables()['is_utf8']];
foreach($cases as$case){oracle_sanitize_text(sanitize_text_input($case),$case);
$states[]=(new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables()['is_utf8'];}
echo json_encode(['states'=>$states,'counts'=>$sanitize_text_counts],JSON_THROW_ON_ERROR);
''',cases)
        self.assertEqual(observed['states'],[None,None,None,True,True])
        profile.validate_counts(observed['counts'],profile.expected_counts(cases))

    def test_cached_false_is_a_profile_refusal_even_for_empty_without_fake_reset(self):
        observed=self.php(r'''
$charset_fixture='ISO-8859-1';wp_check_invalid_utf8('standalone poison');unset($charset_fixture);
check((new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables()['is_utf8']===false);$caught=0;
foreach($cases as$case){try{oracle_sanitize_text(sanitize_text_input($case),$case);throw new LogicException('cached false accepted');}
catch(RuntimeException$error){check($error->getPrevious()===null);++$caught;}}
echo json_encode(['caught'=>$caught,'state'=>(new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables()['is_utf8'],
'counts'=>$sanitize_text_counts],JSON_THROW_ON_ERROR);
''',[profile.fixture(b''),profile.fixture(b'normal',profile.APIS[1])])
        self.assertEqual(observed,{'caught':2,'state':False,'counts':dict.fromkeys(profile.COUNT_KEYS,0)})

    def test_bad_protocol_and_lt_refuse_before_static_initialization_or_global_mutation(self):
        cases=[]
        for raw in(b'<PRIVATE>',b'\xff<',b'x'*4097):cases.append(profile.fixture(raw))
        for field,value in [('input',{'type':'bool','value':'PRIVATE_BOOL'}),('input',{'type':'string','hex':'PRIVATE_HEX'}),
                            ('empty_filters',1),('text_profile','PRIVATE_PROFILE'),('api','other')]:
            case=profile.fixture();case[field]=value;cases.append(case)
        for name in profile.LIMITS:
            for value in(False,1.0):
                case=profile.fixture();case['limits'][name]=value;cases.append(case)
        observed=self.php(r'''
$old=new stdClass();$wp_filter=$old;$caught=0;foreach($cases as$case){
try{oracle_sanitize_text(sanitize_text_input($case),$case);throw new LogicException('bad profile admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
check($wp_filter===$old);}
echo json_encode(['caught'=>$caught,'state'=>(new ReflectionFunction('wp_check_invalid_utf8'))->getStaticVariables()['is_utf8'],
'counts'=>$sanitize_text_counts],JSON_THROW_ON_ERROR);
''',cases)
        self.assertEqual(observed,{'caught':len(cases),'state':None,'counts':dict.fromkeys(profile.COUNT_KEYS,0)})

    def test_active_other_scopes_and_all_globals_restore_after_success_or_source_failure(self):
        body=r'''
$names=['wp_filter','wp_filters','wp_current_filter','wp_actions','wp_hasher','wpdb','wp_object_cache','auth_cookie_scope','title_scope'];$n=0;
foreach([3,0,1,2]as$state){foreach($cases as$case){$before=[];foreach($names as$name){
if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>[],default=>['active'=>true,'PRIVATE_STATE'=>new stdClass()]};}
$before[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
$caught=false;try{oracle_sanitize_text(sanitize_text_input($case),$case);}catch(RuntimeException$error){
check(EXPECT_FAILURE&&$error->getMessage()==='Text sanitizer reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));$caught=true;}
check($caught===EXPECT_FAILURE);foreach($before as$name=>[$had,$old]){check(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}++$n;}}
echo json_encode(['n'=>$n,'restored'=>$sanitize_text_counts['restored']],JSON_THROW_ON_ERROR);
'''
        for fail in(False,True):
            prefix='const EXPECT_FAILURE='+('true;'if fail else'false;')
            if fail:prefix+="function sanitize_text_field($raw){throw new RuntimeException('PRIVATE_SOURCE');}"
            self.assertEqual(self.php(prefix+body,[profile.fixture(b'PRIVATE_RAW')],actual=not fail),{'n':4,'restored':4})

    def test_full_glue_preserves_old_utf8_auth_title_and_absint_around_both_sanitizers(self):
        import auth_cookie_profile
        import title_profile
        string=lambda raw:{'type':'string','hex':raw.hex()}
        cases=[profile.fixture(b''),
               {'api':'is_utf8_charset','input':string(b'ISO-8859-1')},
               {'api':'wp_check_invalid_utf8','input':string(b'\xff'),'strip':False},
               profile.fixture(b'A\t%41\tB','sanitize_textarea_field'),
               auth_cookie_profile.fixture(b'u',b'h',b't',b's',expiration=0),
               title_profile.fixture(b'Wordpress'),{'api':'absint','input':{'type':'int','value':'-7'}},
               profile.fixture(b'A\t%41\tB'),
               {'api':'wp_check_invalid_utf8','input':string(b'valid\0UTF8'),'strip':False},
               profile.fixture(b'\xc3%41\xa9','sanitize_textarea_field')]
        for index,case in enumerate(cases):case['id']=f'sanitize-text-glue-{index}'
        command=[str(PHP),'-n','-d','memory_limit=128M',str(ROOT/'scripts/wp_oracle.php'),str(WP)]
        _,result=owned_probe.run_captured_probe(command,os.environ.copy(),timeout=20,
            input_bytes=json.dumps(cases,separators=(',',':')).encode())
        self.assertEqual(result.returncode,0,result.stdout or result.stderr);self.assertEqual(result.stderr,b'')
        observed=json.loads(result.stdout)
        key=hmac.new(b's',b'u|h|0|t',hashlib.md5).hexdigest().encode()
        cookie=b'u|0|t|'+hmac.new(key,b'u|0|t',hashlib.sha256).hexdigest().encode()
        self.assertEqual([entry['value']for entry in observed['results']],
                         [string(b''),False,string(b''),string(b'A\t\tB'),
                          string(cookie),string(b'WordPress'),{'type':'int','value':'7'},string(b'A B'),
                          string(b'valid\0UTF8'),string(b'')])
        profile.validate_profile(observed['sanitize_text_profile'])
        profile.validate_counts(observed['sanitize_text_counts'],profile.expected_counts(cases))
        auth_cookie_profile.validate_counts(observed['auth_cookie_counts'],auth_cookie_profile.expected_counts(cases))
        title_profile.validate_counts(observed['title_counts'],title_profile.expected_counts(cases))
        self.assertEqual(observed['json_utf8_fallback_profile'],{'blog_charset':'UTF-8','mb_convert_encoding_exists':False,'is_utf8_charset':True})
        for wire in ({'type':'bool','value':'PRIVATE_BOOL'},{'type':'string','hex':'PRIVATE_HEX'}):
            bad=profile.fixture(b'PRIVATE_RAW');bad.update(id='refused');bad['input']=wire
            _,failed=owned_probe.run_captured_probe(command,os.environ.copy(),timeout=20,
                input_bytes=json.dumps([bad],separators=(',',':')).encode())
            self.assertNotEqual(failed.returncode,0);self.assertNotIn(b'PRIVATE_',failed.stdout+failed.stderr)


if __name__=='__main__':unittest.main()
