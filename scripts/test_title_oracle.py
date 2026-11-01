"""Owned PHP contracts for the actual prepared-post title/filter chain."""
import json
import os
from pathlib import Path
import tempfile
import unittest

import owned_probe
import title_profile

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / '.reference/php-8.5.12/php.exe'
WP = ROOT / '.reference/wordpress-7.1.3'
BOOT = r'''
$repo=$argv[1];$root=$repo.'/.reference/wordpress-7.1.3';
error_reporting(E_ALL);ini_set('zend.exception_ignore_args','0');setlocale(LC_CTYPE,'C');
set_error_handler(static function($level,$message,$file,$line){throw new ErrorException($message,0,$level,$file,$line);});
require$repo.'/scripts/reference_functions.php';require$root.'/wp-includes/plugin.php';
require$repo.'/scripts/php_value_codec.php';require$repo.'/scripts/post_meta_oracle_profile.php';
require$repo.'/scripts/texturize_oracle_profile.php';texturize_load_reference_functions($root);
require$repo.'/scripts/title_oracle_profile.php';
function get_option(#[\SensitiveParameter]$name){if(($GLOBALS['title_scope']['active']??false)===true){++$GLOBALS['title_scope']['external_calls'];}throw new RuntimeException('Prepared title option access forbidden');}
function check($condition){if(!$condition){throw new LogicException('Title contract failed');}}
$cases=json_decode(stream_get_contents(STDIN),true,128,JSON_THROW_ON_ERROR);
'''
SUPPORT = r'''
foreach(['OBJECT'=>'OBJECT','ARRAY_A'=>'ARRAY_A','ARRAY_N'=>'ARRAY_N']as$name=>$value){define($name,$value);}
require$root.'/wp-includes/class-wp-post.php';
load_reference_function($root.'/wp-includes/post.php','get_post');
load_reference_function($root.'/wp-includes/load.php','is_admin');
foreach(['convert_chars','capital_P_dangit']as$name){load_reference_function($root.'/wp-includes/formatting.php',$name);}
'''


@unittest.skipUnless(PHP.is_file() and (WP / 'wp-includes/post-template.php').is_file(), 'pinned PHP/WordPress unavailable')
class TitleOracleTests(unittest.TestCase):
    def setUp(self):
        self.addCleanup(owned_probe.reap_owned_probes)

    def php(self, body, cases, *, actual=True):
        code = BOOT + ('title_load_reference_functions($root);\n' if actual else SUPPORT) + body
        pid, result = owned_probe.run_captured_probe([str(PHP), '-n', '-d', 'memory_limit=128M', '-r', code, str(ROOT)],
            os.environ.copy(), timeout=20, input_bytes=json.dumps(cases, ensure_ascii=True, separators=(',', ':')).encode())
        self.assertGreater(pid, 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        self.assertLessEqual(len(result.stdout), 65536)
        return json.loads(result.stdout)

    def test_real_constructor_raw_get_post_frontend_and_four_named_default_callbacks(self):
        samples = [b'  Wordpress  ', b'"Wordpress"', b'A&B', b'&a;', b'&amp;',
                   b'\0 \tWordpress\r\n\v\0', '\u00a0Wordpress\u00a0'.encode(), b'', b'0']
        cases = [title_profile.fixture(raw, post_id=713, password=b'0') for raw in samples]
        result = self.php(r'''
$results=[];foreach($cases as$case){$results[]=bin2hex(oracle_title(title_input($case),$case));}
echo json_encode(['results'=>$results,...title_metadata(),'texturize_counts'=>$texturize_counts],JSON_THROW_ON_ERROR);
''', cases)
        expected = [b'WordPress', b'&#8220;WordPress&#8221;', b'A&#038;B', b'&#038;a;', b'&amp;',
                    b'WordPress', '\u00a0WordPress\u00a0'.encode(), b'', b'0']
        self.assertEqual(result['results'], [raw.hex() for raw in expected])
        title_profile.validate_profile(result['title_profile'])
        title_profile.validate_counts(result['title_counts'], title_profile.expected_counts(cases))
        self.assertTrue(all(value == 0 for value in result['texturize_counts'].values()))

    def test_priming_is_separate_from_existing_texturize_and_does_not_reset_in_getter(self):
        cases = [title_profile.fixture(b'XWordpress'), title_profile.fixture(b''), title_profile.fixture(b'0')]
        result = self.php(r'''
$t=['texturize_profile'=>'valid_utf8_no_lt_default_english_empty_filters','reset'=>true,
'limits'=>['max_input_bytes'=>65536,'max_work_bytes'=>1048576,'max_output_bytes'=>65536]];
oracle_texturize('prime-existing',$t);$before=$texturize_counts;
foreach($cases as$case){oracle_title(title_input($case),$case);}
check($before===$texturize_counts);
echo json_encode(['title'=>$title_counts,'texturize'=>$texturize_counts],JSON_THROW_ON_ERROR);
''', cases)
        expected = title_profile.expected_counts([{'api': 'wptexturize', 'input': {'hex': b'prime-existing'.hex()}}, *cases])
        self.assertEqual(result['title'], expected)
        self.assertEqual(result['title']['getter_x_calls'], 0)
        self.assertEqual(result['texturize']['calls'], 1)

    def test_protocol_refusals_precede_global_changes_and_never_leak_inputs(self):
        cases = []
        for raw in (b'<PRIVATE_RAW>', b'\xffPRIVATE_UTF8', b'x'*65537):
            cases.append(title_profile.fixture(raw))
        for field, value in [('input', {'type': 'bool', 'value': False}), ('post_id', {'type': 'int', 'value': '0'}),
                             ('post_id', {'type': 'int', 'value': '-1'}), ('post_id', {'type': 'int', 'value': '9223372036854775808'}),
                             ('post_id', {'type': 'int', 'value': 1}), ('post_password', {'type': 'string', 'hex': b'PRIVATE_PASSWORD'.hex()}),
                             ('post_status', {'type': 'string', 'hex': b'private'.hex()}), ('post_filter', 'display'),
                             ('frontend', 1), ('title_profile', 'PRIVATE_PROFILE'), ('limits', None)]:
            case = title_profile.fixture(b'PRIVATE_RAW'); case[field] = value; cases.append(case)
        for field in title_profile.LIMITS:
            for value in (True, -1, float(title_profile.LIMITS[field]), title_profile.LIMITS[field]-1):
                case = title_profile.fixture(b'PRIVATE_RAW'); case['limits'][field] = value; cases.append(case)
        case = title_profile.fixture(b'PRIVATE_RAW'); case['limits']['extra'] = 1; cases.append(case)
        result = self.php(r'''
$names=['wp_filter','wp_filters','wp_current_filter','current_screen','title_scope','texturize_scope'];
$before=[];foreach($names as$name){$GLOBALS[$name]=new stdClass();$before[$name]=$GLOBALS[$name];}
$n=0;foreach($cases as$case){try{oracle_title(title_input($case),$case);throw new LogicException('invalid accepted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$n;}}
foreach($before as$name=>$old){check($GLOBALS[$name]===$old);}
echo json_encode(['n'=>$n,'calls'=>$title_counts['calls'],'restored'=>$title_counts['restored']],JSON_THROW_ON_ERROR);
''', cases)
        self.assertEqual(result, {'n': len(cases), 'calls': 0, 'restored': 0})

    def test_global_presence_null_array_and_sentinel_restore_on_success_and_source_failure(self):
        body = r'''
$names=['wp_filter','wp_filters','wp_current_filter','wp_actions','post','current_screen','shortcode_tags',
'wp_cockneyreplace','texturize_scope','title_scope','post_meta_cached_scope','wpdb','wp_hasher','wp_object_cache'];$n=0;
foreach([0,1,2,3]as$state){foreach($cases as$case){$before=[];
 foreach($names as$name){if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>[],default=>['PRIVATE_STATE'=>new stdClass()]};}
 $before[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
 $caught=false;try{oracle_title(title_input($case),$case);}catch(RuntimeException$error){
 check(EXPECT_FAILURE&&$error->getMessage()==='Prepared title reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));$caught=true;}
 check($caught===EXPECT_FAILURE);foreach($before as$name=>[$had,$old]){check(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}++$n;
}}
echo json_encode(['n'=>$n,'restored'=>$title_counts['restored']],JSON_THROW_ON_ERROR);
'''
        for fail in (False, True):
            prefix = 'const EXPECT_FAILURE=' + ('true;' if fail else 'false;')
            if fail:
                prefix += "function get_the_title(#[\\SensitiveParameter]$post=0){throw new RuntimeException('PRIVATE_SOURCE');}"
            self.assertEqual(self.php(prefix + body, [title_profile.fixture(b'PRIVATE_RAW'), title_profile.fixture(b'')], actual=not fail), {'n': 8, 'restored': 8})

    def test_existing_cache_and_new_database_external_tripwires_detect_real_attempts(self):
        operations = [
            ("wp_cache_get(1,'posts')", "$GLOBALS['title_scope']['cache_calls']"),
            ("update_meta_cache('post',[1])", "$GLOBALS['title_scope']['cache_calls']"),
            ("$GLOBALS['wpdb']->posts", "$GLOBALS['wpdb']->calls()"),
            ("$GLOBALS['wpdb']->query('PRIVATE_SQL')", "$GLOBALS['wpdb']->calls()"),
            ("get_option('PRIVATE_OPTION')", "$GLOBALS['title_scope']['external_calls']"),
            ("sanitize_post($post,'raw')", "$GLOBALS['title_scope']['external_calls']"),
            ("__('PRIVATE_TRANSLATION')", "$GLOBALS['title_scope']['external_calls']"),
        ]
        for operation, counter in operations:
            stub = "function get_the_title(#[\\SensitiveParameter]$post=0){try{" + operation + ";}catch(Throwable$unused){$GLOBALS['attempts']=" + counter + ";throw new RuntimeException('PRIVATE_SOURCE');}}"
            result = self.php(stub + r'''
$caught=false;try{oracle_title(title_input($cases[0]),$cases[0]);}catch(RuntimeException$error){
check($error->getMessage()==='Prepared title reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));$caught=true;}
echo json_encode(['caught'=>$caught,'attempts'=>$attempts,'restored'=>$title_counts['restored']],JSON_THROW_ON_ERROR);
''', [title_profile.fixture(b'PRIVATE_RAW')], actual=False)
            self.assertEqual(result, {'caught': True, 'attempts': 1, 'restored': 1})

    def test_fixed_admin_constant_is_refused_without_overriding_host_state(self):
        result = self.php(r'''
define('WP_ADMIN',true);$caught=false;try{oracle_title(title_input($cases[0]),$cases[0]);}
catch(RuntimeException$error){$caught=true;check(!str_contains((string)$error,'PRIVATE_'));}
echo json_encode(['caught'=>$caught,'admin'=>WP_ADMIN,'calls'=>$title_counts['calls']],JSON_THROW_ON_ERROR);
''', [title_profile.fixture(b'PRIVATE_RAW')])
        self.assertEqual(result, {'caught': True, 'admin': True, 'calls': 0})

    def test_callback_source_is_data_only_hash_bound_and_failure_path_is_redacted(self):
        directory = tempfile.TemporaryDirectory(prefix='title-source-contract-', dir=ROOT / 'target')
        self.addCleanup(directory.cleanup)
        self.addCleanup(owned_probe.reap_owned_probes)
        path = Path(directory.name) / 'PRIVATE_SOURCE.php'
        raw = b'<?php throw new RuntimeException("PRIVATE_EXECUTION");'
        path.write_bytes(raw)
        case = title_profile.fixture(b'PRIVATE_RAW')
        case['path'] = str(path)
        result = self.php(r'''
$caught=false;try{title_check_callbacks($cases[0]['path']);}
catch(RuntimeException$error){$caught=true;check($error->getMessage()==='Prepared title callback source differs'
&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));}
echo json_encode(['caught'=>$caught,'callbacks'=>title_check_callbacks()],JSON_THROW_ON_ERROR);
''', [case])
        self.assertEqual(result, {'caught': True, 'callbacks': title_profile.CALLBACKS})
        self.assertEqual(path.read_bytes(), raw)

    def test_actual_full_oracle_get_option_tripwire_redacts_arguments_without_boot_stub(self):
        code = r'''
$repo=$argv[1];$argv[1]=$repo.'/.reference/wordpress-7.1.3';
ini_set('zend.exception_ignore_args','0');
ob_start();require$repo.'/scripts/wp_oracle.php';ob_end_clean();
$GLOBALS['title_scope']=['active'=>true,'cache_calls'=>0,'external_calls'=>0];$caught=false;
try{get_option('PRIVATE_GLUE_OPTION');}catch(RuntimeException$error){
if($error->getMessage()!=='Prepared title option access forbidden'||$error->getPrevious()!==null
||str_contains((string)$error,'PRIVATE_GLUE_OPTION')){throw new LogicException('Glue privacy contract failed');}$caught=true;}
echo json_encode(['caught'=>$caught,'calls'=>$GLOBALS['title_scope']['external_calls']],JSON_THROW_ON_ERROR);
'''
        pid, result = owned_probe.run_captured_probe([str(PHP), '-n', '-r', code, str(ROOT)],
            os.environ.copy(), timeout=20, input_bytes=b'[]')
        self.assertGreater(pid, 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        self.assertEqual(json.loads(result.stdout), {'caught': True, 'calls': 1})


if __name__ == '__main__':
    unittest.main()
