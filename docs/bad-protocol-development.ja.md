# wp_kses_bad_protocol の限定バイト実装

完了した全gate：`target/protocol-outer-and-mysql-server-restart-full-check.log`でworkspace389／独立29 Rust・Python133、全6 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPが成功し全fixtureを回収した。`target/protocol-outer-server-restart-final-wp-provenance.log`でWP current hashも確認。以下の途中gateのDB hash拒否は履歴で、最新証跡は実測で再生成済み。

2026-10-27。固定 WordPress 7.1.3 の `wp-includes/kses.php:1984` を正本とする。API60 の本体・契約・比較ハーネスを実装し、正式な3つの実PHP比較を実行した。`target/protocol-outer-reference-workspace-check.log`で60 API / 401,980比較（本API7,768追加）、hooks572、dispatcher405が成功した。workspace Rust389件、Python128件、Clippy・Linux cross-check・依存license照合も通過したが、この実行はDB試験コード変更に伴う旧DB証跡のhash不一致で停止した。全fixtureを実測で再生成する全gateは別途行う。直前の完全チェックポイント59 API / 394,212比較は履歴として維持する。

`wp_kses_bad_protocol_c_bytes(content, allowed_protocols, max_output_bytes)` は Bytes 入力、全値が Bytes の明示配列、C locale の部分集合を扱う。配列の全値を事前検査し、未使用値の型がPHPの短絡で隠れる場合も対応対象に含めない。既定の許可プロトコル方針は選択しない。

最初に内容全体へ `wp_kses_no_null` の既定 Remove を一度だけ適用する。その後、文字どおり小文字の `https://` または `http://` で始まり、配列に厳密一致する小文字値 `https` または `http` がある場合だけ、その結果を即時返す。大文字の入力・配列値はこの経路に入らず、スキームの一般的な解析や部分文字列検索には置き換えない。

それ以外は default count=1 の once working 入口を最大6回呼び、前後のバイト列を厳密比較する。6回目で安定すれば返すが、6回目も変化すれば空を返す。7回目は呼ばない。各回の feed 再帰カウントは1へ戻る。once 内部の修復、最初の区切り、RAW `/?` guard、PHP trim、once2、feed 判定の順序はAPI59と共有する。

raw input の1,048,576バイト上限はこのAPIの最上位入口だけの host bound である。修復で増えた作業データや内部反復へ、公開 once API の入口上限を再適用しない。一方、実際に once2 へ渡す prefix には既存decoderの入力1MiB・両passで認識する値0..255の範囲を適用する。fastpath、未分割本文、RAW guard、未到達の第7回prefixを事前にdecodeしない。したがって `plain&#256;` や `http:&#256;` が保持される経路と、同じ参照が呼出し対象prefixになって拒否される経路を区別する。

最終 cap は完成した出力だけへ適用する。拒否結果の空は cap=0 で返せる。修復・NoNull・再帰・反復の Vec、入力および許可配列は最終capの外にある未課金の作業領域であり、RSS・shared pool・製品容量設定ではない。checked 長さと fallible reserve により部分出力は返さない。

PHP oracle は Bytes／配列全値／C／top bound を検査し、実際に到達するprefixだけのdomain admissionを行う。その後に実ソースの公開 `wp_kses_bad_protocol` を呼び、PCRE error zero と文字列結果を要求する。比較値を admission 側の手製出力で代用しない。profile metadata は型・キー・値を厳密照合し、global NoNull 1回、once最大6回、内部top guard無しを記録する。

検証準備は新Rust契約8件と既存once契約7件、Python comparison/evidence対象19件、Clippy、PHP構文検査、固定253 entity literal照合を通過した。生成した新API60 corpusは7,768件、全体60 API / 401,980件であり、native probeでは新7,768件の対応domainを確認した。これはPHP一致の件数ではない。正式比較は親の実行待ちである。

事前の有限PHP調査167件と範囲外配列型5件は `target/bad-protocol-outer-observation.{php,json}` に分離され、正式corpusへ加算しない。1MiBのraw入力が修復後に上限を超える例、5／6回の境界、未到達prefixの診断差を検証の根拠にした。

非Bytes coercion・診断の全短絡順、他locale、PCRE資源失敗、WordPress全体の互換性、URL文法／認可、HTML sanitizer、出力安全性、公開HTTP／DBへの自動適用は未対応である。
