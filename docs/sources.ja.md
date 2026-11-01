# RecastPress 設計の出典

2026-10-30の実装照合追記：[実装と上位設計の照合](implementation-design-reconciliation-2026-10-30.ja.md)に現在の実装・検証と重要順の残件を整理した。要求R01〜R61／試験計画T01〜T44の採用内容は変更していない。以下の日付付き記録の「未実施」「次の議題」は各決定時点の状態であり、現在地は詳細設計v0.45と引継ぎを優先する。製品コード開発は中断のままで、今回追加したのは文書整合化だけである。

確認日：2026-10-25（日本時間）。会話の提案とユーザー決定を区別する。

## 0. 設計再開後の追加決定

2026-10-25：DBスキーマ方針D07-01のA／B比較に対し、ユーザーが「A」と回答。編集用正本のWP互換構造維持と独自状態・公開用データの分離をR58に採用し、詳細設計v0.43・T43へ反映。[DB方針](database-schema.ja.md)を正本とする。直接SQLの許可は含めず、[D08-01の比較案](plugin-database-access.ja.md)で別途協議する。実装・DB操作・試験は未実施。

## 1. 会話とローカル資料

| 資料 | 確認内容・制約 |
| --- | --- |
| [WPのRust移植](https://chatgpt.com/c/6add9539-7378-83e8-a916-baa5d605812e) | 元チャット取得では回答が参照記号になっていた。Work側チャットと添付全文を併用 |
| [名前候補を考える](https://chatgpt.com/c/6add9939-7d4c-83ee-898e-cd58878bacd7) | ユーザーのRecastPress採用・英語説明要求を確認。過去の英語説明文そのものは未取得 |
| [移行先プラットフォーム設計](https://chatgpt.com/c/6add98f3-96f0-83ee-a1dd-fbe7591a72da) | 元チャット取得では設計の回答本文が取れず、引継ぎ先の添付全文を併用 |
| WPのRust移植（Work） | 引継ぎ資料と、その後のGPLv3以降の推奨回答を確認 |
| 「移行先プラットフォーム設計」をインポート | 添付全文の場所と、標準構成などが提案段階であることを確認 |
| 本チャット | 複数配備先、App Runner除外、GPLv2-onlyへの対応余地、標準テーマ用API全体を決定。当初の自動変換・ローカル完結は、最新指示で「利用者自身の契約AIによるRust移植、製品はAPI・検証・配備を提供」へ更新。詳細はD04・第2章の再開後記録を参照 |
| [基本設計レビュー](basic-design-review-2026-10-25.ja.md) | 2026-10-25 16:47 JSTの設計評価。ユーザーの指示により以後の協議で参照。最優先課題と判断順を[対応表](review-decisions.ja.md)へ整理。原文は評価時点の記録として保持 |

第8件でWP準拠のコメント設定と新規サイト初期受付無効、第9件でユーザーがAを選択して元WPの受付・承認設定引継ぎを決定し、R27・R28へ反映。全件承認固定や全既存投稿の一括無効化は採用していない。第10件もAを選択し、D10-07・R29へ管理者・記事著者向けメール通知のRust初回対応を記録。第11件は「WP本体の仕様に準拠。」とのユーザー指定をD10-08・R30へ反映し、独自の追加制限を含む以前のA選択とは扱わない。第12件では「WP本体の標準仕様で一貫して」と指定。D10-09・R32へコメントフィード初回対応、D10-10・R31へ今後の標準仕様の統一を記録し、標準機能の採否の再質問をやめる。既決の差分・実行制約を維持。第13件はAを選択しD05・R33へ公開系・管理系Rustの別プロセスを記録。第14件はAを選択し、D06-01・R34へ管理系をHTTPS＋認証・認可でインターネットから利用可能、VPN任意と記録。第15件はユーザーが全ユーザー向けSSO標準提供・初期offを指定し、D06-03・R35へ採用。D06-02のMFA必須／任意への回答とはしない。第16件で「MFAは実装しない。SSOはOIDCのみ。」と指定。D06-02・R36でMFA非実装、D06-04・R37でOIDC限定を確定し、SAMLの後続対応予定も採用しない。第17件でユーザーがオフ／ユーザー任意／必須要求の3段階を指定し、さらに必須時の既存ユーザーのパスワードログインを一度だけ許可すると追加指定。D06-05・06、R38・R39に採用。第18件でユーザーがAを採用し、D06-07・R41に一度のログインをOIDC連携手続きだけへ限定すると記録。第19件D06-08もAを採用し、R44に有効な手続き用セッションの期限内の再開を記録。第20件D19-01はユーザーがAを採用し、監視不成立時の全パージ・キャッシュ停止・通常配信をR49へ記録。続く「デフォルトでオン」と移行アシスタント内で追加機能の設定を聞く指定をD19-02・R50とD16-02・R51へ採用。SSO等の既定値は維持。D19-03はユーザーの「完全自動」でBを採用し、手動容量設定を設けないR52へ記録。D27・D27-01で更新方針を確定。本体は自動更新し、その他は操作説明・セットアップ支援のみで更新実行コードを持たない。セキュリティ更新が自動配信される版をサポートし、配信終了版には強く警告する（R40・R56）。本体の更新設定等は今後具体化する。スキーマ変更なしの本体二系統切替は後続D27-02・R57で決定済み。残る更新設定とスキーマ変更ありの方式は未決。具体的な容量・監視・質問項目等は未確定。[コメント方針](comments-policy.ja.md)と[統一方針](wp-standard-policy.ja.md)を参照。

当初の追加指示「デプロイ後の各種アップデート（OS・ミドルウェア・RecastPress）を自動適用する支援機能」はD18へ記録したが、後続のユーザー指定D27で自動更新対象はRecastPress本体だけに変更した。OS・Docker・DB・PHP等は運用者向け手順の案内に限る。[現行の範囲](automatic-updates.ja.md)を参照。実装開始・更新実行は指示されていない。

更新方針の追加指定（本スレッド、2026-10-25）：RecastPress以外は操作説明・セットアップ支援に限定し、更新を実行するコードは持たない。本体は一般的な自動更新機能を持つ。セキュリティ更新が自動配信される版をサポート対象とし、配信終了版を強く警告する。D27-01・R40・R56へ反映。特定の対応版・配信終了日の調査を完了した意味ではない。

追加指示により、アクセスされたページ・コンテンツのオンメモリキャッシュ、環境に応じた容量自動設定、GETクエリー対応、DB・ファイル書換え検知時の全パージをD19・R42へ採用。「永続的」の確認にはユーザーがTTLなし、パージ・容量超過・再起動で消失可と回答。再起動復元のためのディスク永続保存は要件化しない。独自機能をWP管理画面の追加1画面から設定可能にする指定をD20・R43へ採用。[設計案](cache-and-settings.ja.md)を参照。続くD06-08へのA回答はR44として上記の認証決定へ記録した。

追加指定の想定ユーザー・用途をD21・R45へ採用。CDNを使うまでもないアクセス規模の、大企業ではないカンパニーサイト、商品紹介・カタログ・ブランディング、大規模キャンペーンを打たないLP、アマチュアサイトを主対象とする。後続のユーザー指定で単一サイト月間20万PVまでを想定し、製品側の突発的な集中アクセス対策はオンメモリキャッシュまで、必要な追加対応はインフラ構成で扱う。[対象サイト案](target-sites.ja.md)を参照。「WordPressユーザーの大半が該当すると考えている」はユーザーの市場仮説として区別し、外部調査で検証済みの割合にしない。通常時の性能・費用等の残る数値は未確定。

企画レビューへのユーザーの修正：具体的なAI向けプロンプトと手順を製品が提示し、利用者側の運用者または委託先が移行を行う想定。問い合わせフォームはCMS本体から完全に切り離し、WP内統合を非推奨として移行診断で伝える。基本的に個別サポートのないOSSとして公開し、制作者が全利用者を受託する前提を置かない。AIコーディングによる実装量の多さ自体は問題視せず、必要性や高い実装・検証・保守負荷による対応先の絞り込みを検討する。月間PV以外のD14定量条件は課題として保持。D22〜D24・R46〜R48へ記録した。

後続のユーザー指定でCloud Run、Render、Linux systemdを初回公式対象から外し、Linux Docker Compose、Railway、AWS LightsailをDockerイメージ基準の三環境とした。LightsailはLinuxインスタンス上のCompose。systemdは基本構造だけ維持する。自動更新はRecastPress本体だけとし、環境側更新は手順案内に限る。移行診断では製品方針・作業範囲を説明し、元WPの画面・HTTP応答を初回診断時と最終同期後に記録する。移植用・テスト用プロンプトと比較支援を提供するが、移植・結果確認・公開判断は利用者または委託先の責任で、移行成功は保証しない。コメントと通知は標準機能の移行用で通常利用を推奨せず、外部メールサーバー接続・配信成功を保証しない。D25〜D29・R11〜R12・R40・R53〜R55を参照。

読取り元のローカル資料：

- `C:/Users/nodew/.codex/attachments/8274b78d-37a9-4788-8efd-c725abe3abf8/貼り付けたテキスト.txt`：WPのRust移植の元会話全文。
- `C:/Users/nodew/Documents/Codex/2026-10-25/referenced-chatgpt-conversation-this-is-an/outputs/wordpress-rust-handoff.ja.md`：旧引継ぎ資料。
- `C:/Users/nodew/.codex/attachments/82189702-b690-46ef-ba6e-e4447e98af3b/貼り付けたテキスト.txt`：移行先プラットフォーム設計の元会話全文。

これらの資料は参照のみで変更していない。設計書は参照記号の回答内容を推測して復元したものではない。過去資料の製品名・初回スコープなどは今回の決定を優先した。

D27-02の追加指定（2026-10-25）：ユーザーが「スキーマ変更がない場合はRUST全体を一時的に二重起動して切り替える」と指定し、R57へ採用。直後の「実証しない＞保証しない」という訂正を優先し、無停止を保証しないと記録する。検証計画の廃止ではなく、実装・実証は未実施。スキーマ変更を伴う更新の方式は未決。

本スレッド終了時の文書整合確認（2026-10-25）：[確認記録](design-closeout-2026-10-25.ja.md)へ修正内容・未実施範囲を保存。未決一覧は、別章に存在したD04残件の補完により19分類から20分類（保留1）へ整理した。ユーザー判断8〜10論点という見込みは整理上の目安であり、質問数・実装完了率の保証ではない。次の選択肢は未提示。

## 2. 公式資料の確認

2026-10-25の再開後、D07は推奨案の選択によりMariaDB・MySQL両対応で決定。続いてユーザーが手持ちAIによるテーマ・functions.phpのRust移植構成を提案し、「ユーザーが自分で（自分が契約したAIを使って）コード変換することを前提に設計する」と明示した。[移植設計](theme-porting.ja.md)へ役割分担の決定を記録。製品自身のAI接続は行わず、利用者のAI操作を製品外の作業とする。R10・R14を更新し、R17を追加した。SDK・実行形態等の詳細は提案段階。

その後、実行方式の三案比較に対しユーザーが「RustからWasmへビルドして実行（推奨）」を選択し、R18へ反映。SDK・成果物・能力・状態・配備の詳細は[Wasm実行契約](theme-module-contract.ja.md)へ記録した。Wasm方式は決定、エンジン・Component Modelの採用版と個別APIは未決。

2026-10-25時点の[公式リリース一覧](https://wordpress.org/download/releases/)ではWordPress 7.1.3（2026-10-06公開）が最新。7.1.3へ固定し、同版に残る非推奨の標準テーマ用APIも対象にする質問へユーザーが「はい」と回答し、R19へ反映した。公式配布アーカイブURL・公開SHA-1を[互換基準記録](wp-compatibility.ja.md)へ記録。アーカイブ本体の取得・再計算照合は未実施。

| 対象 | 出典 | 設計で使う事実 |
| --- | --- | --- |
| WordPress | [License](https://wordpress.org/about/license/) | 本体GPLv2以降 |
| GPL版の組合せ | [GNU FAQ](https://www.gnu.org/licenses/gpl-faq.en.html#v2v3Compatibility) | v2-onlyとv3、v2-or-laterを区別 |
| Apache-2.0 | [Apache公式](https://www.apache.org/licenses/GPL-compatibility.html) | GPLv3との互換性、GPLv2との非互換性 |
| テーマ | [Template Hierarchy](https://developer.wordpress.org/themes/classic-themes/basics/template-hierarchy/) | ページ種別に応じたテンプレート階層 |
| functions.php | [Custom Functionality](https://developer.wordpress.org/themes/core-concepts/custom-functionality/) | 任意PHPコード、管理・公開双方での読込み、親子テーマの順序 |
| Rust成果物 | [Linkage](https://doc.rust-lang.org/reference/linkage.html) | ソース組込み・ライブラリ等の出力形式。採用方式は未決 |
| Wasm候補 | [Component Model](https://component-model.bytecodealliance.org/design/why-component-model.html) | 型付きimport/exportによる呼出し契約。性能・GPLv2互換性の監査は未実施 |
| RustからWasm | [Rust component build](https://component-model.bytecodealliance.org/language-support/building-a-simple-component/rust.html) | Rustによるコンポーネント作成。具体的ターゲット・ツール版は未選定 |
| Wasmホスト境界 | [Wasmtime Security](https://docs.wasmtime.dev/security.html) | import/exportを通す外部アクセス。ホストが渡す能力の検査が必要 |
| Wasm二世代の実行モデル | [Component Instance](https://docs.wasmtime.dev/api/wasmtime/component/struct.Instance.html)／[Store](https://docs.wasmtime.dev/api/wasmtime/struct.Store.html) | インスタンスとStoreの所有・寿命を二世代切替案の参照にする。D27-02・R57でスキーマ変更なしのRust本体全体の一時二重起動・切替を採用。無停止は保証しない。API参照はエンジン・開発版の採用や無停止の実証を意味しない |
| Wasm資源制限 | [実行中断](https://docs.wasmtime.dev/examples-interrupting-wasm.html)／[StoreLimitsBuilder](https://docs.rs/wasmtime/latest/wasmtime/struct.StoreLimitsBuilder.html) | 計算量・中断・メモリ等の制限機構。全資源を自動制限する保証ではない |
| エンジンの許諾候補 | [Wasmtime LICENSE](https://github.com/bytecodealliance/wasmtime/blob/main/LICENSE) | Apache-2.0とLLVM例外。依存・featuresを含む配布構成の監査は未実施 |
| ビルド時コード実行 | [Cargo build scripts](https://doc.rust-lang.org/cargo/reference/build-scripts.html)／[Procedural macros](https://doc.rust-lang.org/reference/procedural-macros.html) | ビルドスクリプト・手続きマクロも資源へアクセスし、Wasm実行時の隔離とは別に扱う |
| Rust ABI | [External blocks](https://doc.rust-lang.org/reference/items/external-blocks.html) | Rust ABIは安定性を保証しない。モジュール境界の契約と分ける |
| 非推奨API | [_deprecated_function](https://developer.wordpress.org/reference/functions/_deprecated_function/) | 非推奨呼出しと診断。7.1.3に残る標準テーマ用APIを対象とし、SDKの意味対応・比較ケースを具体化 |
| 基準配布物 | [7.1.3 tar.gz SHA-1](https://wordpress.org/wordpress-7.1.3.tar.gz.sha1)／[zip SHA-1](https://wordpress.org/wordpress-7.1.3.zip.sha1) | 公式公開チェックサムの値。アーカイブ本体の照合は未実施 |
| マルチサイト | [Multisite](https://developer.wordpress.org/advanced-administration/multisite/)／[switch_to_blog](https://developer.wordpress.org/reference/functions/switch_to_blog/) | 複数サイト・共有ユーザー・サイト切替とコード読込みの意味。R20で初回は単一サイト先行、ネットワーク固有機能は後続対応 |
| 検索フォーム | [get_search_form](https://developer.wordpress.org/reference/functions/get_search_form/) | 標準テーマの検索フォームとフック。D10-01で検索初回対応を決定。7.1.3の詳細挙動は固定ソースで別途確認 |
| 記事更新フィード | [WordPress Feeds](https://developer.wordpress.org/advanced-administration/wordpress/feeds/)／[get_bloginfo](https://developer.wordpress.org/reference/functions/get_bloginfo/) | フィード配信と標準テーマAPIのURL参照。D10-02でRSS・Atom初回対応を決定。基準7.1.3の細部は固定ソースで確認 |
| コメント表示・フォーム | [comments_template](https://developer.wordpress.org/reference/functions/comments_template/)／[comment_form](https://developer.wordpress.org/reference/functions/comment_form/) | 表示と投稿フォームの別API。D10-03で既存移行・表示、D10-04で新規Rust受付を初回対応とした。基準7.1.3の挙動は固定ソースで確認 |
| コメント送信受付 | [wp_handle_comment_submission](https://developer.wordpress.org/reference/functions/wp_handle_comment_submission/) | 入力・投稿・返信等の受付処理。Rust側の互換契約と、PHPを呼ばない限定書込み経路を別途設計 |
| コメントの承認状態・検査 | [wp_allow_comment](https://developer.wordpress.org/reference/functions/wp_allow_comment/)／[wp_check_comment_data](https://developer.wordpress.org/reference/functions/wp_check_comment_data/)／[check_comment](https://developer.wordpress.org/reference/functions/check_comment/) | 受付検査と承認判定。D10-05でWP準拠の設定を採用し、個別契約は固定ソースと照合して具体化 |
| コメントの受付・初期値 | [comments_open](https://developer.wordpress.org/reference/functions/comments_open/)／[populate_options](https://developer.wordpress.org/reference/functions/populate_options/) | 投稿ごとの受付判定と新規初期値の区別。公開参照コードではdefault_comment_statusはopen。RecastPressのoffはユーザー指定の差分として記録。基準7.1.3アーカイブ照合は未実施 |
| コメント通知 | [wp_notify_moderator](https://developer.wordpress.org/reference/functions/wp_notify_moderator/)／[wp_notify_postauthor](https://developer.wordpress.org/reference/functions/wp_notify_postauthor/) | 承認待ちの管理者等への通知、記事著者等への通知と設定・権限条件。D10-07でRust初回対応を決定、送信基盤等は未決 |
| コメントの基本検査 | [wp_allow_comment](https://developer.wordpress.org/reference/functions/wp_allow_comment/)／[check_comment](https://developer.wordpress.org/reference/functions/check_comment/)／[wp_check_comment_disallowed_list](https://developer.wordpress.org/reference/functions/wp_check_comment_disallowed_list/)／[wp_check_comment_flood](https://developer.wordpress.org/reference/functions/wp_check_comment_flood/) | D10-08でWP本体の設定・判定への準拠を決定。重複・連投・語句・リンク数・例外等の参照。固定7.1.3ソースとの照合は未実施 |
| コメント更新フィード | [get_post_comments_feed_link](https://developer.wordpress.org/reference/functions/get_post_comments_feed_link/)／[do_feed_rss2](https://developer.wordpress.org/reference/functions/do_feed_rss2/)／[do_feed_atom](https://developer.wordpress.org/reference/functions/do_feed_atom/) | コメント購読リンクとRSS 2.0・Atom配信入口の参照。D10-09で初回対応を決定、固定ソースとの比較試験は未実施 |
| WP管理操作の権限 | [Checking User Capabilities](https://developer.wordpress.org/plugins/security/checking-user-capabilities/)／[Nonces](https://developer.wordpress.org/apis/security/nonces/) | 管理能力の検査とCSRF対策。nonceを認証・認可・再送防止の代替にしない |
| 移行プラグインの通信 | [Custom Endpoints](https://developer.wordpress.org/rest-api/extending-the-rest-api/adding-custom-endpoints/)／[wp_safe_remote_post](https://developer.wordpress.org/reference/functions/wp_safe_remote_post/) | 認可付きAPIと転送先URL検査の基礎。転送方式は両対応で決定、具体的プロトコルは未決 |
| 仲介サービスでのデータ取扱い | [Plugin Privacy](https://developer.wordpress.org/plugins/privacy/) | データ最小化、外部共有、保持・削除等の設計観点。サービス採用・具体的仕様・適用法の判断は行っていない |
| ブロック | [Dynamic blocks](https://developer.wordpress.org/block-editor/how-to-guides/block-tutorial/creating-dynamic-blocks/) | 動的ブロックは表示時のデータに応じた処理を持つ |
| テンプレートタグ | [解説](https://developer.wordpress.org/themes/classic-themes/basics/template-tags/)／[一覧](https://developer.wordpress.org/themes/classic-themes/references/list-of-template-tags/) | WPタグの役割と対象母集合調査の入口 |
| テーマ実行状態 | [The Loop](https://developer.wordpress.org/themes/classic-themes/basics/the-loop/)／[Conditional Tags](https://developer.wordpress.org/themes/classic-themes/basics/conditional-tags/) | ループ・クエリ状態に依存する動作 |
| テーマのフック依存 | [the_content](https://developer.wordpress.org/reference/functions/the_content/)／[wp_head](https://developer.wordpress.org/reference/functions/wp_head/) | 本文フィルターとアクションの呼出し |
| フック | [save_post](https://developer.wordpress.org/reference/hooks/save_post/) | 保存後のフック、再帰への注意 |
| 移行 | [WP-CLI search-replace](https://developer.wordpress.org/cli/commands/search-replace/) | シリアライズを考慮した置換 |
| TLS | [Caddy Automatic HTTPS](https://caddyserver.com/docs/automatic-https) | 証明書取得・更新を委任できる |
| HTTPキャッシュ | [RFC 9111](https://www.rfc-editor.org/rfc/rfc9111.html) | URI・クエリーと表現、認証応答・保存禁止・失効等の参照。内部TTLなしを外部HTTPキャッシュの無期限保存へ置換しない |
| メモリ制限・空き容量 | [Linux cgroup v2](https://docs.kernel.org/admin-guide/cgroup-v2.html)／[proc_meminfo(5)](https://man7.org/linux/man-pages/man5/proc_meminfo.5.html) | 階層的な制限・使用量、MemAvailableの推定値を容量案の参照にする。空き容量は専有予約ではなく、具体的な算式や対象配備構成での取得は未検証。D19-03で完全自動を選択したことと、具体的な算式の検証は区別 |
| 外部DB変更の検知 | [MySQL Binary Log](https://dev.mysql.com/doc/refman/8.4/en/binary-log.html) | 変更イベントの検知候補。MySQLの版採用、MariaDBとの同一性、全環境での権限・完全検知は未検証 |
| 変更監視の補足 | [MariaDB Binary Log](https://mariadb.com/docs/server/server-management/server-monitoring-logs/binary-log)／[inotify](https://man7.org/linux/man-pages/man7/inotify.7.html) | DBログの設定と、ファイル監視のイベント欠落・再照合等を参照。D19-01でユーザーが停止案を採用。各環境での実証は未実施 |
| OS自動更新の参考 | [Ubuntu Automatic updates](https://ubuntu.com/server/docs/how-to/software/automatic-updates/) | OS更新・再起動・サービス再起動と構成上の注意。Ubuntu自体の採用確定やRecastPressの更新設定の決定ではない |
| ベースイメージ自動更新の参考 | [Cloud Run Automatic base image updates](https://docs.cloud.google.com/run/docs/configuring/services/automatic-base-image-updates) | 対応ベースイメージ・アプリイメージ構成等の条件。任意のRustイメージへの適用を検証済みとはしない |
| 管理系の認証・認可 | [OWASP Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html)／[Authorization](https://cheatsheetseries.owasp.org/cheatsheets/Authorization_Cheat_Sheet.html) | TLS、最小権限・要求ごとの認可等の設計参照。D05の分離を具体化し、D06-01のアクセス方式と認証の詳細を区別。準拠・試験完了を示すものではない |
| 管理者等のMFA | [OWASP MFA](https://cheatsheetseries.owasp.org/cheatsheets/Multifactor_Authentication_Cheat_Sheet.html) | 高権限ユーザーへのMFA、安全な回復、複数入口の保護を推奨。旧提案の参照として保持。D06-02・R36でユーザーはMFA非実装を指定し、この推奨は採用しない |
| SSO方式・アカウント連携 | [OpenID Foundation](https://openid.net/developers/how-connect-works/)／[OWASP Authentication](https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html) | OIDCの認証／SSOとしての役割、SAML、外部IDの検証と既存アカウント連携の参照。全ユーザー向け標準SSO・初期offはユーザー指定、方式はD06-04・R37でOIDCのみに決定。SAMLは採用せず、OIDCの実装・検証は未実施 |
| Cloud Run | [Container contract](https://docs.cloud.google.com/run/docs/container-contract)／[Jobs](https://docs.cloud.google.com/run/docs/create-jobs) | コンテナ寿命・ローカル保存、Jobsという実行形態 |
| Cloud SQL | [Overview](https://docs.cloud.google.com/sql/docs/introduction) | 提供DBエンジン |
| Render | [Disks](https://render.com/docs/disks)／[Private services](https://render.com/docs/private-services)／[Service types](https://render.com/docs/service-types) | 永続ディスクと内部処理サービス |
| Railway | [Volumes](https://docs.railway.com/volumes/reference) | volumeとreplicaの制約 |
| Lightsail | [Databases](https://docs.aws.amazon.com/lightsail/latest/userguide/amazon-lightsail-databases.html) | マネージドMySQL／PostgreSQL |
| App Runner | [Availability change](https://docs.aws.amazon.com/apprunner/latest/dg/apprunner-availability-change.html) | 新規顧客受付終了。既存顧客は継続利用可能 |

資料からの事実と、RecastPressでどの構成を採るかの提案を分ける。料金、採用バージョン、性能、全テーマ互換、全プラグイン互換は本調査で検証していない。
