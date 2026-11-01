# WordPress互換実装の開発と証跡

> **ユーザー指示で中断・Cookie照合の本体取り込みはfocused検証まで**：`auth_cookie_verify` と既存生成側のprivate MAC共通化、strict example、別枠30関係のoracle／case／checker／comparator／任意gate flagを保存。Rust focused21、Python12、fmt／WP全target Clippyは成功し全担当process回収済み。先行target-onlyでは実WP30・native両端60・closed CLI5が成功したが、本体採用後の正式79 API／519,014・既存crypto3＋新関係・全8 fresh DB／pure／依存inventory・license／全体gateは未実行。既存証跡は変更前の履歴で、現在のsourceへの成功へ付け替えない。管理ready503・principal未接続を維持。再開手順は `target/auth-cookie-verification-pause.json`、旧formal3の比較用copyは `target/auth-cookie-verify-pre-refresh-{primitives,hooks,dispatcher}.json`。

実装済み `check.py --with-auth-cookies` は専用comparator→checkerを呼ぶが、予定出力 `compatibility/evidence/auth-cookie-relations.json` は未生成であることを確認した。targetの完成checkpointをこのpathへコピーしたり、旧３proof／crypto／DBのsource hashをrestampして代用しない。main採用後のPHP実比較・Linux cross compile・normal graph／pure・inventory／licenseも待ちで、現Rust21／Python12はその代わりにはならない。新serde dev-edgeでregistry163 tuple不変という監査も、runtime graphの新実測とは区別する。再開後は台帳実再生成をsource freeze開始前に済ませ、正式79／hooks572／dispatcher405の全入力・結果保持、新関係を含む最終12checkerまで確認する。以下の完成checkpointは各当時のsourceに対する履歴である。

[selected Wasm consumer](selected-wasm-consumer-development.ja.md)を独立packageへ採用し、採用後native７・fmt・Clippyと、別mainのcurrent Aだけを実観測した。固定checkpoint importはtest fixture用で、標準host APIやproduction SDK／engine採用ではない。registry118は既request146のtuple部分集合、追加変更０、root／既存453sourceとnormal server51／DB100構成を保持。最終11checkerは全exit0で、直前assetsのRust636／Python436／DB／HTTP結果を今回の再実行へ付け替えない。WP正式79 API／519,014への加算・再比較やauth／theme policy変更はない。

[sealed CSS／JavaScript](sealed-public-assets-development.ja.md)の3回目全体gateと最終11checkerは全exit0。Rust636（ignore1）＋独立29／Python436（429成功・skip7）、認証3種actual、全8 fresh DB mode各62、Clippy・Linux cross compile・default inventory218・HTTPが成功し全DB回収。2回のtest-only前提失敗履歴を保持し、旧V1/V2・report1七MIMEのbyte互換とV3/report2九MIMEを分ける。baseline23中13proof更新・10保持、source13不変を実assert。pure313／license file218は今回再実測せず既観測保持、WP正式API件数も追加しない。checkpoint `target/sealed-assets-checkpoint.json` は生成済み。後続の独立selected Wasm consumer採用は上段参照。

[同期one-shot／serve runtime分離](synchronous-oneshot-development.ja.md)のguard付き全体gateと最終11checkerは全exit0（`target/oneshot-runtime-full-check.log`、`target/oneshot-runtime-current-check-summary.json`、controller28341回収済み）。Rust626（ignore1）、Python431（424成功・skip7）、認証３種actual、fresh alpha２mode各62、Clippy・Linux cross compile・通常inventory218・HTTP成功。pure313／license file観測218／binlog29とWasmpub native６は今回未実行の保持値、Wasmpubは同一producer SHAで所有HTTPだけ再成功。baseline23の５file実更新／18保持・source３不変・DBdir空と実22 childを確認した（`target/oneshot-runtime-retained-identities.json`）。全８fresh DBやWP正式79 API／519,014の再比較ではなく、追加WP API・認証・theme・DB仕様変更・latency／RSS／thread数改善を主張しない。checkpoint `target/oneshot-runtime-checkpoint.json` はdocs freeze後rootが生成する。

[同一世代selected pages](selected-public-pages-development.ja.md)のguard付き全体gateはexit0／session75334回収済み（`target/selected-pages-full-check.log`）。Rust626（ignore1）＋pure313（62 artifact／features空／normal WP1）＋独立29、Python426（419成功・skip7）、認証3種actual、全8 fresh DB mode各62、Clippy・Linux cross compile・license218・HTTP成功。Wasmpub native6＋所有HTTP／capture2／比較8trueも再成功し全回収。baseline23のDB10＋認証3を実更新し、WP3＋依存7の10fileはbytes同一で保持、source2file一致とDBdir空を実assertした（`target/selected-pages-retained-identities.json`）。最終11checkerは全exit0（`target/selected-pages-current-check-summary.json`）、controller20541もexit0で回収済み。checkpoint `target/selected-pages-checkpoint.json` はdocs freeze後にrootが生成する。正式WP79／519,014・hooks572／dispatcher405は再比較しておらず、新WP subset・theme host・実HTTP batch接続を追加しない。caller明示の事前cap／fallible reserve・単一read lock・group freshness、別read A/B対照と有限lock保持契約の範囲を保つ。旧package-unpack checkpointは完成履歴として保持する。

[固定ZIP import CLI](package-unpack-development.ja.md)のguard付き全体gateはexit0（`target/package-unpack-full-check.log`）。Rust620（ignore1）＋pure313（62 artifact／features空／normal WP1）、Python426（419成功・skip7）、認証３種actual、fresh alpha２mode各62、Clippy・Linux cross compile・license218・HTTP成功。baseline23のalpha２＋認証３だけ更新し残り18file保持、source６file不変とDBdir空を実assertした。全８DB再測定とは扱わない。依存Wasmpubもnative６／所有HTTP／capture２回・８比較trueで成功し全回収（`target/package-unpack-wasm-http-observation.json`）、初回sandbox10013診断と同fixture retryを区別する。最終11checker全exit0（`target/package-unpack-current-check-summary.json`）、controller34301回収済み0。残る６DB modeは既観測保持で、証跡JSON file数とは区別する。checkpoint `target/package-unpack-checkpoint.json` はdocs freeze後にrootが生成する。これは開発用ファイル導線で、WP正式79 API／519,014への追加やWP実比較ではなく、認証・theme・DB・公開policyは変更しない。

[workbench の ZIP 書出し](prepared-post-workbench-package-development.ja.md)のguard付き全体gateはexit0（`target/workbench-package-full-check.log`）。Rust610（ignore1）＋pure313（62 artifact／features空／normal WP1）、Python419（412成功・skip7）、認証3種10／113（actual88＋refusal25）／33（actual25＋refusal8）、Clippy・server/WP Linux cross compile・license218・process smokeが成功。Node29、native retry４件・12種dummyの２file bytes一致、所有sealed HTTP、実ブラウザーのWebCrypto準備と狭幅表示を確認した。OS ZIP保存／downloadは未実行。今回fresh DB起動やWP正式79／519,014・hooks572／dispatcher405の再比較は行わず、UIをAPI数へ加算しない。最終11checkerは全exit0（`target/workbench-package-current-check-summary.json`）、controller49832も回収済み0。20／23fileをbaseline同一で保持し（除外は実再観測した認証3証跡）、WP3proof／全DB証跡のbytes同一と選択source10file不変を実assertした（`target/workbench-package-retained-identities.json`、`target/workbench-package-source-freeze-identities.json`）。checkpointはdocs freeze後にrootが `target/workbench-package-checkpoint.json` へ生成する。theme／認証／DB／自動公開は追加していない。

[開発gate source freeze guard](development-source-freeze.ja.md)の統合gateと最終11checkerは全exit0。Rust610（ignore1）＋pure313＋binlog29／Python415（408成功・skip7）、認証3種actual、影響するfresh DB4 mode各62・Clippy・Linux cross compile・license218・HTTPが成功し全回収。WP正式79／519,014・hooks572／dispatcher405は再比較せず3proof byte同一でcurrent保持し、今回全8DB再実測へ加算しない。fresh4 modeは更新JSON4個を意味しない。旧sanitize79 checkpointは履歴保持し、新APIや認証policyを追加しない。checkpoint生成はdocs freeze後の別操作。

[sanitize_text field／textarea](sanitize-text-development.ja.md)の全体gateはexit0、正式79 API／519,014件・旧77保持を実確認した。Rust610（ignore1）＋pure313＋独立29／Python402（396成功・symlink6 skip）／全8 DB各62・認証3種・Clippy・Linux cross compile・license218・HTTPが成功し全fixture回収。最終11checkerは全exit0（`target/sanitize-text-current-check-summary.json`）、依存6fileの現SHA＝開始snapshot＝parser77 checkpoint SHAも実assert済み。sanitize checkpoint生成はdocs freeze後の別操作。raw NoLiteralLt／UTF8 site／空filterの限定profileで、自動DB／UI適用、HTML安全性、URL decode、認証policyを追加しない。parser77は完成履歴として保持する。

[明示cookie parser](auth-cookie-parse-development.ja.md)の全体gateはexit0。正式77 API／513,292件・旧76入力順／件数／両結果SHA保持、Rust602（ignore1）＋pure305＋独立29／Python393（387成功・symlink6 skip）／全8 DB各62・認証3種・Clippy・Linux cross compile・license218・HTTPが成功し全DB回収。最終11checkerは全exit0（`target/auth-cookie-parse-current-check-summary.json`）。manifest／lock／inventory／license6fileの現bytes＝開始snapshot＝auth76 checkpoint SHAも実assert済み。parser checkpoint生成はdocs freeze後の別操作。auth76完成履歴は保持し、field分解をHMAC検証・session／principal・管理認証へ昇格しない。sanitize_textは次工程draftでmain未採用。

最新static MIME gate（2026-10-30）：[固定MIME参照](static-public-mime-development.ja.md)に対し、対象core21／Rust589（ignore1）＋独立29／Python374（368成功・symlink6 skip）／今回認証3種／全8 DB62・HTTP・Clippy・core/server Linux cross-check・license file観測215が成功。依存Wasmpub native6＋実HTTPも再成功し、最終11checker全0・全fixture回収。公開signature・値・拒否境界・error優先を維持する内部差分であり、WP75 API508,297／pure300はsource不変の前回観測をcurrent checkerで保持し今回再測定へ加算しない。

最新page snapshot共有gate（2026-10-30）：[内部Arc共有](shared-public-page-development.ja.md)に対し、対象core16／Rust586（ignore1）＋独立29／Python374（368成功・symlink6 skip）／今回認証3種／全8 DB62・HTTP・Clippy・core/server Linux cross-check・license file観測215が成功。独立Wasmpub native6＋実HTTPも再成功し、最終11checker全0・全fixture回収。公開API・Revision／deliveryのfreshness判定を維持する内部差分であり、WP75 API508,297とpure300はsource不変の既観測をcurrent checkerで保持し今回再測定へ加算しない。

最新本文共有gate（2026-10-30）：[sealed public body共有](shared-public-body-development.ja.md)のcore/server差分を確認。Rust581（ignore1）＋独立29／Python374（368成功・symlink6 skip）／全8 DB62・HTTP・Clippy・core/server Linux cross-check・license file観測215が成功。独立Wasmpub native6＋実HTTP／capture比較も再成功。WP入力・実装・probe sourceは変更せず、正式75 API508,297は実再比較せず既証跡のrequired-source／binaryを確認した。認証3種はworkspace後のbinary差を受け現binaryで実再観測し、最終11checker全0（`target/shared-body-current-check-summary.json`）、全fixture回収。pure300は直前観測を保持し今回の件数へ加算しない。

最新標準タイトルgate（2026-10-30）：[準備済みraw投稿と標準4filter](title-development.ja.md)を追加し、正式75 API508,297件（新12,961）・hooks572／dispatcher405が成功。`target/title-default-full-check.log`はdefault Rust573（ignore1）＋pure native300＋独立29／Python368（362成功・symlink6 skip）／全8 DB62・credential3種・Clippy・default Linux cross-check・license file観測215・HTTPもexit0。旧74の全件数と入力・両結果prefixSHAを実照合、最終11checker全0・全fixture回収。別escaping→固定package／sealed HTTP関係も実成功したが、ID・status・passwordの確認をDB選択・認証・公開資格の証明へ昇格させず、既存literal入力へ自動接続しない。

最新文字整形gate（2026-10-30）：[wptexturize限定profile](texturize-development.ja.md)を追加し、正式74 API495,336件（新9,183）・hooks572／dispatcher405が成功。`target/texturize-full-check.log`はdefault Rust566（ignore1）＋pure native293＋独立29／Python353（347成功・symlink6 skip）／全8 DB62・credential3種・Clippy・default Linux cross-check・license215・HTTPもexit0。旧73の全件数と入力・両結果prefixSHAを実照合、最終11checker全0・全fixture回収。English固定・whole UTF-8・literal `<` なし・空shortcode/filterのprofileと3論理byte上限だけを対象とし、既存literalタイトルrendererへ自動接続しない。

最新truncate gate（2026-10-29）：正式72 API・482,951比較（新_truncate_post_slug16,064）、hooks572／dispatcher405が成功。`target/truncate-slug-and-post-name-pipeline-full-check.log`はRust536＋独立29・Python254（1skip）・全8 DB61・Clippy・Linux向けcheck・license215観測・実HTTPもexit0。旧71 API466,887の件数とcanonical input/result SHAを実prefixで保持。最終11checker current（`target/truncate-slug-current-check-summary.json`）、全fixture回収済。raw任意Bytes枝とchanged decoded UTF8枝の条件付きprofileで、結果をURL安全性／一意slugへ昇格させない。

最新outer-title gate（2026-10-29）：正式71 API・466,887比較（新sanitize_title5,365）、hooks572／dispatcher405が成功。`target/sanitize-title-and-post-name-retry-full-check.log`はRust529＋独立29・Python248（1skip）・全8 DB60・Clippy・Linux向けcheck・license215観測・実HTTPもexit0。旧70 API461,522の件数とcanonical input/result SHAを実prefix再計算で保持した。最終11checker全current、`target/sanitize-title-current-check-summary.json`、全fixture回収済。outerは明示locale／NoIntl／NoMB／標準callbackのみの返値profileであり、任意hooks・安全なURL・一意slug・認証・公開・native Linux実行の完成ではない。

最新URI/title gate（2026-10-29）：正式70 API・461,522比較（新URI5,920／title2,538）、hooks572／dispatcher405が成功。`target/uri-title-and-credential-pipeline-full-check.log`はRust520＋独立29・Python241（1skip）・全8 DB57・Clippy・Linux向けcheck・license215観測・実HTTPもexit0。旧68 API453,064のcase countとcanonical input/result SHAを実prefix再計算で保持した。最終11checkerは全current、`target/uri-title-current-check-summary.json`。全fixture回収済。direct titleはaccent除去を呼ばず、URI200-byte上限が後cleanupより先に働く限定profileであり、URL割当・unique slug・新規投稿・native Linux実行の成功ではない。

停止checkpoint（2026-10-27、日本時間）：target/empty-filter-escaping-and-memory-diagnostic-workspace-check.logで実PHP63 API／436,656比較、hooks572／dispatcher405、workspace419 Rust（通常ignore1）・Python155、Clippy・Linux cross-check・license208・実HTTP・DB current hashが成功した。新empty-filter esc_html／esc_attr各1,938件は返値だけの明示UTF8 profileで、任意callbacks／native filter bookkeepingをclaimしない。actualPHP hookcounterとcase数を厳密照合し、5globalsとINIを復元する。独立レビューも通過。全native current hashを別途照合し、DB live再実行無しで直前29件・6 fixture各39項目を維持。ユーザーの停止指示で新規作業を止め、全体α目安25％はこのAPI件数から算出したものではない。[escape](escaping-empty-filters-development.ja.md)・[memory CLI](memory-diagnostic-development.ja.md)。

最新specialchars／bundle検証gate（2026-10-27、日本時間）：target/specialchars-and-bundle-check-workspace-check.logで固定PHP61 API／432,780比較、hooks572／dispatcher405、workspace411 Rust・Python149、Clippy・Linux cross-check・license208・実HTTP・DB current hashが成功。`_wp_specialchars`のUTF8限定kernel23,256件は6styles×両bool各1,938、strict profile／全12cellを検査する。既存HTML6,527／XML7,544を維持。一般flag/coercion、esc_html／esc_attr filtersやXML安全性はclaimしない。bundle検証CLIは別のadmission確認でWP比較数へ加算しない。全native current hashも通過、DB live再実行無しで独立29件・6 fixture各39項目を維持する。[特殊文字](specialchars-development.ja.md)・[bundle検証](bundle-check-development.ja.md)。

最新XML／コメントfeed gate（2026-10-27、日本時間）：target/xml-utf8-and-comment-feeds-workspace-check.logで実PHP60 API・409,524ケース／hooks572／dispatcher405、workspace402 Rust・Python141、Clippy・Linux cross-check・license208・実HTTP・DB current hashが成功。既存normalizerのHTML6,527件を保持し、明示XML／UTF8 profile7,544件を加えて同API14,071件、API数60のまま。mappingとruntime profileを厳密照合し、charset／2globalsを復元する。独立レビューも通過。コメントserializerと固定sampleはWP全template互換として数えず、公開準備済み出力だけの別契約とする。全native current hashを別途照合し、DB live再実行無しで独立29件・6 fixture各39項目を維持する。[XML](xml-entity-normalization-development.ja.md)・[コメントfeed](comment-feeds-development.ja.md)。

最新process RSS観測gate（2026-10-27、日本時間）：serverだけの追加後、target/process-rss-observation-workspace-check.logでworkspace393 Rust・Python133、Clippy・Linux cross-check・license208・実HTTP・DB current hashを通過。WP／全nativeも別途current hashを照合し、PHP／DB live再実行無しで60 API／401,980ケース・hooks572／dispatcher405、独立29件・6 fixture各39項目を維持する。RSS観測4契約をWP比較数へ加算せず、native Linux実読・自動容量／監視の完成とは扱わない。[process RSS](process-memory-development.ja.md)。

最新outer protocol／MySQL server再起動全gate（2026-10-27、日本時間）：target/protocol-outer-reference-workspace-check.logで固定PHP60 API・401,980ケース（wp_kses_bad_protocol7,768追加）／hooks572／dispatcher405が成功。全文NoNull1回、top限定の厳密http(s)短絡、default count=1の最大6反復と6回目のstrict byte安定判定を実装した。target/protocol-outer-and-mysql-server-restart-full-check.logでworkspace Rust389件（通常ignore1）＋独立29件・Python133件、全6 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPが通過し全fixture回収。target/protocol-outer-server-restart-final-wp-provenance.logで最終照合。専用server再起動は旧journalの保持／候補rollback検査だけで、binlog再生や製品CDCではない。fixture helperの終了失敗・期限付きIOも補修した。途中gateが変更後の旧DB hashを拒否した記録を保持し、全6 fixtureで実測更新した。[outer helper](bad-protocol-development.ja.md)・[server再起動](mysql-server-restart-development.ja.md)。

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

最新JSON profile gate（2026-10-27、日本時間）：target/json-utf8-fallback-profile-check.logが成功。workspace Rust256件（通常ignore1）・Python88件、固定PHP比較45 API・310,363ケース（JSON23,365＝従来17,073＋明示UTF8/no-mbstring6,292）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check・license208観測とDB current hash検査を通過した。別途binlog／MySQL overlapのcurrent hash検査も成功。今回はDB live／独立experimentを再実行せず、前回の独立19件・3 fixture各39項目・全回収を維持する。新wp_json_encode_utf8_no_mbstringだけが固定profileのwhole-graph補修とキー衝突を扱い、既存2 APIのinvalid UTF8拒否は変えない。元graph全域の未対応値／64段を先に検査し、補修cloneのメモリは出力cap／共有pool対象外。Floatは反例と設定依存を記録して未対応を維持。[JSON開発記録](json-encoding-development.ja.md)。

再開後の最新検証（2026-10-27、日本時間）：target/shared-buffer-and-pretty-json-full-check.logが成功。workspace Rust251件（通常ignore1）＋独立experiment19件・Python86件、固定PHP比較45 API・304,071ケース（wp_json_encode 17,073件／9flags全512組合せ）／hooks572／dispatcher405、HTTP・Clippy・Linux向けcross-check・dependency license208 package観測・DB／binlog／overlap current provenance検査を通過した。通常MariaDB／MySQL・専用GTID MySQLの各39項目、shared poolのrows／index／retained raw buffer接続を有限fixtureで確認し、全3所有fixtureは停止・回収済み。default JSON wrapperを保持する。native Linux、製品CDC、全CSR／R61予算・RSS／自動容量算式の完成を示さない。API母集合は未列挙で、対応率を示さない。

前回default JSON導入時の記録：

記録更新：2026-10-27（日本時間）。基準はWordPress 7.1.3。45 API・292,301ケース、フック572／dispatcher405がtarget/json-encoding-and-license-check.logで実PHP比較成功。workspace Rust224件（通常ignore1）・Python77件、HTTP・Clippy・Linux向けcross-check、208依存パッケージのlicense観測、DB／通常binlog／MySQL overlapのcurrent hash検査も成功。今回は実DB／独立experimentを再実行せず、直前limited-index-native-check.logの独立experiment19件、通常MariaDB／MySQLと専用GTID MySQL各36項目の実測を保持する。標準feed・観測候補・DB fixtureはWP API数へ加算しない。API母集合は未列挙で、対応率は出していない。旧件数・ログは履歴。

## 固定ソースの再現

```powershell
python scripts/wordpress_reference.py fetch
python scripts/wordpress_reference.py verify
```

公式tar.gzの公開SHA-1 `491a78c7676b15dffcc69130c6054b8f49228a91`に一致する配布物だけを扱う。取得後のSHA-256は`d2a09acb6a15e3b9c471d72557753c266d6f41a79ed19bfe04cbfe49e283b2a5`。3,782ファイルの一覧・サイズ・ハッシュと版番号を記録した。SHA-1の一致と署名検証は同じものではなく、このツールは署名検証を実装していない。

取得物は`.reference/wordpress-7.1.3/`へ保存し、Gitへ含めない。展開時はリンク・特殊ファイル・パス逸脱・Windows予約名・大文字小文字の衝突・サイズ超過を拒否する。途中失敗した作業フォルダーは調査用に残り、既存の完成済み参照物を上書きしない。

## 実装と対応台帳

`recastpress-wp`は、PHPバイト列、64bit整数／文字列キー、挿入順を持つ配列と、初期の互換プリミティブを実装する。参照元は`compatibility/registry.json`に、WordPressシンボル、元ファイル・行・ハッシュ、Rustシンボル、契約テスト、未対応入力、比較証跡を記録する。

認証 provider の既定 `auth` feature と、package 単独の no-default-features native 確認は [WP build profile](wp-build-profiles-development.ja.md)を参照。正式比較は既定 profile で維持し、pure profile の依存分離を Wasm／認可の完成へ読み替えない。

[raw認証cookie生成](auth-cookie-development.ja.md)を正式76 API／510,813件へ追加し、全体gateと旧75の件数・入力・両結果prefixSHA保持を確認した（`target/auth-cookie-full-check.log`、`target/auth-cookie-old75-preservation-check.log`）。最終11checkerはすべてexit0（`target/auth-cookie-current-check-summary.json`、controller58647も出力回収後exit0）。caller準備済みuser／resolved salt／truthy tokenからの返却bytesだけで、session・cookie検証・principal・HTTP設定には未接続。次のexplicit parserはdraftのままで正式数へ含めない。

| WP API | Rust側の現状 | 未完了の範囲 |
| --- | --- | --- |
| `wp_kses_bad_protocol_once` | Bytes/C/allByte protocols/defaultcount1、semicolon補修→firstsplit→raw guard→PHP6trim→calledprefixだけonce2→feed、empty0保持、top入力1MiB／最終cap分離 | count override／非Byte診断順／defaultpolicy・calledprefix decoderdomain外、URL認可／safety・RSS／shared pool。修復／再帰Vecはfinalcap外 |
| `wp_kses_bad_protocol_once2` | C-byte scheme＋全値BytesのPhpArray、decode→white除去→NoNullRemove→ASCII lower、allowed側はlower比較のみ。match時colon、最終cap | 非Byte短絡診断順・coercion・非C・decoder0..255／input1MiB外、URL文法／認可／safety・RSS／shared pool。中間Vecはfinalcap外 |
| `wp_kses_stripslashes` | Bytesのbackslash＋doublequoteだけを単回置換。n本のslash＋quoteはn-1本、他escape・末尾slash・raw bytes保持、完全出力cap | scalar／array coercion・診断・PCRE資源失敗、再帰処理・一般HTML sanitizer／escaping・RSS／shared pool |
| `wp_kses_no_null` | Bytes・明示Remove／Keep、指定control除去後のslash＋ASCII0 run処理、TAB／LF／CR／DEL／highbyte保持、完全出力cap | 他option coercion・nonbyte診断・PCRE資源失敗、再帰置換・一般HTML sanitizer／escaping・RSS／shared pool |
| `valid_unicode` | signed i64の3範囲とHT／LF／CR例外。補助平面末尾の非文字も元どおり受理 | 非整数cast・診断、UTF-8解析・一律非文字排除・HTML安全性判定 |
| `wp_kses_decode_entities` | Bytes・両passの参照値0..255、十進→hex各1pass、raw1byte・生成hex検証、入力1MiB／最終出力cap | 範囲外chr診断・modulo／Float・scalar変換・PCRE失敗・HTML sanitizer／escaping・RSS／shared pool。中間Vecは最終cap前に確保 |
| `wp_kses_normalize_entities` | 固定253名称・HTML文脈のbyte API、numeric callback順・非再帰amp処理・完全出力cap | XML／動的allowlist・scalar変換・PCRE資源失敗・HTML sanitizer／escaping・RSS／shared pool。[対応範囲](entity-normalization-development.ja.md) |
| `wp_json_encode` | flags=0 wrapperとselected9 flags（PRETTY_PRINTを含むmask2527）、明示UTF8/no-mbstring APIだけwhole-graph fallback。Null／Bool／i64／bytes／ordered array、host配列深度64・完全出力cap | 既存2 wrapperのinvalid UTF8、他charset／mbstring profile、Float・object／resource／参照・未選択flags・Wasm・RSS／shared budget。補修cloneはcap前。HTML／script向け安全性保証なし |
| `wp_unique_id`／`wp_unique_prefixed_id` | byte prefix、明示UniqueIds context内の独立した全体／prefix別i64 counter | 非string coercion・診断、float overflow、request lifecycle／budget統合・Wasm。衝突可能でsecurity保証なし |
| `wp_validate_boolean` | null・boolean・整数・NaN以外のfloat・バイト列・配列 | オブジェクト／資源／参照、NaN変換の警告 |
| `wp_is_numeric_array` | 整数キーを持つ配列の判定。キーの穴・負数・空配列を許可 | 32bit PHP、完全なPHP値／参照モデル |
| `sanitize_key` | バイト列入力と明示的フィルター呼出しのkernel | 他のscalarの文字列化、WPフック全体の再現 |
| `sanitize_html_class` | byte classname／fallback、percent削除・ASCII whitelist、terminal filterの3引数とtyped戻り値 | 非byte coercion・診断、Wasm callback、出力escaping・filter後の安全性 |
| `is_email` | byte入力・既定deprecated=false、8拒否contextとsuccessNull、3引数filter／typed戻り値 | 非byte coercion・診断・非default deprecated・Wasm、RFC全準拠／DNS／配送／security保証 |
| `sanitize_locale_name`／`sanitize_mime_type` | byte whitelistとcase保持、clean／raw2引数のnamed filter、typed戻り値 | 非string coercion・診断・object／参照・Wasm、OS locale／MIME／headerの検証 |
| `checked`／`selected`／`disabled`／`wp_readonly` | display=falseのreturn API。Null／Bool／i64／Bytesの明示string cast後に厳密比較 | Float／Array等のcast・診断、既定display=trueのecho／stdout ABI |
| `absint` | 整数・十進整数文字列・範囲内の有限float・null／boolean／配列。PHP_INT_MINはfloat | NaN／無限大／範囲外float、小数／指数／prefix-junk／overflow文字列、object／参照 |
| `wp_parse_id_list` | string／array／root bool・整数のID一覧、対応cast、元キーと重複後の穴を保持 | root Nullの診断・root float、absintの未対応変換、一般的なarray_unique |
| `wp_parse_list` | バイト列をASCII空白／カンマで分割。root bool／整数を文字列化。配列はscalarだけ元キーで保持 | root Nullの診断、root float、locale依存、参照／object |
| `wp_array_slice_assoc` | 整数／バイト列キーの指定順に抽出。重複は最初の位置、nullは省略 | 他のキー型の変換、参照／object |
| `wp_parse_args` | 配列／対応query入力と配列defaults。文字列だけparse_str filterを通す | 不正なquery表現、非配列結果のmerge、objectプロパティ、参照 |
| `wp_parse_str` | flatとbalanced配列query、キー変換・append・URL decode・型付きfilter | 不正括弧・内側空白・NUL key・設定変更・診断・参照 |
| `wp_is_uuid` | null版／整数4、小文字の形状と末尾LF、非文字列の短絡 | 他の版型変換・診断、生成・一意性・認証 |
| `wp_check_jsonp_callback` | C localeのbyte判定。英数字・underscore・dotを許可し、非文字列はfalse | C以外のlocale、object／参照、JS構文・出力escaping・endpoint認可 |
| `wp_list_pluck` | 配列要素からのfield抽出、元キー維持、指定indexでの上書きと自動追加 | object／参照、field欠落の警告、floatキー変換、append上限エラー |
| `trailingslashit`／`untrailingslashit` | 末尾のslashとbackslashだけを調整するbyte入力 | 非string入力の変換 |
| `wp_basename` | byte-string pathとsuffix。encoded名でsuffixを照合 | 非string変換・診断、ファイル解決／認可 |
| `wp_normalize_path` | 非streamのbyte入力、Windows drive、UNCとLF境界 | stream wrapper登録と静的cache、非string入力 |
| `maybe_unserialize` | 正規形のnull／boolean／整数／byte列／配列。外側trim・1段だけ復元 | 非正規形・不正値・float・object／参照・PHPの警告／失敗false、復元64段超 |
| `maybe_serialize` | 対応配列の型・byte長・キー順、既存serialized文字列の二重化 | 配列内のfloat・object／参照、深さ64超、unserialize |
| `is_serialized`／`is_serialized_string` | WPの浅い形状判定、strict／looseとPHP trim | デシリアライズ、object／参照、strict引数の型変換 |
| `wp_is_valid_utf8`／`wp_scrub_utf8`／`wp_has_noncharacters` | byte入力のUTF-8判定・maximal subpart置換・独立したnoncharacter検出 | scalar変換・診断、mbstring有効経路の実比較 |
| `zeroise` | i64／Bytes、width0..4096の左0pad、符号の前、byte長・非切詰め、完全出力cap | 負width／Float／Bool／Null／Array等のcast・診断、PHP／製品容量上限の代用 |
| `wp_convert_hr_to_bytes` | 非負ASCIIdecimal＋単一KMG大小・leading0、有効256桁、default1024units、i64MAX飽和 | 長大Inf化／負／符号／空白／小数／指数／任意contains、float返却・製品iniや資源配分 |
| `wp_is_json_request`／`wp_is_xml_request` | 明示Accept／Content-TypeのOption bytesから独立bool hint。JSONはC media判定、XMLはAccept substring／Content-Type exact | 他SERVER context・非string診断・JSONの非C／PCRE failure、q値交渉・本文解析・HTTP認可 |
| `wp_is_json_media_type` | C localeの全byte検索、source固有range／左右境界／jsonとoptional oembed。stateless O(n) | 非C／nonstring／memoization／PCRE資源失敗、MIME構文解析・HTTP認可・JSON内容判定 |
| `is_utf8_charset` | 明示byte slugのUTF-8／UTF8だけをASCII大小文字無視で判定 | Null／省略時get_option、非string、製品charset選定 |
| `wp_check_invalid_utf8` | 明示UTF8/no-mbstring profileのbyte入力とbool strip。valid保持、invalidは空またはU+FFFD補修 | 他charset／mbstring、scalar変換・非bool strip、実行中site設定変更 |
| `wp_checkdate` | 整数componentのGregorian判定とfilter引渡し | numeric string／float変換、日時の解析 |
| `wp_slash`／`wp_unslash` | scalarと配列のバイト列変換。キーは維持 | object／resource／参照、深さ64超はホスト制限 |
| `normalize_whitespace` | byte入力、PHP trimの6byte、CR→LFとLF／space・TAB runの縮約 | 非byte入力のcoercion、Unicode正規化、出力escaping |
| `wp_html_split` | 元regexのbyte分割、delimiter captureと空text、comment／CDATA・不正区切り・引用符の元挙動 | PHP coercion・PCRE資源上限エラー、HTML parsing／検証／escaping |
| `wp_make_link_relative` | byte URL風入力の固定regex置換、optional HTTP(S)、LF／NUL・未一致suffix保持 | PHP coercion・診断、URL parsing／認可／escaping |
| `url_shorten` | byte入力、固定文字列の順次削除、全signed i64長でsubstr相当とhellip付加 | 非整数length・非byte入力のcoercion、Unicode文字数・URL parsing／escaping |
| `sanitize_hex_color`／`sanitize_hex_color_no_hash`／`maybe_hash_hex_color` | byte入力、3／6桁ASCII hex、末尾1LF保持、先頭#処理とnull／空の区別 | 非byte型変換・診断、一般CSS parsing／出力escaping |
| `wp_replace_in_html_tags` | byte haystackとbyte key／value map、空map・single空key、delimiter内だけ最長一致・非再帰置換 | 整数／正規化数値key・非byte値・multiple空key、PHP coercionと遅延診断、出力escaping |

NaNのboolean変換はPHP 8.5.12で警告を出すため、診断を実装するまで`UnsupportedValue`を返す。警告を黙って無視して互換済みにしない。

```powershell
python scripts/compatibility_registry.py
```

この確認は、固定参照元・宣言位置・Rustシンボル・テスト名と、比較証跡のソースハッシュが現在のコードに一致することを確認する。コードや比較入力生成器・台帳・Cargo.lockが変われば証跡を古いものとして拒否する。テスト名の存在確認はテスト実行の代わりではない。

## 実行比較

Windowsでは、公式配布物のSHA-256を固定した開発専用PHP CLIを取得できる。

```powershell
python scripts/setup_php.py
python scripts/compare_primitives.py
```

PHP 8.5.12 NTS x64、公式配布物SHA-256 `2ab242af38d76bdf417cde5ec048b1bcd76a5a40f39a3d15d5f046f38571c477`。システムPATH・php.ini・常駐サービスを変更しない。製品のPHP対応版を確定するものではない。他の環境では`--php /path/to/php`で別の開発用CLIを渡せる。比較は64bit整数環境のみ。

比較器はRustの開発用probeをビルドし、同じ型付き入力をRustとPHPへ渡す。PHP側は固定ソースから対象関数をトークン抽出し、実際のWP `plugin.php`とフッククラスを読んで実行する。DB・WP全体のbootstrap・テーマ・プラグインは起動しない。公開HTTPプロセスからこのCLIや比較器へ到達する経路はない。

参照helperはPHP process内のfile文字列keyごとに抽出済みsnippet stringsだけをcacheし、token配列を保持しない。初回のTOKEN_PARSEとindex後、要求されたcodeだけを毎回evalする。first-match／strict name／nested／interpolation、byref／comment未対応、missingと再load時のredeclaration失敗は従来どおり。検証済みsourceがprocess中不変であることが前提で、変更検知や一般PHP parserではない。現在の4file・48snippetは変更前loaderとbytes／長さ／SHAが全件一致し、synthetic6試験も成功した。有限warm Windows PHP観測ではtoken化48→4、code strings626,996bytes、peak14MiB／既定上限128MiBを維持してbootstrap median約0.474→0.053秒。fullgate速度の保証やAPI比較件数への加算ではない。[loaderの契約・観測とgate履歴](reference-loader-development.ja.md)を参照する。

2026-10-27のPRETTY_PRINT追加後の比較で304,071ケースが一致。空値、0とfalse系の文字列、全256種の単一byte、固定seedのバイト列、キーの穴・負数・正規化・境界値、フィルターへ元バイト列を渡すケースを含む。リスト処理では全byteの区切り、負数・穴・重複・数値文字列キー、null・false・入れ子、NaN／無限大を含むscalar配列も比較。normalize_whitespaceの9,155ケース、wp_html_splitの10,000ケースに続き、wp_replace_in_html_tagsの10,344ケースを追加した。floatはビット列（NaNは分類）で比較し、配列の順序も照合する。NaNのboolean変換は未対応として比較対象外。入力・出力・実装・ハーネス・PHP実行ファイルのハッシュを`compatibility/evidence/primitives.json`へ保存する。

これは45 APIの対応入力に限る比較証跡。任意入力、全コアAPI、テーマの組合せ、PHPプラグイン、DB・管理UI・配備先の互換性を示すものではない。

スラッシュ変換は全byte・バックスラッシュに続く全byte・NUL・末尾バックスラッシュ・入れ子配列を比較。HTML／JSON／URL／SQL向けの安全化APIではない。参照したPHP仕様は[addslashes](https://www.php.net/manual/en/function.addslashes.php)と[stripslashes](https://www.php.net/manual/en/function.stripslashes.php)。配列深さ64を超える入力のRecursionLimitはホストの制限で、PHPとの同一性を主張しない。

## リクエスト内のフック処理

`recastpress-wp::hooks::Hook`は`WP_Hook`の一部をRustで実装する。優先度と同順位の登録順、同じIDの再登録、削除・全削除、実行中の優先度変更、引数数の切詰め（負数を含む）、入れ子のフィルター／アクションを扱う。呼出し先はモジュールIDとコールバックIDで識別する開発用のnative関数で、Wasmの境界は未実装。

```powershell
python scripts/compare_hooks.py
```

固定`WP_Hook`へ同じシナリオを渡し、順序・引数・優先度・深さ・戻り値・最終登録状態を比較する。手書き32シナリオと固定seedで組み合わせた250シナリオ、計282シナリオに型付き値290シナリオを加え、572シナリオが一致。証跡は`compatibility/evidence/hooks.json`。

初回比較で見つけて修正した点：現在の優先度バケットはPHPのforeachと同じスナップショットで回す一方、未来の優先度は変更を追う。全削除で反復が終了した場合、その場の再登録で同じ実行を再開しない。入れ子の`current_priority()`は固定ソースの外側反復ポインターの動作を維持する。

名前付きdispatcherは次節の別コンポーネントとして実装。PHP参照・object同一性とWasm輸送は未実装。再帰制限64はホスト試作の内部制限であり製品の合格数値ではない。コールバック失敗時の後始末はRustホストの動作として試験し、PHPの例外との同一性は主張しない。

## 名前付きフックのdispatcher

`dispatcher::Dispatcher`に、名前付きフィルター／アクション、`all`、呼出し回数、実行中スタックを実装。実行中のフックがグローバル登録から外され、同名で再登録された場合、古いオブジェクトの反復を保持したまま新しいオブジェクトを使う。不要になったオブジェクトは実行終了後に回収する。

```powershell
python scripts/compare_dispatcher.py
```

WordPressの実際の`plugin.php`関数との405シナリオ（従来213＋型付き192）で、引数・順序・戻り値・現在フック・実行回数・最終登録状態・診断が一致。うち22シナリオで計80件の警告も一致した。固定WPが`all`の反復中に優先度変更した際の`Undefined array key`警告を、Rust側は型付きの`HookDiagnostic`として記録する。根拠とハッシュは`compatibility/evidence/dispatcher.json`。

nativeの文字列値を中心にした対応範囲の試験。参照配列の入口、PHP4形式のobject引数、任意のPHP object／資源、Wasmホスト呼出しは未実装。PHPの例外・プロセス失敗・実行回数の整数上限での同一性は主張しない。これらのdispatcherはHTTP経路へ接続していない。

## 宣言とフック呼出しの候補索引

```powershell
python scripts/inventory_wordpress.py
```

固定参照の全PHP 1,512ファイルを`token_get_all`で解析した。13,014件の宣言候補（関数4,461、メソッド7,714、class 769、interface 63、trait 7）と、5,059件のフック関連呼出し候補を記録。フックの第1引数はリテラルソース4,621、動的式438。PHPファイルをinclude／evalせず、コメントや文字列内の偽の宣言を拾わないことを試験した。

詳細索引はGit対象外の`.reference/wordpress-api-candidates.json`、件数・ハッシュ・制限は`compatibility/inventory-summary.json`。字句上の名前空間、所有クラス、メソッドのpublic／protected／private、static、内部用／非推奨doc注記を分類した。匿名クラスの所有名はソース位置に基づく仮IDで、PHPの実行時クラス名ではない。入れ子関数はメソッドと区別し、use functionのimportを宣言へ数えない。未修飾フック呼出しの名前解決、import・継承・trait別名の解決、実行時の利用可能性、テーマからの到達性、匿名宣言・定数・大域状態・依存展開は未完了。この件数を公開APIの母集合や対応率の分母には使わない。

`python scripts/inventory_wordpress.py --check`で集計と索引の作成元・ツールの一致を確認する。`check.py --with-reference`にも組み込んだ。集計の分類は[PHP名前空間](https://www.php.net/manual/en/language.namespaces.definition.php)・[ユーザー定義関数](https://www.php.net/manual/en/functions.user-defined.php)の字句上の区別に基づき、公開SDKへの採用判定とは区別する。

## 一覧フィールド抽出

wp_list_pluckの配列入力を追加。index_keyなし、0、文字列0、空文字列では元の一覧キーを保持する。index_keyが有効なら、各行の値を整数／バイト列／booleanのキーとして使い、重複は最初の位置で上書き。indexが欠落またはnullならPHP 8.3以降の整数追加規則で末尾へ追加する。最初が負のキーの場合も0へ勝手に丸めない。非配列のモデル化済み入力にはWPと同じく空配列を返す。

固定のWP_List_Utilとwp_list_pluck実関数へ491ケースを追加し、合計4,768ケースが一致した。依存クラスのソースハッシュも台帳・比較証跡へ含める。field欠落の警告、非配列の行に対するdoing_it_wrong診断、float index変換、appendの整数上限エラー、object／参照は未実装でUnsupportedValueを返す。これらを正常結果へ置換して比較対象へ入れていない。

## パスと日付の追加範囲

パス処理は全byteと、Windowsのdrive・UNC共有名・slash／backslash・NUL・LF／CR・dot成分を比較した。wp_normalize_pathの元の正規表現はLF直後のslash列を通常位置と同じには縮約しない。この境界も実WPへ合わせた。://を含む入力はstream wrapperの登録状態が必要なためUnsupportedValueを返す。これらの文字列変換はファイルの存在確認・解決・権限確認ではない。

wp_checkdateは整数の月日年を受け取り、PHPの年範囲1～32767とGregorianの世紀・400年規則を扱う。source_dateを解析せず、元のbytesをfilterへ渡す。通常結果と実WP dispatcherで上書きした結果を合わせて5,696ケース追加した。型変換や時刻・timezoneの互換性は含めない。API台帳は13件、対応入力の比較は合計12,947ケースになった。

## シリアライズ値の形状判定

is_serializedのstrict／looseとis_serialized_stringを追加した。全byteのtrim境界、型token、長さprefix、末尾・余分な本文、非stringのモデル値を9,003ケース追加し、合計21,950ケースが一致した。PHP trimはNULとvertical tabを除去し、form feedは除去しない。

WPの判定は浅く、宣言した長さや本文・数値の妥当性まで検証しない。例えばb:222;やa:999:garbage}がtrueとなり、is_serialized_stringは数値でない長さprefixも受け入れる。この挙動を厳密なデコーダーへ置換しない。これらの関数でデシリアライズやオブジェクト生成、DBデータの変更は行わない。strict引数はRustのboolで、PHPの任意型からの変換は未実装。

## 絶対整数とID一覧

absintは対応入力の整数cast後に絶対値を返す。PHP_INT_MINは正のintに入らないため、2^63のfloatとなる。PHP_INT_MAXへclampしたり、負数のままwrapしたりしない。十進整数文字列の前後のASCII空白6種、符号と先頭ゼロ、空文字列／空白だけの文字列、null／boolean／配列の対応入力も実関数と比較した。float、小数・指数・数値prefix後のjunk・桁あふれ文字列、object／参照等の未対応入力を黙って0へ置換しない。

wp_parse_id_listはwp_parse_listのstring／array入力と対応castを使う。arrayのnull／入れ子配列は元関数どおり除外し、重複の最初のキーと位置を保持する。string入力でも重複除去後の番号の穴を詰めない。PHPのarray_uniqueは既定で文字列表現を比較するため、対応整数castのmagnitudeだけで重複管理し、floatの丸めで2^63とPHP_INT_MAXを同一視しない。この限定実装は一般的なarray_uniqueではない。

整数専用のabsint_integer／wp_parse_id_list_integersも残し、より狭い入力契約の呼出し元へ使用できる。全体では17 API・24,395ケースが一致している。

## 条件付きシリアライズ

maybe_serializeは配列をPHPの形式で出力する。整数／boolean／null、文字列のbyte長、元のキー順を保持する。文字列がis_serializedのloose判定に該当すれば、妥当なデータかどうかを別途解釈せず、元の空白・NULを含む文字列そのものを二重シリアライズする。通常の非配列値はそのまま返し、top-levelのfloatも元のbitsを維持する。

floatのserializationはserialize_precision等の影響を受けるため、配列内floatは未対応としてUnsupportedValueを返す。maybe_serialize自体はobject／参照を扱わず、復元処理も行わない。配列の入れ子は64までのホスト制限で、PHPの限界値を示すものではない。DBのraw読取りやメタ値へ自動適用しない。追加3,316ケースが一致し、全体は18 API・27,711ケースとなった。

## 比較ハーネスの入力分割

PHP CLIの既定128 MiBで一括入力が上限に達したため、各バッチ2,048ケースかつcanonical JSONで1 MiB以内に分割する。上限を引き上げず、15バッチで全ケースが一致した。PHP版とinteger幅を各バッチで確認し、全case IDの一意性・順序・件数を検査する。input／outputのhashは全ケースを一つのcanonical JSON配列にした従来形式と同じになるよう逐次計算する。batch数・入力上限・最大実測値も証跡へ記録する。

この制約は開発用比較の入力budgetで、PHPや製品の総RAM／入力展開後メモリを保証しない。すべてのケースは独立した関数入力で、バッチをまたぐWordPressリクエスト状態を比較したものではない。

## 正規形の限定復元

maybe_unserializeは、is_serializedのstrict判定に該当しないモデル値を変更しない。該当したバイト列では外側のPHP trimを行い、正規形のnull／boolean／signed 64bit整数／バイト列／配列だけを復元する。再シリアライズしたbytesとの一致を確認し、キーの暗黙変換・重複等を含む非正規形や余剰データはUnsupportedValueとする。これは任意のPHP serializedデータのパーサーではなく、PHPの不正入力に対するfalseや抑制警告の再現も含めない。

復元は1段だけで、文字列内容をさらにunserializeしない。object／参照の展開・クラスの実行を行わず、認識済みの該当形式やfloatは明示的に未対応とする。復元配列は64段までのホスト制限。宣言された件数で先に容量を確保せず、長さ・件数を入力へ照合する。byte改変でpanicしないことも試験した。追加1,624ケースが一致し、全体は19 API・29,335ケース。

この関数をDB読取り・移行へ自動適用しない。未対応値をnullや空値に置換せず、元DBのraw値保持を維持する。

別の caller-bound helper として [明示 4 上限付き byte 復元](bounded-unserialize-development.ja.md)を追加した。入力 bytes・累計 array entry・所有文字列 bytes・array depth を個別に指定し、旧 `maybe_unserialize` は維持する。DB 接続は選択済み raw row の有限 test-only 関係に限り、正式 72 API／482,951 比較の件数・入力・両結果 SHA を保持している。

続く [準備済み非空 cache の単一 post meta 取得](post-meta-cached-development.ja.md)では、caller が完全性と関連 filter 不在を保証した有限 profile を 3,202 件で比較し、正式 73 API／486,153 件を確認した。旧 72 の件数・入力・両結果 prefix と hooks／dispatcher の結果は保持し、DB lookup／cache miss／一般 metadata policy は追加していない。

### 型付き値とfilter付きAPIの接続

開発用JSONのPHP値codecをプリミティブ・フック・dispatcherで共有する。製品ABIではない。null、bool、整数境界、NaN／無限大／負のゼロ、NUL／非UTF-8 bytes、順序付き配列の戻り値と引数を実PHPで比較した。PHPの参照・object・resourceは対象外。

sanitize_keyとwp_checkdateは呼出し元のrequest-local Dispatcherを使用し、標準のfilter名・元bytesを渡し、戻り値を文字列／boolへ変換せず返す。57ケースで型付きの上書きを追加し、合計19 API・29,392ケース。callbackエラー後のstack復元はホスト側試験であり、PHPエラー互換性の主張ではない。Wasm callback transportと公開ルートへの接続は未実装。

### wp_basenameのencoded suffix

pathだけをURL encodeし、slash／backslashを区切りとしてbasenameへ渡し、suffixを未encodeのまま照合してからURL decodeする。suffixがbasename全体と同一なら除かず、`/x `のsuffix `+`や`/x`＋0xFFのsuffix `F`など、encoded表記に対する切断も保持する。全byte・固定seed・Unicode・NUL・末尾区切りの8,495ケースが実PHPと一致。20 API・37,887ケース。これは表記処理で、ファイルの存在やアクセス許可を判断しない。

### UTF-8の明示的な3処理

wp_is_valid_utf8はBOM・NUL・noncharacterを許し、overlong／surrogate／U+10FFFF超を拒否する。wp_scrub_utf8は不正な最大部分列単位でU+FFFDへ置換する。wp_has_noncharactersは元ソースと同じbyteパターンを探し、前後の不正byteによって検出を失わない。この3処理を相互の代用や自動DB変換へ使わない。

全65,536の2byte列を妥当性判定し、上位lead byteの全2byte列を置換／検出でも比較。Unicode境界・全66 noncharacter・切詰めとbyte改変を含め、各68,436／19,284／19,284ケースが一致した。合計23 API・144,891ケース、72バッチ。PHPのmbstringロード状態を各バッチで確認し、php_mbstring_loaded=falseとutf8_oracle_path=no_mbstring_fallbackを保存・検査する。mbstring有効経路、他scalar型のPHP変換や診断は未比較。

### charset slugと固定UTF8 profile（2026-10-27、正式比較済み）

is_utf8_charset_bytesは固定functions.php:7715→compat.php:102の、明示string引数だけを扱う。UTF-8／UTF8をASCII大文字小文字無視で完全長照合し、空白trim／Unicode casefold／NUL終端は行わない。割当や状態を持たず、Null／引数省略によるget_option参照、非string、製品のcharset選定は未対応。case違い16 aliasと不正byte／余剰／空白／NUL／Unicode類似の2契約を追加した。

wp_check_invalid_utf8_utf8_no_mbstring_bytesは固定formatting.php:1127のbyte入力だけを、sitecharset=UTF8・mbstring無しの明示profileで扱う。empty／validはそのまま保持し、invalidでstrip=falseは空byte列、strip=trueは既存wp_scrub_utf8_bytesのmaximal ill-formed subpartごとのU+FFFD補修を返す。BOM／NUL／noncharacterをvalidから除かず、Unicode正規化・HTML escaping・DB自動変換はしない。PHP staticのcharset cacheを実行中に切替えるprofileやscalar cast／非bool stripは未対応。新2契約でvalid／invalid両strip、境界と入力不変を確認する。

oracleは既に固定UTF8 option stubとno-mbstringを実測する同じprofileを使い、実is_utf8_charset／wp_check_invalid_utf8を呼ぶ。後者のstripはPHP／native probe双方でexact boolを要求する。既存JSON profileとraw UTF8 helpersの入力範囲は変えない。正式比較はis_utf8_charset 8,936件、wp_check_invalid_utf8 8,782件が一致し、全47 API・328,081ケースとなった。新4 Rust契約を含むworkspace291件・Python96件、HTTP・Clippy・Linux cross-check・license208観測・既存DB証跡をtarget/charset-and-invalid-utf8-profile-check.logで確認した。独立レビュー後にPHP oracleへ両APIのbyte-string guardを追加し、target/charset-and-invalid-utf8-oracle-domain-check.logでprimitive／hook／dispatcherを実再比較して証跡を更新した。Rust／HTTP等は意味変更しておらず再実行していない。

独立したtarget/utf8-bytes-only-guard-observation.{py,json}では、Null／bool／integer／NaN・infinity含float／arrayの56 process検査を両実装が特定guardで拒否し、byte positive21ケース・42 processは一致した。46／47のfocused8,936／8,782観測や非bool／欠落strip拒否もtargetへ保存した。正式corpusと同じcaseのfocused観測を重複加算せず、補助的な拒否観測も正式47 API件数へ加算しない。DB live／独立experimentは今回再実行せず、直前の21件・4 fixture各39項目をcurrent hash確認で維持する。

### JSON media判定のC byte profile（2026-10-27）

wp_is_json_media_type_c_bytesは固定load.php:1964の次のPCREが成功するC-locale byte入力に対し、return boolだけを扱う。

```text
/(^|\s|,)application\/([\w!#\$&-\^\.\+]+\+)?json(\+oembed)?($|\s|;|,)/i
```

検索はunanchoredで、左は先頭／ASCII空白6種／comma、右は末尾／同空白／semicolon／comma。大文字小文字はASCIIだけ無視する。classは!／#／$、ASCII38～95、a～zの87byteで、slash／comma／semicolon／backslash／brackets等も含む。一般的なMIME tokenへ置き換えず、application/a;b+jsonやapplication//+jsonをtrue、application/+jsonとapplication/%+jsonをfalseにする。先行prefixがあればapplication/x,application/+jsonもtrueとなる。invalid UTF8は全体を拒否せず、例えばその後のspaceと一致fragmentでtrueになりうる。NUL直後に一致を開始する特別扱いはない。

nativeはclassの連続runで最初のcontent開始位置を保持し、各byteで定数長lookaheadだけを使う。後続候補を同じrunで何度も全走査せず、O(n)・追加割当なし。WordPressの内部memoization、非string診断、非C locale、PCRE資源失敗は再現しない。MIME parameter／JSON内容の解析・HTTP header認可・UTF8補修を提供しない。

oracleは実WP関数の前にPCRE状態をresetし、実関数後のPREG_NO_ERRORも要求する。cache hitに以前のerrorを誤適用せず、resource failureをfalse成功へ混ぜない。Bytes-onlyを両probeで守り、PHP oracleでC localeを照合する。新4 Rust契約と正式7,072件が一致し、48 API・335,153ケースになった。target/json-media-type-c-profile-check.logでworkspace Rust295件・Python96件、HTTP・Clippy・Linux cross-check・license208観測・既存DB証跡を確認し、別途binlog／MySQL overlap／Maria bootstrapのhashも照合した。DB live／独立experimentは再実行せず、直前21件・4 fixture各39項目を保持する。

target/json-media-type-observation.{php,json}の805観測、focused比較7,072件、非string28 process拒否、低いPCRE backtrack_limitを使う明示resource-error拒否は補助観測。正式件数へ重複加算しない。独立レビューでもearliest-prefixの同値性、index安全性、型／locale／PCRE guardと限定範囲を確認した。

### JSON／XML要求hintの明示ヘッダー（2026-10-27、正式比較済み）

固定load.php:1917のwp_is_json_requestは、wp_is_json_request_c_bytes(accept, content_type)へ移植する。AcceptをC-profileのmedia helperへ先に渡し、falseの場合だけContent-Typeを判定する。固定load.php:1982のwp_is_xml_requestはwp_is_xml_request_bytesへ移植し、Acceptでは6literalのcase-sensitive substring、Content-Typeでは同集合の完全一致を使う。独立したwp_is_xml_media_typeは固定treeに存在しない。

XMLの集合はtext/xml、application/rss+xml、application/atom+xml、application/rdf+xml、text/xml+oembed、application/xml+oembed。application/xml単独はfalse。たとえばAcceptのprefixtext/xmlsuffixやtext/xml; charset=UTF8はtrueだが、同じContent-Typeはfalse。JSON／XMLは独立hintなので両方trueになりうる。q=0も解釈しない。

両kernelはOption<&[u8]>を受け取り、Noneはmissing／PHP Nullのisset=false、Some(empty)は存在する空文字列を表す。stateless・追加割当なし・read-onlyで、body解析やproductのHTTP処理へ接続しない。両headerともBytes／Noneだけが対応範囲であり、型違いは短絡で隠さず事前拒否する。JSONのPCRE失敗／memoization／非C、他SERVER fieldや非string診断は未対応。XMLのliteral比較はlocaleに依存しない。

開発protocolのinputはNull sentinelで、accept／content_type fieldにoptional typed Bytes／Nullを持つ。oracleは2つのSERVER fieldだけを存在bit・値とともに保存し、missing／presentNullを区別して設定、実WPの0引数関数を呼び、finallyで元の状態へ戻す。JSONは同じ短絡順にmedia helperのPCRE成功を事前確認してcacheをprimeし、reset後の実request呼出しでもerror0を要求する。最初のheaderのresource errorが2つ目の成功で隠れる比較を防ぐ。PHPのcache／PCRE global状態をnativeへ再現する保証ではなく、return boolの対応subsetである。

新4契約に加え、各3,966ケースの正式比較を用意した。target/request-media-guard-observation.{py,json}では悪型／非Nullsentinel44 process拒否、初期SERVER4状態×8連続caseの復元、低PCRE limitによるAccept固有error拒否を別観測した。これらの補助観測を正式API件数へ加算しない。target/request-media-hints-check.logで両API各3,966件が一致し、全50 API・343,085ケースとなった。workspace Rust299件・Python96件、hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashも成功。binlog／MySQL overlap／Maria bootstrapも別途照合し、DB live／独立experimentは再実行せず、直前21件・4 fixture各39項目を維持する。独立レビューでもheader復元／型／PCRE順序／native非対称のscopeを確認した。

### ゼロ詰めと限定byte量変換（2026-10-27）

zeroiseは固定formatting.php:2792のsprintf %sを、i64整数とraw Bytesに限って扱う。整数は最大20byteのstack上で十進化し、Bytesは解析や正規化をせず借用する。最小幅に足りない分を文字列全体の前へ0として追加し、zeroise(-1,3)は0-1となる。UTF8文字数ではなくbyte長で測り、NUL／invalid UTF8／leading0を保持して切り詰めない。幅0..4096とcallerの出力capはprototype host制約で、PHPや製品の容量仕様ではない。domain／幅／長さ検査後にだけtry_reserve_exactし、失敗時はtypederror・部分出力なし。新4 Rust契約、正式2,769ケースが一致した。

wp_convert_hr_to_bytes_default_units_bytesは固定load.php:1687から、ASCII [0-9]+と単一optional k/m/g（大小可）、先頭0可、有効256桁までを扱う。全byteを検証してから数値化し、default KB=1024／MB=1048576／GB=1073741824でi64MAXへ飽和する。定数はdefault-constants.phpのliteral profileに束縛し、wp_initial_constantsを実行してiniやmemory設定を変更しない。PHP各batchの定数の型・値と有効桁上限をmetadata／checkerで要求し、actual返却もintであることを照合する。これは純粋値の部分集合で、unlimited／製品資源容量の意味を選ばない。

最初の無制限digits飽和仮説は反証された。4,096桁の9と7種類のsuffixでは、PHPの巨大数値変換がInfinityを経由してint0となり、当初nativeのMAXと7差分が出た。target/hr-bytes-unbounded-saturation-counterexamples.jsonに失敗を保存し、有効digits<=256へdomainを明示的に限定した。256桁以下はfiniteで、4,096桁のleading0＋1／MAX／256桁9／allzeroの28実PHP観測もint・診断0だった。257／4,096有効桁とleading0＋257桁を42 processの明示protocol拒否へ移した記録はtarget/hr-bytes-significant-domain-refusals.{py,json}。formalから7件を除外した理由とcounterexampleを隠さず、範囲外の汎用PHP変換を実装した扱いにしない。

min($bytes, PHP_INT_MAX)の引数順も固定PHP8.5.12で型まで実測した。K/M/Gの2^63積境界では元の順序でintMAXとなり、順序反転でfloatになる場合がある。一般PHPのtie規則として横断保証せず、選択した有限profileでinteger飽和を使う。新3 Rust契約・正式2,821ケースが一致した。負値／符号／空白trim／小数／指数や任意文字列内g>m>k検出・float返却は未対応。

比較用php_value_codecのint wireは、PHPが範囲外をcastでclamp／junk除去／巨大値0へ変える穴も補修した。PHP64・string・optionalASCII符号・全decimalbyte・正MAX／負MIN別のlexical範囲を検証し、leading0除去後のcanonical値だけをcastする。Rust i64 parserで扱える+/-/leading0を保持し、nested value／array keyも同経路。新synthetic Python3契約はWordPress bootstrapなしでvalid境界・悪型／範囲外・nestedの拒否を検査する。Bool／Floatなど他wire schemaや製品Wasm ABIは今回変更していない。

target/zeroise-and-bounded-hr-bytes-check.logでworkspace Rust306件・Python100件、52 API・348,675ケース／hooks572／dispatcher405、HTTP・Clippy・Linux cross-check・license208観測・DB current hashが成功。native probe各checkerも別途照合し、DB live／独立experimentは再実行せず、直前21件・4 fixture各39項目を維持する。focused・guard・counterexample観測は正式件数へ重複加算しない。

### queryと数値castの追加範囲

wp_parse_strはarray記法を除くbyte queryを解析し、request-local filterの型をそのまま返す。wp_parse_argsは文字列だけこの処理を通し、配列argsは既存の配列mergeを使う。元PHPの出力参照はnative戻り値で表現し、一般の参照は再現しない。空defaultsと非空defaultsの分岐も保持する。

fixtureはarg_separator.input=&／max_input_vars=1000を各バッチで検査し、php_parse_str_settingsへ記録。nativeは非空component 1,000件までで、array記法やNUL key等の未対応入力ではfilterを呼ばず全結果を拒否する。PHPの切詰めprefixや警告を再現したとは主張しない。

absintの有限floatは[-2^63,2^63)で0方向へtruncateし、その後absする。NaN・無限大・範囲外はPHP 8.5の警告契約を未実装なのでUnsupportedValue。floatから得たIDも元キーを保って重複除去し、2^63のfloatとPHP_INT_MAXを混同しない。absint 2,931ケース、ID一覧1,318ケースを含め、全24 API・148,608ケースで一致した。

### balanced配列queryとUUID

balanced array記法の内側キーはdotを変更せず、空括弧をappendとして扱う。scalar／arrayの置換、重複時の元位置、負数からの採番と数値文字列キーを実PHPで照合した。64 bracket levelsまでを含む。65以上はホストのRecursionLimit、不正括弧・後続文字・内側キーのASCII空白・NUL・append overflowはUnsupportedValue。PHPが許す不正表現や診断まで再現したものではない。fixtureはmax_input_nesting_level=64も検査し、設定差を混ぜない。

wp_is_uuidは小文字だけのregexを再現。版nullならversion／variantを限定せず、整数4ならv4と[89ab]を要求する。1個の末尾LFがあってもregex $の挙動でtrueとなる。非文字列なら版指定の診断より先にfalse。その他の版型変換と_doing_it_wrongは未対応。36位置の全byte変更など18,901ケースで一致。これは形式だけの判定で、IDの生成・一意性・真正性を証明しない。

全25 API・168,769ケース。ケース生成の実行・IDの全体一意性・登録API名の一致を通常試験でも検査する。API母集合全体の列挙や完全互換性・対応率ではない。

### JSONP callbackのbyte規則（2026-10-26）

固定functions.php:4699のwp_check_jsonp_callbackを追加。is_stringで先に分岐し、C localeではASCII英数字・underscore・dotだけを許可する。元のregexは空文字・dotだけ・先頭数字も受理するため、その挙動を保ち、JavaScript識別子の構文規則へ置き換えない。NUL・改行・他の記号・非ASCII byteは拒否する。

PHP oracleはLC_CTYPEの現在値を各batchで実測し、C以外なら比較を失敗させる。jsonp_ctype_locale=Cとして証跡へ保存し、C以外のlocale分類を比較済みにしない。全byteを4配置へ置き、固定seedの文字列と非文字列入力も含めた2,056ケースが一致した。全26 API・170,825ケース、フック572／dispatcher405の証跡を更新。WP crateのテスト72件（今回3件追加）とClippyが通過した。

これは元WPのbyteパターンの部分互換で、一般のJavaScript構文検査、出力escapingや安全性の保証、JSONP endpointの追加・有効化・認可ではない。object／参照・他localeは未完了。ログはtarget/jsonp-primitives.log、target/jsonp-hooks.log、target/jsonp-dispatcher.log、target/jsonp-wp-tests.log、target/jsonp-wp-clippy.log。

### リストのroot bool／signed integer（2026-10-26）

wp_parse_listはroot falseを空文字列、trueを`1`、signed integerを十進byte列として分割する入力範囲を追加した。root falseは空配列だが、整数0はbyte文字列`0`の1要素となる。wp_parse_id_listも先にこのリスト化を通すため、falseは空、trueはID 1、整数0はID 0となる。PHP_INT_MINは既存absintの契約に従い、絶対値が2^63のfloatになる。

root Nullは空配列という値だけで互換済みにしない。固定PHP 8.5.12はpreg_splitにNullを渡す際、E_DEPRECATED（8192）を出すことを両APIで観測した。診断未対応のためnativeはUnsupportedValueを維持する。別観測はtarget/list-root-null-diagnostic-observation.jsonで、値と診断を併記し、native等価性の証跡とは区別した。通常oracleの診断拒否も変更していない。

root floatのformat、object／resource／参照は未対応のまま。配列内scalarの既存の型保持や、wp_parse_id_listの対応float配列値、整数専用helperのドメインは変更しない。bool2件・整数境界と固定seed計1,008件を両APIへ追加し、2,020ケースが一致。全26 API・172,845ケース、フック572／dispatcher405の3証跡を更新した。WP crateテスト75件（今回3件追加）とClippyが成功。ログはtarget/list-root-scalar-{primitives,hooks,dispatcher,wp-tests,clippy}.log。

### 空白byteの正規化とHTML風区切りの分割（2026-10-26）

固定formatting.php:5580のnormalize_whitespaceをnormalize_whitespace_bytesとして実装。両端のPHP default trimはspace（0x20）・TAB（0x09）・LF（0x0a）・CR（0x0d）・NUL（0x00）・VT（0x0b）の6byteだけを除く。その後CRをLFへ置換し、連続LFを1つ、続いて連続space／TABをspaceへ縮約する。行ごとのtrimは行わず、改行間のspaceを残す。FF（0x0c）やUnicode空白はtrimせず、内部NUL／VT／FF・非UTF-8 bytesを保持する。Unicode正規化・出力escaping・非byte入力のPHP coercionは含めない。Rust3件と実PHP9,155ケースが一致した。

固定formatting.php:615のwp_html_splitと:626のget_html_split_regexに従うwp_html_split_bytesを追加。textとcaptureしたHTML風区切りを交互に返し、先頭・末尾・隣接区切り間の空textを残す。空入力は空byte列1要素となり、`<a></a>`は空text・`<a>`・空text・`</a>`・空textとなる。exactな`<!--`／`<![CDATA[`だけを専用終端で扱い、通常の`<`は引用符内でも最初の`>`で終える。例えば`<a title='>'>x`もHTML属性として解析しない。閉じない区切りは末尾までをcaptureし、不正HTMLを修復しない。全byteとNUL／非UTF-8を保存する。HTML parser・妥当性検査・sanitization・出力escapingではなく、PHP coercionとPCRE資源上限エラーは未対応。Rust5件と実PHP10,000ケースが一致した。

追加計19,155ケースにより全28 API・192,000ケース、workspace Rust144件となった。当時のログは`target/whitespace-and-html-split-full-check.log`。DB実測の履歴`target/snapshot-anchor-and-shared-budget-full-check.log`は当時Rust136件＋独立experiment5件のまま保持し、今回DBを再起動した記録へ置き換えない。

### HTML風区切り内のbyte置換（2026-10-26）

固定formatting.php:757のwp_replace_in_html_tagsをwp_replace_in_html_tags_bytesとして追加。haystackはbyte列、置換表はPhpArrayのbyte key／byte valueに限定する。空mapとsingle空key＋byte値は元bytesを返す。全entryを出力割当・分割より前に検査し、整数key、PHP配列で整数へ正規化される数値key、非byte値、multiple空keyはUnsupportedValueとする。keyが現れない入力でも対応範囲外として拒否する。

既存wp_html_split_bytesのodd delimiterだけを置換し、plain textを保持する。singleは重複しない一致を置換、multipleは各位置で最長needleを優先し、map登録順で短いneedleを先に採用しない。どちらも元入力だけを走査し、挿入bytesを再置換・再splitしない。例えばa→b／b→cで`<a>ab`は`<b>ab`となる。引用符・未閉じcomment等の範囲も元splitの挙動を維持し、HTML parser・sanitization・出力escapingを提供しない。

isolated PHP 8.5.12で47件の型・診断観測を別途保存した（`target/html-replace-observation.json`）。single null値はneedleがdelimiterで一致したときだけE_DEPRECATED8192、single array値は同条件でTypeError。multiple array値は実際の置換時だけE_WARNING2、multiple空keyはdelimiterごとにE_WARNING2となる。PHPのcoercionと遅延診断は未実装で、警告を無視して対応済みにしない。この47件はnegative／探索観測であり、native等価性のcorpus件数へ加算していない。

対応入力10,344ケースが実PHPと一致し、全29 API・202,344ケースとなった。追加Rust契約5件を含むworkspace149件・Python44件、フック572／dispatcher405、HTTP・Clippy・Linux向けcross-checkが成功。ログは`target/html-replacement-full-check.log`。この時点のWP gateの記録で、並行するDB byte-limit実装や新しいDB実測を完了と扱わない。

### URL風byte列とsigned短縮長（2026-10-26）

固定formatting.php:4905のwp_make_link_relativeは`|^(https?:)?//[^/]+(/?.*)|i`に沿い、ASCII case-insensitiveなHTTP(S)またはprotocol-relative prefixと非空authority相当を除く。これはURL構文ではなく、query／fragment／LF／NULも`[^/]+`の対象になる。group2のdotがLFで止まっても未一致suffixが保持されるため、最初のpath slash以降の全bytesを残す。7,184ケースで比較した。

固定formatting.php:6318のurl_shortenはhttps://→http://→www.を全位置でcase-sensitiveに順次削除し、既存untrailingslashit後のstrlenがlength超ならsubstr(0,length-3)＋`&hellip;`を返す。全signed i64 lengthを対応し、負数・0～2・i64::MIN／MIN+1／MIN+2も実PHPと比較した。負のsubstr長は末尾からの除去であり、常に空とは限らない。減算がfloatへ昇格する最小値付近でも固定64bit PHPの結果を確認した。5,650ケースが一致。byte長のためUTF-8が途中で切れることがあり、URL parser・表示幅算出・escapingではない。非byte URLと非整数lengthのcoercion／診断は未対応。

### hex色のbyte入力（2026-10-26）

固定formatting.php:6339／6366／6388の3関数を追加した。sanitize_hex_colorは空なら空、それ以外は`|^#([A-Fa-f0-9]{3}){1,2}$|`に一致した元bytesかnullを返す。末尾1LFを受理・保持するが、2LF／CRLF／CR／NUL／空白は拒否し、case変換をしない。sanitize_hex_color_no_hashは先頭#を全て除去し、空なら空、それ以外はhash付きvalidatorで照合する。nativeはOption<Vec<u8>>でPHP nullと空文字を区別する。

maybe_hash_hex_colorはno_hash結果がtruthyなら#を1個付ける。empty結果はfalseのため`#`／`###`の元bytesを保持し、invalidも元のまま返す。`000`はtruthyで`#000`となる。3関数ともbyte入力限定で、root Nullのpreg_match／ltrim E_DEPRECATED8192、非string coercionや型付き返却は未対応。3／6桁hex以外のnamed color・rgba・4／8桁hex等を採用せず、一般CSS parser・出力escaping・安全性を提供しない。

各8,936ケース、3関数計26,808ケースが一致した。target/hex-color-observation.jsonの87探索観測はnative等価性のcorpusへ加算しない。全34 API・241,986ケース、workspace Rust164件・Python44件、フック572／dispatcher405、HTTP・Clippy・Linux向けcross-checkが`target/hex-colors-and-signed-url-lengths-full-check.log`で成功。この34 API時点より後にsanitize_html_classを実装・接続した（下記）。

### classnameのbyte処理とterminal filter（2026-10-26）

固定formatting.php:2438のsanitize_html_classを実装した。`%[a-fA-F0-9][a-fA-F0-9]`を単一passで削除し、続いて`[^A-Za-z0-9_-]`を除く。percent decode・再走査・case変換はしない。cleanが空かつfallbackがPHP-truthyならfallbackをclassnameとする既定fallback空の呼出しを返す。byte fallbackの空と`0`はfalse、`00`はtrue。clean自体の`0`は空ではないのでfallbackへ進まない。

terminalのsanitize_html_class filterだけを1回呼び、引数順はclean／raw classname／fallback。fallback使用時はrawが元fallback、3番目は空となり、外側filterは追加しない。request-local Dispatcherでnull／array等のtyped戻り値を無変換で保持し、callback失敗後のstack復帰・再利用も試験した。非byte入力のcoercion／診断とWasm callback転送は未対応。filter後の値のclassname妥当性や出力escapingは保証しない。

追加7,255ケースが一致し、全35 API・249,241ケース、Rust169件・Python44件、フック572／dispatcher405、HTTP・Clippy・Linux向けcross-checkがtarget/html-class-filter-full-check.logで成功。続くmemory候補でWPソース変更はなく、PHP再実行を省略しregistry／証跡hashを照合した。後続ログtarget/memory-observation-and-html-class-check.logはRust173件・Python44件で、Linux native観測やDB再実測を意味しない。

### 標準feed serializerとの境界（2026-10-26）

serverへ追加した[RSS／Atom候補](feeds-development.ja.md)は標準XML serializerであり、WP feed template／query／filterを移植したAPIではない。35 API・249,241ケースの台帳や対応率へ加算しない。独立XML解析4件とHTTP配信1件・serializer7件を含む後続gateはRust181件／Python48件。WPソースはclassname gateから変更しておらず、PHP再実行を省略して現在hashを照合した。公開記事の選別・private data対策・comment feeds・連続公開は未実装。

### form属性のreturn-only subset（2026-10-26）

固定general-template.php:5515／5533／5551／5569とprivate helper:5597に従いchecked_return／selected_return／disabled_return／wp_readonly_returnを追加した。両入力を先に検証し、Null／falseは空、trueは1、signed i64は十進、Bytesは元bytesという明示(string)変換後に厳密比較する。nullとfalseは一致、falseと整数0は不一致、整数1と01は不一致。MIN／MAX・NUL・非UTF8を保持する。一致時は先頭spaceとsinglequoteを含む固定属性、そうでなければ空bytesを返す。

Float／Array／Unsupported等は等しい値や無一致の場合も両側の対応範囲外として拒否する。NaNの明示castはE_WARNING2、Array castもE_WARNING2を観測し、無視して互換済みにしない。312探索観測はnative等価corpusへ加算しない。display=falseだけを比較し、PHP既定display=trueのecho・stdout ABIは未実装。旧readonly()はPHP<8.1限定で、現APIはwp_readonlyを対象にする。required field indicator／messageも別の翻訳・filter付きAPIとして未実装。

各4,313ケース、4 API計17,252ケースが一致し、全39 API・266,493ケース。実PHP比較はtarget/form-attributes-and-feeds-full-check.log内のWP段階で成功・証跡保存済みで、後段DBの失敗と区別する。以後WP無変更のため最終target/precommit-process-kill-final-check.logはPHP比較を省略しhash照合。Rust185件＋独立experiment9件・Python59件、フック572／dispatcher405、HTTP・Clippy・Linux向けcross-checkも成功。標準RSS／Atom serializerはこのAPI数へ加算しない。

### 非WP候補の後続検証（2026-10-26）

target/filtered-postmeta-and-cgroup-full-check.logでRust190件＋独立experiment13件・Python61件が成功した。39 API・266,493ケース／フック572／dispatcher405のWPソースは変更しておらず、PHP再実行を省略してregistry／証跡hashを照合した。標準feedサンプルはself linkを各RSS／Atom公開routeへ揃え、既存HTTP1件と独立XML4件でも確認。cgroup 1 level観測とDB same-DB選別fixtureもWP互換APIとして加算しない。Linuxはcross-checkで、native観測・production filtering／連続公開の合格ではない。

### localeとMIMEのbyte文字集合（2026-10-27）

固定formatting.php:2468のsanitize_locale_nameは`/[^A-Za-z0-9_-]/`、:5800のsanitize_mime_typeは`/[^-+*.a-zA-Z0-9\/]/`に沿うbyte whitelistを実装した。MIME側はunderscoreを残さず、caseはどちらも維持する。例えばen_US.UTF-8@euroはen_USUTF-8euro、Text/HTML; charset=UTF-8はText/HTMLcharsetUTF-8となり、`*/*`や`///**..++--`もそのまま残る。OS localeの存在・MIME／RFC構文・HTTP header設定・出力escapingを検証するAPIではない。

request-local Dispatcherの固定named filterにclean／元raw bytesの2引数を渡し、null／array等のtyped戻り値を無変換で保持する。全256byte・case・punctuation・empty／wildcard・不正shape、元NUL／非UTF8引数、callback失敗後のcontext復帰と再利用をRust4件で試験した。対応入力はbyte列だけで、その他scalar coercion、PHP診断、object／参照、Wasm転送とWP全体の実行は未対応。

各4,095ケース、計8,190ケースがPHP 8.5.12と一致し、全41 API・274,683ケース、フック572／dispatcher405の証跡を更新した。target/locale-mime-and-filtered-postmeta-check.logでworkspace Rust194件・Python61件、HTTP・Clippy・Linux向けcross-checkとDB／binlog current hash検査も成功。直前のDB実測を維持し、このWP gateで実DB／独立experimentを再実行したとは扱わない。

### emailのbyte検査と3引数filter（2026-10-27）

固定formatting.php:3613のis_emailをbyte入力・既定deprecated=falseのsubsetで実装した。拒否contextは順にemail_too_short、email_no_at、local_invalid_chars、domain_period_sequence、domain_period_limits、domain_no_periods、sub_hyphen_limits、sub_invalid_charsの8種類。domain labelは各labelでhyphen／trim検査を先に行い、続いてinvalid charsを調べるため、後方labelの別エラーより前方labelのエラーを優先する。success時は元bytesを返しcontextはPHP Nullとする。

offset1以降の@存在検査と、その後のfirst@での分割は別処理。local regexの末尾1LFを許すため、`a\n@b.cd`も元LFを残して成功する。2LFやCRLFへの一般的trimはしない。domainはPHP trimの6byte＋dot、labelは6byte＋hyphenのedge規則を守り、Unicode文字列へ変換しない。localの連続dotや短いdomain label等もWPの元挙動を保持するため、RFC全準拠・DNS lookup・配送可能性・security保証・出力escapingと混同しない。

terminalのis_email filterへresult（false／元bytes）・元email・contextの3引数を渡し、null／array等のtyped戻り値を無変換で保持する。全sampleに対しemail_arguments modeでも3引数全体を実PHPへ照合し、未登録filterとconstant typed filterも比較した。Rust5契約は全256byte、@／LF／trim／優先順、typed戻り値、callback失敗後のcontext復帰を検査する。非byte coercion・診断、非default deprecated引数、object／参照、Wasm callback転送とWP全体実行は未対応。

追加8,177ケース、全42 API・282,860ケース、フック572／dispatcher405が一致。target/email-and-mysql-overlap-check.logでworkspace Rust199件・Python68件、HTTP・Clippy・Linux向けcross-checkと通常binlog／overlap current hash検査も成功した。今回は実DB／独立experiment再実行なしで、直前DB gateの194件＋experiment19件・各36項目×3 fixtureという履歴を保持する。

## 明示contextによるwp_unique_id／wp_unique_prefixed_id（2026-10-27）

固定functions.php:8156／8175のfunction staticを、hostがrequest／execution単位で所有するUniqueIdsへ移した。Default／newで両counterを開始し、Cloneやprocess global staticは提供しない。wp_unique_idはprefix全体で1 counter、wp_unique_prefixed_idは正確なbyte prefixごとに1 counterを持ち、両API間は独立。空・numeric・case差・NUL・非UTF8のprefixを保持する。signed i64上限までPHPの十進bytesに一致し、その後はUnsupportedValueでcounter状態不変とする。PHP float overflowまでの互換性は主張しない。

比較driverはbatchごとにPHP／nativeの新processを起動し、同じ順序で交互呼出しする。PHP function staticとnativeの1 UniqueIds contextはそのbatch全体で保持する。証跡unique_id_state_scopeはone_php_process_and_one_native_context_per_input_batchを必須とし、per-case reset／永続globalの記録を拒否する。各API2,069ケース、計4,138ケース追加で44 API・286,998ケースが一致した。private counter上限の状態不変・独立context・interleaving・全byte prefix・連結衝突はRust契約5件で確認する。prefix連結の例a1＋1とa＋11は同じa11になり得るため、普遍的な一意性やsecurity tokenの保証ではない。

非string coercion・PHP診断、float counter overflow、host request lifecycle／budget統合・Wasmは未対応。target/unique-id-and-visible-cgroup-check.logでworkspace Rust209件・Python68件、HTTP・Clippy・Linux向けcross-checkとbinlog／overlap current hash照合が成功。可視cgroup観測5件も含むがWP API数へは加算しない。今回live DB／独立experiment再実行はなく、直前DB gateの194件＋experiment19件・各36項目×3 fixtureを履歴として保持する。

## default wp_json_encodeの対応入力（2026-10-27）

初回導入では固定functions.php:4443のwp_json_encodeについて、最初のjson_encode(value, flags=0, depth=512)が成功する経路だけを移植した。Null／Bool／signed i64／valid UTF8 bytes／ordered PhpArrayを扱い、空配列と挿入順integer key 0..n-1だけをJSON array、それ以外を順序を保ったJSON objectとする。既存PhpArrayのnumeric key正規化を使い、objectのinteger keyは十進文字列にする。slash／quote／backslashとcontrolを既定どおりescapeし、非ASCIIはlowercase UTF16 surrogateを含む\u形式、DELはraw byte、U2028／2029はescapeを保持する。HTML文字<>&'はrawのため、HTML／script escapingやsecurity保証として使うAPIではない。

hostの配列深度は64で、root arrayを1と数え、empty arrayも消費し、scalar葉は追加消費しない。PHPの既定512を再現する上限ではない。2passで最初に対応domain／深度を検証しchecked output長を計測してから、callerの完全出力byte capを照合、reserve後にencodeする。UnsupportedValue／RecursionLimit／OutputLimitを返し、部分出力は返さない。OutputLimitはhostのbyte上限・算術overflowに加えて出力reservation失敗も含み、PHPのfalse sentinelやflagsではない。入力は不変で、RSS上限やshared budget統合・自動配分を意味しない。

Float、invalid UTF8のWP fallback（_wp_json_sanity_check／_wp_json_convert_string）、object／resource／参照、非default depth、Wasmは未対応。flagsの後続拡張は下記に分ける。比較corpusの入れ子は開発用PHP値transportの範囲内の24段までとし、64段のhost境界はRust契約で検証する。別のPHP有限探索76件でcontainer数・scalar葉・escape等も観測したが、その探索をnativeの差分比較5,303件へ加算したり、transport越しの64段比較と扱ったりしない。

追加5,303ケース、全45 API・292,301ケース、フック572／dispatcher405が一致した。target/json-encoding-and-license-check.logでworkspace Rust224件・Python77件、HTTP・Clippy・Linux向けcross-check、license観測208件とDB／binlog／overlap current hash照合が成功。実DB／独立experimentの再実行はこのgateに含めない。このdefault gate後は一度ユーザー指示で停止したが、その後の再開指示で停止は解除された。

## selected JSON flagsの拡張（2026-10-27）

初回のselected flags拡張では、既存wp_json_encodeをflags=0 wrapperのまま、新wp_json_encode_with_flagsとprivate u32のJsonFlagsを追加した。Default0／from_bits／bitsを持ち、HEX_TAG1／AMP2／APOS4／QUOT8、FORCE_OBJECT16、UNESCAPED_SLASHES64、UNESCAPED_UNICODE256、UNESCAPED_LINE_TERMINATORS2048のmask2399だけを受理する。unknown bitはUnsupportedFlags。全256組合せを扱い、HEX_TAGのC／EだけはPHPどおり大文字の\u003C／\u003E、他の非ASCII hexはlowercase。LINE単独ではU2028／2029は変わらず、UNICODE併用時のみrawとなる。keyとvalueで同じescapeを使い、FORCE_OBJECTはnested／empty／listも含む全containerへ適用する。

対応値・64container host深度・2passは維持し、escaping後の完全出力長でcapを判断する。partial output、Float／invalid UTF8 fallback、未選択flagsや非default PHP depth・Wasm・RSS／budget統合は未対応で、HTML／script安全性を保証しない。詳細は[JSON encoding開発記録](json-encoding-development.ja.md)。Rust契約11件とClippyが成功し、独立レビューで新flags7,656ケース／全256組合せのPHP比較は0 mismatch。scratch探索1,792＋175件は正式corpusへ加算しない。その後target/json-selected-flags-full-check.logで全体比較も成功し、後続のbudgeted-meta-view-and-loader-cache-final-check.logでも同じ45 API・299,957ケースの比較を再実行して通過した。

## 共有論理予算ownerと開発loaderの後続gate（2026-10-27）

BudgetedPostMetaViewはrows／indexのlogical leaseを共有poolへ接続し、入力のstrict検証とfinal charge計算後に予約してからpayload clone／index buildを行う。旧世代と新候補・index rebuildは合計chargeを保持し、Arc共有は1 chargeのまま最後のownerまで延長する。明示index capによるScanへの切替と、旧indexを保持するBudgetErrを区別する。loaderは同じsnapshotのlengths preflight→reservation→body取得で、1MiB値の予算拒否時に本文転送を避ける有限fixtureを確認した。入力・driver buffers・allocator capacity・peak／RSSや全CSR等の予算は対象外で、R61全体や製品CDCの完成ではない。

loader snippet cacheの48件bytes一致と有限warm benchmarkのscopeは維持する。初回full-checkはschema tool hashの古さで最後に失敗し、DDLを実再生成して本文SHA不変を確認、起動前generator guardとprovenanceを補修した後のfinal-checkで全成功した。[loader開発記録](reference-loader-development.ja.md)と[依存台帳](dependency-inventory.ja.md)を参照する。db→coreは既存local依存の接続で新third-party packageを追加せず、workspace155／probe188／unique208（registry203・local5）を維持する。

## PRETTY_PRINTと共有raw bufferの後続checkpoint（2026-10-27）

JSON_PRETTY_PRINT128を追加し、JsonFlagsのallowed maskは2527、9flags全512組合せとなった。空containerは[]／{}のcompact形を保持し、nonemptyだけ4space indent・LF・object colon後spaceを付ける。FORCE_OBJECTのnested／empty適用、key順序とHEX／Unicode／LINE／slashは変わらず、default wrapperはflags0のまま。2passはescapingとindent後の完全bytesでcapを検査し、深さ×indentもchecked。新4件を含むRust契約15件が通過し、正式corpusは4,114件追加でJSON 17,073件／WP45 API・304,071ケースとなった。scratch2,560件と独立4,641件は加算しない。deep PHP transport24とnative host64も区別する。[JSON開発記録](json-encoding-development.ja.md)。Float／invalid UTF8 fallback／未選択numeric・partial等flags／object・resource・参照／WasmとRSS予算の残件、HTML／script安全性非保証は維持する。

BudgetedChangeBufferは既存ChangeBufferの構造／DDL優先／GAP／局所cap検査後、rowを保持する前にraw Vec capacity＋既存per-row allowanceを共有poolのChangeBuffer分類へ予約する。shared refusalは全transactionを捨ててfaultを保持し、個別rowをskipしない。cancel／callback error／ACK lossではpendingとleaseを保持し、data破棄後にcreditを返す。RAM dropやlease返却はdurable rollbackの証明ではない。

有限native postmeta試験ではBudgetedPostMetaViewのInternalRows／Indexとraw bufferを同じpoolへ接続し、3 COMMIT callback時の3分類active、zero-pool Rowのexact Exhausted／fault／checkpoint保持、ACK retry・reopen／resume、全owner drop後claims0を確認した。新metadata3項目も厳密な型・値・result相関を検査する。入力／decoded changes／fixture検証用frames copy／driver／allocator capacity／peak・RSS等はowner課金外で、全CSR・自動配分や本番CDCの完成ではない。target/shared-buffer-and-pretty-json-full-check.logで3所有DB各39項目を含む全gateが成功し、回収済み。
