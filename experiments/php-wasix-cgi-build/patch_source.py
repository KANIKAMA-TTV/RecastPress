"""Apply the fixed source patch with standard Git and verified byte identities.

The caller verifies the fixed checkout HEAD and a clean tracked worktree first.
Git does not update the index; the resulting source SHA-256 must match the pin.
"""
import hashlib
import subprocess

import prepare

require = prepare.require


def sha256(wire):
    return hashlib.sha256(wire).hexdigest()


def git_blob_sha1(wire):
    return hashlib.sha1(b'blob ' + str(len(wire)).encode('ascii') + b'\0' + wire).hexdigest()


def apply_to_checkout(php, identity, row):
    patch_path = prepare.HERE.joinpath(*prepare.relative_parts(identity['path']))
    require(patch_path.resolve().is_relative_to(prepare.HERE.resolve()) and not patch_path.is_symlink(),
            'source patch path escaped experiment')
    patch = patch_path.read_bytes()
    row['observed_patch_sha256'] = sha256(patch)
    require(row['observed_patch_sha256'] == identity['sha256'], 'source patch SHA-256 mismatch')
    target = php.joinpath(*prepare.relative_parts(identity['target']))
    require(target.resolve().is_relative_to(php.resolve()) and not target.is_symlink(), 'source patch target escaped checkout')
    before = target.read_bytes()
    row.update({'observed_before_bytes': len(before), 'observed_before_sha256': sha256(before),
                'observed_before_git_blob_sha1': git_blob_sha1(before), 'applied': False})
    require(len(before) == identity['before_bytes'] and sha256(before) == identity['before_sha256']
            and git_blob_sha1(before) == identity['before_git_blob_sha1'], 'source patch input identity mismatch')
    for name, options in [('check', ['--check']), ('apply', [])]:
        result = subprocess.run(
            ['git', 'apply', *options, '--whitespace=nowarn', str(patch_path)],
            cwd=php, shell=False, stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=30)
        row['git_apply_' + name + '_exit_code'] = result.returncode
        require(len(result.stdout) + len(result.stderr) <= 65536, 'git apply output exceeded budget')
        detail = result.stderr.decode('utf-8', errors='replace').strip()[:4096]
        require(result.returncode == 0, 'git apply ' + name + ' failed: ' + detail)
    # Git may accept hunk offsets. The pinned complete input and output hashes
    # still reject any result other than the declared source transformation.
    row['observed_after_sha256'] = sha256(target.read_bytes())
    require(row['observed_after_sha256'] == identity['after_sha256'], 'source changed while applying patch')
    row['applied'] = True
