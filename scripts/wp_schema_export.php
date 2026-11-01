<?php
/** Evaluate only the pinned SQL builder with explicit single-site fixture context. */
error_reporting(E_ALL);
set_error_handler(static function ($level, $message, $file, $line) { throw new ErrorException($message, 0, $level, $file, $line); });
require __DIR__ . '/reference_functions.php';

final class SchemaFixtureWpdb {
    public int $blogid = 1;
    public string $prefix = 'wp_';
    public function get_charset_collate() { return 'DEFAULT CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci'; }
    public function __get($table) {
        if (!preg_match('/^[a-z_]+$/', $table)) { throw new RuntimeException('unexpected table name'); }
        return $this->prefix . $table;
    }
}

function is_multisite() { return false; }
$wpdb = new SchemaFixtureWpdb();
$source = $argv[1] ?? throw new RuntimeException('missing pinned source root');
load_reference_function($source . '/wp-admin/includes/schema.php', 'wp_get_db_schema');
$sql = wp_get_db_schema('all');
preg_match_all('/CREATE TABLE ([a-z0-9_]+) \(/i', $sql, $tables);
echo json_encode(['php_version' => PHP_VERSION, 'context' => ['is_multisite' => false, 'blog_id' => 1, 'table_prefix' => $wpdb->prefix, 'charset_collate' => $wpdb->get_charset_collate()], 'tables' => $tables[1], 'sql' => $sql], JSON_THROW_ON_ERROR), "\n";
