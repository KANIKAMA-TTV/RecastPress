# 明示manifestのtyped redirect

2026-10-27時点。公開用manifestで事前に確定したroot-relative Locationを返す候補を実装した。公開要求に応じたURL生成、転送先の取得、PHP／DB／job起動は行わない。既存の公開世代・Host検査・管理process分離を維持し、WordPressのwp_redirect／wp_safe_redirect、canonical rule、filterやSSO認可の互換実装とは区別する。

## 型と受入範囲

`OriginFormLocation`はprivateなArc strを持ち、検証後の値を借用で参照する。`RedirectStatus`は301／302／303／307／308だけを表す。`PublicPage::new_redirect`だけがLocationを持つ応答を作り、本文を空、Content-Typeを固定`text/plain; charset=utf-8`にする。従来のtext／feedとrasterのconstructorはstatus200／404／410のままで、Locationを持たない。

Locationの受入範囲は次のprototype subsetである。

- 8,192bytes以下のASCIIで、先頭は`/`、先頭`//`を拒否する。
- RFC3986のpchar相当、slash、queryの`?`を受理する。percentは完全なASCII hex2桁を必要とする。
- raw backslash、C0、space、DEL、`#`、非ASCII、quoteやangle bracket等の範囲外文字を拒否する。
- percentで表したcontrol／space／DEL／slash／backslashも、この候補のhost制約として拒否する。
- trim、URLdecode、再escape、dot segment整理、queryの並替え、hostやschemeの補完を行わない。受理したbyte表現はそのまま返す。

pcharの根拠は[RFC3986](https://www.rfc-editor.org/rfc/rfc3986.html#section-3.3)。encoded slash等の拒否は本候補の狭いhost domainであり、RFC全体が禁止するとの主張ではない。既存の公開target validatorを変更せず、Location用に別の型で検証する。

root-relativeの構文は、任意のproxy／browserによる追加decodeや、遷移先applicationの処理を証明しない。特にspecial URLでのbackslashの扱いにはauthority分岐があるため、literalとencodedの両方を候補で拒否する。[WHATWG URLのrelative slash state](https://url.spec.whatwg.org/#relative-slash-state)。URI全体の一般的な安全性判定や、匿名入力からredirectを自動作成するAPIではない。

## Bundleと配信

版2だけに、必須の`target`／`status`／`location`を持つredirect recordを追加した。本文recordは従来の必須`target`／`status`／`content_type`／`body_hex`である。両required構造はunknown fieldを拒否し、untagged variantのraw duplicate／Null／両shape混在も契約試験で拒否を確認する。generic mapでkeyを潰さず元JSONからtyped parseする。版1と通常の本文recordで3xxを許可する変更はない。

公開adapterはmethod・Host・origin-form request・公開routeを先に検査し、GET／HEADへLocationを一つ付ける。両methodとも空body・Content-Length0・固定MIMEで、no-store／nosniffを維持する。POST等は従来405、wrong Hostは421、未登録routeは404となり、これらへLocationを付けない。

redirectはcache.bodyを通さない。pool0でも通常poolでもredirect前後のcache状態・全BudgetStatsを変えず、隣の通常本文routeは従来のcache／lease契約を保つ。LocationのArcやPreparedSite等のmetadataは共有pool／RSS計上を完成したものではない。

HTTPは転送先を内部でfollowしない。Locationが未登録のlocal targetでも応答指示として扱い、clientが次にそのtargetを要求すると通常404となる。自己redirect／多段loopの検出や、遷移先の到達性はこの候補で検証していない。static graphの確認はexporter側の残件である。

Locationは単一URI-referenceであり、相対参照はrequestのtarget URIを基準に解決される。Locationにfragmentがない場合も、clientが元のfragmentを継承するHTTP挙動は残る。サーバーはrequest fragmentを受信していないため、本候補の`#`拒否をfragment消去の保証とは扱わない。[RFC9110 Location](https://www.rfc-editor.org/rfc/rfc9110.html#name-location)。GET／HEAD限定のため、3xxによるPOST保持・変更のclient挙動は実証していない。

## 検証

core新3契約とserver新5契約で、全5status、旧constructor拒否、8,192byte境界、hostile URI／header表現、percent subset、入力保持、公開世代の差替え／撤回、required recordの不正matrix、GET／HEAD、cache非利用を確認した。process smokeにも正常308／302、未登録destination、POST405と、不正Location／status／混在fieldのlisten前拒否を追加した。

サンプル[public-binary-bundle.json](../examples/public-binary-bundle.json)の`/old-pixel`は`/pixel.png`への308を明示する。既存の1pixel画像bytesとユーザーassets/logoは変更していない。target/kses-slashes-and-typed-redirect-full-check.logでworkspace Rust344件（通常ignore1）＋独立27件・Python112件、全5 DB fixture各39項目、Clippy・Linux cross-check・license208観測・実HTTPが成功し、全fixtureを停止・回収した。core変更後の全native証跡を実測で再生成。固定PHP57 API・378,707ケース／hooks572／dispatcher405は別の正式比較ログで成功し、current provenanceも最終照合した。サンプル3routeのGET／HEADと308 Locationを所有processで別途照合し、回収した。新しい第三者依存は追加していない。
