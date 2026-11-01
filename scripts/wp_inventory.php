<?php
/** Lexical declaration/hook candidates only. Never includes or evaluates source. */
error_reporting(E_ALL);
set_error_handler(static function ($level, $message, $file, $line) {
    throw new ErrorException($message, 0, $level, $file, $line);
});
require __DIR__ . '/inventory_scopes.php';

function significant_token($tokens, $start, $direction) {
    for ($index = $start; isset($tokens[$index]); $index += $direction) {
        $token = $tokens[$index];
        if (is_array($token) && in_array($token[0], [T_WHITESPACE, T_COMMENT, T_DOC_COMMENT], true)) {
            continue;
        }
        return $index;
    }
    return null;
}

function inventory_source($source, $path) {
    $tokens = token_get_all($source, TOKEN_PARSE);
    $scopes = inventory_scopes($tokens, $path);
    $declarations = [];
    $hooks = [];
    $hook_names = ['apply_filters', 'apply_filters_ref_array', 'do_action', 'do_action_ref_array', 'add_filter', 'add_action', 'remove_filter', 'remove_action'];
    foreach ($tokens as $index => $token) {
        if (!is_array($token)) { continue; }
        [$kind, $text, $line] = $token;
        if ($kind === T_FUNCTION && isset($scopes[$index])) {
            $next = significant_token($tokens, $index + 1, 1);
            if ($next !== null && (is_array($tokens[$next]) ? $tokens[$next][1] : $tokens[$next]) === '&') {
                $next = significant_token($tokens, $next + 1, 1);
            }
            if ($next !== null && is_array($tokens[$next]) && $tokens[$next][0] === T_STRING) {
                $declarations[] = ['name' => $tokens[$next][1], 'source' => $path, 'line' => $line] + $scopes[$index];
            }
        }
        if (in_array($kind, [T_CLASS, T_INTERFACE, T_TRAIT, T_ENUM], true)) {
            $previous = significant_token($tokens, $index - 1, -1);
            if ($previous !== null && is_array($tokens[$previous]) && $tokens[$previous][0] === T_DOUBLE_COLON) {
                continue; // Foo::class is not a declaration.
            }
            $next = significant_token($tokens, $index + 1, 1);
            if ($next !== null && is_array($tokens[$next]) && $tokens[$next][0] === T_STRING) {
                $declarations[] = ['kind' => match ($kind) {T_CLASS => 'class', T_INTERFACE => 'interface', T_TRAIT => 'trait', T_ENUM => 'enum'}, 'name' => $tokens[$next][1], 'source' => $path, 'line' => $line] + $scopes[$index];
            }
        }
        if (!in_array($kind, [T_STRING, T_NAME_FULLY_QUALIFIED], true)) { continue; }
        $name = ltrim($text, '\\');
        if (!in_array($name, $hook_names, true)) { continue; }
        $previous = significant_token($tokens, $index - 1, -1);
        if ($previous !== null) {
            $previous_token = $tokens[$previous];
            if (is_array($previous_token) && in_array($previous_token[0], [T_FUNCTION, T_OBJECT_OPERATOR, T_NULLSAFE_OBJECT_OPERATOR, T_DOUBLE_COLON], true)) {
                continue;
            }
            if ((is_array($previous_token) ? $previous_token[1] : $previous_token) === '&') {
                $before = significant_token($tokens, $previous - 1, -1);
                if ($before !== null && is_array($tokens[$before]) && $tokens[$before][0] === T_FUNCTION) { continue; }
            }
        }
        $open = significant_token($tokens, $index + 1, 1);
        if ($open === null || $tokens[$open] !== '(') { continue; }
        $argument = [];
        $depth = 0;
        for ($cursor = $open + 1; isset($tokens[$cursor]); ++$cursor) {
            $part = $tokens[$cursor];
            if (!is_array($part)) {
                if (in_array($part, ['(', '[', '{'], true)) { ++$depth; }
                if (in_array($part, [')', ']', '}'], true)) {
                    if ($depth === 0) { break; }
                    --$depth;
                }
                if ($part === ',' && $depth === 0) { break; }
            }
            if (is_array($part) && in_array($part[0], [T_WHITESPACE, T_COMMENT, T_DOC_COMMENT], true)) { continue; }
            $argument[] = $part;
        }
        $literal = count($argument) === 1 && is_array($argument[0]) && $argument[0][0] === T_CONSTANT_ENCAPSED_STRING;
        $expression = implode('', array_map(static fn ($part) => is_array($part) ? $part[1] : $part, $argument));
        $hooks[] = ['function' => $name, 'source' => $path, 'line' => $line, 'first_argument_kind' => $literal ? 'literal_source' : 'expression', 'first_argument_source' => substr($expression, 0, 512), 'truncated' => strlen($expression) > 512] + $scopes[$index];
    }
    return ['declarations' => $declarations, 'hooks' => $hooks];
}

$root = $argv[1] ?? throw new RuntimeException('missing source root');
$files = json_decode(stream_get_contents(STDIN), true, 128, JSON_THROW_ON_ERROR);
$output = ['declarations' => [], 'hooks' => []];
foreach ($files as $file) {
    $source = file_get_contents($root . '/' . $file);
    $result = inventory_source($source, $file);
    array_push($output['declarations'], ...$result['declarations']);
    array_push($output['hooks'], ...$result['hooks']);
}
echo json_encode($output, JSON_THROW_ON_ERROR | JSON_INVALID_UTF8_SUBSTITUTE), "\n";
