"""Exercise integer wire boundaries without loading WordPress or its oracle."""
import json
import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]
PHP = Path(os.environ.get("RECASTPRESS_TEST_PHP", ROOT / ".reference/php-8.5.12/php.exe"))
CODE = r'''
set_error_handler(static function ($level, $message) { throw new ErrorException($message, 0, $level); });
require $argv[1];
$nodes = json_decode(stream_get_contents(STDIN), true, 128, JSON_THROW_ON_ERROR);
$results = [];
foreach ($nodes as $node) {
    try { $results[] = ['ok' => true, 'value' => encode_value(decode_value($node))]; }
    catch (Throwable $error) { $results[] = ['ok' => false, 'error' => get_class($error)]; }
}
echo json_encode($results, JSON_THROW_ON_ERROR);
'''


@unittest.skipUnless(PHP.is_file(), "development PHP CLI is not available")
class PhpValueCodecTests(unittest.TestCase):
    def decode(self, nodes):
        completed = subprocess.run(
            [str(PHP), "-n", "-r", CODE, str(ROOT / "scripts/php_value_codec.php")],
            input=json.dumps(nodes).encode("ascii"), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            cwd=ROOT, timeout=15, check=True,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        self.assertEqual(completed.stderr, b"")
        return json.loads(completed.stdout)

    def test_integer_values_match_signed64_boundaries_signs_and_leading_zeroes(self):
        values = ["0", "+0", "-0", "00", "+0001", "-0001", "9223372036854775807",
                  "-9223372036854775808", "0" * 4096 + "1", "-" + "0" * 256 + "9223372036854775808"]
        results = self.decode([{"type": "int", "value": value} for value in values])
        self.assertEqual(results, [{"ok": True, "value": {"type": "int", "value": str(int(value))}}
                                  for value in values])

    def test_malformed_wrong_type_and_out_of_range_integers_never_coerce(self):
        values = ["", "+", "-", " 1", "1 ", "1\x00", "1.0", "1e3", "1junk", "--1", "+-1",
                  "9223372036854775808", "-9223372036854775809", "9" * 4096,
                  "-" + "9" * 4096, None, True, False, 0, 1.0, [], {}]
        nodes = [{"type": "int", "value": value} for value in values] + [{"type": "int"}]
        self.assertEqual(self.decode(nodes), [{"ok": False, "error": "RuntimeException"}] * len(nodes))

    def test_nested_values_and_array_keys_use_the_same_integer_validation(self):
        def array(key, value):
            return {"type": "array", "entries": [{"key": key, "value": value}]}
        integer = lambda value: {"type": "int", "value": value}
        null = {"type": "null"}
        results = self.decode([array(integer("+0001"), integer("-0002")),
                               array(integer("9223372036854775808"), null),
                               array(integer("0"), array(integer("0"), integer("-9223372036854775809")))])
        self.assertEqual(results[0], {"ok": True, "value": array(integer("1"), integer("-2"))})
        self.assertEqual(results[1:], [{"ok": False, "error": "RuntimeException"}] * 2)


if __name__ == "__main__":
    unittest.main()
