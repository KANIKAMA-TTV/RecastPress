# 開発版の起動

2026-10-30現在、Cookie照合のmain取り込みはfocused Rust21／Python12／fmt／WP Clippyまで成功してユーザー指示で中断中。正式79 API／519,014、認証４種の実比較、全８DB／workspace／Linux cross compile／pure／依存inventory・licenseの再検証は未実行。既存source-bound証跡をcurrent成功とは扱わない。以下は既存CLIの利用手順であり、全体検証済み宣言ではない。再開順は[開発メモ](development.ja.md)と `target/auth-cookie-verification-pause.json` を参照する。管理ready503・principal未接続は維持する。

固定Componentで事前生成したHTMLを既存sealed serverで確認する[Wasm公開経路の実験手順](wasm-publication-example-development.ja.md)を追加した。レビュー用packageは`target/wasm-publication-example/`。これはGET中のWasm実行や本番theme／CMS認証の完成ではない。

2026-10-29時点の実験用HTTPプロセス。CMS・移行ツール・Wasm実行環境としては未完成。

## ビルドと確認

```powershell
cargo build --workspace --locked
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
python scripts/smoke.py
```

初回はCargoが依存を取得する。取得済みなら各Cargoコマンドへ`--offline`を追加できる。

取得済み依存でのまとめて確認は`python scripts/check.py`。最初の失敗で終了する。開発用の固定WP/PHPも取得済みなら`--with-reference`、実HTTPプロセスは`--with-process-smoke`、専用MariaDB/MySQLは`--with-databases`を追加する。HTTP／DB試験は一時的なloopback接続を使用し、検証後に起動したプロセスを終了する。DBの準備と試験範囲は[DB開発メモ](database-development.ja.md)を参照。

WP既定パスワード生成の候補は`--with-password-generation`で専用の実PHP相互検証を要求する。ランダムhashのbyte一致や決定的な正式API比較件数へ加える試験ではない。`--with-linux-check`はserverとWP crateの全targetをLinux向けにcheckするが、Linux binaryのリンク・実行は含まない。

`--with-auth-cookies` はprepared GET／非Ajaxのcookie MAC＋session関係を実PHPのbuiltin-clock bracketとnative両端へ照合する専用flag。`compare_auth_cookies.py` と `check_auth_cookie_evidence.py` への接続は実装済みだが、main採用後にはまだ実行しておらず、`compatibility/evidence/auth-cookie-relations.json` も未生成。ログインを開始するcommandやHTTP認証の有効化ではない。再開時は先にinventory／licenseを実再生成し、既存認証３種と合わせて再測定する。

`--with-pure-wp` は、WP package 単独の no-default-features native 試験・Clippy・通常依存 graph を追加確認する。既定 auth は維持し、Wasm／MSRV／認可方針の検証とは分ける。[build profile の範囲と結果](wp-build-profiles-development.ja.md)を参照。

旧WP MD5／canonical portable phpassの限定検証は`--with-legacy-passwords`で要求する。actual WP呼出しとhost/domain拒否を分けた[専用証跡](wp-legacy-password-development.ja.md)を生成・照合する。新credentialのMD5生成やHTTPログインへ接続するflagではない。

## 公開プロセス

```powershell
cargo run -p recastpress-server --bin recastpress -- public --host localhost:8080 --bundle examples/public-bundle.json
```

`http://localhost:8080/`へ接続する。既定のbindは`127.0.0.1:8080`。`--host`は受け入れるHostヘッダーをポート込みで指定する。Hostが一致しない公開ページ要求は421。接続先の数値IPでアクセスする場合もHostを一致させる必要がある。

`--listen IP:PORT`でbindを明示できる。HTTPのみの実験用サーバーで、インターネット向けTLS終端はまだ同梱していない。`--listen 127.0.0.1:0`ではOSが空きポートを割り当て、標準エラー出力に実ポートを表示する。

`--bundle`を省略するとプロセスは起動するが公開準備未完了（503）。指定したバンドルが壊れている場合は起動を失敗させ、空のサイトへ黙って切り替えない。GET・HEADのみで、クエリーは並び順・符号化も含め完全一致させる。検索・WordPress URLルーティングを実装したものではない。

起動時にバンドルを一度だけ読み、以後は不変の公開世代だけを参照する。ファイル監視、実行中の公開更新、Wasm描画、PHP起動、DB接続はない。全応答に`Cache-Control: no-store`を付ける。メモリキャッシュのHTTP接続は実装済み。ただし実監視・自動容量算出がないため、通常のCLI起動ではキャッシュ保存・ヒットを停止し、設定onを保持して通常配信する。手動の容量指定や監視済みとみなす起動フラグは設けない。内部接続の制約は[HTTPキャッシュ開発メモ](http-cache-development.ja.md)を参照。

公開された非空の200本文はGETの単一byte Rangeを扱う。例は`Range: bytes=0-15`で、206と部分長／Content-Rangeを返す。有効な範囲外は416、HEAD・空本文・非200・複数range等は従来応答を維持する。条件headerは下記のETag profileで先に評価する。一般の条件付きHTTP全体やmultipartは未対応。部分bodyはcacheを通らず、元本文の共有ownerからbyte sliceを作る。部分payload用のVec copyは作らないが、短い非空sliceも元本文全体を保持し、RSSやtransport全体のcopy不在は保証しない。[正確なprofile](byte-range-development.ja.md)と[共有ownerの範囲](shared-public-body-development.ja.md)を参照。

準備済み200本文には起動時のSHA-256由来ETagを付ける。GET／HEADはIf-Matchのstrong比較、If-None-Matchのweak比較、その後のRange／If-Rangeを評価する。304は空bodyで、アプリのContent-Lengthは元本文長。実HTTP/1ではGET304の長さが省略されHEAD304に残る場合がある。412は空body、malformed tag listは400、host上限超は431。日付／Last-Modified・live公開の一般対応は含めない。[正確な条件profileとframework差](etag-development.ja.md)。

## 事前生成CSS／JavaScriptの公開例

固定CSS／JavaScriptを含むV3の[sealed asset例](sealed-public-assets-development.ja.md)も利用できる。旧V1/V2のreport1は維持し、V3だけreport2・九MIME keyを返す。

```powershell
cargo run -p recastpress-server --bin recastpress --offline --locked -- check-package examples/sealed-assets-bundle.json examples/sealed-assets-receipt.json
cargo run -p recastpress-server --bin recastpress --offline --locked -- public --host localhost:8080 --bundle examples/sealed-assets-bundle.json --receipt examples/sealed-assets-receipt.json
```

browserで `http://localhost:8080/asset-demo?view=1` を開く。起動後のfile変更は反映せず、CSS／JSの言語構文や一般theme互換を検証する機能ではない。

## 投稿テキストから公開バンドルを準備する

ローカルの[公開用 JSON 入力ワークベンチ](prepared-post-workbench-development.ja.md)では、記事文字の編集、JSON 書出しと明示ファイルの再読込みができる。読込みは一時表示後の明示適用で全件を置き換える。現在の記事から固定bundleとreceiptをまとめる[単一 ZIP 書出し](prepared-post-workbench-package-development.ja.md)も使える。タブ内入力は自動保存されない。ZIPは保存先を確認して解凍し、完成bundleの最終受理を次のCLIで確認する（pathは解凍先に合わせる）。ダウンロード開始は保存・公開完了の確認ではない。

workbenchが出力した固定ZIPなら、OSの解凍操作の代わりに[限定import](package-unpack-development.ja.md)も使える。ZIP pathは保存先に合わせ、出力先はまだ存在しないディレクトリにする。一般のZIPは受け付けない。

```powershell
cargo run -p recastpress-server --bin recastpress -- unpack-package prepared-post-package.zip target/prepared-post-package
```

```powershell
cargo run -p recastpress-server --bin recastpress -- check-package target/prepared-post-package/bundle.json target/prepared-post-package/receipt.json
```

固定の公開サンプルは`examples/prepared-posts-input.json`、CLIで生成した成果物は`examples/prepared-posts-bundle.json`と`examples/prepared-posts-receipt.json`にある。

```powershell
cargo run -p recastpress-server --bin recastpress -- public --host localhost:8080 --bundle examples/prepared-posts-bundle.json --receipt examples/prepared-posts-receipt.json
```

`http://localhost:8080/prepared-post`で、先頭改行と日本語、HTML風の文字列を確認できる。`title`と`body_text`は文字列としてエスケープし、HTMLとして実行しない。テーマ・ブロック・shortcodeの描画は含まない。

再生成する場合、ビルド後に次のPythonをリポジトリ直下で実行する。標準出力をbyteで保存するため、PowerShellの版によるリダイレクトの文字コード変換に依存しない。

```python
from pathlib import Path
import os
import subprocess

binary = Path("target/debug") / ("recastpress.exe" if os.name == "nt" else "recastpress")
for command, source, destination in [
    ("prepare-posts", "examples/prepared-posts-input.json", "examples/prepared-posts-bundle.json"),
    ("bundle-receipt", "examples/prepared-posts-bundle.json", "examples/prepared-posts-receipt.json"),
]:
    result = subprocess.run([str(binary.resolve()), command, source],
                            capture_output=True, check=True, timeout=15)
    Path(destination).write_bytes(result.stdout)
```

`prepare-posts <input>`は上限付きの版1投稿入力を検査し、完成したバンドルだけを標準出力へ返す。`bundle-receipt <bundle>`はそのファイルのbyte数とSHA-256を記録する。公開起動で`--receipt`を指定すると、同じ一回の読み取り結果で一致確認とバンドル検査を行い、不一致ならlisten前に失敗する。receiptは署名・承認・最新版の保証ではない。`--receipt`は`--bundle`を必要とし、管理roleでは受け付けない。

入力・bundleは8 MiBまで。公開プロセスは起動後に入力ファイルを読み直さず、更新には別プロセスの起動が必要。DBの限定CAS保存からこの経路へ渡す専用fixtureは[公開経路の開発記録](alpha-publication-development.ja.md)を参照。

新しいディレクトリへbundleとreceiptをまとめて準備する実験用コマンドもある。`target`は既存、`target/post-package-example`は未作成であること。

```powershell
cargo run -p recastpress-server --bin recastpress -- prepare-post-package examples/prepared-posts-input.json target/post-package-example
cargo run -p recastpress-server --bin recastpress -- public --host localhost:8080 --bundle target/post-package-example/bundle.json --receipt target/post-package-example/receipt.json
```

成功時は`post package prepared`を表示する。既存出力先は空dirも拒否し、失敗時の不完全dirは残す。自動公開やatomicな2ファイル保存、署名・最新版の保証は含まない。[packageの契約と制限](post-package-development.ja.md)を参照。

公開プロセスを起動せず bundle と receipt の両方を検査する場合は、`cargo run -p recastpress-server --bin recastpress -- check-package target/post-package-example/bundle.json target/post-package-example/receipt.json`。成功時は内容を含まない集計 JSON を返す。[検査の契約と検証状況](package-check-development.ja.md)を参照。

単一URLの手動HTTP baselineを保存する実験用入口もある。既存の開発serverを起動した後、未作成の出力先を指定する。

```powershell
python scripts/capture_http_baseline.py "http://localhost:8080/prepared-post" target/http-baseline-example
```

`body.bin`と`capture.json`を保存する。redirect追跡・Cookie／認証・環境proxyを使わず、成果物には私情報が含まれ得る。公開用packageや画面比較・移行成功の証明ではない。[HTTP baselineの限定profileと失敗時の扱い](http-baseline-development.ja.md)を参照。

保存した2組のbaselineをネットワークへ接続せずに構造比較する場合は、次を実行する。

```powershell
python scripts/compare_http_baselines.py target/http-baseline-before target/http-baseline-after
```

入力が有効なら差があってもexit 0で、body・status・選択header・URL context等の独立boolをJSONで返す。raw値やhashは出力せず、総合一致・画面比較・移行合否は判定しない。[比較の意味とfile検査の限界](http-baseline-comparison-development.ja.md)を参照。

明示した1～32組を順番に比較する場合は、`left`／`right`を並べたpair manifestを指定する。relative pathはmanifestの親を基準にする。

```powershell
python scripts/compare_http_baseline_pairs.py target/pairs.json
```

差があっても有効な一覧はexit 0、後段を含む不正入力はpartial report無しで失敗する。自動ペアリング・URL再取得・移行合否は含まない。[pair一覧の形式と限界](http-baseline-pair-list-development.ja.md)を参照。

callerが選んだ移行元sourceのpath・byte長・hashを記録する実験用入口もある。既存source-rootと明示selection JSON、未作成の出力先を指定する。

```powershell
python scripts/prepare_migration_source_inventory.py path/to/owned-source selection.json target/source-inventory-example
```

`inventory.json`だけを作り、コードを実行・copyせず、依存fileやライセンス適合を推測しない。[選択形式・上限・私情報と原状保持の範囲](migration-source-inventory-development.ja.md)を参照。

## 管理プロセス

別のターミナルで起動する。

```powershell
cargo run -p recastpress-server --bin recastpress -- management
```

既定のbindは`127.0.0.1:8081`。公開用`--bundle`と`--host`は拒否する。認証は未実装で、設定変更・公開切替・PHP実行等の管理APIは提供しない。準備状態は常に503。これは管理機能の完成を示すサーバーではなく、役割分離の起動骨格。

## ヘルスチェック

| パス | 公開 | 管理 |
| --- | --- | --- |
| `/health/live` | 200：HTTPプロセスが応答 | 200：HTTPプロセスが応答 |
| `/health/ready` | 公開世代ありなら200、なしなら503 | 認証未実装につき503 |

ヘルスチェックはHost検査の対象外。内部構成・資格情報・ページ内容を返さない。管理用の認証済み診断APIとは別物。

## 実験用バンドル

`examples/public-bundle.json`を参照。形式版は`format_version: 1`、UTF-8 JSONで8 MiBまで。各ページは`target`、`status`、`content_type`、`body`を持つ。未知フィールド、重複ターゲット、相対／外部URL、ヘッダー改行、予約済みヘルスパス、空サイト、未知形式版は拒否する。

追加の実験用版2は[examples/public-binary-bundle.json](../examples/public-binary-bundle.json)を参照。同じ`--bundle`へ渡せる。全ページが`body`の代わりに必須の`body_hex`を持ち、ASCII hexを2桁ずつbyteへ戻す。大小文字は両方受理し、空白・prefix・奇数桁・非hexは拒否する。text／feedの4種類は復元後のUTF-8を確認し、画像はexact `image/png`／`image/jpeg`／`image/webp` と宣言した種類の署名を照合する。元の版1で画像MIMEを受理する変更はない。

版2でもJSON全体8 MiBまでで、全本文のhex形式と復元長のchecked合計を先に検査する。任意のファイル名・URL・source参照を解決する仕組みは追加しない。画像は署名確認だけで、完全な画像decode・container検証・metadata除去や公開内容の判定ではない。サンプルは固定の1pixel PNGをmanifest内へ埋め込んでおり、既存のユーザーassetsを変更しない。[形式と検証範囲](binary-bundle-development.ja.md)。

版2には本文pageと別に`{"target":"/old-pixel","status":308,"location":"/pixel.png"}`というredirect recordも置ける。statusは301／302／303／307／308、Locationは検証したASCII root-relative subsetに限る。`content_type`／`body_hex`／任意headerとの混在は拒否し、GET／HEADはLocation一つ・空body・Content-Length0を返す。サーバーは転送先を取得せず、内部cacheにも保存しない。外部URL、fragment指定、`\`、不正percent pair、encoded control／space／slash／backslash等は対応しない。詳細なdomainとfragment継承の扱いは[typed redirect開発記録](typed-redirect-development.ja.md)を参照。

内容は公開専用データであることを作成者が確認する。投稿・下書き・秘密設定を丸ごと渡す形式ではない。将来のテーマ・Wasm成果物契約とは別の開発用形式で、PHPや任意の実行コマンドを含めない。

公開targetはクエリーを含む8,192 byteまで。HTTP要求とページ束の準備で同じ上限を使い、超過したページ束は起動前に拒否する。製品の将来のURL長契約を確定したものではない。

## プロセスの終了とLinux向けコンパイル確認

別のDocker利用可能な環境で検証するための[開発用Docker／Compose候補](../deploy/development/README.ja.md)も用意した。builder/runtime imageは呼出し側が実digestと互換性を確認して指定する。この作業環境ではDockerがなく、構成の静的確認だけでbuild・container起動・native Linux動作は未検証。

UnixではSIGINT・SIGTERMを受け、Axumのgraceful shutdownへ通知する。Unixのシグナル登録はlistenの準備完了表示より前に行い、登録に失敗すれば起動を失敗させる。Windowsは従来のCtrl+Cを受け、受信処理の失敗もログに残して終了へ進む。終了待ち時間の固定値や、配備環境による強制終了前の完了保証は追加していない。

WindowsでのHTTP試験に加え、Linux標準ライブラリを持つ開発環境で条件付きコードをコンパイルできる。

```powershell
rustup target add x86_64-unknown-linux-gnu
python scripts/check.py --with-process-smoke --with-linux-check
```

2026-10-26にLinux向けserverのall-targetsのcargo checkが成功した。これはLinuxバイナリのリンク・実行やSIGTERM受信・接続終了・三配備先の検証ではない。WindowsのプロセススモークもCtrl+C送信試験ではなく、HTTP動作と試験プロセスの終了回収を確認する。

将来のLinux hostでRust・実HTTP・one-shot CLI等を選択実行する限定gateは、[Linux native検証の範囲と前提](native-linux-validation.ja.md)を参照。`python scripts/check_native_linux.py --plan`はWindowsでもchildを起動せず予定だけを表示する。準備済みLinux hostでは`python3 scripts/check_native_linux.py`で実行する。この開発環境にはLinux実行環境がなく、native gateは未実行。CI設定・tool install・DB／PHP比較・既存Windows証跡の再認定は含まない。

## 起動拒否と終了の実プロセス試験

scripts/smoke.pyは、壊れた公開bundle・8 MiB超過・予約経路衝突などでプロセスが非zero終了し、listen成功を告知しないことを確認する。専用loopbackソケットを保持して、public／managementとも使用中ポートで失敗することも試験する。公開bundleなしではlive=200、readyとページ応答=503を確認する。

Unix上で実行した場合は両roleへSIGINT／SIGTERMを送り、正常終了とハンドラーログを検査する。5秒は試験の待機期限で、製品の終了SLAではない。現在のWindowsでの実行結果はUnix試験を未実行と明示する。Linux向けcargo checkは成功しているが、WSLは未インストールで、Linuxの実行・シグナル受信を検証したものではない。

## 標準feed serializerの候補

`recastpress_server::feeds::render`は、ホストが公開用に準備したFeedDocumentをRSS2／Atom1のXMLへ変換するnative API。FeedFormat::content_typeは既存PublicPageが許可する`application/rss+xml; charset=utf-8`／`application/atom+xml; charset=utf-8`を返す。render結果をPublicPageにし、PreparedSiteへ指定targetとして入れれば、既存公開経路のGET／HEAD・Host検査で配信できる。CLIに新しいfeed endpointや生成用flagを自動追加したものではない。詳細な入力・byte上限・未対応範囲は[feed開発メモ](feeds-development.ja.md)を参照。

固定2記事の開発用サンプルは[examples/feed-bundle.json](../examples/feed-bundle.json)。既存CLIで読み込める。

```powershell
cargo run -p recastpress-server --bin recastpress -- public --host localhost:8080 --bundle examples/feed-bundle.json
```

`http://localhost:8080/`の相対リンクから`/feed/rss`と`/feed/atom`へ移動できる。本文はfeed_probeの固定例で、実サイトの記事queryやCMSの公開処理を代替しない。追加したのは既存bundle形式のartifactであり、新CLI flag・自動feed endpointではない。

記事の公開選別、private data除去、HTML sanitization、WP feed template／filter／query互換、コメントの公開選別・WPコメントfeed全互換、連続生成・公開世代切替は未実装。既存CLIの起動時不変世代、実監視・自動容量未確認時のcache停止、Linux RAM観測の非Linux Unsupportedも維持する。

最新ログtarget/rss-atom-serialization-and-http-check.logでRust181件・Python48件、HTTP・Clippy・Linux向けcross-checkが成功。WP35 API・249,241ケースの実PHP比較は以前のclassname gateで、後続はWPソース無変更のため再実行を省略してhash照合した。Linux native実行・本番配備の検証ではない。

事前生成host向けにはfeeds::render_budgetedもある。これは明示共有poolの論理予約を出力ownerへ保持するAPIで、既存CLIの挙動やfeed endpointを追加しない。borrowからPublicPage等へコピーしたbytesやHTTP Bodyはこのownerの課金外であり、配信全体の共有予算接続とは扱わない。[課金範囲と検証](feeds-development.ja.md)を参照。

組込みhost向けのpublic_router_with_budgeted_cache(site, host, pool)は明示共有poolを受け取るopt-in API。既存CLIやpublic_router／public_router_with_cacheは従来backendを保ち、監視／local予算未確認時は停止する。内部controlのlocal配分と外部poolのlimitは独立し、Body／Bytesのowner保持だけを課金する。fallback／準備済みpage／transportのメモリは未計上で、配信全体の容量自動化ではない。[所有境界と検証](http-cache-development.ja.md)を参照。

2026-10-27のread-only再確認でもWSL未インストールだった（target/linux-native-environment-check.log）。OS機能の追加は行わず、native Linux／Unix signal試験は未実施を維持する。

## コメント更新feedの固定サンプル

[examples/comment-feed-bundle.json](../examples/comment-feed-bundle.json)は、公開準備済み2コメントのRSS2／Atom1を既存版1bundleへ収めた固定artifactである。

```powershell
cargo run -p recastpress-server --bin recastpress -- public --host localhost:8080 --bundle examples/comment-feed-bundle.json
```

homepageから`/comments/rss`と`/comments/atom`を確認できる。RSSはdescriptionと全文、Atomはcontentと明示的な返信先参照を含む。本文はserializerの固定probe実出力であり、DBの公開コメント選別、PII除去、親子判定、自動endpoint登録や連続生成を行わない。GET/HEAD・ETag・単一byte Rangeは既存のsealed配信経路で処理する。[コメントfeedの範囲](comment-feeds-development.ja.md)

開発用の`python scripts/smoke_comment_feed_example.py`は、ビルド済みの所有loopbackサーバーだけを起動してXMLとHTTPを照合し、終了時に回収する。外部self URLや返信先は取得しない。`target/comment-feed-example-http-observations.json`は固定サンプルの観測であり、実サイトの公開可否を証明するものではない。

## 起動前のbundle受理チェック

```powershell
cargo run -p recastpress-server --bin recastpress --offline --locked -- check-bundle examples/comment-feed-bundle.json
```

このone-shot commandは既存の版1／版2 loaderと同じ受理条件を検査し、成功時に小さなJSONを1行返して終了する。accepted bundle version、ページ数、転送数、decoded body合計bytes、0件を含む固定7MIMEのbody page数だけを報告する。本文・target・Location・pathはreportへ含めない。失敗時はstdout空、固定の短いstderr、exit2とし、listenerや公開処理は起動しない。

これは既存strict admissionの開発用確認であり、HTML/XMLの完全解析、画像の完全decode、転送graph、公開権限・PII対策、Wasmの検証を代替しない。body bytesは論理的な本文長であり、RAM使用量や自動容量ではない。[入力・集計・検証範囲](bundle-check-development.ja.md)

## 明示的なメモリ観測

```powershell
cargo run -p recastpress-server --bin recastpress --offline --locked -- inspect-memory
```

host物理RAM、自身のRSS、可視cgroup v2階層を独立sectionのJSONで返すone-shot診断。自身とはこのCLI processであり、別に起動中の公開／管理serverの測定ではない。PID・path・容量の指定は受け付けない。raw filesystem pathや任意のIO error文字列はreportへ出さず、未取得を0へ置き換えない。

要求した観測が取得できた場合はexit0、section未取得や可視boundary未観測を含むreportはexit3、不正引数／report失敗はstdout空と固定stderrでexit2。0も全祖先の可視性・瞬間値の原子性・配分可能量を保証しない。Windowsでは全Unsupported／exit3を確認し、Linuxはcrosscompileのみ。監視・cache/pool接続・自動容量算式は実装していない。[診断の範囲](memory-diagnostic-development.ja.md)
