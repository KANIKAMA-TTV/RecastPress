"""Verify native mysqli/wpdb comparison inputs; never a product PHP fallback."""
import hashlib
import io
import json
import os
from pathlib import Path
import sys
import tarfile
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PHP = ROOT / '.reference/php-8.5.12'
WP = ROOT / '.reference/wordpress-7.1.3'
PHP_SHA = '2ab242af38d76bdf417cde5ec048b1bcd76a5a40f39a3d15d5f046f38571c477'
WP_SHA = 'd2a09acb6a15e3b9c471d72557753c266d6f41a79ed19bfe04cbfe49e283b2a5'
WP_FILES = ['wp-includes/load.php', 'wp-includes/plugin.php', 'wp-includes/class-wp-hook.php',
    'wp-includes/class-wp-filter-sentinel.php', 'wp-includes/class-wpdb.php']
FIXTURE = HERE / 'fixtures/native_wpdb.php'


def require(value, reason):
    if not value:
        raise ValueError(reason)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def read(path, maximum):
    require(path.resolve(strict=True) == path and path.is_file(), 'native input missing or linked')
    require(path.stat().st_size <= maximum, 'native input quota')
    wire = path.read_bytes()
    require(len(wire) <= maximum, 'native input grew')
    return wire


def references():
    require(os.name == 'nt', 'native comparison pins Windows PHP')
    archive = ROOT / '.reference/php-8.5.12-nts-Win32-vs17-x64.zip'
    wire = read(archive, 40 * 1024 * 1024)
    require(sha(wire) == PHP_SHA, 'native PHP archive identity')
    expected = {}
    with zipfile.ZipFile(io.BytesIO(wire)) as zipped:
        require(len(zipped.infolist()) < 128, 'native PHP member quota')
        for member in zipped.infolist():
            name = member.filename.rstrip('/')
            parts = name.split('/')
            require(name and '\\' not in name and all(p not in ('', '.', '..') and ':' not in p for p in parts),
                'native PHP archive path')
            if member.is_dir():
                continue
            require(name not in expected and member.file_size <= 64 * 1024 * 1024, 'native PHP member size/duplicate')
            content = zipped.read(member)
            expected[name] = sha(content)
            require(read(PHP.joinpath(*parts), 64 * 1024 * 1024) == content, 'native PHP file differs from archive')
    actual = set()
    for path in PHP.rglob('*'):
        require(path.resolve() == path, 'native PHP linked entry')
        if path.is_file() and path != PHP / 'recastpress-tool.json':
            actual.add(path.relative_to(PHP).as_posix())
    require(actual == set(expected) and len(expected) == 78, 'native PHP complete file set')
    manifest = json.loads(read(PHP / 'recastpress-tool.json', 128 * 1024))
    require(manifest['php_version'] == '8.5.12' and manifest['archive_sha256'] == PHP_SHA
        and len(manifest['files']) == len(expected)
        and {r['path']: r['sha256'] for r in manifest['files']} == expected, 'native PHP manifest differs from archive')
    require({'php.exe', 'php8.dll', 'ext/php_mysqli.dll'} <= set(expected), 'native mysqli input absent')

    archive = ROOT / '.reference/wordpress-7.1.3.tar.gz'
    wire = read(archive, 40 * 1024 * 1024)
    require(sha(wire) == WP_SHA, 'native WordPress archive identity')
    sources = {}
    with tarfile.open(fileobj=io.BytesIO(wire), mode='r:gz') as tar:
        members = tar.getmembers()
        require(len(members) < 5000, 'native WordPress member quota')
        for relative in WP_FILES:
            selected = [m for m in members if m.name == 'wordpress/' + relative]
            require(len(selected) == 1 and selected[0].isfile() and selected[0].size <= 512 * 1024,
                'native WordPress fixed source member')
            with tar.extractfile(selected[0]) as stream:
                content = stream.read(512 * 1024 + 1)
            require(content == read(WP / relative, 512 * 1024), 'native WordPress source differs from archive')
            sources[relative] = sha(content)
    return {'php_archive_sha256': PHP_SHA, 'php_verified_file_count': len(expected),
        'php_file_sha256': expected, 'wordpress_archive_sha256': WP_SHA,
        'wordpress_loaded_source_sha256': sources, 'full_wp_bootstrap_verified': False}


def environment():
    excluded = {'PHPRC', 'PHP_INI_SCAN_DIR', 'RUST_LOG', 'RUST_BACKTRACE',
        'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN'}
    return {k: v for k, v in os.environ.items() if not k.upper().startswith(('MYSQL_', 'RP_'))
        and k.upper() not in excluded}


def command():
    return [str(PHP / 'php.exe'), '-n', '-d', 'display_errors=0', '-d', 'log_errors=0',
        '-d', 'extension_dir=' + str(PHP / 'ext'), '-d', 'extension=mysqli', str(FIXTURE)]


def validate_observation(value, reference):
    require(value['schema'] == 'recastpress.wpdb-native-observation.v0.5', 'native output schema')
    for key in ('passed', 'connected', 'constructor_connection_deferred', 'close_completed', 'included_source_gate_passed'):
        require(value[key] is True, 'native completion contract')
    for key in ('full_wordpress_bootstrap', 'rust_driver_executed', 'database_started_by_fixture',
            'ddl_executed_by_fixture', 'native_tls_verified', 'raw_non_utf8_err_verified',
            'standard_wpdb_compatibility_verified', 'product_driver_selected'):
        require(value[key] is False, 'native scope contract')
    require(value['outcome'] == 'fixed_native_wpdb_cases_passed' and value['tls_profile'] == 'disabled',
        'native fixed outcome/profile')
    require(value['runtime'] == {'version': '8.5.12', 'version_id': 80512, 'int_size': 8, 'sapi': 'cli'},
        'native runtime identity')
    require(value['loaded_source_sha256'] == reference['wordpress_loaded_source_sha256'], 'native loaded source identity')
    names = ['native_tls', 'fixture_empty', 'insert', 'get_row_object', 'get_row_array_a', 'get_row_array_n',
        'get_results_object', 'get_results_array_a', 'get_results_array_n', 'get_results_object_k',
        'get_col_binary', 'get_col_cached', 'get_var_null', 'get_var_empty', 'get_var_integer_string',
        'zero_rows', 'zero_row_column_names', 'prepare_identifiers_and_values', 'prepare_escape',
        'same_value_update', 'changed_update', 'ascii_syntax_error', 'error_recovery', 'begin',
        'transaction_update', 'rollback', 'rollback_read']
    rows = value['observations']
    require(type(rows) is list and [r['case'] for r in rows] == names, 'native complete case order')
    by_name = {r['case']: r for r in rows}
    count = value['bootstrap_num_queries']
    require(type(count) is int and count >= 0, 'native initial query count')
    for row in rows:
        delta = 0 if row['case'] in ('get_col_cached', 'zero_row_column_names') else 1
        state = row['state']
        require(type(state['delta']) is int and state['delta'] == delta, 'native actual query delta')
        count += delta
        require(type(state['num_queries']) is int and state['num_queries'] == count, 'native query count progression')
    require(value['final_num_queries'] == count, 'native final query count')
    for name, returned in [('fixture_empty', {'bytes_hex': '30'}), ('get_var_null', None),
            ('get_var_empty', None), ('get_var_integer_string', {'bytes_hex': '3137'}),
            ('rollback_read', {'bytes_hex': '00ff42'}), ('error_recovery', {'bytes_hex': '3233'}),
            ('prepare_escape', {'bytes_hex': b"quote'\\_".hex()})]:
        require(by_name[name]['returned'] == returned, 'native scalar bytes/NULL')
    for name, returned in [('insert', 1), ('same_value_update', 0), ('changed_update', 1),
            ('ascii_syntax_error', False), ('begin', 0), ('transaction_update', 1), ('rollback', 0)]:
        actual = by_name[name]['returned']
        require(type(actual) is type(returned) and actual == returned, 'native mutation return type/value')
    require(value['syntax_error_code'] == 1064 and value['syntax_error_state'] == '42000', 'native SQL error identity')
    require(by_name['ascii_syntax_error']['state']['last_error_hex']
        and by_name['error_recovery']['state']['last_error_hex'] == '', 'native error recovery state')
    array = by_name['get_row_array_n']['returned']
    require(array == {'type': 'array', 'entries': [{'key': i, 'value': v} for i, v in enumerate([
        {'bytes_hex': '00ff41'}, None, {'bytes_hex': ''},
        {'bytes_hex': b'12345678901234567890.1234567890'.hex()},
        {'bytes_hex': b'18446744073709551615'.hex()}])]}, 'native ordered exact bytes')
    def byte(wire):
        return {'bytes_hex': wire.hex()}
    def array_value(entries):
        return {'type': 'array', 'entries': [{'key': key, 'value': item} for key, item in entries]}
    def indexed(items):
        return array_value(enumerate(items))
    values = [byte(b'\x00\xffA'), None, byte(b''), byte(b'12345678901234567890.1234567890'),
        byte(b'18446744073709551615')]
    columns = [b'payload', b'null_value', b'empty_value', b'dec_value', b'big_value']
    object_row = {'type': 'stdClass', 'properties': [
        {'name_hex': name.hex(), 'value': item} for name, item in zip(columns, values)]}
    assoc_row = array_value((byte(name), item) for name, item in zip(columns, values))
    numeric_row = indexed(values)
    keyed_row = {'type': 'stdClass', 'properties': [
        {'name_hex': b'marker'.hex(), 'value': byte(b'rp-wire')},
        {'name_hex': b'payload'.hex(), 'value': byte(b'\x00\xffA')}]}
    expected = {'native_tls': indexed([byte(b'Ssl_cipher'), byte(b'')]),
        'get_row_object': object_row, 'get_row_array_a': assoc_row, 'get_row_array_n': numeric_row,
        'get_results_object': indexed([object_row]), 'get_results_array_a': indexed([assoc_row]),
        'get_results_array_n': indexed([numeric_row]),
        'get_results_object_k': array_value([(byte(b'rp-wire'), keyed_row)]),
        'get_col_binary': indexed([byte(b'\x00\xffA')]), 'get_col_cached': indexed([byte(b'\x00\xffA')]),
        'zero_rows': indexed([]), 'zero_row_column_names': indexed([byte(b'payload'), byte(b'null_value')]),
        'prepare_identifiers_and_values': indexed([byte(b"quote'\\\x00\xff"), byte(b'7'), byte(b'\x00\xffA')])}
    for name, returned in expected.items():
        require(by_name[name]['returned'] == returned, 'native collection shape/bytes')
    for key, wire in [('prefix_hex', b'rp_'), ('base_prefix_hex', b'rp_'),
            ('posts_hex', b'rp_posts'), ('users_hex', b'rp_users')]:
        require(value['prefix'][key] == wire.hex(), 'native prefix state')
    require(all(value['capabilities'].get(name) is True for name in
        ['collation', 'set_charset', 'utf8mb4', 'identifier_placeholders']), 'native capability baseline')
    return {'fixed_native_api_cases': len(names), 'upstream_wpdb_baseline_passed': True,
        'native_non_tls_verified': True, 'full_wp_bootstrap_verified': False,
        'rust_php_adapter_compared': False, 'standard_wpdb_compatibility_verified': False}


if __name__ == '__main__':
    require(sys.argv[1:] == ['--verify-inputs'], 'usage: native_check.py --verify-inputs')
    references()
    print('Native wpdb references: pinned PHP 78 files and upstream WP 5 files verified; DB not connected')
