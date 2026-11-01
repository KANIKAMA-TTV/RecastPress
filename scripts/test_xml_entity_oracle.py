"""Synthetic process checks for the oracle's temporary PHP charset/global scope."""
import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
PHP = ROOT / ".reference/php-8.5.12/php.exe"


@unittest.skipUnless(PHP.is_file(), "pinned development PHP is unavailable")
class XmlEntityOracleTests(unittest.TestCase):
    def run_scope(self, fail):
        # Load the exact wrapper without executing the oracle's input loop or
        # WordPress bootstrap. Stubs exercise success and a throw *inside* try.
        code = r'''
require $argv[1] . '/scripts/reference_functions.php';
load_reference_function($argv[1] . '/scripts/wp_oracle.php', 'oracle_normalize_xml_utf8_entities');
function default_html_entity_profile() { return []; }
$default_html_entity_names = ['fixed-default'];
$fail = $argv[2] === 'true';
function wp_kses_normalize_entities($input, $context) {
    global $fail;
    if (ini_get('default_charset') !== 'UTF-8' || $context !== 'xml'
        || $GLOBALS['allowedentitynames'] !== ['fixed-default']
        || $GLOBALS['allowedxmlentitynames'] !== ['amp','lt','gt','apos','quot']) {
        throw new LogicException('wrong temporary scope');
    }
    if ($fail) { throw new RuntimeException('injected inner failure'); }
    return $input;
}
$count = 0;
foreach (['UTF-8', 'ISO-8859-1', ''] as $charset) {
    foreach ([0, 1, 2] as $state) {
        $had = $state !== 0;
        ini_set('default_charset', $charset);
        $prior_html = $state === 1 ? null : ['prior-html'];
        $prior_xml = $state === 2 ? null : ['prior-xml'];
        if ($had) { $GLOBALS['allowedentitynames'] = $prior_html; $GLOBALS['allowedxmlentitynames'] = $prior_xml; }
        else { unset($GLOBALS['allowedentitynames'], $GLOBALS['allowedxmlentitynames']); }
        $caught = false;
        try { $result = oracle_normalize_xml_utf8_entities("\0\xff&copy;", 'xml'); }
        catch (RuntimeException $error) {
            if (!$fail || $error->getMessage() !== 'injected inner failure') { throw $error; }
            $caught = true;
        }
        if ($caught !== $fail || (!$fail && $result !== "\0\xff&copy;")
            || ini_get('default_charset') !== $charset
            || array_key_exists('allowedentitynames', $GLOBALS) !== $had
            || array_key_exists('allowedxmlentitynames', $GLOBALS) !== $had
            || ($had && ($GLOBALS['allowedentitynames'] !== $prior_html || $GLOBALS['allowedxmlentitynames'] !== $prior_xml))) {
            throw new LogicException('temporary scope leaked');
        }
        foreach ([[null,'xml'],[false,'xml'],[1,'xml'],[[],'xml'],['x',true],['x','XML'],['x',null],['x','html']] as $bad) {
            $rejected = false;
            try { oracle_normalize_xml_utf8_entities($bad[0], $bad[1]); }
            catch (RuntimeException $error) { $rejected = str_contains($error->getMessage(), 'literal XML context'); }
            if (!$rejected || ini_get('default_charset') !== $charset
                || array_key_exists('allowedentitynames', $GLOBALS) !== $had
                || array_key_exists('allowedxmlentitynames', $GLOBALS) !== $had) {
                throw new LogicException('type admission changed scope');
            }
        }
        ++$count;
    }
}
echo json_encode(['restored' => $count, 'inner_failure' => $fail]), "\n";
'''
        result = subprocess.run([str(PHP), "-n", "-r", code, str(ROOT), "true" if fail else "false"],
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=10)
        self.assertEqual(result.stderr, b"")
        self.assertEqual(json.loads(result.stdout), {"restored": 9, "inner_failure": fail})

    def test_success_restores_absent_null_values_and_non_utf8_prior_charset(self):
        self.run_scope(False)

    def test_inner_failure_and_invalid_types_restore_or_never_enter_temporary_scope(self):
        self.run_scope(True)
