<?php
declare(strict_types=1);

// Closed native/Wasm comparison input. This file accepts no code, path or case arguments.
if (PHP_SAPI !== 'cgi-fcgi'
    || ($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET'
    || ($_SERVER['QUERY_STRING'] ?? '') !== 'rp_fiber_cgi=1'
    || ($_SERVER['argc'] ?? 0) !== 0) {
    echo "{\"schema\":\"recastpress.php-fiber-contract.v0.5\",\"ok\":false,\"error\":\"arguments forbidden\"}\n";
    exit(2);
}

function rp_fiber_message(string $message): string {
    return str_replace([__FILE__, str_replace('\\', '/', __FILE__)], '/fixture/fiber_contract.php', $message);
}

function rp_fiber_observe(callable $operation): array {
    $warnings = [];
    set_error_handler(static function (int $severity, string $message) use (&$warnings): bool {
        $warnings[] = ['severity' => $severity, 'message' => rp_fiber_message($message)];
        return true;
    });
    try {
        return ['status' => 'ok', 'value' => $operation(), 'warnings' => &$warnings];
    } catch (Throwable $error) {
        return ['status' => 'error', 'error' => ['class' => get_class($error),
            'message' => rp_fiber_message($error->getMessage())], 'warnings' => &$warnings];
    } finally {
        restore_error_handler();
    }
}

function rp_fiber_error(callable $operation): array {
    try {
        return ['threw' => false, 'value' => $operation()];
    } catch (Throwable $error) {
        // Object IDs, source locations and exception stack traces are deliberately absent.
        return ['threw' => true, 'class' => get_class($error), 'message' => rp_fiber_message($error->getMessage())];
    }
}

function rp_fiber_frames(string $owner, int $depth): array {
    $local = ['owner' => $owner, 'depth' => $depth, 'values' => [17, false, null, '日本語']];
    if ($depth > 0) {
        $result = rp_fiber_frames($owner, $depth - 1);
    } else {
        $result = ['owner' => $owner, 'received' => Fiber::suspend('pause:' . $owner), 'frames' => 0];
    }
    if ($local !== ['owner' => $owner, 'depth' => $depth, 'values' => [17, false, null, '日本語']]) {
        throw new LogicException('recursive Fiber local changed');
    }
    ++$result['frames'];
    return $result;
}

ob_start();
$cases = [];
$cases['multi_lifetime'] = rp_fiber_observe(static function (): array {
    $fibers = [];
    $trace = [];
    $starts = [];
    foreach (['a', 'b', 'c', 'd'] as $owner) {
        $fibers[$owner] = new Fiber(static function () use ($owner, &$trace): array {
            $trace[] = $owner . ':start';
            $result = rp_fiber_frames($owner, 3);
            $trace[] = $owner . ':return';
            return $result;
        });
        $starts[$owner] = $fibers[$owner]->start();
    }
    $resumes = ['a' => $fibers['a']->resume(['sequence' => 1, 'text' => 'A'])];
    $aDoneWhileOthersSuspended = $fibers['a']->isTerminated()
        && $fibers['b']->isSuspended() && $fibers['c']->isSuspended() && $fibers['d']->isSuspended();
    $clobber = array_fill(0, 64, str_repeat('main-clobber', 8));
    unset($clobber);
    foreach (['b', 'c', 'd'] as $index => $owner) {
        $resumes[$owner] = $fibers[$owner]->resume(['sequence' => $index + 2, 'text' => strtoupper($owner)]);
    }
    $returns = [];
    foreach ($fibers as $owner => $fiber) {
        $returns[$owner] = $fiber->getReturn();
    }
    return ['fiber_count' => count($fibers), 'starts' => $starts, 'resumes' => $resumes,
        'a_done_while_others_suspended' => $aDoneWhileOthersSuspended, 'returns' => $returns,
        'trace' => $trace, 'main_current_is_null' => Fiber::getCurrent() === null];
});

$cases['nested'] = rp_fiber_observe(static function (): array {
    $trace = [];
    $outer = new Fiber(static function () use (&$trace): array {
        $trace[] = 'outer:start';
        $inner = new Fiber(static function () use (&$trace): string {
            $trace[] = 'inner:start';
            try {
                $value = Fiber::suspend('inner-paused');
                $trace[] = 'inner:resume:' . $value;
                return 'inner-return:' . $value;
            } finally {
                $trace[] = 'inner:finally';
            }
        });
        try {
            $start = $inner->start();
            $trace[] = 'outer:inner-paused';
            $value = Fiber::suspend(['outer' => 'paused', 'inner_start' => $start,
                'inner_suspended' => $inner->isSuspended()]);
            $trace[] = 'outer:resume:' . $value;
            $resumed = $inner->resume('inner-next');
            $trace[] = 'outer:inner-done';
            return ['outer_received' => $value, 'inner_resume' => $resumed, 'inner_return' => $inner->getReturn()];
        } finally {
            $trace[] = 'outer:finally';
        }
    });
    $start = $outer->start();
    $resume = $outer->resume('outer-next');
    return ['start' => $start, 'resume' => $resume, 'return' => $outer->getReturn(),
        'terminated' => $outer->isTerminated(), 'trace' => $trace];
});

$cases['values'] = rp_fiber_observe(static function (): array {
    $states = [];
    $fiber = new Fiber(static function (array $seed, int $number) use (&$states): array {
        $current = Fiber::getCurrent();
        $states = ['current_is_fiber' => $current instanceof Fiber, 'running' => $current->isRunning(),
            'started' => $current->isStarted(), 'suspended' => $current->isSuspended()];
        $reply = Fiber::suspend(['integer' => $number, 'float' => 1.5, 'false' => false, 'null' => null,
            'text' => '日本語', 'binary_hex' => bin2hex("\0\xff")]);
        return ['seed' => $seed, 'reply' => $reply];
    });
    $before = ['started' => $fiber->isStarted(), 'suspended' => $fiber->isSuspended(), 'terminated' => $fiber->isTerminated()];
    $start = $fiber->start(['seed' => 7], 17);
    $premature = rp_fiber_error(static fn () => $fiber->getReturn());
    $suspended = $fiber->isSuspended();
    $resume = $fiber->resume(['answer' => 23, 'ok' => true]);
    return ['before' => $before, 'inside' => $states, 'start' => $start, 'suspended' => $suspended,
        'premature_return' => $premature, 'resume' => $resume, 'return' => $fiber->getReturn(),
        'terminated' => $fiber->isTerminated()];
});

$cases['throw_catch_finally'] = rp_fiber_observe(static function (): array {
    $trace = [];
    $fiber = new Fiber(static function () use (&$trace): array {
        $trace[] = 'caught:start';
        try {
            Fiber::suspend('armed');
            throw new LogicException('injection did not interrupt suspension');
        } catch (RuntimeException $error) {
            $trace[] = 'caught:catch:' . $error->getMessage();
            $reply = Fiber::suspend(['class' => get_class($error), 'message' => $error->getMessage()]);
            $trace[] = 'caught:resume:' . $reply;
            return ['reply' => $reply, 'caught_class' => get_class($error)];
        } finally {
            $trace[] = 'caught:finally';
        }
    });
    $start = $fiber->start();
    $thrown = $fiber->throw(new RuntimeException('injected'));
    $suspended = $fiber->isSuspended();
    $resume = $fiber->resume('finish');
    $uncaught = new Fiber(static function () use (&$trace): void {
        $trace[] = 'uncaught:start';
        try {
            Fiber::suspend('uncaught-armed');
        } finally {
            $trace[] = 'uncaught:finally';
        }
    });
    $uncaughtStart = $uncaught->start();
    $escaped = rp_fiber_error(static fn () => $uncaught->throw(new LogicException('uncaught')));
    return ['start' => $start, 'throw_value' => $thrown, 'suspended_after_throw' => $suspended,
        'resume' => $resume, 'return' => $fiber->getReturn(), 'terminated' => $fiber->isTerminated(),
        'uncaught_start' => $uncaughtStart, 'uncaught_error' => $escaped,
        'uncaught_terminated' => $uncaught->isTerminated(),
        'uncaught_return_error' => rp_fiber_error(static fn () => $uncaught->getReturn()), 'trace' => $trace];
});

$cases['suspended_gc'] = rp_fiber_observe(static function (): array {
    $trace = [];
    $enabled = gc_enabled();
    gc_enable();
    try {
        $fiber = new Fiber(static function () use (&$trace): void {
            $self = Fiber::getCurrent(); // A live suspended frame deliberately owns its Fiber: a collectable cycle.
            $trace[] = 'gc:start';
            try {
                Fiber::suspend('gc-paused');
                $trace[] = 'gc:unexpected-resume';
            } finally {
                $trace[] = $self instanceof Fiber ? 'gc:finally' : 'gc:missing-self';
            }
        });
        $start = $fiber->start();
        $weak = WeakReference::create($fiber);
        unset($fiber);
        $aliveBeforeCollection = $weak->get() instanceof Fiber;
        $collected = gc_collect_cycles();
        return ['start' => $start, 'alive_before_collection' => $aliveBeforeCollection,
            'collected_cycles' => $collected, 'weak_is_null' => $weak->get() === null, 'trace' => $trace];
    } finally {
        if (!$enabled) gc_disable();
    }
});

$cases['api_errors'] = rp_fiber_observe(static function (): array {
    $fiber = new Fiber(static fn (): int => 42);
    $before = [
        'resume' => rp_fiber_error(static fn () => $fiber->resume()),
        'throw' => rp_fiber_error(static fn () => $fiber->throw(new RuntimeException('not-started'))),
        'get_return' => rp_fiber_error(static fn () => $fiber->getReturn()),
    ];
    $start = $fiber->start();
    $after = [
        'start' => rp_fiber_error(static fn () => $fiber->start()),
        'resume' => rp_fiber_error(static fn () => $fiber->resume()),
        'throw' => rp_fiber_error(static fn () => $fiber->throw(new RuntimeException('terminated'))),
    ];
    return ['before_start' => $before, 'start' => $start, 'after_termination' => $after,
        'terminated' => $fiber->isTerminated(), 'return' => $fiber->getReturn(),
        'suspend_from_main' => rp_fiber_error(static fn () => Fiber::suspend('forbidden')),
        'main_current_is_null' => Fiber::getCurrent() === null];
});
$unexpected = ob_get_clean();
$ok = $unexpected === '';
foreach ($cases as $case) $ok = $ok && $case['status'] === 'ok' && $case['warnings'] === [];
echo json_encode(['schema' => 'recastpress.php-fiber-contract.v0.5', 'ok' => $ok,
    'runtime' => ['version' => PHP_VERSION, 'version_id' => PHP_VERSION_ID, 'int_size' => PHP_INT_SIZE,
        'sapi' => PHP_SAPI, 'fiber_stack_size' => ini_get('fiber.stack_size')],
    'cases' => $cases, 'unexpected_output_hex' => bin2hex($unexpected)],
    JSON_THROW_ON_ERROR | JSON_UNESCAPED_UNICODE | JSON_UNESCAPED_SLASHES), "\n";
exit($ok ? 0 : 1);
