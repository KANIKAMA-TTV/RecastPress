# WordPress once2 protocol helperの限定移植

正式結果：target/bad-protocol-once2-reference-check.logで58 API・386,865ケース／hooks572／dispatcher405が成功し、正式3証跡を実再生成。target/bad-protocol-and-byte-range-check.logでRust本体360件（通常ignore1）・Python112件、Clippy・Linux cross-check・license208観測・実HTTP・DB／全native current provenanceも成功した。DB live／独立実験は再実行せず、直前27件・5 fixture各39項目の実測を維持し、稼働fixture0も確認した。

2026-10-27時点。固定WordPress 7.1.3の`wp_kses_bad_protocol_once2`を、明示Bytes scheme・全要素Bytesのprotocol array・C localeの範囲でnativeへ移植した。native契約6件、限定PHP比較8,158件、固有guard拒否88 processを確認済み。正式PHP比較でも58 API・386,865ケースの一部として一致を確認し、全gateの結果は冒頭に記録した。

このhelperは与えられたbyte列を処理する部分集合であり、製品のURL認可や安全性判定ではない。default protocol policyを採用せず、public HTTP・raw DBへ自動適用しない。

## Source・API・入力domain

固定ソースは`.reference/wordpress-7.1.3/wp-includes/kses.php:2137`。SHA-256は`ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d`で、registryに位置・identity・契約名を登録する。

```rust
wp_kses_bad_protocol_once2_c_bytes(
    scheme: &[u8],
    allowed_protocols: &PhpArray,
    max_output_bytes: usize,
) -> Result<Vec<u8>, BadProtocolError>
```

schemeは明示byte列、allowed protocolはすべてのvalueが`PhpValue::Bytes`である明示arrayに限る。array keyは比較に使わない。callerが各呼出しへ異なるbyte allowlistを渡すことは可能だが、WordPressのdefault protocol取得、filter、製品の許可policyは実装・選択しない。

native／probeは全protocol valueを検証し、先頭が一致していても後続の非Bytes値を拒否する。元PHPの`array_any`は短絡するため、そのような非Bytes値を含む入力では診断の有無や順序が変わり得る。scalarからarrayへのcast、protocol valueのcast、非Bytesのshort-circuit診断は対応domainへ含めない。

## 正確な変換順

sourceの次の順序を保持する。

1. 既存のnumeric entity decoderで、十進pass→hex passを各1回実行する。
2. C localeの空白6 bytes（TAB／LF／VT／FF／CR／space）を取り除く。
3. 既存`wp_kses_no_null`のRemove相当を実行する。
4. schemeのASCII英大文字を小文字化する。
5. protocol valueごとにASCIIの大小文字だけを無視して比較する。一致すれば変換後schemeにcolonを付け、不一致ならemptyを返す。

protocol側へ適用するのは小文字比較だけであり、entity decode、空白除去、control除去、slash-zero除去は行わない。schemeとallowlistを同じ一般的なURL正規化へ通してはいけない。

空白除去が新しいslash-zero runを作るため、2と3は交換できない。たとえば`ht`、literal backslash、TAB、ASCII `0`、`tp`の連結は、空白除去後に`ht\0tp`となり、次のRemoveで`http`になる。spaceでも同様である。

decodeの各passを繰り返さず、NoNull後に再decodeもしない。十進passやhex passの最後、あるいはNoNullによって新たに参照らしい文字列が現れても、その時点以後のsourceがdecodeしないものはそのまま保持する。

## 文法や安全性の規則を追加しない

この関数はURL scheme文法を検証しない。先頭英字や空でないことを追加要求せず、既存colonを除去せず、不正UTF-8を修復しない。明示allowlistとの一致結果を、sourceどおりのbyte列で返す。

| Scheme入力 | Protocol value | 結果 |
| --- | --- | --- |
| `HTTP` | `http` | `http:` |
| empty | empty | `:` |
| `0` | `0` | `0:` |
| `http:` | `HTTP:` | `http::` |
| `123_+/@` | 同じbytes | `123_+/@:` |
| raw `FF` | raw `FF` | `FF 3A` |
| `ht tp` | `ht tp` | empty。protocol側のspaceは残る |
| `&#104;ttp` | `&#104;ttp` | empty。protocol側はdecodeしない |
| `&#38;#256;` | `&#256;` | `&#256;:`。十進passを再実行しない |
| `&#x26;#x100;` | `&#x100;` | `&#x100;:`。hex passを再実行しない |

上記の一致を、HTTPで許可すべきURLや安全なprotocolと判断してはいけない。任意byteを明示listへ渡した結果であり、公開用の認可policyを定義したものではない。

## Decoderの継承範囲とcap

schemeには既存decoderのdomainをそのまま継承する。全入力は1MiB以下で、**実際に成立する参照値は十進・hexの両passとも0..255**に限る。十進変換で生成されたhex参照も検証する。`&#256;`や、十進passから作られる`&#x100;`はUnsupportedEntityValueであり、PHP 8.5の`chr` deprecated診断やmodulo結果を互換成功へ数えない。

allowlistが空でもschemeのdecodeを実行するため、未許可だからといってこのdomain／入力上限を迂回しない。反対に、最後のpass後に現れた参照を新たにdecodeしてdomain拒否することもない。

内部decoderへはscheme入力長、NoNullへはその入力長を作業用の出力allowanceとして渡す。これらは縮小処理のため、callerの最終capを作業領域へ使って縮小前に誤拒否しない。decoded Vec内でC空白を除去し、NoNull結果をASCII小文字化して比較する。

最終capを照合するのは一致したscheme＋colonのbyte長だけで、加算はcheckedとする。その後colon用のreservationを確保して追加する。不一致ならcap=0でもemptyを返せるが、事前のdomain／入力上限／作業allocation failureが免除されるわけではない。empty schemeが一致した場合もcolon 1 byteが必要なため、cap=0なら失敗する。

`BadProtocolError`は`UnsupportedProtocolValue`、`UnsupportedEntityValue`、`InputLimit`、`OutputLimit`を区別する。OutputLimitには中間／最終allocation failureを含み、部分出力は返さない。入力、allowlist、中間Vec、allocatorの実capacity等は最終byte capの外にあるunpooled領域であり、製品容量設定、RSS全体や共有BudgetPoolを予算化する機能ではない。

## Oracleと限定検証

oracleは固定sourceの実helperと既存依存関数を抽出し、KSES全体をbootstrapしない。schemeのBytes guard、array型・全valueのBytes guard、C locale guardを行い、既存decoderの両pass domain／入力上限をadmissionとして検査した後、実`wp_kses_bad_protocol_once2`を呼ぶ。PCRE状態をresetし、error zeroとstring結果を要求する。

C localeは既存のglobal実測metadataでも照合する。oracleの`entity_decode_profile`（正式証跡では`php_entity_decode_profile`）にあるBytes／0..255／1MiB／decimal-then-hexの厳密な型・値・field集合を、このAPIの証跡検査でも必須にした。metadata欠落を拒否する検査を既存testへ追加し、製品のcharsetやprotocol policyは推定しない。

native契約6件では、変換順、allowlist側非正規化、empty／0／colon／非URL形式、全byte・ASCII case、入力とallowlist不変、未使用の非Bytes value拒否、空allowlist時のdecoder拒否、生成参照、最終cap・colon・作業領域の分離を確認した。focused Clippy、fmt、probe build、PHP lint、Python構文検証と既存の関連17 testsも通過した。

固定PHP 8.5.12を`-n`・128MiB・C profileで実行した限定比較は8,158件で0 mismatch。全byte文脈、numeric参照、keyが混在するarray、大小文字・control・空白、長い先頭ゼロや縮小入力、有限randomを含む。型違い、非Bytes valueが一致候補の前後にあるarray、元／生成された範囲外参照、入力上限超過について計88 processで固有拒否を確認した。

限定結果は`target/bad-protocol-focused-comparison.json`と同directoryのscriptへ保存する。source探索546件は`target/bad-protocol-once2-observation.php`／`.json`で別に保持し、正式等価性corpusへ加算しない。正式corpusへの追加予定は8,158件、全体は58 API・386,865件。正式PHP全比較とcheckpoint結果はrootの実行後に追記し、限定検証と区別する。

## 未対応範囲

非C locale、scalar／array cast、非Bytes protocolの短絡診断順、PCRE resource failure、decoder域外の`chr`挙動、暗黙のdefault allowlistやfilter policyは対応しない。URL認可、HTML sanitization、escaping、出力安全性、公開情報の選別を完成した機能ではなく、public HTTP・raw DB・既存互換処理へ自動適用していない。
