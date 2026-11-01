# 事前生成CSS／JavaScriptのsealed配信

2026-10-30照合。本書の実行件数・source一致・最終11checkerは `target/sealed-assets-checkpoint.json` 保存時点の履歴である。後続の認証cookie検証MAIN変更後の全体再検証・source-bound証跡更新は未完了であり、現在tree全体の成功を示さない。V3 admission／配信の実装範囲と、その時点の観測を区別する。

固定UTF8のCSS／JavaScriptを、HTMLと同じstartup generationの準備済みpageとして受理・配信する。theme成果物にCSS／JSを含める方向へ進む小さなhost機能であり、Wasm engine、theme SDK、WP enqueue／依存解決、認証・公開承認policyを採用するものではない。公開要求からcompiler／PHP／DBを起動せず、明示bundle内のbytesだけを使う。

JavaScriptの実行先はbrowserであり、serverは準備済みbytesを配信する。D04のサーバー側テーマWasmとは別の資産である。CSS／JSの構文・安全性や、scriptから起こる通信を承認・制限する機能はない。

## constructorとMIME

`PublicPage::new_stylesheet(Vec<u8>)` と `new_javascript(Vec<u8>)` はstatus200固定・whole valid UTF8で、順に `text/css; charset=utf-8` と `text/javascript; charset=utf-8` のstatic参照を保持する。不正UTF8は `InvalidTextAssetEncoding`。bodyをtrim／改行変換／minify／URL decodeせず、BOM・NUL・comments等のvalid UTF8 bytesを保持する。言語構文の妥当性やsafe scriptを検証した型ではない。

従来 `PublicPage::new` のtext4種とstatus200／404／410、typed raster3種、redirectは維持する。旧constructorにCSS／JSやMIME alias・case違い・余分parameterを通す入口は追加しない。assetconstructorは404／410の任意statusを選ぶAPIではない。

[IANA text/css](https://www.iana.org/assignments/media-types/text/css)はcharsetを認め、UTF8 authoringとHTTP等によるencoding宣言を案内する。[IANA text/javascript](https://www.iana.org/assignments/media-types/text/javascript)と[RFC9239](https://www.rfc-editor.org/rfc/rfc9239.html)に従いJavaScriptの共通MIMEを `text/javascript` とし、historical aliasを独自に追加しない。HTTPのcharset parameterとHTML scriptの `type` 属性は別契約で、fixtureのscript属性へparameter付き文字列をそのまま使わない。[WHATWG script element](https://html.spec.whatwg.org/multipage/scripting.html#the-script-element)も参照。

## bundleとreportの互換境界

| 入力 | 本文・許可域 | report |
| --- | --- | --- |
| V1 | 従来JSON文字列body／text4 | schema1・従来固定7 MIME key |
| V2 | 従来hex body／text4・raster3・redirect | schema1・従来固定7 MIME key |
| V3 | V2同形のrequired hex body＋CSS／JS、既存redirect | schema2・固定9 MIME key |

V1/V2へCSS／JSを黙って追加せず、旧入力のadmissionとreport1のbytesを維持する。V3だけreport `format_version:2`／`accepted_bundle_version:3` を返す。top-levelは従来の6fieldのままで、MIME mapだけが従来7種＋CSS／JSの9種になる。0件keyも保持し、redirectは別count。旧report1を前提にするconsumerは新report2を明示的に扱う必要があり、未知keyを捨てて成功扱いしない。

既存8MiB wire＋sentinel、hex全体検査、decoded aggregate bound、予約／重複targetやstrict unknown-field拒否を維持する。V3のCSS／JSは200とvalid UTF8を要求し、失敗を旧textへfallbackしない。hexの2倍wireとJSON overheadがあるため、8MiBのasset fileを必ず受理できるという意味ではない。

receiptは引き続きschema1で、同一payload bytesの長さ／SHA256確認後に共通native admissionへ進む。reportがschema2でもreceipt版を上げない。`check-bundle`／`check-package`／fixed ZIP import／public loaderは共通admissionを用い、新CLIは追加しない。最終reportは内容を含まない6fieldでLF込み1,024bytes以内を維持する。

## 公開例

`examples/sealed-assets-bundle.json` と `examples/sealed-assets-receipt.json` は固定public例。rootで次を実行する。

```powershell
cargo run -p recastpress-server --bin recastpress --offline --locked -- check-package examples/sealed-assets-bundle.json examples/sealed-assets-receipt.json
cargo run -p recastpress-server --bin recastpress --offline --locked -- public --host localhost:8080 --bundle examples/sealed-assets-bundle.json --receipt examples/sealed-assets-receipt.json
```

`http://localhost:8080/asset-demo?view=1` を開く。HTMLは `/assets/demo.css?v=1` と `/assets/demo.js?v=1` を明示参照する。配信は既存sealed startup方式で、fileを後から変更してもlive反映しない。GET／HEAD／ETag・Range・Host／queryの既存profileとcache方針を維持し、asset探索・依存順序・外部取得・font配信を追加しない。

CSS `url()`／`@import`やJSのfetch／importは、作者が含めればbrowser側の通信を起こし得る。serverが参照先を取得しないことを、client codeの無通信・sandbox保証へ広げない。入力bodyの事前確保、Arc、JSON／hex decode、allocator／RSS、CPU、同時実行の全予算も保証しない。複数browser subrequestを同じ世代へ原子的に固定した機能ではない。

## 実観測と未完了範囲

main targetedはcore新4契約・Rust bundle6契約・Python CLI5群が成功し、core／server Clippyもexit0（`target/sealed-assets-{core-contracts,bundle-contracts,cli-contracts,clippy}.log`）。Python5群の単発20.123秒は性能保証ではない。実before／after observerの7入力はadmission／CLI stdout bytesが一致（`target/sealed-assets-legacy-{before,after}.{json,log}`）。旧report1の固定7種を現在9種へ書換えて互換と呼ばない。

例のreceiptは実native発行・check-package成功で、wire1,876／body767bytes、report2の9 MIME keyを確認した（`target/sealed-assets-example-observation.json`）。依存Wasmpubはnative6＋所有HTTPで成功し、`target/sealed-assets-wasm-{native,http}.log`／`target/sealed-assets-wasm-http-observation.json` に保持する。

実desktop browserではJavaScript loaded表示／marker `ready`、緑CSS card、console error空、client width＝scroll width1,280を確認した（`target/sealed-assets-browser-observation.json`、画像 `target/sealed-assets-browser-desktop.png`）。tab5を閉じ、所有preview controllerの実PIDは32212、exec session92160はexit0。public実PID19936はWindows強制停止exit1・reader回収済み（`target/sealed-assets-browser-lifecycle.json`）。session IDをOS PIDとは扱わない。これは固定例のdesktop観測で、mobile／一般theme／全JS安全性／native Linuxの成功ではない。

初回のguard付き全体gate（exec session2149）はDB実行前にexit101となった。旧 `binary_bundle_contract` がversion3をunsupportedのnegative controlへ残していたtest fixture不備で、`target/sealed-assets-first-unsupported-version-control.log` に保持した。negative versionだけを3から4へ訂正し、対象binary6契約は成功（`target/sealed-assets-binary-version-control.log`）。productionの制限緩和やcore／runtime変更はしていない。

2回目の全体gate（session51006）はPython436内のHTTP timeout fixtureで失敗した（`target/sealed-assets-second-http-startup-assumption.log`）。起動から0.5秒以内にGETが届くというtest前提を除き、GET待機を最大5秒で確認した後にreal monotonicでtimeoutを計測し、実 `TimeoutExpired` と0.5秒設定を検証するtest-only修正を行った。focused12は11成功・skip1（`target/sealed-assets-http-timeout-test-fix.log`）。production timeoutは変更せず、この失敗をcompiler負荷が原因だったと断定しない。

freeze対象はHTTP test sourceを含む13file（`target/sealed-assets-source-freeze-identities.json`）。3回目の全体retryはexit0で完了した（`target/sealed-assets-full-check.log`、exec session73123も出力回収済み）。Rust636（ignore1）＋isolated binlog29、Python436（429成功・skip7）、認証3種のactual比較、全8 fresh DB mode各62、Clippy、Linux cross compile、default依存inventory218、実HTTPが成功した。全所有DB fixtureを回収した。pure313とlicense file218は今回追加flagを実行せず、直前の既観測を保持した。native Linux実行の成功とは扱わない。

最終11checkerもすべてexit0（`target/sealed-assets-current-check-summary.json`、checker controllerも出力回収後exit0）。`target/sealed-assets-retained-identities.json` の実assertではbaseline23のうち13proof（DB10＋認証3）が変化し、残る10（WP3＋依存7）をbyte保持、source13不変とDB directory空を確認した。8DB modeを証跡JSONの個数と混同しない。

checkpoint用にgeneric観測をphase固有の `sealed-assets-full-native`／`sealed-assets-runtime-boundary`／`sealed-assets-workbench-native`／`sealed-assets-unpack-native` の記録へ保存した。docs freeze後に `target/sealed-assets-checkpoint.json` を生成した。先の2回の全体失敗とtarget coreの初WebP fixture不足も履歴保持する。後続の独立selected Wasm consumer採用は専用文書参照。本assets checkpointにその採用や本番SDKの成功は含めない。
