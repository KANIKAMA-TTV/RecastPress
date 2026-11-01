"""Fetch, pin, and verify the agreed WordPress source without executing PHP.

Only this version is accepted. SHA-1 matches the official published identity;
SHA-256 records the downloaded bytes. This is not a signature verification tool.
"""

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile
import unicodedata
import urllib.request
import uuid

ROOT = Path(__file__).resolve().parents[1]
REFERENCE = ROOT / ".reference"
VERSION = "7.1.3"
ARCHIVE_URL = f"https://wordpress.org/wordpress-{VERSION}.tar.gz"
CHECKSUM_URL = ARCHIVE_URL + ".sha1"
EXPECTED_SHA1 = "491a78c7676b15dffcc69130c6054b8f49228a91"
MAX_ARCHIVE_BYTES = 128 * 1024 * 1024
MAX_EXPANDED_BYTES = 512 * 1024 * 1024
MAX_MEMBERS = 30000
MAX_FILE_BYTES = 32 * 1024 * 1024
MANIFEST_NAME = "recastpress-reference.json"
DEVICE_NAMES = {"CON", "PRN", "AUX", "NUL", *[f"COM{i}" for i in range(1, 10)], *[f"LPT{i}" for i in range(1, 10)]}


def digest_file(path, algorithm="sha256"):
    digest = hashlib.new(algorithm)
    with Path(path).open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_relative(name):
    """Reject traversal, Windows aliases, noncanonical names, and outside roots."""
    if "\\" in name or name.startswith("/"):
        raise ValueError(f"unsafe archive path: {name!r}")
    parts = name.rstrip("/").split("/")
    if not parts or parts[0] != "wordpress":
        raise ValueError(f"unexpected archive root: {name!r}")
    for part in parts:
        if part in ("", ".", "..") or part.endswith((" ", ".")):
            raise ValueError(f"noncanonical archive path: {name!r}")
        if any(ord(c) < 32 or c in ':<>"|?*' for c in part):
            raise ValueError(f"unsafe archive filename: {name!r}")
        if part.split(".")[0].upper() in DEVICE_NAMES:
            raise ValueError(f"reserved archive filename: {name!r}")
    return PurePosixPath(*parts[1:])


def extract(archive, destination):
    """Extract into a fresh directory; never overwrite or follow archive links."""
    destination = Path(destination).resolve()
    destination.mkdir(parents=True, exist_ok=False)
    records = []
    seen = set()
    total = 0
    count = 0
    with tarfile.open(archive, "r|gz") as source:
        for member in source:
            count += 1
            if count > MAX_MEMBERS:
                raise ValueError("archive has too many members")
            relative = safe_relative(member.name)
            if not member.isdir() and not member.isfile():
                raise ValueError(f"archive links/special entries are forbidden: {member.name}")
            if relative == PurePosixPath("."):
                if not member.isdir():
                    raise ValueError("archive root is not a directory")
                continue
            canonical = unicodedata.normalize("NFC", relative.as_posix()).casefold()
            if canonical in seen:
                raise ValueError(f"duplicate/case-colliding archive path: {member.name}")
            seen.add(canonical)
            target = destination.joinpath(*relative.parts)
            if not target.resolve().is_relative_to(destination):
                raise ValueError("archive target escaped reference directory")
            if member.isdir():
                target.mkdir(parents=True, exist_ok=True)
                continue
            if member.size < 0 or member.size > MAX_FILE_BYTES:
                raise ValueError("archive member exceeds size limit")
            total += member.size
            if total > MAX_EXPANDED_BYTES:
                raise ValueError("expanded archive exceeds size limit")
            target.parent.mkdir(parents=True, exist_ok=True)
            digest = hashlib.sha256()
            remaining = member.size
            with source.extractfile(member) as data, target.open("xb") as output:
                while remaining:
                    chunk = data.read(min(1024 * 1024, remaining))
                    if not chunk:
                        raise ValueError("truncated archive member")
                    output.write(chunk)
                    digest.update(chunk)
                    remaining -= len(chunk)
            records.append({"path": relative.as_posix(), "bytes": member.size, "sha256": digest.hexdigest()})
    version_file = destination / "wp-includes" / "version.php"
    contents = version_file.read_text(encoding="utf-8")
    match = re.search(r"^\$wp_version\s*=\s*['\"]([^'\"]+)['\"]\s*;", contents, re.MULTILINE)
    if not match or match[1] != VERSION:
        raise ValueError("extracted WordPress version differs from pinned version")
    return sorted(records, key=lambda row: row["path"])


def download(url, destination, limit):
    request = urllib.request.Request(url, headers={"User-Agent": "RecastPress-reference-verifier/0.1"})
    with urllib.request.urlopen(request, timeout=30) as response, Path(destination).open("xb") as output:
        if not response.url.startswith("https://wordpress.org/"):
            raise ValueError("unexpected download redirect")
        total = 0
        while chunk := response.read(1024 * 1024):
            total += len(chunk)
            if total > limit:
                raise ValueError("download exceeds size limit")
            output.write(chunk)


def fetch():
    REFERENCE.mkdir(exist_ok=True)
    archive = REFERENCE / f"wordpress-{VERSION}.tar.gz"
    target = REFERENCE / f"wordpress-{VERSION}"
    if target.exists():
        verify()
        return
    if not archive.exists():
        checksum_file = REFERENCE / f"checksum-{uuid.uuid4().hex}.txt"
        download(CHECKSUM_URL, checksum_file, 256)
        published = checksum_file.read_text(encoding="ascii").strip().split()[0]
        if published != EXPECTED_SHA1:
            raise ValueError("official checksum differs from agreed identity; do not change baseline automatically")
        partial = REFERENCE / f"archive-{uuid.uuid4().hex}.part"
        download(ARCHIVE_URL, partial, MAX_ARCHIVE_BYTES)
        if digest_file(partial, "sha1") != EXPECTED_SHA1:
            raise ValueError("download SHA-1 does not match pinned official checksum")
        partial.rename(archive)
    if archive.stat().st_size > MAX_ARCHIVE_BYTES or digest_file(archive, "sha1") != EXPECTED_SHA1:
        raise ValueError("local archive is not the pinned WordPress archive")
    staging = REFERENCE / f"extract-{uuid.uuid4().hex}"
    files = extract(archive, staging)
    manifest = {
        "format_version": 1,
        "wordpress_version": VERSION,
        "archive_url": ARCHIVE_URL,
        "checksum_url": CHECKSUM_URL,
        "retrieved_at_utc": datetime.now(timezone.utc).isoformat(),
        "archive_sha1": EXPECTED_SHA1,
        "archive_sha256": digest_file(archive),
        "archive_bytes": archive.stat().st_size,
        "file_count": len(files),
        "files": files,
    }
    (staging / MANIFEST_NAME).write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    staging.rename(target)
    print(f"Pinned WordPress {VERSION}: {len(files)} files, SHA-256 {manifest['archive_sha256']}")


def verify():
    target = REFERENCE / f"wordpress-{VERSION}"
    manifest = json.loads((target / MANIFEST_NAME).read_text(encoding="utf-8"))
    if manifest["wordpress_version"] != VERSION or manifest["archive_sha1"] != EXPECTED_SHA1:
        raise ValueError("reference manifest changed baseline")
    archive = REFERENCE / f"wordpress-{VERSION}.tar.gz"
    if digest_file(archive, "sha1") != EXPECTED_SHA1 or digest_file(archive) != manifest["archive_sha256"]:
        raise ValueError("reference archive was modified")
    expected = {row["path"]: row for row in manifest["files"]}
    if len(expected) != manifest["file_count"] or len(expected) != len(manifest["files"]):
        raise ValueError("reference manifest has inconsistent counts")
    actual = set()
    for path in target.rglob("*"):
        if path.is_symlink():
            raise ValueError("reference tree contains a symbolic link")
        if not path.is_file() or path == target / MANIFEST_NAME:
            continue
        relative = path.relative_to(target).as_posix()
        actual.add(relative)
        row = expected.get(relative)
        if row is None or path.stat().st_size != row["bytes"] or digest_file(path) != row["sha256"]:
            raise ValueError(f"reference file changed: {relative}")
    if actual != set(expected):
        raise ValueError("reference files are missing")
    print(f"Verified WordPress {VERSION}: {len(actual)} unchanged files")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("operation", choices=["fetch", "verify"])
    options = parser.parse_args()
    try:
        {"fetch": fetch, "verify": verify}[options.operation]()
    except (OSError, ValueError, KeyError, tarfile.TarError) as error:
        parser.exit(1, f"Reference verification failed: {error}\n")
