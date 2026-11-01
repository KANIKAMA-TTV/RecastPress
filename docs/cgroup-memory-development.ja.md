# Linuxメモリ観測候補の開発記録

更新日：2026-10-27（日本時間）。読取り専用の資源観測を段階的に追加した記録。実メモリ監視・容量の自動設定・CLI cache接続の完成を意味しない。

## 3つの観測範囲

| module／API | 取得範囲 | 含まない保証 |
| --- | --- | --- |
| memory_observation | /proc/meminfoのMemTotal／MemAvailable | process・container専用の余裕や予約量 |
| cgroup_memory_observation::observe_cgroup_v2_level | caller指定directoryのmemory.current／max／high | 自身の所属、mount対応、祖先、cgroupfsの真正性 |
| cgroup_membership_observation::observe_self_cgroup_v2 | 自身のmembershipと限定mount対応、leafから可視境界まで | namespace外祖先の完全性、atomic snapshot、実効容量、競合の排除 |

host観測はMemFreeを代替にせず、MemAvailableを推定値として扱う。cgroup観測をhost情報から推定して埋めない。[公式proc仕様](https://docs.kernel.org/filesystems/proc.html)、[meminfoの公式実装](https://github.com/torvalds/linux/blob/master/fs/proc/meminfo.c)。

1 level候補は各memory fileを128bytesまで読み、上限超過の検出に1byteだけ追加取得する。ASCII decimal u64とoptional final LFを受け、limitだけexact `max`をNoneにする。0はSome(0)で、missing／不正値／overflowはエラー。3回のreadは非atomicであり、current／max／highの大小関係を強制しない。currentは子孫込み、highはthrottle／reclaimの境界、maxは一時超過し得る制限である。[公式memory interface](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)。

## 自身の可視祖先を解決する限定候補

新APIはVisibleCgroupV2Observationを返す。membershipは/proc/self/cgroupのraw namespace-relative path、mountpointはmountinfoからdecodeしたraw bytesを保持する。levelsはleaf→可視mount境界の順で、各directoryにObserved(CgroupV2LevelObservation)またはBoundaryUnobservedを付ける。upper_ancestorsは常にUnknown、consistencyは常にNonAtomicとする。effective limit・headroom・allocation allowanceを返すfieldは設けない。

初版は単一の`0::PATH`と、uniqueなcgroup2 mount／root `/`だけを対応する。非 `/` root、複数cgroup2 mount、missing membership／mount、曖昧な構造は拒否する。mountpointより上をfilesystemのparentとして歩かない。root `/`の対応でも、そのrootがホスト全階層の根であるとは扱わない。cgroup namespaceはmembershipとmountの表示を仮想化する。[公式namespace仕様](https://docs.kernel.org/admin-guide/cgroup-v2.html#namespace)。

bounded入力はmembership 64KiB、mountinfo 1MiB、path 4,096bytes、membershipの深さ128、mountinfo 4,096 records／1 record 16KiBまで。proc各入力も上限＋sentinelの読取りとする。mountinfoの可変optional fieldはseparatorまで解析し、未知tagを無視する。rootとmountpointは別fieldであり、path対応を文字列prefixで済ませない。[公式mountinfo仕様](https://docs.kernel.org/filesystems/proc.html#proc-pid-mountinfo-information-about-mounts)。

mountinfo pathのescapeは`\040`／`\011`／`\012`／`\134`だけをdecodeし、未知・不完全escapeを拒否する。その後のclean path制約でcontrol byteとbackslashを拒否するため、TAB／LF／backslashを含むpathは初版未対応。spaceは対応する。membershipにはこのescape decodeを適用しない。両pathとも絶対path、空でないcomponent、`.`／`..`なしを要求し、NUL・control byte・backslashや重複／末尾separatorを拒否する。byte pathをlossy UTF-8へ変換しない。[公式mountinfo path出力](https://github.com/torvalds/linux/blob/master/fs/proc_namespace.c)、[公式cgroup membership／namespace-relative root出力](https://github.com/torvalds/linux/blob/master/kernel/cgroup/cgroup.c)。

選択mountpoint配下の別mountは一律拒否する。leafのdirectoryだけでなく、memory.max等の個別fileへのbind mountもこの検査に含める。component境界で比較し、例えば `/cgroup` と `/cgroup-other` を同じsubtreeにしない。memory値を取得できるlevelはcgroup.typeがexact domainであることも要求し、threaded／domain threaded等は未対応とする。[公式thread/domain仕様](https://docs.kernel.org/admin-guide/cgroup-v2.html#threads)。

## 欠落・無制限token・可視境界

可視境界のmemory.current／max／highが**全部NotFound**の場合だけBoundaryUnobservedにする。真のrootではこれらが存在しないことを考慮した観測状態であり、真のrootの証明や無制限表示ではない。この場合はcgroup.typeの存在も要求しない。[公式memory fileのnon-root範囲](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)。

leafや途中の3file全欠落はエラー。境界でも一部欠落、PermissionDenied、その他read error、不正値はエラーであり、Noneへ補完しない。観測できたlevelのNoneはそのfileの`max`だけを意味する。全可視levelが`max`でも外側の祖先はUnknownのままで、無制限の実効容量にはならない。

## 競合と信頼境界

memory pathの各componentをsymlink_metadataで検査し、検出symlinkは拒否する。観測前後でmembership／mountinfoの全内容と、/proc/self/ns/cgroup・mntのnamespace targetのdevice／inodeを照合する。namespace identityはmagic linkをfollowするmetadataを使い、symlink自体のinodeで代用しない。検出した所属・mount・namespace変更は観測全体のエラーにする。

これでもTOCTOU、途中のmount変更、所属移動のABAは排除できない。3memory file間・複数ancestor間の値も同時刻ではない。pathが実際にcgroupfsか、見えない親がないか、初期namespaceかも証明しない。信頼できる安定したproc／mount環境が前提であり、観測結果は権限capabilityではない。kernelもprocからpathを構成して操作する方式の競合を説明している。[公式の競合に関する説明](https://docs.kernel.org/admin-guide/cgroup-v2.html#thread-granularity)。

race-freeなpath confinementが必要なら、fdを基準にした安全なresolverやfilesystem identity検証の採用を別途検討する。stdのpath-based openと検査だけでそれを達成したとはしない。unsafe_code=forbid、新依存なしを維持し、今回は手動FFI・権限追加・controller書換えを行わない。

## 検証と未完了

Windowsで新moduleの5 unitが成功。parserのraw byte保持、escape／path／重複／mount root／submount拒否、所有temp fixtureによるleaf→boundary、全欠落と部分欠落の区別、不正値／128bytes bound／domain拒否、注入readerによる前後変更、非Linux Unsupportedを検査した。追加負例としてleaf3file全NotFoundと、境界2file NotFound＋残り不正値も既存test内で確認した。

対象unit、server all-targets Clippy（-D warnings）、Linux all-targets cargo checkが成功した。Linux専用symlink testはcross-compileだけで、native実行していない。実proc・cgroup namespace・delegation・mount変更をLinuxで実証したという記録ではない。統合gateは別途記録し、この文書では実行中の結果を先取りしない。

cgroup v1・Windows job・process RSS、namespace外祖先、safe fd resolver、native Linux観測、継続監視、共有poolへの接続、複数process配分、自動算式は後続。利用者向け手動容量設定を追加せず、CLI cacheの保存・ヒット停止を維持する。既存のDB・binlog・復元試験の成果とは独立した候補である。[cache開発記録](http-cache-development.ja.md)、[容量仕様](cache-and-settings.ja.md)。
