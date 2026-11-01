# Process RSS の限定観測候補

履歴（2026-10-27）：`target/process-rss-observation-workspace-check.log`でRust393件・Python133件、Clippy・Linux all-target cross-check・license208観測・実HTTP・当時のDB current hashが成功した。WP／全nativeの当時のhashも`target/process-rss-*.log`で別途照合し、PHP／DB live／独立experimentは再実行していない。直前の60 API／401,980比較・hooks572／dispatcher405、独立29件と6 fixture各39項目を保持した記録である。新規4契約はnative Linuxでのproc実読の成功を示さない。最新の認証cookie検証MAIN変更後の全体再検証・source-bound証跡更新は未完了であり、この履歴を現在treeの検証結果へ読み替えない。

2026-10-27。`process_memory_observation::observe_self_smaps_rollup_rss()` は、呼出し時だけ固定 `/proc/self/smaps_rollup` を読んで `ProcessRssObservation { resident_bytes: u64 }` を返す。標準ライブラリのみを使い、unsafe、新依存、外部コマンドは追加しない。

既存の `memory_observation` はhost MemTotal/MemAvailable、`cgroup_memory_observation` と `cgroup_membership_observation` はcgroupの値・可視階層を扱う。このAPIは自身のprocessに対する独立した観測であり、それらを変更したり合算したりしない。

Linux公式資料では、`status`／`statm` のRSSは非同期集計で精度に制約がある。`smaps_rollup` はmapping全体の集計を提供するため、今回はこの読取り経路に固定する。`statm` のページサイズを4KiBと仮定した換算は行わない。[Kernel proc資料](https://docs.kernel.org/filesystems/proc.html)

入力の受理上限は64KiBで、超過検出のsentinelを含め最大65,537バイトだけ読む。唯一必須のfieldは `Rss: <符号なし十進整数> kB` であり、`kB` を1024バイトとしてchecked換算する。0は正当な値として返す。重複・欠落・不正整数・不正単位・余分な値・整数overflow・換算overflow・過大入力は `InvalidData`。header、アドレス表示、Pssや他fieldの形式を、このAPIの追加制約にはしない。

ファイル不在・権限拒否・読取り失敗は元のIO errorを返し、0や `status`／`statm` へfallbackしない。Windowsを含む非Linuxは `Unsupported`。通常のprocfsが自身へ公開する固定pathを読む前提であり、procfs mountを検証する機能や他processの調査機能ではない。

これは読取り量の上限であり、open/readの時間上限ではない。page-table walk中にも状態は変化し得るため、完全に原子的な瞬間値を保証しない。RSSはshared mappingを含み、hugetlb使用量は別fieldであり、このRss値へ含めない。PSS・cgroup charge・logical pool charge・総物理RAM・追加確保可能量とは同一視しない。[Kernel RSS/PSSとhugetlbの説明](https://docs.kernel.org/filesystems/proc.html)

純粋parserとbounded readerの3 unitは、0・正常換算・最大換算可能値・無関係field保持、不正／重複／欠落／overflow、64KiB境界とsentinelだけの読取り・IO error伝播を検査する。非Linux向けintegration契約1件は、未対応を0として成功させないことを確認する。Windowsでの対象実行とLinux向けcrosscheckを検証範囲とし、native Linuxでのprocfs実読は未実行である。

後続の[診断CLI](memory-diagnostic-development.ja.md) `recastpress inspect-memory` は、このAPIとhost／可視cgroupのAPIを各1回呼ぶ。自身のRSSはone-shot CLI自身の値であり、稼働中の公開serverの値ではない。3観測は逐次・独立で、同一瞬間の全memory可視性や合算可能性を保証しない。Windowsの固定観測では全3sectionがUnsupported・exit3だったが、native Linuxのprocfs実読成功は未確認である。

監視loop、cache／poolへの自動接続、容量採用・予算算式、Windows RSS wrapper、PSSやhugetlbの追加集計は未実装である。観測値をR52の完全自動容量管理へ接続する方針詳細も未確定で、開発fixtureの論理capを製品容量設定とは扱わない。
