<?php
/** Development observation only: selected pinned functions, no KSES bootstrap. */
error_reporting(E_ALL);
set_error_handler(static function ($severity, $message, $file, $line) {
    throw new ErrorException($message, 0, $severity, $file, $line);
});
require __DIR__ . '/reference_functions.php';
$root = $argv[1] ?? throw new RuntimeException('missing reference root');
$input = json_decode(stream_get_contents(STDIN), true, 512, JSON_THROW_ON_ERROR);
foreach (['valid_unicode', 'wp_kses_normalize_entities', 'wp_kses_normalize_entities2',
          'wp_kses_normalize_entities3', 'wp_kses_named_entities', 'wp_kses_xml_named_entities'] as $symbol) {
    load_reference_function($root . '/wp-includes/kses.php', $symbol);
}
$charset = ini_get('default_charset');
$had_html = array_key_exists('allowedentitynames', $GLOBALS);
$saved_html = $GLOBALS['allowedentitynames'] ?? null;
$had_xml = array_key_exists('allowedxmlentitynames', $GLOBALS);
$saved_xml = $GLOBALS['allowedxmlentitynames'] ?? null;
try {
    if (ini_set('default_charset', 'UTF-8') === false || ini_get('default_charset') !== 'UTF-8') {
        throw new RuntimeException('UTF-8 setup failed');
    }
    $GLOBALS['allowedentitynames'] = $input['html_names'];
    $GLOBALS['allowedxmlentitynames'] = $input['xml_names'];
    $pairs = [];
    foreach ($input['html_names'] as $name) {
        if (preg_match('//', '') !== 1) { throw new RuntimeException('PCRE reset failed'); }
        $output = wp_kses_normalize_entities('&' . $name . ';', 'xml');
        if (!is_string($output) || preg_last_error() !== PREG_NO_ERROR) {
            throw new RuntimeException('XML observation failed');
        }
        $pairs[] = [$name, bin2hex($output)];
    }
    $result = ['php_version' => PHP_VERSION, 'php_int_size' => PHP_INT_SIZE,
        'charset' => ini_get('default_charset'), 'html5_flag' => ENT_HTML5, 'pairs' => $pairs];
} finally {
    if ($had_html) { $GLOBALS['allowedentitynames'] = $saved_html; } else { unset($GLOBALS['allowedentitynames']); }
    if ($had_xml) { $GLOBALS['allowedxmlentitynames'] = $saved_xml; } else { unset($GLOBALS['allowedxmlentitynames']); }
    if (ini_set('default_charset', $charset) === false || ini_get('default_charset') !== $charset) {
        throw new RuntimeException('charset restore failed');
    }
}
echo json_encode($result, JSON_THROW_ON_ERROR), "\n";
