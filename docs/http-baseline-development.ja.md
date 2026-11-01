# 明示URLの手動HTTP baseline候補

`python scripts/capture_http_baseline.py <http-or-https-url> <new-dir>`は、呼出し側が明示した単一URLへGETを1回行い、その応答をローカルの新規ディレクトリへ記録する実験用CLI。R54に向けた手動のHTTP観測用入口であり、自動URL抽出、巡回、redirect追跡、画面撮影、HTML／JavaScriptの実行、移行・表示比較の合否判定を行わない。このツールの追加だけで元WordPressや外部siteを取得した実績とはしない。

## 起動と成果物

既存の開発用公開serverを`--host localhost:8080`で起動した場合の例。`target`は既存、出力先は未作成であること。

```powershell
python scripts/capture_http_baseline.py "http://localhost:8080/prepared-post" target/http-baseline-example
```

成功時はexit 0、標準出力`HTTP baseline recorded`と改行、標準エラー出力は空。入力検証・取得・保存が失敗した場合はexit 2、標準出力は空、標準エラー出力は`HTTP baseline capture failed`と改行だけを返す。URL・path・本文・peer診断をerrorへ含めない。完成後の成功メッセージ自体が部分的に書かれて失敗した場合は、その出力を撤回できず、完成した成果物も保持する。標準エラー出力の配送成功は出力先の状態にも依存する。

出力は`body.bin`と`capture.json`の2ファイル。bodyはHTTP transfer framingを除いたentity bytesであり、gzip等のContent-Encodingをdecodeしない。TLS record、status line、元headerのwire bytes、chunk境界、trailerを丸ごと残すraw network captureではない。`capture.json`の版1は次を記録する。

- `scope: manual_single_url_http_baseline`、明示`url`、実`request_target`、`method: GET`
- 最終`status`、`http_version`、`recorded_at_utc`
- `transport`と`tls_verification`（HTTPは`not_applicable`、HTTPSは`default_verified`）
- 選択headerのlowercase名から値のlistへのmapping。重複は元順で残し、不在headerは省略
- `body_file: body.bin`、実`body_bytes`、`body_sha256`

選択headerはContent-Type、Content-Encoding、Content-Length、Cache-Control、ETag、Last-Modified、Location、Varyの8種類。Set-Cookie、Authorization等の他headerは保存しない。ただしURLのquery、Location、本文、選択headerに私情報が含まれる可能性があり、成果物はprivateなローカル資料として扱う。秘密情報を除去済み・公開承認済みのbundleとはしない。SHA-256はbodyのbyte identityで、署名・相手の真正性・最新版・移行成功の証明ではない。

## 取得の有限profile

URLは最大8,192 ASCII bytesの明示HTTP／HTTPS subset。userinfo、fragment、制御文字／未encodeのspace、非ASCII URL、不正authority／port等を拒否する。browser相当のURL補正はせず、queryの順序・percent spellingと末尾の空`?`をrequest targetへ維持する。DNS名が元siteか、公開対象か、適切な取得先かを自動判定する機能ではない。callerが取得先を選ぶ。

GET以外のmethod、認証・Cookie、環境proxy、redirect、retryを使わない。3xxはLocationを含む応答として記録し、その転送先を取得しない。4xx／5xxも取得できた応答として記録し、移行先の正常性に読み替えない。HTTPSはPythonの既定verified contextを用い、default CA trust設定を維持する。検証失敗時のinsecure fallbackは無い。worker環境から`SSLKEYLOGFILE`を除去し、TLS keylog生成へ接続しない。既定CA環境やローカルtool自体は信頼するhost設定に含む。

受理する最終statusは200～599、versionはHTTP/1.0／HTTP/1.1。bodyは8 MiB、完成metadata JSONは64 KiB。status／header／chunk-size／trailerのcontrol lineは合計64 KiBという別予算で読む。Content-Lengthは単一のdecimal、Transfer-Encodingは単一のchunkedだけに限定し、両者の混在、途中EOF、不完全header／trailer、壊れたchunk-size／data CRLF、上限超過を成功品へ変換しない。Content-Length／chunkedの完了後に余分なEOF待ちを加えない。close-delimited HTTPSの不完全TLS終了も成功としない。この限定profileは一般HTTP全体のvalidatorではない。

取得には[Python 3.13 http.client](https://docs.python.org/3.13/library/http.client.html)を使う。socket timeoutとは別にworker全体のdeadlineを設ける。TLS既定検証とkeylog環境変数の挙動は[ssl.create_default_context](https://docs.python.org/3.13/library/ssl.html#ssl.create_default_context)、本文完了とTLS途中終了の区別は[RFC 9112](https://www.rfc-editor.org/rfc/rfc9112.html#name-message-body-length)を参照した。204／304はbody無しのstatus semanticsを使うが、204のContent-Length等も含む全field適法性を検証するtoolではない。stdlibが受理するfolded header値は、選択対象であればそのparser表現をprivate JSONへ保存する。

## worker・保存・失敗

core APIは`http_baseline.capture_http_baseline(url, Path(new_dir), timeout=20.0)`。DNS、connect、TLS、header、body、metadata、writeを所有workerで処理し、親は既定20秒のdeadlineと有限cleanupでそのworkerを回収する。試験では短いtimeoutも指定する。deadlineはOS scheduling、filesystem stall、kill権限失敗時まで含む厳密な終了SLAではなく、cleanup未確認を成功にしない。任意の既存serverや他人のPIDを停止しない。

不正URLと既存の出力dir／file／symlinkはnetwork前に拒否する。親dirは事前に必要。body／metadataの完成bufferを検証してから新dirを作り、固定2ファイルをcreate-newでwrite・flush・fsync・closeする。既存出力先は空dirでも上書きせず、成功品の再実行先にも使わない。

途中write／sync／close failureやworker停止で、不完全dir・片方のfileが残る場合がある。自動削除せず、dirの存在だけで完成を判定しない。atomic pair、親dirの耐久性、電源断復旧、敵対的な親path変更への隔離を保証しない。完成body byte上限もRSS／peak・全allocation・共有memory・CPUの上限ではない。

## 所有loopback試験

`python scripts/test_http_baseline.py`はpublic canaryだけを使う所有ThreadingHTTPServerをloopbackのOS割当portへ起動し、実CLI／workerを呼ぶ。外部DNSやsite、元WP、DB、Cargo、正式proofは使わない。各fixtureのserver・request threadを有限stop／joinし、owned workerをreapしてからtemp dirを回収する。

binary／gzip entity bytes、empty・204・304・4xx／5xx、3xx非追跡、selected header重複と除外、proxy環境を使わないGET／Cookie・Authorization不送信、末尾空queryの保持を確認する。Content-Length・chunked・close-delimited各bodyの8 MiB exact／超過、header／chunk／trailer EOF・framing／control budget・metadata上限、短いworker timeoutと実captured handleの回収、URL／引数のopaque拒否、既存path保全を対象とする。private `_write_file` seamによる2番目writeの決定的失敗では、worker失敗・receipt無し・partial dir保持を検査し、不安定なfilesystem raceを誘発しない。

TLS試験は既設OpenSSLから専用の一時CA／localhost leafを作り、default verified成功、未知CA・hostname不一致拒否、SSLKEYLOGFILE不生成を実通信で確認する。既存DB証明書は参照しない。OpenSSLが無い場合だけTLS試験を明示skipし、tool実行失敗を欠如skipへ読み替えない。Windowsのsymlink作成権限が無い場合も、その試験だけskipとして残す。実行結果は確認後に追記し、この文書だけで試験成功を先取りしない。

2026-10-29のWindows限定実行は`target/http-baseline-owned-cli-tests.log`に記録し、11試験中10成功・symlink作成権限不足による1 skip、suite exit 0を確認した。既設OpenSSLを使う実TLSはskipせず、verified localhost成功、未知CA／hostname不一致・close-delimited abrupt TLS拒否、keylog不生成まで成功した。所有HTTP fixture／request thread・workerを回収後、一時資料を削除した。sandbox内のloopback拒否・OpenSSL起動制約を、この許可環境での実測結果へ混ぜない。外部site・元WPの取得、Linux実行、全体gate成功や一般HTTP互換を含む結果ではない。

bodyless 204／304にTE:chunkedがある場合のcore修正後は、terminal chunk無しのheader-only応答を既status群へ追加し、Latin-1 `ff` 12,000 byteの選択header（control約12 KiB、escape後JSONは64 KiB超）による独立metadata拒否も確認した。さらに親側receiptのPID不一致・重複JSON key・整数欄bool・非zero exit・stderrをprocess無しのmock契約で拒否し、cleanup retryを検査する1群を追加。`target/http-baseline-owned-cli-tests-final.log`は12試験中11成功・同じsymlink権限1 skip、exit 0。実TLS・8 MiB各framing・timeout回収等も同じ最新sourceで再確認した。
