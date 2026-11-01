# raw WP password verification dispatcher 候補

後続全体実測：`target/password-dispatch-workspace-reference-check.log`でRust464、Python195、正式64 API・437,198比較／hooks572／dispatcher405、生成相互照合と拡張legacy113ケース（actual WP88／明示拒否25）、Clippy・server/WP Linux check・license215・実HTTPが成功した。DB関連全checkerもcurrentを確認し、DB再起動なしで直前の独立29件／8 fixture各48を維持した。旧64ケースは順序・内容・期待とcorpus SHAを保ち、combined49だけを追加している。認証／sessionやfallback全体の完成ではない。

2026-10-29。固定 WordPress 7.1.3 `wp-includes/pluggable.php:2843` の `wp_check_password` から、raw Bytes、empty custom hasher、empty filter registry、user_id=0を明示した純粋候補。新 API は `wp_check_password_raw_empty_filters_bytes(password, hash, HostPasswordWorkLimits)`。既存MD5・portable phpass・WP-prefixed bcryptの3検証helperへ委譲し、旧APIの受理範囲や順序を変更しない。新crate、Cargo依存、DB依存、HTTP認証経路は追加しない。

## source順序とsubset

| 順序 | 条件 | 動作 |
| --- | --- | --- |
| 1 | hash長32 bytes以下 | MD5 helperを一度だけ呼ぶ。password長4096超でもこの枝を選ぶ |
| 2 | hash長33以上かつraw password長4096超 | prefix/ASCII/cost/salt/checksum解析前にfalse。検証helperは呼ばない |
| 3 | 残りのhashがliteral `$wp` prefix | 既存 `$wp$2y$` helperを一度だけ呼ぶ |
| 4 | 残りのhashがliteral `$P$` prefix | 既存canonical portable helperを一度だけ呼ぶ |
| 5 | その他 | `UnsupportedHash`。別algorithmへのfallbackはしない |

これにより、短い `$P$` / `$wp` 文字列もMD5枝でfalseとなる。hash長33以上の未知形式、非ASCII、malformed cost、非canonical checksumなどと4097-byte passwordの組合せは、形式の良否を判定せずfalseになる。単体helperはそれぞれのenvelopeを先に限定するため、このdispatcherより狭いoverlength admissionを持つ。その既存契約を壊して総合化するのではなく、上位のsource guardを別に置く。

raw password長4096以内では、`$wp` のうち既存helperが受理する63 ASCII bytesの `$wp$2y$` だけ、`$P$` のうち34 bytes/cost7..30/crypt64 salt8/canonical checksum22だけを受理する。裸bcrypt、Argon、`$H$`、crypt fallback、任意salt bytesなどは未対応。hash長32以下のMD5枝はlowercase hexadecimalとの完全一致を使い、短hashはfalse、大文字hexへ正規化しない。passwordへのtrim・Unicode変換・NUL終端処理は一切追加しない。生成側のtrim6を検証側へ持ち込まない。

元WPではcustom hasherがraw password長guardより先に呼ばれ、最後の `check_password` filterはfalseを含む結果を書き換え得る。本候補はその両方が空というpreconditionであり、任意hasher/filterへの同等性やnativeでのhook bookkeepingを主張しない。user lookup、username/email優先、unknown-user処理、password再生成・保存・rehash、session/cookie、login成否、認可のAPIではない。

## host admissionとerror

`HostPasswordWorkLimits::new(HostMaxBcryptCost, HostMaxPhpassCost)` は既存2型を両方必須にし、private fieldsを持つ。defaultはない。getterはtyped costだけを返す。bcryptの4..31とportableの7..30という構築条件は既存型が検証する。costの製品既定・推奨値・CLI設定は選ばない。MD5枝には新しい任意4096上限を追加しないため、legacy MD5の入力に比例するworkは依然として残る。

返値のtrue/falseはmatch/mismatch。`PasswordDispatchError` は `UnsupportedHash` / `BcryptCostLimit` / `PortableCostLimit` / `ProviderFailure` に閉じ、helperのcost拒否とprovider異常を通常の不一致へ潰さない。provider失敗で別helperを試さない。Debug/Display/sourceへpassword、hash、driver/provider本文を保持しない。host cost超過は各helperがHMAC/反復CPU処理前に拒否するが、wall time、CPU cancellation、concurrency、RSSやauth全体のconstant-time性は保証しない。

dispatcher自身はsliceを借用する分岐だけで、payload clone、hash生成、RNG、新しい暗号primitiveを持たない。既存MD5/bcrypt/HMAC providerと派生ownerのzeroize範囲、compiler/provider内部の全消去を保証しない制約は各helperの文書を継承する。private test seamは呼出数と戻り値を検査するためだけで、外部callback/plugin登録機構ではない。

## 契約と独立比較

private3契約は選択された各helperの一度だけの委譲、元sliceとtyped allowanceの引渡し、長さguard時の全helper呼出0、MD5の長さ優先、ProviderFailureの伝播とfallback無しを拘束する。public3契約はactual固定PHP由来のMD5/portable/bcrypt dummy vectors、NUL/非UTF-8/前後空白、hash32/33とpassword4096/4097境界、algorithm別cost拒否、元入力不変を検査する。bcryptは低costの既存公開dummyを使い、新しいrandom hashを生成しない。

既存legacy専用相互比較harnessへcombined modeを追加する方針で、両maximumを厳密型で必須にし、actual `wp_check_password` の空callback/user0 profileと比較する。元の単体MD5/portable corpus/refusalsは維持する。未知形式でも長さguardでfalseとなるcaseはactual PHPを呼べる領域とし、短passwordのunsupported/fallback領域を黙ってfalseとして比較しない。provider failureは成功evidenceへ変換しない。

既存の正式64 API・437,198件に、新しいWordPress関数としてdispatcherを加算しない。純粋Rustの新契約と独立harnessは別scopeで記録する。直前の全gateは458＋独立29 Rust、Python194、生成dummy10、legacy64（actual50＋拒否14）、全8 DB fixture各48項目で成功した歴史記録であり、この新dispatcherの全gate成功を先取りしない。DB/source/proofへの変更は本担当では行わない。
