"""Record locked Cargo license declarations, without choosing or approving licenses."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tomllib

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "compatibility/dependency-inventory.json"
PROJECTS = {"core_workspace": "Cargo.toml", "isolated_binlog_probe": "experiments/binlog-probe/Cargo.toml"}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_record(package, locked, root=ROOT):
    """Cross-check registry identities; keep local/cache absolute paths private."""
    name, version, source = (package[key] for key in ["name", "version", "source"])
    lock = locked.get((name, version, source))
    if lock is None:
        raise ValueError(f"resolved package absent from lockfile: {name}@{version}")
    package_root = Path(package["manifest_path"]).resolve().parent
    record = {"name": name, "version": version, "source": source, "declared_license": package["license"]}
    if source is None:
        if not package_root.is_relative_to(root.resolve()):
            raise ValueError("local dependency is outside the repository")
        record["local_path"] = package_root.relative_to(root.resolve()).as_posix()
    elif source.startswith("registry+"):
        checksum = lock.get("checksum", "")
        if len(checksum) != 64 or any(byte not in "0123456789abcdef" for byte in checksum):
            raise ValueError(f"registry checksum absent or malformed: {name}@{version}")
        record["locked_archive_sha256"] = checksum
    if package.get("license_file"):
        license_file = Path(package["license_file"])
        if not license_file.is_absolute():
            license_file = package_root / license_file
        license_file = license_file.resolve()
        if not license_file.is_relative_to(package_root) or not license_file.is_file():
            raise ValueError(f"declared license file escapes/is missing: {name}@{version}")
        record["declared_license_file"] = {
            "path": license_file.relative_to(package_root).as_posix(), "sha256": digest(license_file),
        }
    identity = f"{name}@{version} ({source or 'path:' + record['local_path']})"
    return identity, record


def inventory():
    projects, packages = {}, {}
    inputs = {"scripts/dependency_inventory.py"}
    for project, manifest in PROJECTS.items():
        manifest_path = ROOT / manifest
        lock_path = manifest_path.with_name("Cargo.lock")
        lock_data = tomllib.loads(lock_path.read_text(encoding="utf-8"))
        locked = {(package["name"], package["version"], package.get("source")): package for package in lock_data["package"]}
        completed = subprocess.run([
            "cargo", "metadata", "--format-version", "1", "--offline", "--locked", "--manifest-path", str(manifest_path),
        ], cwd=ROOT, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
        metadata = json.loads(completed.stdout)
        resolved = {node["id"] for node in metadata["resolve"]["nodes"]}
        identities, members = [], []
        for package in metadata["packages"]:
            if package["id"] not in resolved:
                continue
            identity, record = package_record(package, locked)
            if identity in packages and packages[identity] != record:
                raise ValueError(f"package declarations disagree between projects: {identity}")
            packages[identity] = record
            identities.append(identity)
            if package["id"] in metadata["workspace_members"]:
                members.append(identity)
            if package["source"] is None:
                inputs.add(Path(package["manifest_path"]).resolve().relative_to(ROOT).as_posix())
        inputs.update([manifest, lock_path.relative_to(ROOT).as_posix()])
        projects[project] = {"manifest": manifest, "workspace_members": sorted(members), "resolved_packages": sorted(identities)}
    return {
        "format_version": 1,
        "scope": "locked_cargo_declarations_all_resolved_targets_including_build_and_dev",
        "limitations": [
            "These are publisher declarations, not a completed license/compatibility audit or a license choice.",
            "All resolved target/build/dev packages are listed; this is not a specific release binary's linked component list.",
            "Archive checksums are copied from the lockfiles, not independent archive authenticity verification.",
            "Bundled C/assembly/data, transitive notices, complete license texts, source offers and distribution obligations still require review.",
        ],
        "input_files": {path: digest(ROOT / path) for path in sorted(inputs)},
        "projects": projects,
        "packages": {identity: packages[identity] for identity in sorted(packages)},
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail if the recorded inventory differs; do not write")
    options = parser.parse_args()
    result = inventory()
    wire = json.dumps(result, indent=2, ensure_ascii=True) + "\n"
    if options.check:
        if OUTPUT.read_text(encoding="utf-8") != wire:
            raise ValueError("dependency inventory is stale; run python scripts/dependency_inventory.py")
    else:
        OUTPUT.write_text(wire, encoding="utf-8")
    counts = ", ".join(f"{project}: {len(value['resolved_packages'])}" for project, value in result["projects"].items())
    print(f"Locked dependency declarations match: {len(result['packages'])} unique packages; {counts}. License audit remains pending.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        raise SystemExit(f"Dependency inventory failed: {error}")
