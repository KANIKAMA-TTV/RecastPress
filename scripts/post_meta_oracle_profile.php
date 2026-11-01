<?php
/** Isolated cached SINGLE fixture. Not a WordPress cache/backend/bootstrap. */

$post_meta_cached_counts = array_fill_keys(
    ['calls', 'get_hook', 'default_hook', 'cache_get', 'cache_update', 'db_calls', 'restored'], 0
);

function post_meta_cached_load_reference_functions($root) {
    load_reference_function($root . '/wp-includes/post.php', 'get_post_meta');
    foreach (['get_metadata', 'get_metadata_raw', 'get_metadata_default'] as $name) {
        load_reference_function($root . '/wp-includes/meta.php', $name);
    }
}

function post_meta_cached_profile() {
    global $root;
    return [
        'php_version' => PHP_VERSION, 'name' => 'complete_cached_single_empty_filters',
        'object_id' => 'typed_positive_signed_i64', 'meta_key' => 'php_truthy_arbitrary_bytes',
        'cache' => 'caller_prepared_complete_nonempty_php_array',
        'cache_values' => 'null_or_nonempty_dense_lists_of_raw_null_or_bytes',
        'single' => true, 'complete_cache' => true, 'empty_filters' => true,
        'selected_decode' => 'first_value_only_one_layer_canonical_modeled_subset',
        'canonical_admission' => 'php_serialize_typed_witness_equals_php_trim_selected_raw',
        'missing_or_null' => 'source_default_empty_bytes', 'native_filter_bookkeeping' => false,
        'cache_miss' => 'outside_profile', 'database_calls' => 0,
        'fixture_limits' => ['max_input_bytes' => 65536, 'max_array_entries' => 1024,
            'max_owned_string_bytes' => 65536, 'max_array_depth' => 64],
        'meta_source_sha256' => hash_file('sha256', $root . '/wp-includes/meta.php'),
        'post_source_sha256' => hash_file('sha256', $root . '/wp-includes/post.php'),
    ];
}

function post_meta_cached_metadata() {
    global $post_meta_cached_counts;
    return ['post_meta_cached_profile' => post_meta_cached_profile(),
        'post_meta_cached_counts' => $post_meta_cached_counts];
}

function post_meta_cached_require($condition) {
    if (!$condition) { throw new RuntimeException('Cached post meta profile unsupported'); }
}

function post_meta_cached_exact_keys(#[\SensitiveParameter] $value, $keys) {
    if (!is_array($value) || count($value) !== count($keys)) { return false; }
    foreach ($keys as $key) { if (!array_key_exists($key, $value)) { return false; } }
    return true;
}

// Validate tags/field types before using the shared development codec. In
// particular its bool branch alone is not a typed-wire validator. These input
// and witness buffers are fixture preparation, not the native returned-value
// allocation budget. Unselected cache values are never unserialized.
function post_meta_cached_wire(#[\SensitiveParameter] $wire, $witness = false, $depth = 0) {
    post_meta_cached_require(is_array($wire) && isset($wire['type']) && is_string($wire['type']) && $depth <= 128);
    switch ($wire['type']) {
        case 'null':
            post_meta_cached_require(post_meta_cached_exact_keys($wire, ['type']));
            break;
        case 'bool':
            post_meta_cached_require(post_meta_cached_exact_keys($wire, ['type', 'value']) && is_bool($wire['value']));
            break;
        case 'int':
            post_meta_cached_require(post_meta_cached_exact_keys($wire, ['type', 'value']) && is_string($wire['value']));
            decode_integer_wire($wire['value']);
            break;
        case 'string':
            post_meta_cached_require(post_meta_cached_exact_keys($wire, ['type', 'hex']) && is_string($wire['hex']));
            $hex = $wire['hex'];
            post_meta_cached_require(strlen($hex) % 2 === 0 && strspn($hex, '0123456789abcdefABCDEF') === strlen($hex));
            break;
        case 'float':
            post_meta_cached_require(!$witness && post_meta_cached_exact_keys($wire, ['type', 'value']) && is_string($wire['value']));
            $number = $wire['value'];
            post_meta_cached_require(in_array($number, ['nan', 'inf', '-inf'], true)
                || ($number !== '' && is_numeric($number) && strpbrk($number, " \t\n\r\v\f") === false));
            break;
        case 'array':
            post_meta_cached_require(post_meta_cached_exact_keys($wire, ['type', 'entries'])
                && is_array($wire['entries']) && array_is_list($wire['entries']));
            foreach ($wire['entries'] as $entry) {
                post_meta_cached_require(post_meta_cached_exact_keys($entry, ['key', 'value'])
                    && is_array($entry['key']) && in_array($entry['key']['type'] ?? null, ['int', 'string'], true));
                post_meta_cached_wire($entry['key'], $witness, $depth + 1);
                post_meta_cached_wire($entry['value'], $witness, $depth + 1);
            }
            break;
        default:
            post_meta_cached_require(false);
    }
}

function post_meta_cached_charge(&$used, $amount, $maximum) {
    post_meta_cached_require(is_int($amount) && $amount >= 0 && $used <= $maximum && $amount <= $maximum - $used);
    $used += $amount;
}

function post_meta_cached_measure(#[\SensitiveParameter] $value, $limits, &$pairs, &$owned, $depth = 0) {
    if (is_string($value)) {
        post_meta_cached_charge($owned, strlen($value), $limits['max_owned_string_bytes']);
    } elseif (is_array($value)) {
        post_meta_cached_require($depth < $limits['max_array_depth']);
        post_meta_cached_charge($pairs, count($value), $limits['max_array_entries']);
        foreach ($value as $key => $child) {
            if (is_string($key)) { post_meta_cached_charge($owned, strlen($key), $limits['max_owned_string_bytes']); }
            post_meta_cached_measure($child, $limits, $pairs, $owned, $depth + 1);
        }
    } else {
        post_meta_cached_require(is_null($value) || is_bool($value) || is_int($value));
    }
}

// Keep malformed owner wire errors inside a redacted boundary too: the shared
// codec is intentionally unchanged for every older primitive.
function post_meta_cached_decode_owner(#[\SensitiveParameter] $case) {
    try {
        post_meta_cached_require(is_array($case['input'] ?? null) && ($case['input']['type'] ?? null) === 'int');
        post_meta_cached_wire($case['input']);
        return decode_value($case['input']);
    } catch (Throwable $unused) {
        throw new RuntimeException('Cached post meta input/profile unsupported');
    }
}

function post_meta_cached_guard(#[\SensitiveParameter] $owner, #[\SensitiveParameter] $case) {
    post_meta_cached_require(is_int($owner) && $owner > 0 && PHP_INT_SIZE === 8
        && ($case['post_meta_profile'] ?? null) === 'complete_cached_single_empty_filters'
        && ($case['single'] ?? null) === true && ($case['complete_cache'] ?? null) === true
        && ($case['empty_filters'] ?? null) === true);
    post_meta_cached_require(is_array($case['input'] ?? null) && ($case['input']['type'] ?? null) === 'int');
    post_meta_cached_wire($case['input']);
    post_meta_cached_require(is_array($case['meta_key'] ?? null) && ($case['meta_key']['type'] ?? null) === 'string');
    post_meta_cached_wire($case['meta_key']);
    $key = decode_value($case['meta_key']);
    post_meta_cached_require($key !== '' && $key !== '0');
    post_meta_cached_require(is_array($case['cache'] ?? null) && ($case['cache']['type'] ?? null) === 'array');
    post_meta_cached_wire($case['cache']);
    $cache = decode_value($case['cache']);
    post_meta_cached_require(is_array($cache) && count($cache) > 0);
    $limits = $case['limits'] ?? null;
    $keys = ['max_input_bytes', 'max_array_entries', 'max_owned_string_bytes', 'max_array_depth'];
    post_meta_cached_require(post_meta_cached_exact_keys($limits, $keys));
    foreach ($keys as $name) { post_meta_cached_require(is_int($limits[$name]) && $limits[$name] >= 0); }
    post_meta_cached_require($limits['max_array_depth'] <= 64);
    $default = 1;
    if (isset($cache[$key])) {
        $values = $cache[$key];
        post_meta_cached_require(is_array($values) && count($values) > 0 && array_is_list($values));
        foreach ($values as $value) { post_meta_cached_require(is_null($value) || is_string($value)); }
        $raw = $values[0];
        if ($raw !== null) {
            post_meta_cached_require(strlen($raw) <= $limits['max_input_bytes']);
            if (is_serialized($raw)) {
                post_meta_cached_require(array_key_exists('witness', $case));
                post_meta_cached_wire($case['witness'], true);
                $witness = decode_value($case['witness']);
                $pairs = 0; $owned = 0;
                post_meta_cached_measure($witness, $limits, $pairs, $owned);
                // Never pre-unserialize raw input or instantiate a class.
                // A modeled typed graph has no objects, references or floats.
                post_meta_cached_require(serialize($witness) === trim($raw));
                $default = is_null($witness) ? 1 : 0;
            } else {
                post_meta_cached_require(!array_key_exists('witness', $case)
                    && strlen($raw) <= $limits['max_owned_string_bytes']);
                $default = 0;
            }
        } else {
            post_meta_cached_require(!array_key_exists('witness', $case));
        }
    } else {
        post_meta_cached_require(!array_key_exists('witness', $case));
    }
    return [$key, $cache, $default];
}

// These fixture-only APIs deliberately fail rather than reusing an ambient
// cache implementation. They are not production cache/SQL implementations.
if (function_exists('wp_cache_get') || function_exists('update_meta_cache')) {
    throw new RuntimeException('Cached post meta fixture function conflict');
} else {
    function wp_cache_get(#[\SensitiveParameter] $key, #[\SensitiveParameter] $group) {
        if (($GLOBALS['auth_cookie_scope']['active'] ?? false) === true) {
            ++$GLOBALS['auth_cookie_scope']['cache_calls'];
            throw new RuntimeException('Prepared auth cookie cache access forbidden');
        }
        if (($GLOBALS['title_scope']['active'] ?? false) === true) {
            ++$GLOBALS['title_scope']['cache_calls'];
            throw new RuntimeException('Prepared title cache access forbidden');
        }
        $scope = $GLOBALS['post_meta_cached_scope'] ?? null;
        post_meta_cached_require(is_array($scope) && $key === $scope['owner'] && $group === 'post_meta');
        ++$GLOBALS['post_meta_cached_scope']['cache_get'];
        return $scope['cache'];
    }
    function update_meta_cache(#[\SensitiveParameter] $type, #[\SensitiveParameter] $ids) {
        if (($GLOBALS['auth_cookie_scope']['active'] ?? false) === true) {
            ++$GLOBALS['auth_cookie_scope']['cache_calls'];
            throw new RuntimeException('Prepared auth cookie cache access forbidden');
        }
        if (($GLOBALS['title_scope']['active'] ?? false) === true) {
            ++$GLOBALS['title_scope']['cache_calls'];
            throw new RuntimeException('Prepared title cache access forbidden');
        }
        if (is_array($GLOBALS['post_meta_cached_scope'] ?? null)) { ++$GLOBALS['post_meta_cached_scope']['cache_update']; }
        throw new RuntimeException('Cached post meta miss is outside fixture');
    }
}

final class PostMetaCachedSqlTrap {
    private int $calls = 0;
    public function calls(): int { return $this->calls; }
    public function __get(#[\SensitiveParameter] $name) {
        ++$this->calls;
        throw new RuntimeException('Cached post meta database access forbidden');
    }
    public function __call(#[\SensitiveParameter] $name, #[\SensitiveParameter] $arguments) {
        ++$this->calls;
        throw new RuntimeException('Cached post meta database access forbidden');
    }
}

function oracle_post_meta_cached_single(#[\SensitiveParameter] $owner, #[\SensitiveParameter] $case) {
    global $post_meta_cached_counts;
    try { [$key, $cache, $expected_default] = post_meta_cached_guard($owner, $case); }
    catch (Throwable $unused) { throw new RuntimeException('Cached post meta input/profile unsupported'); }
    $mutated = ['wp_filter', 'wp_filters', 'wp_current_filter', 'post_meta_cached_scope', 'wpdb'];
    $untouched = ['wp_actions', 'wp_hasher', 'wp_meta_keys', 'wp_object_cache'];
    $saved = [];
    foreach ([...$mutated, ...$untouched] as $name) {
        $saved[$name] = [array_key_exists($name, $GLOBALS), $GLOBALS[$name] ?? null];
    }
    $trap = new PostMetaCachedSqlTrap();
    ++$post_meta_cached_counts['calls'];
    try {
        $GLOBALS['wp_filter'] = []; $GLOBALS['wp_filters'] = []; $GLOBALS['wp_current_filter'] = [];
        $GLOBALS['post_meta_cached_scope'] = ['owner' => $owner, 'cache' => $cache, 'cache_get' => 0, 'cache_update' => 0];
        $GLOBALS['wpdb'] = $trap;
        try { $result = get_post_meta($owner, $key, true); }
        catch (Throwable $unused) { throw new RuntimeException('Cached post meta reference failed'); }
        $hooks = ['get_post_metadata' => 1];
        if ($expected_default === 1) { $hooks['default_post_metadata'] = 1; }
        post_meta_cached_require($GLOBALS['wp_filter'] === [] && $GLOBALS['wp_filters'] === $hooks
            && $GLOBALS['wp_current_filter'] === [] && $GLOBALS['post_meta_cached_scope']['cache_get'] === 1
            && $GLOBALS['post_meta_cached_scope']['cache_update'] === 0 && $trap->calls() === 0
            && $GLOBALS['wpdb'] === $trap);
        foreach ($untouched as $name) {
            [$had, $old] = $saved[$name];
            post_meta_cached_require(array_key_exists($name, $GLOBALS) === $had && (!$had || $GLOBALS[$name] === $old));
        }
        return $result;
    } finally {
        $filters = $GLOBALS['wp_filters'] ?? [];
        $scope = $GLOBALS['post_meta_cached_scope'] ?? [];
        $post_meta_cached_counts['get_hook'] += is_array($filters) && is_int($filters['get_post_metadata'] ?? null) ? $filters['get_post_metadata'] : 0;
        $post_meta_cached_counts['default_hook'] += is_array($filters) && is_int($filters['default_post_metadata'] ?? null) ? $filters['default_post_metadata'] : 0;
        $post_meta_cached_counts['cache_get'] += is_array($scope) && is_int($scope['cache_get'] ?? null) ? $scope['cache_get'] : 0;
        $post_meta_cached_counts['cache_update'] += is_array($scope) && is_int($scope['cache_update'] ?? null) ? $scope['cache_update'] : 0;
        $post_meta_cached_counts['db_calls'] += $trap->calls();
        foreach ($saved as $name => [$had, $old]) { if ($had) { $GLOBALS[$name] = $old; } else { unset($GLOBALS[$name]); } }
        foreach ($saved as $name => [$had, $old]) {
            if (array_key_exists($name, $GLOBALS) !== $had || ($had && $GLOBALS[$name] !== $old)) {
                throw new RuntimeException('Cached post meta scope restoration failed');
            }
        }
        ++$post_meta_cached_counts['restored'];
    }
}
