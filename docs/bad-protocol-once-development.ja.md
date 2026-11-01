# WordPress single protocol passの限定移植

2026-10-27時点。固定WordPress 7.1.3の`wp_kses_bad_protocol_once`を、Bytes content・全要素Bytesのprotocol array・C locale・source既定count=1の部分集合で移植した。

`target/bad-protocol-once-reference-check.log`で正式PHP比較59 API・394,212ケース、hooks572／dispatcher405が成功した。続く`target/protocol-once-and-etag-full-check.log`ではworkspace Rust381件・独立experiment27件・Python113件、HTTP/process smoke等の要求チェック、独立5 DB fixture各39項目とcurrent証跡検査が成功した。fixtureは全回収され、`.reference/db-runs`が空であることも確認した。これらは有限な開発checkpointであり、WordPress API全体の互換性や製品のURL認可完成を示すものではない。

## Sourceと明示profile

一次ソースは`.reference/wordpress-7.1.3/wp-includes/kses.php:2100`。SHA-256は`ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d`で、registryへ位置・identity・native契約名を登録する。

```rust
wp_kses_bad_protocol_once_c_bytes(
    content: &[u8],
    allowed_protocols: &PhpArray,
    max_output_bytes: usize,
) -> Result<Vec<u8>, BadProtocolOnceError>
```

contentは明示byte列、protocolはすべてのvalueがBytesの明示arrayに限り、keyは比較に使わない。nativeは全valueを検証するため、sourceなら未参照だった値も非Bytesなら拒否する。非Bytes値のcastや短絡評価による診断順は対応しない。default protocol listを選択せず、callerが各呼出しへ渡すarrayだけを扱う。

countはsourceの既定1に固定し、nativeの公開引数にしない。formal probeも明示的なcount fieldをすべて拒否する。任意のcount、負値、PHPの比較・increment coercionは対応domainへ含めない。

## 正確な処理順

最初に**全文**を次のregexで補修し、matchした文字列へsemicolonを加える。

```text
/(&#0*58(?![;0-9])|&#x0*3a(?![;a-f0-9]))/i
```

十進では直後のsemicolon／数字、hexではsemicolon／hex桁が補修を止める。先頭ゼロや元の字大小を保持する。`http&#058x`は補修後にdelimiterができるが、`http&#0589x`は補修されない。hexの`&#x3af`も末尾fがhex桁なので補修しない。

続いて次のcase-insensitive delimiterを**最初の1回だけ**splitする。sourceのlimit=2を保持する。

```text
/:|&#0*58;|&#x0*3a;|&colon;/i
```

suffixがなければ補修済み全文を返す。suffixがあっても、raw prefixにliteralな`/?`があれば同じく補修済み全文を返す。このguardはdecode・空白除去より先であり、entityやNULで分断されたslash/question-markを先に正規化しない。

guardを通った場合だけ、suffixへPHP既定trimを行う。対象は`00 / 09 / 0A / 0B / 0D / 20`で、form feed（0C）は保持する。C whitespaceやRustの一般的なASCII trimとは集合が異なる。

その後prefixだけへ既存のonce2を呼ぶ。once2はdecode→C空白除去→NoNull Remove→ASCII小文字化→protocol側は小文字比較のみ、という既存profileを保持する。結果が`feed:`の場合だけsuffixを再帰処理し、それ以外ではprotocol結果とtrim済みsuffixを連結する。

## Conditional decoder-domain

既存decoderの両pass recognized値0..255・入力1MiBの制限は、**実際にonce2へ渡したprefixだけ**に適用する。全文の事前decode、NoNull、空白除去は行わない。suffixやguardで未参照となったprefixまで検証するとsourceより広い拒否となる。

以下は`http`と`feed`を含む明示allowlistでの例。

| 入力 | 結果・扱い |
| --- | --- |
| `http:&#256;` | そのまま保持。suffixはdecodeしない |
| `plain&#256;` | そのまま保持。delimiterなし |
| `raw/?&#256;:rest` | raw guardで保持。once2を呼ばない |
| `&#256;:rest` | 呼ばれたprefixのdomain拒否 |
| `bad:&#256;:rest` | `&#256;:rest`。先頭の不許可prefixを1回だけ取り除く |
| `feed:&#256;` | 保持。再帰先にはdelimiterがなく、prefix decodeを呼ばない |
| `feed:&#256;:rest` | 再帰先で呼ばれたprefixのdomain拒否 |
| `raw/?path&#58x` | `raw/?path&#58;x`。guardより前の全文補修は有効 |
| `http:a&#58b` | `http:a&#58;b`。suffixにも先行補修が反映される |

URL scheme文法を追加して先頭英字や空でないことを要求せず、任意byte・空prefix・colon等をsourceと異なる理由で拒否しない。外側の`wp_kses_bad_protocol`が行う複数iterationや全体の前処理を、このonce APIへ混ぜない。

## Feed・第3frame・empty文字列

source既定count=1では最大3 frameとなり、feedを2段まで前置できる。第3frameでもonce2を呼んでから、結果が`feed:`なら`count > 2`でemptyを返す。深さだけを見てprefix検証を先に省略してはいけない。

`feed:feed:&#256;:x`は第3frameのprefix domain拒否である。一方、`feed:feed:feed:&#256;:x`は第3のfeedでemptyとなり、呼ばれない第4frameのentityを検証しない。

再帰結果へPHPの`empty`を適用するため、emptyだけでなく**ちょうど`"0"`も**feed prefixを付けずに返す。`feed:0`と`feed:feed:0`は`0`、`feed:00`は`feed:00`。nativeの`is_empty`だけへ置き換えない。

## Top入力上限・working Vec・最終cap

原文のtop raw inputへ1MiBのhost上限を新設した。これはPHPの制約ではなくprototypeの入力domainであり、入口で一度だけ検査する。semicolon補修は膨張し得るため、補修後のVecや内部再帰tailへ同じ上限を流用しない。decoderの入力制限も、そのdecoderを実際に呼ぶprefixだけの条件として扱う。

補修は同じscannerで長さを計測してchecked reservationし、caseを保持して出力する。prefix正規化へ渡すworking capはprefix長＋colon 1 byte等として別管理する。再帰・連結もchecked lengthとfallible reservationを使う。

callerの最終capを検査するのは、topの処理結果が完成した後だけ。`feed:`や第3feedでemptyへ収束する場合、作業途中の`feed:` 5 bytesをcaller cap=0へ当てはめて誤拒否しない。`feed:0`なら最終1 byteが必要であり、cap=0では失敗する。

補修、once2、再帰結果のVecは最終cap前に確保されるunpooledの作業領域で、完成するworking出力もcaller capより大きくなり得る。入力、protocol array、allocatorの実capacity、RSS全体、共有BudgetPoolを計上する機能ではない。`UnsupportedProtocolValue`／`UnsupportedEntityValue`／`InputLimit`／`OutputLimit`を区別し、allocation failureを含む失敗では部分結果を返さない。

## Oracle・metadata・検証範囲

oracleは選択した実source関数を抽出するだけで、KSES全体をbootstrapしない。Bytes・array全value Bytes・count fieldなし・top入力上限・Cを検証する。domain admissionはsourceと同じrepair／split／raw guard／feed条件をたどり、呼ばれるprefixだけをonce2の既存admissionへ渡す。その後、比較対象として実`wp_kses_bad_protocol_once`を既定countで別に呼び、PCRE error zeroとstring結果を必須にする。

oracleの`protocol_once_profile`（正式証跡の`php_protocol_once_profile`）は、Bytes、all-values Bytes、C、default count=1、top入力1,048,576 bytes、max frames=3を厳密なfield集合・型・値で検証する。既存`entity_decode_profile`もこのAPIで必須とし、prototype profileとPHP source全domainを混同しない。

| 検証 | 結果 |
| --- | --- |
| Native契約 | 7件。lookahead／first split／raw guard／PHP trim／feed／conditional domain／cap／入力不変／修復膨張 |
| 限定実PHP比較 | 7,347件、0 mismatch |
| 固有拒否 | PHP／native計94 process。型、未使用を含む非Bytes値、明示count、元／生成prefix域外、第3frame域外、top入力超過 |
| Python | 新profile検査1件を含む関連18 testsを通過 |
| その他 | focused Clippy、fmt、probe build、PHP lint、Python構文検証を通過 |

限定記録は`target/bad-protocol-once-focused-comparison.json`と同directoryのscriptにある。native契約ではtop raw inputがちょうど1MiBで、修復後tail／結果が1MiBを超える場合も確認した。別のPHP／native直送ではtop input 1,048,575 bytesから大きな修復・再帰tailを作り、第3feedでemptyになる結果が一致した。この直送はbatch入力上限を超える別観測で、7,347件や正式corpusへ追加計上しない。source探索297件と型関連6件も別のscratch記録である。

正式比較ではこのAPIの7,347件を含む全59 API・394,212ケースが一致し、hooks572／dispatcher405も成功した。正式reference logとcombined gate logは冒頭の2 fileを根拠とする。

## 未対応範囲

任意count、非Bytes coercion／array cast／短絡診断順、非C locale、PCRE resource failure、域外`chr`挙動、default protocol取得やfilter policyは対応しない。一般URL検証、認可、HTML sanitization、escaping、出力安全性、公開内容の選別を完成した機能ではなく、public HTTP・raw DB・既存の製品処理へ自動適用しない。
