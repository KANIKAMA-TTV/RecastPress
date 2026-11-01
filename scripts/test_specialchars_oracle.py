"""Synthetic process checks for the exact selected-style oracle scope."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / ".reference/php-8.5.12/php.exe"


@unittest.skipUnless(PHP.is_file(), "pinned development PHP is unavailable")
class SpecialcharsOracleTests(unittest.TestCase):
    def run_scope(self, fail):
        code = r'''
require $argv[1] . '/scripts/reference_functions.php';
load_reference_function($argv[1] . '/scripts/wp_oracle.php', 'oracle_specialchars_utf8');
function default_html_entity_profile() { return []; }
$default_html_entity_names = ['fixed-default'];
$fail = $argv[2] === 'true';
function _wp_specialchars($input, $quote, $charset, $double) {
    global $fail;
    if ($charset !== 'UTF-8' || ini_get('default_charset') !== 'UTF-8'
        || !in_array($quote, [0,2,3,16,'single','double'], true) || !is_bool($double)
        || $GLOBALS['allowedentitynames'] !== ['fixed-default']
        || $GLOBALS['allowedxmlentitynames'] !== ['amp','lt','gt','apos','quot']) {
        throw new LogicException('wrong temporary specialchars scope');
    }
    if ($fail) { throw new RuntimeException('injected inner failure'); }
    return $input;
}
$count = 0;
foreach (['UTF-8','ISO-8859-1',''] as $charset) { foreach ([0,1,2] as $state) {
    ini_set('default_charset', $charset); $had = $state !== 0;
    $html = $state === 1 ? null : ['prior-html']; $xml = $state === 2 ? null : ['prior-xml'];
    if ($had) { $GLOBALS['allowedentitynames']=$html; $GLOBALS['allowedxmlentitynames']=$xml; }
    else { unset($GLOBALS['allowedentitynames'],$GLOBALS['allowedxmlentitynames']); }
    foreach (['noquotes','compat','quotes','single','double','xml1'] as $style) { foreach ([false,true] as $double) {
        $caught=false;
        try { $result=oracle_specialchars_utf8("\0\xff&",$style,$double); }
        catch (RuntimeException $error) {
            if (!$fail || $error->getMessage() !== 'injected inner failure') { throw $error; }
            $caught=true;
        }
        if ($caught !== $fail || (!$fail && $result !== "\0\xff&") || ini_get('default_charset') !== $charset
            || array_key_exists('allowedentitynames',$GLOBALS) !== $had
            || array_key_exists('allowedxmlentitynames',$GLOBALS) !== $had
            || ($had && ($GLOBALS['allowedentitynames'] !== $html || $GLOBALS['allowedxmlentitynames'] !== $xml))) {
            throw new LogicException('specialchars scope leaked');
        }
        ++$count;
    }}
    foreach ([[null,'quotes',false],[true,'quotes',false],[1,'quotes',false],[[],'quotes',false],
              ['x',3,false],['x',16,false],['x',true,false],['x','XML',false],['x','unknown',false],
              ['x','quotes',0],['x','quotes',1.0],['x','quotes','false'],['x','quotes',null]] as $bad) {
        $rejected=false;
        try { oracle_specialchars_utf8($bad[0],$bad[1],$bad[2]); }
        catch (RuntimeException $error) { $rejected=str_contains($error->getMessage(),'selected style'); }
        if (!$rejected || ini_get('default_charset') !== $charset
            || array_key_exists('allowedentitynames',$GLOBALS) !== $had
            || array_key_exists('allowedxmlentitynames',$GLOBALS) !== $had
            || ($had && ($GLOBALS['allowedentitynames'] !== $html || $GLOBALS['allowedxmlentitynames'] !== $xml))) {
            throw new LogicException('invalid admission changed scope');
        }
    }
}}
echo json_encode(['restored'=>$count,'inner_failure'=>$fail]),"\n";
'''
        result = subprocess.run([str(PHP), "-n", "-r", code, str(ROOT), "true" if fail else "false"],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=10)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {"restored": 108, "inner_failure": fail})

    def test_selected_styles_success_restore_ini_and_global_absence_null_and_values(self):
        self.run_scope(False)

    def test_inner_exceptions_restore_scope_and_bad_types_do_not_enter_it(self):
        self.run_scope(True)
