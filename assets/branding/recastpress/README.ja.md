# RecastPress ロゴデータ

採用案：ブリックレッドの中央案（03-D1、暗め）。採用日：2026-10-26。

- 地色・外周色：**#79332B** / RGB **121, 51, 43**
- 文字・円環・タレ：**#FFFFFF**
- 形：縦足を上げ、斜めの足を下げて、両方を中間の高さに揃えた第16案。右足と円枠の隙間は維持しています。

## SVG

- **recastpress.svg**：標準版。円の外側が透過。文字と円環の白は不透明です。
- **recastpress-white-bg.svg**：白背景付き。

文字は輪郭データになっており、フォントのインストールは不要です。

## PNG

- **png/transparent/**：円の外側が透過。
- **png/white/**：白い背景付き。
- 各フォルダーに **16 / 20 / 24 / 32 / 40 / 48 / 64 / 96 / 128 / 180 / 192 / 256 / 384 / 512 / 1024 / 2048 px** の正方形PNGを収録。

SVGとPNGは同じ1000×1000の構図を使用し、円の外側に5%ずつ余白があります。ファイルサイズは円だけの直径ではなく、余白を含めた画像全体の寸法です。

## Web用

- **web/favicon.svg**：SVG favicon。
- **web/favicon.ico**：16 / 24 / 32 / 48 / 64 / 256pxを内包。
- **web/apple-touch-icon.png**：180px、白背景。
- **web/icon-192.png**、**web/icon-512.png**：透過PNG。
- **web/site.webmanifest**：アイコンと採用色の定義。

これらは配置用のデータです。アプリやサイトへの組み込みは行っていません。

## 確認用・色定義

- **preview/size-preview.html**、**preview/size-preview.png**：サイズごとの見え方。
- **brand.json**：採用案と色・サイズの定義。
- **manifest.json**：収録アセットの寸法、背景、SHA-256。

採用色の元案：docs/branding/warm-dark-study-v3/03-D1-brick-red.svg
形の改訂元：docs/branding/recastpress-logo-concept-v16.svg
