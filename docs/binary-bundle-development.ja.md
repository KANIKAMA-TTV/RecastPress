# 公開binary bundleの開発記録

2026-10-27時点。起動前に全本文を確定する実験用bundleへ版2を追加した。既存の版1、公開／管理process分離、起動前Publisher破棄、通常CLIのcache停止、PHP／DB非到達を維持する。これは将来のWasm成果物や本番の画像移行・公開判定の採用ではない。

## 形式と入力の検査

版1は従来の必須`body: String`とtext／feed4MIMEのまま。版2の本文pageは必須の`target`、`status`、`content_type`、`body_hex: String`を持つ。textも画像も同じhex表現で、`body`との混在、欠落、Null、余分なfieldは拒否する。追加のredirect recordは別のrequired構造であり、`target`、`status`、`location`だけを持つ。[typed redirectの範囲](typed-redirect-development.ja.md)。本文recordとのfield混在は受理しない。

version選択でgeneric JSON mapへ変換すると重複keyが消えるため、`VersionEnvelope`をstrict typed parseし、同じ元bytesを版別のrequired typed structへ再parseする。topとpageの未知field・重複fieldを拒否し、負数／float／bool／Nullの版番号を受理しない。

入力は従来どおりUTF-8 JSON全体8MiBまで、file読取りは上限＋1byte。版2は全bodyのASCII hex・偶数桁を検査し、decoded長の合計をchecked加算して入力長／8MiB以下と確認した後だけ、decoded本文を確保する。空文字は0bytes、大小hexは同じbyteとなる。prefix、空白、全角文字、奇数桁を許可しない。text／feed4MIMEは復元後のUTF-8も検査する。

JSON入力、serdeのString、decoded Vec、Arc変換が同時に存在し得る。上限はbundle wire／decoded本文の検査であり、peak／RSS、allocator capacity、共有pool全体の保証ではない。入力bundleは明示CLI file一つで、page内のpath、url、source、ref等から追加ファイルや外部通信を取得しない。

## Rasterの宣言MIMEと署名

coreの既存 `PublicPage::new` は従来のtext／feedだけを受理する。新しい `PublicPage::new_image(status, RasterImageKind, bytes)` がPNG／JPEG／WebPのtyped宣言を扱い、対応する固定署名を照合する。status200／404／410、exact target／query、重複target拒否、予約health route拒否は同じである。

| 宣言MIME | 照合する署名 |
| --- | --- |
| `image/png` | `89 50 4E 47 0D 0A 1A 0A` |
| `image/jpeg` | `FF D8 FF` |
| `image/webp` | `RIFF`＋任意4bytes＋`WEBPVP` |

先頭の空白等を飛ばさず、宣言した種類を別MIMEへ自動変更しない。根拠は[WHATWGのimage type pattern](https://mimesniff.spec.whatwg.org/#matching-an-image-type-pattern)。PNGの8byte署名は[W3C PNG仕様](https://www.w3.org/TR/png-3/#5PNG-file-signature)にも記載される。WebPの完全なRIFF／chunk構造は[公式container仕様](https://developers.google.com/speed/webp/docs/riff_container)にあるが、この候補は完全parserを実装していない。

これは完全な画像decode、chunk／CRC／寸法／RIFF長／pixelの検証、metadata除去、秘密や非公開内容の判定ではない。署名だけの壊れた画像も一致し得る。作者が公開可能と確認したbytesを明示的に束へ入れるprototypeであり、本番の画像検証・無害化pipelineを完了したとは扱わない。SVG、GIF、CSS／JS等の新MIMEは追加しない。

## 配信と所有権

PublicPageは既存のArc byte本文を保持し、GETはそのbytes、HEADは空bodyとdecoded byte長を返す。全応答のno-store／nosniff、Host照合、query完全一致、存在しないtargetの404を維持する。HTTP/router/cache本体の制御を画像用に変更していない。

内部のbudgeted cacheも既存のpaired owner／Bytes契約を使う。HEADはcacheへ本文を保存しない。purge後も未poll Bodyやcollected Bytesのclone／sliceが残れば予約を保持し、最終owner破棄で返す。pool拒否では従来どおり未課金のsealed本文copyへfallbackする。画像decode／resize、Range、Content-Encoding交渉、外部取得、live公開は追加しない。

## サンプルと検証

[examples/public-binary-bundle.json](../examples/public-binary-bundle.json)はhomepageと固定1pixel PNG、追加の`/old-pixel`から`/pixel.png`への308を持つ版2の明示manifest。PNGは新規の70byte container fixtureで、ユーザーassetsやlogoは変更していない。

coreの新2契約とserverの新6契約で、legacyの受理範囲、MIME／署名違い、全byte保持、不正JSON／hex／UTF-8、8MiB境界、GET／HEAD、budgeted cache owner寿命とrefusalを確認した。全byteをPNG署名へ連結した一部test inputは署名fixtureであり、有効なpixel画像とは主張しない。

process smokeへ版2の実HTTP配信と、invalid hex／署名違い／invalid UTF-8のlisten前拒否を追加した。target/no-null-and-binary-bundle-full-check.logでworkspace Rust332件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実process HTTPを通過。core source変更後の全native証跡を実測再生成し、全fixtureを回収した。WP56 API・374,343ケース／hooks572／dispatcher405は別の正式PHP比較ログで成功し、current provenanceも最終照合した。サンプルPNGの全chunk／CRC／IHDR／zlib scanlineと、実processからのサンプルGET／HEAD bytes・MIME・長さも別途照合した。新しい第三者依存は追加していない。
