# 準備済み cache の単一 post meta 取得候補

2026-10-29。caller が明示的に用意した完全・非空の post-meta cache から、`get_post_meta(..., true)` の限定した返却意味を取り出す pure WP helper。対象 3,202 ケースに続き、`target/post-meta-cached-full-check.log` が exit 0 で完了した。正式 73 API／486,153 件、全 8 fresh DB 構成各 62 契約と最終 11 checker が成功した。以下の限定 profile の結果であり、一般の cache／metadata 取得や認証の完成を意味しない。

## Consumer と固定 source

固定 WordPress 7.1.3 の `wp-includes/media.php:1186` には、画像 alt 属性の元文字列を `get_post_meta($attachment_id, '_wp_attachment_image_alt', true)` で取得する consumer がある。本候補はこの getter 部分だけを扱い、その周囲の trim／strip_tags、画像解決・属性出力・公開可否を実装したものではない。

`post.php:2750-2751` の `get_post_meta` は `get_metadata('post', ...)` を呼ぶ。`meta.php:601-607` は raw 結果が Null の場合だけ default へ進む。`meta.php:628-692` の型・ID guard、get filter、cache 取得、選択値の復元と、`712-749` の default-single 空文字列を、本候補の制限された前提と照合する。

全 WP bootstrap、テーマや画像 API、`update_meta_cache` の SQL／object cache、認証・公開処理はこの helper に持ち込まない。

## Native API

`recastpress_wp::post_meta_cached` と lib export は次を提供する。

```rust
PreparedPostMetaCache::new(object_id: i64, cache: &PhpArray)
    -> Result<PreparedPostMetaCache<'_>, PostMetaCachedError>
get_post_meta_single_cached_no_filters(
    cache: &PreparedPostMetaCache<'_>, key: &[u8], limits: UnserializeLimits,
) -> Result<PhpValue, PostMetaCachedError>
```

object ID は正の signed i64、cache は PHP-truthy な非空の `PhpArray`。caller は、その ID に対応する完全な cache であり、`get_post_metadata`／`default_post_metadata`／`all` の関連 filter がないことを明示的に保証する。constructor が確認するのは ID の正値と非空だけで、DB との対応や完全性を検証しない。cache の所有権は caller に残り、view は借用する。

key は PHP-truthy な任意 bytes。空文字と `0` は source では全 cache 取得を要求するため、この single profile から除外する。キー照合は借用した bytes と PHP の canonical signed i64 integer-string 規則を使い、key を clone・文字列整形しない。`1` と integer key 1 は対応し、`01`、`+1`、`-0`、i64 範囲外の文字列は byte key のまま。case・NUL・非 UTF-8 を勝手に正規化しない。

選択した outer entry が存在し、Null でなければ、index 0 から連続する非空 array で、各 raw 値が Null または Bytes であることを検査する。decode するのは先頭 1 値だけ。後続の object 風文字列や、別 key の未選択値を decode したり、その内容から既定値や優先順位を推測したりしない。raw list の全形状検査と、先頭の serialized 内容解釈は別の処理である。

## Null と default の区別

| 選択状態 | 返却値 | 実 PHP の default hook |
| --- | --- | --- |
| key 未存在 | empty Bytes | 1 |
| outer entry が Null（isset-false） | empty Bytes | 1 |
| list index 0 が SQL NULL を表す Null | empty Bytes | 1 |
| 先頭 Bytes の canonical `N;` が decoded Null | empty Bytes | 1 |
| raw empty Bytes | empty Bytes | 0 |
| serialized empty string | empty Bytes | 0 |
| canonical `b:0;` | Bool(false) | 0 |

同じ empty Bytes 結果でも default branch の有無を混同しない。非 Null の返却値は型を保ち、decoded 文字列をさらに復元しない。未対応 encoding や上限超過は型付きエラーで停止し、空文字・Null・PHP false へ置換しない。native API は filter bookkeeping を実行せず、実 PHP oracle で分岐と hook 数を検証する。

## 上限と privacy

先頭 Bytes の復元には [既存の 4 上限](bounded-unserialize-development.ja.md)を必須指定する。入力 bytes・累計 array entry・所有文字列 bytes・array depth は選択した 1 値だけへ適用し、製品の既定値を追加しない。比較 fixture ではそれぞれ 65,536／1,024／65,536／64 を明示する。

元 cache の確保、cache lookup、key／list 形状確認、空の default 返却、型 header／capacity、同時利用はこの decode 上限に含まない。RSS／peak memory／CPU／実行 deadline／共有 memory pool の上限ではなく、全 DB raw 値を常駐・decode する方針でもない。

`PreparedPostMetaCache` の Debug と `PostMetaCachedError` の診断は値を出さない。一方、返却する `PhpValue` と元 `PhpArray` の Debug は redacted ではないので、caller は decoded 値を private に扱う。完全 cache という caller の主張は、DB snapshot、principal、role、visibility、公開承認の証明ではない。通常公開／DB runtime の WP 依存境界も変更しない。

## 比較 wire と witness

`scripts/post_meta_cached_profile.py` が有限 corpus と共通 metadata を定義する。API 名は `get_post_meta`、profile 名は `complete_cached_single_empty_filters`。owner は既存 codec の typed positive int、meta_key は typed Bytes、cache は typed `PhpArray`。`single`・`complete_cache`・`empty_filters` はすべて true で、4 limits を full field name で必須指定する。native fixture はこれらの型と前提を検査し、任意の DB／cache miss／全 cache／multiple 経路へ進まない。

選択した先頭 Bytes が strict serialized と認識される場合だけ、別 field の typed witness を PHP admission に用意する。witness は Null／Bool／i64／Bytes／Array に限り、PHP の `serialize(witness)` が外側の PHP trim 後の raw Bytes と完全一致することを実関数の呼出し前に確認する。型・canonical key・深さと 4 上限も確認し、object／reference を無防備に unserialize してから拒否する順序にはしない。

witness は PHP の安全な比較 domain を限定する資料であり、actual 結果をそこから生成しない。native API は witness を必要とせず、native fixture も witness の意味を getter 結果へ使用しない。このため malformed witness の native／PHP 同一拒否を主張しない。未選択値・後続値の object 風 Bytes には witness を要求せず、それらの復元を試みない。

## 対象結果と正式確認

native の新 public 6 契約に加え、bounded 6、既存 serialization 11、lib 26 が成功した。WP 全 target Clippy と fmt も exit 0。PHP の事前 guard／scope 等 7 契約は `target/post-meta-oracle-contracts.log` で成功した。Python profile 4 契約と既存 evidence 34 契約も成功した。

`target/post-meta-cached-targeted-comparison.json` は、Cargo JSON が選ぶ現 compat probe artifact を使った実 PHP 3,202 件／4 batches の差分 0 を記録する。各所有 child の実 PID、artifact SHA、両結果 SHA を確認し、所有 process を回収、probe registry が空になったことと artifact 不変を確認した。実測 counter は次のとおり。

```json
{"calls":3202,"get_hook":3202,"default_hook":626,"cache_get":3202,"cache_update":0,"db_calls":0,"restored":3202}
```

counter は合計だけでなく case ごとの期待分岐を検査する。同じ結果の raw empty と `N;` で default hook 差が相殺されても成功としない。PHP oracle は pinned 実関数を呼び、関連 hook を空にした scope、cache spy／DB trap と 9 global の存在・値の復元を確認する。source metadata と counter の bool／integer 混同、未知 field も検証対象である。

既存 72 API の入力生成 482,951 件の順序・ID・入力 SHA を保持して新 3,202 件を末尾追加し、正式全比較は 73 API／486,153 件で成功した。旧 72 の件数・入力・PHP／Rust 両結果 prefix SHA、hooks／dispatcher の結果を `target/post-meta-cached-old72-preservation-check.log` で実照合し、完全保持を確認した。旧 68／70／71 prefix 検査も維持する。正式結果の新 API counter も get／cache／restored 各 3,202、default 626、update／DB 0 と一致した。

`target/post-meta-cached-full-check.log` は workspace Rust 554 件（ignore 1）＋独立 29 件、Python 335 件（329 成功・symlink 権限不足 6 skip）、Clippy、server／WP Linux cross-check、license 215 観測、HTTP／process smoke の成功を記録する。生成 10、legacy 113（actual 88）、credential stage 33（actual 25）も実再測定した。全 8 fresh DB profile が各 62 契約を通過し、専用 DB directory は回収後に空である。最終既存 11 checker は `target/post-meta-cached-current-check-summary.json` ですべて exit 0。

runtime manifest と通常公開／DB の依存接続は今回変更しておらず、WP を通常 runtime 依存へ追加していない。依存境界は直前の観測済み graph を source 不変で維持するもので、新しい graph 測定を追加実行した結果ではない。Linux cross-check を native Linux 実行へ、固定 cache の比較を actual DB cache population／principal／公開承認へ読み替えない。
