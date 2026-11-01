# RecastPress

RecastPress — WordPress, Recast in Rust.

Rustで書かれたWordPress専用Webサーバー。キャッシュとDBハンドリングを独自機能として実装し、WPの要求処理、テーマ、プラグイン等はサーバー内部のPHP実行環境で扱う。

## ブランチの役割

| ブランチ | 管理する内容 |
| --- | --- |
| `main` | 新旧構成に共通する確定済みのプロジェクト情報、ライセンス、採用ロゴ、除外・改行変換設定 |
| `design/v0.5` | 既存設計v0.45と、新構成の概要・基本・詳細設計v0.5、開発・検証の記録 |
| `dev/v0.5` | 設計ブランチを土台に保存した既存実装と、0.5.0から始める新構成の開発 |

mainには試作コードや未決の設計詳細を入れず、共通部分への変更だけを反映する。設計の変更は設計ブランチへコミットし、実装側へ必要な設計更新を取り込む。実装の変更は実装ブランチへコミットする。

新設計の入口は、設計・実装ブランチの `docs/proposals/2026-11-01-rust-wp-server/README.ja.md`。既存設計の入口は同じブランチの `docs/design.ja.md`。新構成は設計v0.5・実装0.5.0を開始版とし、個別機能の実装・検証状態は各ブランチの記録で確認する。

ブランチ分割前のREADMEは、設計・実装ブランチの `docs/history/README.before-branch-split.txt` に保持する。実装由来の出典・検証履歴は、実装ブランチの `NOTICE.md` と対応する記録を参照する。

## ライセンスとロゴ

本体のライセンス方針はGPL-2.0-or-later。[LICENSE](LICENSE)と[NOTICE.md](NOTICE.md)を参照。以前のGPLv3本文は `LICENSES/GPL-3.0.txt` に保持する。

採用済みのロゴと形式・色の定義は、[ロゴデータ](assets/branding/recastpress/README.ja.md)を参照。
