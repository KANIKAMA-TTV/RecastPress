"""Actual cached-single source contracts and refusal/scope tests, without SQL."""
import copy
import json
import os
from pathlib import Path
import unittest

import owned_probe
import post_meta_cached_profile as profile

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / '.reference/php-8.5.12/php.exe'
WP = ROOT / '.reference/wordpress-7.1.3'

BOOT = r'''
$repo=$argv[1];$root=$repo.'/.reference/wordpress-7.1.3';
error_reporting(E_ALL);ini_set('zend.exception_ignore_args','0');
set_error_handler(static function($level,$message,$file,$line){throw new ErrorException($message,0,$level,$file,$line);});
require$repo.'/scripts/reference_functions.php';require$root.'/wp-includes/plugin.php';require$repo.'/scripts/php_value_codec.php';
load_reference_function($root.'/wp-includes/load.php','absint');
foreach(['is_serialized','maybe_unserialize']as$name){load_reference_function($root.'/wp-includes/functions.php',$name);}
require$repo.'/scripts/post_meta_oracle_profile.php';
function check($condition){if(!$condition){throw new LogicException('Cached profile contract failed');}}
$cases=json_decode(stream_get_contents(STDIN),true,128,JSON_THROW_ON_ERROR);
'''


def fixture(raw=b'plain', witness=None, *, owner=1, missing=False):
    if missing:
        return profile.fixture(b'_wp_attachment_image_alt', owner=owner)
    if witness is None:
        return profile.fixture(b'_wp_attachment_image_alt', raw, owner=owner)
    return profile.fixture(b'_wp_attachment_image_alt', raw, witness=witness, owner=owner)


@unittest.skipUnless(PHP.is_file() and (WP / 'wp-includes/meta.php').is_file(), 'pinned PHP/WordPress unavailable')
class CachedPostMetaOracleTests(unittest.TestCase):
    def setUp(self):
        self.addCleanup(owned_probe.reap_owned_probes)

    def php(self, body, cases, *, actual=True):
        code = BOOT + ('post_meta_cached_load_reference_functions($root);\n' if actual else '') + body
        pid, result = owned_probe.run_captured_probe(
            [str(PHP), '-n', '-d', 'memory_limit=128M', '-r', code, str(ROOT)],
            os.environ.copy(), timeout=20,
            input_bytes=json.dumps(cases, ensure_ascii=True, separators=(',', ':')).encode())
        self.assertGreater(pid, 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        self.assertLessEqual(len(result.stdout), 65536)
        return json.loads(result.stdout)

    def test_actual_named_chain_preserves_values_first_only_and_source_default_branches(self):
        null = {'type': 'null'}
        false = {'type': 'bool', 'value': False}
        graph = profile.array([(profile.string(b'k'), profile.string(b'a\0b')),
                               (profile.integer(0), false)])
        outer_null = fixture(b'unused')
        outer_null['cache']['entries'][0]['value'] = null
        cases = [fixture(missing=True), outer_null, fixture(None), fixture(b''), fixture(b'0'),
                 fixture(b'N;', null), fixture(b'b:0;', false), fixture(b's:0:"";', profile.string(b'')),
                 fixture(b'\0\xff<literal>'), fixture(b's:2:"N;";', profile.string(b'N;')),
                 fixture(profile.serialize_witness(graph), graph, owner=2**63-1)]
        expected = [profile.string(b''), profile.string(b''), profile.string(b''), profile.string(b''),
                    profile.string(b'0'), profile.string(b''), false, profile.string(b''),
                    profile.string(b'\0\xff<literal>'), profile.string(b'N;'), graph]
        result = self.php(r'''
$results=[];foreach($cases as$case){$owner=post_meta_cached_decode_owner($case);$results[]=encode_value(oracle_post_meta_cached_single($owner,$case));}
echo json_encode(['results'=>$results,...post_meta_cached_metadata()],JSON_THROW_ON_ERROR);
''', cases)
        self.assertEqual(result['results'], expected)
        profile.validate_profile(result['post_meta_cached_profile'])
        self.assertEqual(result['post_meta_cached_counts'], {
            'calls': 11, 'get_hook': 11, 'default_hook': 4, 'cache_get': 11,
            'cache_update': 0, 'db_calls': 0, 'restored': 11})

    def test_all_presence_null_value_global_states_restore_after_success_and_source_failure(self):
        body = r'''
$names=['wp_filter','wp_filters','wp_current_filter','post_meta_cached_scope','wpdb','wp_actions','wp_hasher','wp_meta_keys','wp_object_cache'];
$count=0;foreach([0,1,2,3]as$state){foreach($cases as$case){
 $before=[];foreach($names as$name){if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>[],default=>['PRIVATE_SENTINEL'=>new stdClass()]};}$before[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
 $caught=false;try{$value=oracle_post_meta_cached_single(post_meta_cached_decode_owner($case),$case);}catch(RuntimeException$error){
  check(EXPECT_FAILURE&&$error->getMessage()==='Cached post meta reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_SOURCE_FAILURE'));$caught=true;}
 check($caught===EXPECT_FAILURE);
 foreach($before as$name=>[$had,$old]){check(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}++$count;
}}
echo json_encode(['cases'=>$count,'restored'=>$post_meta_cached_counts['restored']],JSON_THROW_ON_ERROR);
'''
        cases = [fixture(b''), fixture(b'N;', {'type': 'null'})]
        for fail in (False, True):
            prefix = 'const EXPECT_FAILURE=' + ('true;' if fail else 'false;')
            if fail:
                prefix += "function get_post_meta($id,$key,$single){throw new RuntimeException('PRIVATE_SOURCE_FAILURE');}"
            result = self.php(prefix + body, cases, actual=not fail)
            self.assertEqual(result, {'cases': 8, 'restored': 8})

    def test_protocol_and_each_selected_limit_refuse_before_actual_calls_or_global_mutation(self):
        base = fixture(b'plain')
        cases = []
        def changed(field, value):
            item = copy.deepcopy(base); item[field] = value; cases.append(item)
        for owner in (0, -1):
            changed('input', profile.integer(owner))
        for owner in ({'type': 'bool', 'value': 'PRIVATE_BOOL'}, profile.string(b'PRIVATE_OWNER'),
                      {'type': 'int', 'value': '9223372036854775808'}):
            changed('input', owner)
        for key in (b'', b'0'):
            changed('meta_key', profile.string(key))
        changed('meta_key', {'type': 'bool', 'value': 'PRIVATE_KEY'})
        for flag in ('single', 'complete_cache', 'empty_filters'):
            changed(flag, 1)
        changed('post_meta_profile', 'other')
        changed('cache', profile.array([]))
        changed('cache', profile.string(b'not cache'))
        for selected in (profile.array([]), profile.array([(profile.integer(1), profile.string(b'plain'))]),
                         profile.dense([profile.string(b'plain'), profile.integer(1)])):
            item = copy.deepcopy(base); item['cache']['entries'][0]['value'] = selected; cases.append(item)
        changed('witness', {'type': 'null'})  # Nonserialized input has no witness.
        for raw, witness in ((b'O:1:"X":0:{}', {'type': 'null'}), (b'i:+1;', profile.integer(1)),
                             (b'a:2:{i:0;N;i:0;R:2;}', profile.array([(profile.integer(0), {'type': 'null'})])),
                             (b'd:1.5;', {'type': 'float', 'value': '1.5'}),
                             (b'N;', {'type': 'bool', 'value': False})):
            cases.append(fixture(raw, witness))
        graph = profile.array([(profile.string(b'key'), profile.string(b'a\0b'))])
        raw = profile.serialize_witness(graph)
        for key, value in (('max_input_bytes', len(raw)-1), ('max_array_entries', 0),
                           ('max_owned_string_bytes', 5), ('max_array_depth', 0),
                           ('max_array_depth', 65), ('max_array_entries', True)):
            item = fixture(raw, graph); item['limits'][key] = value; cases.append(item)
        result = self.php(r'''
$sentinel=new stdClass();$GLOBALS['wpdb']=$sentinel;$caught=0;
foreach($cases as$case){try{oracle_post_meta_cached_single(post_meta_cached_decode_owner($case),$case);throw new LogicException('bad fixture admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
check($GLOBALS['wpdb']===$sentinel);}
echo json_encode(['caught'=>$caught,'counts'=>$post_meta_cached_counts],JSON_THROW_ON_ERROR);
''', cases)
        self.assertEqual(result['caught'], len(cases))
        self.assertEqual(result['counts'], {key: 0 for key in profile.COUNT_KEYS})

    def test_wrong_per_case_default_hook_counts_cannot_cancel_in_the_aggregate(self):
        result = self.php(r'''
function get_post_meta($id,$key,$single){
 apply_filters('get_post_metadata',null,$id,$key,$single,'post');wp_cache_get($id,'post_meta');
 if($id===1){apply_filters('default_post_metadata','',$id,$key,$single,'post');}return'';
}
$caught=0;foreach($cases as$case){try{oracle_post_meta_cached_single(post_meta_cached_decode_owner($case),$case);throw new LogicException('wrong percase counts admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null);++$caught;}}
echo json_encode(['caught'=>$caught,'counts'=>$post_meta_cached_counts],JSON_THROW_ON_ERROR);
''', [fixture(b'', owner=1), fixture(b'N;', {'type': 'null'}, owner=2)], actual=False)
        self.assertEqual(result['caught'], 2)
        self.assertEqual(result['counts']['default_hook'], 1)
        self.assertEqual(result['counts']['calls'], 2)
        self.assertEqual(result['counts']['restored'], 2)

    def test_database_and_cache_update_traps_are_real_measured_and_have_closed_errors(self):
        result = self.php(r'''
function get_post_meta($id,$key,$single){
 apply_filters('get_post_metadata',null,$id,$key,$single,'post');wp_cache_get($id,'post_meta');
 if($id===1){return$GLOBALS['wpdb']->PRIVATE_PROPERTY;}
 if($id===2){return$GLOBALS['wpdb']->query('PRIVATE_SQL');}
 return update_meta_cache('post',[$id]);
}
$old=new stdClass();$GLOBALS['wpdb']=$old;$caught=0;foreach($cases as$case){
 try{oracle_post_meta_cached_single(post_meta_cached_decode_owner($case),$case);throw new LogicException('SQL trap did not reject');}
 catch(RuntimeException$error){check($error->getMessage()==='Cached post meta reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
 check($GLOBALS['wpdb']===$old);
}
echo json_encode(['caught'=>$caught,'counts'=>$post_meta_cached_counts],JSON_THROW_ON_ERROR);
''', [fixture(owner=1), fixture(owner=2), fixture(owner=3)], actual=False)
        self.assertEqual(result['caught'], 3)
        self.assertEqual(result['counts'], {'calls': 3, 'get_hook': 3, 'default_hook': 0,
            'cache_get': 3, 'cache_update': 1, 'db_calls': 2, 'restored': 3})

    def test_later_and_unrelated_raw_values_do_not_consume_selected_decode_limits(self):
        case = fixture(b'ok')
        case['limits'] = {'max_input_bytes': 2, 'max_array_entries': 0,
                          'max_owned_string_bytes': 2, 'max_array_depth': 0}
        case['cache']['entries'][0]['value']['entries'][1]['value'] = profile.string(b'O:1:"X":0:{}' + b'X' * 60000)
        result = self.php(r'''
$case=$cases[0];$value=oracle_post_meta_cached_single(post_meta_cached_decode_owner($case),$case);
echo json_encode(['value'=>encode_value($value),'counts'=>$post_meta_cached_counts],JSON_THROW_ON_ERROR);
''', [case])
        self.assertEqual(result['value'], profile.string(b'ok'))
        self.assertEqual(result['counts'], {'calls': 1, 'get_hook': 1, 'default_hook': 0,
            'cache_get': 1, 'cache_update': 0, 'db_calls': 0, 'restored': 1})

    def test_full_oracle_glue_keeps_new_and_old_cases_in_one_batch(self):
        cases = [fixture(b'N;', {'type': 'null'}),
                 {'api': 'absint', 'input': profile.integer(-7)}, fixture(b'')]
        for index, case in enumerate(cases):
            case['id'] = f'cached-glue-{index}'
        _, result = owned_probe.run_captured_probe(
            [str(PHP), '-n', '-d', 'memory_limit=128M', str(ROOT / 'scripts/wp_oracle.php'), str(WP)],
            os.environ.copy(), timeout=20, input_bytes=json.dumps(cases, separators=(',', ':')).encode())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        self.assertLessEqual(len(result.stdout), 65536)
        observed = json.loads(result.stdout)
        self.assertEqual(observed['results'], [
            {'id': 'cached-glue-0', 'value': profile.string(b'')},
            {'id': 'cached-glue-1', 'value': profile.integer(7)},
            {'id': 'cached-glue-2', 'value': profile.string(b'')}])
        profile.validate_profile(observed['post_meta_cached_profile'])
        self.assertEqual(observed['post_meta_cached_counts'], {
            'calls': 2, 'get_hook': 2, 'default_hook': 1, 'cache_get': 2,
            'cache_update': 0, 'db_calls': 0, 'restored': 2})


if __name__ == '__main__':
    unittest.main()
