"""Build a lexical candidate index from the pinned source, not a public API universe."""

import argparse
from collections import Counter
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import subprocess

from compare_primitives import canonical
import wordpress_reference as reference

TOOL_FILES = ["scripts/wp_inventory.php", "scripts/inventory_scopes.php", "scripts/inventory_wordpress.py", "scripts/test_inventory.py"]


def validate_summary():
    summary = json.loads((reference.ROOT / "compatibility/inventory-summary.json").read_text(encoding="utf-8"))
    source = reference.REFERENCE / f"wordpress-{reference.VERSION}"
    manifest_path = source / reference.MANIFEST_NAME
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if summary["format_version"] != 2 or summary["scope"] != "lexical_candidates_not_public_API_universe":
        raise ValueError("inventory summary format/scope differs")
    if summary["wordpress_version"] != reference.VERSION or summary["archive_sha256"] != manifest["archive_sha256"]:
        raise ValueError("inventory baseline differs")
    if summary["source_manifest_sha256"] != reference.digest_file(manifest_path):
        raise ValueError("inventory source manifest changed")
    if set(summary["tool_files"]) != set(TOOL_FILES):
        raise ValueError("inventory tool identities incomplete")
    for name, expected in summary["tool_files"].items():
        if reference.digest_file(reference.ROOT / name) != expected:
            raise ValueError(f"inventory is stale: {name}; regenerate scripts/inventory_wordpress.py")
    if sum(summary["namespace_counts"].values()) != sum(summary["declaration_counts"].values()):
        raise ValueError("inventory declaration/namespace counts differ")
    if sum(summary["method_visibility_counts"].values()) != summary["declaration_counts"].get("method", 0):
        raise ValueError("inventory method/visibility counts differ")
    artifact = reference.REFERENCE / "wordpress-api-candidates.json"
    if artifact.is_file():
        if reference.digest_file(artifact) != summary["artifact_sha256"]:
            raise ValueError("inventory raw artifact changed; regenerate inventory")
        candidates = json.loads(artifact.read_text(encoding="utf-8"))
        if dict(Counter(row["kind"] for row in candidates["declarations"])) != summary["declaration_counts"]:
            raise ValueError("inventory raw declaration counts differ")
        if dict(Counter(row["function"] for row in candidates["hooks"])) != summary["hook_call_counts"]:
            raise ValueError("inventory raw hook counts differ")
    print("Inventory summary matches source manifest and tools; lexical candidates only")


def main(php):
    reference.verify()
    source = reference.REFERENCE / f"wordpress-{reference.VERSION}"
    manifest = json.loads((source / reference.MANIFEST_NAME).read_text(encoding="utf-8"))
    files = sorted(row["path"] for row in manifest["files"] if row["path"].endswith(".php"))
    output = subprocess.run([str(php), "-n", str(reference.ROOT / "scripts" / "wp_inventory.php"), str(source)], input=canonical(files), stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=120)
    if output.stderr:
        raise ValueError("unexpected tokenizer diagnostics: " + output.stderr.decode(errors="replace"))
    candidates = json.loads(output.stdout)
    for record in [*candidates["declarations"], *candidates["hooks"]]:
        if record["source"] not in files or record["line"] < 1:
            raise ValueError("invalid inventory source location")
    artifact = reference.REFERENCE / "wordpress-api-candidates.json"
    artifact.write_text(json.dumps(candidates, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    summary = {
        "format_version": 2,
        "scope": "lexical_candidates_not_public_API_universe",
        "wordpress_version": reference.VERSION,
        "archive_sha256": manifest["archive_sha256"],
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "php_file_count": len(files),
        "declaration_counts": dict(Counter(row["kind"] for row in candidates["declarations"])),
        "method_visibility_counts": dict(Counter(row["visibility"] for row in candidates["declarations"] if row["kind"] == "method")),
        "namespace_counts": dict(Counter(row["namespace"] for row in candidates["declarations"])),
        "documented_internal_count": sum(row["documented_internal"] for row in candidates["declarations"]),
        "documented_deprecated_count": sum(row["documented_deprecated"] for row in candidates["declarations"]),
        "hook_call_counts": dict(Counter(row["function"] for row in candidates["hooks"])),
        "hook_first_argument_counts": dict(Counter(row["first_argument_kind"] for row in candidates["hooks"])),
        "artifact_sha256": reference.digest_file(artifact),
        "source_manifest_sha256": reference.digest_file(source / reference.MANIFEST_NAME),
        "tool_files": {name: reference.digest_file(reference.ROOT / name) for name in TOOL_FILES},
        "limitations": ["Lexical namespace, owner, method visibility and documentation flags do not establish public-theme reachability or runtime availability.", "Anonymous owners have synthetic source IDs, not PHP runtime class names; anonymous declarations, constants, globals, import resolution, inheritance and dependencies are not enumerated.", "Hook names remain literal source or dynamic expressions; unqualified calls in namespaces are unresolved candidates, not confirmed global WordPress hooks.", "This inventory does not define the complete API universe and is not a compatibility coverage denominator."]
    }
    (reference.ROOT / "compatibility" / "inventory-summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    print(f"Indexed {len(files)} PHP files: {len(candidates['declarations'])} declaration candidates, {len(candidates['hooks'])} hook-call candidates")
    print("Raw candidates: .reference/wordpress-api-candidates.json; summary: compatibility/inventory-summary.json")
    validate_summary()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--php", type=Path, default=reference.REFERENCE / "php-8.5.12" / "php.exe")
    parser.add_argument("--check", action="store_true", help="check existing summary identities without indexing source or executing PHP")
    options = parser.parse_args()
    try:
        if options.check:
            validate_summary()
        else:
            main(options.php.resolve())
    except subprocess.CalledProcessError as error:
        parser.exit(1, f"Tokenizer command failed: {error}\n{(error.stderr or b'').decode(errors='replace')}\n{(error.stdout or b'').decode(errors='replace')}\n")
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f"WordPress inventory failed: {error}\n")
