"""Apply the two unchanged, pinned PHP patches to one independent CGI checkout.

No canonical source or patch helper is modified. The Fiber helper is reused only
for its pure plan/fingerprint operations; this adapter owns the combined path.
"""
import hashlib
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
PHP = ROOT / 'target/php-wasix-fiber-cgi-probe/source/php'
FIBER = ROOT / 'experiments/php-wasix-fiber-probe'
CGI = ROOT / 'experiments/php-wasix-cgi-build'
COMMIT = 'f4bfae79d5c19028349082f707930cf0951d23b9'


def load_unique(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


canonical = load_unique('recastpress_combined_cgi_canonical_fiber_patch', FIBER / 'patch_source.py')
require, fingerprint, fixed = canonical.require, canonical.fingerprint, canonical.fixed
PATCH_SHA = canonical.PATCH_SHA
ASSEMBLY = canonical.ASSEMBLY
CGI_PATCH_SHA = '941f0f2ce80edcc0077e746bb7e080b163e837a1e60fb3a2faa21ca4008da1e2'


def validate_plan(manifest):
    require(manifest['upstream']['commit'] == COMMIT, 'combined PHP source commit changed')
    references = manifest['canonical_manifests']
    require(set(references) == {'fiber', 'cgi'}, 'canonical manifest set changed')
    for name, directory in [('fiber', FIBER), ('cgi', CGI)]:
        identity = references[name]
        require(identity['path'] == (directory / 'manifest.json').relative_to(ROOT).as_posix()
                and hashlib.sha256(fixed(directory / 'manifest.json')).hexdigest() == identity['sha256'],
                'canonical manifest bytes changed: ' + name)
    fiber_manifest = json.loads(fixed(FIBER / 'manifest.json'))
    fiber_plan = canonical.validate_plan(fiber_manifest)
    cgi_manifest = json.loads(fixed(CGI / 'manifest.json'))
    require(cgi_manifest['sources']['php']['commit'] == COMMIT
            and len(cgi_manifest['source_patches']) == 1, 'canonical CGI source or patch set changed')
    cgi_plan = cgi_manifest['source_patches'][0]
    require(cgi_plan['target'] == 'sapi/cgi/cgi_main.c'
            and cgi_plan['path'] == 'patches/0001-wasi-disable-cgi-prefork.patch'
            and cgi_plan['sha256'] == CGI_PATCH_SHA
            and hashlib.sha256(fixed(CGI / cgi_plan['path'])).hexdigest() == CGI_PATCH_SHA,
            'canonical CGI patch bytes/target changed')
    plans = [
        {'kind': 'fiber', 'path': (FIBER / fiber_plan['path']).relative_to(ROOT).as_posix(),
         'bytes': fiber_plan['bytes'], 'sha256': fiber_plan['sha256'], 'files': fiber_plan['files']},
        {'kind': 'cgi_prefork', 'path': (CGI / cgi_plan['path']).relative_to(ROOT).as_posix(),
         'bytes': len(fixed(CGI / cgi_plan['path'])), 'sha256': cgi_plan['sha256'],
         'files': [{'path': cgi_plan['target'], 'change': 'modify',
                    'before': {'bytes': cgi_plan['before_bytes'], 'sha256': cgi_plan['before_sha256'],
                               'git_blob_sha1': cgi_plan['before_git_blob_sha1']},
                    'after': {'sha256': cgi_plan['after_sha256']}}]},
    ]
    require(manifest['source_patches'] == plans, 'combined patch definitions changed')
    require(not set(row['path'] for row in plans[0]['files']) & set(row['path'] for row in plans[1]['files']),
            'canonical patch target sets overlap')
    return plans, fiber_manifest


def verify_applied(php, manifest):
    require(php == PHP and php.resolve() == php, 'combined patch checkout escaped the fixed path')
    plans, fiber_manifest = validate_plan(manifest)
    observed = []
    for plan in plans:
        files = {row['path']: fingerprint(fixed(php / row['path'])) for row in plan['files']}
        for row in plan['files']:
            require(all(files[row['path']][key] == value for key, value in row['after'].items()),
                    'combined patched source fingerprint changed: ' + row['path'])
        observed.append(files)
    require(fixed(php / canonical.TARGETS[2]) == fixed(ASSEMBLY), 'combined patched assembly differs')
    for header in fiber_manifest['unchanged_reference_headers']:
        require(fingerprint(fixed(php / header['path'])) == {k: v for k, v in header.items() if k != 'path'},
                'Zend public Fiber header changed')
    return observed


def receipt_rows(manifest, after):
    plans, _ = validate_plan(manifest)
    return [{'kind': plan['kind'], 'path': plan['path'], 'bytes': plan['bytes'], 'sha256': plan['sha256'],
             'source_commit': COMMIT, 'combined_tracked_source_clean_before_first_patch': True,
             'applied': True, 'git_apply_check_passed': True, 'git_apply_passed': True, 'index_updated': False,
             'before': {row['path']: row['before'] for row in plan['files']}, 'after': files}
            for plan, files in zip(plans, after)]


def apply_to_checkout(php, manifest, git):
    require(php == PHP and php.resolve() == php, 'combined patch checkout escaped the fixed path')
    plans, fiber_manifest = validate_plan(manifest)
    require(git(['git', 'rev-parse', 'HEAD'], php).strip() == COMMIT.encode(), 'combined checkout HEAD changed')
    require(not git(['git', 'status', '--porcelain', '--untracked-files=no'], php).strip(),
            'tracked PHP source is not clean before combined patches')
    # Validate both complete bases before applying either patch. The second
    # patch deliberately sees the first patch's declared tracked changes.
    for plan in plans:
        for row in plan['files']:
            target = php / row['path']
            require(target.resolve() == target and not target.is_symlink(), 'combined patch target is linked')
            if row['before'] is None:
                require(not target.exists(), 'combined added assembly exists; reapplication refused')
            else:
                require(fingerprint(fixed(target)) == row['before'], 'combined patch base mismatch')
                require(git(['git', 'rev-parse', 'HEAD:' + row['path']], php).strip().decode('ascii')
                        == row['before']['git_blob_sha1'], 'combined patch Git blob mismatch')
    for header in fiber_manifest['unchanged_reference_headers']:
        require(fingerprint(fixed(php / header['path'])) == {k: v for k, v in header.items() if k != 'path'},
                'Zend public header differs before combined patch')
    for plan in plans:
        path = ROOT / plan['path']
        git(['git', 'apply', '--check', '--whitespace=nowarn', str(path)], php)
        git(['git', 'apply', '--whitespace=nowarn', str(path)], php)
    after = verify_applied(php, manifest)
    require(not git(['git', 'diff', '--cached', '--name-only'], php).strip(), 'combined patch modified the index')
    changed = set(git(['git', 'diff', '--name-only', '-z'], php).decode('utf-8').split('\0')) - {''}
    expected = {row['path'] for plan in plans for row in plan['files'] if row['change'] == 'modify'}
    require(changed == expected, 'combined patch changed unexpected tracked files')
    return receipt_rows(manifest, after)
