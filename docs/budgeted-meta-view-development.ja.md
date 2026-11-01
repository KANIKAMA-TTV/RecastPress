# 投稿メタviewと共有論理予約の接続候補

最新phase3：target/shared-buffer-and-pretty-json-full-check.logでworkspace251件＋独立experiment19件・Python86件、PHP比較45 API・304,071ケース（JSON17,073・512組合せ）／hooks572／dispatcher405、通常両DBと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208観測が成功し全fixtureを回収した。postmeta有限windowでは同じpoolへ行／索引とBudgetedChangeBufferのraw保持を接続し、3 COMMITの三分類claimed・pool0の型付き拒否・最終drop後0を照合した。[共有bufferの接続範囲と課金外](budgeted-change-buffer-development.ja.md)。すべてのDB allocationや他probeを同じpoolへ接続したものではない。

直前phase2の記録：2026-10-27（日本時間）。target/budgeted-meta-view-and-loader-cache-final-check.logが成功。workspace Rust238件（通常ignore1）＋独立experiment19件・Python86件、固定PHP比較45 API・299,957ケース／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208 package観測が通過した。今回PHP再比較とDB live実測を行い、全3 fixtureを回収した。cargo graphはworkspace155／独立probe188、unique inventory208を維持。R61の同一process論理ownerという部分実装であり、製品の自動配分・全RSS・公開／cache連携の完成ではない。

## APIと所有範囲

recastpress-db::budgeted_meta_viewのBudgetedPostMetaViewはmove-onlyで、privateなPostMetaView、InternalRowsのReservation、optionalなIndexのReservation、同じ台帳を指すBudgetPoolを所有する。

| API | 契約 |
| --- | --- |
| clone_from_rows(rows, pool, max_rows, max_logical_bytes) | 借用した入力を検証・予約してから最終行bodyをclone |
| prepare_changes(changes, max_rows, max_logical_bytes) | 順序付きstrict検証後、同じpoolへ新世代全行分を追加予約して別ownerを返す |
| try_build_key_index(max_entries, max_logical_bytes) | charge計算・新予約後に別indexを構築し、成功時に交換 |
| discard_index() | indexのdataを破棄してからIndex leaseを返す |
| rows()／matching_key() | ownerの寿命に結び付いた読取り借用だけを返す |
| EditingSnapshot::load_budgeted_post_meta_view(pool, max_rows, max_logical_bytes, page_size) | 同じRRで長さpreflight→予約→本文load・再照合 |

Clone／Copy／Deref／into_inner／mutable inner viewを公開しない。callerはArc<BudgetedPostMetaView>で同じ世代を共有できる。同じArcの複製はbodyを複製せず予約も増やさず、最終Arcまでdataとleaseを保持する。viewをReservationより先にdropするfield順にしている。callerが借用rowsを明示cloneして作る別データまではこのownerの計上対象ではない。

## 予約と割当の順序

行のconstructorと変更準備は、借用BTreeMapによるID重複・欠落・完全before image・PK移動の全strict検証、最終件数とchecked logical bytes、InternalRows予約、payload cloneの順。strictエラーは上限・pool不足より先に返す。予約失敗時にprefix candidateを返さず、旧view・索引・予約を変更しない。

新世代は旧世代のleaseをresize／移譲しない。旧bodyと新bodyの両方が存在する間は両者をclaimedに残す。削除後の新世代が小さくても、同時保持分が足りなければExhaustedになる。empty candidateはhealthy poolなら0byte leaseで作れる。入力changesや元の借用rows自身は別の所有物であり、その保有分をここで新たに予約したとはしない。

索引はborrowed key setでunique非NULL keyと全行offsetを検査し、そのsetを破棄してからIndex予約を取得する。新indexをoffside構築し、swap成功後に旧index dataをdropし、それから旧leaseをdropする。rebuild中は旧・新index分の両予約が必要になる。

| 結果 | 旧index／lease |
| --- | --- |
| max_entries／logical byte capの明示的拒否・overflow | dataを破棄→leaseを返却しScanへ |
| BudgetError::Exhausted／Unavailable | Errを返し、旧indexとleaseを保持 |
| 新index成功 | 新dataとleaseへ交換後、旧data→旧leaseの順に解放 |

この失敗契約は、最初に旧indexをdiscardする既存unbudgeted builderと意図的に分けている。既存APIの挙動を変更せず、charge計算とoffside構築をprivate helperとして共有した。poolのlock中に外部callbackを実行しない。

pool縮小は既存claimedを消さずdebtとして残し、新しい正の予約を拒否する。既存データを解放済みと扱わず、明示discard／Dropで減らす。poison時は既存core契約のUnavailableを維持し、Dropで台帳を復旧したり、過小なclaimedを作ったりしない。poison後の返却不能なchargeは保守的に残り得る。

## 同じsnapshotでのloader

既存byte-bounded loaderと共有するprivateな長さpreflightは、同じread-only RR transaction内で全表のIDとnullable OCTET_LENGTHを読む。件数・logical byte capに適合した後にInternalRowsを予約し、それから既存row loaderで本文を取得する。本文の全件数と合計chargeを再照合してownerを返す。追加の長さpassを重ねず、長さと本文の2pass構造を維持する。

row／byte cap拒否はLoad error、pool不足はBudget errorとして区別する。予約拒否後もsnapshotを通常paged readに使える。ownerはsnapshotと独立して保持でき、snapshot終了だけではleaseを返さない。安定schema前提は既存loaderと同じで、件数・合計chargeだけで任意のDDLや同サイズ改変を完全検出するものではない。

所有DB fixtureは11行のexact charge、pool0拒否、snapshot再利用、索引予約不足・Scan／再構築、世代同時保持、Arc最終dropを検査する。さらにserver上の専用fixture値を1MiBにし、logical byte capをMAX・pool0にした場合も長さだけ読んでBudgetError::Exhaustedとなり、同接続のBytes_sent差が64KiB未満であることをassertする。値は復元して後続の11行／binlog条件を維持する。

## 計上式と未計上領域

行：`Σ(size_of::<PostMetaRow>() + non-NULL key.len() + non-NULL value.len())`

索引：`row_count × size_of::<usize>() + Σ_unique非NULLkey(size_of::<(Vec<u8>, Vec<usize>)>() + key.len())`

NULL行のoffsetも含む。同じkeyのindex bodyは1回計上し、NULLとempty keyを区別する。view内のinline KeyIndex rootは除外する。行keyとindex keyは別owned bytesなので両方の論理chargeを持つ。

ref map／key set preflight、tree-node overhead、Vec／allocator capacity、driver／page buffer、入力changes、他のunbudgeted view、peak allocations、実RSSは未計上。既存unbudgeted APIは引き続き使用でき、その割当を遡って共有poolへ登録しない。同じprocessでも独立poolを作れば相互調整しないため、hostは同じ外部allowanceを別poolへ二重配分してはならない。

cache・decoder／変更イベント比較コピー・index buildの一時領域・rendering全体、cross-process／cgroup配分、環境の実監視、自動容量算式、CLI cacheへは未接続。allowance／fixture値は内部hostから渡すもので、利用者向け手動容量設定を追加していない。

## 依存と証跡

既存workspace内のcoreへDBからpath依存を追加し、core→DBのcycleはない。新しい外部crateや外部package version変更はない。独立probeのresolved集合はcoreが加わり188 package、全体のunique inventoryは208のまま。両lockfileとinventory／license観測を更新する対象になるが、法的判断やlicense選択を新たに完了したものではない。

DB proofはDBだけでなくcoreのsrc／tests／Cargoもrequired sourceとして追跡する。通常binlog proofの生成側とcheckerも同集合へ同期し、overlap proofにもrequired_files経由で反映する。これにより予約台帳の実装変更を、DB wrapperだけのhashで見逃さない。

## 検証状況と再実行の区別

新しいpure契約はintegration7件＋private Unavailable注入1件。exact／不足、NULL／empty／non-UTF8／ID0・MAX、old＋new予約、Arc最終drop、index rebuildのExhausted保持、明示Scanと再構築、debt、strict failure優先を確認した。Unavailableのwrapper伝播は注入試験で、実際のpoison policyは既存core unitで確認する。対象既存契約とClippyも通過した。

初回target/budgeted-meta-view-and-loader-cache-full-check.logでは通常MariaDB／MySQLと専用GTID MySQLの各39項目・WP比較まで通ったが、最後のschema generator tool hash照合でreference_functions.phpの古い記録を検出しexit1となった。DB stageやWP stageの部分成功を、全gate成功と扱わない。

schemaを再生成したDDL SHA-256は`cabfad5a96024c18951e6193289d1b78e5767986c0e21a61e3369e9fb9bbe83c`で不変。起動前にschemaのtool_filesを照合するguardとnegative unitを追加して修正した。その後target/budgeted-meta-view-and-loader-cache-final-check.logで全gateが成功し、3 fixture各39項目・1MiB／pool0拒否・既存復元／選別／overlapまで再実測し全fixtureを回収した。今回MySQL overlapのprefixはfixtureの2 write追加に伴い134となった。旧132は過去の独立fixture観測として保持し、固定の製品期待値にしない。

[DB開発記録](database-development.ja.md)、[cacheと資源観測の境界](http-cache-development.ja.md)、[資源観測候補](cgroup-memory-development.ja.md)も参照。
