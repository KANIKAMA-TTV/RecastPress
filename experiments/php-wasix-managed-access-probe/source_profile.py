"""Compose the unchanged Fiber/CGI patches with the fixed access adaptation."""
import hashlib
import importlib.util
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
PARENT = ROOT / 'experiments/php-wasix-fiber-cgi-probe'
PHP = ROOT / 'target/php-wasix-managed-access-probe/source/php'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


canonical = load('rp_managed_canonical_patch_chain', PARENT / 'patch_source.py')
canonical.PHP = PHP
access = load('rp_managed_access_patch_chain', HERE / 'patch_source.py')
require = access.require
PATCH_SHA, CGI_PATCH_SHA, ASSEMBLY = canonical.PATCH_SHA, canonical.CGI_PATCH_SHA, canonical.ASSEMBLY


def validate_plan(manifest):
    parent_wire = access.fixed(PARENT / 'manifest.json')
    require(manifest['parent_profile'] == {'path': 'experiments/php-wasix-fiber-cgi-probe/manifest.json',
        'sha256': hashlib.sha256(parent_wire).hexdigest()}, 'parent CGI/Fiber profile changed')
    parent = json.loads(parent_wire)
    canonical.validate_plan(parent)
    plan = access.load_plan()
    managed = {'kind': 'managed_access', **plan['source_patches'][0]}
    managed['path'] = (HERE / managed['path']).relative_to(ROOT).as_posix()
    require(access.exact(manifest['source_patches'], parent['source_patches'] + [managed]),
        'ordered three-patch profile changed')
    for key in ['upstream', 'canonical_manifests', 'build_design', 'comparison_fixture', 'canonical_comparison_fixture']:
        require(access.exact(manifest[key], parent[key]), 'inherited PHP build policy changed: ' + key)
    require(manifest['managed_access'] == {'patch_plan': (HERE / 'patch-plan.json').relative_to(ROOT).as_posix(),
        'patch_plan_sha256': hashlib.sha256(access.fixed(HERE / 'patch-plan.json')).hexdigest(),
        'header': access.HEADER.relative_to(ROOT).as_posix(), 'header_sha256': hashlib.sha256(access.fixed(access.HEADER)).hexdigest(),
        'endpoint': '/rp-p0-fs/access', 'max_path_bytes': 4096,
        'request': 'RPA1 + mode-u8 + path-length-u16le + absolute-UTF8-path',
        'response': 'RPR1 + status-u8: allow/denied/missing/unmanaged/invalid/io',
        'product_abi_selected': False}, 'managed access protocol/source plan changed')
    prior_targets = {row['path'] for patch in parent['source_patches'] for row in patch['files']}
    require(not prior_targets & set(access.TARGETS), 'managed patch overlaps canonical source targets')
    return parent, plan


def verify_applied(php, manifest):
    parent, plan = validate_plan(manifest)
    return canonical.verify_applied(php, parent) + [access.verify_applied(php, plan)]


def receipt_rows(manifest, after):
    parent, plan = validate_plan(manifest)
    require(type(after) is list and len(after) == 3, 'three observed patch outputs required')
    row = {'kind': 'managed_access', 'path': (HERE / access.PATCH).relative_to(ROOT).as_posix(),
        'bytes': access.PATCH_BYTES, 'sha256': access.PATCH_SHA, 'source_commit': access.COMMIT,
        'fixed_own_targets_verified_before_patch': True, 'tracked_sources_clean_before_patch': False,
        'prior_canonical_patch_set_verified': True,
        'tracked_changes_before_patch': sorted(access.canonical_after()[0]),
        'applied': True, 'git_apply_check_passed': True, 'git_apply_passed': True, 'index_updated': False,
        'before': access.BEFORE, 'after': after[2], 'installed_header_matches_fixed_fixture': True,
        'canonical_patch_targets_disjoint': True}
    return canonical.receipt_rows(parent, after[:2]) + [row]


def apply_to_checkout(php, manifest, git):
    parent, plan = validate_plan(manifest)
    first = canonical.apply_to_checkout(php, parent, git)
    last = access.apply_to_checkout(php, plan, git)
    last = {'kind': 'managed_access', **last}
    last['path'] = (HERE / access.PATCH).relative_to(ROOT).as_posix()
    observed = first + [last]
    require(observed == receipt_rows(manifest, verify_applied(php, manifest)), 'complete applied patch chain differs')
    return observed
