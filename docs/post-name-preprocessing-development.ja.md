# 投稿名preprocessからtext lookupへの有限関係

全体checkpoint（2026-10-29）：`target/truncate-slug-and-post-name-pipeline-full-check.log` exit0。全8 fresh DB fixture各61でこのdev-only関係を実行し、元4posts／6metaの完全復旧も確認した。Rust536＋独立29（ignore1）、Python254（1skip）、正式72 API482,951、Clippy・server/WP Linux check・license215観測・実HTTPが成功。最終11checker全current、`target/truncate-slug-current-check-summary.json`、全fixture回収済。URL resolver・公開許可・slug一意性・native Linux実行の完成ではない。

新check `post_name_preprocess_lookup_pipeline`は、既存のouter `sanitize_title`候補とediting-side `posts_by_name_text_page`を、DB開発用test controllerだけで接続する。既存60項目へ1項目を追加し総61項目とする。production reader、WP library、server、normal dependency、Cargo manifestは変更しない。WP呼出しは既存DB dev-dependencyの範囲。

呼出し位置は`post_name_lookup_contract`の直後、`meta_view_database_tests`より前。初期meta 5行＋ID0の6行がbaselineであり、後続meta-viewが5行を加えた後の11行とは区別する。元4posts／6postmetaを全rawで保存し、未使用の専用post ID1001・1002だけをcontrollerが作成する。最後に両行を削除し、元行集合との全field Eqを確認する。任意の既存DBを更新する機能ではない。

## 明示Query contextの関係

public dummy raw name `Ä Name / Part`を、callerが明示した`SanitizeTitleContext::Query`、locale `en_US`、空fallbackで処理する。NoLiteral<・whole valid UTF-8・no-mbstring／no-Intl・固定default callbackの限定profileで、native結果は`%c3%84-name-part`。同じrawをSaveへ渡すとaccent除去が先に走り`a-name-part`となる。これらの厳密な期待は実native probeでも確認し、fixture内でも再確認する。

ID1001はnameを`%C3%84-NAME-PART`、typeをcaller明示の`bridge`、statusを`private`として保存する。Query結果のUTF-8を`str::from_utf8`で検査してから、既存readerのname／type text bindへ渡し、固定utf8mb4_unicode_ciで1001を取得する。hex文字のcase差もSQLのtext比較に委ね、Rustでbinary key照合や独自lowercaseを重ねない。全23fieldを既存page readerが取得したraw行と比較する。

raw nameを直接queryした結果と、Save結果でqueryした結果はともに空であることを確認する。この関係はcallerがQueryを選択した有限例であり、一般のrouteから投稿を解決するAPIではない。raw titleから公開URLを生成したり、QueryとSaveのどちらを利用者に適用すべきかを決めたりしない。

## fallbackとUTF-8 host境界

primary `!`のcallback結果は空になるため、明示fallback `RAW`＋NUL＋`<unapproved>`を無加工で選ぶ。これはUTF-8として有効なので、hostの`str::from_utf8`を通してそのままtext queryへ渡せる。ID1002はこのraw name、type `bridge`、status `plugin_review`を持ち、全23fieldが一致して返る。NULやliteral `<`を含むfallbackが取得できることは、URL安全性・HTML安全性・公開承認を意味しない。

別のfallbackは先頭byte `ff`を含む。nativeは任意Bytes fallbackとしてそのまま返すが、hostの`str::from_utf8`は拒否することを確認し、SQL lookupを呼ばない。lossy Stringへ変換する経路を作らない。primaryがNoLiteral<／whole UTF-8 domainの外にある場合は`UnsupportedValue`のままで、fallbackの成功へ変換せず、その値でもSQLへ進めない。

## DB・WordPressのscope

stageは既存READ ONLY snapshotを使い、preprocessとlookupの間にDB writeを行わない。stage前後の全posts／metaを同じsnapshotで比較し、さらにfresh snapshotでも専用行を含む6posts／6metaが全raw一致することを確認する。専用行の作成・削除だけは明示controllerの別connectionで行う。privateとcustom status、password、author／parent、zero date、負のmenu_order／comment_count等をrawに保持し、statusで公開選別しない。

outer kernelの固定callback返値は別の実PHP比較profileで検証する。このDB関係checkは実PHPやWP_Queryを起動せず、native preprocessと実SQL readerを結ぶ。WP_Queryのrewrite、`sanitize_title_for_query`呼出し条件、status／parent／post-type優先、cache、権限、unique slug、canonical URL、HTTP router、DB INSERT／UPDATEや公開処理の再現ではない。特定typeを既定値として採用する製品policyも追加しない。

新checkの実MariaDB／MySQL・全8fixtureはrootの後続gateで測定する。担当作業では実DBを起動せず、既存proofを更新しない。native probeの2例確認やcompile・Clippyを、実DB関係／全体gate成功へ読み替えない。

2026-10-29の担当確認では、既存native probeでQuery／Saveの2結果を確認し、新しいignored DB契約を含むlibtestの`--no-run --offline --locked` compile、DB全targetのClippy（`-D warnings`）、担当fileのrustfmtが成功した。ここには実DBでの新check成功を含めない。
