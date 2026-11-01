"""Pinned-source English texturization contracts; no title pipeline or gettext."""
import json
import os
from pathlib import Path
import unittest

import owned_probe

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / '.reference/php-8.5.12/php.exe'
WP = ROOT / '.reference/wordpress-7.1.3'
COUNTS = ('calls', 'initialized', 'x_calls', 'run_hook', 'no_tags_hook',
          'no_shortcodes_hook', 'spaces_hook', 'restored')
BOOT = r'''
$repo=$argv[1];$root=$repo.'/.reference/wordpress-7.1.3';
error_reporting(E_ALL);ini_set('zend.exception_ignore_args','0');setlocale(LC_CTYPE,'C');
set_error_handler(static function($level,$message,$file,$line){throw new ErrorException($message,0,$level,$file,$line);});
require$repo.'/scripts/reference_functions.php';require$root.'/wp-includes/plugin.php';
require$repo.'/scripts/texturize_oracle_profile.php';
function check($condition){if(!$condition){throw new LogicException('Texturize contract failed');}}
$cases=json_decode(stream_get_contents(STDIN),true,128,JSON_THROW_ON_ERROR);
'''


def fixture(raw='plain'):
    return {'raw': raw, 'texturize_profile': 'valid_utf8_no_lt_default_english_empty_filters',
            'reset': True, 'limits': {'max_input_bytes': 65536, 'max_work_bytes': 1048576,
                                    'max_output_bytes': 65536}}


@unittest.skipUnless(PHP.is_file() and (WP / 'wp-includes/formatting.php').is_file(),
                     'pinned PHP/WordPress unavailable')
class TexturizeOracleTests(unittest.TestCase):
    def setUp(self):
        self.addCleanup(owned_probe.reap_owned_probes)

    def php(self, body, cases, *, actual=True):
        code = BOOT + ('texturize_load_reference_functions($root);\n' if actual else '') + body
        pid, result = owned_probe.run_captured_probe(
            [str(PHP), '-n', '-d', 'memory_limit=128M', '-r', code, str(ROOT)],
            os.environ.copy(), timeout=20,
            input_bytes=json.dumps(cases, ensure_ascii=True, separators=(',', ':')).encode())
        self.assertGreater(pid, 0)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        self.assertLessEqual(len(result.stdout), 65536)
        return json.loads(result.stdout)

    def test_actual_quotes_primes_dashes_entities_and_english_static_replacements(self):
        samples = ["'Hello'", '"world"', "can't", '9"', "9'", 'a---b', 'a--b',
                   'a -- b', 'a - b', 'xn--name', '...', ' (tm)', "'til", '2x3',
                   '0x9999', 'A&B &amp; &bogus; &toolongentity;', '"7\". 8\"!']
        expected = ['&#8216;Hello&#8217;', '&#8220;world&#8221;', 'can&#8217;t',
                    '9&#8243;', '9&#8242;', 'a&#8212;b', 'a&#8211;b',
                    'a &#8212; b', 'a &#8211; b', 'xn--name', '&#8230;', ' &#8482;',
                    '&#8217;til', '2&#215;3', '0x9999',
                    'A&#038;B &amp; &bogus; &#038;toolongentity;',
                    '&#8220;7&#8221;. 8&#8243;!']
        result = self.php(r'''
$results=[];foreach($cases as$case){$results[]=oracle_texturize($case['raw'],$case);}
echo json_encode(['results'=>$results,...texturize_metadata()],JSON_THROW_ON_ERROR);
''', [fixture(value) for value in samples])
        self.assertEqual(result['results'], expected)
        count = len(samples)
        self.assertEqual(result['texturize_counts'], {'calls': count, 'initialized': count,
            'x_calls': count * 11, 'run_hook': count, 'no_tags_hook': count,
            'no_shortcodes_hook': count, 'spaces_hook': 1, 'restored': count})
        self.assertEqual(result['texturize_profile']['ctype_locale'], 'C')

    def test_empty_zero_do_not_initialize_and_spaces_hook_is_process_once(self):
        result = self.php(r'''
$results=[];foreach($cases as$case){$results[]=oracle_texturize($case['raw'],$case);}
echo json_encode(['results'=>$results,'counts'=>$texturize_counts],JSON_THROW_ON_ERROR);
''', [fixture(value) for value in ['', '0', ' ', '"x"', '', '0', "'x'", 'x']])
        self.assertEqual(result['results'], ['', '0', ' ', '&#8220;x&#8221;', '', '0',
                                              '&#8216;x&#8217;', 'x'])
        self.assertEqual(result['counts'], {'calls': 8, 'initialized': 4, 'x_calls': 44,
            'run_hook': 4, 'no_tags_hook': 4, 'no_shortcodes_hook': 4,
            'spaces_hook': 1, 'restored': 8})

    def test_presence_null_empty_and_object_global_states_restore_on_success_and_source_failure(self):
        body = r'''
$names=['wp_filter','wp_filters','wp_current_filter','shortcode_tags','wp_cockneyreplace','texturize_scope',
        'wp_actions','wp_hasher','wpdb'];$n=0;
foreach([0,1,2,3]as$state){foreach($cases as$case){$before=[];
 foreach($names as$name){if($state===0){unset($GLOBALS[$name]);}else{$GLOBALS[$name]=match($state){1=>null,2=>[],default=>['PRIVATE_STATE'=>new stdClass()]};}
 $before[$name]=[array_key_exists($name,$GLOBALS),$GLOBALS[$name]??null];}
 $caught=false;try{oracle_texturize($case['raw'],$case);}catch(RuntimeException$error){
 check(EXPECT_FAILURE&&$error->getMessage()==='Texturize reference failed'&&$error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_SOURCE'));$caught=true;}
 check($caught===EXPECT_FAILURE);foreach($before as$name=>[$had,$old]){check(array_key_exists($name,$GLOBALS)===$had&&(!$had||$GLOBALS[$name]===$old));}++$n;
}}
echo json_encode(['n'=>$n,'restored'=>$texturize_counts['restored']],JSON_THROW_ON_ERROR);
'''
        for fail in (False, True):
            prefix = 'const EXPECT_FAILURE=' + ('true;' if fail else 'false;')
            if fail:
                prefix += r'''
function wptexturize($text,$reset=false){static$run_texturize=true;throw new RuntimeException('PRIVATE_SOURCE');}
function wp_spaces_regexp(){static$spaces='';return$spaces;}
'''
            self.assertEqual(self.php(prefix + body, [fixture('PRIVATE_RAW'), fixture('')],
                                      actual=not fail), {'n': 8, 'restored': 8})

    def test_protocol_refusals_precede_global_mutation_and_actual_calls(self):
        cases = []
        for raw in [None, False, 1, [], '<PRIVATE_RAW>', 'x' * 65537]:
            cases.append(fixture(raw))
        for field, value in [('texturize_profile', 'PRIVATE_PROFILE'), ('reset', False),
                             ('reset', 1), ('limits', None)]:
            case = fixture(); case[field] = value; cases.append(case)
        for key in fixture()['limits']:
            for value in (False, -1, 'PRIVATE_LIMIT', fixture()['limits'][key] - 1):
                case = fixture(); case['limits'][key] = value; cases.append(case)
        case = fixture(); case['limits']['extra'] = 0; cases.append(case)
        result = self.php(r'''
$names=['wp_filter','wp_filters','wp_current_filter','shortcode_tags','wp_cockneyreplace','texturize_scope'];
$before=[];foreach($names as$name){$GLOBALS[$name]=new stdClass();$before[$name]=$GLOBALS[$name];}
$caught=0;foreach($cases as$case){try{oracle_texturize($case['raw'],$case);throw new LogicException('invalid fixture admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}
foreach($before as$name=>$old){check($GLOBALS[$name]===$old);}}
try{oracle_texturize("\xffPRIVATE_UTF8",$cases[0]);throw new LogicException('invalid UTF8 admitted');}
catch(RuntimeException$error){check(!str_contains((string)$error,'PRIVATE_UTF8'));++$caught;}
echo json_encode(['caught'=>$caught,'counts'=>$texturize_counts],JSON_THROW_ON_ERROR);
''', cases)
        self.assertEqual(result, {'caught': len(cases) + 1,
                                  'counts': dict.fromkeys(COUNTS, 0)})

    def test_poisoned_run_or_changed_spaces_statics_refuse_without_attempted_reset(self):
        for poisoning in [r'''
add_filter('run_wptexturize',static fn($run)=>false);wptexturize('poison',true);
''', r'''
add_filter('wp_spaces_regexp',static fn($spaces)=>'PRIVATE_SPACES');wp_spaces_regexp();
''']:
            result = self.php(poisoning + r'''
$before=[$wp_filter,$wp_filters];$caught=0;
foreach($cases as$case){try{oracle_texturize($case['raw'],$case);throw new LogicException('poison admitted');}
catch(RuntimeException$error){check($error->getPrevious()===null&&!str_contains((string)$error,'PRIVATE_'));++$caught;}}
check([$wp_filter,$wp_filters]===$before);
echo json_encode(['caught'=>$caught,'counts'=>$texturize_counts],JSON_THROW_ON_ERROR);
''', [fixture('normal'), fixture('')])
            self.assertEqual(result, {'caught': 2, 'counts': dict.fromkeys(COUNTS, 0)})

    def test_translation_stub_rejects_outside_scope_and_nonwhitelisted_triples(self):
        result = self.php(r'''
$caught=0;try{_x('&#8220;','opening curly double quote');throw new LogicException('ambient translation admitted');}
catch(RuntimeException$error){++$caught;}
$texturize_scope=['active'=>true,'x_calls'=>0];
foreach([['PRIVATE_TEXT','opening curly double quote','default'],['&#8220;','PRIVATE_CONTEXT','default'],
['&#8220;','opening curly double quote','PRIVATE_DOMAIN']]as$args){
try{_x(...$args);throw new LogicException('translation admitted');}catch(RuntimeException$error){check(!str_contains((string)$error,'PRIVATE_'));++$caught;}}
check(_x('&#8220;','opening curly double quote')==='&#8220;');
echo json_encode(['caught'=>$caught,'x_calls'=>$texturize_scope['x_calls']],JSON_THROW_ON_ERROR);
''', [])
        self.assertEqual(result, {'caught': 4, 'x_calls': 1})

    def test_full_oracle_glue_keeps_typed_bytes_and_old_api_results_separate(self):
        cases = []
        for index, raw in enumerate(['"x"', '0', '']):
            case = fixture(raw)
            del case['raw']
            case.update(id=f'texturize-glue-{index}', api='wptexturize',
                        input={'type': 'string', 'hex': raw.encode().hex()})
            cases.append(case)
        cases.insert(1, {'id': 'old-absint', 'api': 'absint',
                         'input': {'type': 'int', 'value': '-7'}})
        _, result = owned_probe.run_captured_probe(
            [str(PHP), '-n', '-d', 'memory_limit=128M', str(ROOT / 'scripts/wp_oracle.php'), str(WP)],
            os.environ.copy(), timeout=20,
            input_bytes=json.dumps(cases, separators=(',', ':')).encode())
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        self.assertLessEqual(len(result.stdout), 65536)
        observed = json.loads(result.stdout)
        self.assertEqual(observed['results'], [
            {'id': 'texturize-glue-0', 'value': {'type': 'string', 'hex': b'&#8220;x&#8221;'.hex()}},
            {'id': 'old-absint', 'value': {'type': 'int', 'value': '7'}},
            {'id': 'texturize-glue-1', 'value': {'type': 'string', 'hex': b'0'.hex()}},
            {'id': 'texturize-glue-2', 'value': {'type': 'string', 'hex': ''}}])
        self.assertEqual(observed['texturize_counts'], {'calls': 3, 'initialized': 1,
            'x_calls': 11, 'run_hook': 1, 'no_tags_hook': 1, 'no_shortcodes_hook': 1,
            'spaces_hook': 1, 'restored': 3})


if __name__ == '__main__':
    unittest.main()
