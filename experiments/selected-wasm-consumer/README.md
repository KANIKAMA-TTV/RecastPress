# Isolated fixed selected-page Wasm consumer experiment

This independent workspace is an experiment, not a production engine, SDK, ABI,
theme, HTTP route, authentication or deployment decision. No root manifest/lock, production source or
pre-existing experiment manifest/lock/source is modified. It directly uses current core and
Wasmtime36.0.19 with std/runtime/cranelift/wat/component-model only. There is no
WASI, WP/DB, host filesystem/network API, arbitrary module/path/input or HTTP.

`src/lib.rs` compiles a fixed Component, selects article/navigation with explicit
count/target-byte caps, passes two UTF8 strings into fresh Stores, then runs the
final `deliver_selected` callback only after output copy and successful
post_return. That final callback only moves the completed Vec; it never invokes
guest/compiler/imports or reenters the publication channel. Returning those bytes
does not prove later socket delivery or revoke already copied old bytes.

The one new host import is `fixture-checkpoint: () -> ()`. This is a private
fixture synchronizer, not a product read API. It signals an owned controller
thread and waits at most5s for acknowledgement. The separate thread publishes B
or withdraws after the guest has assembled A and before the guest returns. It
sends its completion before the fixture joins it. No Publisher operation is run
synchronously in the guest host callback. A missing acknowledgement is terminal;
unconfirmed thread completion is an error, never a successful cleanup claim.

Two strings use a monotonic arena1024..5120 (aggregate input≤4096), not the old
one-input allocator's constant address. Result bytes are in8192..12318. Fixed
UTF8 lift validates then checks the explicit output cap and fallibly copies;
every successful typed render call requires same-function post_return even on
UTF8/cap/copy refusal. post_return errors override tentative output/errors. The
guest replaces result bytes with X and resets its arena; peek is also followed
by post_return. Same-instance1→2 versus new-instance1 is a positive isolation
control. Fixed input/body/output/fuel/resource limits are developer fixture
settings, not product defaults or whole-process RSS/time/concurrency limits.

Static resource admission asserts one memory, zero tables and initial memory1
page. Store limits are set before instantiate: memory65536bytes, at most4
instances/1 memory/1 table/16 table elements, with explicit fuel. The instance
limit is an experimental allowance for the component/import adapters, not an
assertion that four instances are actually instantiated. Start/realloc/render
fuel and post_return fuel are distinguished; the deliberately insufficient
65535byte memory cap is identified by the pinned memory-limit cause. No Rust
guest Wasm target or Linux runtime observation is needed or claimed.

Seven native groups cover currentA, checkpointB/Stale callback0/freshB, checkpoint
withdraw/Stale callback0/freshC, selected-only body Weak ownership versus copied
HTML, caps/UTF8/two-input arena boundary, cleanup/fuel/ack failure priorities, and
same/fresh Store controls. The Weak observations here are body owners; private
core Weak metadata contracts remain the stronger separate page-metadata proof.
Compile/warm happens before selection in the fixed executable observation.

The no-argument binary runs only a finite currentA observation and emits small
fixed metadata, not the full seven-test result. `unpublished_only` means no
external HTTP/deployment occurs; the fixture does use an in-process Publisher.
An independent binary observation and native seven-contract result are distinct.
Historical target-only observations below are not adopted-source observations;
new results must identify the new manifest/source path and actual artifact.

Use only the dedicated target directory:

```
cargo test --manifest-path experiments/selected-wasm-consumer/Cargo.toml --target-dir target/selected-wasm-consumer-build --offline --locked -j 2
cargo clippy --manifest-path experiments/selected-wasm-consumer/Cargo.toml --target-dir target/selected-wasm-consumer-build --all-targets --offline --locked -j 2 -- -D warnings
```

## Historical target-only prototype (not adopted-source verification)

The following files remain under `target/selected-wasm-consumer-draft/`; none
was copied into this experiment as current evidence.

`native-first.log` retains the initial passing build with two warnings, including
an unused test cap. The corrected test actually applies cap0 before checking
post-return-fuel priority; `native-final.log` is the later verification. No
performance result is inferred from initial build/cache timings.

Final native7 groups passed, fmt check passed and all-target Clippy exited0 under
`-D warnings`. Its final log retains an environmental incremental-cache
hard-link warning (copy fallback); this was not suppressed by changing source or
cache policy. `audit.json` confirms all118 used registry version/source/checksum
tuples were already in the request probe's146-package registry set (28 unused
tuples removed, zero additions/changes). It also brackets27 old source identities
around the final verification follow-up and records final target source hashes.
These are host-native finite tests, not an independent executable-PID observation
or any HTTP/DB/PHP/Linux test. All successful mutation scenarios observed worker
completion before joining; an unconfirmed completion remains an explicit error.

## Later independent current-A executable observation

`observe_main.py` performed one fresh offline/locked CargoJSON build in the same
dedicated target, selected exactly one matching manifest/bin/source/kind artifact
and required one successful build-finished record. It ran that exact binary with
no arguments as an owned20s child: actual PID23132 matched the four-field prefix
JSON, format1/body51/unpublished-only true, empty stderr and exit0. A separate
owned child PID6912 rejected one unexpected dummy argument with exit2, empty
stdout and the exact fixed diagnostic (no echo). Build PID37452 and both child
PIDs were absent after confirmed owned reap. No temporary directories were used.

`main-observation.json` is the new observation, with17 source identities and
binary SHA unchanged before/after; `main-build-stream.jsonl`,
`main-build-stderr.log` and `main-observation.log` retain the build/result context.
It observes **currentA only**, not the native7 test suite or B/withdraw scenarios
through the main executable. The earlier `audit.json`'s not-yet-executed status
is a historical pre-observation fact and was not restamped. No HTTP/DB/Linux or
production engine/SDK adoption is implied.

## Adopted-source verification

The adopted experiment path passed native7 groups (test result0.51s), fmt and
all-target Clippy offline/locked in the dedicated cache. Phase logs are
`target/selected-wasm-adopted-native.log`, `-fmt.log` and `-clippy.log`. Compiler
incremental-cache hard-link/copy-fallback warnings are retained; there was no
source lint failure or provider/toolchain change to bypass them.

The fresh CargoJSON selection used exactly this new manifest/source/bin/kind,
then ran actual no-argument PID36952: strict four-field summary, format1/body51,
unpublished-only true, stderr empty and exit0. Unexpected argument PID38160
returned exit2 with fixed non-echo diagnostic/empty stdout. Build PID37004 and
both children were confirmed reaped and subsequently absent. Source17 and binary
identities were unchanged before/after that finite observation. Current evidence
is `target/selected-wasm-adopted-main-observation.json`, build stream/stderr and
`target/selected-wasm-adopted-main.log`; it observes currentA only, not executable
B/withdraw scenarios or a second seven-test run. Old prototype artifact SHA is
neither substituted nor required to match the adopted source path's artifact.

`target/selected-wasm-adopted-audit.json` records the adopted source hashes and
unchanged registry tuple identity. Root manifests/locks, production crates and
all pre-existing experiments are outside this experiment's ownership and their
selected source identity comparison is recorded separately. No normal runtime
server dependency, production engine/SDK/ABI adoption, HTTP/DB/Linux or auth
policy is introduced by this isolated experiment.
