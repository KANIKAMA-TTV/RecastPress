<?php
/** Actual pinned WP calls, public dummy values only; not an authentication API. */
error_reporting(E_ALL);
ini_set('display_errors', 'stderr');
ini_set('log_errors', '0');
set_error_handler(static function ($level, $message, $file, $line) {
    throw new ErrorException('password generation oracle diagnostic', 0, $level, $file, $line);
});
$root = $argv[1] ?? throw new RuntimeException('missing reference root');
require $root . '/wp-includes/plugin.php';
require __DIR__ . '/reference_functions.php';
load_reference_function($root . '/wp-includes/pluggable.php', 'wp_hash_password');
load_reference_function($root . '/wp-includes/pluggable.php', 'wp_check_password');
function exact_keys($value, $keys) {
    if (!is_array($value)) { throw new RuntimeException('invalid wire object'); }
    $actual = array_keys($value); sort($actual); sort($keys);
    if ($actual !== $keys) { throw new RuntimeException('invalid wire fields'); }
}
function bytes_from_hex($value) {
    if (!is_string($value) || strlen($value) > 8194 || strlen($value) % 2 !== 0
        || preg_match('/\A[0-9a-f]*\z/D', $value) !== 1) {
        throw new RuntimeException('invalid byte wire');
    }
    return hex2bin($value);
}
$wire = stream_get_contents(STDIN, 65537);
if (strlen($wire) > 65536) { throw new RuntimeException('input bound'); }
$request = json_decode($wire, true, 32, JSON_THROW_ON_ERROR);
// This probe accepts the harness's unique-key compact ASCII wire only.
// Duplicate keys cannot survive this byte-for-byte round trip.
if ($wire !== json_encode($request, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES)) {
    throw new RuntimeException('noncanonical wire');
}
exact_keys($request, ['operation', 'items']);
if (!is_string($request['operation']) || !is_array($request['items'])
    || !array_is_list($request['items']) || count($request['items']) > 64) {
    throw new RuntimeException('invalid batch');
}
$results = [];
$states = [];
foreach ($request['items'] as $item) {
    exact_keys($item, $request['operation'] === 'generate' ? ['password_hex'] : ['password_hex', 'hash_hex']);
    $password = bytes_from_hex($item['password_hex']);
    $hash = $request['operation'] === 'verify' ? bytes_from_hex($item['hash_hex']) : null;
    $state = ['absent', 'null', 'sentinel', 'empty'][count($results) % 4];
    $states[$state] = true;
    foreach (['wp_filter', 'wp_filters', 'wp_current_filter', 'wp_hasher'] as $name) {
        if ($state === 'absent') { unset($GLOBALS[$name]); }
        else { $GLOBALS[$name] = match ($state) { 'null' => null, 'sentinel' => ['sentinel' => new stdClass()], default => [] }; }
    }
    $saved = [];
    foreach (['wp_filter', 'wp_filters', 'wp_current_filter', 'wp_hasher'] as $name) {
        $saved[$name] = [array_key_exists($name, $GLOBALS), $GLOBALS[$name] ?? null];
    }
    $actions_before = [array_key_exists('wp_actions', $GLOBALS), $GLOBALS['wp_actions'] ?? null];
    try {
        $GLOBALS['wp_filter'] = []; $GLOBALS['wp_filters'] = [];
        $GLOBALS['wp_current_filter'] = []; $GLOBALS['wp_hasher'] = null;
        if ($request['operation'] === 'generate') {
            $value = wp_hash_password($password);
            if (!is_string($value)) { throw new RuntimeException('nonbyte result'); }
            $result = ['hash_hex' => bin2hex($value), 'algorithm_calls' => did_filter('wp_hash_password_algorithm'),
                'options_calls' => did_filter('wp_hash_password_options')];
        } elseif ($request['operation'] === 'verify') {
            $value = wp_check_password($password, $hash, 0);
            if (!is_bool($value)) { throw new RuntimeException('nonbool result'); }
            $result = ['verified' => $value, 'check_calls' => did_filter('check_password')];
        } else { throw new RuntimeException('unknown operation'); }
        if ($GLOBALS['wp_current_filter'] !== []) { throw new RuntimeException('filter context leaked'); }
    } finally {
        foreach ($saved as $name => [$exists, $old]) {
            if ($exists) { $GLOBALS[$name] = $old; } else { unset($GLOBALS[$name]); }
        }
    }
    foreach ($saved as $name => [$exists, $old]) {
        if (array_key_exists($name, $GLOBALS) !== $exists || ($exists && $GLOBALS[$name] !== $old)) {
            throw new RuntimeException('global restoration failed');
        }
    }
    if ([array_key_exists('wp_actions', $GLOBALS), $GLOBALS['wp_actions'] ?? null] !== $actions_before) {
        throw new RuntimeException('actions changed');
    }
    $results[] = $result;
}
$output = json_encode(['process_pid' => getmypid(), 'php_version' => PHP_VERSION,
    'globals_restored' => true, 'actions_unchanged' => true,
    'restoration_states' => array_keys($states), 'results' => $results], JSON_THROW_ON_ERROR);
if (strlen($output) > 65536) { throw new RuntimeException('output bound'); }
echo $output, "\n";
