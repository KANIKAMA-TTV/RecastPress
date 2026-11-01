# WordPress byte control除去の開発記録

2026-10-27時点。固定WordPress 7.1.3の`wp_kses_no_null`を、Bytes入力と明示Remove／Keep optionの範囲でnativeへ移植した。native契約5件、限定PHP比較8,324件、型／optionの固有guard拒否40 processを確認済み。`target/no-null-reference-check.log` で **56 API・374,343ケース／hooks572／dispatcher405の正式PHP比較が成功**し、3正式証跡を実再生成した。続くtarget/no-null-and-binary-bundle-full-check.logでworkspace Rust332件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・binary版2を含む実process HTTPも成功した。全fixtureを回収し、WP current provenanceも最後に別途照合した。

## Sourceと明示option

一次ソースは`.reference/wordpress-7.1.3/wp-includes/kses.php:2020`。SHA-256は`ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d`で、registryにsource位置・identity・契約名を登録する。oracleは実関数だけを抽出し、KSES全体やtag globalsをbootstrapしない。

```rust
wp_kses_no_null_bytes(
    input: &[u8],
    mode: SlashZeroMode,
    max_output_bytes: usize,
) -> Result<Vec<u8>, NoNullError>
```

`SlashZeroMode`は`Remove`と`Keep`の2値。`Default`は`Remove`であり、固定PHPの既定のslash-zero除去に対応する。native関数の引数は明示的で、PHPの任意のoptions配列・scalar coercionを受け付けるAPIではない。

正式corpus用probeは`slash_zero`を厳密な文字列`"remove"`／`"keep"`として必須にし、PHP側ではそれを実関数の`['slash_zero' => $mode]`へ渡す。未設定、Null、Bool、整数、array、`"Remove"`、末尾NUL等は両probeで拒否する。元PHPでは未設定／Null fieldはdefault removeとなり、`"Remove"`等はstrict比較でkeep側になるが、その広いoption解釈をformal対応domainへ含めない。

## Control除去とslash-zeroの処理順

最初に次のraw byteだけを取り除く。

```text
00..08 / 0B / 0C / 0E..1F
```

TAB（09）、LF（0A）、CR（0D）、DEL（7F）、80..FFは保持する。Unicodeのcontrol分類、文字のdecode、UTF-8修復は行わない。不正UTF-8も、このbyte集合に該当する部分以外は保持する。

`Remove`では、その**control除去後のstream**上にある「1個以上のliteral backslashと、その直後の1個以上のASCII `0`」を除去する。`Keep`ではこの第2処理を行わない。ASCII backslash（5C）と数字0（30）の文字列の処理であり、実NULの処理とは区別する。

以下はhex byte列で示す。`5C`はbackslash、`30`はASCII `0`、`00`は実NUL。

| 入力bytes | Remove出力 | Keep出力 | 意味 |
| --- | --- | --- | --- |
| `5C 30` | empty | `5C 30` | literal slash-zero |
| `5C 5C 30 30 30` | empty | 元bytes | backslash列・zero列全体を除去 |
| `5C 00 1F 30 30` | empty | `5C 30 30` | 第1処理で新しいrunが作られる |
| `5C 30 0A 30` | `0A 30` | 元bytes | LFは保持され、runを区切る |
| `5C 09 30` | 元bytes | 元bytes | TABもrunを区切る |
| `5C 30 5C` | `5C` | 元bytes | 末尾の単独backslashは保持 |

slash-zero regexの単回passを再現し、出力を再帰的にescape解釈しない。`\n`／`\r`／`\t`、`\x0`、`&#0;`をdecodeしない。既存の`wp_unslash`、numeric entity decoder、HTML entity normalizerとは別の処理であり、それらへ自動適用しない。

## 最終byte capとallocation

nativeはborrowed inputをcontrol filter付きiteratorで読み、Remove時はその上でbackslash列とzero列を処理する。同じ走査を計測と出力で使い、中間Vecを作らない。入力自体は変更しない。

全出力長をcheckedに計測してからcallerの`max_output_bytes`を照合し、`try_reserve_exact`後に完全出力を生成する。入力byte長を最終capへそのまま当てはめないため、大量のcontrolやslash-zeroがemptyへ縮小される場合はcap=0でも成功する。保持される長いbackslash列は、その最終長のexact capで成功し、1 byte不足なら失敗する。prototype独自の全入力長上限は追加していない。

`NoNullError::OutputLimit`は最終byte cap、長さoverflow、出力allocation失敗を含み、部分出力を返さない。capはPHPの失敗仕様や製品容量設定ではなくhostの出力制約。input、allocatorの実capacity、metadata、RSS全体や共有BudgetPoolの予約を計上した機能ではない。

## 限定検証の記録

native契約5件は、全256 byteの正確なcontrol集合、TAB／LF／CR／DEL／高位byte保持、第1処理で作られるslash-zero run、前後・連続run、単回pass、他escape／entityの非解釈、長いcontrol／backslash／zero列、exact capと1 byte不足、入力不変、enum既定値を検査した。focused Clippy、fmt、probe build、PHP lint、Python構文検証、corpus生成・比較helperの既存5 testsも通過した。

限定実PHP比較は固定PHP 8.5.12を`-n`・128MiBで実行し、Remove 4,162件・Keep 4,162件、合計8,324件で0 mismatch。全byteを異なるslash／zero／control文脈へ置いた例、長いrun、有限random・atom合成を含む。PHP oracleはPCRE状態をresetして実関数を呼び、error zeroとstring結果を必須とし、診断や失敗を成功として数えない。

型違い・option欠落／型違い／大小文字違い／未対応値について、PHP／native計40 processで固有guard拒否を確認した。結果は`target/no-null-focused-comparison.json`、対応する限定scriptは同directoryの`no-null-focused-comparison.py`。source option・診断の探索570件は`target/kses-no-null-observation.php`／`.json`で別に保持し、正式等価性corpusへ加算しない。

正式corpusへ追加する予定は8,324件で、既存分と合わせ56 API・374,343件となる。正式PHP全比較とcheckpoint結果はrootの実行後に追記し、限定検証の成功と混同しない。

## 未対応範囲

非Bytesのscalar／array／object入力、任意optionのcoercion・診断、PCRE resource failureは再現しない。sourceでNull内容を渡すとPHP 8.5のdeprecated診断が出る経路も対象外。statelessなbyte処理であり、locale・site charset・mbstring・WordPress global allowlistを読み取らない。

一般HTML sanitizer、escaping、安全な出力、URL認可、JSON/XML本文解析、製品charset・容量policyの選択、公開情報の選別は提供しない。public HTTP、raw DB読み取り、既存互換APIへ自動接続していない。
