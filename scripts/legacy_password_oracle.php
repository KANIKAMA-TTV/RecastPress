<?php
/** Fixed public-dummy legacy subset. Host/domain refusals never execute costly PHP. */
error_reporting(E_ALL);
ini_set('display_errors', 'stderr'); ini_set('log_errors', '0');
ini_set('zend.exception_ignore_args', '1');
if (ini_get('zend.exception_ignore_args') !== '1') { throw new RuntimeException('diagnostic argument suppression failed'); }
set_error_handler(static function ($level, $message, $file, $line) {
    throw new ErrorException('legacy password oracle diagnostic', 0, $level, $file, $line);
});
$root = $argv[1] ?? throw new RuntimeException('missing pinned root');
define('ABSPATH', $root . '/'); define('WPINC', 'wp-includes');
require $root . '/wp-includes/plugin.php';
require __DIR__ . '/reference_functions.php';
load_reference_function($root . '/wp-includes/pluggable.php', 'wp_check_password');
function exact_keys($value, $expected) {
    if (!is_array($value)) { throw new RuntimeException('object required'); }
    $actual = array_keys($value); sort($actual); sort($expected);
    if ($actual !== $expected) { throw new RuntimeException('unknown or missing field'); }
}
function unhex($text) {
    if (!is_string($text) || strlen($text) > 16384 || strlen($text) % 2 !== 0
        || preg_match('/\A[0-9a-f]*\z/D', $text) !== 1) { throw new RuntimeException('invalid byte wire'); }
    return hex2bin($text);
}
function refusal($mode, $password, $hash, $maximum, $bcrypt_maximum = null) {
    if ($mode === 'combined') {
        if (strlen($hash) <= 32 || strlen($password) > 4096) { return null; }
        if (str_starts_with($hash, '$wp')) {
            if (strlen($hash) !== 63 || !str_starts_with($hash, '$wp$2y$') || $hash[9] !== '$') { return 'unsupported_hash'; }
            $ascii = preg_match('/\A[\x00-\x7f]*\z/D', $hash);
            if ($ascii === false || preg_last_error() !== PREG_NO_ERROR) { throw new RuntimeException('ASCII guard failed'); }
            if ($ascii !== 1) { return 'unsupported_hash'; }
            $cost_wire = substr($hash, 7, 2);
            if (strspn($cost_wire, '0123456789') !== 2) { return 'unsupported_hash'; }
            $cost = (int)$cost_wire;
            if ($cost < 4 || $cost > 31) { return 'unsupported_hash'; }
            if ($cost > $bcrypt_maximum) { return 'bcrypt_cost_limit'; }
            $alphabet = './ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789';
            if (strspn(substr($hash, 10), $alphabet) !== 53
                || strpos($alphabet, $hash[31]) % 16 !== 0 || strpos($alphabet, $hash[62]) % 4 !== 0) {
                return 'unsupported_hash';
            }
            return null;
        }
        if (!str_starts_with($hash, '$P$')) { return 'unsupported_hash'; }
        $error = refusal('phpass', $password, $hash, $maximum);
        return $error === 'cost_limit' ? 'portable_cost_limit' : $error;
    }
    if ($mode === 'md5') { return strlen($hash) <= 32 ? null : 'unsupported_hash'; }
    if (strlen($hash) !== 34 || !str_starts_with($hash, '$P$')) { return 'unsupported_hash'; }
    if (strlen($password) > 4096) { return null; }
    $alphabet = './0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz';
    $cost = strpos($alphabet, $hash[3]);
    if ($cost === false || $cost < 7 || $cost > 30) { return 'unsupported_hash'; }
    if ($cost > $maximum) { return 'cost_limit'; }
    if (strspn(substr($hash, 4), $alphabet) !== 30 || !str_contains('./01', $hash[33])) {
        return 'unsupported_hash';
    }
    return null;
}
$wire = stream_get_contents(STDIN, 1048577);
if (strlen($wire) > 1048576) { throw new RuntimeException('input bound'); }
$request = json_decode($wire, true, 32, JSON_THROW_ON_ERROR);
if ($wire !== json_encode($request, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES)) {
    throw new RuntimeException('noncanonical wire');
}
exact_keys($request, ['items']);
if (!is_array($request['items']) || !array_is_list($request['items']) || count($request['items']) > 128) {
    throw new RuntimeException('batch bound');
}
$results = []; $states = [];
foreach ($request['items'] as $item) {
    if (!is_array($item) || !isset($item['mode']) || !is_string($item['mode'])
        || !in_array($item['mode'], ['md5', 'phpass', 'combined'], true)) { throw new RuntimeException('invalid mode'); }
    $mode = $item['mode'];
    exact_keys($item, match ($mode) {
        'md5' => ['mode', 'password_hex', 'hash_hex'],
        'phpass' => ['mode', 'password_hex', 'hash_hex', 'maximum'],
        'combined' => ['mode', 'password_hex', 'hash_hex', 'bcrypt_maximum', 'portable_maximum'],
    });
    $password = unhex($item['password_hex']); $hash = unhex($item['hash_hex']);
    $maximum = $mode === 'combined' ? $item['portable_maximum'] : ($item['maximum'] ?? null);
    $bcrypt_maximum = $item['bcrypt_maximum'] ?? null;
    if ($mode === 'phpass' && (!is_int($maximum) || $maximum < 7 || $maximum > 30)) {
        throw new RuntimeException('invalid host cost');
    }
    if ($mode === 'combined' && (!is_int($maximum) || $maximum < 7 || $maximum > 30
        || !is_int($bcrypt_maximum) || $bcrypt_maximum < 4 || $bcrypt_maximum > 31)) {
        throw new RuntimeException('invalid combined host costs');
    }
    $error = refusal($mode, $password, $hash, $maximum, $bcrypt_maximum);
    if ($error !== null) {
        $results[] = ['error' => $error, 'actual_called' => false, 'check_calls' => 0];
        continue;
    }
    $state = ['absent', 'null', 'sentinel', 'empty'][count($results) % 4]; $states[$state] = true;
    foreach (['wp_filter', 'wp_filters', 'wp_current_filter', 'wp_hasher'] as $name) {
        if ($state === 'absent') { unset($GLOBALS[$name]); }
        else { $GLOBALS[$name] = match ($state) { 'null' => null, 'sentinel' => ['sentinel' => new stdClass()], default => [] }; }
    }
    $saved = [];
    foreach (['wp_filter', 'wp_filters', 'wp_current_filter', 'wp_hasher'] as $name) {
        $saved[$name] = [array_key_exists($name, $GLOBALS), $GLOBALS[$name] ?? null];
    }
    $actions = [array_key_exists('wp_actions', $GLOBALS), $GLOBALS['wp_actions'] ?? null];
    try {
        $GLOBALS['wp_filter'] = []; $GLOBALS['wp_filters'] = [];
        $GLOBALS['wp_current_filter'] = []; $GLOBALS['wp_hasher'] = null;
        $verified = wp_check_password($password, $hash, 0);
        if (!is_bool($verified) || $GLOBALS['wp_current_filter'] !== []) { throw new RuntimeException('invalid actual result/context'); }
        $result = ['verified' => $verified, 'actual_called' => true, 'check_calls' => did_filter('check_password')];
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
    if ([array_key_exists('wp_actions', $GLOBALS), $GLOBALS['wp_actions'] ?? null] !== $actions) {
        throw new RuntimeException('actions changed');
    }
    $results[] = $result;
}
$output = json_encode(['process_pid' => getmypid(), 'php_version' => PHP_VERSION,
    'globals_restored' => true, 'actions_unchanged' => true, 'restoration_states' => array_keys($states),
    'results' => $results], JSON_THROW_ON_ERROR);
if (strlen($output) > 65536) { throw new RuntimeException('output bound'); }
echo $output, "\n";
