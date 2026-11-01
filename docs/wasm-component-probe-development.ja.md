# Component Modelの独立した固定fixture候補

2026-10-29の開発実験。先行する[core module実験](wasm-runtime-probe-development.ja.md)を変更せず、別の`experiments/wasm-component-probe/`、Cargo.lock、target directoryで調査する。製品のengine／SDK／WIT world／theme ABI、公開描画への接続を採用する作業ではない。任意の利用者guestや外部fileは受け入れず、所有する固定Component WATだけを使う。

Wasmtimeは正確に36.0.19、default featuresは無効、先行4featuresへ`component-model`だけを加える候補とする。WASI、host file／socket API、PHP、DB、HTTP、管理資格情報は渡さない。parent workspaceと製品Cargo.lockへ依存を追加しない。Rust guest targetや新しいtoolchainの導入も要しない。

## 文字列とpost-returnの流れ

固定UTF-8 stringを持つcore moduleをComponent内でinstantiateし、canonical liftでtyped exportへ接続する候補を調べる。ホストは`WasmStr`を受け、`to_str`でUTF-8を検査して借用し、byte capを検査して所有bufferへcopyする。借用を終えてから、同じfunctionの`post_return`を完了させ、成功した場合だけ所有した結果を返す。

この順序は固定版の[TypedFunc／WasmStr source](https://github.com/bytecodealliance/wasmtime/blob/v36.0.19/crates/wasmtime/src/runtime/component/func/typed.rs)で確認した契約に基づく。callが成功した場合は、canonical post-return optionが無くても`post_return`が必要で、call自体が失敗した場合は不要である。UTF-8やcapの拒否でもcall成功後なら後処理する。後処理のtrap／fuel切れはterminalとし、先に取り出した文字列を成功結果にしない。誤ったfunctionや二重のpost-returnなど、APIがpanicと定める誤用を通常の拒否結果として試験しない。

`WasmStr`はmemory内の範囲を表すが、文字列の妥当性は`to_str`まで確定しない。固定UTF-8 fixtureでは借用してcapを先に検査できる一方、UTF-16等の別encodingでは変換時にallocationがある。この実験から任意encodingの全allocationをcapより前に防げるとは主張しない。

## 対象と限界

このphaseの全Python回帰は296実行／292成功／Windows symlink権限4 skip、exit0（`target/wasm-component-probe-python-suite.log`）。既存11証跡checkerも全exit0（`target/wasm-component-current-check-summary.json`）。観測時のsource／lock／checker SHAを`target/wasm-component-probe-checkpoint.json`へ記録した。production source／root lockを変えず、既存PHP／DB証跡は維持した。

正常なstringと後処理、後処理によるguest memory変更前のcopy、cap／UTF-8拒否後の処理、未完了のpost-returnによる再入拒否を個別に確認する。canonical resultrecordのalignmentと範囲、payload範囲、型／export／importの不一致を正常出力へ変換しない。同じStoreの状態変化と、新しいStoreでの初期化も区別する。

fuelとmemory／table／Store内個数のlimiterはinstantiateより前へ設定し、core start、render、post-returnのloopを対象にする。`resources_required`は静的な見積として観測し、実際のlimiterの代わりにはしない。component本体、adapter、compiler、hostや全processのRSS／wall timeを一括して制限した証明にはしない。

Windows nativeのRust 1.96で5つのnamed契約が成功し、fmt／all-target Clippyもexit0を確認した（`target/wasm-component-probe-{tests,fmt,clippy}.log`）。固定31観測の内訳はlifecycle8、canonical拒否8、pending／fuel6、状態4、resource5。literalはNULとéを含むUTF-8の25 bytesで、HTMLや公開用本文と解釈していない。cap拒否・UTF-8拒否の後処理と、後処理のfuel切れが暫定結果を覆して失敗になることも実測した。

初回試験はWAT記述のparseで4契約がCompile拒否になった。解決済みwat 1.262.0の厳密なreference構文へ固定fixtureを直し、legacy許容の環境設定は変更しなかった。初回結果と診断は`target/wasm-component-probe-first-tests.log`／`target/wasm-component-probe-first-compile-diagnostic.log`に保持し、再試験と最終結果を分ける。拒否条件を一般errorへ緩めて成功に変えてはいない。

別の依存観測は`target/wasmtime-component-dependency-observation.json`。registry 118 packageのarchive SHAがlockと一致し、選択license候補194 fileのarchive／local bytesも一致した。候補file無し20 package、MSRV申告無し17 package、`.cargo-checksum.json`不在118件を明示した。申告MSRVに1.94超は無かったが、1.94での実compile試験や全source、実binaryの全linked成分、製品配布物の法的採否を証明する記録ではない。先行core実験の観測も変更していない。

先行実験のLinux cross-checkは既設C crosscompiler不足で失敗しており、このphaseで環境を追加して再試行しない。Windowsの限定成功をnative Linux、三配備環境、SDKや実themeの成功へ読み替えない。実artifactの外側観測と全Python回帰は後続で追記する。

## Component専用artifact実行harness

```powershell
python scripts/check_wasm_component_probe.py
python scripts/test_wasm_component_probe_gate.py -v
```

新checkerは固定manifest `experiments/wasm-component-probe/Cargo.toml`、package／bin `recastpress-wasm-component-probe`、専用`target/wasm-component-probe`へ`cargo build --offline --locked --message-format=json`を実行する。先行core package／gate／testsを変更せず、任意artifact、PID、guest WAT、fileや実行引数の入口も増やさない。tool install／online fetchは行わない。

完全なCargo JSONから固定manifest・main source・bin名／kind・non-test profileと成功したbuild-finishedを検査し、専用target内に存在する唯一のabsolute executableを直接実行する。先行coreのmanifest、target外path、不完全・失敗・曖昧なartifactを拒否し、古い既定pathへfallbackしない。

buildは600秒、native childは20秒をowned helperへ指定する。stderr空、prefix付きstdout一行、exactな7fieldと型、実captured PID、profile、31観測／25 output bytes／fuel 10000／unpublished flagを検査し、cleanup retry後だけ成功を返す。31はnative内の固定観測成功の集計で、assert文の数や、外側checkerが各caseを独立に再実装・検証したという意味ではない。5つのnamed Rust契約とsource reviewを併用する。

Cargo JSON 16 MiB、build diagnostics 1 MiB、native result 4 KiBの上限はcapture後の検査。capture中のallocation／RSS、compiler全工程のCPU、OS scheduling／cleanup失敗まで含む厳密な終了SLAは保証しない。Cargo／tool wrapper／host環境と安定したartifactは信頼する前提で、unsupported target等を別runnerへ黙って代替しない。pathとSHA-256はローカル観測で、署名・reproducible build・敵対的hostの真正性認証ではない。

専用Python契約6件は、固定artifact選択、旧core等のmanifest／source／profile不一致、target外・曖昧・失敗・不完全build、actual PIDとstrict result型、postcapture cap、固定command、cleanup retry、任意引数の拒否をmockで確認する。unittest discoveryではCargo buildやnative childを起動せず、先行owned helperの実timeout／回収試験も重複しない。

2026-10-29のWindows限定結果は`target/wasm-component-probe-gate-tests.log`で6件pass、`target/wasm-component-probe-executable-observation.log`で実checker exit 0。artifact SHA-256は`dd50a68e76dc7b12785f051b5c4c1892c1393d1a5bfb2c07eabc9298c91126a1`、native報告PIDはcaptured PID 29160と一致し、31／25／10000の集計を受理した。後続rebuildでhashは変わり得るため永続の許可hashではない。Linux、全Python回帰、製品SDK／公開経路の成功をこの限定結果に含めない。
