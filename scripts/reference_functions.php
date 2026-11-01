<?php
/**
 * Selected-function loader for hash-verified development references only.
 * Sources must remain immutable for this PHP process. Cache only extracted
 * code strings, never token arrays; this is neither a general PHP parser nor
 * a source-change monitor. Nothing executes until its snippet is requested.
 */
function reference_function_snippets($file) {
    static $by_file = [];
    if (array_key_exists($file, $by_file)) {
        return $by_file[$file];
    }
    $tokens = token_get_all(file_get_contents($file), TOKEN_PARSE);
    $count = count($tokens);
    $snippets = [];
    for ($index = 0; $index < $count; ++$index) {
        if (!is_array($tokens[$index]) || $tokens[$index][0] !== T_FUNCTION) {
            continue;
        }
        $next = $index + 1;
        while ($next < $count && is_array($tokens[$next]) && $tokens[$next][0] === T_WHITESPACE) {
            ++$next;
        }
        if (!is_array($tokens[$next])) {
            continue;
        }
        // Keep the old exact next-token rule: skip only whitespace, not
        // comments or reference markers. Retain the first matching snippet.
        $name = $tokens[$next][1];
        if (array_key_exists($name, $snippets)) { continue; }
        $code = '';
        $depth = 0;
        $opened = false;
        for ($cursor = $index; $cursor < $count; ++$cursor) {
            $token = $tokens[$cursor];
            $code .= is_array($token) ? $token[1] : $token;
            if ($token === '{') { ++$depth; $opened = true; }
            if (is_array($token) && in_array($token[0], [T_CURLY_OPEN, T_DOLLAR_OPEN_CURLY_BRACES], true)) { ++$depth; }
            if ($token === '}') { --$depth; }
            if ($opened && $depth === 0) {
                $snippets[$name] = $code;
                break;
            }
        }
        // Do not skip this body in the outer scan: nested named functions
        // remain independently discoverable, as in the previous loader.
    }
    unset($tokens);
    $by_file[$file] = $snippets;
    return $snippets;
}

function load_reference_function($file, $name) {
    $snippets = reference_function_snippets($file);
    if (is_string($name) && array_key_exists($name, $snippets)) {
        $code = $snippets[$name];
        // Evaluate only the requested function, on every request. A repeated
        // load must retain PHP's redeclaration failure, not silently skip it.
        eval($code);
        return;
    }
    throw new RuntimeException('reference function missing: ' . $name);
}
