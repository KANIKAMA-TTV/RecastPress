<?php
declare(strict_types=1);

// Closed CGI contract: only the host's three fixed requests select a branch.
// Errors remain errors; this fixture provides no CLI or native fallback.
if (PHP_SAPI !== 'cgi-fcgi') {
    throw new RuntimeException('The contract requires the CGI SAPI');
}

$case = $_GET['case'] ?? null;
if ($case === 'get' || $case === 'post') {
    header('Content-Type: application/json');
    $body = json_encode([
        'runtime' => ['version' => PHP_VERSION, 'version_id' => PHP_VERSION_ID,
                      'int_size' => PHP_INT_SIZE, 'sapi' => PHP_SAPI],
        'method' => $_SERVER['REQUEST_METHOD'] ?? null,
        'query_string' => $_SERVER['QUERY_STRING'] ?? null,
        'script_filename' => $_SERVER['SCRIPT_FILENAME'] ?? null,
        'request_uri' => $_SERVER['REQUEST_URI'] ?? null,
        'get' => $_GET,
        'post' => $_POST,
        'cookie' => $_COOKIE,
        'raw_body_hex' => bin2hex(file_get_contents('php://input')),
    ], JSON_THROW_ON_ERROR);
    header('Content-Length: ' . strlen($body));
    echo $body;
} elseif ($case === 'metadata') {
    http_response_code(404);
    header('Content-Type: application/octet-stream');
    if (!setcookie('wp_a', 'one', ['path' => '/', 'httponly' => true]) ||
        !setcookie('wp_b', 'two', ['path' => '/', 'samesite' => 'Lax'])) {
        throw new RuntimeException('Cookie emission failed');
    }
    $body = "\x00\xffCGI\r\n";
    header('Content-Length: ' . strlen($body));
    echo $body;
} else {
    throw new RuntimeException('Unknown fixed contract request');
}
