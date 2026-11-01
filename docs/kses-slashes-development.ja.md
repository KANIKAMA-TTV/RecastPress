# WordPress KSESの限定slash除去

正式結果：target/kses-slashes-reference-check.logで57 API・378,707ケース／hooks572／dispatcher405が成功し3証跡を実再生成。続くtarget/kses-slashes-and-typed-redirect-full-check.logでRust本体344件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPも成功し全fixture回収。WP provenanceも最後に照合した。

2026-10-27時点。固定WordPress 7.1.3の`wp_kses_stripslashes`についてBytes入力の部分集合をnativeへ移植し、native契約4件と限定PHP比較4,364件で一致を確認した。正式PHP比較でも57 API・378,707ケースの一部として一致を確認し、全gateの結果は冒頭に記録した。

## 固定sourceとAPI

元関数は`.reference/wordpress-7.1.3/wp-includes/kses.php:2044`の`wp_kses_stripslashes`。source SHA-256は`ead6be7192295b9e7417c2146704c1935748f92b9ef4f6adca193a0e4118469d`で、registryに位置・identity・native契約名を登録する。固定sourceでは`wp_kses_split2`から呼ばれるが、このbyte helperの移植はKSESのHTML処理全体を移植したものではない。

```rust
wp_kses_stripslashes_bytes(
    input: &[u8],
    max_output_bytes: usize,
) -> Result<Vec<u8>, KsesSlashesError>
```

元のregexが対象にするのは、literal backslash（hex `5C`）とdouble quote（`22`）の隣接2 bytesだけ。これをdouble quote 1 byteへ置換する。入力はbyteのまま読み、UTF-8への変換、Unicodeの判定、localeやsite charsetの参照は行わない。

## 非再帰の単回置換

元入力に対する単回の非重複置換を再現し、出力へ再び同じ処理を適用しない。quote直前のbackslashがn本（n >= 1）なら、最後の1本だけを除去してn−1本を残す。出力にbackslash＋quoteが残っても、それを再処理してゼロ本へ減らさない。

以下の表はhex byte列で示す。

| 入力 | 出力 | 動作 |
| --- | --- | --- |
| `5C 22` | `22` | 2-byte pairの置換 |
| `5C 5C 22` | `5C 22` | 2本のうち1本だけ除去 |
| `5C 5C 5C 22` | `5C 5C 22` | 残るpairを再帰処理しない |
| `5C 22 5C 22` | `22 22` | 元入力の隣接pairをそれぞれ置換 |
| `5C 27` | 元bytes | single quoteのescapeは対象外 |
| `5C 30` | 元bytes | literal slash-zeroはNULへ変換しない |
| `5C 78` | 元bytes | 他escapeのbackslashを除去しない |
| `5C` | 元bytes | 末尾backslashを保持 |

既存`wp_unslash`／`stripslashes`の処理とは異なる。既存処理には他escapeのbackslash除去、slash-zeroからNULへの変換、backslash pairや末尾backslashの処理がある。この新helperでそれを置き換えたり、既存APIへ自動適用したりしない。

raw NUL、不正UTF-8、高位byte、日本語等の文字列、`&quot;`／`&#34;`等のentityテキスト、literalなslash-n／slash-r／slash-tも、対象2-byte pair以外は保持する。quote自体をHTML entityへescapeする機能ではない。

## 最終byte capとallocation

同じscannerを用いて最終出力byte数をcheckedに計測し、callerのcapを照合してから`try_reserve_exact`し、再走査して完全出力を作る。中間Vecやprototype独自の全入力長上限は追加していない。入力は変更しない。

入力のbyte長をそのまま最終capへ当てはめないため、pairが多く含まれた長い入力も縮小後のexact capで成功する。1 byte不足なら`KsesSlashesError::OutputLimit`を返す。このerrorは長さoverflowと出力allocation失敗も含み、部分出力は返さない。empty入力はcap=0で成功する。

capはhostの出力制約であり、PHPの失敗仕様や製品容量設定ではない。入力、allocatorの実capacity、metadata、RSS全体や共有BudgetPoolを予算化する機能ではない。

## 限定検証と正式gateの区別

native契約4件で次を確認した。

- literal pairだけの除去、連続backslashのn−1保持、非再帰処理、他escapeと末尾backslashの保持。
- 全256 byteをpair周辺へ配置した場合の正確な出力。
- empty、長いpair列・backslash列の縮小後exact capと1 byte不足。
- raw NUL／不正UTF-8／entityテキストの保持と入力不変。

固定PHP 8.5.12を`-n`・128MiBで実行した限定比較は4,364件で0 mismatch。全byte文脈、0..8本・64本・4,096本のbackslash列、他escape、日本語／binary入力、長いpair列、有限random／atom合成を含む。PHP oracleは実source関数だけを抽出し、Bytes-only guard、PCRE状態のreset、error zero、string結果を要求する。KSES bootstrapは行わない。

非Bytesの7入力種についてPHP／native計14 processで固有guard拒否を確認した。Null等を実sourceへ渡した際のcoercionや診断は対応しない。source探索の562 byteケースと7型の観測は別記録であり、正式等価性corpusへ加算しない。

限定結果は`target/kses-slashes-focused-comparison.json`、scriptは同directoryの`kses-slashes-focused-comparison.py`、source探索は`target/kses-stripslashes-observation.php`／`.json`に保存する。focused Clippy、fmt、probe build、PHP lint、Python構文検証、既存corpus生成・比較helper 5 testsも通過した。

正式corpusへの追加予定は4,364件で、既存分と合わせ57 API・378,707件となる。この限定検証の成功を、正式PHP全比較やcombined gateの完了と混同しない。

## 対応しない範囲

非Bytesのscalar／array／object入力、PHP coercion／診断、PCRE resource failure、再帰的なescape解釈、一般HTML sanitizer、escaping、安全な出力、URL認可、公開内容の選別は実装していない。site charset・容量policyを選択せず、public HTTP・raw DB・既存の互換処理へ接続しない。
