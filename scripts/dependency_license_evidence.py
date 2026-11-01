"""Observe pinned local Cargo license/notice files; no legal conclusions or extraction."""

import argparse
import gzip
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile

import dependency_inventory as declarations

ROOT = declarations.ROOT
OUTPUT = ROOT / "compatibility/dependency-license-evidence.json"
MAX_ARCHIVE = 64 * 1024 * 1024
MAX_EXPANDED = 128 * 1024 * 1024
MAX_MEMBERS = 32768
MAX_PATH = 4096
MAX_CANDIDATES = 4096
MAX_SELECTED = 1024 * 1024
MAX_JSON = 16 * 1024 * 1024
MAX_HEADER_NESTING = 32
NAME = re.compile(r"(?:LICENSE|LICENCE|NOTICE|COPYING|COPYRIGHT|UNLICENSE)(?:[-_.].*)?", re.ASCII | re.IGNORECASE)
SHA256 = re.compile(r"[0-9a-f]{64}", re.ASCII)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError("duplicate JSON field")
        result[key] = value
    return result


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    """Keep JSON boolean/integer/float identities distinct; reject non-JSON numbers."""
    return json.dumps(value, ensure_ascii=True, sort_keys=True, separators=(",", ":"), allow_nan=False)


def bounded_read(reader, limit):
    data = reader.read(limit + 1)
    if len(data) > limit:
        raise ValueError("input exceeds byte bound")
    return data


def relative_path(value):
    if not isinstance(value, str) or not value or len(value.encode("utf-8")) > MAX_PATH:
        raise ValueError("invalid relative path length")
    if value.startswith(("/", "\\")) or "\\" in value or ":" in value or any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise ValueError("unsupported relative path")
    if any(part in ("", ".", "..") for part in value.split("/")):
        raise ValueError("noncanonical relative path")
    return value


def reject_links(path):
    """Reject observed links/reparse points on every existing component, not just its leaf."""
    path = Path(path).absolute()
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        metadata = current.lstat()
        if stat.S_ISLNK(metadata.st_mode) or getattr(metadata, "st_file_attributes", 0) & 0x400:
            raise ValueError("symlink or reparse point in observed path")
    return path


def read_regular(path, limit, root=None):
    path = reject_links(path)
    if root is not None and not path.resolve().is_relative_to(Path(root).resolve()):
        raise ValueError("observed file escapes package")
    if not stat.S_ISREG(path.lstat().st_mode):
        raise ValueError("observed input is not a regular file")
    # The same opened handle is bounded and hashed; path opens remain non-atomic.
    with path.open("rb") as reader:
        if not stat.S_ISREG(os.fstat(reader.fileno()).st_mode):
            raise ValueError("opened input is not a regular file")
        data = bounded_read(reader, limit)
    reject_links(path)
    if root is not None and not path.resolve().is_relative_to(Path(root).resolve()):
        raise ValueError("observed file changed containment")
    return data


class ExpandedReader:
    """Bound all decompressed bytes, including skipped members and trailing data."""
    def __init__(self, reader):
        self.reader = reader
        self.count = 0

    def read(self, size=-1):
        allowance = MAX_EXPANDED - self.count + 1
        data = self.reader.read(allowance if size < 0 else min(size, allowance))
        self.count += len(data)
        if self.count > MAX_EXPANDED:
            raise ValueError("archive exceeds expanded byte bound")
        return data


class BoundedTarInfo(tarfile.TarInfo):
    def _proc_member(self, archive):
        # Count GNU/PAX extension headers as well as returned file members.
        archive.observed_headers = getattr(archive, "observed_headers", 0) + 1
        archive.header_depth = getattr(archive, "header_depth", 0) + 1
        if archive.header_depth > MAX_HEADER_NESTING:
            raise ValueError("archive extension headers exceed nesting bound")
        try:
            if archive.observed_headers > MAX_MEMBERS:
                raise ValueError("archive exceeds physical header bound")
            return super()._proc_member(archive)
        finally:
            archive.header_depth -= 1


def checksum_record(package_root, archive_sha256):
    path = package_root / ".cargo-checksum.json"
    try:
        path.lstat()
    except FileNotFoundError:
        return {"status": "missing"}, None
    raw = read_regular(path, MAX_JSON, package_root)
    value = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(value, dict) or set(value) != {"package", "files"} or value["package"] != archive_sha256 or not isinstance(value["files"], dict):
        raise ValueError("checksum JSON package binding is invalid")
    for path, expected in value["files"].items():
        relative_path(path)
        if not isinstance(expected, str) or not SHA256.fullmatch(expected):
            raise ValueError("checksum JSON file hash is invalid")
    return {"status": "verified", "sha256": digest(raw), "package_sha256": value["package"]}, value["files"]


def observe_registry(record, package_root, archive_path):
    package_root = Path(package_root)
    expected = record["locked_archive_sha256"]
    if not isinstance(expected, str) or not SHA256.fullmatch(expected):
        raise ValueError("locked archive hash is invalid")
    # Read compressed bytes exactly once. All subsequent archive work uses this immutable object.
    compressed = read_regular(archive_path, MAX_ARCHIVE)
    if digest(compressed) != expected:
        raise ValueError("archive bytes differ from Cargo.lock")
    checksum, files = checksum_record(package_root, expected)
    declared = record.get("declared_license_file", {}).get("path")
    if declared is not None:
        relative_path(declared)
    prefix = record["name"] + "-" + record["version"]
    relative_path(prefix)
    seen, candidates = set(), {}
    with gzip.GzipFile(fileobj=io.BytesIO(compressed), mode="rb") as uncompressed:
        expanded = ExpandedReader(uncompressed)
        with tarfile.open(fileobj=expanded, mode="r|", tarinfo=BoundedTarInfo) as archive:
            for member in archive:
                if len(seen) >= MAX_MEMBERS:
                    raise ValueError("archive exceeds member bound")
                name = member.name
                if member.isdir() and name.endswith("/"):
                    name = name[:-1]
                relative_path(name)
                if name in seen:
                    raise ValueError("duplicate archive member")
                seen.add(name)
                parts = PurePosixPath(name).parts
                if parts[0] != prefix or (len(parts) == 1 and not member.isdir()):
                    raise ValueError("archive member has wrong package root")
                if len(parts) == 1:
                    continue
                relative = "/".join(parts[1:])
                reasons = []
                if NAME.fullmatch(parts[-1]):
                    reasons.append("license_notice_basename")
                if relative == declared:
                    reasons.append("declared_license_file")
                if not reasons:
                    continue
                if len(candidates) >= MAX_CANDIDATES:
                    raise ValueError("archive exceeds candidate bound")
                if member.type not in (tarfile.REGTYPE, tarfile.AREGTYPE) or member.sparse is not None:
                    raise ValueError("selected archive member is not a regular file")
                if not 0 <= member.size <= MAX_SELECTED:
                    raise ValueError("selected archive file exceeds byte bound")
                reader = archive.extractfile(member)
                if reader is None:
                    raise ValueError("selected archive file is unreadable")
                with reader:
                    archived = bounded_read(reader, MAX_SELECTED)
                local = read_regular(package_root / relative, MAX_SELECTED, package_root)
                sha256 = digest(local)
                if len(archived) != member.size or archived != local:
                    raise ValueError("selected local file differs from locked archive")
                if files is not None and files.get(relative) != sha256:
                    raise ValueError("selected file differs from checksum JSON")
                if relative == declared and record["declared_license_file"].get("sha256") != sha256:
                    raise ValueError("declared license file differs from inventory")
                candidates[relative] = {"path": relative, "selection_reasons": reasons,
                    "size_bytes": len(local), "local_sha256": sha256,
                    "archive_member_sha256": digest(archived), "status": "matched",
                    "checksum_file_status": "matched" if files is not None else "unavailable"}
        # gzip integrity and expansion bounds apply even beyond tar's end marker.
        while expanded.read(64 * 1024):
            pass
    if declared is not None and declared not in candidates:
        raise ValueError("declared license file is absent from archive")
    return {"name": record["name"], "version": record["version"], "source": record["source"],
        "declared_license": record["declared_license"], "locked_archive_sha256": expected,
        "observed_archive_sha256": digest(compressed), "archive_status": "matched",
        "checksum_json": checksum, "discovery_status": "candidates_observed" if candidates else "candidate_not_found",
        "selected_files": [candidates[path] for path in sorted(candidates)]}


def evidence():
    inventory_path = ROOT / "compatibility/dependency-inventory.json"
    inventory_bytes = read_regular(inventory_path, MAX_JSON, ROOT)
    inventory = json.loads(inventory_bytes, object_pairs_hook=unique_object)
    if canonical(inventory) != canonical(declarations.inventory()):
        raise ValueError("declaration inventory is stale")
    resolved = {}
    for manifest in declarations.PROJECTS.values():
        result = subprocess.run(["cargo", "metadata", "--format-version", "1", "--offline", "--locked", "--manifest-path", str(ROOT / manifest)], cwd=ROOT,
            check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
        metadata = json.loads(result.stdout, object_pairs_hook=unique_object)
        active = {node["id"] for node in metadata["resolve"]["nodes"]}
        for package in metadata["packages"]:
            if package["id"] not in active:
                continue
            name, version, source = (package[key] for key in ("name", "version", "source"))
            package_root = Path(package["manifest_path"]).parent
            if source is None:
                relative = package_root.resolve().relative_to(ROOT.resolve()).as_posix()
                identity = f"{name}@{version} (path:{relative})"
            else:
                identity = f"{name}@{version} ({source})"
            record = inventory["packages"].get(identity)
            if record is None or (record["name"], record["version"], record["source"], record["declared_license"]) != (name, version, source, package["license"]):
                raise ValueError("resolved package identity differs from inventory")
            if identity in resolved and resolved[identity] != package_root:
                raise ValueError("resolved package cache locations disagree")
            resolved[identity] = package_root
    if set(resolved) != set(inventory["packages"]):
        raise ValueError("resolved package set differs from inventory")
    packages = {}
    for identity in sorted(resolved):
        record, package_root = inventory["packages"][identity], resolved[identity]
        if record["source"] is None:
            packages[identity] = {"name": record["name"], "version": record["version"], "source": None,
                "local_path": record["local_path"], "declared_license": record["declared_license"], "archive_status": "local_not_archive"}
        elif record["source"].startswith("registry+"):
            if package_root.name != record["name"] + "-" + record["version"] or package_root.parent.parent.name != "src":
                raise ValueError("unsupported registry cache layout")
            archive = package_root.parents[2] / "cache" / package_root.parent.name / (package_root.name + ".crate")
            packages[identity] = observe_registry(record, package_root, archive)
        else:
            raise ValueError("nonregistry external packages are unsupported")
    inputs = set(inventory["input_files"]) | {"compatibility/dependency-inventory.json", "scripts/dependency_license_evidence.py", "scripts/test_dependency_license_evidence.py"}
    return {"format_version": 1, "scope": "locked_local_cargo_license_notice_file_observations",
        "python_runtime": {"implementation": sys.implementation.name,
            "version": ".".join(str(part) for part in sys.version_info[:3]),
            "releaselevel": sys.version_info.releaselevel, "serial": sys.version_info.serial},
        "limitations": ["File observations do not choose licenses, determine compatibility, or complete a legal audit or distribution obligations.",
            "Filename discovery may miss notices in README, source comments, bundled data or other names; candidate_not_found is not a legal conclusion.",
            "All resolved target/build/dev packages are included, not a specific release binary's linked component set.",
            "Archive hashes bind to the pinned lockfiles, not independently verified publisher signatures or authenticity.",
            "Local reads and workspace writes are non-atomic and assume a trusted stable cache/workspace; link and output-path checks cannot exclude TOCTOU or ABA. No continuous monitoring is provided.",
            "Selected archive links and nonregular files are rejected; unselected links are not extracted, followed or traversed. This is not an archive-wide no-link claim.",
            "Physical GNU/PAX header bounds depend on the private TarInfo._proc_member hook, tested on Python 3.13 only; Python 3.12 and other runtimes are not verified. Rerun synthetic PAX and nesting-bound regressions on every new runtime before regenerating evidence."],
        "bounds": {"compressed_bytes": MAX_ARCHIVE, "expanded_bytes": MAX_EXPANDED, "members": MAX_MEMBERS,
            "path_bytes": MAX_PATH, "selected_files": MAX_CANDIDATES, "selected_file_bytes": MAX_SELECTED,
            "json_bytes": MAX_JSON, "extension_header_nesting": MAX_HEADER_NESTING},
        "input_files": {path: digest(read_regular(ROOT / relative_path(path), MAX_JSON, ROOT)) for path in sorted(inputs)},
        "packages": packages}


def validate_output_path(path):
    """Check the existing parent and, when present, destination before any write."""
    path = Path(path).absolute()
    reject_links(ROOT)
    parent = reject_links(path.parent)
    if not stat.S_ISDIR(parent.lstat().st_mode) or not parent.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("output parent is not a directory inside the workspace")
    try:
        metadata = path.lstat()
    except FileNotFoundError:
        metadata = None
    if metadata is not None:
        reject_links(path)
        if not stat.S_ISREG(metadata.st_mode):
            raise ValueError("existing output is not a regular file")
        if metadata.st_nlink != 1:
            raise ValueError("existing output has multiple hard links")
    if not path.resolve().is_relative_to(ROOT.resolve()):
        raise ValueError("output escapes workspace")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="regenerate and compare without writing")
    options = parser.parse_args()
    result = evidence()
    wire = json.dumps(result, ensure_ascii=True, indent=2) + "\n"
    if len(wire.encode("utf-8")) > MAX_JSON:
        raise ValueError("generated evidence exceeds JSON byte bound")
    if options.check:
        if read_regular(OUTPUT, MAX_JSON, ROOT).decode("utf-8") != wire:
            raise ValueError("license evidence is stale")
    else:
        validate_output_path(OUTPUT).write_text(wire, encoding="utf-8", newline="\n")
    print(f"Locked license/notice file observations match: {len(result['packages'])} packages; legal audit remains pending.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, UnicodeError, tarfile.TarError, EOFError, subprocess.SubprocessError):
        # Do not expose absolute cache paths, command arguments or environment data.
        raise SystemExit("Dependency license evidence failed; no complete observation was produced.") from None
