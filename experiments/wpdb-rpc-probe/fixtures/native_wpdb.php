<?php
/** Fixed native mysqli/wpdb reference; no arbitrary SQL or database startup. */
declare(strict_types=1);

ini_set('display_errors', '0');
ini_set('log_errors', '0');
error_reporting(E_ALL);

final class RpNativeFailure extends RuntimeException {}

function rp_check(bool $condition, string $code): void {
    if (!$condition) {
        throw new RpNativeFailure($code);
    }
}

/** Strings remain bytes; PHP scalar and collection types remain distinguishable. */
function rp_value(mixed $value): mixed {
    if (is_string($value)) {
        return ['bytes_hex' => bin2hex($value)];
    }
    if ($value === null || is_bool($value) || is_int($value)) {
        return $value;
    }
    if (is_array($value)) {
        $entries = [];
        foreach ($value as $key => $item) {
            $entries[] = ['key' => rp_value($key), 'value' => rp_value($item)];
        }
        return ['type' => 'array', 'entries' => $entries];
    }
    if ($value instanceof stdClass) {
        $properties = [];
        foreach (get_object_vars($value) as $key => $item) {
            $properties[] = ['name_hex' => bin2hex((string) $key), 'value' => rp_value($item)];
        }
        return ['type' => 'stdClass', 'properties' => $properties];
    }
    throw new RpNativeFailure('unexpected_php_value_type');
}

function rp_input(): array {
    rp_check(PHP_SAPI === 'cli' && ($_SERVER['argc'] ?? 0) === 1, 'cli_entry_guard');
    $wire = stream_get_contents(STDIN, 4097);
    rp_check(is_string($wire) && strlen($wire) <= 4096, 'input_quota');
    try {
        $input = json_decode($wire, true, 16, JSON_THROW_ON_ERROR);
    } catch (JsonException $error) {
        throw new RpNativeFailure('invalid_input_json');
    }
    rp_check(is_array($input), 'input_object_required');
    $keys = array_keys($input);
    sort($keys);
    rp_check($keys === ['database', 'host', 'password', 'port', 'schema', 'tls', 'tls_ca_pem', 'user'], 'input_exact_keys');
    rp_check($input['schema'] === 'recastpress.wpdb-native-input.v0.5'
        && $input['host'] === '127.0.0.1'
        && $input['database'] === 'rp_wpdb_probe'
        && $input['user'] === 'rp_wpdb_probe', 'input_closed_destination');
    rp_check(is_int($input['port']) && $input['port'] >= 1 && $input['port'] <= 65535, 'input_port');
    rp_check(is_string($input['password']) && preg_match('/\A[0-9a-f]{48}\z/D', $input['password']) === 1, 'input_secret_shape');
    rp_check($input['tls'] === 'disabled' && $input['tls_ca_pem'] === null, 'native_non_tls_profile_only');
    return $input;
}

function rp_state(wpdb $db, int $before): array {
    return [
        'num_queries' => $db->num_queries,
        'delta' => $db->num_queries - $before,
        'num_rows' => $db->num_rows,
        'rows_affected' => $db->rows_affected,
        'insert_id' => $db->insert_id,
        'last_error_hex' => bin2hex($db->last_error),
        'last_query_hex' => bin2hex((string) $db->last_query),
    ];
}

function rp_observe(wpdb $db, array &$report, string $case, string $api, ?string $query, Closure $operation, int $delta = 1): mixed {
    $before = $db->num_queries;
    $returned = $operation();
    $state = rp_state($db, $before);
    $report['observations'][] = [
        'case' => $case,
        'api' => $api,
        'query_hex' => $query === null ? null : bin2hex($query),
        'return_type' => get_debug_type($returned),
        'returned' => rp_value($returned),
        'state' => $state,
    ];
    rp_check($state['delta'] === $delta, 'num_queries_delta_mismatch');
    return $returned;
}

function rp_query(wpdb $db, array &$report, string $case, string $query): int|bool {
    return rp_observe($db, $report, $case, 'query', $query, fn() => $db->query($query));
}

function rp_included_sources(): array {
    $relative = ['experiments/wpdb-rpc-probe/fixtures/native_wpdb.php'];
    $expected = [realpath(__FILE__)];
    foreach (['load.php', 'plugin.php', 'class-wp-hook.php', 'class-wp-filter-sentinel.php', 'class-wpdb.php'] as $name) {
        $relative[] = '.reference/wordpress-7.1.3/wp-includes/' . $name;
        $expected[] = realpath(ABSPATH . WPINC . '/' . $name);
    }
    rp_check(!in_array(false, $expected, true), 'included_source_missing');
    $actual = array_map('realpath', get_included_files());
    sort($expected, SORT_STRING);
    sort($actual, SORT_STRING);
    rp_check($actual === $expected, 'included_source_set_mismatch');
    sort($relative, SORT_STRING);
    return $relative;
}

$report = [
    'schema' => 'recastpress.wpdb-native-observation.v0.5',
    'outcome' => 'not_run',
    'passed' => false,
    'connected' => false,
    'constructor_connection_deferred' => false,
    'suppress_errors_previous' => null,
    'close_completed' => false,
    'tls_profile' => 'disabled',
    'observations' => [],
    'full_wordpress_bootstrap' => false,
    'rust_driver_executed' => false,
    'database_started_by_fixture' => false,
    'ddl_executed_by_fixture' => false,
    'native_tls_verified' => false,
    'raw_non_utf8_err_verified' => false,
    'standard_wpdb_compatibility_verified' => false,
    'product_driver_selected' => false,
    'included_source_gate_passed' => false,
];
$db = null;
$wp_sources_loaded = false;
set_error_handler(static function (int $severity, string $message, string $file, int $line): never {
    // A warning fails the reference without exposing input, SQL, credentials or paths.
    throw new RpNativeFailure('php_warning');
});

try {
    $input = rp_input();
    rp_check(PHP_VERSION_ID === 80512 && PHP_INT_SIZE === 8, 'fixed_native_php_version');
    rp_check(extension_loaded('mysqli'), 'native_mysqli_required');
    $report['runtime'] = ['version' => PHP_VERSION, 'version_id' => PHP_VERSION_ID, 'int_size' => PHP_INT_SIZE, 'sapi' => PHP_SAPI];

    define('ABSPATH', __DIR__ . '/../../../.reference/wordpress-7.1.3/');
    define('WPINC', 'wp-includes');
    define('WP_SETUP_CONFIG', true);
    define('WP_DEBUG', false);
    define('WP_DEBUG_DISPLAY', false);
    define('DB_CHARSET', 'utf8mb4');
    define('DB_COLLATE', '');
    define('MYSQL_CLIENT_FLAGS', 0);
    // load.php supplies the real absint()/is_multisite() used by wpdb.
    // plugin.php itself loads the original hook and filter-sentinel classes.
    require ABSPATH . WPINC . '/load.php';
    require ABSPATH . WPINC . '/plugin.php';
    require ABSPATH . WPINC . '/class-wpdb.php';
    $wp_sources_loaded = true;
    $report['included_source_names'] = rp_included_sources();
    $report['included_source_gate_passed'] = true;
    $report['loaded_source_sha256'] = [];
    foreach (['load.php', 'plugin.php', 'class-wp-hook.php', 'class-wp-filter-sentinel.php', 'class-wpdb.php'] as $name) {
        $report['loaded_source_sha256']['wp-includes/' . $name] = hash_file('sha256', ABSPATH . WPINC . '/' . $name);
    }

    $db = new wpdb($input['user'], $input['password'], $input['database'], '127.0.0.1:' . $input['port']);
    $report['constructor_connection_deferred'] = !$db->ready && $db->dbh === null;
    rp_check($report['constructor_connection_deferred'], 'constructor_connected_early');
    $report['suppress_errors_previous'] = $db->suppress_errors(true);
    rp_check($db->db_connect(false) === true && $db->ready && $db->dbh instanceof mysqli, 'native_db_connect_failed');
    $report['connected'] = true;
    $report['server_info_hex'] = bin2hex($db->db_server_info());
    $report['capabilities'] = [];
    foreach (['collation', 'set_charset', 'utf8mb4', 'identifier_placeholders'] as $capability) {
        $report['capabilities'][$capability] = $db->has_cap($capability);
    }
    $report['bootstrap_num_queries'] = $db->num_queries;

    $old_prefix = $db->set_prefix('rp_');
    $report['prefix'] = ['previous' => rp_value($old_prefix), 'prefix_hex' => bin2hex($db->prefix),
        'base_prefix_hex' => bin2hex($db->base_prefix), 'posts_hex' => bin2hex($db->posts), 'users_hex' => bin2hex($db->users)];
    rp_check($db->prefix === 'rp_' && $db->base_prefix === 'rp_' && $db->posts === 'rp_posts' && $db->users === 'rp_users', 'prefix_contract');

    $sql = "SHOW SESSION STATUS LIKE 'Ssl_cipher'";
    $tls = rp_observe($db, $report, 'native_tls', 'get_row/ARRAY_N', $sql, fn() => $db->get_row($sql, ARRAY_N));
    rp_check($tls === ['Ssl_cipher', ''], 'native_expected_non_tls');
    $sql = 'SELECT COUNT(*) FROM rp_wire_fixture';
    $empty = rp_observe($db, $report, 'fixture_empty', 'get_var', $sql, fn() => $db->get_var($sql));
    rp_check($empty === '0', 'fixture_not_empty');
    $sql = "INSERT INTO rp_wire_fixture(marker,payload,dec_value,big_value) VALUES('rp-wire',X'00ff41',12345678901234567890.1234567890,18446744073709551615)";
    rp_check(rp_query($db, $report, 'insert', $sql) === 1 && is_int($db->insert_id) && $db->insert_id > 0, 'insert_contract');

    $sql = "SELECT payload,NULL AS null_value,X'' AS empty_value,dec_value,big_value FROM rp_wire_fixture WHERE marker='rp-wire'";
    $expected = ['payload' => "\x00\xffA", 'null_value' => null, 'empty_value' => '',
        'dec_value' => '12345678901234567890.1234567890', 'big_value' => '18446744073709551615'];
    $row = rp_observe($db, $report, 'get_row_object', 'get_row/OBJECT', $sql, fn() => $db->get_row($sql, OBJECT));
    rp_check($row instanceof stdClass && get_object_vars($row) === $expected, 'object_row_contract');
    $row = rp_observe($db, $report, 'get_row_array_a', 'get_row/ARRAY_A', $sql, fn() => $db->get_row($sql, ARRAY_A));
    rp_check($row === $expected, 'associative_row_contract');
    $row = rp_observe($db, $report, 'get_row_array_n', 'get_row/ARRAY_N', $sql, fn() => $db->get_row($sql, ARRAY_N));
    rp_check($row === array_values($expected), 'numeric_row_contract');
    foreach ([OBJECT, ARRAY_A, ARRAY_N] as $mode) {
        $rows = rp_observe($db, $report, 'get_results_' . strtolower($mode), 'get_results/' . $mode, $sql, fn() => $db->get_results($sql, $mode));
        $actual = $mode === OBJECT ? array_map('get_object_vars', $rows) : $rows;
        rp_check($actual === [$mode === ARRAY_N ? array_values($expected) : $expected], 'results_mode_contract');
    }
    $sql = "SELECT marker,payload FROM rp_wire_fixture WHERE marker='rp-wire'";
    $keyed = rp_observe($db, $report, 'get_results_object_k', 'get_results/OBJECT_K', $sql, fn() => $db->get_results($sql, OBJECT_K));
    rp_check(array_keys($keyed) === ['rp-wire'] && get_object_vars($keyed['rp-wire']) === ['marker' => 'rp-wire', 'payload' => "\x00\xffA"], 'keyed_results_contract');
    $sql = "SELECT payload FROM rp_wire_fixture WHERE marker='rp-wire'";
    $col = rp_observe($db, $report, 'get_col_binary', 'get_col', $sql, fn() => $db->get_col($sql));
    rp_check($col === ["\x00\xffA"], 'binary_col_contract');
    $col = rp_observe($db, $report, 'get_col_cached', 'get_col/null', null, fn() => $db->get_col(null), 0);
    rp_check($col === ["\x00\xffA"], 'cached_col_contract');
    foreach (['SELECT NULL' => null, "SELECT X''" => null, 'SELECT 17' => '17'] as $sql => $expected_var) {
        $value = rp_observe($db, $report, 'get_var_' . ($expected_var === null ? (str_contains($sql, 'NULL') ? 'null' : 'empty') : 'integer_string'), 'get_var', $sql, fn() => $db->get_var($sql));
        rp_check($value === $expected_var, 'get_var_contract');
    }
    $sql = 'SELECT payload,NULL AS null_value FROM rp_wire_fixture WHERE 1=0';
    $rows = rp_observe($db, $report, 'zero_rows', 'get_results/ARRAY_N', $sql, fn() => $db->get_results($sql, ARRAY_N));
    rp_check($rows === [] && $db->num_rows === 0, 'zero_rows_contract');
    $columns = rp_observe($db, $report, 'zero_row_column_names', 'get_col_info/name', null, fn() => $db->get_col_info('name'), 0);
    rp_check($columns === ['payload', 'null_value'], 'zero_row_columns_lost');

    // SQL remains ASCII. Binary input is passed as HEX text through real prepare().
    $prepared = $db->prepare('SELECT UNHEX(%s) AS escaped_value,%d AS integer_value,%i AS payload_column FROM %i WHERE marker=%s',
        bin2hex("quote'\\\x00\xff"), 7, 'payload', 'rp_wire_fixture', 'rp-wire');
    rp_check(is_string($prepared) && preg_match('/\A[\x00-\x7f]*\z/D', $prepared) === 1, 'prepared_ascii_required');
    $report['prepare'] = ['returned_sql_hex' => bin2hex($prepared), 'placeholder_removed_sql_hex' => bin2hex($db->remove_placeholder_escape($prepared))];
    $prepared_row = rp_observe($db, $report, 'prepare_identifiers_and_values', 'get_row/ARRAY_N', $prepared, fn() => $db->get_row($prepared, ARRAY_N));
    rp_check($prepared_row === ["quote'\\\x00\xff", '7', "\x00\xffA"], 'prepare_hex_round_trip');
    $escape_input = "quote'\\_";
    $escaped_sql = $db->prepare('SELECT %s AS escaped_value', $escape_input);
    $escaped = rp_observe($db, $report, 'prepare_escape', 'get_var', $escaped_sql, fn() => $db->get_var($escaped_sql));
    rp_check($escaped === $escape_input, 'prepare_escape_round_trip');

    rp_check(rp_query($db, $report, 'same_value_update', "UPDATE rp_wire_fixture SET payload=X'00ff41' WHERE marker='rp-wire'") === 0 && $db->rows_affected === 0, 'same_update_contract');
    rp_check(rp_query($db, $report, 'changed_update', "UPDATE rp_wire_fixture SET payload=X'00ff42' WHERE marker='rp-wire'") === 1 && $db->rows_affected === 1, 'changed_update_contract');
    rp_check(rp_query($db, $report, 'ascii_syntax_error', 'SELEC rp_fixed_ascii_error') === false && $db->last_error !== '', 'syntax_error_contract');
    $report['syntax_error_code'] = mysqli_errno($db->dbh);
    $report['syntax_error_state'] = mysqli_sqlstate($db->dbh);
    rp_check($report['syntax_error_code'] === 1064 && $report['syntax_error_state'] === '42000', 'syntax_error_identity');
    $sql = 'SELECT 23';
    rp_check(rp_observe($db, $report, 'error_recovery', 'get_var', $sql, fn() => $db->get_var($sql)) === '23' && $db->last_error === '', 'error_recovery_contract');
    rp_check(rp_query($db, $report, 'begin', 'BEGIN') === 0, 'begin_contract');
    rp_check(rp_query($db, $report, 'transaction_update', "UPDATE rp_wire_fixture SET payload=X'00ff43' WHERE marker='rp-wire'") === 1, 'transaction_update_contract');
    rp_check(rp_query($db, $report, 'rollback', 'ROLLBACK') === 0, 'rollback_contract');
    $sql = "SELECT payload FROM rp_wire_fixture WHERE marker='rp-wire'";
    rp_check(rp_observe($db, $report, 'rollback_read', 'get_var', $sql, fn() => $db->get_var($sql)) === "\x00\xffB", 'rollback_read_contract');
    $report['final_num_queries'] = $db->num_queries;
    $report['outcome'] = 'fixed_native_wpdb_cases_passed';
    $report['passed'] = true;
} catch (Throwable $error) {
    $report['outcome'] = 'fixed_native_wpdb_cases_failed';
    $report['failure'] = $error instanceof RpNativeFailure ? $error->getMessage() : 'php_throwable';
    $report['exception_class'] = get_class($error);
} finally {
    if ($db instanceof wpdb && $db->dbh instanceof mysqli) {
        try {
            $report['close_completed'] = $db->close();
        } catch (Throwable $error) {
            $report['close_completed'] = false;
        }
        if (!$report['close_completed']) {
            $report['passed'] = false;
            $report['outcome'] = 'fixed_native_wpdb_cases_failed';
            $report['failure'] = 'native_close_failed';
        }
    }
    if ($wp_sources_loaded) {
        try {
            $report['included_source_names'] = rp_included_sources();
            $report['included_source_gate_passed'] = true;
        } catch (Throwable $error) {
            $report['included_source_gate_passed'] = false;
            $report['passed'] = false;
            $report['outcome'] = 'fixed_native_wpdb_cases_failed';
            $report['failure'] ??= 'included_source_set_mismatch';
        }
    }
    restore_error_handler();
}
echo json_encode($report, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES), "\n";
exit($report['passed'] ? 0 : 1);
