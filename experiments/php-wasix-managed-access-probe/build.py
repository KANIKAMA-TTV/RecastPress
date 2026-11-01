"""Closed managed-access profile using the tested CGI/Fiber build procedure.

Only this fixed profile is selectable; no caller path, PHP flags or tools are
accepted. The canonical tool directory and other source checkouts stay intact.
"""
import hashlib
import importlib.util
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1].resolve()
PARENT = ROOT / 'experiments/php-wasix-fiber-cgi-probe'


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


driver = load('rp_managed_fixed_cgi_build_driver', PARENT / 'build.py')
profile = load('rp_managed_fixed_source_profile', HERE / 'source_profile.py')
driver.HERE = HERE
driver.WORK = ROOT / 'target/php-wasix-managed-access-probe'
driver.MANIFEST = HERE / 'manifest.json'
driver.RECEIPT = HERE / 'evidence/build.json'
driver.VARIANT = 'nts-fresh-wasi-zend-fibers-managed-access-v1'
driver.PLAN_SCHEMA = 'recastpress.wasix-php-managed-access-build-plan.v0.5'
driver.BUILD_SCHEMA = 'recastpress.php-wasix-managed-access-build.v0.5'
driver.BUILD_SUCCESS = 'managed_access_candidate_built_not_executed'
driver.BUILD_FAILURE = 'managed_access_build_failed'
driver.CONFIGURE_DIFFERENCE = 'same opt-in NTS CGI flags; Fiber, prefork and explicit managed-access source patches'
driver.RAW = 'target/php-wasix-managed-access-probe/php-fiber-cgi.raw.wasm'
driver.WASM = 'target/php-wasix-managed-access-probe/php-fiber-cgi.wasm'
driver.LINKED = 'target/php-wasix-managed-access-probe/source/php/sapi/cgi/php-cgi'
driver.patch_source = profile
original_inputs = driver.input_hashes


def input_hashes():
    hashes = original_inputs()
    for path in [Path(__file__).resolve(), HERE / 'source_profile.py', HERE / 'patch-plan.json',
            HERE / 'patches/0001-wasi-managed-access.patch', HERE / 'fixtures/recastpress_fs_access.h',
            PARENT / 'manifest.json', PARENT / 'patch_source.py']:
        driver.fixed(path)
        hashes[path.relative_to(ROOT).as_posix()] = driver.digest(path)
    return dict(sorted(hashes.items()))


driver.input_hashes = input_hashes
VARIANT = driver.VARIANT
RECEIPT = driver.RECEIPT
MODULE = ROOT / driver.WASM


def verify_build():
    result = driver.verify_build()
    result.update(managed_access_patch_sha256=profile.access.PATCH_SHA,
        managed_access_header_sha256=hashlib.sha256(profile.access.fixed(profile.access.HEADER)).hexdigest(),
        source_profile_sha256=driver.digest(HERE / 'source_profile.py'),
        scope='same-job NTS CGI candidate with managed-access adaptation; not product ABI or full compatibility')
    return result


if __name__ == '__main__':
    raise SystemExit(driver.main())
