# WP crate の認証 provider と pure build profile

2026-10-30の現状態：prepared Cookie照合はauth feature下のmodule／exportsと専用exampleをmainへ取り込んだ。Rust focused21、Python12、fmt／WP全target Clippyは成功したが、package単独pure、既定対明示authのnormal graph、Linux cross compile、全体gateと依存inventory／licenseは未再実行。過去のpure313／normal WP1等は履歴で、現manifestへのcurrent成功へ付け替えない。中断記録は `target/auth-cookie-verification-pause.json`。

新しい `serde` はexample用のdev edgeで、lockのregistry163 tupleは変更無しという差分監査まで確認した。normal graphが変化していないことの実再観測とは別である。default authの宣言は維持し、認可やruntimeのsecurity境界には用いない。以下の導入時／後続checkpointの件数はそれぞれ当時の結果である。

[sanitize_text field／textarea](sanitize-text-development.ja.md)は新依存無しのpure std helperとしてmainへ採用し、全体gateも成功した。pure313（62 artifact／features空／normal依存WP1）を実確認し、最終11checkerは全exit0（`target/sanitize-text-current-check-summary.json`）、依存6fileの現SHA＝開始snapshot＝parser77 checkpoint SHAも実assert済み。sanitize checkpoint生成はdocs freeze後の別操作。以下のparser77／auth76 checkpointは完成履歴として保持する。

[明示cookie parser](auth-cookie-parse-development.ja.md)はpure std helperとして採用し、auth feature付き生成関係1とpure分解5を実検証した。全体gateのpure305（61 artifact／features空／normal WP1）も成功。最終11checker全exit0、manifest／lock／inventory／license6fileのbyte不変assertも成功した。parser checkpoint生成はdocs freeze後の別操作。下記auth76および導入時checkpointは完成履歴として保持する。

後続の [raw認証cookie生成](auth-cookie-development.ja.md)でoptional `hmac_md5` を追加し、8 provider edge／auth専用7 test targetになった。今回の全体gateは成功し、pure300（60 artifact／features空／normal依存WP1）を実確認した。defaultと明示authのraw treeは一致し、23 distinct package names／28 version別packageを観測。最終11checkerはすべてexit0（`target/auth-cookie-current-check-summary.json`、controller58647も出力回収後exit0）。以下の7依存／6 test targetおよび件数は、分離導入時のcheckpointとして保持する。

`recastpress-wp` の build-time feature 境界を追加した。既定は `default = ["auth"]` のままで、既存の認証 algorithm や通常 caller の既定動作を変更しない。`--no-default-features` の package 単独検査で、認証 provider を必要としない pure helper 群を確認するための候補である。認可・sandbox・公開 route の切替ではない。

## 現manifestのFeature宣言（graph再確認待ち）

現manifestの `auth` は `bcrypt`、`hmac`、`hmac_md5`、`sha2`、`base64`、`zeroize`、`md-5`、`subtle` の８ direct provider edgeをoptionalとして有効にする。`hmac_md5` はHMAC0.13の別名で、既存HMAC0.12／SHA2.10と区別する。外部auth test target７件とexample５件に `required-features = ["auth"]` があり、新matcher内部契約もauth moduleのcfgに従う。`auth_cookie_verify_probe` をpure実行へ混ぜたり、残った古いbinaryで代用したりしない。今回のprovider版は変更せず、serde dev-edgeによるlock更新と、このfeature宣言の意味を分ける。

既定対明示 `--features auth` の通常graph一致は過去に観測したが、今回のmain取り込み後は再確認待ち。DB の既存 WP dev-dependency など、別のcallerが既定featureを有効にすれば、Cargoのfeature統合でauthを再有効化できる。このためworkspace全体の検査をpackage単独のpure profileと取り違えず、専用controllerは `--workspace` を使わない。normal edgeだけのgraphと、試験用serde／serde_jsonを含むdev／build解決を区別する。

変更前の観測 `target/wp-auth-feature-before.json` は、no-default 指定でも通常 graph に 21 package があり、当時の provider と transitive `getrandom` の計 8 対象名が含まれたことを記録する。分離後の当時のpackage単独通常graphでは `recastpress-wp` 自身だけを観測した。現manifestへの再確認はまだ行っていない。これらは固定manifestと選択profileの観測であり、任意caller／将来の全依存／配布licenseの包括的保証ではない。

## 固定 controller

リポジトリのルートから次を実行する。

```powershell
python scripts/check_wp_pure_profile.py
```

引数は受け付けず、`--help` だけは子プロセスなしで使える。任意の manifest／package／binary／target／Cargo 引数を入力する入口はない。既設の rustc／Cargo／Clippy、native linker、取得済み依存と書込み可能な checkout を使い、install・fetch・配備を行わない。

全体確認へ含める場合は `python scripts/check.py --with-pure-wp` を使う。この flag は上記の専用 controller を追加実行するもので、workspace の既定 auth 試験を pure profile に置き換えるものではない。

controller は所有した `rustc -vV` から host を読み、`CARGO_BUILD_TARGET` と各 Cargo `--target` をその host に固定する。固定 package `recastpress-wp`、固定 manifest `crates/recastpress-wp/Cargo.toml`、専用 `target/wp-pure-profile` だけで、次を順に実行する。

1. `cargo test --no-default-features --all-targets --offline --locked --message-format=json`。
2. 同じ profile の `cargo clippy --all-targets --message-format=json -- -D warnings`。
3. 同じ package／feature／host の `cargo tree --edges normal --no-dedupe --prefix none --format "{p}"`。

上記の省略表示にも controller は manifest／package／host を必須指定し、test と Clippy は専用 target directory を明示する。全 command は固定引数で `shell=False` の既存 owned helper を通す。

test／Clippy の Cargo JSON は build-finished 成功まで厳密に読み、全 WP artifact の features が空であること、manifest と source の一致、専用 directory 内の実在 artifact、非 test lib artifact がちょうど 1 件あることを検査する。test は現 test executable artifact があり、build 完了後に成功 summary と正の passed 件数があることも必要。Clippy はリンクしない test artifact の executable が Null でもよい。既存 binary の path を推測して代用しない。

通常 graph は package 自身の 1 行だけを受理し、provider／crypto／RNG／dev dependency を含む追加 edge は拒否する。成功は host、空の WP features、試験件数、通常 package 名と所有 command PID を含む小さな JSON 1 行。失敗表示は固定 `WP pure profile check failed`、通常の検査失敗は exit 1、引数の上書き要求は exit 2。入力や任意の tool 診断を表示しない。stdout 書込み失敗では既に送られた report bytes を撤回できず、stderr 配送も外部出力先に依存する。

## Timeout と検証範囲

rustc は 30 秒、Cargo test／Clippy は各 600 秒、tree は 60 秒の所有 command timeout を指定し、失敗時も外側で未回収 handle の cleanup を再試行する。任意の Cargo wrapper／runner が作る子孫を OS 単位で隔離・強制回収する仕組みではなく、tool と環境は trusted local 前提。compiler の子孫や pipe handle の挙動を含む厳密な全 process-tree deadline は保証しない。capture の stdout 16 MiB／stderr 4 MiB は終了後の検査であり、RSS や取得中の buffer 上限ではない。

これは実行 host の native profile である。Wasm target、guest SDK、Linux native、MSRV、配布 package、認証成功や role 境界の検証ではない。declared rust-version に対する最小版コンパイラー試験や、追加 target の導入を行わない。

## 対象確認と実controllerの導入時履歴

feature 実装担当の先行 package-only native 実行は 281 tests 成功、57 groups（pure examples の 0-test group 2 件を含む）。no-default 全 target Clippy、既定全 target cargo check、file 限定 fmt も成功した。通常 no-default graph は WP 自身 1 package で、既定と明示 auth の通常 graph は byte 一致した。log は `target/wp-pure-profile-{tests,clippy,normal-graph}.log` と `target/wp-auth-feature-{default-check,default-normal-graph,explicit-normal-graph,fmt}.log`。

専用 controller の mock 契約 `python scripts/test_wp_pure_profile.py -v` は 7 件全成功した。auth feature 混入、誤 manifest／source、stale file、複数 lib、build／test 失敗・0 件、通常 graph の crypto／dev edge、固定 host／引数、timeout／cleanup retry と固定診断を検証した。mock 試験では Cargo／Rust binary を起動しない。

単独 controller の実行 `target/wp-pure-profile-controller.log` は exit 0。実 host `x86_64-pc-windows-msvc` で 281 tests／57 test artifacts、WP features 空、通常 package は WP 自身 1 件を確認し、rustc／test／Clippy／tree の捕捉 PID 4 件を記録した。続く `--with-pure-wp` 統合実行の `target/wp-auth-feature-integrated-pure-profile.json` でも 281 件と通常 graph 1 件を確認した。

`target/wp-auth-feature-full-check.log` は exit 0。既定 profile の workspace Rust 554 件（ignore 1）、pure 281 件、独立 29 件、Python 342 件（336 成功・symlink 権限不足 6 skip）が成功した。正式 73 API／486,153 件の API 別件数・入力・PHP／Rust 両結果と hooks／dispatcher の結果は、`target/wp-auth-old73-preservation-check.log` で従来結果との完全一致を実照合した。生成 10、legacy 113（actual 88）、credential stage 33（actual 25）、全 8 fresh DB profile 各 62 契約、HTTP／process smoke、Clippy、既定 profile の Linux cross-check も成功し、専用 DB directory は回収後に空である。

WP manifest の feature 変更に合わせ、通常 inventory は `dependency_inventory.py` を実行して再観測した。license 215 件も `target/wp-auth-license-observation.log` で実再確認し、既存結果の hash 付替えだけで更新していない。既定 WP と明示 auth の 21 package 通常 graph も fresh に比較し、一致を確認した。pure の変更前 21 package／7 direct provider と getrandom の存在に対し、変更後は WP 自身 1 package となった。

最終既存 11 checker は `target/wp-auth-feature-current-check-summary.json` ですべて exit 0。これらは観測した native host と固定 profile の確認であり、Wasm compile、MSRV、Linux native 実行、security policy や認可の検証ではない。
