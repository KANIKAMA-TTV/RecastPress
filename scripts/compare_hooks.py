"""Compare the native Hook subset with actual WP_Hook mutation/nesting scenarios."""

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
import random
from pathlib import Path
import subprocess

from compare_primitives import canonical, compare, integer, string
import wordpress_reference as reference
import compatibility_registry as registry_tool


def cases():
    result = []
    definitions = {name: {} for name in ["a", "b", "c", "d"]}
    call = {"kind": "filter", "value": "start", "extra": ["extra1", "extra2"]}

    def add(name, registrations, callbacks=None, calls=None):
        result.append({"id": name, "callbacks": callbacks or definitions, "initial": [{"id": ident, "priority": priority, "accepted": accepted} for ident, priority, accepted in registrations], "calls": calls or [call]})

    add("empty", [])
    add("ordered", [("a", 10, 1), ("b", 0, 1), ("c", 10, 1), ("d", -10, 1)])
    add("replace_in_place", [("a", 10, 0), ("b", 10, 1), ("a", 10, 1)])
    add("same_callback_different_priority", [("a", 10, 1), ("a", 20, 1)])
    for accepted in [0, 1, 2, 3, 99, -1, -2, -99, -(2**63), 2**63 - 1]:
        add(f"accepted_{accepted}", [("a", 10, accepted)])
    add("extreme_priorities", [("a", -(2**63), 1), ("b", 2**63 - 1, 1)])
    add("action_original_args", [("a", 10, 1), ("b", 20, 3)], calls=[{"kind": "action", "args": ["original", "extra"]}, call])
    add("action_empty_args", [("a", 10, 99), ("b", 20, 0)], calls=[{"kind": "action", "args": []}])
    mutations = [
        ("add_current", [{"kind": "add", "id": "d", "priority": 10}]),
        ("add_future", [{"kind": "add", "id": "d", "priority": 15}]),
        ("add_past", [{"kind": "add", "id": "d", "priority": 5}]),
        ("remove_current_peer", [{"kind": "remove", "id": "b", "priority": 10}]),
        ("remove_future", [{"kind": "remove", "id": "c", "priority": 20}]),
        ("clear_current", [{"kind": "clear", "priority": 10}]),
        ("clear_all", [{"kind": "clear"}]),
        ("clear_readd_current", [{"kind": "clear"}, {"kind": "add", "id": "d", "priority": 10}]),
        ("clear_readd_future", [{"kind": "clear"}, {"kind": "add", "id": "d", "priority": 30}]),
        ("clear_readd_past", [{"kind": "clear"}, {"kind": "add", "id": "d", "priority": 5}]),
        ("replace_current_peer", [{"kind": "add", "id": "b", "priority": 10, "accepted": 0}]),
        ("replace_future", [{"kind": "add", "id": "c", "priority": 20, "accepted": 0}]),
        ("nested_filter", [{"kind": "filter", "value": "nested", "extra": ["nested_extra"]}]),
        ("nested_action", [{"kind": "action", "args": ["nested_action"]}]),
    ]
    for name, changes in mutations:
        callbacks = {**definitions, "a": {"mutations": changes}}
        add(name, [("a", 10, 1), ("b", 10, 1), ("c", 20, 1)], callbacks=callbacks, calls=[call, call])
    add("filter_inside_action", [("a", 10, 1), ("b", 20, 1)], callbacks={**definitions, "a": {"mutations": [{"kind": "filter", "value": "nested"}]}}, calls=[{"kind": "action", "args": ["action"]}, call])
    generator = random.Random(71304)
    priorities = [-5, 0, 5, 10, 20]
    for index in range(250):
        initial = [(generator.choice(["a", "b", "c", "d"]), generator.choice(priorities), generator.choice([-2, -1, 0, 1, 3])) for _ in range(generator.randrange(1, 8))]
        changes = []
        for _ in range(generator.randrange(1, 5)):
            kind = generator.choice(["add", "remove", "clear", "filter", "action"])
            if kind in ("add", "remove"):
                changes.append({"kind": kind, "id": generator.choice(["a", "b", "c", "d"]), "priority": generator.choice(priorities), "accepted": generator.choice([-1, 0, 1, 3])})
            elif kind == "clear":
                changes.append({"kind": kind, "priority": generator.choice([None, *priorities])})
            elif kind == "filter":
                changes.append({"kind": kind, "value": "generated_nested", "extra": ["nested_extra"]})
            else:
                changes.append({"kind": kind, "args": ["generated_action", "extra"]})
        add(f"generated_{index:03d}", initial, callbacks={**definitions, "a": {"mutations": changes}}, calls=[generator.choice([call, {"kind": "action", "args": ["top_action", "extra"]}]), call])
    typed_arrays = [
        {"type":"array","entries":[]},
        {"type":"array","entries":[{"key":string(b"01"),"value":{"type":"null"}},{"key":integer(-2),"value":string(b"\0\xff")} ]},
        {"type":"array","entries":[{"key":string(b"1"),"value":integer(-(2**63))},{"key":integer(1),"value":{"type":"bool","value":False}},{"key":string(b"nested"),"value":{"type":"array","entries":[]}}]},
    ]
    pool=[{"type":"null"},{"type":"bool","value":False},{"type":"bool","value":True}]
    pool += [integer(value) for value in [-(2**63),-1,0,1,2**63-1]]
    pool += [{"type":"float","value":value} for value in ["-0","0.5","nan","inf","-inf"]]
    pool += [string(value) for value in [b"",b"0",b"\0\xff","日本語🦀".encode()]] + typed_arrays

    def typed(name, registrations, callbacks, calls):
        add("typed_"+name,registrations,callbacks,calls)
        result[-1]["typed"]=True

    for index,value in enumerate(pool):
        following=pool[(index+1)%len(pool)]
        calls=[{"kind":"filter","value":value,"extra":[following,{"type":"null"}]},{"kind":"action","args":[value,following]},{"kind":"filter","value":value}]
        typed(f"empty_{index}",[],{"a":{}},[calls[0]])
        for accepted in [0,1,2,-1,-99,-(2**63),2**63-1]:
            typed(f"chain_{index}_{accepted}",[("a",0,accepted),("b",10,1),("c",20,99)],
                  {"a":{"operation":"constant","return_value":following},"b":{},"c":{}},calls)
        typed(f"argument_{index}",[("a",0,-1),("b",10,2),("c",20,1)],
              {"a":{},"b":{"operation":"arg","index":1},"c":{}},calls)
        typed(f"empty_action_{index}",[("a",0,0),("b",10,99)],{"a":{"operation":"constant","return_value":value},"b":{}},[{"kind":"action","args":[]}])
    for index,value in enumerate(typed_arrays):
        for selector,key in enumerate([integer(0),integer(-1),string(b"01"),string(b"1"),string(b"\0\xff")]):
            for nested in [False,True]:
                mutations=[{"kind":"filter","value":value,"extra":[{"type":"null"}]},{"kind":"action","args":[value]}] if nested else []
                typed(f"array_copy_{index}_{selector}_{nested}",[("a",10,1),("b",20,2)],
                      {"a":{"operation":"array_put","key":key,"entry_value":pool[selector],"mutations":mutations},"b":{}},
                      [{"kind":"filter","value":value,"extra":[string(b"\xff")]},{"kind":"action","args":[value]}])
    for index in range(60):
        value=generator.choice(pool)
        changes=[{"kind":"filter","value":generator.choice(pool),"extra":[generator.choice(pool)]},{"kind":"action","args":[generator.choice(pool)]}]
        changes.insert(0,generator.choice([{"kind":"add","id":"d","priority":15},{"kind":"remove","id":"b","priority":10},{"kind":"clear","priority":10}]))
        typed(f"mutation_{index}",[("a",0,1),("b",10,generator.choice([-1,0,1,3])),("c",20,1)],
              {"a":{"mutations":changes},"b":{"operation":"constant","return_value":generator.choice(pool)},"c":{"operation":"arg","index":1},"d":{}},
              [{"kind":"filter","value":value,"extra":[generator.choice(pool),generator.choice(pool)]},{"kind":"action","args":[value]}])
    return result


def main(php):
    source_root = reference.REFERENCE / f"wordpress-{reference.VERSION}"
    manifest = json.loads((source_root / reference.MANIFEST_NAME).read_text(encoding="utf-8"))
    archive = reference.REFERENCE / f"wordpress-{reference.VERSION}.tar.gz"
    if reference.digest_file(archive, "sha1") != reference.EXPECTED_SHA1:
        raise ValueError("WordPress reference archive changed")
    originals = {row["path"]: row["sha256"] for row in manifest["files"]}
    paths = ["wp-includes/plugin.php", "wp-includes/class-wp-hook.php", "wp-includes/class-wp-filter-sentinel.php"]
    source_hashes = {}
    for path in paths:
        digest = reference.digest_file(source_root / path)
        if digest != originals[path]:
            raise ValueError(f"WordPress hook source changed: {path}")
        source_hashes[path] = digest
    subprocess.run(["cargo", "build", "-p", "recastpress-wp", "--example", "hook_probe", "--offline", "--locked"], cwd=reference.ROOT, check=True, timeout=120)
    probe = reference.ROOT / "target" / "debug" / "examples" / ("hook_probe.exe" if os.name == "nt" else "hook_probe")
    inputs = cases()
    wire = canonical(inputs)
    rust = subprocess.run([str(probe)], input=wire, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=30)
    php_run = subprocess.run([str(php), "-n", str(reference.ROOT / "scripts" / "wp_hook_oracle.php"), str(source_root)], input=wire, stdout=subprocess.PIPE, stderr=subprocess.PIPE, check=True, timeout=30)
    if rust.stderr or php_run.stderr:
        raise ValueError("unexpected diagnostics in hook comparison")
    actual = json.loads(rust.stdout)
    expected = json.loads(php_run.stdout)
    failures = compare(inputs, actual, expected["results"])
    output = reference.ROOT / "compatibility" / "evidence"
    output.mkdir(exist_ok=True)
    if failures:
        (output / "hooks-failures.json").write_text(json.dumps(failures, indent=2) + "\n", encoding="utf-8")
        raise ValueError(f"{len(failures)} hook mismatches: compatibility/evidence/hooks-failures.json")
    (output / "hooks-failures.json").unlink(missing_ok=True)
    tracked = sorted(registry_tool.evidence_files(registry_tool.HOOK_SCOPE)["tracked_files"])
    evidence = {"format_version": 1, "scope": "request_local_WP_Hook_subset", "generated_at_utc": datetime.now(timezone.utc).isoformat(), "wordpress_version": reference.VERSION, "archive_sha256": reference.digest_file(archive), "php_version": expected["php_version"], "php_binary_sha256": reference.digest_file(php), "case_count": len(inputs),"typed_value_case_count":sum(bool(case.get("typed")) for case in inputs),"legacy_string_case_count":sum(not case.get("typed") for case in inputs), "mismatches": 0, "input_sha256": hashlib.sha256(wire).hexdigest(), "rust_results_sha256": hashlib.sha256(canonical(actual)).hexdigest(), "wordpress_results_sha256": hashlib.sha256(canonical(expected["results"])).hexdigest(), "source_files": source_hashes, "tracked_files": {path: reference.digest_file(reference.ROOT / path) for path in tracked}, "limitations": ["Native modeled scalar/byte-string/array fixture callbacks; Wasm transport, PHP object identity and references remain unimplemented.", "This evidence targets direct WP_Hook only; named dispatch/all/counters are tracked in their separate evidence.", "Resource-limit and callback-failure policies are Rust host behavior, not PHP equivalence claims."]}
    (output / "hooks.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(f"Matched {len(inputs)} hook scenarios against WP_Hook {reference.VERSION}, PHP {expected['php_version']}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--php", type=Path, default=reference.REFERENCE / "php-8.5.12" / "php.exe")
    options = parser.parse_args()
    try:
        main(options.php.resolve())
    except subprocess.CalledProcessError as error:
        parser.exit(1, f"Hook command failed: {error}\n{(error.stderr or b'').decode(errors='replace')}\n{(error.stdout or b'').decode(errors='replace')}\n")
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f"Hook comparison failed: {error}\n")
