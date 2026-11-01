<?php
// Deliberately public, fixed inputs. Not a WordPress adapter or a plugin API.
$mode = getenv('RP_PROBE_MODE');
header('Content-Type: application/json');
function rp_rpc($command) {
    echo 'RP_P0 ' . $command . "\n";
    flush();
    clearstatcache(true, '/rp-p0-rpc/reply.json');
    return json_decode(file_get_contents('/rp-p0-rpc/reply.json'), true, 32, JSON_THROW_ON_ERROR);
}
if ($mode === 'fuel' || $mode === 'epoch') { while (true) {} }
if ($mode === 'fatal') {
    rp_rpc('CACHE_SET 7');
    echo 'partial response';
    rp_deliberately_missing_function();
}
if ($mode === 'output') { echo str_repeat('x', 1024 * 1024); exit; }
if ($mode === 'memory') { echo str_repeat('x', 512 * 1024 * 1024); exit; }
if ($mode === 'bad_rpc') { rp_rpc('UNKNOWN'); exit; }
if ($mode !== 'basic') { exit(2); }

$included = require __DIR__ . '/include.php';
$input = file_get_contents('php://input');
$before = rp_rpc('CACHE_GET');
rp_rpc('CACHE_SET 7');
$after = rp_rpc('CACHE_GET');
$db = rp_rpc('DB_SELECT_FIXTURE');
$GLOBALS['rp_counter'] = ($GLOBALS['rp_counter'] ?? 0) + 1;
$shared = (object) ['counter' => 0];
rp_shared_hook($shared);
rp_shared_hook($shared);
echo json_encode([
    'version' => PHP_VERSION,
    'include' => $included,
    'input_hex' => bin2hex($input),
    'cookie' => $_COOKIE['rp_fixture'] ?? null,
    'counter' => $GLOBALS['rp_counter'],
    'shared_counter' => $shared->counter,
    'cache_before' => $before['value'],
    'cache_after' => $after['value'],
    'db_nonce' => $db['rows'][0]['nonce'],
    'db_rows' => count($db['rows']),
    'outside_denied' => @file_get_contents('/site/../outside.txt') === false,
    'write_denied' => @file_put_contents(__DIR__ . '/blocked.txt', 'x') === false,
    'env_absent' => getenv('PATH') === false,
], JSON_THROW_ON_ERROR);
