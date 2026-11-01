"""Apply only the fixed WASI managed-access patch with whole-file Git guards.

A caller supplies the finite/output-bounded Git runner. No PHP/compiler is run.
Prior changes may be empty or exactly the two canonical Fiber/CGI source patches;
no arbitrary tracked changes, offsets-only matches, index updates or reapply.
"""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
PHP = ROOT / 'target/php-wasix-managed-access-probe/source/php'
PLAN = HERE / 'patch-plan.json'
COMMIT = 'f4bfae79d5c19028349082f707930cf0951d23b9'
PATCH = 'patches/0001-wasi-managed-access.patch'
PATCH_BYTES = 9077
PATCH_SHA = '343bd0bcd1adb4abcf22653973a5f2d3a75360c1fe06c2748ed83b5646fe3be1'
HEADER = HERE / 'fixtures/recastpress_fs_access.h'
TARGETS = ['ext/standard/filestat.c', 'main/streams/plain_wrapper.c', 'main/recastpress_fs_access.h']
BEFORE = {'ext/standard/filestat.c': {'bytes': 29692,
                             'sha256': 'fac3258f4ed14964bc9d28c69c2de3a05f637b62ee938f670b7c362bd0f0070a',
                             'git_blob_sha1': 'd960d4e8535d91d5432dc54ff3bfed1a9b2d7db0'},
 'main/streams/plain_wrapper.c': {'bytes': 47201,
                                  'sha256': 'e612184e21e8b1cb311feaac3d54fb0902f514b749ca57096d94e0a9c9cd8edf',
                                  'git_blob_sha1': '74d8877a7f3819f5fb1fb3de49082e7ee94f7b4d'},
 'main/recastpress_fs_access.h': None}
AFTER = {'ext/standard/filestat.c': {'bytes': 30529,
                             'sha256': 'db2460056917b7c3689971d8da30fe08f1e32ea657147585bf81a35f672f069f',
                             'git_blob_sha1': 'ecd58dda113f209e1a5a49021d7d041549d00cf0'},
 'main/streams/plain_wrapper.c': {'bytes': 47407,
                                  'sha256': 'e202a4ef58c7cb776463003db13079850109b341f243dd55109d48bfdeab47ca',
                                  'git_blob_sha1': '4ca68635f209b5d2fe880a92d6b5f0d946e8d2ad'},
 'main/recastpress_fs_access.h': {'bytes': 7066,
                                  'sha256': '3973380aeaf7354496dd18bb2f82ffe3f6467ac7ebe9e2c82a0a634f9514268f',
                                  'git_blob_sha1': 'beedbef4a7fc6b6249bc894b8da55f933e8ef4a4'}}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fingerprint(wire):
    return {'bytes': len(wire), 'sha256': hashlib.sha256(wire).hexdigest(),
        'git_blob_sha1': hashlib.sha1(b'blob ' + str(len(wire)).encode('ascii') + b'\0' + wire).hexdigest()}


def fixed(path, limit=1024 * 1024):
    require(path.resolve(strict=True) == path and path.is_file(), 'linked/missing fixed managed-access input')
    size = path.stat().st_size
    require(0 < size <= limit, 'managed-access fixed file byte budget')
    wire = path.read_bytes()
    require(len(wire) == size, 'managed-access input changed while reading')
    return wire


def parse_json(wire):
    def pairs(rows):
        result = {}
        for key, value in rows:
            require(key not in result, 'duplicate managed-access plan key')
            result[key] = value
        return result
    def invalid_number(_):
        raise ValueError('non-finite managed-access plan number')
    return json.loads(wire.decode('utf-8', errors='strict'), object_pairs_hook=pairs, parse_constant=invalid_number)


def exact(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return set(actual) == set(expected) and all(exact(actual[key], value) for key, value in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(exact(left, right) for left, right in zip(actual, expected))
    return actual == expected


def canonical_after():
    fiber = ROOT / 'experiments/php-wasix-fiber-probe'
    cgi = ROOT / 'experiments/php-wasix-cgi-build'
    references = [('fiber', fiber, '81cd79838d653453e27e2bb69ad3afefbbd01293846372080e2db8eb134b5c87'),
                  ('cgi', cgi, '8c625e73920e6dc3fa7f842da0f553b75b6f65ff17e64349dcb98359a62ffd28')]
    manifests = {}
    for key, directory, expected in references:
        wire = fixed(directory / 'manifest.json')
        require(hashlib.sha256(wire).hexdigest() == expected, 'canonical source manifest changed')
        manifests[key] = parse_json(wire)
    fiber_plan = manifests['fiber']['source_patches'][0]
    cgi_plan = manifests['cgi']['source_patches'][0]
    require(hashlib.sha256(fixed(fiber / fiber_plan['path'])).hexdigest() == fiber_plan['sha256']
            and hashlib.sha256(fixed(cgi / cgi_plan['path'])).hexdigest() == cgi_plan['sha256'], 'canonical patch bytes changed')
    require(fingerprint(fixed(fiber / 'fixtures/zend_wasix_fiber.S')) == fiber_plan['files'][2]['after'],
        'canonical Fiber assembly identity changed')
    require(manifests['fiber']['upstream']['commit'] == COMMIT
            and manifests['cgi']['sources']['php']['commit'] == COMMIT, 'canonical PHP source commit changed')
    result = {row['path']: row['after']['sha256'] for row in fiber_plan['files'] if row['change'] == 'modify'}
    result[cgi_plan['target']] = cgi_plan['after_sha256']
    all_targets = [row['path'] for row in fiber_plan['files']] + [cgi_plan['target']]
    require(set(all_targets).isdisjoint(TARGETS), 'managed patch overlaps canonical Fiber/CGI targets')
    return result, all_targets


def validate_plan(plan):
    require(type(plan['format_version']) is int and plan['format_version'] == 1 and plan['schema'] == 'recastpress.php-wasix-managed-access-patch-plan.v0.5',
        'unexpected managed-access patch plan')
    require(plan['upstream']['repository'] == 'wasix-org/php' and plan['upstream']['commit'] == COMMIT,
        'managed-access PHP source pin changed')
    require(type(plan['source_patches']) is list and len(plan['source_patches']) == 1, 'one fixed managed-access patch required')
    patch = plan['source_patches'][0]
    require(patch['path'] == PATCH and type(patch['bytes']) is int and patch['bytes'] == PATCH_BYTES
        and patch['sha256'] == PATCH_SHA, 'managed-access patch definition changed')
    wire = fixed(HERE / PATCH)
    require(len(wire) == PATCH_BYTES and hashlib.sha256(wire).hexdigest() == PATCH_SHA, 'managed-access patch bytes changed')
    expected = [{'path': name, 'change': 'modify' if BEFORE[name] is not None else 'add',
        'before': BEFORE[name], 'after': AFTER[name]} for name in TARGETS]
    require(exact(patch['files'], expected), 'fixed complete source before/after identities changed')
    require(fingerprint(fixed(HEADER)) == AFTER[TARGETS[2]], 'canonical header bytes changed')
    require(exact(plan['fixed_header'], {'path': 'fixtures/recastpress_fs_access.h',
        'bytes': AFTER[TARGETS[2]]['bytes'], 'sha256': AFTER[TARGETS[2]]['sha256'],
        'patch_destination': TARGETS[2], 'destination_bytes_equal': True}), 'header destination binding changed')
    _, targets = canonical_after()
    require(plan['canonical_patch_targets_disjoint'] == targets, 'canonical target declaration changed')
    require(all(value is False for value in plan['claims'].values()), 'source plan must not become execution/adoption proof')
    return patch


def load_plan():
    plan = parse_json(fixed(PLAN))
    validate_plan(plan)
    return plan


def input_hashes():
    paths = [Path(__file__).resolve(), PLAN, HERE / PATCH, HEADER,
        ROOT / 'experiments/php-wasix-fiber-probe/manifest.json',
        ROOT / 'experiments/php-wasix-cgi-build/manifest.json',
        ROOT / 'experiments/php-wasix-fiber-probe/patches/0001-nts-fresh-wasi-zend-fibers.patch',
        ROOT / 'experiments/php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S',
        ROOT / 'experiments/php-wasix-cgi-build/patches/0001-wasi-disable-cgi-prefork.patch']
    return {path.relative_to(ROOT).as_posix(): hashlib.sha256(fixed(path)).hexdigest() for path in paths}


def verify_applied(php, plan):
    require(php == PHP and php.resolve() == php, 'managed-access patch checkout escaped its fixed path')
    validate_plan(plan)
    after = {name: fingerprint(fixed(php / name)) for name in TARGETS}
    require(after == AFTER, 'managed-access complete patched source bytes changed')
    require(fixed(php / TARGETS[2]) == fixed(HEADER), 'installed managed header and canonical fixture differ')
    return after


def apply_to_checkout(php, plan, git):
    require(php == PHP and php.resolve() == php, 'managed-access patch checkout escaped its fixed path')
    patch = validate_plan(plan)
    require(git(['git', 'rev-parse', 'HEAD'], php).strip() == COMMIT.encode('ascii'), 'managed-access PHP HEAD changed')
    require(not git(['git', 'diff', '--cached', '--name-only'], php).strip(), 'source index has changes before patch')
    tracked_before = set(git(['git', 'diff', '--name-only', '-z'], php).decode('utf-8').split('\0')) - {''}
    canonical, _ = canonical_after()
    require(not tracked_before or tracked_before == set(canonical), 'prior changes are not the exact canonical patch set')
    if tracked_before:
        for name, expected in canonical.items():
            require(hashlib.sha256(fixed(php / name)).hexdigest() == expected, 'prior canonical patched source bytes differ')
        reference_assembly = ROOT / 'experiments/php-wasix-fiber-probe/fixtures/zend_wasix_fiber.S'
        require(fixed(php / 'Zend/asm/recastpress_wasix_fiber.S') == fixed(reference_assembly), 'prior Fiber assembly bytes differ')
    for name in TARGETS:
        target = php / name
        require(target.resolve() == target and not target.is_symlink(), 'managed-access target resolves through a link')
        if BEFORE[name] is None:
            require(not target.exists(), 'managed-access header already exists; reapplication refused')
        else:
            require(fingerprint(fixed(target)) == BEFORE[name], 'managed-access complete source base bytes mismatch')
            require(git(['git', 'rev-parse', 'HEAD:' + name], php).strip().decode('ascii') == BEFORE[name]['git_blob_sha1'],
                'managed-access source Git blob mismatch')
    patch_path = HERE / PATCH
    git(['git', 'apply', '--check', '--whitespace=nowarn', str(patch_path)], php)
    git(['git', 'apply', '--whitespace=nowarn', str(patch_path)], php)
    after = verify_applied(php, plan)
    require(not git(['git', 'diff', '--cached', '--name-only'], php).strip(), 'managed-access patch modified the Git index')
    changed = set(git(['git', 'diff', '--name-only', '-z'], php).decode('utf-8').split('\0')) - {''}
    require(changed == tracked_before | set(TARGETS[:2]), 'managed-access patch changed unexpected tracked source files')
    return {'path': PATCH, 'bytes': PATCH_BYTES, 'sha256': PATCH_SHA, 'source_commit': COMMIT,
        'fixed_own_targets_verified_before_patch': True,
        'tracked_sources_clean_before_patch': not bool(tracked_before),
        'prior_canonical_patch_set_verified': bool(tracked_before), 'tracked_changes_before_patch': sorted(tracked_before),
        'applied': True, 'git_apply_check_passed': True, 'git_apply_passed': True, 'index_updated': False,
        'before': BEFORE, 'after': after, 'installed_header_matches_fixed_fixture': True,
        'canonical_patch_targets_disjoint': True}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--validate-only', action='store_true', required=True)
    parser.parse_args()
    load_plan()
    print('Fixed managed-access patch/source/header definitions validated; no source checkout or PHP execution.')
