<?php
// Deliberately public, fixed inputs. Not a WordPress adapter or a plugin API.
$mode = getenv('RP_PROBE_MODE');
header('Content-Type: application/json');
function rp_rpc($command) {
    static $channel = null;
    $channel ??= fopen('/rp-p0-rpc/channel', 'r+b');
    $wire = 'RP_P0 ' . $command . "\n";
    rewind($channel);
    if (fwrite($channel, $wire) !== strlen($wire)) { throw new RuntimeException('short RPC write'); }
    rewind($channel);
    return json_decode(fgets($channel, 1025), true, 32, JSON_THROW_ON_ERROR);
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
if ($mode === 'http_partial') {
    rp_rpc('CACHE_SET 7');
    echo 'partial response';
    flush();
    while (true) {}
}
if ($mode === 'literal') { echo "RP_P0 CACHE_SET 7\n\0\xff"; exit; }
if ($mode === 'response_metadata') {
    http_response_code(201);
    header('Content-Type: application/octet-stream');
    header('Content-Length: 8');
    header('Set-Cookie: first=1; Path=/; HttpOnly', false);
    header('Set-Cookie: second=2; Path=/; SameSite=Lax', false);
    echo "ok\0\xffbody";
    exit;
}
if ($mode === 'redirect_metadata') { header('Location: /next?lang=ja', true, 307); exit; }
if ($mode === 'no_content') { http_response_code(204); exit; }
if ($mode === 'not_modified') { http_response_code(304); header('Content-Length: 1234'); exit; }
if ($mode === 'cgi_wrong_length') { header('Content-Length: 999'); echo 'short'; exit; }
if ($mode === 'cgi_hop_header') { header('Transfer-Encoding: chunked'); echo 'raw'; exit; }
if ($mode === 'cgi_header_limit') { header('X-Too-Large: ' . str_repeat('x', 17000)); echo 'body'; exit; }
if ($mode === 'rpc_oversize' || $mode === 'rpc_incomplete') {
    $channel = fopen('/rp-p0-rpc/channel', 'r+b');
    fwrite($channel, $mode === 'rpc_oversize' ? str_repeat('x', 129) : 'RP_P0 CACHE_');
    fflush($channel);
    echo '{"partial":true}';
    exit;
}
if ($mode === 'fragmented_rpc' || $mode === 'reopen_rpc') {
    $channel = fopen('/rp-p0-rpc/channel', 'r+b');
    fwrite($channel, 'RP_P0 CACHE_');
    fflush($channel);
    fwrite($channel, "SET 7\n");
    rewind($channel);
    $set = json_decode(fgets($channel, 1025), true, 32, JSON_THROW_ON_ERROR);
    if ($mode === 'reopen_rpc') {
        fclose($channel);
        $channel = fopen('/rp-p0-rpc/channel', 'r+b');
    }
    rewind($channel);
    fwrite($channel, "RP_P0 CACHE_GET\n");
    rewind($channel);
    $get = json_decode(fgets($channel, 1025), true, 32, JSON_THROW_ON_ERROR);
    echo json_encode(['set' => $set['ok'], 'value' => $get['value']], JSON_THROW_ON_ERROR);
    exit;
}
if ($mode !== 'basic') { exit(2); }

$included = require __DIR__ . '/include.php';
require '/wp-core/plugin.php';
$wpInitiallyEmpty = has_filter('rp_wasi_filter') === false && did_action('rp_wasi_action') === 0;
add_filter('rp_wasi_filter', static fn ($value, $suffix) => $value . $suffix, 10, 2);
add_filter('rp_wasi_filter', static fn ($value) => $value . '|theme', 20);
$wpFilter = apply_filters('rp_wasi_filter', 'post', '|plugin');
$wpObject = (object) ['counter' => 0];
add_action('rp_wasi_action', static function ($object) { $object->counter++; });
do_action('rp_wasi_action', $wpObject);
add_filter('rp_wasi_nullable', static fn ($value) => null);
$wpNull = apply_filters('rp_wasi_nullable', new WP_Filter_Sentinel()) === null;
$input = file_get_contents('php://input');
ob_start();
$before = rp_rpc('CACHE_GET');
rp_rpc('CACHE_SET 7');
$after = rp_rpc('CACHE_GET');
$db = rp_rpc('DB_SELECT_FIXTURE');
$rpcOutput = ob_get_clean();
$GLOBALS['rp_counter'] = ($GLOBALS['rp_counter'] ?? 0) + 1;
$shared = (object) ['counter' => 0];
rp_shared_hook($shared);
rp_shared_hook($shared);
echo json_encode([
    'version' => PHP_VERSION,
    'include' => $included,
    'input_hex' => bin2hex($input),
    'request_method' => $_SERVER['REQUEST_METHOD'] ?? null,
    'request_uri' => $_SERVER['REQUEST_URI'] ?? null,
    'query_string' => $_SERVER['QUERY_STRING'] ?? null,
    'cookie' => $_COOKIE['rp_fixture'] ?? null,
    'counter' => $GLOBALS['rp_counter'],
    'shared_counter' => $shared->counter,
    'cache_before' => $before['value'],
    'cache_after' => $after['value'],
    'rpc_output_empty' => $rpcOutput === '',
    'db_nonce' => $db['rows'][0]['nonce'],
    'db_rows' => count($db['rows']),
    'outside_denied' => @file_get_contents('/site/../outside.txt') === false,
    'write_denied' => @file_put_contents(__DIR__ . '/blocked.txt', 'x') === false,
    'env_absent' => getenv('PATH') === false,
    'wp_hooks_initially_empty' => $wpInitiallyEmpty,
    'wp_filter' => $wpFilter,
    'wp_action_counter' => $wpObject->counter,
    'wp_action_calls' => did_action('rp_wasi_action'),
    'wp_null_result' => $wpNull,
], JSON_THROW_ON_ERROR);
