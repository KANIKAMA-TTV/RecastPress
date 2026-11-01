<?php
declare(strict_types=1);

// Fixed CLI observations for native PHP versus a Rust-hosted Wasm candidate.
// The host supplies verified WP source and a disposable scratch capability.
if (PHP_SAPI !== 'cgi-fcgi'
    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'
    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_runtime_cgi=1'
    || ($_SERVER['argc'] ?? 0) !== 0) {
    echo "{\"format_version\":1,\"error\":\"this fixture accepts no arguments\"}\n";
    exit(2);
}

function rp_contract_message(string $message): string {
    $message = str_replace('\\', '/', $message);
    foreach (['RP_WP_ROOT' => '/wp-core', 'RP_SCRATCH' => '/scratch'] as $key => $replacement) {
        $path = getenv($key);
        if (is_string($path) && $path !== '') {
            $message = str_replace(str_replace('\\', '/', $path), $replacement, $message);
        }
    }
    return str_replace(str_replace('\\', '/', __FILE__), '/fixture/runtime_contract.php', $message);
}

function rp_contract_observe(callable $operation): array {
    $warnings = [];
    set_error_handler(static function (int $severity, string $message) use (&$warnings): bool {
        $warnings[] = ['severity' => $severity, 'message' => rp_contract_message($message)];
        return true;
    });
    try {
        return ['status' => 'ok', 'value' => $operation(), 'warnings' => &$warnings];
    } catch (Throwable $error) {
        return ['status' => 'error', 'error' => [
            'class' => get_class($error), 'message' => rp_contract_message($error->getMessage()),
        ], 'warnings' => &$warnings];
    } finally {
        restore_error_handler();
    }
}

function rp_contract_directory(string $key): string {
    $path = getenv($key);
    if (!is_string($path) || $path === '' || !is_dir($path)) {
        throw new RuntimeException('missing fixture directory: ' . $key);
    }
    return rtrim($path, '/\\');
}

ob_start();
$cases = [];
$cases['basic'] = rp_contract_observe(static function (): array {
    $binary = "\0\xffRP\r\n日本語";
    $encoded = json_encode(['text' => '日本語', 'items' => [false, null, 7],
        'binary_base64' => base64_encode($binary)], JSON_THROW_ON_ERROR);
    $wide = 2147483647 + 2;
    return [
        'arithmetic' => 2 + 3 * 5,
        'binary_hex' => bin2hex($binary), 'binary_bytes' => strlen($binary),
        'unicode_bytes' => strlen('日本語'), 'substring' => substr('日本語', 0, 3),
        'json_roundtrip' => json_decode($encoded, true, 32, JSON_THROW_ON_ERROR),
        'wide_integer' => $wide, 'wide_integer_type' => get_debug_type($wide),
        'max_integer_decimal' => (string) PHP_INT_MAX,
    ];
});
$cases['exceptions'] = rp_contract_observe(static function (): array {
    $trace = [];
    $previousClass = null;
    $previousMessage = null;
    try {
        $trace[] = 'outer_try';
        try {
            $trace[] = 'inner_try';
            throw new RuntimeException('inner failure');
        } catch (RuntimeException $error) {
            $trace[] = 'inner_catch';
            throw new LogicException('outer failure', 0, $error);
        } finally {
            $trace[] = 'inner_finally';
        }
    } catch (LogicException $error) {
        $trace[] = 'outer_catch';
        $previousClass = get_class($error->getPrevious());
        $previousMessage = $error->getPrevious()->getMessage();
    } finally {
        $trace[] = 'outer_finally';
    }
    return ['trace' => $trace, 'previous_class' => $previousClass, 'previous_message' => $previousMessage];
});
$cases['fiber'] = rp_contract_observe(static function (): array {
    $trace = [];
    $fiber = new Fiber(static function (string $input) use (&$trace): string {
        $trace[] = 'start:' . $input;
        $resumed = Fiber::suspend('paused:' . $input);
        $trace[] = 'resume:' . $resumed;
        return 'returned:' . $resumed;
    });
    $started = $fiber->start('seed');
    $suspended = $fiber->isSuspended();
    $premature = null;
    try {
        $fiber->getReturn();
    } catch (Throwable $error) {
        $premature = get_class($error);
    }
    $resumed = $fiber->resume('next');
    return ['start_value' => $started, 'suspended' => $suspended,
        'premature_return_error' => $premature, 'resume_value' => $resumed,
        'terminated' => $fiber->isTerminated(), 'return_value' => $fiber->getReturn(), 'trace' => $trace];
});
$cases['wordpress_hooks'] = rp_contract_observe(static function (): array {
    $root = rp_contract_directory('RP_WP_ROOT');
    require $root . '/plugin.php';
    $empty = has_filter('rp_contract_filter') === false && did_action('rp_contract_action') === 0;
    add_filter('rp_contract_filter', static fn ($value, $suffix) => $value . $suffix, 10, 2);
    add_filter('rp_contract_filter', static fn ($value) => $value . '|theme', 20);
    $filtered = apply_filters('rp_contract_filter', 'post', '|plugin');
    $object = (object) ['counter' => 0];
    add_action('rp_contract_action', static function ($shared): void { $shared->counter++; });
    do_action('rp_contract_action', $object);
    add_filter('rp_contract_nullable', static fn ($value) => null);
    $null = apply_filters('rp_contract_nullable', new WP_Filter_Sentinel());
    return ['initially_empty' => $empty, 'filtered' => $filtered,
        'action_counter' => $object->counter, 'action_calls' => did_action('rp_contract_action'),
        'null_result' => $null === null, 'current_filter_after' => current_filter()];
});
$cases['scratch_io'] = rp_contract_observe(static function (): array {
    $path = rp_contract_directory('RP_SCRATCH') . '/ordinary.txt';
    $bytes = "ordinary\0\xff\r\n";
    $written = file_put_contents($path, $bytes);
    clearstatcache(true, $path);
    $read = file_get_contents($path);
    return ['write_result' => $written, 'read_hex' => bin2hex($read),
        'readable' => is_readable($path), 'writable' => is_writable($path)];
});
$cases['readonly_io'] = rp_contract_observe(static function (): array {
    $path = rp_contract_directory('RP_SCRATCH') . '/readonly.txt';
    clearstatcache(true, $path);
    $readable = is_readable($path);
    $writable = is_writable($path);
    $before = file_get_contents($path);
    $attempt = file_put_contents($path, 'tampered');
    clearstatcache(true, $path);
    $after = file_get_contents($path);
    return ['readable' => $readable, 'writable' => $writable, 'write_result' => $attempt,
        'before_hex' => bin2hex($before), 'after_hex' => bin2hex($after), 'unchanged' => $before === $after];
});
$unexpected = ob_get_clean();
echo json_encode([
    'format_version' => 1,
    'runtime' => ['version' => PHP_VERSION, 'version_id' => PHP_VERSION_ID, 'int_size' => PHP_INT_SIZE, 'sapi' => PHP_SAPI],
    'cases' => $cases, 'unexpected_output_hex' => bin2hex($unexpected),
], JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES | JSON_UNESCAPED_UNICODE), "\n";
