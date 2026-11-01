# NoLiteralLtのsanitize_text field／textarea

固定WordPress 7.1.3の `sanitize_text_field`（formatting.php:5656）、`sanitize_textarea_field`（5684）、内部 `_sanitize_text_fields`（5708）の限定移植をmainへ採用した。callerはraw Bytes、literal `<` 無し、固定UTF8 site、mbstring無し、当該own-name／`all` filter無しを明示する。文字列以外のPHP coercionやHTML分岐は対象外。

## native契約とsource順

`sanitize_text_field_utf8_site_no_open_angle_empty_filters_bytes(raw, limits)` と `sanitize_textarea_field_utf8_site_no_open_angle_empty_filters_bytes(raw, limits)` は所有bytesを返す。`SanitizeTextLimits::new(max_input_bytes, max_output_bytes, max_percent_rounds)` は3値必須でDefaultを持たない。closed errorは `UnsupportedValue`／`InputLimit`／`OutputLimit`／`PercentRoundLimit`／`AllocationFailed` で、入力bytesを保持せずsource errorを連結しない。

最初にraw入力長capを検査し、次にraw `<` があればprofile外として拒否する。その後whole UTF8を検査し、不正UTF8ならreplacement decodeせず空bytesを返す。この枝では入力サイズのbufferを確保しない。不正UTF8と `<` を併せ持つ入力もraw NoLt guardが先なのでprofile外となる。

valid入力は `try_reserve_exact` による入力サイズの1つのVecへコピーし、以後そのbuffer内で縮める。fieldモードだけがCR／LF／TAB／ASCII SPACEの連続をSPACEへ潰す。textareaはこの初回collapseを行わない。続いてPHP trim6（NUL／SPACE／TAB／CR／LF／VT）を両端へ適用する。内部NULやVTは残り、FFとNBSPは両端でもtrim対象ではない。

percent除去は最初のASCII `%HH` をcase-insensitiveなhex判定で探し、**その時選択した3bytesと完全一致するtoken**を全箇所削除する。例えば `%Ab` を選んだroundで `%aB` を同時には消さない。1roundの置換は元のread cursorを前へ進め、新たな連結でできたmatchを同roundでは再評価しない。次roundの先頭探索で新matchを検出する。これはURL decodeではなくtoken削除である。

round上限はoccurrence数ではなく、このglobal置換の回数を数える。上限0でもmatchが無ければ成功し、上限ちょうどの除去後にmatchが無ければ成功する。まだmatchがあれば `PercentRoundLimit`。除去が1回以上あった場合だけ最後にASCII SPACEの連続を潰し、trim6を再適用する。textareaの内部改行／TABはこの最終SPACE-only処理でも保持する。最終出力長capは全縮小処理の後に適用し、途中の長さを最終capで早期拒否しない。

処理は入力を変更せず、失敗時に部分結果を返さない。入力所有buffer、Vec capacity、allocator／RSS、共有pool、同時実行、CPU時間の上限は保証しない。round毎に再走査するため入力長とround数の両方が作業量へ関係する。各成功roundは最低3bytesを削るが、これはCPU期限の保証ではない。

## policyとの境界

HTML枝を実装せず、HTML安全性やURL安全性を返す型ではない。raw entity、`>`、内部NUL、Unicodeなどは選択した処理以外では保持する。DB読取／保存時の自動変換、workbench入力の自動sanitization、認証・principal・公開資格、filter登録policyを追加しない。新しい依存も追加しない。管理ready503とαの認証後編集・保存／theme公開／native Linux試用は未完了のままである。

## stage観測と採用後検証

`target/sanitize-text-draft/` のpure契約8群はstageで成功した。4095／4096bytesのtight familyでは、初回token除去で新しいmatchが連結され続け、1,365roundで成功・1,364で拒否する関係を含める。入力4KiBだからround数が少数になるとは仮定しない。native独立read-only reviewは `target/sanitize-text-native-review.txt` に記録し、source順とscopeにblocking無しとした。review時点のlogと追加直後のtest sourceの差はメモに保持する。

main受入後のnative8契約も成功し、1,365round tight familyを実行した（`target/sanitize-text-native-targeted.log`）。lib／test Clippy `-D warnings` はexit0（`target/sanitize-text-targeted-clippy.log`）。環境のhardlink copy fallback警告は残るため、全出力が警告ゼロだったとは扱わない。

PHP stage7契約とactual named sourceの有限5,722case（各API2,861）は成功した（`target/sanitize-text-oracle-draft/contracts.log`）。これはnative比較前のstage観測として保持する。

main採用後の限定actual比較は5,722case／12batchでPHPとRustが全一致した（`target/sanitize-text-targeted-comparison.{json,log}`）。calls／restored各5,722、field hook／textarea hook各2,861を実観測し、所有25 PIDを全回収、対象sourceとbinaryの前後SHA一致を確認した。probe Clippy `-D warnings` もexit0（`target/sanitize-text-probe-clippy.log`、環境warningは残る）。HTML枝や内部UTF8 helperの呼出し回数を計測したとは扱わない。

main PHP/profile9契約は成功した。初回mixed試験では旧 `is_utf8_charset` のraw boolをtyped Boolと取り違えた期待値不備があり、期待値だけを訂正した。初失敗は `target/sanitize-text-oracle-first-mixed-bool-fixture.log` に保持し、本番4fileはこの訂正で変更していない。

`target/sanitize-text-full-check.log` の全体gateはexit0で完了した。Rust610（ignore1）、native pure313（62 artifact／features空／normal依存WP1）、isolated binlog29、Python402（396成功・symlink6 skip）、正式79 API／519,014case・hooks572／dispatcher405、generation10／legacy113（actual88・refusal25）／credential stage33（actual25・refusal8）、全8 fresh DB各62、Clippy、WP／server Linux cross compile、license file観測218、実HTTPが成功した。全所有fixtureを回収した。Linux cross compileはLinux native実行ではない。

旧77の入力順・件数・両結果保持は `target/sanitize-text-old77-preservation-check.log` で確認した。最終11checkerはすべてexit0（`target/sanitize-text-current-check-summary.json`、controller1951も出力回収後exit0）。root／WP manifest、root／binlog lock、inventory／licenseの6fileは、現在SHA＝開始snapshot＝完了parser77 checkpointの各SHAという実assertに成功し、DB fixture directoryも空を確認した。sanitize checkpoint生成はdocs freeze後の別操作。今回はcore／server不変のためWasmpubを再実測せず、前回の既観測を保持した。

直前のparser77完成は `target/auth-cookie-parse-checkpoint.json` に実生成済みで、履歴として保持する。今回source差を含む対象証跡は実gateで再生成し、旧checkpointを現在sourceの成功へ読み替えない。auth-cookie生成／分解は別の限定実装で、ここからHMAC検証やsession接続へ進めたとは扱わない。次のsource guard候補はtarget stageのみで、mainへ未採用。
