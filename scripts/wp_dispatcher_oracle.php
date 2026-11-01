<?php
/** Development-only named scenarios using pinned WordPress plugin.php functions. */
error_reporting(E_ALL);
set_error_handler(static function ($level, $message, $file, $line) {
    $GLOBALS['fixture_diagnostics'][] = ['level' => $level, 'message' => $message];
    return true;
});
require $argv[1] . '/wp-includes/plugin.php';
require __DIR__ . '/php_value_codec.php';

function fixture_counts() {
    $result = [];
    foreach (['alpha', 'beta', 'all', 'missing'] as $name) { $result[$name] = ['action' => did_action($name), 'filter' => did_filter($name)]; }
    return $result;
}

function run_dispatcher_scenario($case) {
    $typed = $case['typed'] ?? false;
    $decode = static fn ($value) => $typed ? decode_value($value) : $value;
    $encode = static fn ($value) => $typed ? encode_value($value) : $value;
    global $wp_filter, $wp_actions, $wp_filters, $wp_current_filter;
    $wp_filter = []; $wp_actions = []; $wp_filters = []; $wp_current_filter = [];
    $GLOBALS['fixture_diagnostics'] = [];
    $callbacks = [];
    $trace = [];
    foreach ($case['callbacks'] as $ident => $definition) {
        $callbacks[$ident] = static function (...$args) use ($ident, $definition, &$callbacks, &$trace, $typed, $decode, $encode) {
            global $wp_current_filter;
            $trace[] = ['event' => 'callback', 'id' => $ident, 'current' => current_filter(), 'depth' => count($wp_current_filter), 'doing' => doing_filter(), 'alpha_active' => doing_filter('alpha'), 'counts' => fixture_counts(), 'args' => array_map($encode, $args)];
            if (count($wp_current_filter) === ($definition['mutate_at_depth'] ?? 1)) {
                foreach ($definition['mutations'] ?? [] as $mutation) {
                    $hook = $mutation['hook'] ?? 'alpha';
                    switch ($mutation['kind']) {
                        case 'add': add_filter($hook, $callbacks[$mutation['id']], $mutation['priority'], $mutation['accepted'] ?? 1); break;
                        case 'remove': remove_filter($hook, $callbacks[$mutation['id']], $mutation['priority']); break;
                        case 'clear': remove_all_filters($hook, $mutation['priority'] ?? false); break;
                        case 'filter':
                            $output = apply_filters($hook, $decode($mutation['value']), ...array_map($decode, $mutation['extra'] ?? []));
                            $trace[] = ['event' => 'nested_filter_result', 'hook' => $hook, 'value' => $encode($output)];
                            break;
                        case 'action':
                            do_action($hook, ...array_map($decode, $mutation['args'] ?? []));
                            $trace[] = ['event' => 'nested_action_result', 'hook' => $hook];
                            break;
                        default: throw new RuntimeException('unknown mutation');
                    }
                }
            }
            if ($typed) {
                switch ($definition['operation'] ?? 'first') {
                    case 'first': return array_key_exists(0, $args) ? $args[0] : null;
                    case 'arg': return array_key_exists($definition['index'] ?? 0, $args) ? $args[$definition['index'] ?? 0] : null;
                    case 'constant': return $decode($definition['return_value']);
                    case 'array_put':
                        if (!is_array($args[0] ?? null)) throw new RuntimeException('array_put requires array');
                        $output = $args[0];
                        $output[$decode($definition['key'])] = $decode($definition['entry_value']);
                        return $output;
                    default: throw new RuntimeException('unknown typed fixture operation');
                }
            }
            return ($args[0] ?? '') . '|' . $ident;
        };
    }
    foreach ($case['initial'] as $registration) { add_filter($registration['hook'] ?? 'alpha', $callbacks[$registration['id']], $registration['priority'], $registration['accepted'] ?? 1); }
    $outputs = [];
    foreach ($case['calls'] as $call) {
        $hook = $call['hook'] ?? 'alpha';
        if ($call['kind'] === 'filter') { $output = $encode(apply_filters($hook, $decode($call['value']), ...array_map($decode, $call['extra'] ?? []))); }
        elseif ($call['kind'] === 'action') { do_action($hook, ...array_map($decode, $call['args'] ?? [])); $output = null; }
        else { throw new RuntimeException('unknown call'); }
        $outputs[] = ['value' => $output, 'current' => current_filter(), 'depth' => count($wp_current_filter)];
    }
    $priorities = [];
    foreach (['alpha', 'beta', 'all', 'missing'] as $hook) {
        $entries = [];
        foreach ($callbacks as $ident => $callback) { $found = has_filter($hook, $callback); $entries[$ident] = $found === false ? null : $found; }
        $priorities[$hook] = (object) $entries;
    }
    return ['id' => $case['id'], 'outputs' => $outputs, 'trace' => $trace, 'priorities' => $priorities, 'counts' => fixture_counts(), 'diagnostics' => $GLOBALS['fixture_diagnostics']];
}

$cases = json_decode(stream_get_contents(STDIN), true, 128, JSON_THROW_ON_ERROR);
echo json_encode(['php_version' => PHP_VERSION, 'results' => array_map('run_dispatcher_scenario', $cases)], JSON_THROW_ON_ERROR), "\n";
