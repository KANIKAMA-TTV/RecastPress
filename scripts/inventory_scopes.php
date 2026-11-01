<?php
/** Lexical declaration scopes only; never loads any indexed PHP source. */

function inventory_brace_pairs($tokens) {
    $stack = [];
    $pairs = [];
    foreach ($tokens as $index => $token) {
        if ($token === '{' || (is_array($token) && in_array($token[0], [T_CURLY_OPEN, T_DOLLAR_OPEN_CURLY_BRACES], true))) {
            $stack[] = $index;
        } elseif ($token === '}') {
            $open = array_pop($stack);
            if ($open === null) throw new RuntimeException('unmatched source brace');
            $pairs[$open] = $index;
        }
    }
    if ($stack) throw new RuntimeException('unclosed source brace');
    return $pairs;
}

function inventory_body_open($tokens, $start, $pairs) {
    $parentheses = 0;
    $brackets = 0;
    for ($index = $start; isset($tokens[$index]); ++$index) {
        $token = $tokens[$index];
        if (is_array($token)) {
            if ($token[0] === T_ATTRIBUTE) ++$brackets;
            if (in_array($token[0], [T_CURLY_OPEN, T_DOLLAR_OPEN_CURLY_BRACES], true)) $index = $pairs[$index];
            continue;
        }
        if ($token === '(') ++$parentheses;
        if ($token === ')') --$parentheses;
        if ($token === '[') ++$brackets;
        if ($token === ']') --$brackets;
        if ($token === '{') {
            if ($parentheses === 0 && $brackets === 0) return $index;
            $index = $pairs[$index];
        }
        if ($token === ';' && $parentheses === 0 && $brackets === 0) return null;
    }
    return null;
}

function inventory_prefix($tokens, $start) {
    $visibility = 'public';
    $static = false;
    $doc = '';
    for ($index = $start - 1; isset($tokens[$index]); --$index) {
        $token = $tokens[$index];
        if (in_array($token, [';', '{', '}'], true)) break;
        if (!is_array($token)) continue;
        if (in_array($token[0], [T_PUBLIC, T_PROTECTED, T_PRIVATE], true)) $visibility = strtolower($token[1]);
        if ($token[0] === T_STATIC) $static = true;
        if ($token[0] === T_DOC_COMMENT && $doc === '') $doc = $token[1];
    }
    return ['visibility' => $visibility, 'static' => $static,
        'documented_internal' => (bool) preg_match('/@internal\b|@access\s+private\b/', $doc),
        'documented_deprecated' => (bool) preg_match('/@deprecated\b/', $doc)];
}

function inventory_scopes($tokens, $path) {
    $pairs = inventory_brace_pairs($tokens);
    $pending = [];
    $frames = [];
    $namespace = '';
    $annotations = [];
    $import_until = -1;
    $hook_names = ['apply_filters', 'apply_filters_ref_array', 'do_action', 'do_action_ref_array', 'add_filter', 'add_action', 'remove_filter', 'remove_action'];
    foreach ($tokens as $index => $token) {
        while ($frames && end($frames)['end'] < $index) {
            $old = array_pop($frames);
            if ($old['scope_kind'] === 'namespace') $namespace = $old['previous_namespace'];
        }
        if (isset($pending[$index])) {
            $frames[] = $pending[$index];
            if ($pending[$index]['scope_kind'] === 'namespace') $namespace = $pending[$index]['namespace'];
        }
        if (!is_array($token)) continue;
        if ($index <= $import_until) continue;
        [$kind, $text, $line] = $token;
        $owner = null;
        foreach (array_reverse($frames) as $frame) {
            if ($frame['scope_kind'] !== 'namespace') { $owner = $frame; break; }
        }
        $context = ['namespace' => $namespace, 'lexical_container' => $owner['qualified_name'] ?? null];
        if ($kind === T_USE && $owner === null) {
            $next = significant_token($tokens, $index + 1, 1);
            if ($next !== null && $tokens[$next] !== '(') {
                for ($cursor = $next; isset($tokens[$cursor]) && $tokens[$cursor] !== ';'; ++$cursor) {}
                $import_until = $cursor;
                continue;
            }
        }
        if ($kind === T_NAMESPACE) {
            $name = '';
            for ($cursor = $index + 1; isset($tokens[$cursor]); ++$cursor) {
                $part = $tokens[$cursor];
                if ($part === ';' || $part === '{') break;
                if (is_array($part) && in_array($part[0], [T_STRING, T_NAME_QUALIFIED, T_NS_SEPARATOR], true)) $name .= $part[1];
            }
            if (($tokens[$cursor] ?? null) === '{') {
                $pending[$cursor] = ['scope_kind' => 'namespace', 'end' => $pairs[$cursor],
                    'namespace' => $name, 'previous_namespace' => $namespace];
            } else { $namespace = $name; }
            continue;
        }
        if (in_array($kind, [T_CLASS, T_INTERFACE, T_TRAIT, T_ENUM], true)) {
            $previous = significant_token($tokens, $index - 1, -1);
            if ($previous !== null && is_array($tokens[$previous]) && $tokens[$previous][0] === T_DOUBLE_COLON) continue;
            $next = significant_token($tokens, $index + 1, 1);
            $named = $next !== null && is_array($tokens[$next]) && $tokens[$next][0] === T_STRING;
            $name = $named ? $tokens[$next][1] : '{anonymous@' . $path . ':' . $line . '#' . $index . '}';
            $qualified = '\\' . ($namespace === '' ? '' : $namespace . '\\') . $name;
            $open = inventory_body_open($tokens, $index + 1, $pairs);
            if ($open !== null) $pending[$open] = ['scope_kind' => 'class', 'end' => $pairs[$open],
                'qualified_name' => $qualified, 'anonymous' => !$named];
            if ($named) {
                $prefix = inventory_prefix($tokens, $index);
                $annotations[$index] = $context + ['qualified_name' => $qualified,
                    'documented_internal' => $prefix['documented_internal'], 'documented_deprecated' => $prefix['documented_deprecated']];
            }
        }
        if ($kind === T_FUNCTION) {
            $next = significant_token($tokens, $index + 1, 1);
            if ($next !== null && (is_array($tokens[$next]) ? $tokens[$next][1] : $tokens[$next]) === '&') $next = significant_token($tokens, $next + 1, 1);
            $named = $next !== null && is_array($tokens[$next]) && $tokens[$next][0] === T_STRING;
            $method = $named && ($owner['scope_kind'] ?? null) === 'class';
            $name = $named ? $tokens[$next][1] : '{closure@' . $path . ':' . $line . '#' . $index . '}';
            $qualified = $method ? $owner['qualified_name'] . '::' . $name : '\\' . ($namespace === '' ? '' : $namespace . '\\') . $name;
            $open = inventory_body_open($tokens, $index + 1, $pairs);
            if ($open !== null) $pending[$open] = ['scope_kind' => 'function', 'end' => $pairs[$open], 'qualified_name' => $qualified];
            if ($named) {
                $prefix = inventory_prefix($tokens, $index);
                $annotations[$index] = $context + ['kind' => $method ? 'method' : 'function',
                    'qualified_name' => $qualified, 'class' => $method ? $owner['qualified_name'] : null,
                    'anonymous_class' => $method && $owner['anonymous'],
                    'visibility' => $method ? $prefix['visibility'] : null, 'static' => $method && $prefix['static'],
                    'documented_internal' => $prefix['documented_internal'], 'documented_deprecated' => $prefix['documented_deprecated']];
            }
        }
        if (in_array($kind, [T_STRING, T_NAME_FULLY_QUALIFIED], true) && in_array(ltrim($text, '\\'), $hook_names, true)) {
            $annotations[$index] = $context + ['name_resolution' => $kind === T_NAME_FULLY_QUALIFIED ? 'explicit_global' : 'unresolved_unqualified'];
        }
    }
    return $annotations;
}
