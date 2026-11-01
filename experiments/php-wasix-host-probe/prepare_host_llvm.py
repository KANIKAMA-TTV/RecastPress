"""Verify/extract the fixed host LLVM archive in a fresh Linux CI job."""
from pathlib import Path
import hashlib
import json
import os
import subprocess
import tarfile

root = Path.cwd().resolve()
manifest = root / 'experiments/php-wasix-host-probe/llvm-toolchain.json'
identity = json.loads(manifest.read_text(encoding='utf-8'))
archive = root / '.reference' / identity['archive_filename']
if archive.stat().st_size != identity['archive_bytes']:
    raise SystemExit('LLVM archive length mismatch; refusing to extract')
checksum = hashlib.sha256()
with archive.open('rb') as stream:
    for block in iter(lambda: stream.read(1024 * 1024), b''):
        checksum.update(block)
if checksum.hexdigest() != identity['archive_sha256']:
    raise SystemExit('LLVM archive SHA-256 mismatch; refusing to extract')
prefix = (root / identity['extract_directory']).resolve()
if not prefix.is_relative_to(root / '.reference'):
    raise SystemExit('LLVM extraction target escaped .reference')
if identity['strip_components'] != 0:
    raise SystemExit('Unexpected LLVM archive layout')
prefix.mkdir(exist_ok=False)
# Wasmer's official build.yml extracts directly into LLVM_DIR; bin/
# is at this root. Python's data filter rejects escaping archive paths.
with tarfile.open(archive, 'r:xz') as source:
    source.extractall(prefix, filter='data')
llvm_config = prefix / identity['required_llvm_config']
if not llvm_config.resolve().is_relative_to(prefix) or not llvm_config.is_file():
    raise SystemExit('Verified LLVM archive lacks bin/llvm-config')
version = subprocess.check_output([str(llvm_config), '--version'], text=True, timeout=10).strip()
expected = identity['llvm_version_prefix']
if version != expected and not version.startswith(expected + '.'):
    raise SystemExit('LLVM version does not match the fixed 22.1 family')
with open(os.environ['GITHUB_ENV'], 'a', encoding='utf-8') as output:
    output.write(f'LLVM_SYS_221_PREFIX={prefix}\n')
    output.write(f'LD_LIBRARY_PATH={prefix / "lib"}\n')
with open(os.environ['GITHUB_PATH'], 'a', encoding='utf-8') as output:
    output.write(str(prefix / 'bin') + '\n')
evidence = root / 'experiments/php-wasix-host-probe/evidence'
evidence.mkdir(exist_ok=True)
(evidence / 'llvm-toolchain-verification.json').write_text(json.dumps({
    'format_version': 1,
    'verifier_source_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
    'llvm_toolchain_manifest_sha256': hashlib.sha256(manifest.read_bytes()).hexdigest(),
    'archive_bytes': archive.stat().st_size,
    'archive_sha256': checksum.hexdigest(),
    'llvm_config_version': version,
    'php_executed_by_this_step': False,
}, indent=2) + '\n', encoding='utf-8', newline='\n')
print(f'Pinned host LLVM verified: {version}')
