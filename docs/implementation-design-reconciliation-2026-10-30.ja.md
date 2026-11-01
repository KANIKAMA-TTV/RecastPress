# 実装状況と上位設計の照合（2026-10-30）

製品コードの開発中断後に行った文書整合化。今回の作業はソース・検証記録の読取りと文書更新だけで、製品コード、互換証跡、依存台帳を変更せず、全体試験・DB起動・配備も行っていない。要求R01〜R61、試験計画T01〜T44の採用内容は変更していない。

## 判定の基準

「実装済み」はコードが存在すること、「対象検証済み」は記載した条件の試験を通ったこと、「製品として達成」は上位要求と受入条件まで満たしたことを区別する。過去checkpointはその時点のソースの履歴であり、現在のソースへの成功証明へ付け替えない。基本設計レビューの原文と2026-10-25終了記録も歴史資料として保持する。

現時点の試作用途に限れば、公開起点のPHP禁止、公開／管理の役割分離、WP正本維持、手動キャッシュ容量を設けない方針を破る実装は確認していない。ただし、下記は部分実装を製品経路へ接続するときに解消が必要で、実運用の安全性や全要求適合を保証した結論ではない。

## 現在地

| 領域 | 実装・実測の範囲 | 未接続・未完了 |
| --- | --- | --- |
| 公開HTTP・package | sealed bundle、receipt照合、固定ZIPの書出し／native取込み、HTTP条件付き応答・Range、typed CSS／JSのbundle V3。旧全体gateと固定ブラウザー観測あり | 編集DBからの常駐ライブ反映、配布元認証、一般テーマ、製品配備 |
| 公開世代・Wasm | process内revision、選択した複数pageの同世代読取り／最終callback検査。独立selected-Wasm consumerのnative７契約・別current-A main実測 | 製品engine／SDK／ABI、HTTP描画経路、複数processの切替・撤回、socketへ渡したbytesの回収 |
| DB | 固定WP参照DDL12表、raw reader／個別lookup、既存投稿３textとcredentialのCAS、snapshot／binlog／障害を扱う所有fixture | WP保存フック全体、認証付き編集、永続job／outbox、製品常駐購読、全SQL・任意schema対応 |
| 認証部品 | 既存生成／解析／password／credential部品にprepared GET・non-Ajax Cookie照合を追加。直近Rust focused21、Python12、fmt・WP全target Clippy成功 | 採用後の実PHP30関係・全体gate、principal／権限／CSRF／HTTP／OIDC・session lifecycle。管理readyは503 |
| 最新検証の区切り | Cookie採用前のtarget-only実WP30、native両endpoint60、closed CLI５は成功。直前sealed-assets全体gateの履歴はRust636＋独立29／Python436／DB８mode各62 | Cookie本体採用後の正式79 API・519,014、hooks572、dispatcher405、認証別枠４種、DB８mode、pure・依存／license・全体再検証 |

[中断記録](../target/auth-cookie-verification-pause.json)と[Cookie照合の開発記録](auth-cookie-verification-development.ja.md)を正本とする。旧formal３証跡のbytesは `target/auth-cookie-verify-pre-refresh-{primitives,hooks,dispatcher}.json` に保存され、再実測時の入力／結果SHA・件数比較に使う。現在の成功flagを作るためにSHAだけを書き換えない。

## 重要な順の衝突・未決事項

優先度は次に扱う順番であり、脆弱性の重大度判定ではない。「要判断」は新しい製品条件の合意、「実装・検証」は既決事項を実現する作業を示す。WP標準の採否や初期値を同じ質問として出し直さない。

| 順位 | 分類・対象 | 上位層との関係と影響 | 次に必要なこと |
| --- | --- | --- | --- |
| 1 | 検証の空白／記述衝突 | 本体Cookie変更後も旧証跡をcurrentとして読むと、実装の検証状態と食い違う。逆に上位文書の「実装は未着手」「DDL未抽出」も現物と食い違っていた。今回、現状と歴史を分けて修正した | **実装・検証**：実metadataからinventory→licenseを再生成後、source freeze下でformal３・認証４関係・全８DB等の全体gate、旧結果保持比較、最終12checker。製品判断待ちではない |
| 2 | 認証付き編集への接続／D06・D13・R34・R41・R44 | MACとprepared sessionが一致してもprincipal・role／capability・CSRFの証明ではない。現helperはGET／non-Ajax・canonical i64・caller時刻だけで、標準WP全Cookie処理ではない。OIDC手続き専用sessionを通常sessionへ取り違える接続は上位方針に反する | **要判断＋実装**：salt／秘密・session保存と失効、OIDC連携・一度限り許可の原子消費、期限・回復・既存session。WP標準処理の不足branchは仕様抽出して拡張。管理503は維持 |
| 3 | 公開反映・撤回／R49・R61・D14 | DB COMMIT、cache purge、Readerの最終revision検査は、それぞれ公開反映完了とは異なる。現serverは起動時に読んだsealed世代を配信し、A/Bは別process観測。process内callbackの拒否で、既にtransportやbrowserへ渡したbytesを撤回できたとはしない | **要判断**：外部更新の反映遅延、非公開化・削除の期限、期限超過時の応答、保存ACKと公開ACKの区別。**実装**：常駐反映／再構築／複数process失効・復元時の再露出防止 |
| 4 | Wasm・持込み成果物の採用／D01・D04 | 独立experimentで固定Wasmtime36.0.19を使っても製品engine採用ではない。製品のGPLv2配布条件、SDK／ABI・能力・資源・更新互換性を満たす確認が残る。receiptは同一bytesのchecksumで、署名・作成者・公開承認の証明ではない | **実装側提案＋必要な判断**：製品依存closureと配布単位の個別監査、SDK／ABIと受入・署名／由来の契約。CSS／JSのUTF8受理をbrowser codeの安全性審査としない。ライセンス不適合をこの照合だけで確定していない |
| 5 | α合格条件・WP標準機能／D03・D10・R31 | 79 APIや固定HTTP／DB実験の件数だけでは、ログイン→編集→保存→公開→閲覧という利用者操作の合格にならない。３text CASはwp_update_postではなく、workbenchもDB editorではない。検索・media・comments・REST等の初回採用要求は残る | **要判断**：αの受入操作・代表サイト／theme／pluginの版・試験データ。**実装・調査**：標準機能のsource-derived契約と操作の結合。未実装を初回対象の削減決定へ変えない |
| 6 | DB・三環境の正式対応／D07・D11・R58〜R61 | Windows所有fixtureとLinux向けcross checkは、Linux native・Docker Compose／Railway／Lightsailでの常駐動作を証明しない。snapshot／ログ再開の候補試験を製品monitorや全schema対応と扱うと要求との間に空白が残る | **要判断**：正式DB／OS／CPU版、配備先のbinlog権限・保持・監視不能環境の扱い。**実装・検証**：native三環境・切断／ログ消失／再構築と最小権限 |
| 7 | 容量完全自動・更新時の資源／R52・R57 | explicitなfixture cap／論理予約と、製品の自動容量管理は別。process診断や共有bodyも全RSS／allocator／多process総予算を保証しない。新旧本体二重起動・Wasmロードの余裕も未実測 | **実装側提案＋検証**：共有予算の算式、圧迫時の停止／縮小／復帰と更新延期条件。性能・停止・資源の合格値はD14で合意。管理者の手動容量設定を追加しない |
| 8 | PHP／直接SQLの権限／D08・D09 | Rust所有fixtureのSQLや外部更新の追跡を実装しても、pluginの直接SQL・任意table・外部副作用を許可したことにはならない。D08-01のA案は未採用 | **要判断**：許可領域、独自tableのDDL・DB直結、通信／mail／file権限、保存フック再入と他投稿への影響。**実装**：PHP host／IPC・操作別認可・監査。公開PHP fallbackは禁止のまま |
| 9 | 永続公開・復旧・更新／D12・D14・D18／27 | in-memory revision、固定bundleと２file packageの成功は、永続job／outbox・冪等実行・世代切替・backup復旧・Rust全体二系統切替を満たさない。packageは署名やatomicな２file公開単位ではない | **要判断**：RPO／RTO、保持、切戻し、スキーマ変更ありの更新。**実装・検証**：永続管理状態、役割引継ぎ・排出・二重実行防止、整合backup／復旧。R57の採用自体は再質問しない |

直ちに上位要件を変更する根拠はない。まず順位１の再検証で現在の採用状態を確定し、その後、順位２・３の接続条件を具体化する。未決の製品条件は提案と判断を分け、既決の標準動作は実装・検証へ進める。

## 修正した文書上の食い違い

- 詳細設計・未決一覧の版／要求範囲と「実装未着手」を、v0.45の文書整合化・R01〜R61／T01〜T44・部分実装へ同期。
- DB上位文書の「DDL抽出・DB操作・試験未実施」を、固定DDL／所有fixtureの既実測と製品最適化・常駐監視の未実装へ分離。
- 認証設計の「Cookie／session部品がない」と、既存生成／解析・新prepared照合を分離。製品認証未接続は維持。
- Wasm更新の「旧世代を完走」という無条件の記述を、失効検査を満たす結果だけ継続する契約へ訂正。旧bytesの保持は配信許可ではなく、公開撤回を優先する。
- process memoryの「CLI未実装」をinspect-memoryの実装済みへ訂正し、自身のRSS等の診断と製品allocator／自動容量管理の未接続を区別。
- Wasm候補・静的資産・cache／メモリの観測を製品engine・一般theme・自動容量管理へ拡大しない状態表記へ同期。
- 現在の開発／実行／依存／互換文書は、古い証跡のcurrent断定を外して再開時の再生成順を提示。過去の原ログとcheckpointは改変しない。

今回の文書確認は既存の採用要件とローカル実装の照合であり、新しい法的判断、脆弱性の不存在、外部サービスの最新仕様・配信状態の確認ではない。
