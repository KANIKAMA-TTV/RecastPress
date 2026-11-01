<?php
// Closed CGI entry and stderr alias; the original memory programs are unchanged.
if (PHP_SAPI !== 'cgi-fcgi'
    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'
    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_memory_cgi=1'
    || ($_SERVER['argc'] ?? 0) !== 0) {
    throw new RuntimeException('Fixed memory CGI request required');
}
$rp_case = getenv('RP_MEMORY_CASE');
if ($rp_case !== 'normal' && $rp_case !== 'allocate') {
    throw new RuntimeException('Fixed memory case required');
}
if (defined('STDERR')) {
    throw new RuntimeException('CGI stderr alias must be explicit');
}
$rp_stderr = fopen('php://stderr', 'wb');
if ($rp_stderr === false || !define('STDERR', $rp_stderr)) {
    throw new RuntimeException('CGI stderr alias failed');
}
header('Content-Type: application/json');
if ($rp_case === 'normal') {
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
'kind'=>'normal','memory_limit'=>ini_get('memory_limit'),'allocated_bytes'=>0,
'first_byte'=>null,'last_byte'=>null,'echo'=>'recastpress-v0.5-php-memory-cap'], JSON_THROW_ON_ERROR);
} else {
fwrite(STDERR, 'RP_PHP_MEMORY_ALLOCATION_READY memory_limit='.ini_get('memory_limit')."\n");
$payload = str_repeat('x', 128 * 1024 * 1024);
echo json_encode(['php_version'=>PHP_VERSION,'php_int_size'=>PHP_INT_SIZE,
'kind'=>'allocation','memory_limit'=>ini_get('memory_limit'),'allocated_bytes'=>strlen($payload),
'first_byte'=>ord($payload[0]),'last_byte'=>ord($payload[strlen($payload)-1]),
'echo'=>'recastpress-v0.5-php-memory-cap'], JSON_THROW_ON_ERROR);
}
