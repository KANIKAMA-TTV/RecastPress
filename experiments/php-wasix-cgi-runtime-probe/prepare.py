"""Verify a preseeded fixed WordPress archive and place three pure-hook sources.

No network, native PHP, WordPress bootstrap or Wasm execution is performed.
Only three already pinned regular files are copied to a fresh target directory.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tarfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
WORK = ROOT / 'target/php-wasix-cgi-runtime-probe'
DESTINATION = WORK / 'wp-core'
MANIFEST = HERE / 'manifest.json'
RECEIPT = HERE / 'evidence/wordpress-source.json'
SOURCE_MANIFEST = ROOT / 'experiments/php-wasi-host-probe/wordpress-fixture.json'
SOURCE_SHA = 'd2a09acb6a15e3b9c471d72557753c266d6f41a79ed19bfe04cbfe49e283b2a5'
SOURCE_BYTES = 35368461
NAMES = ['plugin.php', 'class-wp-hook.php', 'class-wp-filter-sentinel.php']

sys.path.insert(0, str(ROOT / 'experiments/php-wasix-host-probe'))
from language_check import equal, parse_json


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(wire):
    return hashlib.sha256(wire).hexdigest()


def digest(path):
    value = hashlib.sha256()
    with path.open('rb') as source:
        for block in iter(lambda: source.read(1024 * 1024), b''):
            value.update(block)
    return value.hexdigest()


def fixed(path, limit):
    require(path.resolve(strict=True) == path and path.is_file(), 'linked or missing fixed source input')
    require(0 < path.stat().st_size <= limit, 'fixed source input byte budget')
    return path


def load_manifest():
    m = parse_json(fixed(MANIFEST, 65536).read_bytes())
    equal(m['schema'], 'recastpress.php-wasix-cgi-runtime-plan.v0.5', 'runtime plan schema')
    equal(m['format_version'], 1, 'runtime plan format')
    wp = m['wordpress']
    expected = parse_json(fixed(SOURCE_MANIFEST, 65536).read_bytes())
    equal(wp, {'version': '7.1.3', 'archive_url': 'https://wordpress.org/wordpress-7.1.3.tar.gz',
        'archive_path': '.reference/wordpress-7.1.3.tar.gz', 'archive_bytes': SOURCE_BYTES,
        'archive_sha256': SOURCE_SHA, 'source_manifest': SOURCE_MANIFEST.relative_to(ROOT).as_posix(),
        'source_manifest_sha256': digest(SOURCE_MANIFEST), 'files': expected['files']}, 'fixed WP inputs')
    equal(expected['wordpress_version'], '7.1.3', 'source manifest version')
    equal(expected['archive_sha256'], SOURCE_SHA, 'source manifest archive')
    equal(set(wp['files']), set(NAMES), 'exact pure hook source set')
    require(all(value is False for value in m['claims'].values()), 'plan must not claim execution')
    require(m['fixture']['path'] == 'experiments/php-wasix-cgi-runtime-probe/fixtures/runtime_cgi_contract.php',
        'fixed runtime fixture path')
    fixture = fixed(ROOT / m['fixture']['path'], 65536)
    equal({'path': fixture.relative_to(ROOT).as_posix(), 'bytes': fixture.stat().st_size,
        'sha256': digest(fixture)}, m['fixture'], 'runtime fixture identity')
    equal(m['php_build_profile'], 'experiments/php-wasix-fiber-cgi-probe/manifest.json', 'combined profile')
    equal(digest(fixed(ROOT / m['php_build_profile'], 65536)), m['php_build_manifest_sha256'], 'combined profile digest')
    return m


def input_hashes():
    return {p.relative_to(ROOT).as_posix(): digest(p) for p in [Path(__file__).resolve(), MANIFEST,
        SOURCE_MANIFEST, ROOT / 'scripts/wordpress_reference.py',
        ROOT / 'experiments/php-wasix-host-probe/language_check.py']}


def verify_sources():
    m = load_manifest()
    require(DESTINATION.resolve(strict=True) == DESTINATION and DESTINATION.is_dir(), 'fixed WP source root linked/missing')
    equal({p.name for p in DESTINATION.iterdir()}, set(NAMES), 'exact prepared source set')
    result = {}
    for name in NAMES:
        p = fixed(DESTINATION / name, 65536)
        identity = {'bytes': p.stat().st_size, 'sha256': digest(p)}
        equal(identity, m['wordpress']['files'][name], 'prepared WP source identity')
        result[name] = {'path': p.relative_to(ROOT).as_posix(), **identity}
    return result


def verify_prepared():
    m = load_manifest()
    wire = fixed(RECEIPT, 65536).read_bytes()
    receipt = parse_json(wire)
    for key, expected in [('schema', 'recastpress.php-wasix-cgi-runtime-source.v0.5'),
            ('outcome', 'fixed_three_wordpress_sources_prepared'), ('wordpress_executed', False),
            ('native_php_executed', False), ('network_used_by_preparer', False), ('inputs_unchanged', True),
            ('input_files', input_hashes()), ('files', verify_sources())]:
        equal(receipt[key], expected, 'WP prepare receipt ' + key)
    archive = fixed(ROOT / m['wordpress']['archive_path'], SOURCE_BYTES)
    equal(receipt['archive'], {'path': m['wordpress']['archive_path'], 'bytes': SOURCE_BYTES,
        'sha256': SOURCE_SHA}, 'WP archive receipt')
    equal(archive.stat().st_size, SOURCE_BYTES, 'WP archive bytes')
    equal(digest(archive), SOURCE_SHA, 'WP archive digest')
    return {'receipt_sha256': sha(wire), 'input_files': receipt['input_files'], 'files': receipt['files'],
        'archive': receipt['archive']}


def prepare(report):
    m = load_manifest()
    before = input_hashes()
    archive = fixed(ROOT / m['wordpress']['archive_path'], SOURCE_BYTES)
    equal(archive.stat().st_size, SOURCE_BYTES, 'WP archive bytes')
    equal(digest(archive), SOURCE_SHA, 'WP archive digest')
    require(WORK.resolve() == WORK and WORK.is_relative_to(ROOT / 'target'), 'fixed source target escaped')
    require(not DESTINATION.exists() and not DESTINATION.is_symlink(), 'fresh WP source target required')
    # Reuse the existing pure archive-path validator, without calling fetch/extract.
    spec = importlib.util.spec_from_file_location('rp_cgi_runtime_archive_paths', ROOT / 'scripts/wordpress_reference.py')
    reference = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(reference)
    selected, seen, count, expanded = {}, set(), 0, 0
    with tarfile.open(archive, 'r|gz') as source:
        for member in source:
            count += 1
            require(count <= 10000 and 0 <= member.size <= 32 * 1024 * 1024, 'WP member quota')
            expanded += member.size
            require(expanded <= 256 * 1024 * 1024, 'WP expanded quota')
            path = reference.safe_relative(member.name)
            require(member.isdir() or member.isfile(), 'WP links/special files refused')
            name = path.as_posix()
            require(name not in seen, 'duplicate WP archive member')
            seen.add(name)
            if name.startswith('wp-includes/') and name.removeprefix('wp-includes/') in NAMES:
                short = name.removeprefix('wp-includes/')
                expected = m['wordpress']['files'][short]
                require(member.isfile() and member.size == expected['bytes'], 'WP selected member type/bytes')
                with source.extractfile(member) as data:
                    wire = data.read(65537)
                equal({'bytes': len(wire), 'sha256': sha(wire)}, expected, 'WP selected archive bytes')
                selected[short] = wire
    equal(set(selected), set(NAMES), 'all three WP sources found')
    equal(digest(archive), SOURCE_SHA, 'WP archive unchanged')
    equal(input_hashes(), before, 'WP prepare inputs unchanged')
    DESTINATION.mkdir(parents=True, exist_ok=False)
    for name in NAMES:
        with (DESTINATION / name).open('xb') as destination:
            destination.write(selected[name])
    report.update(input_files=before, archive={'path': m['wordpress']['archive_path'], 'bytes': SOURCE_BYTES,
        'sha256': SOURCE_SHA}, files=verify_sources(), archive_members_checked=count,
        declared_expanded_bytes_checked=expanded, inputs_unchanged=True,
        outcome='fixed_three_wordpress_sources_prepared')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validate-only', action='store_true')
    parser.add_argument('--record', action='store_true')
    args = parser.parse_args()
    report = {'format_version': 1, 'schema': 'recastpress.php-wasix-cgi-runtime-source.v0.5',
        'observed_at_utc': datetime.now(timezone.utc).isoformat(), 'outcome': 'not_run',
        'wordpress_executed': False, 'native_php_executed': False, 'network_used_by_preparer': False}
    passed = False
    try:
        load_manifest()
        if args.validate_only:
            print('Fixed CGI runtime definitions validated; no archive or PHP executed.')
            return 0
        prepare(report)
        passed = True
    except (Exception, KeyboardInterrupt) as error:
        report['outcome'] = 'source_preparation_failed'
        report['diagnostics'] = str(error).replace(str(ROOT), '<workspace>').replace(ROOT.as_posix(), '<workspace>')[:4096]
    if args.record:
        RECEIPT.parent.mkdir(exist_ok=True)
        require(RECEIPT.resolve() == RECEIPT and not RECEIPT.is_symlink(), 'source receipt linked')
        RECEIPT.write_text(json.dumps(report, indent=2, ensure_ascii=True) + '\n', encoding='utf-8', newline='\n')
    print('CGI runtime WP source preparation: ' + report['outcome'])
    return 0 if passed else 1


if __name__ == '__main__':
    sys.exit(main())
