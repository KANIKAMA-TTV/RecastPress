# 明示on-demand memory診断の開発CLI

全gateは`target/empty-filter-escaping-and-memory-diagnostic-workspace-check.log`で成功。新pure builder4・実CLI2を含むworkspace419 Rust／Python155、HTTP・Clippy・Linux cross-check・license208・DB current hashを確認し、独立レビューも通過した。`target/memory-diagnostic-windows-observation.json`では実CLIが全3section Unsupported・data無し・exit3となり、所有processを回収した。native Linuxの観測成功ではない。WPは別変更を実測した63 API／436,656比較、hooks572／dispatcher405。全native current hashも通過、DB live再実行無しで直前29件・6 fixture各39項目を維持する。gate後にユーザー指示で停止した。

2026-10-27。`recastpress inspect-memory` は既存の3観測APIを各1回、独立に呼び、固定キーの小さいJSONを1行返す。listener、router、shutdown handler、DB、PHP、fetch、publicationは起動しない。`check-bundle`と既存serve経路は維持し、CLI helpへ1行追加する。これを完全自動容量の算式や監視の完成とは扱わない。

## APIと独立section

新 `memory_diagnostic::inspect_memory()` は型付き・field非公開の `MemoryDiagnosticReport` または固定 `MemoryDiagnosticError` を返す。reportは `is_partial()` と `to_json()` を持つ。`format_version=1` の実験reportであり、新しい製品設定・計測APIの完全性・容量policyではない。

- `host_physical`: 既存 `observe_physical_memory` のtotal／available bytes。sourceは固定 `host_meminfo`。
- `self_rss`: 既存 `observe_self_smaps_rollup_rss` のresident bytes。sourceは固定 `self_smaps_rollup`。
- `visible_cgroup_v2`: 既存 `observe_self_cgroup_v2` の可視階層。sourceは固定 `self_visible_cgroup_v2`。

`self` はこのone-shot CLI process自身であり、別に稼働中の公開／管理server processのRSSを測定するものではない。PID指定や他process探索も行わない。

caller指定pathの単level観測をCLIから別途呼ばない。上位祖先やnamespaceを推測せず、任意path・容量値・sampling intervalなどの引数を受け付けない。一つのAPI失敗で残りを中止せず、3結果を独立したsectionへ変換する。

各sectionは `status=observed` とdata、または `unsupported`／`unavailable` と固定kindだけを持つ。kindはunsupported／not_found／permission_denied／invalid_data／interrupted／timed_out／other。元のIO error表示、raw OS message、membership、mountpoint、directory、任意label、path/hashはJSONにも診断にも含めない。未取得・未知・未対応は0 countersへ置き換えない。正当な観測値0はobserved dataとして維持する。

## cgroupの可視性

levelはleafからのordinalだけを持ち、raw directoryは出さない。`current_bytes` とmax／highをそのまま表す。max／highは `{state: "bytes", bytes: N}` と `{state: "max"}` を区別し、未知の祖先や未取得をnull／maxに統合しない。currentがmax／highを超える値もそのまま維持し、差し引き・sum・min・headroom・allowanceを計算しない。

可視mount境界でmemory filesを取得できない既存 `BoundaryUnobserved` は、ordinalと `state=boundary_unobserved` だけにし、架空の数値を付けない。upper_ancestorsは常にunknown、cgroupと全体のconsistencyはnon_atomicを保持する。全ての可視levelがmaxでも、不可視の上位制約がないことを意味しない。[既存可視cgroup候補](cgroup-memory-development.ja.md)

## 終了状態と上限

引数はちょうど `inspect-memory` だけである。`args_os` の先行分岐で処理し、余分な引数は観測前に拒否する。正常reportはstderr空でJSON＋LF、LFを含め64KiB以内。levelは1..128に制限し、過大／空の階層や過大reportをtruncateしない。

- exit0: 要求した3APIが成功し、可視levelにBoundaryUnobservedがなかったという意味だけ。常在するupperUnknownやnonAtomicは維持する。全memoryの可視性・容量安全・配分可能量の意味ではない。
- exit3: sectionにerror／Unsupportedがある、または可視BoundaryUnobservedがある部分report。取得できた独立sectionも返す。
- exit2: 不正引数・serialization・階層／report上限失敗。stdout空、stderrは固定 `memory inspection failed` とLFのみ。

Windowsでは現在3APIともUnsupportedであるため、各sectionにunsupportedを明示した部分reportとexit3を返す。0 bytesや測定成功を返さない。Linuxでは既存proc/cgroup IOを呼ぶが、native Linux実測は現在未検証である。

入力／report byte上限は実行時間の上限ではない。open/readやpage-table walkの時間、全一時割当・allocator overhead・RSSの上限は保証しない。3観測は時点・対象が異なる。host MemAvailableはkernel推定、RSSはshared mappingを含みhugetlbを別計上し、cgroupは配下全体を含む。[process RSS候補](process-memory-development.ja.md)、[Kernel proc資料](https://docs.kernel.org/filesystems/proc.html)、[cgroup v2 memory](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory-interface-files)

## 対象検証

pure builderの4契約は0と未取得の区別、u64最大値、current>max、max／unknown／BoundaryUnobserved、leaf ordinal、raw pathとsecret error textの除去、他sectionの維持、128level境界、exact JSON＋LF capと1byte不足を確認する。fixtureから報告を作る試験であり、native procfsの実測ではない。

新Python実CLI2契約はCargo JSONが返す現在のbinaryを使い、所有childを有限timeoutで回収する。Windowsの3section Unsupported／exit3／data無しと、引数strict／manual knob拒否／固定診断／help／既存check-bundle分離を検査する。Linux native samplingの契約試験へ読み替えない。既存check-bundle CLI4契約も再実行する。

server全targets Clippy、Linux全targets crosscheck、対象fmtを検証範囲とし、全体gateの結果は先取りしない。既存4観測module、monitor、BudgetPool、cache、CLI容量設定、自動容量式、cross-process調整、Windows wrapper、WP/core/DB/Cargo.lockは変更しない。
