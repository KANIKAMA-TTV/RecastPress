# legacy WP password verify-only 候補

後続全体実測：`target/legacy-password-and-user-lookup-full-check.log`でRust458＋独立29、Python194、正式64 API・437,198件／hooks572／dispatcher405、生成相互照合と別枠legacy64ケース（actual WP50・明示拒否14）、8 DB fixture各48項目、Clippy・server/WP Linux check・license215・HTTPが成功した。artifactは `compatibility/evidence/wp-legacy-password-crosscheck.json`。実行入口は `scripts/compare_legacy_passwords.py`／`scripts/check_legacy_password_evidence.py`、全体flagは `--with-legacy-passwords`。本subsetを正式API数や既存比較件数へ加算しない。

2026-10-29。固定 WP 7.1.3 `pluggable.php:2843` の `wp_check_password` と `class-phpass.php:131,247` に基づく、raw Bytes・empty custom hasher・empty filter registry・user_id=0の限定候補。新APIは `wp_check_password_legacy_md5_bytes(password, hash)` と `wp_check_password_phpass_portable_bytes(password, hash, HostMaxPhpassCost)`。既存WP-prefixed bcrypt検証・既定bcrypt生成は変更しない。MD5/phpass生成、rehash、保存・認証・session・HTTP・実ユーザーcredentialへの接続は追加しない。

元の分岐順は、hash長32 bytes以下ならMD5、次にcustom hasher、次にraw password長4096超ならfalse、その後 `$wp`、`$P$`、その他の `password_verify`、最後に `check_password` filterである。各native APIは明示的なsubsetのbool semanticsだけを担い、hook bookkeepingや任意callbackを実行しない。2APIを無条件に順番に試す総合password dispatcherでもない。

## MD5 branch

hash長32 bytes以下だけを受理する。MD5は全raw password bytesのlowercase hexadecimal 32 bytesと、stored hashを固定長比較する。32 bytes未満は一致不可能なのでfalseを返す（nativeではMD5計算を省略する）。32 bytesの不正文字・大文字hex文字はmismatchであり、uppercase化や数値比較へ変換しない。hash長33以上は `UnsupportedHash`。source順に合わせてpasswordの4096制限を追加せず、4097 bytesのdummyも一致する。trim・UTF-8変換・NUL終端は行わない。

この分岐にはsource由来の入力長上限がなく、入力に比例したCPU workが残る。host/APIへ接続する場合の入力admission、concurrency、bounded blockingは別契約である。nativeが勝手に4096超をfalseへ変えることはしない。MD5は旧値の検証用途に限定し、新しいpasswordの生成方式として提示しない。[PHP MD5](https://www.php.net/manual/en/function.md5.php)、[hash_equals](https://www.php.net/manual/en/function.hash-equals.php)

## portable phpass branch

初版のadmission順は次の通り。

1. exact34 bytesかつliteral `$P$` prefixを確認する。それ以外は `UnsupportedHash`。
2. raw password長4096超ならfalseを返す。ここではcost/salt/checksumをまだ解析しないため、同じenvelopeのmalformed bodyや非ASCII bodyとの組合せもfalseになる。
3. cost記号のalphabet indexが7..30であることを検査し、必須の `HostMaxPhpassCost` 以下かを検査する。無効costは `UnsupportedHash`、host allowance超過は `CostLimit`。
4. salt8 bytesとchecksum22 bytesを `./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz` に限定し、checksum末尾を `./01` に限定する。これ以外は `UnsupportedHash`。
5. 全条件適合後だけMD5を計算し、stored prefix12 bytesと再encodeしたchecksum22 bytesの全34 bytesを比較する。

`HostMaxPhpassCost::new(u8)` は7..30のみ、31は不可、defaultはない。元classのconstructor引数8は検証costを固定しない。stored hashのcostにより、初回 `MD5(salt || password)` と `2^cost` 回の `MD5(previous_digest || password)` を行う。cost30は1,073,741,824回の反復であるため、stored costを無条件に実行しない。host内部の明示allowanceであり、製品の推奨cost・manual設定・wall time・CPU cancellationの保証ではない。高costの拒否試験はproviderを呼ばず、cost30の実計算をしない。

全raw password bytesを使い、trim・UTF-8正規化・NUL終端・72-byte切詰めはない。saltはsourceでは任意8bytesを許すが、本候補は上記crypt64 ASCII alphabetだけに狭める。sourceの独自alphabetをlittle-endian 6-bit順にencodeする。standard Base64、bcrypt Base64やdecoded16 bytesだけの比較へ置換しない。stored checksumの未使用上位bitsを無視すると非canonical表記を誤受理し得るため、canonical admissionと全34-byte再encode比較の双方で固定する。

sourceの`crypt_private`は不正prefix/cost/salt長で `*0`（特定prefixでは `*1`）を返し、`CheckPassword`はその場合だけPHP `crypt()`へfallbackする。`$H$`をclass自体は認識するが、WordPressの `$P$` branchとは同一ではない。これらfallback、任意salt、異なる長さ、他形式、PHP型coercionや診断は本候補の対象外であり、一律falseと一般化しない。[PHP crypt](https://www.php.net/manual/en/function.crypt.php)

## provider・所有・観測範囲

MD5 primitiveを独自実装しない。rootが追加した `md-5 0.11.0`（default features無効、zeroize有効）のsafe APIでMD5を計算する。`digest 0.11` は既存graph内、固定長比較には既存 `subtle 2.6.1`（default features無効）をdirect利用する。新packageはMD5 providerの1件で、取得・manifest/lock・license inventoryはroot担当。providerのstate/block lengthはzeroize featureでdrop時消去され、自前16-byte digestと32/34-byte encoding ownersも `Zeroizing` で消去する。入力、provider temporaries、compiler copies、allocatorや全process memoryの消去は保証しない。自前scratchは固定長stack ownersだがshared pool/RSS監視には接続しない。[MD5 provider](https://docs.rs/md-5/0.11.0/md5/)、[固定長比較API](https://docs.rs/subtle/2.6.1/subtle/trait.ConstantTimeEq.html)

`LegacyPasswordVerifyError::{UnsupportedHash, CostLimit}` とcost型の診断は有限の固定文字列で、password/hash/salt/provider文字列を保持しない。true/falseはmatch/mismatchで、unsupportedとhost拒否は別の結果である。MD5 providerはこの入力型でinfallible APIとして利用し、process abort・panic等を全てtyped provider failureへ変換するとは主張しない。subtleによる固定長比較を利用するが、形式・長さ・cost分岐、unknown-user path、login全体のconstant-time性や暗号audit完了を意味しない。元phpassの `===` と同じ返値を得ることと、元のtiming挙動の再現は区別する。

private3契約はhost型、短hash/overlength/host拒否/非canonical時のCPU呼出0と順序、little-endian encodingと全storage比較を検証する。public3契約は固定PHP 8.5.12から得たMD5 dummy4件とportable dummy6件、empty/NUL/非UTF-8/前後空白、cost7/13、4096/4097境界、mismatchとUnsupported/CostLimitの区別、入力不変を検証する。fixtureは公開dummyのみ。portable値は固定classの `crypt_private` が出力した観測データで、nativeで生成した期待値ではない。

固定classのSHA-256は `eac68d6268d6704c8ca3195a731a6cc476e6dbfc6a604adb784886e3a0b10221`。source依存とactual PHP comparisonは独立harnessで束縛する方針であり、既存正式64 API・437,198件にこの検証経路を新しいWordPress関数として無断加算しない。この文書時点で後続の独立比較・全体gateの成功を先取りしない。既存bcrypt proof、DB/server、root Cargo/lock、全体checkerや正式proofは本担当で変更していない。

## combined modeの限定比較（2026-10-29）

既存の `legacy_password_probe` と専用PHP/Python harnessに `combined` modeを追加した。新しいprobe・checker frameworkやflagを増やさず、同じ `wp_check_password` の実呼出しと `wp_check_password_raw_empty_filters_bytes` の返値を比較する。元の単体64ケースは内容・順序・期待値を維持し、corpus SHA-256 `4c19c37feeac6367141e79a6a8e2a9264895d3aee9ffe2469937447055c99e6f` でも照合する。追加49ケースを含む限定比較113ケースは成功し、実WP呼出し88件とhost/domain拒否25件を明確に分けている。この成功は専用比較の結果で、後続workspace全体gateの成功や既存正式API件数の増加を意味しない。

combinedの順序はhash長32以下のMD5、次にraw password長4096超のfalse、その後だけ `$wp` と `$P$` の既存限定helper、それ以外はUnsupportedである。したがってMD5はpassword8192bytesでも一致し得る一方、33bytes以上のunknown/non-ASCII/noncanonical hashはpassword4097bytesなら形式検査前にfalseになる。同じhashとpassword4096bytesの組合せはUnsupportedになり得る。短い `$P$` もhash長32以下ならMD5分岐であり、portable parserへ渡さない。raw bytesをtrimしない。

両方のtyped host allowanceを `HostPasswordWorkLimits` に明示する。harnessでも `bcrypt_maximum`（4..31）と `portable_maximum`（7..30）を両方の厳密integerとして必須化し、未使用branchのallowanceも省略・型coercionしない。このhost入力subsetはPHPのcoercionや診断順の再現ではない。bcryptとportableのcost拒否は別分類で、provider failureはfalseや成功したrefusal観測へ変換せずchild失敗とし、証跡を書き出さない。bcryptの固定公開dummyは既存 `wp_bcrypt_profile.py` を再利用し、その定義sourceとordered vector hashも束縛する。新たなrandom saltや生成APIは使わない。

実PHPを呼んだcaseは `check_password` filter1回、host/domain拒否は `actual_called=false`・filter0回を要求する。空custom hasher/filter・user_id=0、4globalの未設定/null/sentinel/空状態のfinally復元、actions不変を検査する。成功前にpinned source/class/tool、Cargoが返した現executable、実child PID、source/binary hash、全case順序、厳密field/typeを照合する。診断引数を抑止し、型拒否の公開secret markerがstdout/stderrへ出ないことも確認する。Python契約は追加分を含む7件、対象Clippy/fmtも通過している。未対応のfallback、認証/session、unknown-user timing、HTTP/DB接続や本番採用へ範囲を広げない。

再実行は `python scripts/compare_legacy_passwords.py` と `python scripts/check_legacy_password_evidence.py`。既存 `--with-legacy-passwords` が同じ専用比較を実行し、registry API64件・正式3proofへ加算しない。
