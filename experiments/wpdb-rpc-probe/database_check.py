"""Own one fresh pinned Windows DB for a closed physical-session wire trial.

Four independent profiles are selected by --backend and --tls. Existing DBs,
services, production schema/features and arbitrary executables are not accepted.
The optional native mode runs only the fixed upstream-wpdb comparison fixture.
Secrets/configs/logs stay in the retained ignored target run.
Every public result remains a fixture observation, never product adoption.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import secrets
import socket
import stat
import subprocess
import sys
import time
import uuid
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
RUNS = ROOT / 'target/wpdb-rpc-probe/database-runs'
TARGET = ROOT / 'target/wpdb-rpc-wire-probe'
GPG = Path('C:/Program Files/Git/usr/bin/gpg.exe')
OPENSSL = Path('C:/Program Files/Git/usr/bin/openssl.exe')
MYSQL_SIGNER = 'BCA43417C3B485DD128EC6D4B7B3B788A8D3785C'
PINS = {
    'mysql': {'version': '8.4.13', 'archive_sha256': 'e9cd98b1d23f122987b1afca26ad942efec5891481103d48ae3cde237fa3ea54',
        'files': 296, 'server': 'mysqld.exe', 'client': 'mysql.exe', 'admin': 'mysqladmin.exe'},
    'mariadb': {'version': '11.8.9', 'archive_sha256': '830c46727d9278eae212ae3eca44eeb9e71b2a68704e95f344a64fba7b1963f5',
        'files': 624, 'server': 'mariadbd.exe', 'client': 'mariadb.exe', 'admin': 'mariadb-admin.exe'},
}
DDL = b"""CREATE DATABASE rp_wpdb_probe CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci;
CREATE TABLE rp_wpdb_probe.rp_wire_fixture(
id BIGINT UNSIGNED AUTO_INCREMENT PRIMARY KEY,
marker VARBINARY(32) NOT NULL UNIQUE,payload VARBINARY(64) NULL,
dec_value DECIMAL(30,10) NOT NULL,big_value BIGINT UNSIGNED NOT NULL) ENGINE=InnoDB;
"""


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
    return module


wire_check = load('rp_owned_database_wire_build_gate', HERE / 'wire_check.py')
sys.path.insert(0, str(ROOT / 'scripts'))
import wordpress_reference as reference


def require(ok, reason):
    if not ok: raise ValueError(reason)


def equal(actual, expected, reason):
    require(type(actual) is type(expected) and actual == expected, reason)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def digest(path):
    return reference.digest_file(path)


def parse_json(wire):
    def pairs(rows):
        value = {}
        for name, item in rows:
            require(name not in value, 'duplicate_json_key'); value[name] = item
        return value
    def bad_number(_): raise ValueError('non_finite_json')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=pairs, parse_constant=bad_number)


def fixed(path, maximum=512 * 1024 * 1024):
    path = Path(path)
    require(path.is_absolute() and path.resolve(strict=True) == path, 'fixed_path_linked_or_missing')
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and 0 < metadata.st_size <= maximum
        and not getattr(metadata, 'st_file_attributes', 0) & 0x400, 'fixed_file_type_or_quota')
    return path


def owned(path):
    path = Path(path)
    require(path.is_absolute() and path.resolve() == path and path.is_relative_to(RUNS), 'owned_run_path')
    for parent in [RUNS, *path.parents]:
        if parent.exists() and parent.is_relative_to(ROOT / 'target'):
            require(not parent.lstat().st_file_attributes & 0x400 if os.name == 'nt' else not parent.is_symlink(), 'owned_parent_linked')
    return path


def environment(extra=None):
    values = dict(os.environ)
    if extra: values.update(extra)
    return {key: value for key, value in values.items() if not key.upper().startswith(('MYSQL_', 'MARIADB_'))
        and key.upper() not in {'GH_TOKEN', 'GITHUB_TOKEN', 'ACTIONS_RUNTIME_TOKEN', 'RUST_LOG', 'RUST_BACKTRACE',
            'SSLKEYLOGFILE', 'RANDFILE', 'OPENSSL_CONF', 'OPENSSL_MODULES'}}


def write_private(path, wire):
    path = owned(path)
    require(type(wire) is bytes and len(wire) <= 256 * 1024, 'private_input_quota')
    with path.open('xb') as stream: stream.write(wire)
    return path


def stop_process(child, *, normal_wait=0):
    """Use only the captured handle/PID, never enumerate or reuse other DBs."""
    tree_kill = False; tree_exit = None; tree_error = None; direct_kill = False
    if child.poll() is None and normal_wait:
        try: child.wait(timeout=normal_wait)
        except subprocess.TimeoutExpired: pass
    if child.poll() is None:
        # Do not reap a live leader before Windows captures its process tree.
        tree_kill = True
        try:
            tree = subprocess.run(['taskkill.exe', '/PID', str(child.pid), '/T', '/F'], stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, env=environment(), timeout=10,
                creationflags=subprocess.CREATE_NO_WINDOW, check=False)
            tree_exit = tree.returncode
        except BaseException as error:
            # A failed tree-cleanup utility must not skip the owned-handle kill
            # and bounded reap. No command/exception text enters public output.
            tree_error = type(error).__name__
        try: child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            direct_kill = True
            try: child.kill()
            except OSError:
                if child.poll() is None: raise
            child.wait(timeout=5)
    else: child.wait(timeout=5)
    require(child.returncode is not None, 'owned_process_not_reaped')
    return {'pid': child.pid, 'reaped': True, 'exit_code': child.returncode, 'forced_tree_cleanup': tree_kill,
        'tree_cleanup_command_exit_code': tree_exit, 'tree_cleanup_failure_class': tree_error,
        'direct_captured_kill_used': direct_kill, 'independent_descendant_inventory_verified': False}


class Harness:
    def __init__(self, work, report):
        self.work, self.report = work, report
        self.server = None; self.log = None
        self.owned_children = {}

    def remember(self, child, row=None):
        self.owned_children[id(child)] = (child, row)
        return child

    def cleanup(self, child, *, normal_wait=0):
        result = stop_process(child, normal_wait=normal_wait)
        _, row = self.owned_children.get(id(child), (child, None))
        if row is not None:
            row['cleanup'] = result; row['owned_reaped'] = True
        self.owned_children.pop(id(child), None)
        return result

    def reap_remaining(self):
        failed = False
        for child, row in reversed(list(self.owned_children.values())):
            try: self.cleanup(child)
            except BaseException:
                failed = True
                if row is not None: row['cleanup_failed'] = True
        require(not failed and not self.owned_children, 'remaining_owned_process_cleanup_failed')

    def run(self, label, command, *, timeout, input_bytes=None, cwd=None, limit=256 * 1024, extra_env=None):
        """Finite wait and live file-capture quota; no secret-bearing diagnostics."""
        token = uuid.uuid4().hex
        out = owned(self.work / (token + '.stdout')); err = owned(self.work / (token + '.stderr'))
        source = None
        if input_bytes is not None:
            source = write_private(self.work / (token + '.stdin'), input_bytes).open('rb')
        child = None; original = None; row = {'stage': label, 'timeout_seconds': timeout, 'owned_reaped': False}
        self.report['commands'].append(row)
        try:
            with out.open('xb') as stdout, err.open('xb') as stderr:
                child = subprocess.Popen(command, cwd=cwd or self.work, env=environment(extra_env),
                    stdin=source if source is not None else subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                    creationflags=subprocess.CREATE_NO_WINDOW | subprocess.CREATE_NEW_PROCESS_GROUP)
                self.remember(child, row)
                row['pid'] = child.pid
                deadline = time.monotonic() + timeout
                while child.poll() is None:
                    require(out.stat().st_size <= limit and err.stat().st_size <= limit, 'child_capture_quota')
                    if time.monotonic() >= deadline: raise TimeoutError('owned_child_deadline')
                    time.sleep(0.02)
                child.wait(timeout=5)
            require(out.stat().st_size <= limit and err.stat().st_size <= limit, 'child_capture_quota')
            stdout, stderr = out.read_bytes(), err.read_bytes()
            row.update(exit_code=child.returncode, stdout_bytes=len(stdout), stdout_sha256=sha(stdout),
                stderr_bytes=len(stderr), stderr_sha256=sha(stderr))
            return subprocess.CompletedProcess(command, child.returncode, stdout, stderr)
        except BaseException as error:
            original = error; raise
        finally:
            if source is not None: source.close()
            if child is not None:
                try:
                    self.cleanup(child)
                except BaseException:
                    row['cleanup_failed'] = True
                    if original is None: raise

    def checked(self, label, command, **kwargs):
        result = self.run(label, command, **kwargs)
        require(result.returncode == 0, label + '_failed')
        return result


def verify_tool(backend, harness):
    pin = PINS[backend]; name = backend + '-' + pin['version']
    directory = ROOT / '.reference' / name
    archive = fixed(ROOT / '.reference' / (name + '-winx64.zip'))
    require(directory.resolve(strict=True) == directory, 'tool_directory_linked')
    equal(digest(archive), pin['archive_sha256'], 'fixed_db_archive_changed')
    manifest_path = fixed(directory / 'recastpress-tool.json', 256 * 1024)
    manifest = parse_json(manifest_path.read_bytes())
    equal(manifest['tool_id'], name, 'fixed_tool_id')
    equal(manifest['archive_sha256'], pin['archive_sha256'], 'fixed_manifest_archive')
    expected = {}; seen = set()
    with zipfile.ZipFile(archive) as zipped:
        rows = zipped.infolist()
        require(len(rows) <= 20000 and sum(row.file_size for row in rows) <= 2 * 1024**3, 'tool_archive_expansion_quota')
        for row in rows:
            path = reference.safe_relative('wordpress/' + row.filename)
            require(path.parts and path.parts[0] == name + '-winx64', 'fixed_tool_zip_root')
            parts = path.parts[1:]
            if not parts:
                require(row.is_dir(), 'tool_zip_root_file'); continue
            key = '/'.join(parts)
            require(key.casefold() not in seen and not stat.S_ISLNK(row.external_attr >> 16)
                and not row.flag_bits & 1, 'tool_zip_duplicate_link_or_encryption')
            seen.add(key.casefold())
            if row.is_dir(): continue
            digestor = hashlib.sha256(); count = 0
            with zipped.open(row) as stream:
                for chunk in iter(lambda: stream.read(1024 * 1024), b''): count += len(chunk); digestor.update(chunk)
            require(count == row.file_size, 'tool_zip_member_changed')
            expected[key] = digestor.hexdigest()
    require(len(expected) == pin['files'], 'tool_full_file_count')
    entries = manifest['files']; require(type(entries) is list and len(entries) == len(expected), 'tool_manifest_full_inventory')
    observed = {}
    for row in entries:
        key = row['path']; require(key not in observed, 'tool_manifest_duplicate')
        observed[key] = row['sha256']
    equal(observed, expected, 'tool_manifest_vs_verified_archive')
    installed = {}
    for path in directory.rglob('*'):
        metadata = path.lstat()
        require(not stat.S_ISLNK(metadata.st_mode) and not getattr(metadata, 'st_file_attributes', 0) & 0x400,
            'tool_tree_linked')
        if path.is_dir(): continue
        # The pinned MySQL ZIP contains a 604,549,120-byte PDB. Validate its
        # complete hash within the already checked 2-GiB archive budget too.
        fixed(path, 2 * 1024**3); key = path.relative_to(directory).as_posix()
        if key != 'recastpress-tool.json': installed[key] = digest(path)
    equal(installed, expected, 'tool_installed_full_tree')
    signature = {'verified': False, 'scope': 'fixed SHA-256 archive pin only; no MariaDB signature claim'}
    if backend == 'mysql':
        fixed(GPG, 64 * 1024 * 1024)
        key = fixed(ROOT / '.reference/mysql-release-key.asc', 65536)
        sig = fixed(ROOT / '.reference/mysql-8.4.13-winx64.zip.asc', 65536)
        equal(digest(key), 'a4bcd9f16a53cc763f87b9955dbcdced33c7aa90296b157eb6ceef0f156f4327', 'pinned_mysql_key_bytes')
        equal(digest(sig), '81501212b6c52222bcf86e1af1b5c73f9c157f1191405c03936bc9e89f5495ee', 'pinned_mysql_signature_bytes')
        home = owned(harness.work / 'gpg'); home.mkdir()
        def gpg_path(path):
            value = path.as_posix(); return '/' + value[0].lower() + value[2:] if len(value) > 2 and value[1] == ':' else value
        command = [str(GPG), '--no-options', '--homedir', gpg_path(home), '--batch', '--no-tty', '--no-autostart', '--no-auto-key-retrieve']
        result = harness.checked('mysql_release_key', [*command, '--with-colons', '--show-keys', gpg_path(key)], timeout=30, limit=65536)
        fingerprints = [line.split(':')[9] for line in result.stdout.decode('ascii').splitlines() if line.startswith('fpr:')]
        require(fingerprints and fingerprints[0] == MYSQL_SIGNER, 'mysql_release_key_fingerprint')
        # A public verification keyring needs no secret-key agent. Dearmor into
        # this fresh run instead of import, which tries to contact gpg-agent.
        keyring = home / 'release-key.gpg'
        harness.checked('mysql_public_keyring', [*command, '--dearmor', '--output', gpg_path(keyring), gpg_path(key)], timeout=30, limit=65536)
        result = harness.checked('mysql_archive_signature', [*command, '--no-default-keyring', '--keyring', gpg_path(keyring),
            '--trust-model', 'always', '--status-fd=1', '--verify', gpg_path(sig), gpg_path(archive)], timeout=60, limit=65536)
        valid = [line.split()[2] for line in result.stdout.decode('ascii').splitlines() if line.startswith('[GNUPG:] VALIDSIG ')]
        require(MYSQL_SIGNER in valid, 'mysql_archive_signature_fingerprint')
        signature = {'verified': True, 'signer_fingerprint': MYSQL_SIGNER, 'key_sha256': digest(key),
            'signature_sha256': digest(sig), 'scope': 'offline pinned key and archive detached signature'}
    ledger = json.dumps(expected, sort_keys=True, separators=(',', ':')).encode('ascii')
    return directory, {'version': pin['version'], 'archive_bytes': archive.stat().st_size, 'archive_sha256': pin['archive_sha256'],
        'file_count': len(expected), 'full_file_inventory_sha256': sha(ledger), 'manifest_sha256': digest(manifest_path),
        'server_sha256': expected['bin/' + pin['server']], 'client_sha256': expected['bin/' + pin['client']],
        'admin_sha256': expected['bin/' + pin['admin']], 'signature': signature}


def certificates(harness):
    fixed(OPENSSL, 64 * 1024 * 1024)
    directory = owned(harness.work / 'tls'); directory.mkdir()
    ca_config = write_private(directory / 'ca.cnf', b'''[req]
prompt=no
distinguished_name=dn
x509_extensions=ca_ext
[dn]
CN=RecastPress owned DB test CA
[ca_ext]
basicConstraints=critical,CA:TRUE
keyUsage=critical,keyCertSign,cRLSign
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid:always,issuer
''')
    leaf_config = write_private(directory / 'leaf.cnf', b'''[req]
prompt=no
distinguished_name=dn
req_extensions=request_ext
[dn]
CN=127.0.0.1
[request_ext]
subjectAltName=IP:127.0.0.1
[server_ext]
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature,keyEncipherment
extendedKeyUsage=serverAuth
subjectAltName=IP:127.0.0.1
subjectKeyIdentifier=hash
authorityKeyIdentifier=keyid:always,issuer
''')
    ca, ca_key, cert, key, csr = [directory / name for name in ['ca.pem', 'ca-key.pem', 'server.pem', 'server-key.pem', 'server.csr']]
    commands = [
        ['req', '-new', '-x509', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-days', '1', '-set_serial', '1',
            '-config', str(ca_config), '-keyout', str(ca_key), '-out', str(ca)],
        ['req', '-new', '-newkey', 'rsa:2048', '-nodes', '-sha256', '-config', str(leaf_config), '-keyout', str(key), '-out', str(csr)],
        ['x509', '-req', '-in', str(csr), '-CA', str(ca), '-CAkey', str(ca_key), '-set_serial', '2', '-days', '1', '-sha256',
            '-extfile', str(leaf_config), '-extensions', 'server_ext', '-out', str(cert)],
        ['verify', '-x509_strict', '-purpose', 'sslserver', '-verify_ip', '127.0.0.1', '-CAfile', str(ca), str(cert)],
    ]
    for index, arguments in enumerate(commands):
        harness.checked('tls_fixture_' + str(index), [str(OPENSSL), *arguments], timeout=20, limit=65536,
            extra_env={'MSYS_NO_PATHCONV': '1'})
    for path in [ca, ca_key, cert, key, csr]: fixed(path, 65536)
    require(ca.stat().st_size <= 16 * 1024, 'wire_ca_quota')
    return ca, cert, key


def client_config(harness, name, user, password, port, backend, tls, ca):
    require(password is None or re.fullmatch('[0-9a-f]{48}', password), 'fixed_credential_shape')
    text = '[client]\nhost=127.0.0.1\nprotocol=tcp\nport=' + str(port) + '\nuser=' + user + '\ndefault-character-set=utf8mb4\n'
    if password is not None: text += 'password=' + password + '\n'
    if backend == 'mysql':
        text += 'ssl-mode=' + ('VERIFY_IDENTITY' if tls == 'required' else 'DISABLED') + '\n'
        if tls == 'disabled': text += 'get-server-public-key\n'
    else: text += ('ssl\nssl-verify-server-cert\n' if tls == 'required' else 'skip-ssl\n')
    if tls == 'required': text += 'ssl-ca=' + ca.as_posix() + '\n'
    return write_private(harness.work / name, text.encode('ascii'))


def client_command(tool, backend, config, *, admin=False):
    pin = PINS[backend]
    result = [str(tool / 'bin' / pin['admin' if admin else 'client']), '--defaults-file=' + str(config)]
    if backend == 'mysql': result.append('--no-login-paths')
    if not admin: result += ['--batch', '--skip-column-names', '--raw', '--connect-timeout=3']
    return result


def identity_ready(harness, command, port, version, backend, *, wait=False):
    deadline = time.monotonic() + (30 if wait else 5)
    while True:
        require(harness.server is not None and harness.server.poll() is None, 'owned_db_exited_before_identity')
        result = harness.run('db_identity', [*command, '--execute=SELECT VERSION(), @@datadir, @@port'], timeout=5, limit=65536)
        if result.returncode == 0:
            fields = result.stdout.decode('utf-8', errors='strict').strip().split('\t')
            require(len(fields) == 3 and (fields[0] == version if backend == 'mysql' else fields[0].startswith(version + '-MariaDB')),
                'owned_db_version')
            equal(Path(fields[1]).resolve(), (harness.work / 'data').resolve(), 'owned_db_datadir')
            equal(fields[2], str(port), 'owned_db_port')
            equal(int(fixed(harness.work / 'server.pid', 64).read_text(encoding='ascii').strip()), harness.server.pid, 'owned_db_pid')
            require(harness.server.poll() is None, 'owned_db_exited_after_identity')
            return fields[0]
        require(wait and time.monotonic() < deadline, 'owned_db_identity_deadline')
        time.sleep(0.1)


def bytes_hex(value, maximum=4096):
    require(type(value) is str and len(value) <= maximum * 2 and re.fullmatch('(?:[0-9a-f]{2})*', value) is not None, 'raw_hex_shape')
    return bytes.fromhex(value)


def validate_wire(value, backend, tls):
    """Independent raw-result checks, without trusting only the probe verdict."""
    for key, expected in [('schema', 'recastpress.wpdb-wire-observation.v0.5'), ('outcome', 'fixed_wire_cases_passed'),
            ('passed', True), ('tls_profile', tls), ('whole_probe_deadline_seconds', 60),
            ('initial_disconnect_completed', True), ('fresh_disconnect_completed', True), ('physical_session_only', True)]:
        equal(value[key], expected, 'wire_' + key)
    for key in ['raw_non_utf8_err_verified', 'native_wpdb_comparison_passed', 'standard_wpdb_compatibility_verified',
            'product_driver_selected', 'database_started_by_probe', 'ddl_executed_by_probe']:
        equal(value[key], False, 'wire_unclaimed_' + key)
    for key in ['initial_disconnect_error', 'fresh_disconnect_error']:
        equal(value[key], None, 'wire_disconnect_error_absent')
    handshakes = value['handshakes']; require(type(handshakes) is list and len(handshakes) == 2, 'two_physical_handshakes')
    versions = []
    for index, handshake in enumerate(handshakes):
        equal(handshake['connection'], ['initial', 'fresh'][index], 'handshake_order')
        for key in ['found_rows', 'multi_statements', 'local_files']: equal(handshake[key], False, 'forbidden_capability_' + key)
        equal(handshake['three_forbidden_capabilities_off'], True, 'forbidden_capabilities_off')
        equal(handshake['ssl_capability'], tls == 'required', 'serialized_ssl_capability')
        mask = handshake['serialized_capabilities']; require(type(mask) is int and 0 <= mask <= 2**32 - 1, 'serialized_mask_type')
        require(mask & (2 | 128 | 65536) == 0 and bool(mask & 2048) == (tls == 'required'), 'actual_serialized_capability_bits')
        require(type(handshake['connection_id']) is int and handshake['connection_id'] > 0, 'physical_connection_id')
        version = bytes_hex(handshake['raw_server_version_hex'], 256)
        require(version and PINS[backend]['version'].encode('ascii') in version, 'raw_server_version_independent')
        versions.append(version)
    equal(versions[0], versions[1], 'same_raw_server_version')
    require(handshakes[0]['connection_id'] != handshakes[1]['connection_id'], 'fresh_connection_id')
    queries = [
        ('initial_tls', b"SHOW SESSION STATUS LIKE 'Ssl_cipher'"), ('initial_connection_id', b'SELECT CONNECTION_ID()'),
        ('fixture_empty', b'SELECT COUNT(*) FROM rp_wire_fixture'), ('set_latin1', b'SET NAMES latin1'),
        ('non_utf8_sql_bytes', b"SELECT _binary'\xff' AS raw_query_byte"), ('non_utf8_column_name', b'SELECT 7 AS `\xff`'),
        ('insert', b"INSERT INTO rp_wire_fixture(marker,payload,dec_value,big_value) VALUES('rp-wire',X'00ff41',12345678901234567890.1234567890,18446744073709551615)"),
        ('raw_ordered_rows', b"SELECT payload AS duplicate,NULL AS duplicate,X'' AS empty_value,dec_value,big_value FROM rp_wire_fixture WHERE marker='rp-wire'"),
        ('zero_rows_with_columns', b"SELECT payload AS duplicate,NULL AS duplicate,X'' AS empty_value,dec_value,big_value FROM rp_wire_fixture WHERE 1=0"),
        ('same_value_update', b"UPDATE rp_wire_fixture SET payload=X'00ff41' WHERE marker='rp-wire'"),
        ('changed_update', b"UPDATE rp_wire_fixture SET payload=X'00ff42' WHERE marker='rp-wire'"),
        ('ascii_syntax_error', b'SELEC rp_fixed_ascii_error'), ('syntax_error_recovery', b'SELECT 17'),
        ('multi_statement_rejection', b'SELECT 1; SELECT 2'), ('multi_rejection_recovery', b'SELECT 23'),
        ('begin_rollback', b'BEGIN'), ('transaction_update', b"UPDATE rp_wire_fixture SET payload=X'00ff43' WHERE marker='rp-wire'"),
        ('rollback', b'ROLLBACK'), ('rollback_read', b"SELECT payload FROM rp_wire_fixture WHERE marker='rp-wire'"),
        ('session_marker', b'SET @rp_wire_marker=91'), ('begin_before_disconnect', b'BEGIN'),
        ('uncommitted_before_disconnect', b"UPDATE rp_wire_fixture SET payload=X'00ff44' WHERE marker='rp-wire'"),
        ('fresh_tls', b"SHOW SESSION STATUS LIKE 'Ssl_cipher'"), ('fresh_connection_id', b'SELECT CONNECTION_ID()'),
        ('fresh_session_variable', b'SELECT @rp_wire_marker'), ('bounded_lock_wait', b'SET SESSION innodb_lock_wait_timeout=2'),
        ('fresh_acquires_old_row_lock', b"UPDATE rp_wire_fixture SET payload=X'00ff42' WHERE marker='rp-wire'"),
        ('disconnect_rollback_read', b"SELECT payload FROM rp_wire_fixture WHERE marker='rp-wire'"),
    ]
    observations = value['observations']; require(type(observations) is list and len(observations) == len(queries), 'all_fixed_wire_cases')
    by_name = {}
    for row, (name, sql) in zip(observations, queries):
        equal(row['case'], name, 'fixed_case_order'); equal(bytes_hex(row['sql_hex']), sql, 'actual_sql_bytes')
        by_name[name] = row
        if name in ['ascii_syntax_error', 'multi_statement_rejection']:
            for key, expected in [('server_rejected', True), ('code', 1064), ('state', '42000'),
                    ('message_is_ascii', True), ('raw_err_packet_preserved', False)]: equal(row[key], expected, 'fixed_ascii_server_error')
            require(bytes_hex(row['ascii_message_hex']).isascii(), 'ascii_error_message_only'); continue
        equal(row['protocol'], 'COM_QUERY/TextProtocol', 'text_protocol'); equal(row['complete'], True, 'complete_result')
        columns, rows = row['columns'], row['rows']
        require(type(columns) is list and len(columns) <= 16 and type(rows) is list and len(rows) <= 16, 'wire_result_quota')
        for column in columns:
            equal(set(column), {'schema_hex', 'table_hex', 'original_table_hex', 'name_hex', 'original_name_hex',
                'type', 'flags', 'character_set', 'declared_length', 'decimals'}, 'raw_column_fields')
            for key in ['schema_hex', 'table_hex', 'original_table_hex', 'name_hex', 'original_name_hex']: bytes_hex(column[key])
            for key in ['type', 'flags', 'character_set', 'declared_length', 'decimals']:
                require(type(column[key]) is int and 0 <= column[key] <= (2**32 - 1 if key == 'declared_length' else 65535), 'raw_column_integer')
        for record in rows:
            require(type(record) is list and len(record) == len(columns), 'positional_row_width')
            for cell in record:
                if cell is not None: bytes_hex(cell)
        require(type(row['affected_rows']) is str and row['affected_rows'].isdigit(), 'affected_rows_decimal_string')
        require(row['last_insert_id'] is None or type(row['last_insert_id']) is str and row['last_insert_id'].isdigit(), 'insert_id_decimal_string_or_null')
        require(type(row['warnings']) is int and 0 <= row['warnings'] <= 65535, 'warning_count')
    for label, index in [('initial', 0), ('fresh', 1)]:
        equal(by_name[label + '_connection_id']['rows'], [[str(handshakes[index]['connection_id']).encode().hex()]], 'query_vs_handshake_connection_id')
        rows = by_name[label + '_tls']['rows']; require(len(rows) == 1 and len(rows[0]) == 2, 'actual_tls_status_shape')
        equal(rows[0][0], b'Ssl_cipher'.hex(), 'actual_tls_status_name'); cipher = bytes_hex(rows[0][1])
        require(bool(cipher) == (tls == 'required'), 'actual_tls_cipher')
    expected_rows = {'fixture_empty': [['30']], 'non_utf8_sql_bytes': [['ff']], 'non_utf8_column_name': [['37']],
        'raw_ordered_rows': [['00ff41', None, '', b'12345678901234567890.1234567890'.hex(), b'18446744073709551615'.hex()]],
        'zero_rows_with_columns': [], 'syntax_error_recovery': [['3137']], 'multi_rejection_recovery': [['3233']],
        'rollback_read': [['00ff42']], 'fresh_session_variable': [[None]], 'disconnect_rollback_read': [['00ff42']]}
    for name, rows in expected_rows.items(): equal(by_name[name]['rows'], rows, 'exact_positional_bytes_and_null')
    equal(by_name['non_utf8_column_name']['columns'][0]['name_hex'], 'ff', 'non_utf8_column_name_preserved')
    equal([c['name_hex'] for c in by_name['raw_ordered_rows']['columns'][:2]], [b'duplicate'.hex()] * 2, 'duplicate_column_names_preserved')
    equal(by_name['zero_rows_with_columns']['columns'], by_name['raw_ordered_rows']['columns'], 'zero_rows_keep_full_metadata')
    for name, count in [('insert', '1'), ('same_value_update', '0'), ('changed_update', '1'),
            ('transaction_update', '1'), ('fresh_acquires_old_row_lock', '0')]: equal(by_name[name]['affected_rows'], count, 'changed_row_semantics')
    require(type(by_name['insert']['last_insert_id']) is str and int(by_name['insert']['last_insert_id']) > 0, 'actual_insert_id')
    return {'fixed_cases': len(queries), 'physical_connections': 2, 'three_forbidden_capabilities_off': True,
        'actual_tls_profile_verified': True, 'raw_sql_and_result_bytes_verified': True,
        'raw_column_metadata_and_order_verified': True, 'ascii_err_fixture_verified': True,
        'fresh_session_variable_and_disconnect_rollback_verified': True,
        'raw_non_utf8_err_verified': False, 'native_wpdb_comparison_passed': False}


def source_hashes(mode='wire'):
    paths = [Path(__file__).resolve(), HERE / 'wire_check.py', ROOT / 'scripts/wordpress_reference.py',
        ROOT / 'scripts/setup_mysql.py', ROOT / 'scripts/setup_mariadb.py', ROOT / 'scripts/pinned_zip_tool.py']
    if mode == 'native': paths += [HERE / 'native_check.py', HERE / 'fixtures/native_wpdb.php', ROOT / 'scripts/owned_probe.py']
    return {path.relative_to(ROOT).as_posix(): digest(fixed(path, 4 * 1024 * 1024)) for path in paths}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--backend', choices=['mysql', 'mariadb'], required=True)
    parser.add_argument('--tls', choices=['disabled', 'required'], required=True)
    parser.add_argument('--mode', choices=['wire', 'native'], default='wire')
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    report = {'schema': 'recastpress.wpdb-owned-database-' + args.mode + '.v0.5', 'format_version': 1,
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'platform': platform.system(),
        'backend': args.backend, 'tls_profile': args.tls, 'mode': args.mode, 'outcome': 'not_run', 'passed': False,
        'commands': [], 'database_started': False, 'database_identity_verified_before_fixture_ddl': False,
        'controller_account_bootstrapped_by_owned_server': False, 'admin_transport': 'verified_tls',
        'database_closed_and_reaped': False, 'all_command_children_reaped': False, 'run_retained': False,
        'separate_admin_and_limited_credentials': True, 'mysql_environment_forwarded': False,
        'credentials_in_command_line': False, 'credentials_in_public_record': False,
        'native_wpdb_comparison_passed': False, 'standard_wpdb_compatibility_verified': False,
        'product_driver_selected': False, 'product_abi_selected': False, 'existing_editing_sqlx_changed': False,
        'production_database_used': False, 'binary_redistributed': False, 'production_tls_policy_selected': False}
    report['cleanup_scope'] = 'captured child handles only; Windows tree cleanup is requested while leader is live, without independent descendant enumeration'
    report['process_tree_inventory_verified'] = False
    harness = None; work = None; stage = 'preflight'; passed = False; admin_password = secrets.token_hex(24); limited_password = secrets.token_hex(24)
    try:
        require(os.name == 'nt', 'windows_pinned_fixture_only')
        before = source_hashes(args.mode); report['input_files'] = before
        if args.mode == 'wire':
            require(args.binary is not None, 'wire_binary_required')
            binary = args.binary.absolute()
            require(binary in [TARGET / profile / 'recastpress-wpdb-wire-probe.exe' for profile in ['debug', 'release']], 'closed_wire_binary')
            fixed(binary, 256 * 1024 * 1024)
            build = wire_check.verify_receipt(binary); report['wire_build_receipt_sha256'] = digest(HERE / 'evidence/windows-wire-build.json')
            report['wire_binary'] = {'path': binary.relative_to(ROOT).as_posix(), **build['executable_identity']}
        else:
            require(args.binary is None and args.tls == 'disabled', 'native_closed_non_tls_profile')
            native = load('rp_owned_native_wpdb_reference', HERE / 'native_check.py')
            report['native_reference_before'] = native.references()
        RUNS.mkdir(parents=True, exist_ok=True); owned(RUNS)
        work = owned(RUNS / (args.backend + '-' + args.tls + '-' + uuid.uuid4().hex)); work.mkdir()
        report['run'] = work.relative_to(ROOT).as_posix(); report['run_retained'] = True
        harness = Harness(work, report)
        stage = 'tool_verification'; tool, report['database_tool'] = verify_tool(args.backend, harness)
        stage = 'tls_fixture'; ca, cert, key = certificates(harness)
        report['tls_fixture'] = {'san_ip': '127.0.0.1', 'fresh_ca_and_leaf': True, 'chain_and_ip_verified': True,
            'ca_sha256': digest(ca), 'server_cert_sha256': digest(cert), 'openssl_sha256': digest(OPENSSL)}
        with socket.socket() as reservation:
            reservation.bind(('127.0.0.1', 0)); port = reservation.getsockname()[1]
        pin = PINS[args.backend]; server_exe = tool / 'bin' / pin['server']
        stage = 'database_initialization'
        initialize = ([str(server_exe), '--no-defaults', '--initialize-insecure', '--basedir=' + str(tool), '--datadir=' + str(work / 'data'), '--console']
            if args.backend == 'mysql' else [str(tool / 'bin/mariadb-install-db.exe'), '--datadir=' + str(work / 'data'), '--silent'])
        harness.checked('initialize_fresh_database', initialize, timeout=90, cwd=tool, limit=4 * 1024 * 1024)
        server_text = ('[mysqld]\nbasedir=' + tool.as_posix() + '\ndatadir=' + (work / 'data').as_posix()
            + '\npid-file=' + (work / 'server.pid').as_posix() + '\nbind-address=127.0.0.1\nport=' + str(port)
            + '\nskip-name-resolve\nmax-connections=8\ninnodb-buffer-pool-size=64M\n'
            + 'ssl-ca=' + ca.as_posix() + '\nssl-cert=' + cert.as_posix() + '\nssl-key=' + key.as_posix()
            + '\nrequire-secure-transport=' + ('ON' if args.tls == 'required' else 'OFF') + '\n')
        # Initial root is localhost-only on MySQL with skip-name-resolve.
        # The captured server itself loads this private auth-only bootstrap;
        # no client sends account/schema DDL to an unverified listener.
        controller = write_private(work / 'controller-private.sql', (
            "CREATE USER 'rp_probe_admin'@'127.0.0.1' IDENTIFIED BY '" + admin_password + "' REQUIRE SSL;\n"
            "GRANT ALL PRIVILEGES ON rp_wpdb_probe.* TO 'rp_probe_admin'@'127.0.0.1' WITH GRANT OPTION;\n"
            "GRANT CREATE USER,SHUTDOWN ON *.* TO 'rp_probe_admin'@'127.0.0.1';\n").encode('ascii'))
        server_text += 'init-file=' + controller.as_posix() + '\n'
        server_text += ('mysqlx=0\nperformance-schema=OFF\ninnodb-redo-log-capacity=32M\nskip-log-bin\n' if args.backend == 'mysql' else 'innodb-log-file-size=32M\n')
        server_config = write_private(work / 'server.ini', server_text.encode('utf-8'))
        admin_config = client_config(harness, 'admin-private.ini', 'rp_probe_admin', admin_password, port, args.backend, 'required', ca)
        bootstrap_config = admin_config
        limited_config = client_config(harness, 'limited-private.ini', 'rp_wpdb_probe', limited_password, port, args.backend, args.tls, ca)
        config_hash = digest(server_config)
        stage = 'database_start'
        command = [str(server_exe), '--defaults-file=' + str(server_config)]
        if args.backend == 'mysql': command.append('--no-monitor')
        command.append('--console')
        harness.log = owned(work / 'server-private.log').open('xb')
        harness.server = subprocess.Popen(command, cwd=work, env=environment(), stdin=subprocess.DEVNULL,
            stdout=harness.log, stderr=subprocess.STDOUT, creationflags=subprocess.CREATE_NO_WINDOW | subprocess.CREATE_NEW_PROCESS_GROUP)
        harness.remember(harness.server)
        report.update(database_started=True, database_pid=harness.server.pid)
        stage = 'database_identity'
        version = identity_ready(harness, client_command(tool, args.backend, bootstrap_config), port, pin['version'], args.backend, wait=True)
        report.update(database_identity_verified_before_fixture_ddl=True, sql_server_version=version,
            controller_account_bootstrapped_by_owned_server=True)
        stage = 'database_fixture'
        auth = ("CREATE USER 'rp_wpdb_probe'@'127.0.0.1' IDENTIFIED BY '" + limited_password + "' REQUIRE "
            + ('SSL' if args.tls == 'required' else 'NONE') + ';\n').encode('ascii')
        auth += DDL + b"GRANT SELECT,INSERT,UPDATE,DELETE ON rp_wpdb_probe.* TO 'rp_wpdb_probe'@'127.0.0.1';\n"
        harness.checked('owned_schema_and_limited_account', client_command(tool, args.backend, bootstrap_config), timeout=30, input_bytes=auth)
        admin = client_command(tool, args.backend, admin_config)
        identity_ready(harness, admin, port, pin['version'], args.backend)
        limited = client_command(tool, args.backend, limited_config)
        initial = harness.checked('limited_fixture_empty', [*limited, '--execute=SELECT COUNT(*) FROM rp_wpdb_probe.rp_wire_fixture'], timeout=5, limit=65536)
        equal(initial.stdout.strip(), b'0', 'fresh_empty_fixture')
        report['fixture_empty_before_probe'] = True
        grants = harness.checked('limited_user_grants', [*limited, '--execute=SHOW GRANTS FOR CURRENT_USER()'], timeout=5, limit=65536)
        grant_text = grants.stdout.decode('ascii', errors='strict').upper()
        require(all(token in grant_text for token in ['SELECT', 'INSERT', 'UPDATE', 'DELETE'])
            and not any(token in grant_text for token in ['ALL PRIVILEGES', 'SHUTDOWN', 'GRANT OPTION', 'FILE,', 'SUPER']), 'limited_grants_scope')
        report['limited_dml_grants_verified'] = True
        request = {'schema': 'recastpress.wpdb-wire-input.v0.5', 'host': '127.0.0.1', 'port': port,
            'database': 'rp_wpdb_probe', 'user': 'rp_wpdb_probe', 'password': limited_password,
            'tls': args.tls, 'tls_ca_pem': ca.read_text(encoding='ascii') if args.tls == 'required' else None}
        if args.mode == 'wire':
            stage = 'wire_execution'
            result = harness.run('owned_wire_probe', [str(binary)], timeout=75,
                input_bytes=(json.dumps(request, separators=(',', ':')) + '\n').encode('ascii'), limit=512 * 1024)
            require(result.stdout.endswith(b'\n') and result.stdout.count(b'\n') == 1, 'single_wire_result')
            value = parse_json(result.stdout); report['wire_result'] = value
            report['wire_stdout_sha256'] = sha(result.stdout); report['wire_stderr_sha256'] = sha(result.stderr)
            require(result.returncode == 0 and result.stderr == b'', 'wire_probe_failed')
            report['wire_observations'] = validate_wire(value, args.backend, args.tls)
        else:
            stage = 'native_execution'
            request['schema'] = 'recastpress.wpdb-native-input.v0.5'
            result = harness.run('owned_native_wpdb', native.command(), timeout=30,
                input_bytes=json.dumps(request).encode('ascii'), extra_env=native.environment(), limit=512 * 1024)
            require(result.stdout.endswith(b'\n') and result.stdout.count(b'\n') == 1, 'single_native_result')
            report['native_result'] = parse_json(result.stdout)
            report['native_stdout_sha256'] = sha(result.stdout); report['native_stderr_sha256'] = sha(result.stderr)
            require(result.returncode == 0 and result.stderr == b'', 'native_wpdb_probe_failed')
            report['native_observations'] = native.validate_observation(report['native_result'], report['native_reference_before'])
            equal(native.references(), report['native_reference_before'], 'native_reference_changed')
        identity_ready(harness, admin, port, pin['version'], args.backend)
        final = harness.checked('admin_final_rows', [*admin, '--execute=SELECT HEX(payload),COUNT(*) FROM rp_wpdb_probe.rp_wire_fixture GROUP BY payload'], timeout=5, limit=65536)
        equal(final.stdout.strip(), b'00FF42\t1', 'actual_final_database_rows')
        equal(digest(server_config), config_hash, 'owned_server_config_changed')
        if args.mode == 'wire': equal(wire_check.verify_receipt(binary), build, 'wire_build_chain_changed_after_db')
        equal(source_hashes(args.mode), before, 'harness_sources_changed')
        stage = 'database_shutdown'
        harness.checked('admin_shutdown', [*client_command(tool, args.backend, admin_config, admin=True), 'shutdown'], timeout=10, limit=65536)
        report['database_cleanup'] = harness.cleanup(harness.server, normal_wait=10)
        require(not report['database_cleanup']['forced_tree_cleanup'], 'normal_admin_shutdown_not_observed')
        report['database_closed_and_reaped'] = True
        report['all_command_children_reaped'] = all(row['owned_reaped'] is True for row in report['commands'])
        require(report['all_command_children_reaped'], 'owned_command_not_reaped')
        passed = True; report['outcome'] = 'fixed_owned_database_' + args.mode + '_profile_passed'
    except (Exception, KeyboardInterrupt) as error:
        report.update(outcome='owned_database_' + args.mode + '_profile_failed', failure_stage=stage, failure_class=type(error).__name__)
        if isinstance(error, ValueError) and re.fullmatch('[a-z][a-z0-9_]{0,95}', str(error)):
            report['failure_reason'] = str(error)
    finally:
        if harness is not None:
            try:
                if harness.server is not None:
                    cleanup = harness.cleanup(harness.server, normal_wait=0)
                    if 'database_cleanup' not in report: report['database_cleanup'] = cleanup
                    report['database_closed_and_reaped'] = True
            except (Exception, KeyboardInterrupt):
                passed = False; report.update(outcome='owned_database_cleanup_failed', cleanup_failed=True)
            finally:
                try: harness.reap_remaining()
                except (Exception, KeyboardInterrupt):
                    passed = False; report.update(outcome='owned_database_cleanup_failed', cleanup_failed=True)
                report['all_command_children_reaped'] = all(row['owned_reaped'] is True for row in report['commands'])
                if harness.log is not None: harness.log.close()
        if report['database_started'] and not report['database_closed_and_reaped']: passed = False
        if not report['all_command_children_reaped']: passed = False
    report['passed'] = passed
    wire = (json.dumps(report, indent=2, ensure_ascii=True, allow_nan=False) + '\n').encode('ascii')
    require(admin_password.encode() not in wire and limited_password.encode() not in wire, 'private_secret_in_public_record')
    require(len(wire) <= 1024 * 1024, 'public_database_evidence_quota')
    if args.record:
        directory = HERE / 'evidence'; directory.mkdir(exist_ok=True)
        path = directory / ('windows-' + args.backend + '-' + args.tls + '-' + args.mode + '-database.json')
        require(path.resolve() == path and not path.is_symlink(), 'public_database_record_linked')
        path.write_bytes(wire)
    print('WPDB owned database: ' + report['outcome'] + ' (' + stage + ')')
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
