# 実装の進捗と再開メモ

## 現在地：Cookie照合のmain取り込み後、ユーザー指示で中断

2026-10-30のdocs-only整合記録。`target/auth-cookie-verification-pause.json` のとおり、prepared GET／非Ajaxのcookie MAC＋session照合、既存生成側のprivate MAC共通化、専用probe／PHP oracle／checker／comparator／gate flagはmainに取り込んだ。対象Rust21、Python12、fmt／WP全target Clippyだけが成功し、所有processは回収済み。先行target-onlyの実WP30／native両端60／closed5は履歴で、本体採用後の同じ実比較ではない。

本体の正式79 API／519,014・hooks572／dispatcher405、既存認証３種＋新cookie関係の４種、全８DB mode、workspace全体、Linux cross compile、pure、inventory／licenseはまだ実再検証していない。WP source／manifestとroot lockの変更により、既存のsource-bound証跡と依存台帳を現在の成功として扱わない。旧formal３copyは `target/auth-cookie-verify-pre-refresh-{primitives,hooks,dispatcher}.json` に保持する。以下の各「最新checkpoint」やcurrent照合成功は、その段階のsourceに対する完成履歴である。

`check.py --with-auth-cookies` は実装済みで、専用comparator→checkerを追加するが、予定出力 `compatibility/evidence/auth-cookie-relations.json` はまだ生成されていない。この関係を新しいstatic API件数へ加算しない。unfinalizedな一致はprincipal／role／session発行／HTTP認証ではなく、管理ready503を維持する。

### 再開時の未実行手順

これは実行済み記録でも再開指示でもない。新たな実行の許可後に、まず実metadata／archiveから台帳を再生成し、source freeze guardの開始snapshotより前に固定する。hashだけを手更新しない。

```powershell
python scripts/dependency_inventory.py
python scripts/dependency_license_evidence.py
python scripts/check.py --with-reference --with-password-generation --with-legacy-passwords --with-credential-stage --with-auth-cookies --with-pure-wp --with-license-evidence --with-linux-check --with-process-smoke --with-databases --with-binlog-probes --with-mysql-overlap --with-mariadb-snapshot-bootstrap --with-mysql-rotation-handoff --with-mysql-server-restart --with-alpha-publication
```

その後、旧79全件数と入力／PHP／Rust結果SHA、hooks／dispatcherを保存copyと比較する。新cookie関係を含む最終12checker、全所有process／DB directoryの回収を確認し、docsとcheckpointを最後に更新する。main取り込み前のtarget checkpointや過去のnormal graphをrestampして代用しない。

## 完成済みcheckpointの履歴

最新build feature checkpoint（2026-10-30、日本時間）：[既定有効authとpure WP構成](wp-build-profiles-development.ja.md)を分離し、専用native gateと`check.py --with-pure-wp`連携を追加。`target/wp-auth-feature-full-check.log` exit0。default Rust554（ignore1）＋pure native281＋独立29、Python342実行／336成功／symlink権限6 skip、正式73 API486,153／hooks572／dispatcher405、credential3種、全8 fresh DB各62、Clippy・default Linux cross-check・license215・HTTPが成功。旧73の全件数・入力・PHP/Rust結果SHAも実保持照合（`target/wp-auth-old73-preservation-check.log`）。pure実Cargo JSONはfeatures空・57 test artifacts・通常依存WP1件で、以前のNoDefault21件に残った暗号7 provider／getrandomを除外。defaultと明示authの通常graphは一致し、アルゴリズム・版・Root lockは維持した。manifest変更後の依存一覧とlicense file観測は実再生成。最後の11checker全exit0（`target/wp-auth-feature-current-check-summary.json`）、全DB回収。これはnative hostのbuild構成で、Wasm compile／ABI／sandboxや認可の成功ではない。次のtheme getter候補は既定title filterの扱いまで含めて検討する。

最新cached単件metadata checkpoint（2026-10-29、日本時間）：[get_post_meta限定profile](post-meta-cached-development.ja.md)を追加し`target/post-meta-cached-full-check.log` exit0。workspace Rust554（ignore1）＋独立29、Python335実行／329成功／symlink権限6 skip、正式73 API486,153（新3,202）／hooks572／dispatcher405、credential3種実測、全8 fresh DB各62、Clippy・Linux cross-check・license215・HTTPが成功。旧72の全件数・入力・PHP/Rust結果prefixSHAも実保持照合（`target/post-meta-cached-old72-preservation-check.log`）。実metadata calls/get/cache/restored各3,202、default626、cache update／SQL0。最後の11checker全exit0（`target/post-meta-cached-current-check-summary.json`）、全DB回収。caller準備済みcomplete/nonempty cache・single・関連get/default/all filter無しだけで、empty key/"0"・cache miss・multiple・登録default filter・画像描画は未追加。borrowedキー照合と選択値の4上限、Null defaultとfalse維持を扱い、PHP-only witnessは入場検査だけでnative結果へ使わない。次は既定動作を保った暗号/認証依存のbuild feature分離を検討する。

最新上限付き復元checkpoint（2026-10-29、日本時間）：[caller明示のserialized値復元](bounded-unserialize-development.ja.md)を追加し、`target/bounded-unserialize-full-check.log` exit0。workspace Rust548（ignore1）＋独立29、Python324実行／318成功／symlink権限6 skip、正式72 API482,951／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB profile各62、Clippy・server/WP Linux cross-check・license file215観測・実HTTPが成功。保存した旧72の全件数・入力・PHP/Rust結果SHAも実再比較後に保持を照合（`target/bounded-unserialize-old72-preservation-check.log`）。最後の11checker全exit0（`target/bounded-unserialize-current-check-summary.json`）、全DB回収。入力／累積pair／所有string・key bytes／array深さはcaller必須・既定値なしで、第二の完全canonical bufferを新経路に作らない。raw行を明示選択し、missing／SQLNULL／Null／false／空Bytesと未対応拒否を区別して元meta6/options4へ復旧した。normal DB/public依存のWP境界も維持。get_option・cache選別・theme選択・認可・RSS/CPU保証は追加しない。次の候補は、実consumerを一つ選んだ準備済みmetadata cacheの利用で、未選定のAPI拡張は始めない。

最新receipt付き検査checkpoint（2026-10-29、日本時間）：[check-package](package-check-development.ja.md)を実装。workspace Rust541成功（ignore1）・Clippy・server/WP Linux cross-check、Python324実行／318成功／symlink権限6 skip、依存・license file観測215、実HTTPが成功した。初回gateの版2正常fixtureに非ASCII redirect Locationを入れた1失敗は、既存仕様に合わせたtest-only修正で訂正し、初回logも保持。Rust等の成功は`target/package-check-workspace-check.log`、修正後Pythonと残工程のexit0は`target/package-check-continuation-check.log`へ分けた。生成10・legacy113・stage33とalpha両DB各61を実再測定、全DB回収。最終11checker全exit0（`target/package-check-current-check-summary.json`）。保存済み2packageも新CLIで無変更検査し、本文547／153 bytesを確認。server依存を持つWasm公開実験だけnative6＋実HTTPを再実行し全回収（`target/package-check-wasm-publication-observation.json`）、他の独立Wasm実験はsource不変の過去観測を維持。署名・認証・最新世代やnative Linux成功は追加しない。次は既存maybe_unserializeのcaller明示資源上限と、選択したraw DB値だけの復元を検討する。

最新JSON再読込みcheckpoint（2026-10-29、日本時間）：[入力ワークベンチ](prepared-post-workbench-development.ja.md)へUTF-8／schema専用の8MiB readerと、確認後の全件置換・取消を追加。Node13契約が成功（`target/prepared-workbench-import-node-contracts.log`）、既存native artifactとの有限73件はJS32受理／native30受理・41両拒否で、差2件はHTML escape／bundle最終上限の意図した拒否（`target/prepared-workbench-import-native-parity.json`）。実browserは2件の読込みpreview時に元1件保持、取消後も保持、狭幅CSS375pxで横overflow無しを確認。ApplyはNode統合契約で確認し、実browserの置換操作は未実施。画像は`target/prepared-workbench-import-{desktop,mobile}.jpg`。全所有server/thread/tabを回収。取消はpending結果を無視するだけでFile読取のabortではなく、8MiBもtab RSS／CPU上限ではない。続いてreceipt付きpackageをlistener無しに確認するone-shot CLIを実装中。

最新ローカル入力画面checkpoint（2026-10-29、日本時間）：[PreparedPost入力ワークベンチ](prepared-post-workbench-development.ja.md)を追加し、ブラウザーの実download 215 bytesから既存CLIでbundle776／receipt117 bytesを生成、別sealed serverの2記事GET／HEAD、query順序保持／逆順404／Host421を確認した。script風文字・先頭LF・日本語・emoji・空title/bodyは独立HTML parseで一致。画面の重複target／予約path／NUL拒否・記事切替保持も実操作した。desktopと狭幅の画像を`target/prepared-workbench-{desktop,mobile}.jpg`、native観測を`target/prepared-workbench-native-roundtrip.json`に保存。狭幅は390px指定時CSS375pxで横overflow無し、実mobile/touchと削除Undoの実操作は未検証。所有HTTP fixtureは停止exit0／thread回収、検証tabも閉じた。タブ内編集とJSON書出しだけで、自動保存・認証付きDB編集・公開操作は未追加。直前backend全Python318／最終11checkerのsourceは維持し、静的画面追加後にbackend全再実行はしていない。

最新要求時Wasm／pair一覧checkpoint（2026-10-29、日本時間）：独立loopback request probeのnative7契約・fmt／Clippy、Python gate7契約、実HTTP9要求が成功。startup準備後のGET A/B/AとHEADだけがCALL4件（各guest counter1）になり、Host／method／query／unknown／absolute-target拒否はCALL無し。stdout空・全process／reader／temp回収も確認。local pair一覧は対象8群中7成功／symlink1 skip。両追加を含む全Python318実行／312成功／symlink権限6 skip、最終11証跡checker全exit0（`target/request-pair-list-current-check-summary.json`）。観測時sourceは`target/request-pair-list-checkpoint.json`。root source／lock／旧binary・probeは維持。[要求時実験](wasm-request-probe-development.ja.md)／[pair一覧](http-baseline-pair-list-development.ja.md)。続行してタブ内だけで編集・downloadするPreparedPost入力ワークベンチを実装中、CMS認証／DB編集UIやRust guest buildは未完成。

最新固定Wasm公開経路checkpoint（2026-10-29、日本時間）：独立producerのnative6契約・fmt／Clippy、frontend7群中6成功／symlink権限1 skip、所有HTTP E2Eが成功。Componentで生成した153-byte HTMLをpost_return後に所有し、281-byte bundle／117-byte receiptを既存admissionへ渡し、別serverでGET／HEAD／304／Host421、同URL capture2回の8項目一致を確認した。全Python303実行／298成功／symlink権限5 skip、最終11証跡checker全exit0（`target/wasm-publication-current-check-summary.json`）。全所有process／reader／temp回収。review用packageは`target/wasm-publication-example/`へ別途新規作成し、source／両artifact／package観測は`target/wasm-publication-example-checkpoint.json`。root source／lock／旧binary・先行probeは維持、GET中Wasm／実theme／認証／Linuxは未完成。[再現手順](wasm-publication-example-development.ja.md)。次は別のtyped request-context／要求ごとのStore実験を調査する。

最新独立Component checkpoint（2026-10-29、日本時間）：Windows native5契約・31固定観測、fmt／Clippy、Cargo選択artifactの有限実行・PID照合が成功。全Python296実行／292成功／symlink権限4 skip、exit0（`target/wasm-component-probe-python-suite.log`）、試験後の既存11証跡checkerも全exit0（`target/wasm-component-current-check-summary.json`）。観測時source hashは`target/wasm-component-probe-checkpoint.json`、registry118の限定license／archive観測を別保存。最初のWAT parse拒否は厳密構文へ訂正し、失敗logも保持。UTF8／cap拒否時もpost_return、後処理失敗なら暫定出力を返さない境界を実測。製品source／root lock／旧core実験は無変更、LinuxやSDK採用は未確認。[専用記録](wasm-component-probe-development.ja.md)。次は固定公開HTMLのComponent生成→既存bundle/receipt→別sealed HTTPへ接続する独立実験を進める。

最新独立Wasm core checkpoint（2026-10-29、日本時間）：独立Wasmtime36.0.19候補でWindows native Rust5契約・27固定観測、fmt／Clippy、実Cargo artifactの有限実行・PID照合が成功した。全Python290実行／286成功／symlink権限4 skip、exit0（`target/wasm-runtime-probe-python-suite.log`）、試験後の既存11証跡checkerも全exit0（`target/wasm-core-current-check-summary.json`）。観測時source hashは`target/wasm-runtime-probe-checkpoint.json`、registry101の限定license／archive観測も保存。Linux cross-checkは既設C crosscompiler不足でexit101、失敗logを保持。製品workspace／Cargo.lock／公開path／DB／WPには無変更で、engine／SDK採用・Linux成功とは扱わない。[実験の範囲](wasm-runtime-probe-development.ja.md)。次は別のComponent Model固定fixture候補を調査する。

最新source inventory checkpoint（2026-10-29、日本時間）：明示選択fileだけをopaque bytesとしてhash観測するoffline toolを追加。対象9群中8成功・Windows symlink権限1 skip、全Python284実行／280成功／4 skip、exit0（`target/migration-source-inventory-python-suite.log`）。試験後の既存11証跡checker全exit0（`target/source-inventory-current-check-summary.json`）。元source非更新、選択順、selection／manifest独立64KiB、file8MiB・accepted aggregate32MiBのexact／one-over、partial保存を確認。再帰探索・PHP実行・コードcopy・AI送信・自動診断はしない。[手順](migration-source-inventory-development.ja.md)。production Rust／DB／PHP sourceは維持。続いて[独立Wasmtime実験](wasm-runtime-probe-development.ja.md)を進行中で、engine／SDKの採用は未確定。

最新HTTP記録・比較checkpoint（2026-10-29、日本時間）：単一URL captureと保存済み2記録のoffline比較を追加し、全Python275実行／272成功／Windows symlink権限3 skip、exit0（`target/http-baseline-and-comparison-python-suite.log`）。対象capture12群・比較9群も独立実行済み。実TLSはskip無し、比較は独立ローカルfixture／実CLI・workerを使い、timeout改変はmock契約と区別した。試験後の11証跡checker全exit0（`target/http-tools-final-check-summary.json`）。新Python／手順書のみでRust／DB／PHP sourceは無変更、直前Rust536＋独立29／正式72 API482,951／全8DB61実測を維持。外部site・画面取得・実移行・Linux実行は未実施。移植／テスト依頼の[手動ひな形](manual-migration-review.ja.md)も追加。次は明示source一覧のoffline hash inventoryを検討する。

最新手動HTTP baseline checkpoint（2026-10-29、日本時間）：明示1URLのGETを所有workerで取得し、新規dirのbody.bin／capture.jsonへ保存する開発用toolを追加。実CLI／loopback／TLSを含む12群中11成功・Windows symlink作成権限不足1 skip、exit0（`target/http-baseline-owned-cli-tests-final.log`）。204／304のTE付きheader-only、独立metadata上限、worker receipt異常値とcleanup retryも検査した。既存11証跡checker全exit0（`target/http-baseline-current-check-summary.json`）。Rust／DB／正式PHPのsourceと前checkpointは維持、今回それらの全再試験は行っていない。外部site取得・画面比較・移行成功判定・Linux実行は未実施。[手順と限界](http-baseline-development.ja.md)。継続して2つの保存済みHTTP記録のオフライン構造比較を実装中。差は観測として返し、移行の合否へ変換しない。

最新truncate-slug／post-name pipeline checkpoint（2026-10-29、日本時間）：`target/truncate-slug-and-post-name-pipeline-full-check.log` exit0。workspace Rust536（ignore1）＋独立29、Python254実行（253成功・symlink権限不足1 skip）、正式PHP72 API・482,951比較／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB fixture各61、Clippy・server/WP Linux check・license215観測・実HTTPが成功。新truncate16,064（raw非切詰め13,172／raw substr1,155／decoded URI1,737）をappendし、保存した旧71の件数・入力／両結果SHA保持を実prefix照合（`target/truncate-slug-old71-preservation-check.log`）。最終11checker全exit0（`target/truncate-slug-current-check-summary.json`）、全DB回収。dev-only Query→text lookupはcasevariant・任意UTF8 fallback・invalid fallback非lossy拒否・元4posts/6meta復旧を実測した。unique slug／URL解決／認証／自動公開／native Linuxは未完成。次はR54の明示1URL HTTP baseline記録candidateを検討する。

HTTP baseline記録候補の所有loopback検証は最上段checkpointで完了。直上72 API／DB61はRust／DBの完了記録であり、新しいHTTP toolの試験とは分ける。

最新sanitize-title／post-name checkpoint（2026-10-29、日本時間）：`target/sanitize-title-and-post-name-retry-full-check.log` exit0。workspace Rust529（ignore1）＋独立29、Python248実行（247成功・symlink権限不足1 skip）、正式PHP71 API・466,887比較／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB fixture各60、Clippy・server/WP Linux check・license215観測・実HTTPが成功。新sanitize_title5365をappendし、保存した旧70の件数・入力／両結果SHA保持を実prefix照合した（`target/sanitize-title-old70-preservation-check.log`）。最終11checker全exit0（`target/sanitize-title-current-check-summary.json`）、全DB回収、public／runtimeDBのnormal依存境界も維持。初回gateはpost-name試験開始時のmetaを後段11行と混同したassertで停止し、call順と6＋後段追加5を確認してbaseline6へ訂正した。両backend60の直接再測定後、上記full gateを再実行。失敗logとserver logは保持し、正常終了/PID停止確認後に失敗fixtureも回収済み。URL解決・unique slug・新規投稿・認証・自動公開・native Linuxは未完成。

最新URI/title checkpoint（2026-10-29、日本時間）：`target/uri-title-and-credential-pipeline-full-check.log` exit0。workspace Rust520（ignore1）＋独立29、Python241実行（240成功・symlink権限不足1 skip）、正式PHP70 API・461,522比較／hooks572／dispatcher405、生成10・legacy113・stage33、全8 fresh DB fixture各57、Clippy・server/WP Linux check・license215観測・実HTTPが成功。新URI5920／title2538をappendし、保存した旧68記録の件数・入力SHA・両結果SHAを実再計算prefixと照合して保持を確認（`target/uri-title-old68-preservation-check.log`）。最終11checker全exit0（`target/uri-title-current-check-summary.json`）、全DB回収・db-runs空。URL妥当性・unique slug・認証・自動公開・native Linux実行は未完成。次は外側sanitize_titleの明示locale/default callback限定compositionを検討する。

WordPressの無ORDER LIMIT1・PHP truthiness・hierarchy／numeric date条件があるため、既存ID順pageをそのままunique-slug関数互換とは扱わない。衝突回避・予約・DB write・公開policyは今回の切詰め候補へ混ぜない。

最新Linux helper checkpoint（2026-10-29、日本時間）：`scripts/check_native_linux.py`と専用mock7契約を追加。Windowsで`--plan` exit0、通常実行の非Linux guard exit1、mock7件成功を`target/native-linux-gate-{plan,windows-refusal,contract}.log`に記録した。Linux実機でのRust/HTTP/Unix signalと新select capture本体は未実行。Cargo actual artifactをsmokeへ束縛し、host target固定・選択8Pythonのみの限定gateで、install/fetch/CI/DB/PHP/正式proof生成はしない。既存production sourceは無変更、最終11checker全exit0（`target/native-linux-helper-current-check-summary.json`）、直前offline package checkpointのRust504／Python228（1skip）・DB57／PHP68 API453,064実測を維持。Linux向けcross-checkをnative成功へ数えない。[Linux実行の前提と残件](native-linux-validation.ja.md)。

最新offline package checkpoint（2026-10-29、日本時間）：`target/offline-post-package-full-check.log` exit0。workspace Rust504（ignore1）、Python228実行（symlink権限不足1 skip、227成功）、Clippy・server/WP Linux check・license215・実HTTP、fresh alpha DB2構成各57が成功。新module6件＋実CLI6群中5成功/1 skipを含む。DB/WP/core本体は無変更、独立29と他6DB構成の直前実測はsource一致のまま維持。workspace再buildで比較binaryが変わったため、生成10・legacy113・stage33を`target/post-package-*-remeasure.log`で実再生成し、最終11checker全exit0（`target/post-package-current-check-summary.json`）。正式68 API453,064のsource/台帳もcurrent、全DB回収済。新コマンドは明示入力から新規dirへ固定2fileを作るoffline操作で、principal/session/Wasm/自動公開は未追加。

offline package候補とLinux限定gateの準備は上記checkpointで完了。現在は明示slug入力に使うURI encoding／title-with-dashesの有限pure profileを固定WP sourceから調査中。URL自動割当・一意slug・新規INSERT・認可・公開のpolicyは追加しない。

次工程調査メモ：新規投稿のraw INSERTは今回は採用しない。固定WPのAPI既定はdraft（`post.php:4612`）に対しDDL既定はpublish（`schema.php:167`）で、title/bodyだけを挿入すると意味が変わる。slug・日時・権限・terms/meta・GUID・cache/hooksも別工程であり、低level INSERTを`wp_insert_post`互換や認証付き新規投稿の完成へ読み替えない。将来の全列caller-prepared storage候補も、採番欠番・COMMIT unknown・重複作成回復を別契約にする必要がある。

最新credential stage checkpoint（2026-10-29、日本時間）：`target/credential-stage-and-pipeline-full-check.log`がexit0。workspace Rust498件（ignore1）＋独立29、Python222、正式PHP68 API・453,064比較／hooks572／dispatcher405、生成10・legacy113、別枠credential stage33（actual25／refusal8）、全8 fresh DB fixture各57、Clippy・server/WP Linux check・license215・実HTTPが成功。最終11checker全exit0（`target/credential-stage-current-check-summary.json`）、全DB回収・db-runs空。raw candidateはprincipal/sessionではなく、確定不一致のみemailへ進み、lookup／hash／cost／provider不明は停止。次は明示入力からbundleとreceiptを一操作で作るoffline package候補へ進む。既存出力は上書きせず、不完全出力は失敗として保持する。自動公開・認証・Wasm方針は追加しない。

最新username prefix checkpoint（2026-10-29、日本時間）：`target/username-preprocessing-and-credential-pipeline-full-check.log`がexit0。workspace Rust490件（ignore1）＋独立29、Python214、正式PHP68 API・453,064比較／hooks572／dispatcher405、生成10相互照合各24・legacy113（actual88/refusal25）、全8 fresh DB fixture各56、Clippy・server/WP Linux check・license215・HTTPが成功。新67/68は2572／7414件、旧66/443078を保持。実get_localeのscoped first-isset branchへ移行し旧66の3698出力も維持。最終10checker全exit0（`target/username-current-check-summary.json`）、全fixture回収・db-runs空。PreparedLoginInputはS1/P1/S2とPHP-emptyを保持する非principal型、次はusername/email credential stageの未確定candidateまでを検討する。HTTP認証・session・role・cache/action・自動rehash policyは未接続。

credential stageの実装・全体gateは上記checkpointで完了。HTTP認証・principal・session・自動更新policyは未接続のまま。次のoffline packageは検討を終え、これから実装する。

最新remove_accents checkpoint（2026-10-29、日本時間）：`target/remove-accents-and-credential-pipeline-full-check.log`がexit0。workspace Rust477件（ignore1）＋独立29件、Python207件、正式PHP66 API・443,078比較／hooks572／dispatcher405、生成10ダミー相互照合各24・legacy113（actual88/refusal25）、全8 fresh DB fixture各55、Clippy・server/WP Linux check・license215・HTTPを通過。新3698件は既65 API/439380件の後へappend-only、311base＋locale overridesはdata-only exporterでpinnedsourceから一致検査。最終10checker全exit0＋exporter--check成功（`target/remove-accents-current-check-summary.json`）、全fixture回収・db-runs空。次は標準login名前処理へ向け、NoLiteral< strip／sanitize_userの限定pure候補をsourceから検討。[入口の順序と未接続](authentication-entry-development.ja.md)。

最新credential pipeline checkpoint（2026-10-29、日本時間）：`target/password-rehash-and-credential-pipeline-full-check.log`でRust471件（ignore1）＋独立29件、Python200件、正式PHP65 API・439,380比較／hooks572／dispatcher405、生成10ダミー相互照合各24、legacy113（actual88/refusal25）、全8 fixture各55、Clippy・server/WP Linux check・license215・HTTPが成功した。DBdev-only WP結合に合わせsource追跡を強化した後、`target/credential-pipeline-complete-source-provenance-check.log`で全8 fixtureを実再生成。全DB記録がWP Cargo／全srcもbindすることを確認し、collector回帰assertも成功した。buildで比較binaryが変わったため生成・legacyと正式PHP3比較を`target/credential-pipeline-final-*-remeasure.log`で再実測。最終10 checkerは全exit0（`target/credential-pipeline-current-check-summary.json`）。全fixture回収・db-runs空、runtimeDB/serverにWP normal依存無し。次はlogin名前処理のremove_accents候補。session／HTTP認証／自動rehash policyは未接続。

最新raw dispatcher checkpoint（2026-10-29、日本時間）：`target/password-dispatch-workspace-reference-check.log`がexit0。workspace Rust464件（ignore1）、Python195件、正式PHP64 API・437,198比較／hooks572／dispatcher405、生成10ダミー相互照合各24、拡張legacy113ケース（元64保持＋combined49、actual WP88／refusal25）、Clippy・server/WP Linux check・license215・実HTTPを通過。DB/core/Cargo/check.pyを変更していないためDBを起動せず、通常DB・binlog・overlap・Maria snapshot・rotation・restart・alphaの全checkerを照合してcurrentを確認。独立29件・8 fixture各48の直前実測を維持し、db-runs空。次は既定rehash要否のpure候補と、別privatepoolのraw credential CAS。認証API／session／自動credential更新はまだ接続しない。

最新legacy／user lookup checkpoint（2026-10-29、日本時間）：`target/legacy-password-and-user-lookup-full-check.log`がexit0。workspace Rust458件（ignore1）＋独立29件、Python194件、正式PHP64 API・437,198比較／hooks572／dispatcher405、生成10ダミー相互照合各24回、別枠legacy64ケース（actual WP50／明示拒否14）、全8 fresh DB fixture各48項目、Clippy・server/WP Linux check・license215観測・HTTPが成功した。全fixture回収、db-runs空。旧hash検証を正式API件数へ加算していない。公開normal dependency treeもDB/WP/sqlx/mysql_async/bcrypt/hmac/md-5を含まない。途中のMaria NUL推定とMySQL metadata signed型の失敗は修正し、失敗logを保存して所有停止確認後に専用dirを回収した。次は同じsource分岐順のraw password dispatcher候補。認証・session・credential保存・native Linux実行の完成ではない。

継続作業中：旧WP MD5／portable phpass検証候補とediting-only user login/email lookupへ進む。rootがmd-5 0.11.0(zeroize)と既存subtle2.6.1をdirect追加し、164／188／unique215、210 archive／377 license/notice fileの実観測を更新済み。DB required checksはuser lookupの3契約を予定し45→48へ追従中。新sourceに対するPHP・DB・全体gateの実測は後続で行い、直前generation checkpointの件数や成功を変更後へ読み替えない。

最新generation checkpoint（2026-10-29、日本時間）：`target/wp-password-generation-and-alpha-full-check.log`がexit0。workspace Rust449件（ignore1）＋独立29件、Python188件、正式PHP64 API・437,198比較／hooks572／dispatcher405、生成の独立相互検証10ダミー（両方向各24照合）、全8 fresh DB fixture各45契約、Clippy・server/WP Linux all-target check・license214観測・HTTPが成功した。全fixture回収、`.reference/db-runs`空。生成を既存API件数へ加算していない。新依存追加無し。次は旧hash検証とediting-sideユーザー検索を検討・実装する。標準login入口はpasswordをtrimするが低levelverifyはtrimしない、user_statusを単一サイトbanへ転用しない、DB collationに従うlookupをRust byte比較で置換しない点をsourceから確認した。

最新checkpoint（2026-10-29、日本時間）：`target/wp-bcrypt-and-alpha-publication-full-check.log`で全体gateがexit0。workspace Rust443件（ignore1）＋独立29件、Python182件、実PHP64 API・437,198ケース／hooks572／dispatcher405、8つの新規所有DB fixture各45項目、Clippy・server Linux cross-check・license214観測・実HTTPが成功し、`.reference/db-runs`は空。WP crateも別途`target/wp-bcrypt-linux-cross-check.log`で全targetのLinux向けcheckが成功した。native Linuxのリンク／実行ではない。新bcrypt542件はactual hook542を含め差分0。全DB／native／α evidenceは実測で再生成した。公開normal dependency treeはDB/WP/sqlx/mysql_async/bcrypt/hmacを含まない。次はWP既定パスワード生成候補へ進む。管理認証・session・Wasm・current DB同期／live公開は未完成。

2026-10-29の作業中checkpoint：`target/post-save-to-sealed-publication-full-check.log`でworkspace Rust437件（ignore1）＋独立29件、Python170件、8つの新規所有DB fixture各45項目、Clippy・Linux cross-check・license208観測・実HTTPが成功した。その後、WP既定bcrypt検証候補とHTTP子回収の補強を加えたため、現ソースの全体検証・PHP比較・8 fixture再実測を続ける。旧gateの成功を変更後の成功に読み替えない。新依存は163／188／unique214、209 archiveと375 license/notice fileを実観測して記録済み。`target/smoke-lifecycle-root-review.log`の実子8試験と固定comment-feed／prepared-postサンプルのHTTPも成功した。[保存から公開snapshotまで](alpha-publication-development.ja.md)、[起動実例](running.ja.md)。Docker/Compose候補は静的確認のみで、Docker不在のnative検証は未完了。

再開（2026-10-29、日本時間）：ユーザーの続行指示により停止解除。subagentは許可上限4本、実環境の同時4枠にはrootも含むため3本で分担。今回は記事保存→明示的な公開準備→bundle／receipt→別公開processの有限縦断を優先する。DB writerは原子な既存row text CAS、rendererはcaller指定のtarget/title/body_textのみ、receiptはchecksumであり署名・認可・rollback防止ではない。管理認証・Wasm・WP保存hooksや継続公開は未完成。検証完了前の実装をα完了として数えない。

**停止中（2026-10-27、日本時間）**：ユーザーの「キリの良いところで停止」に従い、実行中の全gate完了を確認して停止した。新規実装・継続作業は再開指示まで行わない。両subagentは完了済み、実行sessionは回収済み、専用DB directoryも空。commit／push／外部配備は行っていない。

最新empty-filter escape／メモリ診断gate：target/empty-filter-escaping-and-memory-diagnostic-workspace-check.logで実PHP63 API・436,656ケース／hooks572／dispatcher405、workspace419 Rust（通常ignore1）・Python155、Clippy・Linux cross-check・license208観測・実HTTP・DB current hashが成功。esc_html／esc_attrは各1,938件の明示UTF8・empty-filter profileで、5globals／INI復元・actualhookcounterを厳密照合する。memory CLIは3観測を独立報告し、Windowsでは全Unsupported／exit3を実測。self RSSは診断CLI自身であり既存serverの測定ではない。両変更の独立レビューも通過した。全native current hashも別ログで成功し、DB live／独立experiment再実行無しで直前29件・6 fixture各39項目を維持する。[escape](escaping-empty-filters-development.ja.md)・[memory CLI](memory-diagnostic-development.ja.md)。

α版までの機能面の概算は約25％（20〜30％）。ここでは「認証後に記事を作成・編集・保存し、テーマ付き公開サイトへの反映をLinux上で試せる版」をα版の基準として仮置きする。HTTP／DB読取り・復旧試験／WP互換部品と検証基盤は進んだ。認証・管理画面、投稿書込みから公開への接続、Wasmテーマ、検索・移行、Linux配備など大きな利用機能が残る。API件数やテスト件数を互換対象全体の完了率へ換算した数字ではなく、工数・残時間の実測値でもない。次回は管理編集→保存→公開の一連の導線を優先する候補を検討する。

最新specialchars／bundle検証gate（2026-10-27、日本時間）：target/specialchars-and-bundle-check-workspace-check.logで固定PHP61 API・432,780ケース／hooks572／dispatcher405、workspace Rust411件（通常ignore1）・Python149件、Clippy・Linux cross-check・license208観測・実HTTP・DB current hashが成功。特殊文字helperは6 quote styles×2 boolの各1,938件＝23,256追加、条件付きUTF8検査とHTML401／XML5参照を維持し、独立レビューも通過。新one-shot check-bundleは既存admissionを共用し、成功時だけ1KiB以内の集計を返す。失敗はstdout空／固定stderr／exit2、listener無し。3 sampleの集計も独立計算と一致。全native current hashも別途照合し、DB live／独立experimentは再実行せず直前29件・6 fixture各39項目を維持。一般htmlspecialchars／esc_html・esc_attr filters、完全XML／画像検査・公開判定は未完成。[特殊文字](specialchars-development.ja.md)・[bundle検証](bundle-check-development.ja.md)。

最新XML／コメントfeed gate（2026-10-27、日本時間）：target/xml-utf8-and-comment-feeds-workspace-check.logで固定PHP60 API・409,524ケース／hooks572／dispatcher405、workspace Rust402件（通常ignore1）・Python141件、Clippy・Linux cross-check・license208観測・実HTTP・DB current hashが成功。XMLは同じnormalizerの別profileで、既存HTML6,527＋XML7,544件を独立集計し、API数は増やさない。固定PHP観測表、UTF8明示、charset／2globalsの完全復元と独立レビューを確認した。公開準備済みコメントserializerはRSSの要約と全文、Atom content／返信参照、同じRendering ownerを扱い、旧記事bytesを維持する。固定sampleもGET/HEAD・ETag・Range・XML parseを通過・回収。全nativeのcurrent hashも別ログで通過し、DB live／独立experimentは再実行せず直前29件・6 fixture各39項目を維持。公開選別／PII除去／連続公開・一般HTML5／XML sanitizerは未完成。[XML](xml-entity-normalization-development.ja.md)・[コメントfeed](comment-feeds-development.ja.md)。

最新process RSS観測gate（2026-10-27、日本時間）：target/process-rss-observation-workspace-check.logでworkspace Rust393件（通常ignore1）・Python133件、Clippy・Linux cross-check・license208観測・実HTTPとDB current hashを確認した。固定`/proc/self/smaps_rollup`のRssを読む明示APIを4契約で検査し、非LinuxはUnsupported。native Linuxの実読取り、監視・CLI/cache接続、自動容量算式は未実装。WPと全nativeのcurrent hashも別ログで照合し、PHP／DB live／独立experimentは再実行せず、直前60 API・401,980ケース／hooks572／dispatcher405、独立29件・6 fixture各39項目の実測を維持する。[process RSS](process-memory-development.ja.md)。

最新outer protocol／MySQL server再起動全gate（2026-10-27、日本時間）：target/protocol-outer-reference-workspace-check.logで固定PHP60 API・401,980ケース（outer helper7,768追加）／hooks572／dispatcher405が成功。target/protocol-outer-and-mysql-server-restart-full-check.logでworkspace Rust389件（通常ignore1）＋独立29件・Python133件、全6 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPを通過し全fixture回収。WP current hashもtarget/protocol-outer-server-restart-final-wp-provenance.logで最終照合。外側helperは全文NoNull1回・厳密http(s)短絡・最大6反復を維持する。新しい専用MySQL試験はowned server終了→pending child回収→初期化SQL無しの同datadir再起動→全old tupleとsource T2／indexの検査であり、binlog再生・製品CDCではない。初回liveも成功し、server recoveryログを保存。終了失敗時のPopen無期限waitとambient例外誤認を修正し、捕捉handle保持・期限付きIO・外側再回収を独立レビューした。途中の非DB gateが旧DB hashを拒否した履歴は保持し、証跡は全6 fixtureの実測で再生成した。[outer helper](bad-protocol-development.ja.md)・[server再起動](mysql-server-restart-development.ja.md)。

最新protocol once／ETag全gate（2026-10-27、日本時間）：target/bad-protocol-once-reference-check.logで固定PHP59 API・394,212ケース（once helper7,347追加）／hooks572／dispatcher405が成功。target/protocol-once-and-etag-full-check.logでworkspace Rust381件（通常ignore1）＋独立27件・Python113件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・ETag実HTTPも通過し全fixture回収。WP hashもtarget/protocol-once-etag-final-wp-provenance.logで最終照合。新しいserver依存辺sha2 0.10.9(default feature無し)は既存packageを再利用し、155／188／unique208は不変、lock変更後のDB証跡は実測で再生成した。ETagは同じsealed readerのstartup hashで、IfMatch→IfNone→IfRange順。304はアプリでfull長、実GET wireは省略／HEADはfull長という差を観測・記録し、非空表現のCL0は不許可。body/MIME/statusの独立hash一致・process間安定も確認。新たな監視／live公開／認可は未採用。[once](bad-protocol-once-development.ja.md)・[ETag](etag-development.ja.md)。

最新protocol helper／byte Range gate（2026-10-27、日本時間）：target/bad-protocol-once2-reference-check.logで固定PHP58 API・386,865ケース（once2 helper8,158追加）／hooks572／dispatcher405が成功。target/bad-protocol-and-byte-range-check.logでworkspace Rust360件（通常ignore1）・Python112件、Clippy・Linux cross-check・license208観測、実HTTPの206／416／HEAD／redirect維持、DBと全nativeのcurrent hash照合も通過。今回はDB live／独立experimentを再実行せず、直前27件・5 fixture各39項目・全回収の実測を維持する。単一RangeはGET200非空のsealed bytesだけで、部分copyは未課金・cachebypass。条件header／multipart／空本文等は従来応答を保つ。C protocol helperはdecode→white除去→no-null→ASCII lowerとByte値allowlistの限定移植で、URL認可／safetyをclaimしない。サンプルの実PNG部分取得も別processで照合・回収済み。[once2](bad-protocol-once2-development.ja.md)・[byte Range](byte-range-development.ja.md)。

最新KSES slash／typed redirect gate（2026-10-27、日本時間）：target/kses-slashes-reference-check.logで固定PHP57 API・378,707ケース（wp_kses_stripslashes4,364追加）／hooks572／dispatcher405が成功。target/kses-slashes-and-typed-redirect-full-check.logでworkspace Rust344件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・binary/redirect実process HTTPを通過し全fixtureを回収した。WP current hashもtarget/kses-slashes-typed-redirect-final-wp-provenance.logで最終照合。typed LocationはASCII root-relative subset、5 status、空body、単一headerで、cacheを通らず転送先を内部取得しない。版1・旧constructorは不変、外部redirect／WP redirect互換・loop検証・本番認可は未完成。サンプルの308と画像/textも実GET/HEADで照合し回収済み。[KSES slash](kses-slashes-development.ja.md)・[typed redirect](typed-redirect-development.ja.md)。

最新control除去／binary bundle gate（2026-10-27、日本時間）：target/no-null-reference-check.logで固定PHP56 API・374,343ケース（wp_kses_no_null8,324追加）／hooks572／dispatcher405が成功。target/no-null-and-binary-bundle-full-check.logでworkspace Rust332件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・版2を含む実HTTP process smokeが通過し、全fixtureを停止・回収した。target/no-null-binary-bundle-final-wp-provenance.logでWP current hashも最終照合。版1は維持し、版2はexplicit body_hex・textUTF8・PNG/JPEG/WebPの宣言署名だけを扱う。新しいfilesystem／外部fetch・PHP／DB経路は追加しない。サンプル70byte PNGはchunk/CRC/zlibと実HTTP GET/HEADを別途確認し、ユーザーassets/logoは不変。完全画像decoder・metadata除去・本番公開判定、製品CDC・自動容量・native Linuxは未完了。[control除去](no-null-development.ja.md)・[binary bundle](binary-bundle-development.ja.md)。

最新byte decode／MySQL切替再試行 checkpoint（2026-10-27、日本時間）：target/byte-entity-decode-reference-check.logで固定PHP55 API・366,019ケース（数値参照decode7,646追加）／hooks572／dispatcher405が成功。target/byte-entity-decode-and-rotation-retry-workspace-check.logでworkspace Rust319件（通常ignore1）・Clippy・Linux cross-check・HTTP・license208観測を通過し、target/byte-entity-decode-and-rotation-retry-native-check.logで独立Rust27件・Python112件・既存4 fixture各39項目・全current provenanceも成功した。別のtarget/mysql-rotation-owned-cut-second-live-check.logでは専用fixture39項目と厳密39 fieldsが通過。実Rotate後に所有dumpだけを終了し、旧C1の行・索引・checkpoint／poolを保持して新接続から再試行した。全5 fixtureを回収済み。初期FDEのend0と物理FDEのend127を区別し、失敗履歴も保持する。通常decoder／coreは不変、journal・source再起動／保持保証・連続CDC／公開は未完成。[文字実体](entity-normalization-development.ja.md)・[切替再試行](mysql-rotation-handoff-development.ja.md)。


最新Unicode／HTML実体・MySQL切替 gate（2026-10-27、日本時間）：target/unicode-and-default-html-entities-reference-check.logで固定PHP54 API・358,373ケース（valid_unicode3,171／HTML実体正規化6,527追加）・hooks572／dispatcher405が成功。target/unicode-entities-and-mysql-rotation-full-check.logでworkspace Rust313件（通常ignore1）＋独立experiment25件・Python111件、既存4 fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測を通過した。別の専用MySQL切替liveも39項目成功し、合計5 fixtureを回収。target/unicode-entities-rotation-final-provenance-check.logでWP／全nativeのcurrent hashを最終照合した。新候補は実Rotate後の人工通知を専用phaseで検証し、headerでは旧checkpointを維持、file2の成功COMMITだけでRAM viewとfile付き座標を切り替える。通常decoder／coreは不変。本番rotation・journal／source再起動・連続CDC・公開は未完成。HTMLは固定253名称のみでXML／sanitizer未対応。[文字実体](entity-normalization-development.ja.md)・[切替候補](mysql-rotation-handoff-development.ja.md)。


最新zeroise／byte量 gate（2026-10-27、日本時間）：target/zeroise-and-bounded-hr-bytes-check.logが成功。workspace Rust306件（通常ignore1）・Python100件、固定PHP比較52 API・348,675ケース（zeroise2,769／wp_convert_hr_to_bytes2,821追加）／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。binlog／MySQL overlap／Maria bootstrapも別途照合し、DB live／独立experiment再実行なしで直前21件・4 fixture各39項目を維持する。zeroiseは%sの符号前0pad／rawBytesとhostwidth4096・出力cap、byte変換はdefault1024units・非負decimal有効256桁までの明示subset。無制限飽和の7反例(actual0)を保存してdomainを狭め、範囲外42 process拒否を別観測にした。共通int wireもsigned64厳密検証へ補修し、nestedkey含む3契約を追加。[対応範囲](compatibility-development.ja.md)。

最新request hint gate（2026-10-27、日本時間）：target/request-media-hints-check.logが成功。workspace Rust299件（通常ignore1）・Python96件、固定PHP比較50 API・343,085ケース（JSON／XML要求各3,966追加）／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。binlog／MySQL overlap／Maria bootstrapも別途照合し、DB live／独立experiment再実行なしで直前の21件・4 fixture各39項目を維持。明示Option headerのstateless kernelはNone／empty／case／params／NUL／不正byteと、JSON Accept先の短絡・XML Accept substring対Content-Type exactを保持する。SERVER2fieldの完全復元、PCRE first-header error検査、型拒否も別process観測した。q値交渉・本文parse・認可・製品HTTP接続は追加しない。[対応範囲](compatibility-development.ja.md)。

最新JSON media C-profile gate（2026-10-27、日本時間）：target/json-media-type-c-profile-check.logが成功。workspace Rust295件（通常ignore1）・Python96件、固定PHP比較48 API・335,153ケース（wp_is_json_media_type 7,072追加）／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。binlog／MySQL overlap／Maria bootstrapも別途current hashを照合し、DB live／独立experiment再実行なしで直前の21件・4 fixture各39項目を維持する。新predicateはC localeのbyte入力・return boolだけを扱い、元PCREのunanchored境界・特殊ASCII range・optional oembedを線形・追加割当なしで維持する。PCRE errorを成功boolへ混ぜずoracleで拒否し、nonstring／非C／memoization／resource failureは未claim。MIME解析・HTTP認可へ接続しない。[対応範囲](compatibility-development.ja.md)。

最新charset／UTF8 gate（2026-10-27、日本時間）：target/charset-and-invalid-utf8-profile-check.logが成功。workspace Rust291件（通常ignore1）・Python96件、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを通過。47 API・328,081ケース（is_utf8_charset 8,936／wp_check_invalid_utf8 8,782）／hooks572／dispatcher405が固定PHPと一致。review後oracleのbyte-string guardを追加し、target/charset-and-invalid-utf8-oracle-domain-check.logでPHP3比較を実再生成してcurrent hashを照合した。非string56 process拒否とbyte positive21ケースも別観測で確認し正式件数へ加算しない。今回はDB live／独立experiment再実行なし、前回の独立21件・4 fixture各39項目・全回収を維持し、binlog／MySQL overlap／Maria bootstrap current hashも確認。Null／省略option・scalar変換／非bool strip・他charset／mbstring・動的site設定は未対応。[対応範囲](compatibility-development.ja.md)。

最新Maria bootstrap全gate（2026-10-27、日本時間）：target/mariadb-snapshot-bootstrap-full-check.logが成功。workspace Rust287件（通常ignore1）＋独立experiment21件・Python96件、通常MariaDB／MySQL・専用MySQL GTID・専用MariaDBの4 fixture各39項目、HTTP・Clippy・Linux cross-check・license208観測・全current provenanceを通過し全4 fixtureを回収。新Maria候補はsame-session anchor A=4246641→bootstrap3 COMMIT C=4248068→strict1 COMMIT D=4248466、snapshot／最終11行、共有pool callback4と返却、cap／pool／CRC／gap拒否を実測。別途WP hash確認済みでPHP再実行なし、45 API・310,363ケース／hooks572／dispatcher405を維持する。FTWRL／global pauseを追加せず、journal／source再起動／連続CDC／公開／本番bootstrapは未採用。[Maria候補の範囲](mariadb-snapshot-bootstrap-development.ja.md)。


最新HTTP owner gate（2026-10-27、日本時間）：target/budgeted-http-cache-owner-check.logが成功。workspace Rust287件（通常ignore1）・Python89件、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認し、WP／binlog／MySQL overlapも別途current hashを照合した。PHP／DB live／独立experimentは再実行せず、WP45 API・310,363ケース／hooks572／dispatcher405、前回の独立19件・3 fixture各39項目を維持。明示public_router_with_budgeted_cacheだけでcache handleをBytes ownerへ渡し、nonempty Body／Bytes clone／sliceの実owner寿命まで予約を保持する。RenderingとResponseCacheの同pool競合・独立解放も実API同士で確認。empty最適化はowner早期解放可、fallback／準備済み本文／transport等は未課金。既存constructor／CLIはlegacy・default停止、sealed generation・撤回非保証は維持。[HTTP cache開発記録](http-cache-development.ja.md)。

最新共有cache全gate（2026-10-27、日本時間）：target/budgeted-cache-shared-pool-full-check.logが成功。workspace Rust276件（通常ignore1）＋独立experiment19件・Python89件、通常MariaDB／MySQLと専用GTID MySQL各39項目、cold restore／kill／filter／共有pool／overlap、HTTP・Clippy・Linux cross-check・license208観測・current provenanceを通過し全3 fixtureを回収。WP source不変のためPHP再実行なしで45 API・310,363ケース／hooks572／dispatcher405を維持し、別途target/budgeted-cache-wp-provenance-check.logでWP証跡を照合した。BudgetedPageCacheは外部本文handleの最終dropまで全entry予約を維持し、global拒否で旧値／LRUを保つ。新lease slotのEntry header増分を既存logical chargeへ反映した。HTTP／CLIは従来backendを維持し、cache停止・自動算式・全RSS／cross-processは未対応。[cache開発記録](http-cache-development.ja.md)。

最新feed出力gate（2026-10-27、日本時間）：target/budgeted-feed-output-final-check.logが成功。workspace Rust266件（通常ignore1）・Python89件（独立XML5）、HTTP・Clippy・Linux向けcross-check・license208観測・DB current hash検査が通過。別途WP／binlog／MySQL overlapのcurrent hash照合も成功。WP source不変のためPHP再実行なしで45 API・310,363ケース／hooks572／dispatcher405を維持し、DB live／独立experimentも前回の19件・3 fixture各39項目を維持する。render_budgetedは事前生成するRSS／Atomの出力len＋owner headerだけを同一poolのRendering予約へ結び、Arc最終dropまで維持。入力／一時領域／HTTPコピー／RSS・自動容量・CLI cache接続は対象外。[feed開発記録](feeds-development.ja.md)。

最新JSON profile gate（2026-10-27、日本時間）：target/json-utf8-fallback-profile-check.logが成功。workspace Rust256件（通常ignore1）・Python88件、固定PHP比較45 API・310,363ケース（JSON23,365＝従来17,073＋明示UTF8/no-mbstring6,292）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check・license208観測とDB current hash検査を通過した。別途binlog／MySQL overlapのcurrent hash検査も成功。今回はDB live／独立experimentを再実行せず、前回の独立19件・3 fixture各39項目・全回収を維持する。新wp_json_encode_utf8_no_mbstringだけが固定profileのwhole-graph補修とキー衝突を扱い、既存2 APIのinvalid UTF8拒否は変えない。元graph全域の未対応値／64段を先に検査し、補修cloneのメモリは出力cap／共有pool対象外。Floatは反例と設定依存を記録して未対応を維持。[JSON開発記録](json-encoding-development.ja.md)。

最新phase3全gate（2026-10-27、日本時間）：target/shared-buffer-and-pretty-json-full-check.logが成功。workspace Rust251件（通常ignore1）＋独立experiment19件・Python86件、固定PHP比較45 API・304,071ケース（wp_json_encode17,073・9flags全512組合せ）／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208観測を通過し全3 fixtureを回収した。両nativeでpostmeta_shared_pool_commits=3、postmeta_shared_pool_refusal_preserved=true、postmeta_shared_pool_released=trueを確認。postmetaの一poolに行・索引・raw保持を接続した候補で、他ProbeBufferは専用pool。decoder／input／frame比較コピー／position／outer Vec等、全RSS・cross-process・自動容量・CLI cacheは未対応。[共有buffer記録](budgeted-change-buffer-development.ja.md)。旧238／230等のgateは履歴として維持する。

直前phase2の履歴（2026-10-27、日本時間）：target/budgeted-meta-view-and-loader-cache-final-check.logが成功。workspace Rust238件（通常ignore1）＋独立experiment19件・Python86件、固定PHP再比較45 API・299,957ケース／hooks572／dispatcher405、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208 package観測、cargo graph workspace155／probe188を通過。今回はPHP再比較とDB live実測を行い全3 fixtureを回収した。新BudgetedPostMetaViewは同一poolのInternalRows／Indexをdata lifetimeへ結び、old＋new／Arc最終dropと予約失敗時保持を確認。最終論理chargeだけで全RSS・node／capacity／driver／ref map／peak・cross-process・自動配分・CLI cacheは含まない。初回full gate終盤の古いschema helper記録検出、DDL同一の再生成と起動前tool_files guard修正を含む経緯は[予算付きview開発記録](budgeted-meta-view-development.ja.md)。今回MySQL overlap prefix134はfixtureの追加2 write由来で、旧132は歴史として維持する。

直前の検証履歴（2026-10-27、日本時間）：target/json-selected-flags-full-check.logが成功。workspace Rust230件（通常ignore1）・Python79件、固定PHP比較45 API・299,957ケース（wp_json_encode 12,959件、うち新flags付き7,656件）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check・dependency license208 package観測・DB／binlog／overlap current hash検査を通過した。8種類のJSON flagsの全256組合せを比較し、PHP定数の型・値と組合せ別件数の完全性も検査する。今回live DB／独立experimentは再実行せず、前回の独立19件・3 fixture各36項目の実測を保持。native Linux、製品CDC、自動容量算式の完成を示さない。

**再開中（2026-10-27、日本時間）**：ユーザーの「再開」により停止を解除した。JSON selected flags／PRETTY／明示UTF8 profileの実装・固定PHP比較を終え、共有予算への追加接続候補を調べている。サブエージェントは最大2本。未決方針や製品の対応範囲は先取りしない。

**前回停止時の記録（2026-10-27、日本時間）**：ユーザーの「切りのいいところで停止」に従い、下記のdefault JSON全gateと引継ぎ保存を区切りに停止した。再開は新しいユーザー指示後とし、以下の旧「ノンストップ」指示を自動再開の根拠にしない。commit／push／配備なし、稼働fixtureは0。

停止時の最新gate：target/json-encoding-and-license-check.logが成功。workspace Rust224件（通常ignore1）・Python77件、実PHP再比較45 API・292,301ケース（wp_json_encodeの5,303ケース追加）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check、dependency license208 package観測、DB／binlog／overlap証跡current hash確認を通過した。今回はPHPを再実行した一方、DB live／独立experimentは再実行せず、直前target/limited-index-native-check.logの独立19件・通常MariaDB／MySQLと専用GTID MySQL各36項目、全3 fixture回収の実測を維持する。Linux native実行・製品CDC・自動容量やlicenseの法的判断を完了したものではない。

再開候補の位置：JSON本体はdefault flags=0のsubsetまで。selected flagsはtarget/json-selected-flags-observation.php／.jsonへ固定PHPの1,792観測を保存しただけで、wpagentの拡張draftは未作成・未統合。source／registry／正式proofへ採用していない。default JSONの76ケース探索はtarget/json-encode-observation.php／.jsonl。再開後に必要性を検討する候補で、自動的に着手しない。

更新日：2026-10-27（日本時間）。ユーザーの「利用制限に当たるまでノンストップで開発」「確認が必要な点はメモして迂回」という指示に基づき、実装を開始。過去の「別途の開始指示待ち」はこの指示で解除。未採用案を新たな製品要求へ格上げしない。

DB作業の区切りで一度停止した後、ユーザーの「DB領域以外で続行して」に従い再開。その後の「DB周りの仕様書を更新したので追従。他の部分もノンストップで続けて」で作業範囲を更新。R59～R61を採用仕様として扱い、DBと非DBを継続する。DB全機能の完成を意味しない。

## 実装の一覧と初期記録

この節の件数・列挙は初期実装の記録である。現mainの採用状態と検証待ちは冒頭、認証の適用範囲は認証設計、最新の完成履歴はhandoffを参照する。

直前の検証履歴（2026-10-27、日本時間）は2 gateに分ける。target/limited-index-native-check.logで独立experiment19件・Clippy、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、cold restore／pre-COMMIT kill／filter／limited changes・index各3 COMMIT／overlapが成功し、全3 fixtureを回収。target/limited-index-and-license-check.logでworkspace Rust219件（通常ignore1）・Python77件、Clippy・Linux向けcross-check・HTTP、dependency license208 package観測、DB／binlog／overlap／WP証跡current hash確認が成功。WP44 API・286,998ケース／フック572／dispatcher405は維持しPHP再実行なし。licenseは203 locked archive／364 cache file一致、checksum JSON missing203・候補なし4・local5の観測で、法的判断未完了。索引も最終logical bytesだけの制約で、node overhead・capacity・rows・peak／RSS・shared budget／自動算式は対象外。

以下の214件の全gateは直前の履歴。

直前全gateの履歴（2026-10-27、日本時間）：`target/limited-meta-changes-full-check.log`でworkspace Rust214件（通常ignore1）＋独立experiment19件・Python68件、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、cold restore／pre-COMMIT kill／同DB選別／論理上限0拒否と3 COMMIT／overlap、HTTP・Clippy・Linux向けcross-check・registry current hash検査が成功。両binlog証跡のpostmeta_logical_limit_preserved=true／postmeta_limited_commits=3を照合し、全3 fixtureは回収済み。WP44 API・286,998ケース／フック572／dispatcher405はcurrent hash検査で維持し、今回はPHP再実行なし。上限は最終candidateの行構造体＋payloadだけで、old/new同時保持・changes・参照map・索引・capacity・RSS／peak・shared budgetや自動算式は含まない。[DB開発記録](database-development.ja.md)

直前WP gateの履歴（2026-10-27、日本時間）：`target/unique-id-and-visible-cgroup-check.log`でworkspace Rust209件（通常ignore1）・Python68件、実PHP比較44 API・286,998ケース／フック572／dispatcher405、HTTP・Clippy・Linux向けcross-check、通常binlog／MySQL overlapのcurrent hash検査が成功。UniqueIdsの2 APIを各2,069ケース追加し、可視cgroup観測候補の新unit5件も含む。このrunは実DB／独立experimentを再実行せず、下記の直前DB gateを保持した。

直前DB実測の履歴：`target/mysql-overlap-bootstrap-full-check.log`が成功。workspace Rust194件（通常ignore1）＋独立experiment19件・Python68件、通常MariaDB／MySQL各36項目と専用GTID MySQL36項目、HTTP・Clippy・Linux向けcross-check、registry current hash照合を通過した。WP41 API・274,683ケース／フック572／dispatcher405は既存比較証跡を維持。MySQL重複履歴bootstrapの独立候補では132 prefix group・3 bootstrap COMMIT・1 strict COMMIT・12行のraw／索引一致を確認した。全3 fixtureは停止・回収済み。非prefix Aは実runtime selectorを通す純粋試験で、製品bootstrapやnative Linux実行の完成とはしない。

過去の非DB gateの履歴：`target/locale-mime-and-filtered-postmeta-check.log`でworkspace Rust194件（通常ignore1）・Python61件、実PHP比較41 API・274,683ケース／フック572／dispatcher405、HTTP・Clippy・Linux向けcross-check、DB／binlog current hash検査が成功。sanitize_locale_name／sanitize_mime_typeを各4,095ケース追加した。今回は実DB・独立experimentを再実行せず、直前target/filtered-postmeta-and-cgroup-full-check.logのRust190件＋独立experiment13件・両DB各36項目・cold restore／pre-COMMIT kill／same-DB選別の実測を保持する。所有fixtureの停止・回収、db-runs空、過去失敗ログ保存の履歴も維持する。

2026-10-26の追加承認で[DB自動最適化・変更検知](database-auto-optimization.ja.md)を仕様書へ反映（D07-02、R59～R61、T44、詳細設計v0.44）。この段落は文書反映時点の記録。現在はユーザーの継続指示に従い、以下に記載した実装・専用fixture試験を進めている。製品の内部データ・自動索引・連続CDCの完成は未宣言。対応版・監視条件・反映期限等の残件は同仕様第7節を参照。

Rust 2024のCargoワークスペースを追加。`recastpress-core` は外部依存なしで、DB・PHP・ネットワークを操作しない。

- 既定設定：キャッシュon、SSO off、新規コメント受付off。移行元のコメント受付状態を保持できる。容量の手動設定なし。
- ローカルキャッシュ：クエリーを含む要求キー、TTLなし、論理メモリ予算、LRU退避、全パージ、監視断時の保存・ヒット停止、再照合後の復帰。停止中も利用者のon設定を維持。
- 生成チケット：パージ・停止・予算縮小をまたぐ登録を拒否。別キャッシュのチケットも拒否。
- 公開世代：事前構築した公開ページ一式の原子的置換・撤回、改訂の競合検出、読取り専用ハンドル。公開ハンドルには編集・PHP実行・ジョブ投入機能なし。
- 配信直前の世代再確認：古い応答の配信を拒否し、同期配信コールバック中は世代変更を待たせる。送信済みデータの回収は保証しない。

`recastpress-server`にaxumによるHTTP骨格を追加した。`public`と`management`を別プロセスで起動し、実ソケットで役割分離とHTTP動作を確認。公開は起動時の不変なページ束だけを読み、管理は認証未実装のため操作APIを提供せずready=503。起動手順は[開発版の起動](running.ja.md)。

公開ページ構造は静的な実験用契約。Wasm実行、WordPressテーマ描画、実行中の公開切替用IPC、管理画面は未実装。公開プロセスは編集DBへ接続しない。構造検査をテーマ互換性検証と混同しない。キャッシュはHTTP経路へ接続したが、実監視と自動容量算出がないCLI起動では保存・ヒットを停止したまま。HTTP送信の完了をResponse構築と混同しないため、実サーバーでは公開用Publisherを起動前に破棄している。

合意済みのWordPress 7.1.3公式tar.gzを取得し、固定SHA-1一致・PHP内の版番号・展開ファイルごとのSHA-256を記録した（3,782ファイル、アーカイブSHA-256 `d2a09acb6a15e3b9c471d72557753c266d6f41a79ed19bfe04cbfe49e283b2a5`）。参照物は`.reference/`へ保存しGit対象外。取得・照合ツールは`scripts/wordpress_reference.py`。PHPの実行はしていない。

その後、開発用PHP 8.5.12 CLIを公式SHA-256で固定して`.reference/`内へ取得し、`recastpress-wp`の44 APIの対応入力を実行比較した。286,998ケースで一致。normalize_whitespace／wp_html_splitはbyte入力に限定し、Unicode正規化・HTML parser・出力escapingの保証ではない。JSONP判定は実測・強制したC localeのbyte規則だけを扱い、一般のJavaScript構文検査・安全性検査とはしない。wp_parse_list／wp_parse_id_listのroot bool・signed integerを追加したが、root NullはPHP 8.5のE_DEPRECATED 8192を伴うためUnsupportedを維持する。WP全体をbootstrapせず、固定ソースから抽出した対象関数と実際のWPフック処理を使用。NaN等の未対応診断も明示した。詳しくは[互換実装の開発](compatibility-development.ja.md)。

リクエスト内のWP_Hook相当部分を追加。登録順・優先度・実行中の変更・引数数・入れ子について、実WP_Hookと572シナリオが一致。全削除後の再登録と入れ子の現在優先度の差を実行比較で見つけて修正した。その後、名前付きdispatcher・allフック・実行回数・実行中スタックを追加し、405シナリオで実WPのグローバル呼出しと一致。22シナリオの80警告も型付き診断として一致した。参照配列・object同一性・Wasm呼出しは未実装。

固定参照の全PHP 1,512ファイルを実行せずトークン解析し、13,014宣言候補と5,059フック呼出し候補を索引化した。関数4,461・メソッド7,714へ分け、名前空間・所有クラス・可視性・doc注記を分類した。テーマ向け公開APIの採用と母集合の確定は未完了。再現ツールはscripts/inventory_wordpress.py、集計はcompatibility/inventory-summary.json。

編集用DBリーダーをrecastpress-dbへ追加。固定WPから単一サイト用12表のDDLを生成し、専用MariaDB 11.8.9／MySQL 8.4.13で各34項目を実試験した。posts・postmeta・optionsのraw値、NULL／重複／孤立meta、ゼロ日付、uint64、読取り専用スナップショット、非InnoDB／欠落表の拒否を扱う。詳細は[DB開発メモ](database-development.ja.md)。SQLx 0.9.0に合わせ、開発用rust-versionは1.94へ更新（実測1.96.0）。製品の正式な対応版・配置は未決のまま。

DB以外の継続開発として公開HTTPキャッシュを接続し、監視／容量未確認時の停止、認証・未知ヘッダー／クエリーの除外、ウォーム済み本文の非再利用を試験した。CLIの実キャッシュは停止状態。詳細は[HTTPキャッシュ開発メモ](http-cache-development.ja.md)。wp_parse_list・wp_array_slice_assoc・wp_parse_argsの配列入力・wp_slash／wp_unslashの対応値を実装し、固定WPの実関数との比較へ追加した。ヘルス経路の405にもno-store・nosniffを付けるよう、ルーター全体へ応答保護を適用した。

R03のGPL-2.0-or-later宣言に合わせ、ルートLICENSEの旧GPLv3本文を`LICENSES/GPL-3.0.txt`へ保存し、GPLv2本文とlater許諾へ揃えた。移植元の表示は`NOTICE.md`。

## 確認事項と迂回

| ID | 確認事項 | 扱い |
| --- | --- | --- |
| DEV-01 / D08 | 直接SQL・構造変更・プラグイン権限 | 未採用のまま。PHPホスト／DB権限を追加せず進める |
| DEV-02 / D06 | SSOの一度限りの消費・失効後の回復 | 3モードの設定型のみ。認証・回復は保留 |
| DEV-03 / D19 | 容量算式・共有予算・監視方式・反映時間 | 単一process論理pool・Linux host RAM・caller指定cgroup v2の1 level・限定self membership／可視祖先の観測候補まで。外祖先Unknown・NonAtomicで、namespace完全性／競合排除・native Linux・実監視・job／RSS・配分算式・CLI cache接続は未完了。[資源観測の境界](cgroup-memory-development.ja.md)。unsafe forbidを維持し、Windows safe wrapper依存の採用は未決 |
| DEV-04 / D04 | Wasmエンジン・契約・配布ライセンス | エンジン依存をまだ追加しない |
| DEV-05 / D02 | 固定ソースからのAPI抽出・比較試験 | 公式配布物の取得・チェックサム照合は完了。API互換性や比較試験の完了とは区別 |
| DEV-06 / D18 | 自動更新の初期値・スキーマ変更時の方式 | 既定値を推定せず、更新実行は保留 |
| DEV-07 / D14 | 遅延・負荷・費用・撤回の数値目標 | 数値保証を設定せず、状態遷移を先に検証 |
| DEV-08 / D02 | PHPの型変換時の警告・診断 | NaNのboolean変換はPHP 8.5で警告が出る。診断契約が実装できるまでUnsupportedValueとして明示し、比較済み範囲から除外 |
| DEV-09 / D07 | DBの正式対応版・配置・charset | 今回の2版はWindows開発fixture。製品の対応版・Linux配備・legacy charsetの採用判断へ流用しない |
| DEV-10 / D12 | 非トランザクション表の一貫移行・同時DDL | 試作のsnapshotは非InnoDBを拒否。書込停止・ロック等の移行方式は未決のまま |
| DEV-12 / Linux | native Linuxの実行環境 | WSL未インストール。cross-checkのみ確認済み。OS機能を追加せず、実シグナル試験は保留 |
| DEV-13 / R60 | MariaDBのstandalone互換通知 | # Dumを未分類SQLとして拒否。正の分類器を緩めず、正式なGTID／再接続プロファイルは後続 |
| DEV-14 / WP数値表示 | size_formatのtranslation／locale／filter／float丸め | identity翻訳等のtarget観測だけ。default localeを選ばず、今回は純粋zeroiseと限定byte変換へ迂回 |
| DEV-15 / WP entity | XML named entityのPHP default_charset／HTML5変換表 | child PHP観測でUTF8とISO8859-1の返却差を確認。XML／_wp_specialchars／esc_html全体は未採用、純粋Unicode predicate／default HTML entity集合を調査 |
| DEV-11 / R60 | 初期snapshotとログ開始位置の取得方式 | 本体へFTWRL APIを公開する案は自動承認レビューで拒否。サーバー全体の書込み停止と失敗時の影響が理由。追加せず、方式の選定を保留して読取り・互換性の開発を続行 |

内部契約は変更可能な仮実装。未決の製品方針や新しいR要求を承認したものではない。

## 検証

以下は個別commandの参考。現在の再開順は冒頭の台帳実再生成→全体gate→保存結果比較→最終12checkerであり、この一覧の一部だけの成功を現ソース全体の成功とはしない。

環境：Windows、rustc/cargo 1.96.0、Python 3.13。coreは外部依存なし。serverはaxum 0.8.9、dbはSQLx 0.9.0等をCargo.lockで固定。全推移依存の個別ライセンス監査は未完了。

```powershell
cargo fmt --all -- --check
cargo test --workspace --offline --locked
cargo clippy --workspace --all-targets --offline --locked -- -D warnings
python -m unittest discover -s scripts -p "test_*.py" -v
python scripts/smoke.py
python scripts/compare_primitives.py
python scripts/compare_hooks.py
python scripts/compare_dispatcher.py
python scripts/compatibility_registry.py
```

直前WP検証の履歴：workspace Rust209件（実DB試験1件は通常ignore）・Python68件、WP44 API・286,998ケース／WP_Hook572／dispatcher405の実PHP比較、HTTP・Clippy・Linux向けcross-checkと証跡hash照合が成功。ログはtarget/unique-id-and-visible-cgroup-check.log。このrunは実DB／独立experimentを再実行せず、直前mysql-overlap-bootstrap-full-check.logのRust194件＋独立experiment19件、通常MariaDB／MySQLと専用GTID MySQL各36項目を保持した。Linux native実行・三配備先・Wasm・WP全体／テーマ／プラグインとの比較は未実施。

直前非DB検証の履歴：最新検証はworkspace Rust194件（実DB試験1件は通常ignore）・Python61件、WP41 API・274,683ケース、WP_Hook572／dispatcher405の実PHP比較、HTTP・Clippy・Linux向けcross-checkと証跡hash照合が成功。ログはtarget/locale-mime-and-filtered-postmeta-check.log。実DB／独立experimentは今回再実行せず、直前filtered-postmeta-and-cgroup gate（Rust190件＋独立experiment13件、両DB各36項目と復元／選別fixture）の記録を保持する。Linux native実行・三配備先・Wasm・WP全体／テーマ／プラグインとの比較は未実施。

## 非DB開発の検証済みチェックポイント

2026-10-26：`python scripts/check.py --with-reference --with-process-smoke`が成功。Rust 56件・Python 12件、Clippy、固定WPのプリミティブ4,277ケース／フック282／dispatcher 213、索引の整合、実HTTPを確認。DBコード・スキーマ・実DB証跡は変更せず、今回DBサーバーは起動していない。現在の検証ログはGit対象外の`target/non-db-check.log`。実監視・自動容量算出がないためCLIのキャッシュ実利用は停止したまま。

## 次の作業

R59～R61のDB仕様と他領域の継続開発中。DBの残件は[DB開発メモ](database-development.ja.md)を参照。

1. 公開応答とキャッシュを接続済み。自動容量算出・実監視・ライブ世代切替は未完了。
2. 公開／管理プロセスの認証・IPC設計。骨格とヘルスチェックは実装済み。
3. 固定した基準ソースの候補を公開APIへ分類し、対応一覧・比較を広げる。取得検証・字句索引・部分的な実行比較は実装済み。
4. 未決事項に依存しない契約・失敗系試験の拡張。

既存のREADMEとdocs（ロゴ資料を含む）は保持。commit/push/外部配備は行っていない。

## DB仕様更新後のチェックポイント

ユーザーの最新指示でDB領域を再び実装対象に含めた。R59～R61の採用方針へ追従し、binlog設定を変更せず観測するAPIを追加。MariaDB/MySQLは各14項目、`check.py --with-reference --with-process-smoke --with-databases`が成功。観測はCDC成立・索引完成の証明ではない。非DBでは公開ターゲットの8,192 byte上限を準備時とHTTPで共有し、上限ちょうどの配信と1 byte超過の拒否を試験した。追加の境界試験・Clippyも成功。未決のbinlog購読ライブラリ、位置とスナップショットの対応、COMMIT・永続位置・索引世代・復帰の詳細を引き続き扱う。

## 変更追跡の保留処理チェックポイント

change_bufferでCOMMIT単位のイベント保留・反映失敗時の再試行・永続位置の遅延更新・DDL／切断／欠落／予算超過時の破棄を実装。7件の抽象イベント試験を追加し、全体検証を通過。実binlog購読と永続データ反映はまだ接続しておらず、監視成立・CDC完成とは扱わない。詳細は[DB開発メモ](database-development.ja.md)。

## 実binlog試作チェックポイント

独立experiments/binlog-probeでMySQL／MariaDBの実ログを読み、各17イベント・3行変更／COMMIT・16 CRC32、前後値とROLLBACK除外を確認。対応イベント版の差を記録。`check.py --with-reference --with-process-smoke --with-binlog-probes`と試作Clippyが成功。実装は有限fixture用で、ChangeBuffer・永続ストア・公開／キャッシュへ未接続。次はログ位置・snapshotの対応、source識別と再開、イベントの厳格分類とトランザクション保留の接続を進める。

## プロセス終了のチェックポイント

Unix SIGINT／SIGTERMをlisten前に登録しgraceful shutdownへ接続。WindowsのCtrl+C受信エラーを黙って捨てずログに残す。`check.py --with-process-smoke --with-linux-check`が成功。既存Rust toolchainへ公式x86_64-unknown-linux-gnu標準ライブラリを追加してcross-checkした。Linuxのリンク／実行・シグナル受信・三配備先・終了期限は未検証。

## 先頭カーソルと破損拒否のチェックポイント

posts／postmeta／optionsにカーソルなしの先頭ページを追加。ID=0の取りこぼしを防ぎ、Some(0)・既存after(0)は従来どおり0を除外する。両DBの実試験は各15項目へ増加。実binlogの複製に対するCRC破損拒否も両DBで成功。制御・購読のTLS確立をそれぞれ確認した。`check.py --with-process-smoke --with-binlog-probes --with-linux-check`が成功。WP比較・索引証跡は直前の全体検証で整合済み。

## 実イベントと原子的ジャーナルのチェックポイント

実binlogをChangeBufferへ接続し、両DBで3つのCOMMIT済みバッチを処理した。実イベント列のCOMMIT直前打切りと1フレーム欠落を検査し、保留行を破棄して古い位置を維持することを確認。

MariaDBの初回人工Rotateでは、FDE到着前のCRC32がファイル名末尾へ混入する違いを検出。フレーム全体をCRC32文脈で再解析・照合してから名前を検査する。MariaDB既定の内部end_log_pos=0は、トランザクション内部だけ位置を累積し、明示的なXID位置の一致を必須とした。legacy設定は変更しない。CRC検証はMySQL16件、MariaDB17件になった。

ChangeBufferへ非同期反映APIを追加し、COMMIT応答紛失・キャンセル時はRAM位置が古くても永続側は進んでいる場合があることを契約へ反映。9件のモデル試験を実施。専用SQLジャーナルでは、イベントと位置の同時COMMIT、応答紛失後の同一バッチ再試行、異なる内容の同位置再送拒否を確認。3バッチ・同一再試行1件で保存行数は3のまま。

`check.py --with-reference --with-process-smoke --with-binlog-probes --with-linux-check`が成功。Rust66件・Python12件、両DBのraw試験各15項目、WP比較、Linux向けcheckを維持した。ジャーナルは架空表・固定した有限ログ区間だけの試験で、製品のデータ投影・永続スキーマ・連続購読の完成ではない。

## 分類データ読取りのチェックポイント

terms・term_taxonomy・term_relationships・termmetaの4表を追加し、扱う表は7表になった。両DBの各19項目が成功。複合主キーでのページ送り、ゼロ／最大ID、符号付き境界、NULL・空・重複meta、孤立参照と同時更新を確認した。`check.py --with-reference --with-process-smoke --with-binlog-probes --with-linux-check`が成功。ログはGit対象外のtarget/taxonomy-full-check.log。

## 一覧フィールド抽出のチェックポイント

wp_list_pluckの配列行・整数／バイト列fieldとindexを実装。491ケースを追加し、固定WPの実関数と合計4,768ケースが一致した。Rust70件・Python12件、フック282／dispatcher213、Clippy・Linux向けcheck・HTTPが成功。DBコードは直前の各19項目の結果と整合。ログはtarget/pluck-full-check.log。

## 既存コメント保持のチェックポイント

comments全15列とcommentmetaを追加し、9表の存在・InnoDBを検査する。承認状態や種別で行を落とさず、元のバイト列・ゼロ日付・孤立メタを保持する。両DBの各22項目が成功。Rust71件・Python12件、Clippy・実binlog試作・HTTP・Linux向けcheckが成功した。新規コメント受付や公開判定を実装したものではない。

## 固定依存台帳のチェックポイント

本体155・独立試作187、計208個のパッケージ宣言を各Cargo.lockへ照合する台帳を追加し、通常check.pyへ接続した。registryのchecksum・source／版とlicense_fileの境界を検査し、ローカル絶対パスを記録しない。Python15件・Rust71件とWP比較4,768ケースが成功。ライセンスの選択・バイナリー単位の義務確認は未完了。詳細は[固定依存の宣言台帳](dependency-inventory.ja.md)。

## 12表の読取りチェックポイント

users・usermeta・linksを追加し、固定した単一サイト用12表すべてのrawページ走査が揃った。各表の欠落・非InnoDBをsnapshotで拒否する。資格情報・アカウント情報はDebugへ出さず、capability等の値を解釈して権限判定しない。両DBで各28項目、Rust73件・Python15件、Clippy・binlog・HTTP・Linux向けcheck・依存台帳が成功した。公開用データ生成、連続CDC、snapshotとログ位置の対応は残件。

## パス・日付・起動失敗のチェックポイント

trailingslashit／untrailingslashitのバイト列処理、wp_normalize_pathの非stream入力、wp_checkdateの整数入力と明示的フィルターを追加。固定WPの実関数と合計12,947ケースが一致し、Rust78件・Python15件・Clippy・HTTP・Linux向けcheck・依存台帳が成功した。ログはtarget/date-and-process-check.log。

HTTP実プロセスでは欠落・破損・未知版・重複ページ・予約経路衝突・未知field・8 MiB超過のバンドル、役割不正引数、使用中ポートの起動拒否も確認。バンドル省略時のlive=200／readyと公開応答=503を検査した。stderr読取りスレッドはプロセス終了後にjoinしてから閉じる。Unix SIGINT／SIGTERMの正常終了・ハンドラーログ確認は試験へ追加したが、このWindows環境では未実行。WSLも未インストールで、OS機能の追加を行わず保留する。

## シリアライズ判定のチェックポイント

is_serialized／is_serialized_stringを追加し、API台帳は15件、固定WPとの比較は合計21,950ケースになった。strict／loose、PHP trimのNUL・vertical tabとform feedの差、長さ・本文・数値を厳密に検証しないWPの挙動を保持する。デシリアライズやDB値の変換は行わない。Rust81件・Python15件、Clippy・PHP比較・HTTP・Linux向けcheckが成功。ログはtarget/serialization-check.log。

## 実DDL区間の拒否チェックポイント

所有確認済みの架空表だけをALTERした別の有限binlog区間を検証。MySQLは実ALTERのQueryEventでSchemaChangedとなる。MariaDBでは先行する互換通知が短い# Dumとなり、現行の分類器は未分類SQLとして先にSchemaChangedで停止する。許可規則を広げず、負の観測用プローブだけがその後のALTERを確認し、停止状態が維持されることを検査した。両者の拒否点は証跡のddl_prefix_rejectedで区別する。

いずれも旧checkpoint・保存行なしを維持し、同イベントの再送やfinishで再開しない。MySQL／MariaDBの各28項目、実binlogの3バッチと応答紛失後の重複防止を維持した。独立試作のfmt／Clippy、DB／binlog証跡確認も成功。ログはtarget/ddl-maria-rejection.logとddl-mysql-rejection.log。失敗fixture2件は停止確認後にログを保存して回収済み。MariaDBのstandalone GTID／互換通知の一般対応・DDL後の再構築・公開／キャッシュ失効は未完了。

## 最終証跡の整合確認

DB証跡は28という件数だけでなく必須の検証名すべてを照合し、同数の別項目への置換・重複・欠落も拒否するよう補強した。Python16件が成功。WP台帳・固定依存・DB・binlogの現在ファイルとの整合、Rust fmt、git diffの空白検査も成功。一時DBの実行フォルダーは空。最新のコード・結果・保留事項をhandoffへ保存した。

## 接続再開・欠落ログ要求のチェックポイント

最初のCOMMIT後にbinlog購読を閉じ、ジャーナル用SQL接続も新しい接続へ置換した。DBから読み直したcheckpointで新しいデコーダーを作り、残り2 COMMITだけ取得する。同一fixture内の接続再開を両DBで確認し、保存結果は3バッチ・3行、同一再送は1件のまま。異なるsource epoch／ログファイルを期待するcheckpoint読取りは拒否する。再接続の人工Rotate／FDEも確認するため、主区間は19イベント、CRCはMySQL17／MariaDB19となった。

存在しないことをログ索引で確認したファイルを要求し、両DBのserver error 1236／HY000を確認。通常EOFやtimeoutは成功扱いにせず、失敗要求の前後でSQLジャーナルと位置が変わらないことも検査する。mysql_async 0.37.1の汎用is_fatalはこのServerErrorでfalseだった。この値を購読の健全性へ流用しない。

試験結果をversion付きJSONへ変更し、欠落・重複key・booleanと整数の混同・不整合なイベント件数・backend差を厳密に検査。Python19件、Rust81件、Clippy・両DB各28項目・HTTP・Linux向けcheck・依存台帳が成功した。ログはtarget/missing-log-binlog-check.log。serde_json 1.0.154を独立試作の直接依存として明示し、本体の依存は変更していない。宣言台帳は208パッケージのまま整合済み。

これは同じ稼働fixtureでの接続再開と欠落ファイル要求の試験。プロセス／サーバーのクラッシュ・再起動、source置換、実ログ削除、安定した製品source identity、連続購読・再スナップショット・公開失効は未検証または未実装。

## 行型の境界値とID一覧のチェックポイント

実binlogの別区間で、固定4列の架空表へ0／1／i64::MAX／2^63／u64::MAXのID、signedの最小・最大値、NULL／空／NUL・非UTF-8、UTF-8本文を入れた。SQL列定義とTableMapの型・NULL bitmapを照合し、5行が1 COMMIT境界で復元されることを両DBで確認。MariaDBの大きなunsigned値2件はValue::Intの負値として現れ、既知のBIGINT UNSIGNED列だけでbit解釈が必要だった。MySQLの同件数は0。一般的な型デコーダーやschema cacheの実装完了とは扱わない。ログはtarget/row-types-binlog-check.log。

非DBではabsintとwp_parse_id_listを追加。整数・十進整数文字列・null／boolean／配列の対応castと、string／arrayのID一覧を実装した。元キー・重複除去後の穴、PHP_INT_MINの絶対値がfloatの2^63になる挙動を保持し、PHP_INT_MAXとの誤った重複除去を避ける。float・小数／指数／prefix-junk／overflow文字列等の未対応変換はUnsupportedValueを返す。固定WPの17 API、24,395ケースが一致。Rust86件・Python19件・Clippy・HTTP・Linux向けcheckが成功。ログはtarget/id-coercions-compatibility-check.log。

## 条件付きシリアライズと比較バッチのチェックポイント

maybe_serializeを追加。対応配列はキー順・整数／boolean／null・文字列のbyte長を保持してシリアライズし、既存のserialized形状を持つ文字列は元の空白とNULも含めて二重シリアライズする。floatを含む配列・object／参照はUnsupportedValue、配列の深さ64超はホスト側RecursionLimitとして明示。通常の非配列値は変更しない。DB読取りへ自動適用するものではない。

ケース増加で一括PHP比較が既定128 MiBに達したため、メモリ上限を変更せず、各バッチ2,048ケース・入力1 MiB以内に分けた。15バッチ、最大入力1,047,644 byteで27,711ケースが一致。IDの全体一意性、バッチごとのPHP版・64bit、全結果の順序と既存の全体JSON hash形式を検査する。結果を全件保持せず、hashを逐次更新する。入力budgetは比較用fixtureの制約で、製品のメモリ上限・自動容量式ではない。

Rust89件・Python21件、Clippy・WP比較・HTTP・Linux向けcheckが成功。API台帳は18件で、欠落／未知API・不正な件数・batch観測の型／上限も証跡検査で拒否する。ログはtarget/maybe-serialize-batched-check.log。

## 正規形の限定復元チェックポイント

maybe_unserializeに、正規形のnull／boolean／signed整数／バイト列／配列の復元を追加。外側だけをPHP trimし、1段だけ復元する。文字列内にserialized形式があっても再帰的に解釈せず、クラス生成や参照の展開を行わない。再シリアライズして元の正規形と一致することを要求し、認識済みの不正・非正規形・余剰データ・float・object／参照はUnsupportedValueとする。PHPのunserialize失敗値falseや警告まで再現したとは主張しない。

宣言された長さ・件数を入力へ照合し、その件数を使って先にメモリ確保しない。復元配列64段の上限、65段の拒否、長さ・件数・active形式の拒否、byte改変でpanicしないことも試験。19 API・29,335ケース、Rust94件・Python21件、Clippy・HTTP・Linux向けcheckが成功した。ログはtarget/canonical-unserialize-full-check.log。DBのraw値や移行処理へ自動適用していない。

### 型付きフックと実API接続（2026-10-26）

共通の開発用値codecを抽出し、null・bool・64bit整数・float・任意bytes・順序付き配列をフック比較へ追加。WP_Hookは従来282＋型付き290＝572シナリオ、名前付きdispatcherは従来213＋型付き192＝405シナリオで一致した。既存22シナリオの80警告も維持。配列戻り値がactionの元引数を置換しないことと、allの戻り値が対象filterを変えないことも確認した。

sanitize_key／wp_checkdateをリクエスト内Dispatcherへ接続し、元bytesの引数順序と、配列・nullなどfilter戻り値の型を保持する。カーネル関数も残し、Wasm転送契約は導入していない。入力領域は従来と同じ文字列／整数部分集合。57ケースを追加し、19 API・29,392ケースで一致。Rust100件・Python23件、Clippy・HTTP・Linux向けcheckが成功。ログはtarget/filtered-dispatcher-full-check.log。DB実装・公開HTTPへの組込みは変更していない。

### 互換証跡の必須ファイル照合（2026-10-26）

証跡に列挙されたファイルだけを検査すると、必要な項目を省いた証跡でも通る余地があった。検証側で必須ファイル集合を組み立て、実装全module・契約試験・共有codec・比較harness・Cargo設定とlockを省略／差し替えできないようにした。WP参照ソースの集合も照合し、重複JSONキーを拒否する。証跡生成側も同じ集合を使う。省略・差し替え・重複の拒否を試験し、Rust100件・Python23件と全実PHP比較が成功。ログはtarget/strict-compatibility-evidence-check.log。実行比較の対象範囲自体は拡大していない。

### 内部索引とパス処理のチェックポイント（2026-10-26）

投稿メタの任意キー索引を追加。NULL／空／重複／孤立行を保持し、索引なし・件数制限による破棄後も同じ走査結果を返す。完全な更新前イメージを照合して変更後の別行集合を準備し、途中失敗で旧行・旧索引を変更しない。両DBのbinary比較および更新後snapshotと一致し、各32項目へ拡大。通常collation・SQL自動変換・連続CDC・公開切替・自動容量管理は完成していない。詳細はdatabase-development.ja.md。

wp_basenameのbyte-string path／suffixを追加。WPはpathだけをURL encodeしてからbasenameへ渡すため、suffixは未encodeのまま照合する。空白・percent escapeの途中切断・NUL／非UTF-8・両slashを含む8,495ケースで一致した。ファイルアクセスやパスの認可を行わない。

最終確認：Rust108件・Python24件、20 API・37,887ケース、フック572／dispatcher405、Clippy・HTTP・Linux向けcheck・両DB／binlogが成功。ログはtarget/basename-reference-check.logとtarget/meta-view-prepared-changes-check.log。依存追加なし。新しい製品仕様の採用・commit・push・配備は行っていない。

### 行集合の完全構築とUTF-8のチェックポイント（2026-10-26）

load_post_meta_viewは、同じ読取り専用snapshot内で全postmetaをページ送りし、件数上限を超えれば行集合を返さない。上限と同件数の場合も1行の超過確認を行い、不完全なprefixを完成扱いしない。上限0／10／11／usize::MAX、ページ1／3／32、ゼロ・最大IDと拒否後の通常読取りを実DBで確認。両DB各34項目、既存binlog・HTTP・Linux向けcheckが成功。4,096行と1,027のキー条件による独立走査との比較も追加した。件数制約はメモリ上限・自動配分ではない。ログはtarget/combined-meta-and-wp-check.log。

WP標準のwp_is_valid_utf8／wp_scrub_utf8／wp_has_noncharactersを追加。well-formed UTF-8判定、maximal subpart単位のU+FFFD置換、全体が不正UTF-8でも有効なnoncharacterパターンを検出する処理を分けた。全2byte列、Unicode境界・noncharacter全66点・打切り／byte変更を含むコーパスを比較。mbstringなしの公式fallback経路を各バッチで検査し、実行分岐を証跡へ記録する。mbstring有効時は今回の比較に含まない。

Rust112件・Python24件、23 API・144,891ケース、フック572／dispatcher405、Clippy・HTTP・Linux向けcheck・DB証跡が成功。UTF-8最終ログはtarget/utf8-oracle-identity-check.log。raw DBやシリアライズ内容へ自動適用せず、HTML/XML検査やデータ正規化の代わりにしない。

### flat queryと有限floatのチェックポイント（2026-10-26）

wp_parse_strのflat byte queryと、wp_parse_argsの文字列分岐を追加。URL decode、空白／dotキーの変換、数字キー、重複時の位置保持を再現し、wp_parse_strの型付きfilter結果を保持する。配列argsではfilterを呼ばない。空defaultsでは任意のfilter結果を返し、非空defaultsで非配列結果をmergeする場合はUnsupportedValue。配列記法・raw NUL／decoded key内NUL・他scalar変換・参照は未対応として、部分結果を返さない。

PHPのarg_separator.input=&／max_input_vars=1000を実行時検査・証跡化した。この試作の上限は非空component 1,000件で、設定変更や警告を模倣しない。parse_str 646ケースとparse_args計1,775ケースが一致。flat query最終ログはtarget/flat-query-oracle-identity-check.log。

absintとID一覧の対応castへ、[-2^63,2^63)の有限floatを追加。0方向へ切り詰め、最小値のabsだけは従来どおり2^63のfloat。NaN・無限大・範囲外は固定PHPでcast警告を観測したためUnsupportedValueを維持し、飽和・wrapで代用しない。小数／指数／prefix-junk等の文字列は引き続き未対応。固定seedのfloat bit列とID一覧を追加し、24 API・148,608ケース、Rust118件・Python24件・Clippy・HTTP・Linux向けcheckが成功。ログはtarget/representable-float-check.log。

### 実postmeta binlog反映のチェックポイント（2026-10-26）

固定WPのpostmeta 4列を実行時のSQL定義とTableMapへ照合し、実binlogの5変更・3 COMMITからメモリ内の別行集合を構築した。主キー変更・ownerのunsigned境界・NULL・NUL・Unicode・serialized bytes、ロールバックの除外を含む。COMMITごとに独立したSQL読取りと全行・索引が一致し、旧Arc viewは不変。負のconsumerで反映拒否時のcheckpoint維持も確認した。

両DBの34項目・既存耐久ジャーナル・固定型コーパス・新メモリ反映が成功。Rust118件・Python24件、Clippy・HTTP・Linux向けcheck・依存台帳が通過。ログはtarget/postmeta-binlog-projection-check.log。耐久projection、snapshot/log anchor、安定source、一般decoder、連続CDC、公開・キャッシュ連携は残件のまま。

### 型付きprojectionの原子保存チェックポイント（2026-10-26）

投稿メタの行集合・位置・元フレームreceiptを専用InnoDB表へ一括保存するfixture候補を追加。3 COMMITを保存し、最初の保存直後に応答紛失を注入。同じpending batchを再送し、1回の一致再送として扱い二重更新しない。競合データは型付きReplayConflict。SQL接続を作り直して行・位置を一緒に読み戻し、索引を再構築した。誤epoch／fileは型付きSourceContextで拒否。

未COMMITのpairが別接続に見えず、ROLLBACKでreceiptも含め全保存値が戻ることを確認。両DBで成功し、DB各34項目・Rust118件・Python24件・Clippy・HTTP・Linux向けcheckが通過。最終ログはtarget/postmeta-rollback-and-context-check.log。製品ストレージ形式、プロセス／サーバー障害復旧、stable sourceとsnapshot/log anchor、連続CDC・公開連携の完成ではない。

### 保存済みprojectionからの購読再開と終了処理

初回COMMITの保存・同一再送を確認後、型付き行とcheckpointをSQLから再読込みし、索引とデコーダーを作り直した。購読も別接続へ切替え、保存済み位置から残り2 COMMITを取り込むことを両DBで確認。同一fixture内の接続再開であり、プロセス／サーバー再起動、安定source、連続購読・公開連携は未完成。

MySQLの間欠10054を旧SQL接続のdisconnect箇所で観測。使用完了済み接続のclosed-socket終了だけを別計測にし、接続・購読・SQL・COMMITエラーを無視しない。SQLとreceiptの再照合を維持したままMySQLを4 fixtureで反復し全成功、2回で終了resetを箇所別に記録。分類器のexperiment単体試験1件を追加し、--with-binlog-probesでこれも実行する。workspace Rust118件＋experiment 1件、Python24件、両DB各34項目、Clippy・HTTP・Linux向けcheckが成功。ログはtarget/source-resume-and-cleanup-final-check.logとtarget/mysql-source-resume-repeat-1.log～4.log。

### balanced array queryとUUIDのチェックポイント（2026-10-26）

wp_parse_str／wp_parse_argsのquery分岐を、正しく対応する括弧の配列記法へ拡大。空括弧append、負数キーからの採番、内側キーのdot保持、入れ子、scalarとarrayの上書きを再現する。400個の生成queryと括弧64段までの境界を実PHPで比較し、parse_str 1,063ケース・parse_args 2,609ケースが一致。不正な括弧・内側キーの空白・NUL key・append overflow・深さ65以上は拒否。max_input_nesting_level=64も実行時に照合・証跡化する。従来のparse_flat_queryはflat専用のまま残す。

wp_is_uuidは文字列のnull版／整数4指定を追加。小文字の形状判定、v4／variant、PCREの末尾LFの許容を保持し、全36位置のbyte変更等18,901ケースが一致。非文字列は版の診断より先にfalseとなる。UUID生成・一意性・認証や安定sourceの検証ではない。

ケース生成の定義順ミスを修正し、全fixtureが生成可能・ID一意・登録API名を網羅することを通常Python試験へ追加。workspace Rust124件・Python25件、25 API・168,769ケース、フック572／dispatcher405、Clippy・HTTP・Linux向けcheckが成功。個別ログはtarget/uuid-complete-fixture-check.log。

### anchor観測追加より前の一括検証記録（2026-10-26）

当時のソースで`python scripts/check.py --with-reference --with-binlog-probes --with-process-smoke --with-linux-check`が成功。workspace Rust124件＋独立experiment 1件、Python25件、WP25 API・168,769ケース、WP_Hook572／dispatcher405、MariaDB／MySQL各34項目、有限binlogの耐久ジャーナルとpostmeta反映・rollback・応答紛失後の再送・SQL／購読再接続、HTTP・Clippy・Linux向けcheck・依存台帳が通過した。

当時のログ：target/final-meta-recovery-and-compatibility-check.log。両DB・binlogの記録は当時のDDL／ソースと一致し、所有fixtureの一時ディレクトリは空だった。MySQLの旧SQL接続終了resetも観測として記録し、保存・購読失敗とは分けている。commit／push／配備は行っていない。

次の残件：並行更新下でのsnapshot/log anchor、stable source identity・server/process再起動、MariaDB native GTID／standalone通知、連続購読とfiltering、索引／cacheへの共有メモリ配分と自動算式、公開撤回・世代切替・cache失効、Wasm engine／ABI、認証。未決の製品方針を推定して採用しない。FTWRLの本体APIは拒否済みのまま迂回し、native Linux実行はWSL未導入のため未検証。

### 正常終了後の復元と証跡検査（2026-10-26）

producerの正常終了を待ち、実際に起動したPIDを照合してから別OSプロセスでcold restoreする。同じ所有DBは稼働を継続し、保存行・位置・receiptと索引再構築を照合する。file／position／行数違い、所有flagなし、余分な引数の5拒否は固有の理由と空stdoutまで確認し、最後に正常復元を再実行する。両DBで成功。強制終了・OS／DBクラッシュ・電源断の復帰保証ではない。[DB開発メモ](database-development.ja.md)に境界を記録した。

JSON数値はboolやfloatを整数として受け入れず、値・範囲・必須metadata・重複キーを検査する。process実行は所有するPopenのcommunicate／終了確認を経て結果を採用し、失敗・timeout時も子の停止と回収を扱う。PIDや成功フラグの自己申告だけを実行証拠にしない。

### 単一process内の共有論理予算候補（2026-10-26）

`recastpress-core::resource_budget`に、同一poolのclone間で共有する論理bytesの予約を追加した。応答cache・内部行・索引・索引構築・変更buffer・描画の6分類と総量を同時に計上する。reserve／grow失敗は既存予約を変えず、limit縮小時は既存予約を消さず超過量を残す。move-onlyの予約をshrink／Dropで返し、poison時はUnavailableとして過大予約を許さない。

これはホストから配分済みのallowanceを受ける単独候補で、既存cache／DB索引／driver／描画へ未接続。実RSS・allocatorの上限、実監視、自動容量算式、cgroup／複数processの調整を実装したものではない。

### snapshot／log位置の追加観測（2026-10-26、実測済み）

MariaDB11.8.9では同じread-only RR transactionの専用`Binlog_snapshot_file`／`Binlog_snapshot_position`を取り、最初の表SELECT前の別接続COMMIT後も旧値とanchorが不変か確認するコードを追加した。MySQL8.4.13では通常のSHOW位置が後続COMMITを含む一方、snapshot SELECTは旧値を返すraceの負の観測を追加した。両方とも専用toy表のみで、WP表や既存projectionを変更しない。MariaDBはanchor73651のままglobal73954へ進み、MySQLはbefore69375から通常SHOWのreported69735へ進む間もsnapshotは旧値・writerは新値を確認した。両readerの書込みは1792で拒否された。本番snapshot／CDC protocolの完成やMySQLの全anchor方式の不可能性を主張しない。内部同期・DDL等の条件と公式sourceは[DB開発メモ](database-development.ja.md)を参照。

### byte空白処理とHTML区切り分割（2026-10-26）

normalize_whitespace_bytesはPHP default trimのspace・TAB・LF・CR・NUL・VT（0x0b）だけを両端から除き、CR→LF、連続LFの縮約、連続space／TABのspaceへの縮約の順で処理する。FF（0x0c）やUnicode空白を端から除かず、内部NUL／VT／FFと非UTF-8を保持する。Unicode正規化や出力escaping、非byte入力のPHP coercionは実装していない。3件のRust契約試験と9,155件の実PHP比較が成功。

wp_html_split_bytesは元のregexに従うbyte分割で、captureしたHTML風区切りと、先頭・末尾・区切り間の空textを残す。厳密な`<!--`と`<![CDATA[`だけを特別扱いし、通常の`<`は引用符内でも最初の`>`で終わる。閉じない区切りは入力末尾までをcaptureする。不正なHTMLもparserで修復せず、NUL・非UTF-8を保持する。HTML parser・検証・sanitization・出力escapingではない。5件のRust契約試験と10,000件の実PHP比較が成功。全体は28 API・192,000ケース、Rust144件となった。

## postmeta byte上限の追加 checkpoint

DBの二段階先読みと論理bytes、1MiBの拒否時転送量検査を追加し、両DB各36項目へ拡張。詳細は[DB開発メモ](database-development.ja.md)。HTML内置換の対応入力10,344ケースも追加し、29 API・202,344ケースになった。全gateはworkspace152件＋独立experiment5件・Python44件を含め成功。自動容量算式・実監視・shared pool接続・本番CDCの未完了は維持する。

### URL byte整形とhex色のチェックポイント（2026-10-26）

wp_make_link_relativeは固定regexのoptional HTTP(S)と非空authority相当bytesを除去する。query／fragment／LF／NULもURL構文へ解釈せず、group2以降の未一致suffixを保持する。url_shortenはhttps://→http://→www.を全位置でcase-sensitiveに順次削除し、末尾slash／backslashを除去後、byte長で切り詰める。lengthは全signed i64へ対応し、負数・0～2・i64::MIN付近のPHP substr挙動も固定PHPで比較した。Unicode文字数でなく、UTF-8の途中で切れる場合がある。URL parser・escaping・認可ではない。

sanitize_hex_color／sanitize_hex_color_no_hash／maybe_hash_hex_colorのbyte入力を追加。3／6桁のASCII hexとcaseを保持し、固定regexの末尾1LFも受理して保持する。nullと空文字を区別し、no_hashは先頭#を全て除くが、maybe_hashは空結果やinvalidなら元入力を返す。一般CSS parser・出力escaping・安全性の保証ではない。詳細と対応外の型診断は[互換開発メモ](compatibility-development.ja.md)を参照。

この時点は34 API・241,986ケース、Rust164件のWP gate。その後sanitize_html_classを実装・接続し、下記の35 API checkpointへ進んだ。

### classname filterとLinux host RAM観測候補（2026-10-26）

sanitize_html_classを実装・台帳と実PHP比較へ接続。percent escapeを単一passで削除し、ASCII英数字・underscore・hyphenだけを残す。結果が空でもfallbackの空／`0`はPHP-falseのため再帰せず、それ以外のfallbackはterminal呼出しだけをfilterへ渡す。clean／raw classname／fallbackの3引数を保持し、fallback時はrawが元fallback・fallback引数が空となり、filter呼出しは1回。null／array等のtyped戻り値も保持する。7,255ケースが一致し、35 API・249,241ケースとなった。出力escapingやfilter後のclassname安全性は保証しない。

serverのmemory_observationはstdのみの読取り専用候補で、Linuxの/proc/meminfoからMemTotalとMemAvailableを取得する。MemFree代替なし、kB×1024をchecked計算し、受理入力64KiB（超過検出用1byteのみ追加読取）、重複／欠落／不正単位・数値／overflow／total0／available>totalを拒否する。Windowsを含む非LinuxはUnsupported。workspaceのunsafe_code=forbidを維持し、手動FFIも新しいsafe Windows wrapper依存も追加していない。

これはhost物理RAM情報で、cgroup・job object・process RSS・共有余裕・製品への配分や自動算式ではない。CLI／cacheへ接続せず、実監視は未完了。Windows上ではparser等4件とUnsupportedを検証し、Linuxはcross-checkのみでnative観測は未実行。[公式Linux仕様](https://docs.kernel.org/filesystems/proc.html)、[kB単位の公式実装](https://github.com/torvalds/linux/blob/master/fs/proc/meminfo.c)。Windowsは[公式構造体](https://learn.microsoft.com/windows/win32/api/sysinfoapi/ns-sysinfoapi-memorystatusex)と[公式API](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)を調査したが未実装。

### RSS／Atomの標準serializer候補（2026-10-26）

hostが公開用に準備したFeedDocumentからRSS2／Atom1のXMLを生成するpure APIを追加した。UTC日時、ASCII HTTP(S) URI subset、XML1.0Charとescaping、必須metadata・literal entryID重複、出力byte上限を検査する。summary_htmlのXMLescapeとHTML sanitizerは別で、公開記事の選別・private data除去・WP template／filter／query互換は未完成。既存PreparedSiteへ載せればMIMEで配信できるが、CLIのfeed endpoint自動追加や連続公開はない。[feed開発メモ](feeds-development.ja.md)

Rust serializer7件＋HTTP1件、独立XML parser4件を追加し、全181／48件へ更新。WP比較件数は35 API・249,241ケースのまま。Linux host RAM候補も引き続きWindows他Unsupportedで、cgroup／job／RSS・自動配分・CLI cache接続・native Linux観測は未完了。

### form属性のreturn-only subsetとpre-COMMIT client kill（2026-10-26）

checked／selected／disabled／wp_readonlyをdisplay=false専用のnative return APIとして追加した。Null・Bool・signed i64・Bytesを明示string cast後に厳密比較し、固定の先頭space・singlequote属性か空bytesを返す。Float／Array等は両側とも先検証で拒否する。4 API各4,313ケース、計17,252ケースが一致し、39 API・266,493ケースとなった。既定display=trueのecho・stdout ABIは未実装。標準feed serializerはWP比較数へ加算しない。

新しいDB実験はWP表を変更せず、専用clone source／projectionで未COMMITの行・位置を保存したclientだけをREADY後に強制終了する。nonce・actual PID・epoch・canonical markerを照合し、同一所有PopenのDBが稼働中であることを確認。別processが行lock取得でrollback完了を待ち、旧tuple保持から1 COMMITだけを再反映して11行と索引を比較する。最後に通常projectionのfresh restoreも再実行する。詳細は[DB開発メモ](database-development.ja.md)。COMMIT途中の曖昧な結果、server／OS／電源断、安定source identity・連続CDCは未検証のまま。

### 同一DB内の選別反映とcgroup 1 level観測（2026-10-26）

専用clone targetと除外toy表の3 COMMITから、target変更3件だけをMetaChangeへ変換し、除外変更2件を含む全raw RowsEventを元どおりstage／receiptへ保持するfixtureを追加した。除外だけのCOMMITでも新位置と全receiptを保存する。これは保持payload予算を減らさず、core／adapterの既存Advance契約も変更しない。rollback・ACK loss後の正確なreplay、除外行CRC／gap／DDLとwrong profile拒否、全TableMap schema・保存7tupleのfrom／receiptを含む照合を確認。cross_database_live_verified=falseで、同一DB限定・追加DB grantなし。[DB開発メモ](database-development.ja.md)

Linuxのcgroup_memory_observationはcaller指定pathのmemory.current／max／highだけをstdで読む1 level候補。各readは非atomicで、currentがmax／highを超える観測も拒否しない。Noneはmax tokenでありread失敗や祖先制約なしを意味しない。所属／mount／namespace／祖先探索、cgroupfsの真正性・有効余裕・自動配分は未実装。非LinuxはUnsupported、CLI／cacheへ未接続でnative Linux実読も未実行。[観測の境界](http-cache-development.ja.md)

2026-10-27追加：別module cgroup_membership_observationにself membershipとunique cgroup2 mount root `/`を照合する限定候補を追加した。leaf→可視境界を読み、外祖先はUnknown、観測はNonAtomic。境界3memory file全部NotFoundだけBoundaryUnobservedとし、一部欠落や途中の全欠落はエラー。submount／検出symlink／前後proc・namespace identity変更を拒否するが、TOCTOU・ABA・真正性・外祖先の完全性は証明しない。Windowsの新unit5件とLinux向けcompileまででnative実行なし。自動容量・CLI cache・共有poolへ未接続。[詳しい制約と公式根拠](cgroup-memory-development.ja.md)

RSS／Atomサンプルのself preferred URIはRSS／Atomの公開routeと一致するよう修正し、query & のattribute roundtripはentry alternate linkで維持した。既存HTTP1件・独立XML4件と今回の全gateが通過した。標準feedや観測候補をWP比較API数へ加算しない。

### locale／MIMEのbyte whitelistとnative filter（2026-10-27）

固定formatting.php:2468／5800に従いsanitize_locale_name／sanitize_mime_typeを追加した。localeはASCII英数字・underscore・hyphen、MIMEはASCII英数字と`-+*./`だけを残してcaseを保持する。空・wildcardだけ・不正なMIME形状も残り得る。named filterへclean／元rawの2引数を渡し、null／array等のtyped戻り値を保持し、callback失敗後のcontext復帰を検査する。byte入力だけの部分互換で、非string coercion・診断・object／参照・Wasm callback転送は未対応。OS locale・MIME／RFC構文・HTTP header設定や出力escapingの検証ではない。

各4,095ケース、計8,190ケースが固定PHPと一致し、全41 API・274,683ケースとなった。Rust契約4件追加を含むworkspace194件・Python61件、HTTP・Clippy・Linux向けcross-check、DB／binlog hash照合がtarget/locale-mime-and-filtered-postmeta-check.logで成功。過去のDB実測を今回の再起動・再実行として数えない。

### MySQL重複履歴bootstrapの独立候補（2026-10-27）

GTID ON専用のfresh MySQL fixtureで、A取得→fresh RR snapshot B→全読取り後のbinlog Cを使う非公開RAM構成候補を実測した。全frameをfile1/pos4からCRCと位置連続性で読み、先頭A所属だけを通過し、first-not-A以後はA所属も含めFULL AFTERで再生する。T1の重複、ページ間のPK移動とID再利用を経て、Cの全raw行・SQL索引を比較し、Cから通常strict before-imageでT4を1 COMMIT適用した。元WP・通常projectionは不変。132 prefix／3 bootstrap／1 strict／12行の専用second-live成功後、全gateでも成功した。

FDE descriptor長、実runtime ReplayFloor、checked GTID group長とstrictの正確な次GTID／D境界をunit6件で検証。非prefix Aは純粋試験であり、liveでGTID公開遅延や非prefix集合を作ったものではない。source identity・rotation／保持競合・任意schema・永続journal・公開・連続CDCは未完了で、公式MySQL手順や製品方式の採用とはしない。通常GTID設定は変更せず、FTWRL本体APIの拒否も維持する。[専用開発記録](mysql-overlap-bootstrap-development.ja.md)。

### WP固有email検査のbyte subset（2026-10-27）

固定formatting.php:3613のis_emailをbyte入力・既定deprecated=falseに限定して追加した。8種類の拒否contextを元順序で選び、successは元email bytesとPHP Null contextを返す。offset1以降の@存在検査とfirst@での分割を区別し、local partの末尾1LFを許すregex挙動も保持する。named filterへresult／元email／contextの3引数を渡し、typed戻り値とcallback失敗後の復帰を扱う。非byte coercion・診断・非default deprecated引数・Wasm転送は未対応で、RFC完全準拠・DNS／配送可能性・security保証・escapingではない。

8,177ケースが実PHPと一致して42 API・282,860ケースとなり、Rust契約5件を含むworkspace199件・Python68件等のemail gateが成功。今回DBは再起動せず、直前の3 fresh fixture実測とcurrent hash照合を区別して記録した。

### 明示contextのWP ID counter（2026-10-27）

固定functions.php:8156／8175のwp_unique_id／wp_unique_prefixed_idを、byte prefixを受けるUniqueIdsへ移植した。hostがrequest／execution単位のcontextを明示所有し、Default／newで開始する。global staticやCloneは持たず、全体counterとprefix別counterは独立。空・数字・case差・NUL・非UTF8 prefixを保持し、signed i64範囲の十進表現を返す。i64上限後はUnsupportedValueで状態不変。prefix連結や別context間ではIDが衝突し得るため、普遍的な一意性・security tokenの保証ではない。

比較では入力batchごとにPHP processとnative contextを同時に新規作成し、batch内で2 APIを交互に呼び出してPHP function staticと同じ寿命を保つ。unique_id_state_scopeを証跡へ記録・検査する。各2,069ケース、計4,138ケース追加で44 API・286,998ケース。Rust契約5件と可視cgroup観測5件を含むworkspace209件・Python68件がtarget/unique-id-and-visible-cgroup-check.logで成功した。cgroupはUnknown／NonAtomicの観測候補のままでnative Linux実読は未実行。非string coercion・診断、PHPのfloat counter overflow、host request lifecycle／budget統合・Wasmは未対応。DB／独立experimentは再実行していない。
