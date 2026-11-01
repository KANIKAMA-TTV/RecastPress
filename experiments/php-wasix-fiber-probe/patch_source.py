"""Apply only the pinned fresh-WASI Fiber patch to its independent checkout.

Standard Git applies the patch without an index update. Whole-file byte/hash/
Git-blob checks before and after reject changed bases, offsets and reapplication.
The caller supplies its finite, output-bounded Git command runner.
"""
import hashlib
from pathlib import Path
import re

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
PHP = ROOT / 'target/php-wasix-fiber-probe/source/php'
COMMIT = 'f4bfae79d5c19028349082f707930cf0951d23b9'
PATCH = 'patches/0001-nts-fresh-wasi-zend-fibers.patch'
PATCH_BYTES = 19479
PATCH_SHA = '2c334bcfcc34cb518971a8b7965ede50032b2813fa0c426ddfde206a8105d231'
TARGETS = ['configure.ac', 'Zend/zend_fibers.c', 'Zend/asm/recastpress_wasix_fiber.S']
ASSEMBLY = HERE / 'fixtures/zend_wasix_fiber.S'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fingerprint(wire):
    return {'bytes': len(wire), 'sha256': hashlib.sha256(wire).hexdigest(),
            'git_blob_sha1': hashlib.sha1(b'blob ' + str(len(wire)).encode('ascii') + b'\0' + wire).hexdigest()}


def fixed(path, limit=1024 * 1024):
    require(path.resolve(strict=True) == path and path.is_file(), 'linked/missing fixed patch input')
    size = path.stat().st_size
    require(0 < size <= limit, 'patch input byte budget')
    wire = path.read_bytes()
    require(len(wire) == size, 'patch input changed during read')
    return wire


def validate_plan(manifest):
    require(manifest['upstream']['commit'] == COMMIT, 'Fiber upstream commit changed')
    plan = manifest['source_patches']
    require(type(plan) is list and len(plan) == 1, 'exactly one Fiber patch required')
    plan = plan[0]
    require(plan['path'] == PATCH and plan['bytes'] == PATCH_BYTES and plan['sha256'] == PATCH_SHA,
            'fixed Fiber patch pin changed')
    patch = fixed(HERE / PATCH)
    require(len(patch) == PATCH_BYTES and hashlib.sha256(patch).hexdigest() == PATCH_SHA, 'Fiber patch bytes/hash changed')
    require([row['path'] for row in plan['files']] == TARGETS, 'Fiber patch target set changed')
    for index, row in enumerate(plan['files']):
        require(row['change'] == ('add' if index == 2 else 'modify'), 'unexpected patch change kind')
        require((row['before'] is None) == (index == 2), 'unexpected patch base policy')
        for identity in [row['before'], row['after']]:
            if identity is None:
                continue
            require(set(identity) == {'bytes', 'sha256', 'git_blob_sha1'}
                    and type(identity['bytes']) is int and 0 < identity['bytes'] <= 1024 * 1024
                    and re.fullmatch('[0-9a-f]{64}', identity['sha256'])
                    and re.fullmatch('[0-9a-f]{40}', identity['git_blob_sha1']), 'invalid patch file identity')
    assembly = fingerprint(fixed(ASSEMBLY))
    require(plan['files'][2]['after'] == assembly and manifest['fixed_assembly'] == {
        'path': 'fixtures/zend_wasix_fiber.S', 'bytes': assembly['bytes'], 'sha256': assembly['sha256'],
        'patch_destination': TARGETS[2], 'destination_bytes_equal': True}, 'added assembly differs from fixed fixture')
    require([row['path'] for row in manifest['unchanged_reference_headers']] == ['Zend/zend_fibers.h'],
            'unchanged Zend public header set changed')
    return plan


def verify_applied(php, manifest):
    require(php == PHP and php.resolve() == php, 'Fiber patch checkout is not the independent fixed path')
    plan = validate_plan(manifest)
    files = {row['path']: fingerprint(fixed(php / row['path'])) for row in plan['files']}
    require(files == {row['path']: row['after'] for row in plan['files']}, 'patched source fingerprint changed')
    require(fixed(php / TARGETS[2]) == fixed(ASSEMBLY), 'patched assembly and fixture bytes differ')
    for header in manifest['unchanged_reference_headers']:
        require(fingerprint(fixed(php / header['path'])) == {k: v for k, v in header.items() if k != 'path'},
                'Zend public Fiber header changed')
    return files


def apply_to_checkout(php, manifest, git):
    require(php == PHP and php.resolve() == php, 'Fiber patch checkout escaped fixed independent path')
    plan = validate_plan(manifest)
    require(git(['git', 'rev-parse', 'HEAD'], php).strip() == COMMIT.encode(), 'Fiber checkout HEAD changed')
    require(not git(['git', 'status', '--porcelain', '--untracked-files=no'], php).strip(),
            'tracked PHP source is not clean immediately before patch')
    before = {}
    for row in plan['files']:
        target = php / row['path']
        require(target.resolve() == target and not target.is_symlink(), 'patch target resolves through a link')
        if row['before'] is None:
            require(not target.exists(), 'added assembly already exists; reapplication refused')
            before[row['path']] = None
        else:
            before[row['path']] = fingerprint(fixed(target))
            require(before[row['path']] == row['before'], 'PHP patch base bytes/blob/hash mismatch')
            blob = git(['git', 'rev-parse', 'HEAD:' + row['path']], php).strip().decode('ascii')
            require(blob == row['before']['git_blob_sha1'], 'Git source blob identity mismatch')
    for header in manifest['unchanged_reference_headers']:
        require(fingerprint(fixed(php / header['path'])) == {k: v for k, v in header.items() if k != 'path'},
                'Zend public header does not match fixed source')
    patch = HERE / PATCH
    git(['git', 'apply', '--check', '--whitespace=nowarn', str(patch)], php)
    git(['git', 'apply', '--whitespace=nowarn', str(patch)], php)
    after = verify_applied(php, manifest)
    require(not git(['git', 'diff', '--cached', '--name-only'], php).strip(), 'patch unexpectedly modified the index')
    changed = set(git(['git', 'diff', '--name-only', '-z'], php).decode('utf-8').split('\0')) - {''}
    require(changed == set(TARGETS[:2]), 'patch changed unexpected tracked files')
    return {'path': PATCH, 'bytes': PATCH_BYTES, 'sha256': PATCH_SHA, 'source_commit': COMMIT,
            'clean_tracked_source_immediately_before': True, 'applied': True, 'git_apply_check_passed': True,
            'git_apply_passed': True, 'index_updated': False, 'before': before, 'after': after,
            'added_assembly_matches_fixed_fixture': True}
