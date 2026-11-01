"""Download one pinned, ignored P0 fixture. Does not execute PHP or install tools."""
from pathlib import Path
import hashlib
import json
import shutil
import subprocess
import sys

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def verify(path, fixture):
    if path.stat().st_size != fixture['bytes']:
        raise ValueError('fixture length mismatch')
    data = path.read_bytes()
    if data[:8] != b'\x00asm\x01\x00\x00\x00':
        raise ValueError('not a core Wasm module')
    if hashlib.sha256(data).hexdigest() != fixture['sha256']:
        raise ValueError('fixture SHA-256 mismatch')


def main():
    if len(sys.argv) != 1:
        raise ValueError('this preparer accepts no arguments')
    fixture = json.loads((HERE / 'fixture.json').read_text(encoding='utf-8'))
    directory = ROOT / '.reference' / 'php-wasi-8.2.6'
    directory.mkdir(parents=True, exist_ok=True)
    destination = directory / fixture['filename']
    if destination.exists():
        verify(destination, fixture)
        print('Pinned PHP WASI fixture already verified.', flush=True)
        return
    partial = directory / 'php-cgi-8.2.6-slim.download'
    curl = shutil.which('curl.exe') or shutil.which('curl')
    if not curl:
        raise RuntimeError('curl is required only to prepare this development fixture')
    for attempt in range(1, 17):
        size = partial.stat().st_size if partial.exists() else 0
        if size > fixture['bytes']:
            raise ValueError('oversized partial fixture; refusing to resume')
        if size == fixture['bytes']:
            break
        print(f'Fixture download attempt {attempt}: {size}/{fixture["bytes"]} bytes', flush=True)
        result = subprocess.run([
            curl, '--location', '--fail', '--silent', '--show-error',
            '--proto', '=https', '--proto-redir', '=https',
            '--max-time', '45', '--max-filesize', str(fixture['bytes']),
            '--continue-at', '-', '--output', str(partial), fixture['url'],
        ], timeout=60, check=False, creationflags=getattr(subprocess, 'CREATE_NO_WINDOW', 0))
        if result.returncode not in (0, 28):
            raise RuntimeError(f'fixture download failed with curl exit {result.returncode}')
    verify(partial, fixture)
    partial.replace(destination)
    print(f'Pinned PHP WASI fixture verified: {fixture["sha256"]}', flush=True)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f'Cannot prepare PHP WASI fixture: {error}', file=sys.stderr)
        sys.exit(1)
