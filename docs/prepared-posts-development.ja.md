# 明示的に準備した記事textから既存bundleへの変換候補

後続実測（2026-10-29）：CLI接続と両DBからの固定snapshot受渡しを含む全体gateが`target/wp-bcrypt-and-alpha-publication-full-check.log`で成功した。固定サンプルと再生成／起動方法は[起動手順](running.ja.md)、8 fixture各45契約からsealed HTTPまでの制限は[α経路](alpha-publication-development.ja.md)を参照。以下のlibrary単体対象検証に続く結果であり、theme／認証／live公開の完成ではない。

2026-10-29（日本時間）。`recastpress_server::prepared_posts` は、callerが匿名公開用に選んだtextを固定layoutへ入れ、既存版1bundleへ変換するpureな候補である。DB読取り・記事選別・保存・公開を行わず、ファイル、ネットワーク、listener、publication channel、WordPress／PHP／Wasmにもアクセスしない。Wasmテーマの標準をRust組込みへ変更したものではない。

## 入力と選別の境界

`PreparedPost` は `target / title / body_text` の3つのStringだけを持つ。raw `PostRow` からの変換や、status・password・author・slug・post type・private metaの自動解釈を提供しない。callerが公開用の内容を明示的に準備することが前提であり、本文内の個人情報や秘密をこのrendererが発見・除去するわけではない。

実験用JSON入力schema1は次の形で、全fieldを必須とする。

```json
{"format_version":1,"posts":[{"target":"/chosen","title":"記事タイトル","body_text":"公開用に準備した本文"}]}
```

`parse_input(bytes)` は8MiBを含む範囲だけを受理し、超過は解析前に拒否する。未知／重複field、欠落、null、型不一致、他のversion、不正UTF-8を拒否する。raw編集fieldを追加しても取込み範囲を拡張しない。CLI等の入力readerには別途bounded readが必要であり、このmoduleはファイルを開かない。

native APIは `render_post(post,max_body_bytes)`、`export_bundle(posts,max_body_bytes,max_bundle_bytes)`、JSON入力から両段階を合成する `prepare_bundle(input,max_body_bytes,max_bundle_bytes)`。typed exporterに渡すcaller所有のStringやslice全体のheap／時間上限を意味しない。`PreparedPost`のDebugと全errorのDebug／Displayは、入力内容・target・path・私的値を表示しない。

## 固定layoutとliteral text

固定HTML shellはUTF-8 metadata、title、h1、body用divだけを含む。dataの `& < > " '` を参照へescapeし、元の `&copy;` や `&#13;` もliteral textとして残す。HTML、PHP、shortcode、block、template、JSとして本文を解釈しない。

bodyは `white-space:pre-wrap` のdivへ置き、pre開始直後のLF除去を避ける。入力CRは `&#13;` へ変換し、HTML入力のCR/LF正規化で値を変えない。LF、TAB、Unicodeは保持する。NULはHTML parserが置き換えるためtitle／bodyのいずれでも拒否する。空titleと空bodyは許可し、WordPressの投稿受付policyやfilterを推定しない。一般的なHTML validity、sanitization、PII対策、テーマ互換を保証するものではない。

## cap・JSON・既存admission

HTMLは同じtraversalをcount／emitで使い、escape後の完成body長をcheckedで計測してからfallible reserveする。callerのbody capに加え8MiBのhard ceilingを適用する。

版1bundleはcaller順のpagesを持ち、固定status200と `text/html; charset=utf-8` を出力する。JSON文字列escapeと末尾LFまで含め、同じSerialize処理をbounded writerのcount／emitで走らせる。caller wire capと既存8MiB boundの両方を守る。JSONのoverheadをbody長と混同しない。途中prefixや部分bundleを返さない。

生成後は必ず既存 `bundle::parse` を通す。duplicate target、予約health route、invalid target、empty site等の受理条件を共有し、独自の緩い公開経路を作らない。targetのpercent encoding／query順序はliteralのまま保持する。slug／permalinkの生成、redirect、公開選別や公開操作は行わない。

入力、render中のString／Vec、page descriptors、JSONとadmissionの検査用コピーは未課金の一時領域である。完成byte長のcapはcapacity・allocator rounding・peak RSS・共有poolや自動容量の上限ではない。aggregate bodyの必要量と固定shellの下限を使い、出力に適合し得ない巨大record数／bodyを不要に保持しない。

## 対象検証

`target/prepared-posts-contract-check.log`でRust6契約が成功した。入力の未知private field、重複／欠落／null／型、Debug非露出、inputの8MiB境界、5char escape、Unicode、CR／LF、input不変、NUL拒否、escape後exact cap／1byte不足、JSON overheadとLF、duplicate／reserved／invalid route、empty site、完成wireのhard8MiB／1byte超過を確認した。

`prepared_posts_probe`の実出力は、独立したPython HTMLParser2契約でtitle／h1／bodyのliteral text、先頭LF・CRLF、scriptがelementにならないこと、empty fields、既存bundle形状へroundtripすることを検査した。結果は `target/prepared-posts-html-parser-check.log`。probeは固定の公開準備済み値だけを使い、DBや実サイトの公開判断はしない。

`target/prepared-posts-clippy-check.log`と`target/prepared-posts-linux-cross-check.log`でserver全targetのClippy／Linux向けcross-checkが成功した。新3Rust fileのrustfmtも確認済み。native Linux実行、全workspace gate、認証・記事保存・DB fixture・公開世代切替の実証はこの記録には含めない。Cargo.lockや既存loader／feed挙動は変更していない。
