# RecastPress
RecastPress — WordPress, Recast in Rust.

## 現在の状態

2026-10-26の継続開発指示によりRustでの実装を開始。詳細設計v0.44、確定要求R01〜R61を基準とし、未決事項はメモして迂回する。実装範囲・検証結果・残件は[開発メモ](docs/development.ja.md)を参照。[引継ぎガイド](docs/handoff.ja.md)に現在地、[前回の文書整合確認](docs/design-closeout-2026-10-25.ja.md)に再開前の整理を保存。D07-01で編集用DBのWP互換構造維持を採用（R58）。現在は[D08-01：プラグインの直接SQL](docs/plugin-database-access.ja.md)を提案中・未採用。

CDNを必須としない規模の、大企業ではない企業サイト、商品紹介・カタログ・ブランディング、通常規模のLP、個人・趣味等のサイトを主対象とする。単一サイトあたり月間20万PVまでを想定。製品側の突発的な集中アクセス対策はオンメモリキャッシュまでとし、必要な追加対応はインフラ構成で扱う。通常時の性能・費用等の数値目標は今後具体化する。

基本的に個別サポートのないOSSとして提供する。利用者または委託先が具体的な手順・AI向けプロンプトに沿ってテーマ・functions.phpを移植し、テスト結果の確認と公開判断を担う。移行診断は製品方針と作業範囲を説明し、元WPの画面・HTTP応答の記録、移植用・テスト用プロンプト、差分報告を提供するが、サイト移行の成功は保証しない。問い合わせフォームはCMS本体から切り離し、WordPress内への統合を非推奨とする。[外部フォーム](docs/external-forms.ja.md)はTallyを第一推奨、独自デザイン向けにWeb3Formsを代替候補として案内する。

初回の公式配備先はLinux Docker Compose、Railway、AWS Lightsailの三環境。Dockerイメージを配布基準とし、LightsailはLinuxインスタンス上のComposeで動かす。systemdでの直接実行は基本構造を保つが公式対象外。RecastPress本体だけを自動更新し、OS・Docker・DB・PHP等は操作説明・セットアップ支援に限定し、更新実行コードは持たない。セキュリティ更新が自動配信される版をサポートし、配信終了版を強く警告する。

## 開発

Rustワークスペースの初期実装。現時点でCMSとしての本番利用・WordPress互換性は未検証。

```powershell
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
```

[公開・管理プロセスの起動手順](docs/running.ja.md)。初回は依存を取得し、取得後は`--offline`を付けて検証できる。

## 設計文書

2026-10-26に[コア内部のDB自動最適化と変更検知](docs/database-auto-optimization.ja.md)を承認（D07-02）。詳細設計v0.44、要求R59～R61、受入試験計画T44を追加。編集用リーダー・内部索引・有限binlog処理を試作し、所有する使い捨てDBで検証中。本番の連続追従・自動容量管理・公開連携と性能検証は未完了。最新の範囲は[DB開発メモ](docs/database-development.ja.md)を参照。

| 文書 | 用途 |
| --- | --- |
| [実装の進捗と確認待ち](docs/development.ja.md) | 継続開発の現在地、確認事項と迂回、検証結果 |
| [開発版の起動](docs/running.ja.md) | 公開・管理プロセス、開発用ページ束、ヘルスチェック |
| [互換実装の開発](docs/compatibility-development.ja.md) | 固定WPソース、対応台帳、PHPとの比較証跡 |
| [編集用DBの開発](docs/database-development.ja.md) | 参照DDL、raw reader、一貫読取り、専用MariaDB/MySQL試験と残件 |
| [引継ぎ・再開ガイド](docs/handoff.ja.md) | 最初に読む。最新決定、残件、次回の推奨議題、作業場所 |
| [終了時の文書整合確認](docs/design-closeout-2026-10-25.ja.md) | 整理内容、参照・IDの確認、未実施範囲と再開方法 |
| [設計判断・確認事項](docs/decisions.ja.md) | 決定履歴と未決事項 |
| [詳細設計書](docs/design.ja.md) | 要件、アーキテクチャ、互換性、移行、配備、検証計画 |
| [DBスキーマ方針](docs/database-schema.ja.md) | 編集用WP互換構造と独自状態・公開用データの分離（採用済み） |
| [プラグインの直接SQL](docs/plugin-database-access.ja.md) | 読取り・更新の許可範囲の比較案（未採用） |
| [想定ユーザー・対象サイト](docs/target-sites.ja.md) | CDNを必須としない対象像、代表サイト・利用シナリオ、性能・運用合格条件の具体化 |
| [利用者のAIによるテーマ移植](docs/theme-porting.ja.md) | 決定した役割分担、Rust SDK・成果物検証の詳細案 |
| [Wasm実行・成果物契約](docs/theme-module-contract.ja.md) | 採用した実行方式と、SDK・権限・状態・配備の詳細案 |
| [WordPress互換基準](docs/wp-compatibility.ja.md) | 7.1.3の参照配布物、非推奨API、対応一覧・版管理の方針 |
| [WP標準仕様への統一方針](docs/wp-standard-policy.ja.md) | コア標準を基本仕様とし、維持する既決差分・今後協議する事項を整理 |
| [認証・SSO設計](docs/authentication.ja.md) | 全ユーザー向け標準SSO・初期off・OIDC限定、MFA非実装、アカウント連携の設計 |
| [本体自動更新と環境の設定支援](docs/automatic-updates.ja.md) | 本体の自動更新、外部環境の操作説明・セットアップ支援、対応版と配信終了警告 |
| [キャッシュ・独自設定画面](docs/cache-and-settings.ja.md) | 容量自動設定・GETクエリー・TTLなし・変更検知時の全パージ、独自機能の追加1画面 |
| [基本設計レビュー](docs/basic-design-review-2026-10-25.ja.md) | 2026-10-25 16:47 JST時点の評価と優先課題 |
| [レビューを踏まえた判断順](docs/review-decisions.ja.md) | レビュー指摘と未決事項の対応、一件ずつの協議の現在地 |
| [WP側移行プラグイン](docs/wp-migration-plugin.ja.md) | PHP製の対話型移行経路と、仲介サービスの検討課題 |
| [コメント方針](docs/comments-policy.ja.md) | WP準拠の受付・承認設定、初期受付無効、移行設定の協議 |
| [出典](docs/sources.ja.md) | 元の議論、全文資料、公式情報 |

WPコア標準を基本仕様とし、互換基準は7.1.3。利用者が自分のAIでテーマ・functions.phpをRustへ移植し、サーバー側Wasmで実行する。公開要求からPHPを起動しない。

SSOは全ユーザー向け標準・OIDCのみ・初期off、MFAは実装しない。必須時の既存ユーザーの一度のパスワード認証は連携専用で、有効な手続きセッション内なら再開可能。

キャッシュは初期on・容量完全自動・手動上限なし・TTLなし・GETクエリー対応。変更検知で全パージし、監視不成立時は停止する。独自機能の初期設定は移行アシスタントで質問し、移行後は追加1画面にまとめる。

スキーマ変更なしの本体更新は、Rust本体全体を一時的に二重起動して切り替える。無停止は保証しない。スキーマ変更ありの方式は未決で、検証計画は維持する。

[未決一覧](docs/decisions.ja.md)は20分類（保留1、調査・実装詳細を含む）。DBスキーマ方針はR58で採用済み。現在はプラグインの直接SQL方針を協議中。既決要件の基盤を実装中。未決事項の採用は保留する。
