# 固定archiveの内部メタデータ確認

2026-11-02。[固定依存treeの調査](../php-wasix-cgi-build/dependency-source-findings.ja.md)の後続として、未観測/曖昧だった3archiveを固定commitから取得し、CIのSHA-256・byte数とGit blob SHA-1の両方へ照合した。バイナリは`.reference`内だけに保存し、実行・展開・Git追加・再配布はしない。

```text
python experiments/php-static-archive-probe/inspect.py --record
```

`inspect.py`自身はネットワーク取得を行わない。固定場所の3ファイルを照合してarのGNU/BSD member表を読み、basename化したmember一覧のhash、magicによる形式、Rust crate名・registry path内の版文字列候補だけを[JSON](evidence/archive-metadata.json)へ記録する。元の絶対pathは出力しない。長名表のentry先頭、重複、memberの長さ・総数・終端を検査する。

| archive | 通常member | magicの観測 | registry内版文字列候補 |
| --- | ---: | --- | ---: |
| libicrt.a | 33 | LLVM bitcode | 0 |
| libwasix_rust_libs.a | 641 | Wasm | 44 |
| libwasix_sendmail.a | 488 | Wasm | 29 |

`libwasix_rust_libs.a`には`wasix_sendmail`や`wasix_httpd`に対応するRust member名も含まれる。別の`libwasix_sendmail.a`とは、`anyhow`（1.0.97/1.0.86）、`rustls-pki-types`（1.11.0/1.7.0）等のregistry path文字列が異なる。archive名を単一crateの同一buildと扱わず、個別の由来確認が必要である。

これは最終Wasmへ残ったコードや実行中の依存versionを確定する結果ではない。member名・path文字列は手掛かりであり、完全なCargo graph、実際のsource commit、patch、build recipe、license/notice対応を証明しない。magicだけでWasm objectとしての構造全体を検証したとも扱わない。元のlink観測の未観測/曖昧を未使用に置き換えず、配布条件・製品採用は未確認のまま維持する。
