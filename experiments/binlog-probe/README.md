# Binlog capability experiment

Latest verification is split between two completed gates (2026-10-27 JST).
`target/limited-index-native-check.log` passed 19 isolated tests, Clippy, 36
checks each on normal MariaDB/MySQL and dedicated GTID MySQL, cold restore,
pre-COMMIT kill, filtering, three limited changes/index candidates and overlap.
All three fixtures were reclaimed. `target/limited-index-and-license-check.log`
then passed 219 workspace tests, 77 Python tests, Clippy, Linux cross-check,
HTTP, observations for 208 dependency packages, and current hashes of DB,
binlog, overlap and WP evidence. WP44 / 286,998 cases remain unchanged; PHP was
not rerun. The dependency observation matched 203 locked archives and 364 cache
files; 203 checksum JSON files were missing, four packages had no selected
candidates, and five were local. These are observations, not a legal approval.

The 214-test full gate below is retained as history.

Previous completed full gate (2026-10-27 JST):
`target/limited-meta-changes-full-check.log` passed 214 workspace Rust tests,
19 isolated experiment tests and 68 Python tests; 36 checks each on normal
MariaDB/MySQL and dedicated GTID MySQL. Cold restore, pre-COMMIT client kill,
same-DB filtering, the typed zero-byte refusal and three limited commits,
overlap bootstrap, HTTP, Clippy, Linux cross-check and registry current-hash
verification passed. Both normal binlog records confirm
`postmeta_logical_limit_preserved=true` / `postmeta_limited_commits=3`.
All three fixtures were reclaimed. WP44 / 286,998 cases, 572 hook and 405
dispatcher scenarios are retained by current-hash verification; PHP was not
rerun. Final-candidate logical limits are not RSS, peak-memory or shared budgets.

Previous completed full gate (2026-10-27 JST):
`target/mysql-overlap-bootstrap-full-check.log` passed 194 workspace Rust tests,
19 isolated experiment tests and 68 Python tests; 36 checks each on normal
MariaDB and MySQL plus 36 on the dedicated GTID MySQL fixture; cold restore,
pre-COMMIT client-kill recovery, same-DB filtering, overlap bootstrap, HTTP,
Clippy, Linux cross-check and registry current-hash verification. The unchanged
WP proof remains 41 APIs / 274,683 cases, 572 hook and 405 dispatcher scenarios.
All three owned fixtures were stopped and reclaimed. The following earlier
counts and logs retain their historical meaning.

Previous completed full gate (record updated 2026-10-27 JST): `target/filtered-postmeta-and-cgroup-full-check.log`
passed 190 workspace Rust tests, 13 isolated experiment tests and 61 Python tests,
36 checks per database, cold restore, pre-COMMIT client-kill recovery, same-DB
filtered projection, HTTP, Clippy and Linux cross-check. The unchanged WP source
remains 39 APIs / 266,493 cases, 572 hook scenarios and 405 dispatcher scenarios;
PHP was last compared in the WP stage of the form-attributes gate, with later
runs checking source/evidence hashes rather than rerunning PHP. Owned fixtures
are stopped/reclaimed and db-runs is empty. Earlier failure logs and UUID-checked
cleanup remain recorded. No native Linux run or production CDC is claimed.

A separate MySQL-only overlap bootstrap passed its second dedicated live run:
`target/mysql-overlap-bootstrap-second-live.log`. It skipped 132 prefix groups,
replayed three bootstrap commits and one strict commit, and verified 12 snapshot /
final rows against independent SQL and binary-key indexes. The actual child PID
and one `overlap_snapshot_reader` disconnect reset were strictly validated; the
owned fixture was reclaimed. The subsequent full gate
`target/mysql-overlap-bootstrap-full-check.log` also passed, as recorded above.

Development-only, outside the production workspace and its lockfile. Uses pinned
mysql_async 0.37.1 / mysql_common 0.37.3 to inspect an owned, disposable local DB.
The dependencies offer MIT/Apache-2.0 alternatives; the MIT option is intended
for this GPL-2.0-or-later experiment. A complete transitive license audit is pending.

```powershell
cargo fetch --manifest-path experiments/binlog-probe/Cargo.toml
python scripts/check.py --with-reference --with-process-smoke --with-binlog-probes
```

The existing portable MariaDB/MySQL development tools are required. The harness
enables ROW/FULL logging only on its fresh instance, creates synthetic rows, then
stops and removes that instance. The MySQL probe uses its own CA and a loopback
SAN certificate generated with the existing Git for Windows OpenSSL. Certificate
and server-name checks remain enabled. No system services or existing DBs change.

The probe requests a finite nonblocking stream from the position preceding its
synthetic transactions. It verifies one INSERT, UPDATE and DELETE, their before/
after bytes (including NUL/non-UTF-8), three COMMITs, absence of the rolled-back
row, and the CRC32 checksums offered by the stream. It also flips one payload byte
in a locally serialized copy and requires a checksum-mismatch rejection before
row decoding. Both MySQL connections must report a nonempty TLS cipher. It fails on unrecognized event types.
Only counts and event-type IDs are emitted; no row payload or credentials.

This is not a continuous subscriber or trusted production decoder. General row
types, unsigned boundaries, GTID/rotation/restart, snapshot/log alignment, atomic
persistent application, index construction and cache/publication invalidation
remain unverified. It drives ChangeBuffer and a fixture-only atomic SQL journal, but is not connected
to the production data projection or public server.
The event-count boundary is a fixture guard, not a preallocation bound on the
underlying driver's packet reader. These results do not establish process memory
limits or the product's supported database/version/deployment matrix.

Evidence lives in compatibility/evidence/binlog-probe-{mysql,mariadb}.json and can
be checked without connecting via scripts/check_binlog_evidence.py.

The classifier validates frame continuity and exercises COMMIT staging, an
incomplete prefix and a missing frame. MariaDB interior zero offsets are derived
only until the next explicit XID boundary, which must match. The initial MariaDB
Rotate is reparsed with CRC context rather than trimming a filename suffix.

The fixture journal atomically stores opaque payloads and a source/file position.
A deliberately lost acknowledgement after SQL COMMIT must retry without adding
rows; different bytes at the same position must be rejected. Three committed
source batches leave exactly three persisted event rows and one accepted replay.
The source endpoint is fixed before journal initialization, avoiding feedback
from journal writes only in this bounded experiment. This is not a production
storage schema, a restartable normalized event format, or a continuous filter.

A separate finite window alters only the fixture's toy table. MySQL reaches the
actual ALTER query and latches SchemaChanged without applying a batch or moving
the checkpoint. MariaDB first emits a truncated compatibility notification
(`# Dum`); the positive classifier remains conservative and rejects that query.
Only the negative observer continues inspecting the failed stream to find the
actual ALTER and verify that it is also refused. Evidence distinguishes this
with ddl_prefix_rejected. This does not establish MariaDB standalone-GTID support,
DDL recovery, resnapshotting, index rebuilds or public/cache invalidation.

The main window now closes the source stream after its first durable COMMIT,
reopens the journal connection, reloads its epoch/file/position, and creates a new
decoder to consume only the remaining two COMMITs. Wrong expected epoch and file
are rejected. There are 19 main-window events including the Rotate/FDE pair for each of two stream
connections; the final journal remains three rows with one accepted identical replay.
This tests connection reopening within one live fixture, not server/process
restarts, persistent production source identity or continuous filtering.

A read-only request for a verified-absent log must return server error 1236/HY000.
Normal EOF or timeout is not accepted. The journal is checked again afterward.
The driver's generic is_fatal flag is recorded separately and is not interpreted
as binlog readiness. Output is one versioned JSON result line, validated strictly
by scripts/binlog_probe_result.py; this is not a production IPC contract.

A separate fixed-schema row window verifies five unsigned ID boundaries, adjacent
signed BIGINT values, nullable/empty/binary bytes and UTF-8 text. SQL column
metadata and TableMap types/nullability must agree. MariaDB exposed two high
unsigned values as negative Int variants; only that verified unsigned column is
reinterpreted as uint64 bits. MySQL required no such reinterpretations. This is a
codec-only, memory-acknowledged fixture window, not a generic production schema
cache or an additional durable journal batch.

## Finite WordPress postmeta projection

A separate window now reads the fixture's actual four-column `wp_postmeta` row events, following SQL schema and TableMap validation. It decodes five full-image mutations across three commits: two inserts, an update changing the primary key/owner/key/value, and two deletes. A rolled-back update is absent. NULL, NUL, Unicode, serialized bytes and unsigned ownership are retained. Rows and the optional exact-byte index match an independent SQL read at EACH commit, not just at the final endpoint. An old Arc view remains unchanged.

`postmeta_changes=5`, `postmeta_memory_commits=3`, `postmeta_projected_matches`, `postmeta_old_view_preserved` and `postmeta_apply_refusal_preserved` describe ONLY this extra memory projection window. They are separate from the original three durable opaque-event journal batches and the five-row type corpus. A parallel consumer refuses the first commit and verifies that its checkpoint and visible rows do not advance.

This is a serialized, bounded owned fixture, not a production snapshot/binlog anchor, persistent projection/index store, recovery receipt, general schema decoder, background index policy or public cache invalidation. `accept_projected` is explicitly memory-only; it is not a replacement for `accept_durable`. No production database, global lock, setting or privilege is changed.

## Limits on a prepared final metadata candidate

`PostMetaView::prepare_changes_with_limits` borrows `&[MetaChange]` without
changing the existing owned `prepare_changes` API. A `BTreeMap<u64, &PostMetaRow>`
first validates every ordered change, full before image, missing row, duplicate
identity and primary-key move. Strict change errors take precedence over limits.
Only after the final row count and checked logical charge fit does it clone the
final row bodies into an unindexed candidate. Failure changes neither old rows
nor their existing index; no partial candidate is returned.

Logical bytes cover only each final `PostMetaRow` structure and its non-NULL
key/value lengths. They exclude the old view, simultaneous old/new ownership,
supplied changes, reference map, indexes, Vec/allocator capacity, driver overhead,
RSS and peak memory. This is not shared-budget accounting or an automatic
capacity formula. Deletions can bring an old view within a smaller allowance;
an inserted-then-deleted row contributes no final rows or bytes. Five new pure
contracts cover exact/minus-one/zero/MAX allowances, raw bytes, NULL versus empty,
ID0/MAX, ordered PK moves/ABA, strict error precedence and old-index preservation.

The normal finite postmeta consumer now uses borrowed changes with fixture-only
64-row/64-KiB allowances for three commits and independently compares all raw
rows and indexed matches with SQL. A separate first-commit callback receives a
zero byte allowance and must downcast to exactly
`LimitedChangeError::ByteLimitExceeded { limit: 0 }`, preserving old rows, index
and checkpoint. An arbitrary callback error is insufficient. Strict Python
validation checks `postmeta_logical_limit_preserved=true` and
`postmeta_limited_commits=3`, including their metadata correlation. These fixture
constants are not manual product capacity settings.

Targeted contracts, DB all-targets Clippy and the integrated
`target/limited-meta-changes-full-check.log` passed. Both normal database runs
confirmed the typed zero-byte refusal and all three limited commits with raw-row
and index comparisons; strict evidence validation passed and all three owned
fixtures were reclaimed. Counts and the retained WP proof are recorded above.
Existing durability/recovery fixtures and production-CDC limitations remain distinct.

## Final logical index byte allowance

`build_key_index_with_byte_limit(max_entries, max_logical_bytes)` is separate
from the unchanged `build_key_index(max_entries)`. It first discards the old
index and checks the row allowance. Borrowed keys in a `BTreeSet` then preflight
this checked charge:

`row_count * size_of::<usize>() + sum_unique_non_NULL_keys(size_of::<(Vec<u8>, Vec<usize>)>() + key.len())`

Every row contributes an offset, including NULL rows. Repeated key bytes incur
one key-entry header/body charge; NULL and empty keys remain distinct. The
inline `KeyIndex` root already in the view is excluded. An empty view can build
with zero allowances. After the entire charge fits, the borrowed set is dropped
before reusing the existing owned index builder. Overflow or excess leaves
unchanged rows using Scan, with no partial index. Later rebuilds remain possible.

This is final logical index accounting only. It excludes preflight references,
tree-node overhead, Vec/allocator capacity, row bodies, peak allocations, RSS,
shared budgets and automatic capacity formulas. Five new pure contracts passed,
covering exact/minus-one/zero/MAX, NULL/empty, duplicate-key versus all-offset
charges, binary/case/ID0/MAX ordering, refusal/rebuild and empty-zero behavior.

Each of the three native postmeta candidates now requires Scan at zero bytes
and ExactKeyIndex at 64 KiB, comparing all raw rows and every tested key result
with independent SQL expectations in both states. Strict metadata validation
requires `postmeta_byte_limited_indexes=3`. The allowance is a fixture-internal
test value, not a manual product setting. The 19 isolated tests, Clippy, normal
MariaDB/MySQL and dedicated GTID fixture (36 checks each) passed in
`target/limited-index-native-check.log`, including all three index candidates
and the existing recovery/filter/overlap fixtures. All three fixtures were
reclaimed. The separate `target/limited-index-and-license-check.log` passed
workspace 219 / Python 77, HTTP, Linux cross-check and current-evidence hashes.
PHP was not rerun; WP44 / 286,998 remains the retained proof.

## Atomic typed projection fixture store

The postmeta window now ALSO saves its complete typed row set, source epoch/file, checkpoint and exact frame receipt in one InnoDB transaction in `rp_postmeta_projection`. The SQL table and bounded JSON are experiment-only formats; production storage remains unselected. Indexes are rebuilt from the saved rows.

The first projection COMMIT deliberately loses its acknowledgement. The buffer and old memory view stay at their prior boundary; retry matches the stored receipt and data without another update. Changed data at that same receipt is rejected as a typed ReplayConflict and leaves the durable pair unchanged. A new SQL connection with a distinct connection ID reloads the exact rows/position and rejects wrong expected epoch AND file with a typed SourceContext fault.

A separate reader sees the old committed pair while a toy-row update is uncommitted. ROLLBACK restores all stored bytes, receipt and positions. This checks fixture InnoDB behavior and explicit application transaction boundaries; it is not a device-loss, process crash or server-restart test. The original opaque journal's three batches, the five-row codec corpus and the postmeta projection window remain separate observations.

The typed projection window also closes and recreates its binlog connection (client 7007), discards decoder/table-map state, rebuilds its index from reloaded rows, and consumes the remaining two commits from the SQL checkpoint. `postmeta_source_resumed` records this SAME-RUN fixture reconnect, not process/server crash recovery or stable production identity.

Consumed SQL connections can report a Windows closed-socket error after COM_QUIT/TLS shutdown. Only their disconnect calls classify reset/broken-pipe/EOF as cleanup observations in `sql_disconnect_resets`. Connection establishment, source reads, SQL statements, commits, server errors and timeouts still fail. Reopened connections and receipts are always revalidated. Four additional MySQL fixtures passed; two recorded one reset each at `postmeta_journal_old` and `original_journal_old`. This behavior is not a source-health or durability guarantee. The independent experiment's cleanup classifier test runs with --with-binlog-probes.

## Fresh-process read-only restore

The harness waits for the producer to exit successfully and validates its actual spawned PID. It then starts a distinct `--restore-postmeta` process, using the same still-running owned database. The restore mode performs the existing ownership and TLS guards, branches before DDL, reads the saved row/position/receipt tuple, checks the expected epoch/file/position/row count and log file size, rebuilds its index, and compares every row to the quiescent source. Stored bytes must remain unchanged. Receipt shape validation does not revalidate checksums of complete source wire frames.

Five separate rejected children exercise wrong file, position and row count, missing fixture ownership flag and extra CLI arguments. A nonzero exit alone is insufficient: the expected reason and empty stdout are required. A final successful restore confirms that the saved data remains usable. `fresh_process_restore` records actual PIDs, producer exit ordering, refusals and final confirmation.

This subtest is cold restoration after a NORMAL producer exit. It does not test abrupt termination; the separate pre-COMMIT client-kill test below covers one narrow interruption point. Server restart, OS crash, storage failure, continuous resume and production source identity remain unverified. No production restore CLI or SQL/JSON storage format is adopted.

Result and evidence checks require exact JSON numeric types, bounds, required
metadata and unique keys; booleans and floats cannot stand in for integers.
The harness waits for the owned Popen process, checks its actual PID and normal
exit, and handles child termination/reaping on timeout or failure. A child's
reported success or PID alone is insufficient process-lifecycle evidence.

## Snapshot position observations: measured on pinned fixtures

The MariaDB 11.8.9-only module creates its dedicated InnoDB toy table, then pins
one reader connection to REPEATABLE READ and a READ ONLY consistent snapshot.
It reads `Binlog_snapshot_file` and `Binlog_snapshot_position` through SHOW
SESSION STATUS. Before the reader's first table SELECT, control commits a new
value. The test requires global binlog progress in the same file, the old reader
value and unchanged snapshot coordinates, error 1792 on a valid UPDATE, and the
same old value/anchor after that rejection. ROLLBACK ends the transaction, and
control must retain the new value. Missing/invalid status or an anchor following
global progress fails. MySQL reports no MariaDB anchor, rather than substituting
ordinary current status.

The MySQL 8.4.13-only toy-table observation instead checks that ordinary SHOW
BINARY LOG STATUS can include another connection's later commit while the
reader's snapshot still sees the old row. This tests one unsafe association; it
does not prove that all MySQL snapshot anchoring methods are impossible.

Both run only in Produce mode after the existing projection tests and ownership,
identity and TLS guards. They change neither WP tables nor saved projection.
Both observations passed on the pinned fixtures. MariaDB retained anchor 73651
while global status advanced to 73954. MySQL advanced from before 69375 to
reported 69735 while the snapshot retained the old row and control saw the new
row. Both read-only readers rejected an UPDATE with error 1792. Positions are
observed fixture values, not hardcoded expected positions. Evidence was
regenerated and checked in the full gate above. No production snapshot/CDC
protocol, database-version policy or privilege policy is adopted.
The earlier automatic approval rejection of a production FTWRL API, due to
server-wide write stoppage and the impact of failure, remains unresolved. That
API is not retried or invoked under a different name.

MariaDB's [status documentation](https://mariadb.com/docs/server/ha-and-performance/standard-replication/replication-and-binary-log-status-variables)
describes transaction-consistent coordinates. The [11.8.9 implementation](https://github.com/MariaDB/server/blob/mariadb-11.8.9/sql/log.cc#L12302-L12325)
prefers the current THD's saved position despite the documentation's Global scope
label, and falls back to current global state without that saved snapshot.
[Snapshot start](https://github.com/MariaDB/server/blob/mariadb-11.8.9/sql/handler.cc#L3046-L3058)
uses an internal commit-order mutex; this is not a claim of zero internal locks
or commit stalls. Binary logging, one connection, RR and snapshot-capable engines
are prerequisites. DDL/metadata locking, schema stability, log retention and
production recovery remain separate concerns.

## Pre-COMMIT client-process kill and finite recovery

Two fixture-only modes, `--prepare-postmeta-crash` and `--resume-postmeta-crash`,
use `rp_postmeta_crash_source` and `rp_postmeta_crash_projection`. The source is
an eleven-row clone, not a modification to wp_postmeta or the normal projection.
Cloning preserves ID0 by temporarily adding SESSION NO_AUTO_VALUE_ON_ZERO when
needed and restoring the original session mode, including an attempted restore
on failure. No global SQL mode is changed.

Prepare saves a committed baseline, then writes the next row/position/receipt
inside a transaction without committing it. It flushes a canonical READY line
and remains pending. The harness checks the actual Popen PID, nonce, epoch/file
and ordered positions before killing ONLY that still-running owned client.
It never kills the database or another existing process. The same owned DB
Popen must remain alive throughout.

READY is bounded to 64 KiB; old row data and receipt are each bounded to 12 KiB.
The receipt must contain exactly one nonempty frame. IDs/positions reject
bool-as-int, and duplicate JSON keys, wrong identity, malformed hex or row shape
are refused. A cancellable bounded thread/queue reads READY, and timeout or any
parse failure still kills/reaps the child and joins the reader. Resume receives
the validated marker over bounded stdin. Extra stdout or stderr is rejected;
Windows nonzero kill codes are not assumed to be Unix negative signal numbers.

Resume obtains the journal row's FOR UPDATE lock under short session/async
limits to drain rollback, then compares the entire old committed tuple to READY.
A plain consistent read alone would not prove rollback has finished. A new
finite decoder applies one COMMIT/one UPDATE from the saved baseline, checks all
11 rows and the exact-byte index against the dedicated source, and records the
new tuple. A final ordinary `--restore-postmeta` confirms that the original
projection still restores correctly. Evidence is stored in
`process_crash_recovery`, with actual PIDs, kill exit, nonce, marker, resumer and
normal-restore confirmation.

Both pinned engines passed. This covers a kill BEFORE SQL COMMIT, not ambiguous
outcomes during COMMIT, a server/OS crash, power loss, durable source identity,
continuous CDC, retention or public/cache invalidation. Production storage and
recovery protocols remain unselected. Existing monitor/capacity limitations and
non-Linux physical-memory Unsupported behavior are unchanged.

## Finite same-database postmeta filtering

`filtered_postmeta` recognizes only the fixture database and the dedicated
`rp_filter_target` / `rp_filter_other` tables. The target is an eleven-row clone
with the same SESSION NO_AUTO_VALUE_ON_ZERO preservation/restoration as the
client-kill probe. No WP source, normal projection, global setting or extra DB
grant is changed. Cross-database live verification is explicitly false.

Three finite commits contain three selected mutations and two ignored mutations:
a mixed commit, an ignored-only commit and a target-only commit. Every raw
RowsEvent, including ignored frames, is still staged and included in the receipt;
only MetaChange decoding/application is selected. An ignored-only commit saves
the new position and full receipt while retaining target rows. This does not
reduce retained-payload budgets or change core/adapter Advance contracts.

Every TableMap is schema-checked even without a subsequent row. Both selected
and ignored row images are checked. Each commit verifies all seven saved tuple
fields, including from-position and full receipt. The ignored-only commit tests
SQL rollback, lost acknowledgement after COMMIT and exact replay; wrong filter
profiles must be specifically refused without changing storage. Corrupted or
missing ignored frames still fail CRC/position checks. Actual ALTER on the
ignored table latches SchemaChanged and cannot be healed by replay. MariaDB
records ignored_ddl_prefix_rejected=true for its standalone compatibility prefix;
MySQL records false.

The 22-field result records commits=3, selected_changes=3, ignored_changes=2,
11 final rows and SQL/index agreement, with cross_database_live_verified=false.
The finite endpoint is captured before journal writes. This is not general
production filtering, cross-database live verification, durable source identity,
a production snapshot/log protocol, continuous CDC or publication invalidation.
The full gate also checks the independent caller-path-only cgroup v2 observation
candidate; it does not add capacity formulas or cache/CLI integration, and
non-Linux observation remains Unsupported.

## Independent unpublished MySQL overlap bootstrap

```powershell
python scripts/database_smoke.py --backend mysql --gtid-bootstrap-probe
python scripts/check_mysql_overlap_evidence.py
```

This exclusive flag starts a fresh MySQL 8.4.13 GTID-ON fixture. It does not turn
on GTIDs in the normal probes or alter an existing database. It requires AUTO
normal GTIDs from one origin, ROW/FULL, ordered commits, compression off, one
first log file, empty purged / Previous-GTIDs sets and stable InnoDB schema.
The clone retains all 11 original rows, including zero and uint64 MAX IDs, using
the session-only NO_AUTO_VALUE_ON_ZERO preservation/restoration pattern.

The experiment reads executed set A before a fresh read-only RR snapshot B.
T1 inserts ID100 before B; T2 moves its primary key between snapshot pages;
T3 deletes and reuses the identity. B retains the earlier 12 rows. From file1 /
position4, the private decoder checks every physical frame and skips only the
leading A-owned groups. After first-not-A, every transaction is replayed,
including any later A-owned one. Full after-image upserts and missing-tolerant
primary-key deletes operate only on an unpublished RAM map. At the complete C
XID boundary, all rows and binary-key searches match independent SQL. A further
T4 UPDATE is then processed by the unchanged strict adapter from C, with complete
before-image agreement and an exact next GTID / transaction-length fence.

The second live run observed A=1-132, floor=2149380, C=2150732 and strict end
2151162 in `probe-bin.000001`. These are historical observations, not fixed
expected positions. Evidence is `compatibility/evidence/mysql-overlap-bootstrap.json`.
The module's six unit tests and Clippy passed. Nonprefix A behavior is tested
through the same runtime ReplayFloor selector in pure tests; the live run had a
prefix A and did not manufacture a nonprefix set or GTID-publication delay.

FDE envelopes restore the checksum algorithm descriptor byte removed by the
pinned driver's decoder, in addition to the CRC32 suffix. Other frame envelopes
are unchanged. Physical continuity and verified checksums remain mandatory.
GTID group lengths are bounded and checked for overflow, with the strict seam
requiring the exact D endpoint. The initial FDE-size refusal remains recorded in
`target/mysql-overlap-bootstrap-first-live.log`; it was not bypassed by ignoring
the FDE or allowing a position gap.

This is not an official MySQL bootstrap procedure, an adopted production
protocol, a durable bootstrap journal, publication, continuous CDC or a server /
OS crash test. C denotes a logical binlog commit prefix, not an arbitrary SQL
visibility instant, client acknowledgement or durability guarantee. Rotation,
log loss / purge races, anonymous or tagged GTIDs, manual GTIDs, source replacement,
nonlogged changes and general schema support remain unverified or rejected by
the narrow profile. Prefix-skip handling does not relax normal unknown-event,
DDL or Advance classification. The rejected FTWRL product API remains unadded;
this experiment introduces no server-wide write / commit pause or extra grants.
See [the dedicated development record](../../docs/mysql-overlap-bootstrap-development.ja.md).
