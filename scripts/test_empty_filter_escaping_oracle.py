"""Synthetic checks of the exact escaping wrapper's temporary global scope."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / ".reference/php-8.5.12/php.exe"


@unittest.skipUnless(PHP.is_file(), "pinned development PHP is unavailable")
class EmptyFilterEscapingOracleTests(unittest.TestCase):
    def run_scope(self, fail):
        code = r'''
require $argv[1] . '/scripts/reference_functions.php';
load_reference_function($argv[1] . '/scripts/wp_oracle.php', 'oracle_escape_utf8_empty_filters');
function default_html_entity_profile() { return []; }
$default_html_entity_names=['fixed-default'];
$empty_filter_hook_counts=['esc_html'=>0,'attribute_escape'=>0];
$fail=$argv[2]==='true';
function fake_escape($api,$input) {
    global $fail;
    if (ini_get('default_charset')!=='UTF-8' || $GLOBALS['wp_filter']!==[]
        || $GLOBALS['wp_filters']!==[] || $GLOBALS['wp_current_filter']!==[]
        || $GLOBALS['allowedentitynames']!==['fixed-default']
        || $GLOBALS['allowedxmlentitynames']!==['amp','lt','gt','apos','quot']) {
        throw new LogicException('wrong temporary escaping scope');
    }
    $GLOBALS['wp_filters']=[$api==='esc_html'?'esc_html':'attribute_escape'=>1];
    if ($fail) { throw new RuntimeException('injected inner failure'); }
    return $input;
}
function esc_html($input) { return fake_escape('esc_html',$input); }
function esc_attr($input) { return fake_escape('esc_attr',$input); }
$tracked=['wp_filter','wp_filters','wp_current_filter','allowedentitynames','allowedxmlentitynames'];
$count=0;
foreach (['UTF-8','ISO-8859-1','']as$charset) { foreach ([0,1,2]as$state) { foreach (['esc_html','esc_attr']as$api) {
    ini_set('default_charset',$charset);$had=$state!==0;$saved=[];
    foreach ($tracked as $index=>$name) {
        if (!$had) { unset($GLOBALS[$name]); } else { $GLOBALS[$name]=($index%2===($state-1))?null:['sentinel']; }
        $saved[$name]=['had'=>array_key_exists($name,$GLOBALS),'value'=>$GLOBALS[$name]??null];
    }
    if ($state===0) { unset($GLOBALS['wp_actions']); }
    else { $GLOBALS['wp_actions']=$state===1?null:['untouched'=>13]; }
    $had_actions=array_key_exists('wp_actions',$GLOBALS);$actions=$GLOBALS['wp_actions']??null;
    $caught=false;
    try { $out=oracle_escape_utf8_empty_filters($api,"\0\xff&",'empty'); }
    catch (RuntimeException $error) {
        if (!$fail || $error->getMessage()!=='injected inner failure') { throw $error; }
        $caught=true;
    }
    if ($caught!==$fail || (!$fail&&$out!=="\0\xff&") || ini_get('default_charset')!==$charset
        || array_key_exists('wp_actions',$GLOBALS)!==$had_actions || ($had_actions&&$GLOBALS['wp_actions']!==$actions)) {
        throw new LogicException('escaping INI/actions scope leaked');
    }
    foreach ($saved as$name=>$old) {
        if (array_key_exists($name,$GLOBALS)!==$old['had'] || ($old['had']&&$GLOBALS[$name]!==$old['value'])) {
            throw new LogicException('escaping globals scope leaked');
        }
    }
    $counter_before=$empty_filter_hook_counts;
    foreach ([[null,'empty'],[false,'empty'],[1,'empty'],[1.5,'empty'],[[],'empty'],['',null],['',true],['',1],['','EMPTY'],['',[]]]as$bad) {
        $rejected=false;
        try { oracle_escape_utf8_empty_filters($api,$bad[0],$bad[1]); }
        catch (RuntimeException $error) { $rejected=str_contains($error->getMessage(),'explicit empty filter profile'); }
        if (!$rejected || $empty_filter_hook_counts!==$counter_before || ini_get('default_charset')!==$charset) {
            throw new LogicException('invalid escaping admission mutated scope');
        }
        foreach ($saved as$name=>$old) {
            if (array_key_exists($name,$GLOBALS)!==$old['had'] || ($old['had']&&$GLOBALS[$name]!==$old['value'])) {
                throw new LogicException('invalid escaping admission mutated globals');
            }
        }
    }
    ++$count;
}}}
echo json_encode(['restored'=>$count,'inner_failure'=>$fail,'counts'=>$empty_filter_hook_counts]),"\n";
'''
        result = subprocess.run([str(PHP), "-n", "-r", code, str(ROOT), "true" if fail else "false"],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=10)
        self.assertEqual(result.stderr, b"")
        expected_count = 0 if fail else 9
        self.assertEqual(json.loads(result.stdout), {"restored": 18, "inner_failure": fail,
                         "counts": {"esc_html": expected_count, "attribute_escape": expected_count}})

    def test_success_restores_three_hook_and_two_entity_globals_and_ini_without_touching_actions(self):
        self.run_scope(False)

    def test_inner_failure_restores_scope_and_invalid_types_never_enter_or_increment_counts(self):
        self.run_scope(True)
