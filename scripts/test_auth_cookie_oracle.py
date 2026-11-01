"""Pinned actual cookie contents and closed explicit-fixture admission."""
import copy
import hashlib
import hmac
import json
import os
from pathlib import Path
import unittest

import auth_cookie_profile as profile
import owned_probe

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / '.reference/php-8.5.12/php.exe'
WP = ROOT / '.reference/wordpress-7.1.3'
BOOT = r'''
$repo=$argv[1];$root=$repo.'/.reference/wordpress-7.1.3';
error_reporting(E_ALL);ini_set('zend.exception_ignore_args','0');
set_error_handler(static function($level,$message,$file,$line){throw new ErrorException($message,0,$level,$file,$line);});
require$repo.'/scripts/reference_functions.php';require$root.'/wp-includes/plugin.php';
require$repo.'/scripts/php_value_codec.php';require$repo.'/scripts/post_meta_oracle_profile.php';
require$repo.'/scripts/auth_cookie_oracle_profile.php';
function get_option(#[\SensitiveParameter]$name){throw new RuntimeException('Auth cookie option resolution forbidden');}
function check($condition){if(!$condition){throw new LogicException('Auth cookie contract failed');}}
$cases=json_decode(stream_get_contents(STDIN),true,128,JSON_THROW_ON_ERROR);
'''


def expected(case):
    # Independent test provider, never product crypto or session implementation.
    raw = lambda field: bytes.fromhex(case[field]['hex'])
    login, password, token, salt = [raw(field) for field in ('input','password_hash','token','resolved_salt')]
    fragment = password[8:12] if password.startswith((b'$P$', b'$2y$')) else password[-4:]
    expiration = str(int(case['expiration']['value'])).encode()
    key = hmac.new(salt, b'|'.join([login, fragment, expiration, token]), hashlib.md5).hexdigest().encode()
    message = b'|'.join([login, expiration, token])
    return message+b'|'+hmac.new(key, message, hashlib.sha256).hexdigest().encode()


class AuthCookieProfileTests(unittest.TestCase):
    def test_strict_nested_types_counts_and_previous_whole75_identity(self):
        profile.validate_profile(copy.deepcopy(profile.PROFILE))
        for path, value in [(('native_filter_bookkeeping',), 0), (('filter_arguments',), True),
                            (('fixture_limits','max_input_bytes'), 65536.0),
                            (('schemes',), ['auth'])]:
            changed=copy.deepcopy(profile.PROFILE); target=changed
            for key in path[:-1]: target=target[key]
            target[path[-1]]=value
            with self.assertRaises(ValueError): profile.validate_profile(changed)
        counts=profile.expected_counts([profile.fixture()])
        profile.validate_counts(counts,counts)
        for bad in (dict(counts,session_calls=1),dict(counts,cache_calls=True),dict(counts,hash_salt_calls=1.0)):
            with self.assertRaises(ValueError): profile.validate_counts(bad,counts)
        profile.validate_previous(profile.OLD_COUNT,profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256)
        for args in [(float(profile.OLD_COUNT),profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256),
                     (profile.OLD_COUNT,profile.OLD_INPUT_SHA256,'0'*64,profile.OLD_OUTPUT_SHA256)]:
            with self.assertRaises(ValueError): profile.validate_previous(*args)

    def test_finite_corpus_has_fragment_binary_scheme_and_expiration_boundaries(self):
        cases=profile.cases();self.assertEqual(cases,profile.cases())
        self.assertGreater(len(cases),2000)
        self.assertEqual({case['scheme']for case in cases},set(profile.SCHEMES))
        self.assertTrue({str(-(2**63)),str(2**63-1),'0','-1'}<={case['expiration']['value']for case in cases})
        values={bytes.fromhex(case['password_hash']['hex'])for case in cases}
        self.assertTrue({b'',b'$P$',b'$2y$',b'$wp$2y$12$publicnotcanonical'}<=values)
        self.assertTrue(any(len(expected(case))==65536 for case in cases))

    def test_registry_recomputes_coverage_instead_of_trusting_paired_forged_counters(self):
        import compatibility_registry as registry
        cases=profile.cases();counts=profile.expected_counts(cases)
        arguments=[profile.PROFILE,counts,counts,len(cases),profile.OLD_COUNT,
                   profile.OLD_INPUT_SHA256,profile.OLD_OUTPUT_SHA256,profile.OLD_OUTPUT_SHA256]
        registry.validate_auth_cookie_coverage(*arguments)
        forged=dict(counts,hash_salt_calls=0)
        bad=list(arguments);bad[1]=forged;bad[2]=forged
        with self.assertRaises(ValueError):registry.validate_auth_cookie_coverage(*bad)
        bad=list(arguments);bad[3]=float(len(cases))
        with self.assertRaises(ValueError):registry.validate_auth_cookie_coverage(*bad)
        bad=list(arguments);bad[7]='0'*64
        with self.assertRaises(ValueError):registry.validate_auth_cookie_coverage(*bad)


@unittest.skipUnless(PHP.is_file() and (WP/'wp-includes/pluggable.php').is_file(),'pinned PHP/WordPress unavailable')
class AuthCookieOracleTests(unittest.TestCase):
    def setUp(self): self.addCleanup(owned_probe.reap_owned_probes)

    def php(self,body,cases,*,actual=True):
        loader='auth_cookie_load_reference_functions($root);\n' if actual else "load_reference_function($root.'/wp-includes/pluggable.php','wp_hash');\n"
        pid,result=owned_probe.run_captured_probe([str(PHP),'-n','-d','memory_limit=128M','-r',BOOT+loader+body,str(ROOT)],
            os.environ.copy(),timeout=20,input_bytes=json.dumps(cases,ensure_ascii=True,separators=(',',':')).encode())
        self.assertGreater(pid,0);self.assertEqual(result.returncode,0,result.stdout or result.stderr)
        self.assertEqual(result.stderr,b'');self.assertLessEqual(len(result.stdout),1024*1024)
        return json.loads(result.stdout)

    def test_actual_named_cookie_and_hash_use_exact_raw_fragment_hex_key_and_signed_decimal(self):
        cases=[profile.fixture(),profile.fixture(password_hash=b'$P$0123456789abcdef'),
               profile.fixture(password_hash=b'$2y$0123456789abcdef'),
               profile.fixture(password_hash=b'$wp$2y$12$publicnotcanonical'),
               profile.fixture(password_hash=b'$P$'),profile.fixture(password_hash=b'abc'),
               profile.fixture(b'\0\xff|user',b'\xff\0hash',b'\0\xff|token',b'\0salt\xff',expiration=-(2**63)),
               profile.fixture(b'',b'',b'00',b'0',expiration=2**63-1,scheme='secure_auth'),
               profile.fixture(token=b' ',salt=b'x'*65,expiration=-1,scheme='logged_in')]
        result=self.php(r'''
$results=[];foreach($cases as$case){$results[]=encode_value(oracle_auth_cookie(auth_cookie_input($case),$case));}
echo json_encode(['results'=>$results,...auth_cookie_metadata()],JSON_THROW_ON_ERROR);
''',cases)
        self.assertEqual(result['results'],[profile.string(expected(case))for case in cases])
        profile.validate_profile(result['auth_cookie_profile'])
        profile.validate_counts(result['auth_cookie_counts'],profile.expected_counts(cases))

    def test_success_and_source_failure_restore_absent_null_array_and_object_global_values(self):
        body=r'''
$names=['wp_filter','wp_filters','wp_current_filter','auth_cookie_scope','wpdb','wp_actions','wp_hasher',
'wp_object_cache','wp_meta_keys','post_meta_cached_scope','title_scope'];$n=0;
foreach([0,1,2,3]as$state){foreach($cases as$case){$before=[];foreach($names as$name){
if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>[],default=>['PRIVATE_STATE'=>new stdClass()]};}
$before[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
$caught=false;try{oracle_auth_cookie(auth_cookie_input($case),$case);}catch(RuntimeException$error){
check(EXPECT_FAILURE&&$error->getMessage()==='Auth cookie reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));$caught=true;}
check($caught===EXPECT_FAILURE);foreach($before as$name=>[$had,$old]){check(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}++$n;
}}
echo json_encode(['n'=>$n,'counts'=>$auth_cookie_counts],JSON_THROW_ON_ERROR);
'''
        cases=[profile.fixture(b'PRIVATE_LOGIN',b'PRIVATE_HASH',b'PRIVATE_TOKEN',b'PRIVATE_SALT')]
        for fail in (False,True):
            prefix='const EXPECT_FAILURE='+('true;'if fail else'false;')
            if fail: prefix+=r'''
function wp_generate_auth_cookie($id,$expiration,$scheme,$token){get_userdata($id);wp_hash('dummy',$scheme);
apply_filters('auth_cookie','dummy',$id,$expiration,$scheme,$token);throw new RuntimeException('PRIVATE_SOURCE');}
'''
            result=self.php(prefix+body,cases,actual=not fail)
            self.assertEqual(result['n'],4)
            profile.validate_counts(result['counts'],profile.expected_counts(cases*4))

    def test_bad_types_truthiness_salts_profiles_and_caps_refuse_before_any_scope_mutation(self):
        cases=[]
        def changed(field,value):
            case=profile.fixture();case[field]=value;cases.append(case)
        for field in ('input','password_hash','token','resolved_salt'):
            for wire in ({'type':'null'},{'type':'bool','value':'PRIVATE_BOOL'},profile.integer(1),
                         {'type':'string','hex':'PRIVATE_BAD_HEX'}):changed(field,wire)
        for token in (b'',b'0'):changed('token',profile.string(token))
        changed('resolved_salt',profile.string(b''))
        for owner in (0,-1):changed('user_id',profile.integer(owner))
        for field in ('user_id','expiration'):
            for wire in (profile.string(b'PRIVATE_INT'),{'type':'int','value':'9223372036854775808'},
                         {'type':'int','value':True}):changed(field,wire)
        for field,value in [('scheme','PRIVATE_SCHEME'),('empty_filters',1),('auth_cookie_profile','PRIVATE_PROFILE')]:changed(field,value)
        for key in profile.LIMITS:
            for value in (False,65535,65536.0):
                case=profile.fixture();case['limits'][key]=value;cases.append(case)
        case=profile.fixture();case['limits']['extra']=0;cases.append(case)
        cases.extend([profile.fixture(b'x'*65537),profile.fixture(b'x'*65530)])
        result=self.php(r'''
$names=['wp_filter','wp_filters','wp_current_filter','auth_cookie_scope','wpdb'];$before=[];
foreach($names as$name){$GLOBALS[$name]=new stdClass();$before[$name]=$GLOBALS[$name];}$caught=0;
foreach($cases as$case){try{oracle_auth_cookie(auth_cookie_input($case),$case);throw new LogicException('bad fixture admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
foreach($before as$name=>$old){check($GLOBALS[$name]===$old);}}
echo json_encode(['caught'=>$caught,'counts'=>$auth_cookie_counts],JSON_THROW_ON_ERROR);
''',cases)
        self.assertEqual(result,{'caught':len(cases),'counts':dict.fromkeys(profile.COUNT_KEYS,0)})

    def test_real_cache_database_session_and_option_tripwires_refuse_and_restore(self):
        result=self.php(r'''
function wp_generate_auth_cookie($id,$expiration,$scheme,$token){get_userdata($id);
switch($id){case 1:return wp_cache_get('PRIVATE_CACHE','post_meta');case 2:return update_meta_cache('post',[1]);
case 3:return$GLOBALS['wpdb']->PRIVATE_PROPERTY;case 4:return$GLOBALS['wpdb']->query('PRIVATE_SQL');
case 5:return WP_Session_Tokens::get_instance($id);default:return get_option('PRIVATE_OPTION');}}
$old=new stdClass();$GLOBALS['wpdb']=$old;$caught=0;foreach($cases as$case){
try{oracle_auth_cookie(auth_cookie_input($case),$case);throw new LogicException('tripwire admitted');}
catch(RuntimeException$error){check($error->getMessage()==='Auth cookie reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
check($GLOBALS['wpdb']===$old);}
echo json_encode(['caught'=>$caught,'counts'=>$auth_cookie_counts],JSON_THROW_ON_ERROR);
''',[profile.fixture(user_id=value)for value in range(1,7)],actual=False)
        self.assertEqual(result,{'caught':6,'counts':{'calls':6,'user_lookup_calls':6,'hash_salt_calls':0,
            'auth_cookie_hook':0,'session_calls':1,'db_calls':2,'cache_calls':2,'restored':6}})

    def test_empty_hook_profile_rejects_wrong_hash_hook_counts_or_mutated_record(self):
        result=self.php(r'''
function wp_generate_auth_cookie($id,$expiration,$scheme,$token){$user=get_userdata($id);
if($id===1){return'no hash or hook';}if($id===2){wp_hash('one',$scheme);wp_hash('two',$scheme);}
else{wp_hash('one',$scheme);$user->user_pass='changed';}
return apply_filters('auth_cookie','dummy',$id,$expiration,$scheme,$token);}
$caught=0;foreach($cases as$case){try{oracle_auth_cookie(auth_cookie_input($case),$case);throw new LogicException('wrong counts admitted');}
catch(RuntimeException$error){++$caught;}}echo json_encode(['caught'=>$caught,'restored'=>$auth_cookie_counts['restored']],JSON_THROW_ON_ERROR);
''',[profile.fixture(user_id=value)for value in range(1,4)],actual=False)
        self.assertEqual(result,{'caught':3,'restored':3})

    def test_fixture_functions_are_closed_outside_active_scope(self):
        result=self.php(r'''
$caught=0;foreach([null,[],new stdClass()]as$value){$auth_cookie_scope=$value;
foreach([static fn()=>get_userdata('PRIVATE_USER'),static fn()=>wp_salt('PRIVATE_SALT'),
static fn()=>WP_Session_Tokens::get_instance('PRIVATE_SESSION')]as$call){
try{$call();throw new LogicException('unscoped fixture admitted');}catch(RuntimeException$error){check(!str_contains((string)$error,'PRIVATE_'));++$caught;}}}
echo json_encode(['caught'=>$caught,'counts'=>$auth_cookie_counts],JSON_THROW_ON_ERROR);
''',[])
        self.assertEqual(result,{'caught':9,'counts':dict.fromkeys(profile.COUNT_KEYS,0)})

    def test_full_oracle_glue_mixes_old_title_and_absint_without_cookie_scope_leak(self):
        import title_profile
        cases=[profile.fixture(),{'api':'absint','input':profile.integer(-7)},
               title_profile.fixture(b'Wordpress'),profile.fixture(token=b'00',scheme='logged_in')]
        for index,case in enumerate(cases):case['id']=f'auth-cookie-glue-{index}'
        _,result=owned_probe.run_captured_probe([str(PHP),'-n','-d','memory_limit=128M',str(ROOT/'scripts/wp_oracle.php'),str(WP)],
            os.environ.copy(),timeout=20,input_bytes=json.dumps(cases,separators=(',',':')).encode())
        self.assertEqual(result.returncode,0,result.stdout or result.stderr);self.assertEqual(result.stderr,b'')
        value=json.loads(result.stdout)
        self.assertEqual([item['value']for item in value['results']],
                         [profile.string(expected(cases[0])),profile.integer(7),profile.string(b'WordPress'),profile.string(expected(cases[3]))])
        profile.validate_counts(value['auth_cookie_counts'],profile.expected_counts(cases))
        title_profile.validate_counts(value['title_counts'],title_profile.expected_counts(cases))
        bad=profile.fixture(token=b'PRIVATE_TOKEN',salt=b'PRIVATE_SALT')
        bad.update(id='refusal',user_id={'type':'int','value':'PRIVATE_INVALID_OWNER'})
        _,failed=owned_probe.run_captured_probe([str(PHP),'-n','-d','memory_limit=128M',str(ROOT/'scripts/wp_oracle.php'),str(WP)],
            os.environ.copy(),timeout=20,input_bytes=json.dumps([bad],separators=(',',':')).encode())
        self.assertNotEqual(failed.returncode,0)
        self.assertNotIn(b'PRIVATE_',failed.stdout+failed.stderr)


if __name__=='__main__':unittest.main()
