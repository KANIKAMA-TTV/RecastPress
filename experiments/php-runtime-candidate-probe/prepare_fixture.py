"""Verify one already downloaded official Actions ZIP and extract its pinned Wasm."""
from pathlib import Path
import hashlib
import json
import stat
import sys
import zipfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def main():
    if len(sys.argv) != 1:
        raise ValueError('fixed artifact preparer accepts no arguments')
    identity = json.loads((HERE / 'artifact.json').read_text(encoding='utf-8'))
    directory = ROOT / '.reference/php-wasix-8.5.12'
    archive = directory / 'php-wasix-64.zip'
    if archive.stat().st_size != identity['zip_bytes']:
        raise ValueError('wrong ZIP length')
    if hashlib.sha256(archive.read_bytes()).hexdigest() != identity['zip_sha256']:
        raise ValueError('wrong ZIP SHA-256')
    with zipfile.ZipFile(archive) as source:
        entries = source.infolist()
        if len(entries) != 1 or entries[0].filename != 'php.wasm':
            raise ValueError('unexpected artifact ZIP members')
        entry = entries[0]
        if stat.S_ISLNK(entry.external_attr >> 16) or entry.file_size != identity['wasm_bytes']:
            raise ValueError('wrong artifact type or length')
        with source.open(entry) as stream:
            data = stream.read(identity['wasm_bytes'] + 1)
        if len(data) != identity['wasm_bytes'] or data[:8] != b'\x00asm\x01\x00\x00\x00':
            raise ValueError('wrong core Wasm module')
        if hashlib.sha256(data).hexdigest() != identity['wasm_sha256']:
            raise ValueError('wrong Wasm SHA-256')
    destination = directory / 'php.wasm'
    if destination.exists():
        if hashlib.sha256(destination.read_bytes()).hexdigest() != identity['wasm_sha256']:
            raise ValueError('existing extracted artifact changed')
    else:
        with destination.open('xb') as output:
            output.write(data)
    print('Pinned PHP 8.5.12 Actions ZIP and extracted Wasm verified.')


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, zipfile.BadZipFile) as error:
        print(f'Artifact preparation failed: {error}', file=sys.stderr)
        sys.exit(1)
