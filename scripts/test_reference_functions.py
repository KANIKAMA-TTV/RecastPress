"""Synthetic loader contracts; no WordPress download, DB or full bootstrap."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import wordpress_reference as reference

PHP = Path(os.environ.get("RECASTPRESS_TEST_PHP", str(reference.REFERENCE / "php-8.5.12/php.exe")))
LOADER = reference.ROOT / "scripts/reference_functions.php"
FIXTURE = '''<?php
throw new RuntimeException('SOURCE TOP LEVEL MUST NOT EXECUTE');
// function fake_in_comment() { }
$literal = 'function fake_in_string() { }';
$anonymous = function () { return 'anonymous'; };
if (false) { function duplicate_fixture() { return 'first'; } }
if (false) { function duplicate_fixture() { return 'second'; } }
function OuterFixture() {
    function InnerFixture() { return 'inner'; }
    return 'outer';
}
function curly_fixture($input) {
    return "{$input['name']}" . "}";
}
function dollar_curly_fixture($input) {
    return "${input}";
}
function &by_reference_fixture() { static $value = 1; return $value; }
function /* deliberately unsupported placement */ comment_fixture() { return 2; }
function selected_fixture() { return 'selected'; }
function unselected_fixture() { return 'must not be defined'; }
'''


@unittest.skipUnless(PHP.is_file(), "development PHP CLI not configured")
class ReferenceFunctionTests(unittest.TestCase):
    def setUp(self):
        parent = (reference.ROOT / "target/reference-functions-tests").resolve()
        parent.mkdir(parents=True, exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(dir=parent)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "fixture.php"
        self.source.write_bytes(FIXTURE.encode("utf-8"))

    def run_php(self, code, check=True):
        prefix = '''error_reporting(E_ALL);
set_error_handler(static function ($level, $message, $file, $line) { throw new ErrorException($message, 0, $level, $file, $line); });
require $argv[1];
$source = $argv[2];
'''
        completed = subprocess.run([str(PHP.resolve()), "-n", "-r", prefix + code, str(LOADER), str(self.source)],
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=15,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        if check:
            self.assertEqual(completed.returncode, 0, (completed.stdout + completed.stderr).decode(errors="replace"))
            self.assertEqual(completed.stderr, b"")
            return json.loads(completed.stdout)
        return completed

    def test_first_duplicate_payload_and_only_selected_declaration_execute(self):
        actual = self.run_php('''
load_reference_function($source, 'selected_fixture');
load_reference_function($source, 'duplicate_fixture');
echo json_encode([selected_fixture(), duplicate_fixture(), function_exists('unselected_fixture'), function_exists('OuterFixture'), function_exists('InnerFixture'), function_exists('fake_in_string')]);
''')
        self.assertEqual(actual, ["selected", "first", False, False, False, False])

    def test_exact_names_missing_nonstrings_byref_and_comment_remain_unsupported(self):
        actual = self.run_php('''
$results = [];
foreach (['SELECTED_FIXTURE', 'missing_fixture', 'by_reference_fixture', 'comment_fixture', 'fake_in_comment', 'fake_in_string', 123, true, null] as $name) {
    try { load_reference_function($source, $name); $results[] = 'unexpected success'; }
    catch (RuntimeException $error) { $results[] = $error->getMessage(); }
}
echo json_encode($results);
''')
        self.assertEqual(actual, ["reference function missing: " + name for name in
            ["SELECTED_FIXTURE", "missing_fixture", "by_reference_fixture", "comment_fixture", "fake_in_comment", "fake_in_string", "123", "1", ""]])

    def test_nested_function_remains_discoverable_without_evaluating_outer(self):
        actual = self.run_php('''
load_reference_function($source, 'InnerFixture');
echo json_encode([InnerFixture(), function_exists('OuterFixture')]);
''')
        self.assertEqual(actual, ["inner", False])

    def test_curly_and_dollar_curly_tokens_preserve_exact_fixture_snippets(self):
        actual = self.run_php('''
$snippets = reference_function_snippets($source);
load_reference_function($source, 'curly_fixture');
echo json_encode([$snippets['OuterFixture'], $snippets['InnerFixture'], $snippets['curly_fixture'], $snippets['dollar_curly_fixture'], curly_fixture(['name' => 'Ada'])]);
''')
        self.assertEqual(actual, [
            "function OuterFixture() {\n    function InnerFixture() { return 'inner'; }\n    return 'outer';\n}",
            "function InnerFixture() { return 'inner'; }",
            "function curly_fixture($input) {\n    return \"{$input['name']}\" . \"}\";\n}",
            'function dollar_curly_fixture($input) {\n    return "${input}";\n}',
            "Ada}",
        ])

    def test_cache_holds_only_code_strings_and_does_not_retokenize_same_file_key(self):
        # Deliberate temporary-file deletion exercises the cache mechanism only;
        # production callers are required to keep hash-verified sources immutable.
        actual = self.run_php('''
$first = reference_function_snippets($source);
unlink($source);
$second = reference_function_snippets($source);
$state = (new ReflectionFunction('reference_function_snippets'))->getStaticVariables();
$only_strings = true;
foreach ($state['by_file'] as $snippets) {
    foreach ($snippets as $code) { $only_strings = $only_strings && is_string($code); }
}
load_reference_function($source, 'selected_fixture');
echo json_encode([$first === $second, $only_strings, array_keys($state), count($state['by_file']), selected_fixture(), ini_get('memory_limit')]);
''')
        self.assertEqual(actual, [True, True, ["by_file"], 1, "selected", "128M"])

    def test_different_files_keep_independent_caches_and_repeated_eval_fails(self):
        self.source.write_bytes(b"<?php function first_file_fixture() { return 'first file'; }\n")
        other = self.root / "other.php"
        other.write_bytes(b"<?php function other_file_fixture() { return 'other file'; }\n")
        actual = self.run_php('''
load_reference_function($source, 'first_file_fixture');
load_reference_function(dirname($source) . '/other.php', 'other_file_fixture');
echo json_encode([first_file_fixture(), other_file_fixture(), count((new ReflectionFunction('reference_function_snippets'))->getStaticVariables()['by_file'])]);
''')
        self.assertEqual(actual, ["first file", "other file", 2])
        repeated = self.run_php('''
load_reference_function($source, 'first_file_fixture');
load_reference_function($source, 'first_file_fixture');
echo 'unexpected success';
''', check=False)
        self.assertNotEqual(repeated.returncode, 0)
        self.assertIn(b"Cannot redeclare", repeated.stdout + repeated.stderr)
        self.assertNotIn(b"unexpected success", repeated.stdout + repeated.stderr)


if __name__ == "__main__":
    unittest.main()
