<?php
// Closed CGI entry; the two original RPC program bodies below are unchanged.
if (PHP_SAPI !== 'cgi-fcgi'
    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'
    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_rpc_cgi=1'
    || ($_SERVER['argc'] ?? 0) !== 0) {
    throw new RuntimeException('Fixed RPC CGI request required');
}
$rp_case = getenv('RP_RPC_CASE');
if ($rp_case !== 'normal' && $rp_case !== 'incomplete_ignored') {
    throw new RuntimeException('Fixed RPC case required');
}
header('Content-Type: application/octet-stream');
if ($rp_case === 'normal') {
function rp_rpc($channel, $command) {
    if (!rewind($channel)) { throw new RuntimeException('RPC rewind before write failed'); }
    $wire = 'RP_P0 ' . $command . "\n";
    $first = substr($wire, 0, 9);
    $second = substr($wire, 9);
    if (fwrite($channel, $first) !== strlen($first) || !fflush($channel) ||
        fwrite($channel, $second) !== strlen($second) || !fflush($channel)) {
        throw new RuntimeException('RPC fragmented write failed');
    }
    if (!rewind($channel)) { throw new RuntimeException('RPC rewind before read failed'); }
    $reply = fgets($channel, 1025);
    if ($reply === false || !str_ends_with($reply, "\n")) { throw new RuntimeException('RPC reply missing'); }
    return json_decode($reply, true, 32, JSON_THROW_ON_ERROR);
}
$before = array_key_exists('RP_RPC_STATE', $GLOBALS);
$marker = getenv('RP_RPC_REQUEST_MARKER');
$GLOBALS['RP_RPC_STATE'] = $marker;
$channel = fopen('/rp-p0-rpc/channel', 'r+b');
if ($channel === false) { throw new RuntimeException('RPC open failed'); }
$miss = rp_rpc($channel, 'CACHE_GET');
$set = rp_rpc($channel, 'CACHE_SET 7');
$hit = rp_rpc($channel, 'CACHE_GET');
$db = rp_rpc($channel, 'DB_SELECT_FIXTURE');
if (!fclose($channel)) { throw new RuntimeException('RPC close failed'); }
$channel = fopen('/rp-p0-rpc/channel', 'r+b');
if ($channel === false) { throw new RuntimeException('RPC reopen failed'); }
$reopened = rp_rpc($channel, 'CACHE_GET');
if (!fclose($channel)) { throw new RuntimeException('RPC second close failed'); }
echo "RP_P0 CACHE_SET 7\n\x00\xff";
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
    'kind'=>'normal','marker'=>$marker,'globals_before_present'=>$before,
    'globals_after'=>$GLOBALS['RP_RPC_STATE'],'cache_before'=>$miss,'cache_set'=>$set,
    'cache_after'=>$hit,'database'=>$db,'reopened_cache'=>$reopened,'channel_closed'=>true,
    'echo'=>'recastpress-v0.5-private-rpc'], JSON_THROW_ON_ERROR);
} else {
$channel = fopen('/rp-p0-rpc/channel', 'r+b');
if ($channel === false) { throw new RuntimeException('RPC open failed'); }
$wire = 'RP_P0 CACHE_';
$written = fwrite($channel, $wire);
$flushed = fflush($channel);
$closed = fclose($channel);
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
    'kind'=>'incomplete_ignored','marker'=>getenv('RP_RPC_REQUEST_MARKER'),
    'wire_hex'=>bin2hex($wire),'written'=>$written,'flushed'=>$flushed,'channel_closed'=>$closed,
    'echo'=>'recastpress-v0.5-private-rpc'], JSON_THROW_ON_ERROR);
}
