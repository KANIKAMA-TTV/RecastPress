# RecastPress 設計作業の引継ぎ

> **2026-10-30 文書整合化完了**：[実装状況と上位設計の重要順照合](implementation-design-reconciliation-2026-10-30.ja.md)を先に参照。詳細設計v0.45（R01〜R61／T01〜T44、採用要件変更なし）へ現在地を同期。旧「実装未着手」「DDL未抽出」と無条件の旧Wasm世代完走を訂正し、部品／有限試験／製品達成を分離。最優先はCookie本体変更後の再検証、次が認証・認可の接続と公開反映／撤回期限。製品コード開発は引き続き中断し、今回ソース・証跡・依存台帳・DBは変更していない。

> **ユーザー指示で中断・Cookie照合の本体取り込みはfocused検証まで**：`auth_cookie_verify` と既存生成側のprivate MAC共通化、strict example、別枠30関係のoracle／case／checker／comparator／任意gate flagを保存。Rust focused21、Python12、fmt／WP全target Clippyは成功し全担当process回収済み。先行target-onlyでは実WP30・native両端60・closed CLI5が成功したが、本体採用後の正式79 API／519,014・既存crypto3＋新関係・全8 fresh DB／pure／依存inventory・license／全体gateは未実行。既存証跡は変更前の履歴で、現在のsourceへの成功へ付け替えない。管理ready503・principal未接続を維持。再開手順は `target/auth-cookie-verification-pause.json`、旧formal3の比較用copyは `target/auth-cookie-verify-pre-refresh-{primitives,hooks,dispatcher}.json`。

> **selected Wasm consumer独立package採用・最終checker完了**：[固定２引数consumer](selected-wasm-consumer-development.ja.md)を新独立workspaceの６fileだけへ採用。adopted native７／fmt／Clippy成功と、別実PID36952のcurrent-A mainだけ（body51）、unexpected argument38160のclosed exit2を分けて記録し全回収。registry118は旧request146 tupleの部分集合・追加変更０、既453選択source bytes同一。normal server51／DB100のtreeとROOT SourceFreeze検査も成功（`target/selected-wasm-adopted-{audit,main-observation,normal-graphs}.json`）。最終11checkerは全exit0、controller出力を回収し、docs freeze後に `target/selected-wasm-adopted-checkpoint.json` を生成した。直前assets checkpointを保持し、Rust636／Python436／DB／HTTPの全再実行とは扱わない。guestはStateLock外、copy／post-return後の最終callbackは完成Vecのmoveだけ。製品SDK／engine／HTTP render／socket撤回／auth／RAM policyは未採用。

> **sealed CSS／JavaScript全体gate・最終checker完了**：[事前生成asset](sealed-public-assets-development.ja.md)の3回目gateはexit0（`target/sealed-assets-full-check.log`、session73123回収済み）。Rust636（ignore1）＋独立29／Python436（429成功・skip7）／認証3種actual／全8 fresh DB mode各62／Clippy・Linux cross compile・default inventory218・HTTPが成功しDB全回収。最終11全exit0（`target/sealed-assets-current-check-summary.json`、checker controller出力回収済み）。baseline23中13proof更新（DB10＋認証3）・残10保持（WP3＋依存7）とsource13不変を実assert。pure313／license file218は今回再実測せず保持。旧unsupported-versionとHTTP起動前提の2失敗log・test-only訂正を残す。V1/V2／report1七MIMEの実byte保持、V3／report2九MIME、固定desktop／Wasmpub成功は専用doc参照。checkpoint `target/sealed-assets-checkpoint.json` は生成済み。一般theme／SDK・mobile・native Linux完成ではなく、その後の独立Wasm consumer採用は上段の別checkpoint参照。

> **同期one-shot 全体gate・最終checker成功**：[runtime入口分離](synchronous-oneshot-development.ja.md)のguard付き全体gateはexit0／session95981回収済み（`target/oneshot-runtime-full-check.log`）。Rust626（ignore1）、Python431（424成功・skip7）、認証３種actual、fresh alpha２mode各62、Clippy・server/WP Linux cross compile・通常inventory218・HTTP成功。pure313／license file観測218／binlog29は今回再実行せず保持。Wasmpubも所有HTTPだけ再成功、同一producer SHAのnative６は既観測保持（`target/oneshot-runtime-wasm-http-observation.json`）。baseline23のalpha２＋認証３だけ実更新し18file保持、source３不変・DBdir空、全体試験中のactual22 childと全回収を実assert。最終11checkerは全exit0（`target/oneshot-runtime-current-check-summary.json`）、controller28341回収済み。checkpoint `target/oneshot-runtime-checkpoint.json` はdocs freeze後rootが生成する。新CLI、RAM／速度／thread数改善、全８fresh DB、WP再比較、認証policyは主張しない。

> **同一世代selected pages：全体gate・最終checker成功**：[選択ページbatch](selected-public-pages-development.ja.md)のguard付き全体gateはexit0／session75334回収済み（`target/selected-pages-full-check.log`）。Rust626（ignore1）＋pure313（62 artifact／features空／normal WP1）＋独立29、Python426（419成功・skip7）、認証3種actual、全8 fresh DB mode各62、Clippy・Linux cross compile・license218・HTTP成功。依存Wasmpub native6＋所有HTTP／capture2／比較8trueも再成功し全回収（`target/selected-pages-wasm-http-observation.json`）。baseline23のDB10＋認証3を実更新し、WP3＋依存7の10fileをbytes同一で保持、source2file一致とDBdir空を実assert（`target/selected-pages-retained-identities.json`）。WP正式79／519,014の再比較は行っていない。最終11checkerは全exit0（`target/selected-pages-current-check-summary.json`）、controller20541もexit0で回収済み。checkpoint `target/selected-pages-checkpoint.json` はdocs freeze後にrootが生成する。core native50／明示2limits／単一read lock／選択Arc保持とgroup freshnessを維持し、実concurrent batch race・現sealed1page HTTPのbug修正・theme host・管理認証503の完成とは扱わない。

> **固定ZIP import 全体gate・最終checker成功**：[unpack-package](package-unpack-development.ja.md)のguard付き全体gateはexit0（`target/package-unpack-full-check.log`、session14751回収済み）。Rust620（ignore1）＋pure313（62 artifact／features空／normal WP1）、Python426（419成功・skip7）、認証３種actual、fresh alpha MariaDB／MySQL２mode各62、Clippy・Linux cross compile・license218・HTTP成功。全８fresh DBとは扱わず、baseline23のalpha２＋認証３だけ更新し18file保持、source６file不変とDBdir空を実assert。依存Wasmpubもnative６＋所有HTTP再成功、body153／bundle281／receipt117、post_return／copy後mutation、capture２回・比較８trueと全回収を確認（`target/package-unpack-wasm-http-observation.json`）。初回sandbox10013の診断を保持し同fixture retry成功、コード修正無し。最終11checker全exit0（`target/package-unpack-current-check-summary.json`）、controller34301回収済み0。残る６DB modeの既観測を保持し、mode数をproofJSON数と混同しない。checkpoint `target/package-unpack-checkpoint.json` はdocs freeze後rootが生成する。固定ZIP限定・partial出力保持・既存先拒否を維持し、一般ZIP・認証・自動公開ではない。

> **workbench ZIP 全体gate・最終checker成功**：[単一 ZIP 書出し](prepared-post-workbench-package-development.ja.md)のguard付き全体gateはexit0（`target/workbench-package-full-check.log`、session21914回収済み）。Rust610（ignore1）＋pure313（62 artifact／features空／normal WP1）、Python419（412成功・skip7）、認証3種10／113（actual88＋refusal25）／33（actual25＋refusal8）、Clippy・server/WP Linux cross compile・license218・process smokeが成功。Node29、native retry４件・12種dummyの２file bytes一致、所有sealed HTTPも確認済み。実ブラウザーのWebCrypto準備・表示は確認したが、OS ZIP保存／downloadは未実行。今回はfresh DB起動もWP79／519,014・hooks572／dispatcher405の再比較も行っていない。最終11checker全exit0（`target/workbench-package-current-check-summary.json`）、controller49832回収済み0。認証3証跡を除く20／23fileはbaselineとbytes同一でWP3proof／全DB証跡を保持し、選択source10file不変も実assert（`target/workbench-package-retained-identities.json`、`target/workbench-package-source-freeze-identities.json`）。checkpoint `target/workbench-package-checkpoint.json` はdocs freeze後にrootが生成する。認証・theme・DB・自動公開は追加しない。

> **開発gate source freeze guard完了**：[選択source固定確認](development-source-freeze.ja.md)の統合gate／最終11は全exit0（`target/gate-source-freeze-full-check.log`、`target/gate-source-freeze-current-check-summary.json`）。Rust610（ignore1）＋pure313／binlog29、Python415（408成功・skip7）、認証3種actual、影響するfresh DB4 mode各62、Clippy・Linux cross compile・license218・HTTPが成功し全回収。WP正式79／519,014・hooks572／dispatcher405は再比較せず3proof byte同一でcurrent保持。fresh4 modeと更新JSON数は区別し、今回全8DB再測定とは扱わない。157選択source等の一致も確認、旧79 checkpointは履歴保持。checkpoint生成はdocs freeze後、次workbench案は未main。

> **sanitize_text checkpoint検証完了**：[NoLiteralLt field／textarea](sanitize-text-development.ja.md)を正式79 API／519,014件へ追加。Rust610（ignore1）＋pure313（62 artifact／features空／normal WP1）＋独立29／Python402（396成功・symlink6 skip）／hooks572・dispatcher405／認証3種／全8 DB各62／Clippy・WP/server Linux cross compile・license218・HTTPが成功、全fixture回収。旧77保持は `target/sanitize-text-old77-preservation-check.log`、全体logは `target/sanitize-text-full-check.log`。最終11は全exit0（`target/sanitize-text-current-check-summary.json`、controller1951回収済み）。依存6fileの現SHA＝開始snapshot＝parser77 checkpoint SHAを実assertしDB空を確認、sanitize checkpoint生成はdocs freeze後の別操作。parser77 checkpointは完成履歴を保持し、自動DB／UI変換・HTML安全性・認証policyは追加しない。管理503を維持、Wasmpubはcore/server不変の既観測保持で今回再実測無し。次source guardはtarget stageのみ。

> **明示cookie parser checkpoint検証完了**：[raw field分解](auth-cookie-parse-development.ja.md)を正式77 API／513,292件へ追加。Rust602（ignore1）＋pure305／61 artifact／features空／normal WP1＋独立29、Python393（387成功・symlink6 skip）、hooks572／dispatcher405、認証3種、全8 DB各62、Clippy・WP/server Linux cross compile・license218・HTTPが成功し全DB回収。旧76入力順・件数・両結果SHAを保持（`target/auth-cookie-parse-old76-preservation-check.log`）。全体logは `target/auth-cookie-parse-full-check.log`、最終11は全exit0（`target/auth-cookie-parse-current-check-summary.json`、controller42156回収済み）。manifest／lock／inventory／license6fileの現bytes＝開始snapshot＝auth76 checkpoint SHAを実assertし、DB fixture directoryも空。parser checkpoint生成はdocs freeze後の別操作。auth76は完成履歴、Wasmpubはsource不変の前回static MIME観測を保持し今回再観測無し。field分解は認証検証ではなく管理ready503を維持。sanitize_textは次工程target draftでmain未採用。

> **最新static MIME checkpoint（2026-10-30）**：[固定MIME参照](static-public-mime-development.ja.md)を完了。pageが入力Stringの余剰capacityを保持せず、明示的なPublicPage cloneでもMIMEを複製しない内部差分。公開signature・self lifetime・text4／raster3／redirect領域・status優先順は維持。対象core21／全体gate exit0、Rust589（ignore1）＋独立29／Python374（368成功・symlink6 skip）／今回認証3種／全8 fresh DB各62／Clippy・core/server Linux cross-check・license file観測215・HTTPが成功、全DB回収。依存Wasmpub native6＋実HTTP／2capture／8比較も成功し全子回収。最終11checker全0（`target/static-mime-current-check-summary.json`）。WP75 API508,297／pure300はsource不変の既観測をcurrent確認し、今回再比較・再実測していない。callerの入力確保、他allocation、Arc<Page>構築費用、allocatorからOSへのRSS返還は保証しない。

> **最新page snapshot共有checkpoint（2026-10-30）**：[公開page metadata共有](shared-public-page-development.ja.md)を完了。内部 `Arc<PublicPage>` により読取／response cloneのMIME Stringコピーと入れ子Arc cloneを削減し、公開API・Revision／deliveryのfreshness判定を維持。対象core16／全体gate exit0、Rust586（ignore1）＋独立29／Python374（368成功・symlink6 skip）／今回認証3種10・113・33／全8 fresh DB各62／Clippy・core/server Linux cross-check・license file観測215・HTTPが成功、全DB回収。依存Wasmpubもnative6＋実HTTP／2capture／8比較成功・回収済み。最終11checker全0（`target/shared-page-current-check-summary.json`）。WP75 API508,297とpure300はsource不変の既観測のcurrent保持確認で、今回再比較／再実測していない。route毎のArc allocation増と引換えであり、RSS・latency改善の実測や撤回可能なtransportの実装ではない。

> **最新本文共有checkpoint（2026-10-30）**：[sealed public body共有](shared-public-body-development.ja.md)を完了。cache迂回・Range応答を元のArc payload owner共有へ変更し、cache挿入時コピー・lease・適格性・監視・LRUは維持。retry gate exit0、Rust581（ignore1）＋独立29／Python374（368成功・symlink6 skip）／全8 fresh DB各62／Clippy・core/server Linux cross-check・license file観測215・HTTPが成功、全DB回収。独立Wasmpubもnative6＋実HTTP／2capture／8比較が成功し全子回収。正式75 API508,297はsource不変の既証跡をcurrent checkerで維持し、今回再比較していない。workspace後に認証3binary差を検出したため、3種を現binaryで実再観測し、最終11checker全0（`target/shared-body-current-check-summary.json`）。pure300は直前観測の保持。旧pointer前提の2失敗と初回crypto stale記録を保持し、sourceやhashの手更新で成功扱いしない。本文共有はRSS・全allocation・network zero-copyの保証ではない。

> **最新Linux検証入口checkpoint（2026-10-30）**：[native Linux gate](native-linux-validation.ja.md)の選択試験を10件へ拡張し、workspace Clippy後へpure WPの独立検証を追加。通常commandも直接Popen handle・1800秒の待機期限・回収再試行を持ち、stdinはEOF、stdout/stderrはライブ継承。最終source13契約、package CLI6件、owned probe8件が成功。Windowsで実plan/helpは0、通常実行は非Linux guardで1、所有3childを回収（`target/native-linux-updated-entrypoint-observation.json`）。既存11checkerも全0（`target/native-linux-harness-current-check-summary.json`）。今回Rust・DB・全Pythonは再実行せず、直前title全体gateと区別する。Linux native、WSL、Dockerは未導入・未実行で、process-tree全体や全gateの時間上限を保証しない。

> **迂回メモ（当時の状態を保持）**：`wp_generate_auth_cookie` の限定移植は、既cacheのHMAC0.12（digest0.10）とMD5 0.11（digest0.11）のtrait不一致で標準providerの追加が必要だった。この時点では取得・暗号primitive自作・session／principal接続を行わず、resolved salt・PHP偽値token `"0"` 等のprofile条件だけを調査し、実装完了には数えなかった。

> **provider迂回解決・raw cookie生成checkpoint完了**：[raw認証cookie生成](auth-cookie-development.ja.md)を正式76 API／510,813件へ追加し全体gate exit0（`target/auth-cookie-full-check.log`）。default Rust596（ignore1）＋pure300＋独立29／Python384（378成功・symlink6 skip）／旧75のprefixとhooks572・dispatcher405保持／認証3種10・113・33／全8 fresh DB各62／Clippy・default WP/server Linux cross-check・license file観測218・HTTPが成功、全DB回収。HMAC0.13追加によるprovider trait迂回は解決し、上段メモは当時の履歴として残す。最終11checkerはすべてexit0（`target/auth-cookie-current-check-summary.json`、controller58647も出力回収後exit0）。既存user／resolved salt／truthy tokenをcallerが用意する返却bytesだけで、session／principal／Set-Cookieや管理ready503を開放しない。記事編集・保存、theme公開、native Linux試用は未完了。parserはtarget下の次工程draftでmain未採用。今回はWasmpubを再実測せず既観測を保持した。

> **最新標準タイトルcheckpoint（2026-10-30）**：[準備済みraw投稿と標準4filter](title-development.ja.md)を完了。全体gate exit0、default Rust573（ignore1）＋pure native300＋独立29／Python368（362成功・symlink6 skip）／正式75 API508,297（新12,961）・hooks572・dispatcher405／credential3種／全8 fresh DB各62／Clippy・default Linux cross-check・license file観測215・HTTPが成功。旧74の全件数と入力・両結果prefixSHA保持、最終11checker全0・全DB回収（`target/title-default-current-check-summary.json`）。pure featuresは空、normal依存WP1件。別段階のescaping→固定HTML bundle→receipt→sealed HTTP関係も成功（body310／bundle440／receipt117、6検査、起動後の両file変更でも同応答、Windows強制停止exit1・回収済み）。認可・公開資格や一般のtheme描画を追加したとは扱わず、既存literal入力へ自動接続しない。全体logは `target/title-default-full-check.log`。

> **最新文字整形checkpoint（2026-10-30）**：[wptexturize限定profile](texturize-development.ja.md)を完了。全体gate exit0、default Rust566（ignore1）＋pure native293＋独立29／Python353（347成功・symlink6 skip）／正式74 API495,336（新9,183）・hooks572・dispatcher405／credential3種／全8 fresh DB各62／Clippy・default Linux cross-check・license215・HTTPが成功。旧73の全件数と入力・両結果prefixSHA保持を実照合、最終11checker全0・全DB回収（`target/texturize-current-check-summary.json`）。pure featuresは空、normal依存WP1件。入力・中間buffer・最終出力の3上限を必須化。現在のliteralタイトル入力画面へ自動接続しない。全体logは `target/texturize-full-check.log`、次候補は標準4filter付きの準備済み投稿タイトル取得。

> **最新build feature checkpoint（2026-10-30）**：[default auth／pure WP分離](wp-build-profiles-development.ja.md)を完了。全体gate exit0、default Rust554（ignore1）＋pure native281＋独立29／Python342（336成功・symlink6 skip）／正式73 API486,153と旧全結果SHA保持／credential3種／全8 fresh DB各62／Clippy・default Linux cross-check・license215・HTTPが成功。pureの実compiler featuresは空、normal依存WP1件、defaultと明示auth graphは同じ。依存一覧とlicense file観測もmanifest変更後に実更新。最終11checker全0・全DB回収（`target/wp-auth-feature-current-check-summary.json`）。`target/wp-auth-feature-full-check.log`・`target/wp-auth-feature-integrated-pure-profile.json`を参照。Wasm compile・ABI・security sandboxの成功とは扱わない。次のtitle getter候補はdefault filter差を考慮して未選定。

> **最新cached単件metadata checkpoint（2026-10-29、日本時間）**：[get_post_meta限定profile](post-meta-cached-development.ja.md)を完了。全体gate exit0、Rust554（ignore1）＋独立29／Python335（329成功・symlink6 skip）／正式73 API486,153・hooks572・dispatcher405／credential3種／全8 fresh DB各62／Clippy・Linux cross-check・license215・HTTPが成功。旧72の入力・全件数・両結果prefixSHA保持も実照合、最後の11checker全0・全DB回収。実metadata calls/get/cache/restored3,202・default626・update/SQL0。`target/post-meta-cached-full-check.log`・`target/post-meta-cached-current-check-summary.json`を参照。complete/nonempty cacheとsingle/関連filter無しのcaller契約だけで、cache miss・wholecache・multiple・登録default・画像描画や認可は未追加。次候補は既定Auth動作を維持したpure build feature分離。

> **最新上限付き復元checkpoint（2026-10-29、日本時間）**：[新helper](bounded-unserialize-development.ja.md)と選択raw option/metaのdev-only結合を完了。全体gate exit0、Rust548（ignore1）＋独立29／Python324（318成功・symlink6 skip）／正式72 API482,951・hooks572・dispatcher405／credential3種実測／全8 fresh DB各62／Clippy・Linux cross-check・license215・HTTPが成功。旧72の入力・全件数・PHP/Rust結果SHA保持も実照合、最後の11checker全0・全DB回収。`target/bounded-unserialize-full-check.log`・`target/bounded-unserialize-current-check-summary.json`を参照。normal public/DB→WP境界を維持し、get_option／theme選択／認可やRSS保証は追加しない。次のmetadata cache候補は実consumer選定待ちで未実装。

> **最新receipt付き検査checkpoint（2026-10-29、日本時間）**：[check-package](package-check-development.ja.md)を追加しRust541（ignore1）／Python324（318成功・symlink6 skip）、Clippy／Linux cross-check／license215／HTTP、credential3種実測・alpha両DB各61、最終11checkerが成功。初回のtest fixture redirect違反1失敗はtest-only訂正し、初回logとexit0 continuation logを保持。保存済み2packageの新CLI検査・Wasm公開依存経路native6とHTTP再実行も成功し全回収。`target/package-check-current-check-summary.json`・`target/package-check-wasm-publication-observation.json`を参照。source hash付き総合checkpointを別保存する。次候補は既存serialized値復元の資源上限付きAPI＋raw行の明示選択で、get_option／theme選択／認可の意味は追加しない。

> **最新JSON再読込みcheckpoint（2026-10-29、日本時間）**：strict UTF8/schema reader＋明示適用・取消を追加しNode13契約成功、既存native比較73件も期待どおり（JS32／native30、最終出力上限だけの意図差2）。実browserはstaging／取消の元入力保持と狭幅を確認、ApplyはNode統合で確認しbrowser未操作。画像・parity JSON・Node logは`target/prepared-workbench-import-*`。所有server38092／25760はexit0・thread回収、tab3閉鎖。[詳細](prepared-post-workbench-development.ja.md)。次のreceipt-aware `check-package` CLIを実装中で、server source変更後はalpha証跡を実測で更新する必要がある。

> **最新ローカル入力画面checkpoint（2026-10-29、日本時間）**：[ワークベンチ](prepared-post-workbench-development.ja.md)の実download215 bytes→既存CLI bundle776／receipt117→別sealed HTTPで2記事の文字保持／GET／HEAD／query逆順404／Host421を確認。重複・予約path・NUL拒否、desktop／狭幅横overflow無しも実操作。`target/prepared-workbench-native-roundtrip.json`と画像を保持、所有server停止exit0／thread回収・tab閉鎖済み。削除Undoはsource reviewのみ、実mobile未検証。管理認証／DB編集／自動公開は未完成で、直前backend318／11checker sourceは維持。新画面だけを理由に旧proofを再stampしない。

> **最新要求時Wasm／pair一覧checkpoint（2026-10-29、日本時間）**：native7契約＋実HTTP9要求、成功CALL4でfresh guest counter1、拒否CALL無し・stdout空・全回収を確認。pair一覧8群（7成功／symlink1 skip）、共同Python318／312成功／symlink権限6 skip exit0、最終11checker current（`target/request-pair-list-current-check-summary.json`）。source／HTTP観測は`target/request-pair-list-checkpoint.json`、旧production／lock／binaryは維持。次のPreparedPost入力ワークベンチは新tools/下のローカル編集・downloadだけで、管理認証・DB・自動公開は追加しない。[要求時実験](wasm-request-probe-development.ja.md)。

> **最新固定Wasm公開経路checkpoint（2026-10-29、日本時間）**：native6契約・frontend7群（6成功／symlink1 skip）・実HTTP／capture比較成功、全Python303／298成功／symlink権限5 skip、最終11checker current（`target/wasm-publication-current-check-summary.json`）。固定Component HTML153bytes→既存bundle281／receipt117→別sealed serverでGET／HEAD／304／Host拒否を確認し全回収。review用`target/wasm-publication-example/`の固定2fileも生成済み。root source／lock／旧binaryは維持。[手順・限界](wasm-publication-example-development.ja.md)。続行中で要求ごとのtyped lower／lift／Store候補を調査、GET-time Wasm／実theme／認証／Linux成功はまだ主張しない。

> **最新独立Component checkpoint（2026-10-29、日本時間）**：native5契約／31観測・fmt／Clippy・実artifact PID照合、全Python296／292成功／symlink権限4 skip exit0、既存11checker current（`target/wasm-component-current-check-summary.json`）。UTF8/cap後のpost_returnとterminal cleanup失敗を検証、初回構文拒否のlogを保持。旧core／production source／root lockは維持。[実験記録](wasm-component-probe-development.ja.md)。続行して固定Component公開HTMLを既存bundle／receiptへ準備し、別sealed serverでHTTP観測する候補へ進行中。実theme／SDK／Linux／認証の完成ではない。

> **最新独立Wasm core checkpoint（2026-10-29、日本時間）**：Windows native5契約／27観測、fmt・Clippy、Cargo選択artifactの有限実行／PID照合成功。全Python290／286成功／symlink権限4 skip exit0、既存11証跡checker current（`target/wasm-core-current-check-summary.json`）。Linux cross-checkはC crosscompiler不足でexit101を保存し、環境追加せず迂回する。独立packageだけでproduction source／root lockは維持、engine／Component SDK採用は未確定。[専用記録](wasm-runtime-probe-development.ja.md)。続行中で別Component固定fixtureを調査する。

> **最新source inventory checkpoint（2026-10-29、日本時間）**：対象9群8成功・symlink権限1 skip、全Python284／280成功／4 skip exit0、既存11証跡checker current（`target/source-inventory-current-check-summary.json`）。明示sourceだけの長さ・hash／手動未確認を新規manifestへ記録し、code実行・copy・自動探索は無し。production sourceと72 API／8DB61 checkpointは維持。[独立Wasm実験](wasm-runtime-probe-development.ja.md)を続行中で、製品engine／SDK契約は未採用。

> **最新HTTP記録・比較checkpoint（2026-10-29、日本時間）**：全Python275実行・272成功・Windows symlink権限3 skip、exit0（`target/http-baseline-and-comparison-python-suite.log`）。実TLSとcapture12群／比較9群を確認、最終11証跡checker current（`target/http-tools-final-check-summary.json`）。Rust／DB／PHP sourceは無変更で、直前536＋独立29／正式72 API482,951／8DB各61を維持。privateなHTTP記録の構造比較は差があってもvalidならexit0、移行の合否ではない。外部site・画面・実移行・native Linuxは未確認。[手動移植レビューと依頼ひな形](manual-migration-review.ja.md)。続行中で次は明示sourceのoffline inventory候補。

> **最新手動HTTP baseline checkpoint（2026-10-29、日本時間）**：新しいcapture CLI／core／所有TLS helperの限定12群は11成功・symlink権限1 skip（`target/http-baseline-owned-cli-tests-final.log`、exit0）。既存11証跡checker current（`target/http-baseline-current-check-summary.json`）、Rust／DB sourceは無変更。外部site・画面・移行成功の実績ではない。[専用手順](http-baseline-development.ja.md)。続行中で、保存済み2記録の有限オフライン比較へ進む。valid入力の比較は差があってもexit0で、合否判定はしない。

> **最新truncate-slug／post-name pipeline checkpoint（2026-10-29、日本時間）**：`target/truncate-slug-and-post-name-pipeline-full-check.log` exit0。Rust536＋独立29（ignore1）／Python254（253成功・1skip）／正式PHP72 API482,951／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB61、Clippy・server/WP Linux check・license215観測・HTTP成功。最終11checker current（`target/truncate-slug-current-check-summary.json`）、旧71 input/result SHA保持、全DB回収。_truncateはraw任意Bytesとchanged decoded UTF8枝を区別、Query→SQL結合はdev-onlyで公開判断無し。次はR54の1URL HTTP記録candidate、画面／自動URL抽出／移行成功判定・native Linuxは未実装。

> **最新sanitize-title／post-name checkpoint（2026-10-29、日本時間）**：`target/sanitize-title-and-post-name-retry-full-check.log` exit0。Rust529＋独立29（ignore1）／Python248（247成功・1skip）／正式PHP71 API466,887／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB60、Clippy・server/WP Linux check・license215観測・HTTP成功。最終11checker current（`target/sanitize-title-current-check-summary.json`）、旧70 input/result SHA保持、全DB回収・normal依存境界維持。初回のpostmeta baseline11誤期待をcall順に沿う6へ訂正し、全行Eq復旧は維持して全gate再測定した。編集用name/type readerは生の全23fieldを返し、公開条件を採用しない。unique slug／投稿作成／認証／native Linux等は未完成。

> **最新URI/title checkpoint（2026-10-29、日本時間）**：`target/uri-title-and-credential-pipeline-full-check.log` exit0。Rust520＋独立29（ignore1）／Python241（240成功・1skip）／正式PHP70 API461,522／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB57、Clippy・server/WP Linux check・license215観測・HTTP成功。最終11checker current（`target/uri-title-current-check-summary.json`）、全DB回収済。旧68 API453,064の件数・入力と結果SHAは実prefix照合で保持。URI encoded-lengthとfinal cap、title URI200前処理順・sentinel衝突・NoMB profileを限定して実装。次はouter sanitize_title composition候補、slug一意性・新規投稿・認証・native Linuxは未完成。

> **最新Linux helper checkpoint（2026-10-29、日本時間）**：限定`check_native_linux.py`を追加。Windowsでplan成功・非Linux guard拒否を実確認、mock7件成功。Linux native本体／select captureは未実行。Cargo実artifact束縛・host固定・Rust/HTTP/Unix signal/選択8Pythonの準備のみで、環境追加・CI・配備は無し。production source不変、最終11checker current（`target/native-linux-helper-current-check-summary.json`）。直前package gateのRust504／Python228（1skip）とDB57／正式PHP68 API453,064を維持。[専用手順](native-linux-validation.ja.md)。

> **最新offline package checkpoint（2026-10-29、日本時間）**：`target/offline-post-package-full-check.log` exit0。Rust504（ignore1）／Python228実行（227成功・symlink権限不足1 skip）、Clippy・server/WP Linux check・license215・HTTP、fresh alpha DB2構成各57を通過。他6DB／独立29は直前実測をsource一致で維持。比較binaryの再build後に生成・legacy・stageを実再測定、最終11checker全exit0（`target/post-package-current-check-summary.json`）。正式68 API453,064はsource不変でcurrent。`prepare-post-package <input> <new-dir>`と実例を追加、固定bundle/receipt2file・既存非上書き・partial保持。認証／Wasm／native Linux実行／live公開は未完了。

> **最新credential stage checkpoint（2026-10-29、日本時間）**：`target/credential-stage-and-pipeline-full-check.log` exit0。Rust498＋独立29／Python222／PHP68 API・453,064／hooks572／dispatcher405、生成10・legacy113・別枠stage33（actual25/refusal8）、全8DB57、Clippy・server/WP Linux check・license215・HTTP成功。最終11checkerもcurrent（`target/credential-stage-current-check-summary.json`）、全DB回収済。username/emailの未確定raw candidateまでで、principal/session/HTTPloginは未接続。次は既存明示入力からbundle/receiptのoffline packageを作る候補。raw INSERTはWP API draft既定とDDL publish既定の差、slug/date/hooks等を残すため今回採用しない。

> **最新username prefix checkpoint（2026-10-29、日本時間）**：`target/username-preprocessing-and-credential-pipeline-full-check.log` exit0。Rust490＋独立29／Python214／PHP68 API・453,064／hooks572／dispatcher405、生成10・legacy113、全8DB56、Clippy・server/WP Linux check・license215・HTTP成功。最終全10checkerもcurrent、`target/username-current-check-summary.json`。全DB回収済。NoLiteral<／post-stripUTF8／noIntl／empty callbacksの限定componentで、principal/session/HTTPloginではない。継続してusername成功優先・確定mismatch時email fallbackのpure未確定candidate stageへ進む。

> **継続実装中**：次のNoLiteral `<` strip／sanitize_user／S1-P1-S2 prefixとDB有限結合check56へ進行。source変更中なので直下のAPI66・DB55成功を現変更後へ読み替えない。正式PHP・生成／legacy・全8 fixture再実測はsource freeze後。認証session／principal domain／cache-action／HTTPは未接続。

> **最新remove_accents checkpoint（2026-10-29、日本時間）**：`target/remove-accents-and-credential-pipeline-full-check.log` exit0、Rust477＋独立29／Python207／PHP66 API・443,078／hooks572／dispatcher405／生成10／legacy113／8DB各55／Clippy・server/WP Linux check・license215・HTTP成功。最終全checkerとpinned表--checkも成功、全DB回収、`target/remove-accents-current-check-summary.json`へ実行要約。継続中で次はNoLiteral<前処理とsanitize_user候補。S1→S2二回sanitize・passwordtrim・email成功fallback・IDint化などのsource差分を[入口メモ](authentication-entry-development.ja.md)へ記録、session／HTTP認証・policyは未採用。

> **最新credential pipeline checkpoint（2026-10-29、日本時間）**：Rust471＋独立29／Python200、PHP65 API・439,380／hooks572／dispatcher405、生成10・legacy113（actual88/refusal25）、8 fixture各55、Clippy・server/WP Linux check・license215・HTTP成功。DBdev-only WP結合のsourcebindingを強化して全8 fixtureを再実測し、全10現証跡checkerもexit0。build後のproducerbinary変更はhashの書換えで済ませず、生成／legacy／正式PHP3比較を実再実行。ログは`target/credential-pipeline-*`、要約は`target/credential-pipeline-current-check-summary.json`。全一時DB回収済み。継続中で次はusername用文字変換候補、認証session・HTTP・cache/action・自動rehash policyは未接続。

> **最新raw dispatcher checkpoint（2026-10-29、日本時間）**：`target/password-dispatch-workspace-reference-check.log` exit0、Rust464／Python195／正式PHP64 API437,198／hooks572／dispatcher405、生成相互照合・legacy113（actual88/refusal25）・Clippy・server/WP Linux check・license215・HTTP通過。今回はDB起動／独立再試験無し、全DB/native/alpha checkerでcurrentを確認し直前29件／8 fixture各48を維持。db-runs空。次はrehash要否／別pool raw credential CAS候補で継続する。[raw dispatcher](password-dispatch-development.ja.md)。

> **最新検証済みlegacy／lookup checkpoint（2026-10-29、日本時間）**：`target/legacy-password-and-user-lookup-full-check.log` exit0。workspace Rust458＋独立29／Python194、正式PHP64 API・437,198件／hooks572／dispatcher405、生成10ダミー相互照合各24、別枠legacy64（actual50／refusal14）、全8 fixture各48、Clippy・server/WP Linux check・license215・HTTP通過。全fixture回収・db-runs空・公開依存分離再確認済。MySQL metadata signed→unsigned castとMaria NUL期待を修正し失敗logも保持。停止ではなく継続中、次はraw password dispatcher。以下の「現在は継続実装中」はこのcheckpoint前の履歴。

> **現在は継続実装中**：次の旧hash検証／user lookupのためsourceを変更中。新依存md-5.11.0＋既存subtle、inventory unique215／license210 archive・377 fileは更新済み。予定DB契約48へcheckerを更新中で、実fixture再生成は後続。以下のgeneration checkpointは変更前の検証済み履歴であり、現変更後の全体成功を先取りしない。

> **最新検証済みgeneration checkpoint（2026-10-29、日本時間）**：`target/wp-password-generation-and-alpha-full-check.log`がexit0。workspace Rust449＋独立29、Python188、正式PHP64 API・437,198件／hooks572／dispatcher405、別枠の生成10ダミー相互照合各24回、8 fixture各45項目、Clippy・server/WP Linux check・license214・HTTPを通過し全fixtureを回収。新依存追加無し、生成を正式API比較件数へ加算しない。継続中で次は旧WP hash／editing-only user lookup候補。認証session・HTTPlogin・DBへのcredential保存・Wasm・native Linux実行は未実装／未検証。[生成記録](wp-password-generation-development.ja.md)。

> **最新検証済みcheckpoint（2026-10-29、日本時間）**：`target/wp-bcrypt-and-alpha-publication-full-check.log`がexit0。workspace Rust443＋独立29／Python182、実PHP64 API・437,198比較／hooks572／dispatcher405、8 fixture各45項目、Clippy・server Linux cross-check・license214・実HTTPを通過。別途WP crateのLinux all-target checkも成功。全fixture回収、`.reference/db-runs`空。commit／push／外部配備無し。停止指示ではなく継続中で、次はWP既定password生成候補。Dockerは静的候補のみ、native Linuxは未実行。保存から配信までの有限経路は現在DBへ同期する製品公開ではない。[開発記録](development.ja.md)。

> 2026-10-29作業中：保存CAS→固定公開snapshot→receipt→別sealed HTTPの初回full gateはRust437＋独立29／Python170／8 fixture各45項目で成功。その後のWP bcrypt候補・HTTP子回収変更に合わせ、実PHPと全8 fixtureを含むcurrent source再検証へ進む。新依存はunique214、license実観測も更新済み。公開用固定sampleとCLI手順、Docker/Composeの静的候補を追加した。Docker/native Linuxは未検証。[現作業記録](development.ja.md)。

> **再開中（2026-10-29、日本時間）**。ユーザーの「続行。サブエージェントは4本使ってよい」で停止解除。この環境はroot含む同時4枠のため、subagent3本を並行使用する。α版に向け、既存投稿textのCAS保存、明示公開準備済み記事の固定renderer、既存bundleのbyte identity receiptと別sealed processへの受渡しを接続している。認証503・Wasm未接続を維持し、rawWP保存／公開policy／live差替えの完成とは扱わない。以下の停止checkpointは直前の履歴。

> **停止中（2026-10-27、日本時間）**。ユーザー指示「キリの良いところで停止」に従い、全gateが完了した区切りで停止。再開指示まで新規作業を開始しない。両subagent完了・実行session回収・専用DB directory空、commit／push／外部配備無し。
>
> 最終gateはtarget/empty-filter-escaping-and-memory-diagnostic-workspace-check.log。実PHP63 API／436,656比較・hooks572／dispatcher405、workspace419 Rust・Python155、HTTP・Clippy・Linux cross-check・license208・DB current hashが成功。全native current hashも通過し、DB live再実行無しで独立29件・6 fixture各39項目を維持。empty-filter esc2API各1,938件、memory CLIのWindows全Unsupported／exit3と独立レビューも確認。native Linux実測・自動容量・監視・CMS連続公開は未完成。
>
> α版の機能面の目安は約25％（20〜30％）。認証→記事編集／保存→テーマ付き公開をLinuxで試せる版を仮の基準とする。次回は部品追加の数より、管理編集→保存→公開の導線をつなぐ候補を優先して検討する。Wasm engine／ABI、認証の未決事項は推測で採用しない。[最終開発記録](development.ja.md)。

> 最新specialchars／bundle検証gate（2026-10-27、日本時間）：target/specialchars-and-bundle-check-workspace-check.logが成功。61 API／432,780 PHP比較・hooks572／dispatcher405、workspace411 Rust・Python149、HTTP・Clippy・Linux cross-check・license208・DB current hashを通過。新helper23,256件は6styles×2boolを各1,938件で拘束し、conditional UTF8／HTML401・XML5とPHPscope復元を確認。check-bundleは既存admissionだけを共有するone-shotで、bodyを出さず、3sample集計も一致。全native current hashも通過し、DB live再実行無しで独立29件・6 fixture各39項目を維持。esc_html／esc_attr・任意filter・公開可否や完全画像／XML検証は未完成。[特殊文字](specialchars-development.ja.md)・[bundle検証](bundle-check-development.ja.md)。

> 最新XML／コメントfeed gate（2026-10-27、日本時間）：target/xml-utf8-and-comment-feeds-workspace-check.logが成功。固定PHP60 API／409,524ケース・hooks572／dispatcher405、workspace402 Rust・Python141、HTTP・Clippy・Linux cross-check・license208・DB current hashを通過。normalizerはHTML6,527／XML7,544の別profileでAPI60を維持。PHP charset／2globals復元、コメント出力／Rendering ownerと旧記事bytes維持を独立レビューし、固定sample HTTPも回収済み。全native current hashも別途成功し、DB live再実行無しで独立29件・6 fixture各39項目を維持。製品のコメント公開選別／PII／live publicationや一般XML安全性は未完成。[XML](xml-entity-normalization-development.ja.md)・[コメントfeed](comment-feeds-development.ja.md)。

> 最新process RSS観測gate（2026-10-27、日本時間）：target/process-rss-observation-workspace-check.logでworkspace393 Rust・Python133、Clippy・Linux cross-check・license208・実HTTP・DB current hashを確認。WPと全nativeも別途current hashを照合し、PHP／DB live再実行無しで60 API／401,980ケース・hooks572／dispatcher405、独立29件・6 fixture各39項目を維持。固定self smaps_rollupの4契約であり、native Linux実読・監視・自動容量・CLI/cache接続は未完成。[process RSS](process-memory-development.ja.md)。

> 最新outer protocol／MySQL server再起動全gate（2026-10-27、日本時間）：target/protocol-outer-reference-workspace-check.logで固定PHP60 API・401,980ケース（outer helper7,768追加）／hooks572／dispatcher405が成功。target/protocol-outer-and-mysql-server-restart-full-check.logでworkspace Rust389件（通常ignore1）＋独立29件・Python133件、全6 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPを通過し全fixture回収。WP current hashもtarget/protocol-outer-server-restart-final-wp-provenance.logで最終照合。専用owned MySQLはCOMMIT前server終了→pending child回収→初期化SQL無しの同datadir再起動→old全tuple／source T2／index照合で、binlog再生・製品CDC・OS／電源断保証ではない。fixture helperの無期限waitとambient例外誤認を修正し、失敗時handle保持・期限付きIO・外側再回収も独立レビューした。途中gateの旧DB hash拒否は履歴として残し、実測で全証跡を更新した。[outer helper](bad-protocol-development.ja.md)・[server再起動](mysql-server-restart-development.ja.md)。

> 最新protocol once／ETag全gate（2026-10-27、日本時間）：target/bad-protocol-once-reference-check.logで固定PHP59 API・394,212ケース（once helper7,347追加）／hooks572／dispatcher405が成功。target/protocol-once-and-etag-full-check.logでworkspace Rust381件（通常ignore1）＋独立27件・Python113件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・ETag実HTTPも通過し全fixture回収。WP hashもtarget/protocol-once-etag-final-wp-provenance.logで最終照合。新しいserver依存辺sha2 0.10.9(default feature無し)は既存packageを再利用し、155／188／unique208は不変、lock変更後のDB証跡は実測で再生成した。ETagは同じsealed readerのstartup hashで、IfMatch→IfNone→IfRange順。304はアプリでfull長、実GET wireは省略／HEADはfull長という差を観測・記録し、非空表現のCL0は不許可。body/MIME/statusの独立hash一致・process間安定も確認。新たな監視／live公開／認可は未採用。[once](bad-protocol-once-development.ja.md)・[ETag](etag-development.ja.md)。


> 最新protocol helper／byte Range gate（2026-10-27、日本時間）：target/bad-protocol-once2-reference-check.logで固定PHP58 API・386,865ケース（once2 helper8,158追加）／hooks572／dispatcher405が成功。target/bad-protocol-and-byte-range-check.logでworkspace Rust360件（通常ignore1）・Python112件、Clippy・Linux cross-check・license208観測、実HTTPの206／416／HEAD／redirect維持、DBと全nativeのcurrent hash照合も通過。今回はDB live／独立experimentを再実行せず、直前27件・5 fixture各39項目・全回収の実測を維持する。単一RangeはGET200非空のsealed bytesだけで、部分copyは未課金・cachebypass。条件header／multipart／空本文等は従来応答を保つ。C protocol helperはdecode→white除去→no-null→ASCII lowerとByte値allowlistの限定移植で、URL認可／safetyをclaimしない。サンプルの実PNG部分取得も別processで照合・回収済み。[once2](bad-protocol-once2-development.ja.md)・[byte Range](byte-range-development.ja.md)。

> 最新KSES slash／typed redirect gate（2026-10-27、日本時間）：target/kses-slashes-reference-check.logで固定PHP57 API・378,707ケース（wp_kses_stripslashes4,364追加）／hooks572／dispatcher405が成功。target/kses-slashes-and-typed-redirect-full-check.logでworkspace Rust344件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・binary/redirect実process HTTPを通過し全fixtureを回収した。WP current hashもtarget/kses-slashes-typed-redirect-final-wp-provenance.logで最終照合。typed LocationはASCII root-relative subset、5 status、空body、単一headerで、cacheを通らず転送先を内部取得しない。版1・旧constructorは不変、外部redirect／WP redirect互換・loop検証・本番認可は未完成。サンプルの308と画像/textも実GET/HEADで照合し回収済み。[KSES slash](kses-slashes-development.ja.md)・[typed redirect](typed-redirect-development.ja.md)。

> 最新control除去／binary bundle gate（2026-10-27、日本時間）：target/no-null-reference-check.logで固定PHP56 API・374,343ケース（wp_kses_no_null8,324追加）／hooks572／dispatcher405が成功。target/no-null-and-binary-bundle-full-check.logでworkspace Rust332件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・版2を含む実HTTP process smokeが通過し、全fixtureを停止・回収した。target/no-null-binary-bundle-final-wp-provenance.logでWP current hashも最終照合。版1は維持し、版2はexplicit body_hex・textUTF8・PNG/JPEG/WebPの宣言署名だけを扱う。新しいfilesystem／外部fetch・PHP／DB経路は追加しない。サンプル70byte PNGはchunk/CRC/zlibと実HTTP GET/HEADを別途確認し、ユーザーassets/logoは不変。完全画像decoder・metadata除去・本番公開判定、製品CDC・自動容量・native Linuxは未完了。[control除去](no-null-development.ja.md)・[binary bundle](binary-bundle-development.ja.md)。

> 最新byte decode／MySQL切替再試行 checkpoint（2026-10-27、日本時間）：target/byte-entity-decode-reference-check.logで固定PHP55 API・366,019ケース（数値参照decode7,646追加）／hooks572／dispatcher405が成功。target/byte-entity-decode-and-rotation-retry-workspace-check.logでworkspace Rust319件（通常ignore1）・Clippy・Linux cross-check・HTTP・license208観測を通過し、target/byte-entity-decode-and-rotation-retry-native-check.logで独立Rust27件・Python112件・既存4 fixture各39項目・全current provenanceも成功した。別のtarget/mysql-rotation-owned-cut-second-live-check.logでは専用fixture39項目と厳密39 fieldsが通過。実Rotate後に所有dumpだけを終了し、旧C1の行・索引・checkpoint／poolを保持して新接続から再試行した。全5 fixtureを回収済み。初期FDEのend0と物理FDEのend127を区別し、失敗履歴も保持する。通常decoder／coreは不変、journal・source再起動／保持保証・連続CDC／公開は未完成。[文字実体](entity-normalization-development.ja.md)・[切替再試行](mysql-rotation-handoff-development.ja.md)。


> 最新Unicode／HTML実体・MySQL切替 gate（2026-10-27、日本時間）：target/unicode-and-default-html-entities-reference-check.logで固定PHP54 API・358,373ケース（valid_unicode3,171／HTML実体正規化6,527追加）・hooks572／dispatcher405が成功。target/unicode-entities-and-mysql-rotation-full-check.logでworkspace Rust313件（通常ignore1）＋独立experiment25件・Python111件、既存4 fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測を通過した。別の専用MySQL切替liveも39項目成功し、合計5 fixtureを回収。target/unicode-entities-rotation-final-provenance-check.logでWP／全nativeのcurrent hashを最終照合した。新候補は実Rotate後の人工通知を専用phaseで検証し、headerでは旧checkpointを維持、file2の成功COMMITだけでRAM viewとfile付き座標を切り替える。通常decoder／coreは不変。本番rotation・journal／source再起動・連続CDC・公開は未完成。HTMLは固定253名称のみでXML／sanitizer未対応。[文字実体](entity-normalization-development.ja.md)・[切替候補](mysql-rotation-handoff-development.ja.md)。


> 最新zeroise／byte量 gate（2026-10-27、日本時間）：target/zeroise-and-bounded-hr-bytes-check.logが成功。workspace Rust306件（通常ignore1）・Python100件、固定PHP比較52 API・348,675ケース（zeroise2,769／wp_convert_hr_to_bytes2,821追加）／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。binlog／MySQL overlap／Maria bootstrapも別途照合し、DB live／独立experiment再実行なしで直前21件・4 fixture各39項目を維持する。zeroiseは%sの符号前0pad／rawBytesとhostwidth4096・出力cap、byte変換はdefault1024units・非負decimal有効256桁までの明示subset。無制限飽和の7反例(actual0)を保存してdomainを狭め、範囲外42 process拒否を別観測にした。共通int wireもsigned64厳密検証へ補修し、nestedkey含む3契約を追加。[対応範囲](compatibility-development.ja.md)。

> 最新request hint gate（2026-10-27、日本時間）：target/request-media-hints-check.logが成功。workspace Rust299件（通常ignore1）・Python96件、固定PHP比較50 API・343,085ケース（JSON／XML要求各3,966追加）／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。binlog／MySQL overlap／Maria bootstrapも別途照合し、DB live／独立experiment再実行なしで直前の21件・4 fixture各39項目を維持。明示Option headerのstateless kernelはNone／empty／case／params／NUL／不正byteと、JSON Accept先の短絡・XML Accept substring対Content-Type exactを保持する。SERVER2fieldの完全復元、PCRE first-header error検査、型拒否も別process観測した。q値交渉・本文parse・認可・製品HTTP接続は追加しない。[対応範囲](compatibility-development.ja.md)。

> 最新JSON media C-profile gate（2026-10-27、日本時間）：target/json-media-type-c-profile-check.logが成功。workspace Rust295件（通常ignore1）・Python96件、固定PHP比較48 API・335,153ケース（wp_is_json_media_type 7,072追加）／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。binlog／MySQL overlap／Maria bootstrapも別途current hashを照合し、DB live／独立experiment再実行なしで直前の21件・4 fixture各39項目を維持する。新predicateはC localeのbyte入力・return boolだけを扱い、元PCREのunanchored境界・特殊ASCII range・optional oembedを線形・追加割当なしで維持する。PCRE errorを成功boolへ混ぜずoracleで拒否し、nonstring／非C／memoization／resource failureは未claim。MIME解析・HTTP認可へ接続しない。[対応範囲](compatibility-development.ja.md)。

> 最新charset／UTF8 gate（2026-10-27、日本時間）：target/charset-and-invalid-utf8-profile-check.logが成功。workspace Rust291件（通常ignore1）・Python96件、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを通過。47 API・328,081ケース（is_utf8_charset 8,936／wp_check_invalid_utf8 8,782）／hooks572／dispatcher405が固定PHPと一致。review後oracleのbyte-string guardを追加し、target/charset-and-invalid-utf8-oracle-domain-check.logでPHP3比較を実再生成してcurrent hashを照合した。非string56 process拒否とbyte positive21ケースも別観測で確認し正式件数へ加算しない。今回はDB live／独立experiment再実行なし、前回の独立21件・4 fixture各39項目・全回収を維持し、binlog／MySQL overlap／Maria bootstrap current hashも確認。Null／省略option・scalar変換／非bool strip・他charset／mbstring・動的site設定は未対応。[対応範囲](compatibility-development.ja.md)。

> 最新Maria bootstrap全gate（2026-10-27、日本時間）：target/mariadb-snapshot-bootstrap-full-check.logが成功。workspace Rust287件（通常ignore1）＋独立experiment21件・Python96件、通常MariaDB／MySQL・専用MySQL GTID・専用MariaDBの4 fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測・全current provenanceを通過し全4 fixtureを回収。新Maria候補はsame-session anchor A=4246641→bootstrap3 COMMIT C=4248068→strict1 COMMIT D=4248466、snapshot／最終11行、共有pool callback4と返却、cap／pool／CRC／gap拒否を実測。別途WP hash確認済みでPHP再実行なし、45 API・310,363ケース／hooks572／dispatcher405を維持する。FTWRL／global pauseを追加せず、journal／source再起動／連続CDC／公開／本番bootstrapは未採用。[Maria候補の範囲](mariadb-snapshot-bootstrap-development.ja.md)。


> 最新HTTP owner gate（2026-10-27、日本時間）：target/budgeted-http-cache-owner-check.logが成功。workspace Rust287件（通常ignore1）・Python89件、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認し、WP／binlog／MySQL overlapも別途current hashを照合した。PHP／DB live／独立experimentは再実行せず、WP45 API・310,363ケース／hooks572／dispatcher405、前回の独立19件・3 fixture各39項目を維持。明示public_router_with_budgeted_cacheだけでcache handleをBytes ownerへ渡し、nonempty Body／Bytes clone／sliceの実owner寿命まで予約を保持する。RenderingとResponseCacheの同pool競合・独立解放も実API同士で確認。empty最適化はowner早期解放可、fallback／準備済み本文／transport等は未課金。既存constructor／CLIはlegacy・default停止、sealed generation・撤回非保証は維持。[HTTP cache開発記録](http-cache-development.ja.md)。

> 最新共有cache全gate（2026-10-27、日本時間）：target/budgeted-cache-shared-pool-full-check.logが成功。workspace Rust276件（通常ignore1）＋独立experiment19件・Python89件、通常MariaDB／MySQLと専用GTID MySQL各39項目、cold restore／kill／filter／共有pool／overlap、HTTP・Clippy・Linux cross-check・license208観測・current provenanceを通過し全3 fixtureを回収。WP source不変のためPHP再実行なしで45 API・310,363ケース／hooks572／dispatcher405を維持し、別途target/budgeted-cache-wp-provenance-check.logでWP証跡を照合した。BudgetedPageCacheは外部本文handleの最終dropまで全entry予約を維持し、global拒否で旧値／LRUを保つ。新lease slotのEntry header増分を既存logical chargeへ反映した。HTTP／CLIは従来backendを維持し、cache停止・自動算式・全RSS／cross-processは未対応。[cache開発記録](http-cache-development.ja.md)。

> 最新feed出力gate（2026-10-27、日本時間）：target/budgeted-feed-output-final-check.logが成功。workspace Rust266件（通常ignore1）・Python89件（独立XML5）、HTTP・Clippy・Linux向けcross-check・license208観測・DB current hash検査が通過。別途WP／binlog／MySQL overlapのcurrent hash照合も成功。WP source不変のためPHP再実行なしで45 API・310,363ケース／hooks572／dispatcher405を維持し、DB live／独立experimentも前回の19件・3 fixture各39項目を維持する。render_budgetedは事前生成するRSS／Atomの出力len＋owner headerだけを同一poolのRendering予約へ結び、Arc最終dropまで維持。入力／一時領域／HTTPコピー／RSS・自動容量・CLI cache接続は対象外。[feed開発記録](feeds-development.ja.md)。

> 最新JSON profile gate（2026-10-27、日本時間）：target/json-utf8-fallback-profile-check.logが成功。workspace Rust256件（通常ignore1）・Python88件、固定PHP比較45 API・310,363ケース（JSON23,365＝従来17,073＋明示UTF8/no-mbstring6,292）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check・license208観測とDB current hash検査を通過した。別途binlog／MySQL overlapのcurrent hash検査も成功。今回はDB live／独立experimentを再実行せず、前回の独立19件・3 fixture各39項目・全回収を維持する。新wp_json_encode_utf8_no_mbstringだけが固定profileのwhole-graph補修とキー衝突を扱い、既存2 APIのinvalid UTF8拒否は変えない。元graph全域の未対応値／64段を先に検査し、補修cloneのメモリは出力cap／共有pool対象外。Floatは反例と設定依存を記録して未対応を維持。[JSON開発記録](json-encoding-development.ja.md)。

> 最新phase3全gate（2026-10-27、日本時間）：target/shared-buffer-and-pretty-json-full-check.logが成功。workspace Rust251件（通常ignore1）＋独立experiment19件・Python86件、固定PHP比較45 API・304,071ケース（JSON17,073・9flags全512組合せ）／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208観測が通過し全3 fixtureを回収。両nativeのpostmeta_shared_pool_commits=3／postmeta_shared_pool_refusal_preserved=true／postmeta_shared_pool_released=trueも照合した。postmetaだけ行・索引・raw保持を一poolへ接続し、他ProbeBufferは専用pool。callback失敗／cancelではlease保持、commit成功後の解放をErrへ変換せず、pool0はRow受理前の型付き拒否と旧checkpointを確認する。decoder／input／比較コピー／position／outer Vec等、全RSS・cross-process・自動算式・CLI cacheは未対応。[開発記録](budgeted-change-buffer-development.ja.md)。旧238／230等は履歴として保持。

> 直前phase2の履歴（2026-10-27、日本時間）：target/budgeted-meta-view-and-loader-cache-final-check.logが成功。workspace Rust238件（通常ignore1）＋独立experiment19件・Python86件、固定PHP再比較45 API・299,957ケース／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208 package観測、cargo graph155／188を確認。今回PHP再比較とDB live実測を行い全3 fixtureを回収した。BudgetedPostMetaViewは同一poolの行／索引予約をdata寿命へ結ぶ候補で、old＋new／Arc最終drop、Budget error時保持と明示capのScanを区別する。全RSS・node／capacity／driver／ref map／peak・cross-process・自動算式・CLI cacheは未対応。初回full gateのschema helper旧hashによる終盤失敗、DDL同一の再生成と起動前tool_files guard修正を[開発記録](budgeted-meta-view-development.ja.md)に保持する。MySQL overlap prefix134は今回の追加2 writeによる観測で、旧132を上書きしない。

> 直前の検証履歴（2026-10-27、日本時間）：target/json-selected-flags-full-check.logが成功。workspace Rust230件（通常ignore1）・Python79件、固定PHP比較45 API・299,957ケース（wp_json_encode 12,959件、うち新flags付き7,656件）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check・dependency license208 package観測・DB／binlog／overlap current hash検査を通過した。8種類のJSON flagsの全256組合せを比較し、PHP定数の型・値と組合せ別件数の完全性も検査する。今回live DB／独立experimentは再実行せず、前回の独立19件・3 fixture各36項目の実測を保持。native Linux、製品CDC、自動容量算式の完成を示さない。
>
> JSON flagsは本体へ統合・検証済み。前回停止時の「未統合」は履歴で、現在の再開状態・対応範囲は[JSON開発記録](json-encoding-development.ja.md)を参照する。

> **再開中（2026-10-27、日本時間）**：新しいユーザー指示「再開」により停止を解除。JSON selected flags／PRETTY／明示UTF8 profileの実装・固定PHP比較を終え、共有予算への追加接続候補を調べている。サブエージェント最大2本、未決の採用判断は変更しない。
>
> **前回停止時の記録（2026-10-27、日本時間）**：ユーザーの「切りのいいところで停止」に従い、default JSON全gate成功と本引継ぎ保存を区切りに停止。再開は新しいユーザー指示後。過去の「ノンストップ」指示から継続・自動再開しない。稼働fixture0、commit／push／配備なし。
>
> 停止時の最新検証：target/json-encoding-and-license-check.logでworkspace Rust224件（通常ignore1）・Python77件、PHP再比較45 API・292,301ケース（wp_json_encode 5,303追加）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check、dependency license208 package観測とDB／binlog／overlap証跡current hash確認が成功。今回は実PHPを再実行し、DB live／独立experimentは再実行していない。直前target/limited-index-native-check.logの独立19件・通常MariaDB／MySQLと専用GTID MySQL各36項目・全3 fixture回収を維持する。製品bootstrap／連続CDC・native Linux・容量自動算式・法的判断等の残件は維持する。
>
> 再開候補：JSON本体はdefault flags=0のみ。selected flagsはtarget/json-selected-flags-observation.php／.jsonの1,792観測だけで、wpagentの拡張draft未作成・未統合。正式source／registry／proofへ採用していない。default JSON探索76ケースはtarget/json-encode-observation.php／.jsonlにある。新しい指示なく次のflags実装へ進まない。

> 直前の検証履歴（2026-10-27、日本時間）：target/limited-index-native-check.logで独立experiment19件・Clippy、通常MariaDB／MySQLと専用GTID MySQL各36項目、cold restore／kill／filter／limited changes・index各3 COMMIT／overlapが成功し全3 fixtureを回収。別のtarget/limited-index-and-license-check.logでworkspace Rust219件（通常ignore1）・Python77件、Clippy・Linux向けcross-check・HTTP、dependency license208 package観測、DB／binlog／overlap／WP証跡current hash確認が成功。PHP再実行なしでWP44 API・286,998ケース／フック572／dispatcher405を維持する。licenseは203 locked archive／364 cache file一致、checksum JSON missing203・候補なし4・local5を記録した観測で、法的判断未完了。新索引APIの上限は最終logical chargeだけでnode／capacity／rows／peak／RSS／shared budgetを含まない。以下の214／209／194件のgateは履歴として保持する。

> 外部フォーム推奨の追加決定（2026-10-26、D23-01）：Tallyを第一推奨、独自デザイン向けにWeb3Formsを代替候補とする。プラグイン不要のリンク・埋込み・外部送信への置換を案内し、本体は受付の中継・フォーム用SMTP・配送・問い合わせデータ保管を持たない。契約・設定・データ管理は利用者が担う。R47と[外部フォーム推奨](external-forms.ja.md)へ反映。導入検証は未実施、既存問い合わせデータの扱い・コメント通知基盤は未決。

> 最新の設計決定（2026-10-26）：D07-02の[DB自動最適化・変更検知](database-auto-optimization.ja.md)を承認。詳細設計v0.44、確定要求R01～R61、受入試験計画T01～T44。製品用の内部索引・連続CDCは未完了。部分実装と専用fixture試験の最新結果は下記と開発メモを参照。DB互換性と公開境界を維持し、D08の直接SQL許可は未採用のまま。以下のv0.43・R58・T43は追加決定前の記録。

> 最新の実装（2026-10-27）：DBと非DBの継続開発中。単一サイト12表・両DB各36項目、有限binlog、postmetaの原子保存・再送・SQL／購読再接続を確認。producerの正常終了後に別OSプロセスで同一稼働DBからcold restoreし、5種の拒否と最終正常復元も両DBで確認した。厳密な数値metadataと所有process lifecycle検査を追加。単一process・6分類の共有論理予算poolは単独候補として試験済みで、cache／索引／CDCへ未接続。実監視・容量算式・cross-process調整は未実装。WP44 API・286,998ケース、フック572／dispatcher405。normalize_whitespaceはPHP trimの6byte→CR変換→LF／space・TABのrun縮約、wp_html_splitは空capture・不正区切り・引用符内の最初の>も保持するregex準拠のbyte分割。両者はUnicode正規化・HTML parser・出力escapingではない。JSONPはC locale限定、listのroot bool／signed integerを追加、NullはPHP 8.5のE_DEPRECATED 8192の診断対応待ちでUnsupported。commit／push／配備なし。 postmetaの新しいbyte上限付きloaderは同じsnapshotで全表の長さだけを先読みし、適合後に本文を取得する。論理chargeは行構造体とkey/value長で、実RSSや自動容量算式・共有poolとの接続を完成させたものではない。HTML内置換もbyte入力の対応範囲を追加。 MySQLの独立した非公開overlap bootstrapも専用fresh GTID fixtureで実測し、132 prefix／3 bootstrap／1 strict／12行のraw・索引一致を確認。非prefix Aは共通runtime selectorを通す純粋試験で、製品bootstrap・連続CDCの完成ではない。
>
> 直前全gateの履歴（2026-10-27、日本時間）：target/limited-meta-changes-full-check.logでworkspace Rust214件（通常ignore1）＋独立experiment19件・Python68件、通常MariaDB／MySQLと専用GTID MySQL各36項目、cold restore／pre-COMMIT kill／選別／上限0拒否と3 COMMIT／overlap、HTTP・Clippy・Linux向けcross-check・registry current hash検査が成功。両DBのpostmeta_logical_limit_preserved=true／postmeta_limited_commits=3を再確認し全3 fixtureを回収。WP44 API・286,998ケース／フック572／dispatcher405はcurrent hash検査で維持しPHP再実行なし。新prepare_changes_with_limitsは全strict検証と最終行数／論理bytes検査の後にだけpayloadをcloneし、旧view／索引を維持する。old/new同時保持・changes・参照map・index・capacity・RSS／peak・共有予算や自動算式を計上したものではない。[DB開発記録](database-development.ja.md)。
>
> 直前WP gateの履歴（2026-10-27、日本時間）：target/unique-id-and-visible-cgroup-check.logでworkspace Rust209件（通常ignore1）・Python68件、WP44 API・286,998ケース／フック572／dispatcher405の実PHP比較、HTTP・Clippy・Linux向けcross-check、通常binlog／MySQL overlapのcurrent hash検査が成功。UniqueIdsの2 APIを各2,069ケース追加し、可視cgroup観測候補の新unit5件も含む。明示request／execution context内で全体・prefix別counterは独立し、PHP staticとnative contextの寿命をbatch単位で同期する。byte prefix／i64範囲のみで、連結IDの衝突可能性を保持しsecurity保証はしない。非string診断・float overflow・request lifecycle／budget統合・Wasmは未対応。このrunはDB／独立experiment再実行なし。
>
> 直前DB gateの履歴：`target/mysql-overlap-bootstrap-full-check.log`が成功。workspace Rust194件（通常ignore1）＋独立experiment19件・Python68件、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、cold restore／kill／同DB選別／overlap候補、HTTP・Clippy・Linux向けcross-check、registry current hash照合を確認。WP41 API・274,683ケース／フック572／dispatcher405の既存比較証跡を維持し、全3 fixtureは停止・回収済み。非公開RAM構成・通常strictへ切替える限定実験であり、永続化・source identity・保持／rotation・本番bootstrapの残件は維持する。[専用開発記録](mysql-overlap-bootstrap-development.ja.md)。
>
> 直前の非DB検証履歴：target/locale-mime-and-filtered-postmeta-check.logでworkspace Rust194件（通常ignore1）・Python61件、WP41 API・274,683ケース／フック572／dispatcher405の実PHP比較、HTTP・Clippy・Linux向けcross-check、DB／binlog current hash検査が成功。locale／MIMEの2 byte APIを各4,095ケース追加し、clean／raw2引数filterとtyped戻り値を保持。文字集合の除去だけでOS locale／MIME構文／header設定を検証せず、非string coercion・診断・Wasmは未対応。今回は実DB／独立experiment再実行なし。直前DBgateはtarget/filtered-postmeta-and-cgroup-full-check.logのRust190件＋独立experiment13件・両DB各36項目とcold restore／kill／同一DB選別を保持する。Linux native実行・自動容量・CLI cache接続等の未完了も維持する。
>
> 過去のDB実測：`target/byte-preflight-and-html-replacement-full-check.log`。当時workspace Rust152件＋独立experiment5件・Python44件、WP29 API・202,344ケース、両DB各36項目、cold restoreの5拒否＋最終確認が成功。1MiB値の拒否前後の転送量差も64KiB未満。所有fixtureは回収済み。最新WP gateをDB再実測と扱わず、Linux native実行の合格とも区別する。
>
> 過去のDB checkpoint：`target/snapshot-anchor-and-shared-budget-full-check.log`。当時workspace Rust136件＋独立experiment5件・Python44件、WP26 API・172,845ケース、両DB各34項目、cold restoreの5拒否＋最終正常復元が成功。MariaDB anchor73651／global73954、MySQL before69375／reported69735とsnapshot旧値／writer新値・read-only拒否1792を実測。所有fixtureは停止・回収済みでdb-runsは空。
>
> 継続時の残件：本番snapshot／log protocol、安定source識別、COMMIT途中の結果不明・OS／DBクラッシュ復帰、MariaDB standalone互換通知、連続CDC、索引・公開／cache連携、実監視と自動容量、Wasm・認証。MariaDB11.8.9の専用snapshot statusとMySQL8.4.13の通常SHOW位置とのraceは専用toy表で実測済み。MariaDB内部のcommit-order同期や表MDLをなくす保証ではない。FTWRLを公開する本体APIはサーバー全体の書込み停止と失敗時の影響を理由に自動承認レビューで拒否済みで、未解決のまま追加しない。新規の製品版・権限方針は採用しない。詳細は[DB開発メモ](database-development.ja.md)と[開発メモ](development.ja.md)。

> 2026-10-26追記：ユーザーから継続開発の指示を受け、実装を開始した。最新の実装状況と確認待ちは[開発メモ](development.ja.md)を先に参照する。以下の「実装開始待ち」「未着手」は2026-10-25時点の設計引継ぎの記録。未決の方針は引き続き未採用。

以下は2026-10-25（日本時間）の設計引継ぎ記録。**当時は設計協議中・実装未着手。D07-01はA案採用（R58）。現在はD08-01の直接SQL比較案を提示中・未採用。**

## 1. 再開時に読むもの

1. 本書：最新の決定、作業範囲、次回の推奨議題。
2. [設計判断・未決一覧](decisions.ja.md)：決定履歴と残件の正本。過去のA/B比較は、その節の最新決定を優先する。
3. [詳細設計書](design.ja.md)：v0.43、確定要求R01〜R58、受入試験計画T01〜T43。
4. [基本設計レビュー](basic-design-review-2026-10-25.ja.md)と[対応表](review-decisions.ja.md)：過去のレビュー評価と、その後の採用決定を区別する。
5. [終了時の整合確認記録](design-closeout-2026-10-25.ja.md)：今回整理した点と、未実施範囲。

新しいユーザー指示を優先し、提案・未決・将来の試験計画を採用済み／検証済みへ格上げしない。原会話・公式資料の出典は[sources.ja.md](sources.ja.md)を参照する。

## 2. 作業の範囲と進め方

ユーザーの依頼は設計の継続と、レビュー資料を参照しながら一件ずつ決定すること。コード・試作・DBマイグレーション・配備設定・クラウド資源の作成は、別途の実装開始指示を待つ。

WPコア標準は採用対象に統一済み。標準機能の採否を繰り返し質問せず、固定ソースから仕様・比較試験へ整理する。[統一方針](wp-standard-policy.ja.md)を適用する。エンジン、IPC、容量算式等は実装側の提案にまとめ、ユーザーへは互換性・使い方・運用負担に影響する判断を一件ずつ示す。

本体更新の「実証しない」は、ユーザーが直後に**「保証しない」へ訂正済み**。無停止を保証しないという条件であり、検証計画の廃止ではない。

## 3. 決定事項の要約

| 項目 | 決定 |
| --- | --- |
| 製品・コア | RecastPress。WordPressコアをRustへ移植するOSS。元PHP版コアは本番に残さない |
| ライセンス | GPL-2.0-or-laterを基本にGPLv2互換の配布構成を維持。個別監査は未実施 |
| PHP境界 | 既存PHPランタイムを別プロセスで管理・内部処理向けに利用。公開閲覧・APIから直接／間接／非同期にPHPを起動しない |
| プロセス | 公開系と管理系のRustは別プロセス。別ホスト・別バイナリまで必須ではない |
| HTTP | 既存ライブラリによる内蔵HTTP。TLS終端との併用可。具体的ライブラリ等は候補段階 |
| 移植とAI | 利用者／委託先が自分の契約AIでテーマ・functions.phpをRustへ移植。製品はAI接続・キー管理・モデル実行を持たない |
| 実行 | 移植Rustをサーバー側Wasmとして実行。SDK・エンジン・版等は詳細化が必要 |
| WP互換 | 7.1.3固定、同版に残る非推奨APIを含む標準テーマ用API全体。コア標準仕様へ統一。固定アーカイブの取得・照合は未実施 |
| 初回サイト | 単一サイト先行。マルチサイトは後続。特定テーマや標準ブロックだけに製品目標を狭めない |
| 配備 | Linux Docker Compose、Railway、AWS Lightsailの三環境。Dockerイメージが基準。LightsailはLinuxインスタンス上のCompose |
| 対象外 | systemd直接実行は基本構造のみ維持し公式対象外。Render・Cloud Run・App Runner・ECS・Windowsネイティブは初回対象外 |
| DB | MariaDB・MySQL両対応。編集用正本はWP互換構造を維持し、独自状態・公開用データを別領域へ保持（R58）。対応版・詳細・直接SQLは未決 |
| 公開機能 | Rust検索、記事／コメントRSS・Atom、コメント移行・表示とRust受付。標準動作は固定WPを照合 |
| コメント | 新規サイト初期受付off。移行元のサイト／投稿別受付・承認設定は維持。通常利用は推奨せず標準機能の移行用に提供 |
| 通知・フォーム | コメント通知はRustで提供。外部メール接続・送達は保証しない。問い合わせフォームはCMSから分離し、移行診断で案内 |
| 管理接続 | HTTPS＋認証・認可でインターネットから利用可。VPN追加は任意 |
| SSO | 全ユーザー向け標準、OIDCのみ、初期off。off／ユーザー任意／必須要求の3モード。MFAを実装せず、SAML後続予定も採用しない |
| 必須SSOへの移行 | 既存ユーザーごとに一度のパスワード認証をOIDC連携専用に許可。通常機能は連携確定後。有効な手続きセッション内は再開可能 |
| キャッシュ | 初期オン、容量完全自動、手動上限なし、GETクエリーを含むオンメモリ保持。TTLなし。DB／ファイル変更で全パージ |
| キャッシュ停止 | 監視不成立時は全パージし保存・ヒットを停止してRust通常配信。整合確認後に再開。容量超過・再起動での消失可、ディスク永続化は要件ではない |
| 独自設定 | 初期値は移行アシスタント内で質問。移行後はWP管理画面へ追加する1画面で同じ設定を変更 |
| 更新範囲 | 本体のみ自動更新。他は操作説明・セットアップ支援で、更新実行コード・代行API・更新ジョブは持たない |
| 対応版・警告 | セキュリティ更新が自動配信される範囲をサポートし、配信終了版は強く警告。具体的な版・配信元・期間・警告実装は未確定 |
| 本体更新方式 | スキーマ変更なしはRust本体全体を新旧一時二重起動して切替（R57）。無停止を保証しない。スキーマ変更ありの方式は未決 |
| 移行プラグイン | 元WPに導入するPHP製対話型プラグイン。認証付き直接転送とファイル書出し／取込みの両対応 |
| 移行の責任 | 方針説明、診断に応じた移植用／テスト用プロンプト、元WPの画面・HTTP応答を初回と切替直前に記録、比較支援を提供。移植・確認・公開判断は利用者／委託先が担い、移行成功は保証しない |
| 想定・提供形態 | CDNを必須としない企業・商品紹介・カタログ・通常LP・個人等、単一サイト月間20万PVまでを想定。個別サポートのないOSS。追加の集中アクセス対策はインフラ側 |
| 仲介サービス | 必要性の検証まで採用保留。アイデアは残すが、必須のサービス・データ預託・AI提供とはしない |

## 4. 次回の推奨議題と残件

**2026-10-26のDB仕様更新を反映**：D07-01・D07-02、R58～R61は採用済み。[WP互換DB構成](database-schema.ja.md)と[内部自動最適化・binlog変更追跡](database-auto-optimization.ja.md)の採否は残件ではない。現在の方針判断は**D08-01：プラグインの直接SQL**で、[比較案](plugin-database-access.ja.md)のAは未採用。変更追跡の採用をSQL実行権限の承認に広げない。

[未決一覧](decisions.ja.md)は20分類（保留1）。以前の19分類から、一覧に欠けていたD04のWasm・SDK・成果物契約を補った。調査・検証や実装詳細も含むため、20回のユーザー判断を意味しない。ユーザー判断を8〜10論点程度へまとめる見込みは整理上の目安で、質問回数の確約ではない。

DB関連の残件は、正式対応版と三配備先のbinlog設定・権限・保持・形式差・監視不能環境、全コア表とlegacy charset／collation・非InnoDB移行・同時DDL、内部データ／索引の範囲・検索意味論、自動構築／破棄基準・共有メモリ予算、スナップショット／ログ位置・COMMIT反映・再開／再構築・世代切替の実装詳細、公開反映／撤回の期限と同期・期限超過時の応答、T44と性能検証。[DB開発メモ](database-development.ja.md)に記録された参照DDL生成・3表のreader・専用DB試験と、未実装の内部索引・CDC・公開データ生成を区別する。固定WPの取得・照合も完了済みだが、API全体・Linux三配備先の互換性は未検証。

その他の残件は、直接SQL・プラグイン権限、SSO連携・回復、移行切替・復旧、スキーマ変更ありの更新と自動更新設定、保存先、バックアップ／運用目標、初回合格条件、配布・供給網。D06とD13等の重複はまとめて判断する。正本の[残件一覧](decisions.ja.md)を参照し、試作・部分的な試験を製品全体の完成や検証済みとしない。

## 5. 再開時に誤って戻さないこと

- GPLv3推奨の旧案、六／五環境の旧配備案、全層自動更新案、任意の手動キャッシュ容量案は現行仕様ではない。
- SSO初期off・キャッシュ初期onは個別の決定。本体自動更新の初期on／offは未決。
- 一度の連携用パスワード認証を通常ログインへ広げず、許可を勝手に再発行しない。具体的な消費・回復は未決。
- 本体二系統切替と、テーマだけのWasm二世代切替を混同しない。後者の詳細は補助案。DB・PHP等を二重起動する決定ではない。
- R57を採用済みとして維持し、「未実証」を理由に採用未確定へ戻さない。実装・性能の検証結果や無停止保証とは区別する。
- 数値目標・版・既存プラグイン全互換・市場比率を確認済みとしない。WP標準の抽出作業も未実施のまま。

## 6. 作業場所・保存状態・更新方法

作業先：`C:\Users\nodew\recastpress`。旧Downloads配下は履歴上のパス。追加の作業先として使わない。

本整理ではMarkdownだけを更新し、実装・実環境への更新・commit・pushは行っていない。終了時のGit読取り確認ではREADME.mdが変更済み、docs/が未追跡。ローカルファイルは保存済みだが、Gitへ記録済みとは扱わない。他スレッドのロゴ資料・画像等は保持した。

再開時は現状を読み直し、他スレッドの変更を保護する。新しい回答はdecisions.ja.mdへ日付・決定・影響を記録し、design.ja.mdと関連詳細書・本書へ同期する。ユーザー判断がない実装案を新しいR要求へ追加しない。元のレビュー本文は今回編集していない。

## pre-COMMIT killの引継ぎ（2026-10-26）

process_crash_recoveryにREADY／実PID／非zero kill終了／nonce／epoch／別resumer／同一DB Popenの生存／通常projection最終復元を保存する。kill対象はharnessが起動した専用clientのみで、既存DBやDB本体はkillしない。cloneのID0はSESSION NO_AUTO_VALUE_ON_ZEROを追加して保持し、元session modeを復元する。global modeは変えない。markerはcanonical1行・64KiB、old tupleのreceipt／row payloadは各12KiB、receiptはexact1 frame。余分stdout／stderrを拒否し、timeout時も所有childとreaderを終了・回収する。

この成功はCOMMIT前のrollbackと有限replayまで。COMMIT途中の曖昧な結果、server／OS／power loss、安定identity・連続CDC・公開切替の完成ではない。[DB開発メモ](database-development.ja.md)と[experiment](../experiments/binlog-probe/README.md)を参照する。

## 選別fixtureとcgroup観測の引継ぎ（2026-10-26）

filtered_postmetaは専用clone／journalだけを使い、WP表・通常projectionを変更しない。同一DBの既知target／other表の全TableMap schemaと全row imageを検査する。除外だけのcommitもraw receiptと新位置を保存し、rollback／ACK lossの再送、CRC／gap／actual ignored DDL／wrong profile拒否と保存7tupleを照合した。MariaDBだけignored_ddl_prefix_rejected=trueで、MySQLはfalse。未知／別DBは許可せず、cross_database_live_verified=false。実運用filtering、安定identity、本番snapshot anchor、連続CDCは未完成。

cgroup v2はcaller指定pathのcurrent／max／highを読むraw 1 level観測で、kernel上許されるcurrent>max等を保持する。所属・mount・祖先・namespace探索やcgroupfs検証はせず、headroom・RSS・Windows job・自動配分・CLI cache接続へ流用しない。実監視・容量算式・native Linux観測は引き続き未完了。[HTTP cache開発メモ](http-cache-development.ja.md)

2026-10-27追加：別のself membership候補がunique cgroup2 mount root `/`とclean bounded pathを照合し、可視祖先だけをleaf順で読む。upper_ancestors=Unknown、consistency=NonAtomic。境界3memory file全NotFoundだけBoundaryUnobserved、途中／部分欠落や不正値はエラー。submount／検出symlinkと前後proc・namespace identityの変更を拒否しても、TOCTOU・ABA・namespace外祖先やcgroupfs真正性の証明にはならない。safe fd resolver・native Linux実読・自動容量・CLI cache接続は後続。新unit5件はWindowsで通過しLinux向けcompileまで確認、target/unique-id-and-visible-cgroup-check.logの統合gateも成功。native Linux実読は未実行。[観測API・制約・公式根拠](cgroup-memory-development.ja.md)
