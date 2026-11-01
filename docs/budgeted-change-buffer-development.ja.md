# 変更bufferと共有論理予約の接続候補

更新日：2026-10-27（日本時間）。target/shared-buffer-and-pretty-json-full-check.logが成功。workspace Rust251件（通常ignore1）＋独立experiment19件・Python86件、固定PHP比較45 API・304,071ケース（JSON 17,073件・9flags全512組合せ）／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208 package観測が通過した。全3 fixtureは停止・回収済み。これは有限fixtureと部分的な論理予約の検証で、製品CDC・全RSS上限・全process配分の完成ではない。

## APIと既存契約

DBのBudgetedChangeBuffer<P>は、privateなChangeBuffer、Option<Reservation>、BudgetPoolをmove-onlyで所有する。innerを先、leaseを次にdropするfield順で、Debug／Deref／mutable inner／into_innerを公開しない。pending batchは借用だけで、payloadをDebugへ出さない。

new(checkpoint, payload_budget, pool)、push、checkpoint／charged_bytes／fault／pending、apply_payload_budget、disconnect、同期／async apply_pendingを提供する。空／position-only batchはraw行を保持しないため予約を作らない。最初のRowでChangeBuffer分類のReservationを取得し、後続Rowでは新しい合計へtry_resizeする。

既存ChangeBuffer::pushはprivateなRow admission hookへInfallibleのno-opを渡す。旧public Event／Fault／BufferError／OutcomeとAdvance規則は変更しない。検査順はBroken fault→DDL→PendingのApplyRequired→位置gap→状態／event組合せ・local cap／overflow→Row予約→rows.push。DDLをpending backpressureで隠さず、未知eventやDDLをAdvanceへ読み替えない。

poolのExhausted／UnavailableはBudgetedPushError::Budgetで、旧bufferエラーはBufferで区別する。global拒否では入力Rowだけをskipせず全Transactionを破棄し、古いcheckpointを保持してPayloadBudgetExceededをlatchする。復帰には既存のdurable照合が必要で、次の行を投入して自動回復しない。

## 保持・解放・durable結果

| 状態／操作 | 予約の扱い |
| --- | --- |
| staging→Pending | 同じdataとleaseを保持 |
| callback Err／ack紛失／async cancel | Pendingとleaseを保持して同一batch再試行 |
| applyがfalse、途中Transactionあり | Transactionのleaseを保持 |
| apply成功 | checkpoint更新・data破棄後にleaseをdrop |
| rollbackで空Pending／disconnect／DDL／gap／cap fault | data破棄後にleaseをdrop。checkpointは既存規則に従う |
| wrapper Drop | inner data→leaseのfield順でdrop |

解放をfallibleなresize(0)へ置き換えず、dataが0になってからOptionのleaseをdropする。core Reservation::Dropはpoisonでも非Result／非panicで、返却不能なchargeを保守的に残す。これにより、成功したCOMMITを解放後処理のErrで未COMMIT扱いにしない。poisonを台帳復旧や利用可能量の増加へ読み替えず、後続の正のRow予約はUnavailableになる。callbackへbudget lockを持ち越さない。

ack紛失後はRAM checkpointが古くてもdurable側が進み得る。disconnectやcap縮小でPendingを破棄しleaseを返しても、durable rollbackを意味しない。再開前に保存source／positionとbatch identityを照合する従来の責任は維持する。

## chargeと計上外

Rowごとの既存chargeをそのまま予約する：

`raw Vec<u8>.capacity() + 2 × size_of::<Vec<u8>>()`

raw payloadの余剰capacityも含み、lenだけの計算へ変えない。empty payloadでもper-row allowanceは必要。合計はcheckedで、local capに収まることを検証してから共有poolへ予約し、予約後にだけbufferがRowを保持する。Row受信前にdecoder側で作られた入力allocationを抑止する保証ではない。

decoder／input、受信event、デコード済みMetaChange、frame照合・receipt照合の比較コピー、SQL結果・driver、position、outer Vecの余剰capacityやnode overhead等はこの予約の対象外。全RSS／peakや同一process全allocationの上限ではない。別pool同士は相互調整せず、cross-process／cgroup配分・自動算式・CLI cacheへ接続しない。allowanceは内部host／fixture値で、製品の手動容量設定ではない。

## native接続の範囲

既存の各ProbeBufferはBudgetedChangeBufferを使用する。通常constructor／resumed constructorはそれぞれ専用の1MiB poolを作るため、すべてのprobeが一つのprocess-wide poolを共有したという試験ではない。

postmeta有限windowだけは明示的に同じ1MiB poolを渡し、BudgetedPostMetaViewのInternalRows／Indexと、staging raw frameのChangeBufferを合算する。3 COMMIT用callbackで三分類のclaimedがすべて正であることを必須にする。source再接続時も同じpoolを渡し、旧Arc・新candidate・raw Pendingの同時保持を過小計上しない。

別consumerへ0byte poolを渡した負例は、実際のRowsEventでtyped BudgetError::Exhausted、PayloadBudgetExceeded、元checkpoint保持、旧view保持、claimed=0をすべて要求する。単なるnetwork／timeout等のErrや、COMMITまで受理した結果は成功扱いしない。

最後にwrapper／view／旧Arcを破棄してpoolのclaimed=0を確認する。この0は予約対象ownerが残っていないという観測であり、課金外の比較用コピーやprocess全メモリが消えたという意味ではない。

両通常DBのstrict JSON／metadataで以下を照合した。

- postmeta_shared_pool_commits=3
- postmeta_shared_pool_refusal_preserved=true
- postmeta_shared_pool_released=true

## 検証と残件

新pure契約はintegration8件＋private Unavailable注入1件。capacity511の短bodyとcapacity19のempty bodyを連続stageしてreserve→resize合計を確認し、既存優先、全取引拒否、ack紛失のidempotent retry、async cancel、callback内の別分類予約、rollback／disconnect／DDL／Drop／cap fault、共有view・indexとdebt、ack紛失後のRAM破棄とdurable結果の区別を検査した。実poisonのDrop policyは既存core unitで、wrapperのUnavailable伝播はprivate注入で検査する。

旧238件のbudgeted view／loader gate、230件のJSON flags gateはそれぞれ過去の記録として維持する。snapshot／log protocol、source identity、連続CDC、公開・cache失効、自動配分・全RSS保証へこの部分試験を拡張解釈しない。[予算付きview](budgeted-meta-view-development.ja.md)、[DB開発記録](database-development.ja.md)、[cacheの範囲](http-cache-development.ja.md)。
