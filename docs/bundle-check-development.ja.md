# One-shot bundle admission の開発CLI

後続の[sealed CSS／JavaScript](sealed-public-assets-development.ja.md)でV3／report2（九MIME key）を追加した。V1/V2／report1の七MIME・CLI bytesは実before／afterで保持確認済み。以下の導入時schema1／v1-v2説明・checkpointは履歴として残す。新しい固定例は `examples/sealed-assets-bundle.json`。

receipt の byte identity も同時に確認する場合は、新しい [`check-package <bundle> <receipt>`](package-check-development.ja.md) を使う。既存 `check-bundle` の挙動は維持する。

全workspace確認は`target/specialchars-and-bundle-check-workspace-check.log`で成功した。新Rust3・実CLI4を含むworkspace411 Rust／Python149、Clippy・Linux all-target cross-check・license208・実HTTP・DB current hashを確認。Unix非UTF8 path契約はcrosscompileだけで、native Linux実行とはしない。WPの別変更は実PHP61 API／432,780比較・hooks572／dispatcher405で再生成した。全native current hashも別途通過し、DB live再実行無しで独立29件・6 fixture各39項目を維持する。以下の全gate予定は履歴である。

固定sampleでの実行結果は`target/bundle-check-example-observations.json`。記事feed版1は3 pages／2,800 body bytes、binary版2は3 pages／1 redirect／327 body bytes、コメントfeed版1は3 pages／3,328 body bytesで、独立計算と一致した。各commandはexit0で終了・回収済み。MIMEは0件を含む固定7キーを返すため、非zeroキーだけのmapではない。全workspace gateは後続のspecialchars追加と合わせて実施する。

2026-10-27。`recastpress check-bundle <path>` は、既存のv1／v2 strict bundle loaderへ入力を通し、成功時だけ本文を含まない開発用JSONを1行返す。listener、router、shutdown handler、DB、PHP、source fetch、publication channelは起動しない。既存 `public`／`management` のRole・Config・parseとserve診断を維持し、`--help` へこのcommandの行だけを追加する。

## 実行とreport

例: `cargo run -p recastpress-server --bin recastpress --offline --locked -- check-bundle examples/feed-bundle.json`。

新commandはちょうどcommand名とpathの2引数だけを受ける。`args_os` で先に分岐し、native pathをStringへ変換せず借用する。serve側は従来のString引数処理を維持する。Unixの非UTF-8 path保持にはcfgUnix単体契約を用意するが、現在Windows上では未実行で、Linux crosscheckでのコンパイル確認と区別する。

成功時はexit0、stderr空、末尾LFを含め1024バイト以内のJSON1行。reportは次の項目だけを持つ。

- `format_version`: report schemaの実験版1。入力bundleの版とは別。
- `accepted_bundle_version`: 実際に受理したbundleの1または2。
- `pages`: redirectを含む全page件数。
- `redirects`: 明示redirect件数。
- `body_bytes`: decoded body長の合計。UTF-8は文字数でなくbytes、hexはwire長でなくdecode後のbytes。redirectは0。
- `mime_counts`: body pageだけの固定7種MIME別件数。text/html・text/plain・RSS・Atom・PNG・JPEG・WebPを常に列挙し、0件も返す。redirectはこの内訳へ含めず、内訳合計＋redirectsがpagesに対応する。

target、Location、入力path、raw body、body hash等はreportへ出さない。全件数と合計長はchecked加算する。固定MIME labelを使い、入力から自由なキーを追加しない。未知のmetadataを配信契約へ採用するための出力ではない。

引数不正、読取り・JSON・admission失敗時はexit2、stdout空、stderrは固定 `bundle check failed` とLFだけとする。serdeやOSの詳細は入力文字列を含む可能性があるため、新commandからは表示しない。これは通常のadmission失敗に対する契約であり、stdout/stderr自体のOS書込み障害時の完全送信を保証するものではない。

## Loader共用と未対応範囲

`bundle::load`／`parse` と `bundle::check` は、同じ私的Admissionを用いる。v1／v2が構築する `(target, PublicPage)` のVecをborrowして集計し、`PreparedSite::new` による非空・target形式・duplicate等の最終検証に成功した後にだけreportを外へ返す。検証用にpublish／router構築を行わない。共有admissionは検証のため実際に入力・decoded body・PreparedSiteを割り当てるため、reportが小さいことはRSSやpeak memoryの上限を意味しない。

従来の8MiB入力上限と1byte sentinel読取り、v1 UTF-8本文、v2 complete hex admission／decoded aggregate／text UTF-8／raster signature／origin-form redirect Location、reserved health route・重複field拒否をそのまま用いる。公開選別や製品policyを別途推定しない。[bundle運用の既存範囲](running.ja.md)

この成功は既存loaderの受理結果である。XML全構文、画像の完全decode／CRC／metadata除去、redirect graph、parent関係、target到達性、公開承認・PII除去、外部URL取得、Wasm package、連続公開・本番配備の承認を意味しない。新bundle formatや許可MIMEは追加しない。

## 対象検証

新Rust loader契約3件はv1 Unicode bytes／v2 raster・binary・redirectの集計、固定MIME・内容非保持、既存loaderと同じ拒否、exact8MiB成功と1byte超過失敗を確認する。従来input2件・binary6件・redirect5件も再実行する。

新Python実CLI契約4件は、`cargo build --bin recastpress --message-format=json --offline --locked` が返す現在のexecutableを必ず使用する。所有childを有限timeoutで回収し、v1／v2正常report、MIME／decoded bytes、重要admission拒否、unknown／duplicate field、secret markerを含む本文・path・Location・不正値がstdout／stderrへ漏れないこと、引数混在／missing file、旧role診断とhelpを検査する。fixtureはworkspace target内の一時directoryで回収する。

対象Rust・Python、server全targets Clippy、Linux全targets crosscheckを検証範囲とする。native Linux実行や全体gateの成功を対象試験から先取りしない。Cargo.lock、WP／DB／native experiment、既存公開経路、cache停止条件、全体check scriptは変更しない。
