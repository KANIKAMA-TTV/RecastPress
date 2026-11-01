"""Pinned parser-source contracts, refusal, privacy and scope restoration."""
import copy
import hashlib
import hmac
import json
import os
from pathlib import Path
import sys
import unittest

HERE=Path(__file__).resolve().parent
ROOT=HERE.parent
sys.path.insert(0,str(ROOT/'scripts'))
import owned_probe

import auth_cookie_parse_profile as profile
PHP=ROOT/'.reference/php-8.5.12/php.exe'
WP=ROOT/'.reference/wordpress-7.1.3'
BOOT=r'''
$repo=$argv[1];$root=$repo.'/.reference/wordpress-7.1.3';$fixture=$argv[2];
error_reporting(E_ALL);ini_set('zend.exception_ignore_args','0');
set_error_handler(static function($level,$message,$file,$line){throw new ErrorException($message,0,$level,$file,$line);});
require$repo.'/scripts/reference_functions.php';require$repo.'/scripts/php_value_codec.php';
require$root.'/wp-includes/plugin.php';require$fixture.'/auth_cookie_parse_oracle_profile.php';
function check($condition){if(!$condition){throw new LogicException('Explicit parser contract failed');}}
$cases=json_decode(stream_get_contents(STDIN),true,128,JSON_THROW_ON_ERROR);
'''


def typed_result(cookie,scheme):
    values=cookie.split(b'|')
    if len(values)!=4:return{'type':'bool','value':False}
    return{'type':'array','entries':[{'key':profile.string(key.encode()),'value':profile.string(value)}
                                   for key,value in zip(profile.FIELDS,[*values,scheme])]}


class AuthCookieParseProfileTests(unittest.TestCase):
    def test_profile_types_and_completed_old76_identity_are_strict(self):
        profile.validate_profile(copy.deepcopy(profile.PROFILE))
        changed=copy.deepcopy(profile.PROFILE);changed['session_or_hmac_verification']=0
        with self.assertRaises(ValueError):profile.validate_profile(changed)
        changed=copy.deepcopy(profile.PROFILE);changed['fixture_limits']['max_input_bytes']=65536.0
        with self.assertRaises(ValueError):profile.validate_profile(changed)
        cases=profile.cases();self.assertEqual(cases,profile.cases())
        self.assertGreater(len(cases),2000)
        counts=profile.expected_counts(cases);profile.validate_counts(counts,counts)
        with self.assertRaises(ValueError):profile.validate_counts(dict(counts,ssl_calls=False),counts)
        profile.validate_previous(profile.OLD_COUNT,profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256)
        with self.assertRaises(ValueError):profile.validate_previous(float(profile.OLD_COUNT),profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256)
        with self.assertRaises(ValueError):profile.validate_previous(profile.OLD_COUNT,profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,'0'*64)

    def test_registry_rejects_paired_forged_counts_and_old76_hash_changes(self):
        import compatibility_registry as registry
        cases=profile.cases();counts=profile.expected_counts(cases)
        args=[profile.PROFILE,counts,counts,len(cases),profile.OLD_COUNT,
              profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256]
        registry.validate_auth_cookie_parse_coverage(*args)
        forged=dict(counts,parsed=0,false_results=len(cases))
        bad=list(args);bad[1]=forged;bad[2]=forged
        with self.assertRaises(ValueError):registry.validate_auth_cookie_parse_coverage(*bad)
        bad=list(args);bad[3]=float(len(cases))
        with self.assertRaises(ValueError):registry.validate_auth_cookie_parse_coverage(*bad)
        bad=list(args);bad[7]='0'*64
        with self.assertRaises(ValueError):registry.validate_auth_cookie_parse_coverage(*bad)


@unittest.skipUnless(PHP.is_file()and(WP/'wp-includes/pluggable.php').is_file(),'pinned PHP/WordPress unavailable')
class AuthCookieParseOracleTests(unittest.TestCase):
    def setUp(self):self.addCleanup(owned_probe.reap_owned_probes)

    def php(self,body,cases,*,actual=True):
        loader='auth_cookie_parse_load_reference_functions($root);\n'if actual else''
        pid,result=owned_probe.run_captured_probe([str(PHP),'-n','-d','memory_limit=128M','-r',BOOT+loader+body,str(ROOT),str(HERE)],
            os.environ.copy(),timeout=20,input_bytes=json.dumps(cases,ensure_ascii=True,separators=(',',':')).encode())
        self.assertGreater(pid,0);self.assertEqual(result.returncode,0,result.stdout or result.stderr)
        self.assertEqual(result.stderr,b'');self.assertLessEqual(len(result.stdout),8*1024*1024)
        return json.loads(result.stdout)

    def test_actual_source_preserves_every_raw_field_and_arbitrary_scheme(self):
        cases=[profile.fixture(b'|||',b''),profile.fixture(b'\0\xff||0|bad-hmac',b'\xff\0|unknown'),
               profile.fixture(b'u|not-an-integer||',b'unknown'),profile.fixture(b'abc',b'auth'),
               profile.fixture(b'a|b|c|d|e',b'logged_in')]
        result=self.php(r'''
$out=[];foreach($cases as$case){$out[]=encode_value(oracle_auth_cookie_parse(auth_cookie_parse_input($case),$case));}
echo json_encode(['results'=>$out,...auth_cookie_parse_metadata()],JSON_THROW_ON_ERROR);
''',cases)
        expected=[typed_result(bytes.fromhex(case['input']['hex']),bytes.fromhex(case['scheme']['hex']))for case in cases]
        self.assertEqual(result['results'],expected)
        profile.validate_profile(result['auth_cookie_parse_profile'])
        profile.validate_counts(result['auth_cookie_parse_counts'],profile.expected_counts(cases))

    def test_finite_corpus_matches_actual_parser_without_any_ambient_cookie_or_ssl(self):
        cases=profile.cases()
        result=self.php(r'''
$out=[];foreach($cases as$case){$out[]=encode_value(oracle_auth_cookie_parse(auth_cookie_parse_input($case),$case));}
echo json_encode(['results'=>$out,...auth_cookie_parse_metadata()],JSON_THROW_ON_ERROR);
''',cases)
        self.assertEqual(result['results'],[typed_result(bytes.fromhex(case['input']['hex']),bytes.fromhex(case['scheme']['hex']))for case in cases])
        profile.validate_counts(result['auth_cookie_parse_counts'],profile.expected_counts(cases))

    def test_preflight_empty_zero_types_and_combined_cap_refuse_before_globals_or_calls(self):
        cases=[]
        for raw in (b'',b'0',b'x'*65537):cases.append(profile.fixture(raw))
        cases.append(profile.fixture(b'x',b's'*65536))
        for field in ('input','scheme'):
            for wire in ({'type':'null'},{'type':'bool','value':'PRIVATE_BOOL'},
                         {'type':'string','hex':'PRIVATE_HEX'},{'type':'int','value':'1'}):
                case=profile.fixture();case[field]=wire;cases.append(case)
        for field,value in [('explicit_cookie',1),('auth_cookie_parse_profile','PRIVATE_PROFILE')]:
            case=profile.fixture();case[field]=value;cases.append(case)
        for limit in (False,65535,65536.0):
            case=profile.fixture();case['limits']['max_input_bytes']=limit;cases.append(case)
        result=self.php(r'''
$before=[];foreach(['_COOKIE','_SERVER','auth_cookie_parse_scope']as$name){$GLOBALS[$name]=new stdClass();$before[$name]=$GLOBALS[$name];}
$caught=0;foreach($cases as$case){try{oracle_auth_cookie_parse(auth_cookie_parse_input($case),$case);throw new LogicException('bad parser fixture admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
foreach($before as$name=>$old){check($GLOBALS[$name]===$old);}}
echo json_encode(['caught'=>$caught,'counts'=>$auth_cookie_parse_counts],JSON_THROW_ON_ERROR);
''',cases)
        self.assertEqual(result,{'caught':len(cases),'counts':dict.fromkeys(profile.COUNT_KEYS,0)})

    def test_success_and_source_failure_restore_superglobals_and_hooks_in_all_presence_states(self):
        body=r'''
$names=['_COOKIE','_SERVER','auth_cookie_parse_scope','wp_filter','wp_filters','wp_current_filter','wp_actions','wpdb','wp_object_cache'];$n=0;
foreach([0,1,2,3]as$state){foreach($cases as$case){$before=[];foreach($names as$name){
if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>[],default=>['PRIVATE_STATE'=>new stdClass()]};}
$before[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
$caught=false;try{oracle_auth_cookie_parse(auth_cookie_parse_input($case),$case);}catch(RuntimeException$error){
check(EXPECT_FAILURE&&$error->getMessage()==='Explicit cookie parser reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));$caught=true;}
check($caught===EXPECT_FAILURE);foreach($before as$name=>[$had,$old]){check(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}++$n;}}
echo json_encode(['n'=>$n,'restored'=>$auth_cookie_parse_counts['restored']],JSON_THROW_ON_ERROR);
'''
        for fail in (False,True):
            prefix='const EXPECT_FAILURE='+('true;'if fail else'false;')
            if fail:prefix+="function wp_parse_auth_cookie($cookie,$scheme){$GLOBALS['_SERVER']='changed';throw new RuntimeException('PRIVATE_SOURCE');}"
            self.assertEqual(self.php(prefix+body,[profile.fixture(b'PRIVATE_USER|raw|PRIVATE_TOKEN|PRIVATE_HMAC',b'PRIVATE_SCHEME')],actual=not fail),
                             {'n':4,'restored':4})

    def test_environment_attempts_are_measured_refusals_and_wrong_false_is_not_success(self):
        result=self.php(r'''
function wp_parse_auth_cookie($cookie,$scheme){if($scheme==='ssl'){return is_ssl();}
if($scheme==='cookie'){return$_COOKIE['PRIVATE_COOKIE'];}return false;}
$caught=0;foreach($cases as$case){try{oracle_auth_cookie_parse(auth_cookie_parse_input($case),$case);throw new LogicException('wrong parser branch admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}}
echo json_encode(['caught'=>$caught,'counts'=>$auth_cookie_parse_counts],JSON_THROW_ON_ERROR);
''',[profile.fixture(b'a|b|c|d',scheme)for scheme in (b'ssl',b'cookie',b'wrong-false')],actual=False)
        self.assertEqual(result,{'caught':3,'counts':{'calls':3,'parsed':0,'false_results':0,
                                                    'ssl_calls':1,'cookie_accesses':1,'restored':3}})

    def test_actual_generation_relation_requires_delimiter_free_login_and_token_only(self):
        # Separate fixed record/salt fixtures for this relation. It never loads
        # or executes the main development cookie-generation oracle module.
        result=self.php(r'''
load_reference_function($root.'/wp-includes/pluggable.php','wp_generate_auth_cookie');
load_reference_function($root.'/wp-includes/pluggable.php','wp_hash');
function get_userdata($id){return$GLOBALS['relation_user'];}
function wp_salt($scheme){return'public fixed relation salt';}
$out=[];foreach($cases as$case){$login=hex2bin($case['login']);$token=hex2bin($case['token']);
$GLOBALS['relation_user']=(object)['ID'=>1,'user_login'=>$login,'user_pass'=>'$wp$2y$public-dummy'];
$cookie=wp_generate_auth_cookie(1,-1,'auth',$token);
$parse_case=['input'=>['type'=>'string','hex'=>bin2hex($cookie)],'scheme'=>['type'=>'string','hex'=>bin2hex('unknown')],
'explicit_cookie'=>true,'auth_cookie_parse_profile'=>'explicit_php_truthy_cookie_arbitrary_byte_scheme','limits'=>['max_input_bytes'=>65536]];
$value=oracle_auth_cookie_parse($cookie,$parse_case);$expected=!str_contains($login,'|')&&!str_contains($token,'|');
check(($value!==false)===$expected);
if($expected){check($value['username']===$login&&$value['expiration']==='-1'&&$value['token']===$token
&&$value['hmac']===substr($cookie,-64)&&$value['scheme']==='unknown');}
$out[]=is_array($value);}
echo json_encode(['parsed'=>$out,'counts'=>$auth_cookie_parse_counts],JSON_THROW_ON_ERROR);
''',[{'login':login.hex(),'token':token.hex()}for login,token in [(b'public',b't'),(b'',b'00'),(b'a|b',b't'),(b'a',b't|x')]])
        self.assertEqual(result,{'parsed':[True,True,False,False],
                                'counts':{'calls':4,'parsed':2,'false_results':2,'ssl_calls':0,'cookie_accesses':0,'restored':4}})

    def test_full_glue_mixes_generation_title_absint_and_parser_with_opaque_bad_input(self):
        import auth_cookie_profile
        import title_profile
        message=b'u|0|t'
        key=hmac.new(b's',b'u|h|0|t',hashlib.md5).hexdigest().encode()
        generated=message+b'|'+hmac.new(key,message,hashlib.sha256).hexdigest().encode()
        cases=[profile.fixture(b'a|raw-expiry||bad-hmac',b''),
               auth_cookie_profile.fixture(b'u',b'h',b't',b's',expiration=0),
               title_profile.fixture(b'Wordpress'),{'api':'absint','input':auth_cookie_profile.integer(-7)},
               profile.fixture(generated,b'unrelated'),profile.fixture(b'no-pipes',b'\xff\0')]
        for index,case in enumerate(cases):case['id']=f'parser-glue-{index}'
        command=[str(PHP),'-n','-d','memory_limit=128M',str(ROOT/'scripts/wp_oracle.php'),str(WP)]
        _,result=owned_probe.run_captured_probe(command,os.environ.copy(),timeout=20,
            input_bytes=json.dumps(cases,separators=(',',':')).encode())
        self.assertEqual(result.returncode,0,result.stdout or result.stderr);self.assertEqual(result.stderr,b'')
        observed=json.loads(result.stdout)
        self.assertEqual([item['value']for item in observed['results']],
                         [typed_result(b'a|raw-expiry||bad-hmac',b''),profile.string(generated),
                          profile.string(b'WordPress'),auth_cookie_profile.integer(7),
                          typed_result(generated,b'unrelated'),{'type':'bool','value':False}])
        profile.validate_profile(observed['auth_cookie_parse_profile'])
        profile.validate_counts(observed['auth_cookie_parse_counts'],profile.expected_counts(cases))
        auth_cookie_profile.validate_counts(observed['auth_cookie_counts'],auth_cookie_profile.expected_counts(cases))
        title_profile.validate_counts(observed['title_counts'],title_profile.expected_counts(cases))
        for field,wire in [('input',{'type':'string','hex':'PRIVATE_BAD_HEX'}),
                           ('scheme',{'type':'int','value':'PRIVATE_BAD_SCHEME'})]:
            bad=profile.fixture(b'PRIVATE_USER|raw|PRIVATE_TOKEN|PRIVATE_HMAC',b'PRIVATE_SCHEME')
            bad.update(id='private-refusal');bad[field]=wire
            _,failed=owned_probe.run_captured_probe(command,os.environ.copy(),timeout=20,
                input_bytes=json.dumps([bad],separators=(',',':')).encode())
            self.assertNotEqual(failed.returncode,0)
            self.assertNotIn(b'PRIVATE_',failed.stdout+failed.stderr)


if __name__=='__main__':unittest.main()
