<?php
/** Development-only scenarios executed by the real pinned WP_Hook class. */
error_reporting(E_ALL);
set_error_handler(static function ($level, $message, $file, $line) {
    throw new ErrorException($message, 0, $level, $file, $line);
});
require $argv[1] . '/wp-includes/plugin.php';
require __DIR__ . '/php_value_codec.php';

function run_hook_scenario($case) {
    $typed = $case['typed'] ?? false;
    $decode = static fn ($value) => $typed ? decode_value($value) : $value;
    $encode = static fn ($value) => $typed ? encode_value($value) : $value;
    $hook = new WP_Hook();
    $callbacks = [];
    $trace = [];
    $depth = 0;
    foreach ($case['callbacks'] as $name => $definition) {
        $callbacks[$name] = static function (...$args) use ($name, $definition, $hook, &$callbacks, &$trace, &$depth, $typed, $decode, $encode) {
            $trace[] = ['event' => 'callback', 'id' => $name, 'priority' => $hook->current_priority(), 'depth' => $depth, 'args' => array_map($encode, $args)];
            if ($depth === ($definition['mutate_at_depth'] ?? 1)) {
                foreach ($definition['mutations'] ?? [] as $mutation) {
                    switch ($mutation['kind']) {
                        case 'add':
                            $hook->add_filter('fixture', $callbacks[$mutation['id']], $mutation['priority'], $mutation['accepted'] ?? 1);
                            break;
                        case 'remove':
                            $hook->remove_filter('fixture', $callbacks[$mutation['id']], $mutation['priority']);
                            break;
                        case 'clear':
                            $hook->remove_all_filters($mutation['priority'] ?? false);
                            break;
                        case 'filter':
                            ++$depth;
                            $value = $decode($mutation['value']);
                            $output = $hook->apply_filters($value, [$value, ...array_map($decode, $mutation['extra'] ?? [])]);
                            --$depth;
                            $trace[] = ['event' => 'nested_filter_result', 'value' => $encode($output)];
                            break;
                        case 'action':
                            ++$depth;
                            $hook->do_action(array_map($decode, $mutation['args'] ?? []));
                            --$depth;
                            $trace[] = ['event' => 'nested_action_result'];
                            break;
                        default:
                            throw new RuntimeException('unknown mutation');
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
            return ($args[0] ?? '') . '|' . $name;
        };
    }
    foreach ($case['initial'] as $registration) {
        $hook->add_filter('fixture', $callbacks[$registration['id']], $registration['priority'], $registration['accepted'] ?? 1);
    }
    $outputs = [];
    foreach ($case['calls'] as $call) {
        ++$depth;
        if ($call['kind'] === 'filter') {
            $value = $decode($call['value']);
            $outputs[] = $encode($hook->apply_filters($value, [$value, ...array_map($decode, $call['extra'] ?? [])]));
        } elseif ($call['kind'] === 'action') {
            $hook->do_action(array_map($decode, $call['args'] ?? []));
            $outputs[] = null;
        } else {
            throw new RuntimeException('unknown call');
        }
        --$depth;
    }
    $priorities = [];
    foreach ($callbacks as $name => $callback) {
        $found = $hook->has_filter('fixture', $callback);
        $priorities[$name] = $found === false ? null : $found;
    }
    return ['id' => $case['id'], 'outputs' => $outputs, 'trace' => $trace, 'priorities' => (object) $priorities, 'active_depth' => $depth];
}

$cases = json_decode(stream_get_contents(STDIN), true, 128, JSON_THROW_ON_ERROR);
echo json_encode(['php_version' => PHP_VERSION, 'results' => array_map('run_hook_scenario', $cases)], JSON_THROW_ON_ERROR), "\n";
