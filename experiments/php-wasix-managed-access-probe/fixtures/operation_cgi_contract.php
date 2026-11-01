<?php
// Closed CGI entry; original operation-budget program bodies are unchanged.
if (PHP_SAPI !== 'cgi-fcgi'
    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'
    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_operation_cgi=1'
    || ($_SERVER['argc'] ?? 0) !== 0) {
    throw new RuntimeException('Fixed operation-budget CGI request required');
}
$rp_case = getenv('RP_OPERATION_CASE');
if ($rp_case !== 'normal' && $rp_case !== 'loop') {
    throw new RuntimeException('Fixed operation-budget case required');
}
if ($rp_case === 'normal') {
header('Content-Type: application/json');
$before = array_key_exists('RP_OPERATION_STATE', $GLOBALS);
$GLOBALS['RP_OPERATION_STATE'] = 'request-local';
echo json_encode(['php_version'=>PHP_VERSION,'version_id'=>PHP_VERSION_ID,
'php_int_size'=>PHP_INT_SIZE,'sapi'=>PHP_SAPI,'kind'=>'normal',
'globals_before_present'=>$before,'globals_after'=>$GLOBALS['RP_OPERATION_STATE'],
'echo'=>'recastpress-v0.5-php-operation-budget'], JSON_THROW_ON_ERROR);
} else {
$rp_stdout = fopen('php://stdout', 'wb');
if ($rp_stdout === false || !define('STDOUT', $rp_stdout)) {
    throw new RuntimeException('Fixed CGI STDOUT alias required');
}
fwrite(STDOUT, "RP_PHP_OPERATION_BUDGET_READY\n");
fflush(STDOUT);
while (true) {}
}
