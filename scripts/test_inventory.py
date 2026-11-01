import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

import wordpress_reference as reference

PHP = Path(os.environ.get("RECASTPRESS_TEST_PHP", str(reference.REFERENCE / "php-8.5.12" / "php.exe")))


@unittest.skipUnless(PHP.is_file(), "development PHP CLI not configured")
class InventoryTests(unittest.TestCase):
    def test_tokenization_does_not_execute_source_or_read_fake_declarations(self):
        root = (reference.ROOT / "target" / "inventory-tests").resolve()
        assert root.is_relative_to(reference.ROOT.resolve())
        root.mkdir(parents=True, exist_ok=True)
        temporary = tempfile.TemporaryDirectory(dir=root)
        assert Path(temporary.name).resolve().parent == root
        with temporary as folder:
            source = Path(folder) / "fixture.php"
            source.write_text('''<?php
throw new RuntimeException('SOURCE MUST NOT EXECUTE');
// function fake_comment() {} do_action('fake');
$string = "function fake_string() {}";
function &by_reference() { static $x; return $x; }
function add_filter($name) {}
$closure = function () {};
class Example { public function method() {} }
$class_name = Example::class;
$anonymous = new class { public function anonymous_method() {} };
add_filter('real_literal', 'callback');
\\do_action('qualified');
apply_filters('dynamic_' . $name, $value);
$object->do_action('method_is_not_global_hook');
Example::do_action('static_is_not_global_hook');
''', encoding="utf-8")
            result = subprocess.run([str(PHP.resolve()), "-n", str(reference.ROOT / "scripts" / "wp_inventory.php"), folder], input=b'["fixture.php"]', stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=15)
            self.assertEqual(result.stderr, b"")
            inventory = json.loads(result.stdout)
            names = [row["name"] for row in inventory["declarations"]]
            self.assertEqual(names, ["by_reference", "add_filter", "Example", "method", "anonymous_method"])
            self.assertEqual([row["function"] for row in inventory["hooks"]], ["add_filter", "do_action", "apply_filters"])
            self.assertEqual([row["first_argument_kind"] for row in inventory["hooks"]], ["literal_source", "literal_source", "expression"])
            self.assertTrue(all(row["source"] == "fixture.php" and row["line"] > 0 for row in inventory["declarations"]))

    def test_namespaces_class_owners_visibility_and_nested_functions_are_lexical(self):
        root = (reference.ROOT / "target" / "inventory-tests").resolve()
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as folder:
            source = Path(folder) / "fixture.php"
            source.write_text('''<?php
namespace First\\Second {
    throw new \\RuntimeException('SOURCE MUST NOT EXECUTE');
    /** @internal */
    class Example {
        /** @deprecated old method */
        protected static function &method($arg = ['{']) {
            function nested_global() {}
            $closure = function () { function in_closure() {} };
            $x = new class(new class { function inner() {} }) {
                private function hidden() {}
            };
            \\do_action('explicit');
        }
        public function after() {}
    }
    interface Contract { public function required(); }
    trait Shared { function shared_method() {} }
    enum Choice { case A; function enum_method() {} }
    function top() { $x = "{$GLOBALS['name']}"; }
    do_action('unresolved');
}
namespace {
    function global_function() {}
}
''', encoding="utf-8")
            result = subprocess.run([str(PHP.resolve()), "-n", str(reference.ROOT / "scripts/wp_inventory.php"), folder], input=b'["fixture.php"]', stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=15)
            self.assertEqual(result.stderr, b"")
            inventory = json.loads(result.stdout)
            rows = {row["name"]: row for row in inventory["declarations"]}
            self.assertEqual(rows["Example"]["qualified_name"], r"\First\Second\Example")
            self.assertTrue(rows["Example"]["documented_internal"])
            method = rows["method"]
            self.assertEqual(method["kind"], "method")
            self.assertEqual(method["qualified_name"], r"\First\Second\Example::method")
            self.assertEqual(method["visibility"], "protected")
            self.assertTrue(method["static"])
            self.assertTrue(method["documented_deprecated"])
            for name in ["nested_global", "in_closure", "top"]:
                self.assertEqual(rows[name]["kind"], "function")
                self.assertEqual(rows[name]["qualified_name"], "\\First\\Second\\" + name)
            self.assertEqual(rows["after"]["class"], r"\First\Second\Example")
            for name in ["inner", "hidden"]:
                self.assertEqual(rows[name]["kind"], "method")
                self.assertTrue(rows[name]["anonymous_class"])
            self.assertNotEqual(rows["inner"]["class"], rows["hidden"]["class"])
            self.assertEqual(rows["hidden"]["visibility"], "private")
            self.assertEqual(rows["required"]["class"], r"\First\Second\Contract")
            self.assertEqual(rows["shared_method"]["class"], r"\First\Second\Shared")
            self.assertEqual(rows["enum_method"]["class"], r"\First\Second\Choice")
            self.assertEqual(rows["global_function"]["namespace"], "")
            self.assertEqual(rows["global_function"]["qualified_name"], r"\global_function")
            self.assertEqual([row["name_resolution"] for row in inventory["hooks"]], ["explicit_global", "unresolved_unqualified"])

    def test_semicolon_namespaces_and_function_imports_do_not_create_declarations(self):
        root = (reference.ROOT / "target" / "inventory-tests").resolve()
        root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(dir=root) as folder:
            (Path(folder) / "fixture.php").write_text('''<?php
namespace Alpha;
use function strlen;
use Some\\Package\\{function imported, function second as alias};
$closure = function () use ($capture) { function nested() {} };
class A { public function first() {} function second() {} }
function real() {}
namespace Beta\\Child;
function real() {}
''', encoding="utf-8")
            result = subprocess.run([str(PHP.resolve()), "-n", str(reference.ROOT / "scripts/wp_inventory.php"), folder], input=b'["fixture.php"]', stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=15)
            self.assertEqual(result.stderr, b"")
            rows = json.loads(result.stdout)["declarations"]
            self.assertEqual([row["name"] for row in rows], ["nested", "A", "first", "second", "real", "real"])
            self.assertEqual(rows[-2]["qualified_name"], r"\Alpha\real")
            self.assertEqual(rows[-1]["qualified_name"], r"\Beta\Child\real")
            self.assertEqual(rows[0]["kind"], "function")
            self.assertEqual(rows[2]["visibility"], "public")
            self.assertEqual(rows[3]["visibility"], "public")


if __name__ == "__main__":
    unittest.main()
