# Linux host上の限定実行gate

現在地の照合（2026-10-30）：[実装と上位設計・重要順報告](implementation-design-reconciliation-2026-10-30.ja.md)を参照。以下のcheckpoint／件数／currentという記載は各観測時点の履歴であり、直近Cookie本体取り込み後の全体合格を示さない。現在は対象Rust21／Python12・fmt／Clippyまでで、正式比較・DB・依存台帳等の再検証は未実行。今回の文書更新で製品コード開発は再開していない。

更新後の対象確認（2026-10-30）：選択 Python を 10 ファイルへ拡張し、workspace Clippy の後に独立 pure WP profile を追加した。通常 tool command は所有した直接 child として有限 timeout／cleanup retry を扱う。`target/native-linux-gate-updated-contract.log` の 13 契約に続き、stdin 非対話化後の最終 source でも全 13 契約が成功した（`target/native-linux-gate-final-contract.log`）。更新後の Windows 実 plan／help は exit 0、通常実行の非 Linux 拒否は exit 1、最終既存 11 checker も全 exit 0 を確認した。Linux native 本体は未実行であり、以下の旧準備確認も Linux 成功へ読み替えない。

旧版の Windows 準備確認履歴（2026-10-29）：mock7件成功、実`--plan` exit0、通常実行は非Linux guardによりexit1。記録は`target/native-linux-gate-contract.log`、`target/native-linux-gate-plan.log`、`target/native-linux-gate-windows-refusal.log`。独立read-only reviewでblocking無し。Linux select capture本体を含め、以下のnative経路は未実行である。当時の既存11証跡checker全exit0も、このgateのLinux成功証明ではない。

この開発環境にはLinux native実行環境がなく、以下のLinux実行gateは未実行。WindowsでのRust・HTTP・CLI試験やLinux向けcross-checkの成功を、Linuxのリンク・実行・シグナル受信の成功へ読み替えない。将来、利用者が用意したLinux host上で有限のローカル契約を確認するための入口であり、CI設定・配備手順・ツール導入機構ではない。

今回の Windows host 調査では、既設 `wsl.exe` の distribution 一覧確認が WSL 未導入を示し、Docker／Podman コマンドも見つからなかった。Linux runtime や container tool を追加導入せず、この環境で実行できない部分は未確認のまま残す。

## 前提と範囲

実行には`sys.platform == "linux"`、Python 3.11以上、既設のrustc・Cargo・rustfmt・Clippy、native linkerと必要なローカルtoolchain、lock済みcrateの取得済みcacheが必要。依存のnative buildに必要なC compiler・`ar`等も環境に応じて事前に用意する。checkoutとtargetへ書込みでき、所有する一時child processの起動・停止、loopbackのbind・HTTP接続が許可されていることも前提とする。不足するツールのinstallやcrateのfetchを行わず、Cargoのtest・Clippy・buildは`--offline --locked`で動かす。

rustcが報告する実hostをCargoの`CARGO_BUILD_TARGET`へ固定する。Cargo JSONから今回のserver executableを一つだけ特定し、そのpathを`smoke.BINARY`へ渡して既存`smoke.main()`を呼ぶ。古い既定`target/debug/recastpress`が存在するという理由だけで、それを今回のnative binaryとして採用しない。

確認対象はローカルのRust契約、実HTTP、既存one-shot CLI、owned child lifecycle、固定prepared HTML・feed XMLの独立parserによる契約。実HTTPではpublic／managementの分離、GET／HEAD、Host・query・readiness、起動拒否、binary／redirect／Range／ETagを含む。Linuxでは両roleそれぞれへSIGINT・SIGTERMを送る4通りの正常終了とhandler logも実際に確認する。試験の待機期限は製品の終了SLAではない。

package CLIは固定2ファイルの生成・byte identity・既存出力先の非上書き等を確認し、check-package は receipt と同一 buffer の bundle admission を検査する。title publication は固定公開 dummy の整形済みタイトルから bundle／receipt と所有 HTTP までの有限経路で、実 DB 保存や認証付き CMS の完成ではない。symlinkを作れない環境では該当試験がskipされるため、全体成功だけでsymlink契約の実測済みを主張せず、skipを確認する。Rustの既存live DB契約は通常のignoreを維持し、実DBを起動した試験には数えない。

対象にDB・binlog・PHP・WordPress実比較・既存Windows証跡の再認定は含めない。Linux memory診断、RSS／cgroup観測、自動メモリ容量決定、負荷試験、production shutdown、container／三配備先の互換性をこの選択試験から一般化しない。既存の広い`check.py`はreference・証跡checkerも含むため、この限定gateと同一ではない。

## コマンドと選択試験

リポジトリ直下から、予定の確認だけを行う場合は次を実行する。Windowsからもplanを表示できる。

```sh
python scripts/check_native_linux.py --plan
```

準備済みのLinux hostで実行する場合は引数を付けない。hostとPythonの先行guardを通ってから外部toolを起動し、最初の失敗で停止する。

```sh
python3 scripts/check_native_linux.py
```

更新後の実runはrustc・Cargo等のローカルtool情報を確認し、`cargo fmt --all -- --check`、workspaceの`cargo test`、全targetのClippy、独立 pure WP profile、server binaryのCargo JSON build、今回特定したbinaryによるHTTP smokeの順に進む。Cargoのtest・Clippy・buildは`--offline --locked`を用いる。既存のtargetディレクトリ指定は維持し、rustcの実hostでbuild targetを固定する。

pure WP は `check_wp_pure_profile.py` を独立 command として実行する。WP package 単独の no-default-features、Cargo artifact の features 空、normal dependency が WP 自身だけであることを確認する。workspace の DB dev-dependency が既定 auth を再有効化し得るため、workspace test の結果でこの独立確認を代用しない。専用 `target/wp-pure-profile` と tool wrapper／runner／Cargo global 設定は、その controller の trusted local 前提に従う。

続いて次の10ファイルだけを、個別のunittest discoveryで実行する。これらも同じCargo環境を継承し、Cargoを呼ぶ試験は実artifactを選択する。

- `scripts/test_owned_probe.py`
- `scripts/test_smoke_lifecycle.py`
- `scripts/test_bundle_check_cli.py`
- `scripts/test_publication_cli.py`
- `scripts/test_post_package_cli.py`
- `scripts/test_prepared_posts_html.py`
- `scripts/test_feed_xml.py`
- `scripts/test_comment_feed_xml.py`
- `scripts/test_package_check_cli.py`
- `scripts/test_title_publication.py`

Python部分は標準ライブラリとリポジトリ内moduleを使う。Rust exampleを実行するHTML/XML試験も含むため、Python試験だけをtoolchain不要な検査とは扱わない。全`test_*.py`のdiscoveryや、既存証跡checkerの一括実行は行わない。

## 所有 process と timeout の範囲

通常の tool command は `Popen` で所有する直接 child を保持する。stdin は `DEVNULL` の EOF として非対話にし、stdout／stderr だけを継承して live 出力する。wait は command ごとに 1,800 秒。timeout／終了失敗では、生きている直接 child の kill と 10 秒の有限 wait を試し、未回収 handle は保持して外側 cleanup で全件 retry する。1 件の cleanup 失敗で残りの handle の試行を省略しない。これは command 単位の timeout であり、全 gate 一括の deadline ではない。Cargo が起動する compiler／wrapper／runner や Python 試験内の子孫を OS process-tree 単位で隔離・回収する保証ではない。

metadata／Cargo JSON の取得は別の Linux nonblocking select capture 経路を使い、直接 child と pipe の回収を確認する。rustc／Cargo version は各 30 秒、server Cargo JSON build は 600 秒。通常 command の継承出力と、capture の有限 byte 検査を同一視しない。選択試験や pure controller はそれぞれの所有 helper と一時 directory を用い、Linuxの一時 file／dir・loopback 許可が別途必要になる。失敗後に cleanup を確認できなければ成功とは扱わない。

## 観測の意味

更新後の 13 契約には、metadata／host guard／実行順と選択対象の mock に加え、Windows 上の既設 Python child の成功、非zero exit 7、0.2 秒 timeout の 3 実観測を含めた。それらの捕捉 child は回収済み。13 件にさらに 3 件を加算する数え方はしない。Rust／Cargo／DB／Linux runtime をこの対象試験から起動した結果ではなく、Linux nonblocking capture や Unix signal の実行成功でもない。

stdin を DEVNULL へ固定した後は、成功 child が stdin を読み empty EOF を確認するよう同じ実 child 契約を再測定し、非zero／timeout とともに成功した（`target/native-linux-gate-stdin-contract.log`）。stdout／stderr も capture へ変えたという結果ではなく、両出力の継承は維持する。

更新後の入口を Windows で所有する Python process として実行した記録は `target/native-linux-updated-entrypoint-observation.json`。`--plan` は exit 0（stdout 1,200 bytes）、`--help` は exit 0（592 bytes）、通常実行は非 Linux guard の exit 1（stdout 空、stderr 95 bytes）で、plan に pure stage と追加 2 試験を含むことを確認した。対応 log は `target/native-linux-updated-{plan,help,windows-refusal}.log`。捕捉した 3 process は終了・回収済みで、source SHA は確認前後一致した。これは入口自体を実行した外側の観測記録であり、gate が Linux の tool／HTTP を起動した記録ではない。plan／help と先行 guard が tool child を起動しないことは契約試験でも拘束する。

今回の関連差分は、package CLI 既存 6 群（`target/native-linux-package-check-cli.log`）と owned probe lifecycle 8 群（`target/native-linux-owned-probe-lifecycle.log`）も成功した。最終既存 11 checker は `target/native-linux-harness-current-check-summary.json` ですべて exit 0、所有 controller PID 15771 も exit 0 で終了した。今回 Rust／DB／全 Python discovery は再実行せず、関連差分の対象試験と既存証跡の source edge current 確認を行った。

直前の title checkpoint の既定 Rust 573（ignore 1）／pure 300／Python 368／正式 75 API／全 8 DB 各 62 契約は、その時点の過去全 gate として維持する。今回の Windows 準備確認でそれらを再実測したことにせず、既存証跡の確認から Linux native 成功を導かない。

planは実行予定の範囲を表示するだけで、どのhostでもchild processを起動せず、コンパイル・ツールprobe・HTTPを実行しない。planの成功はLinux環境・ツール・cacheの準備完了や試験成功を意味しない。

実runで得るhost、tool version、Cargo artifact等は、信頼するローカルtoolと設定の観測値。署名、CI attestation、遠隔hostの真正性証明、敵対的なCargo設定・tool wrapperに対する隔離ではない。版や成功数は実際の出力に従い、Windows側の実測値を転記してLinuxの実績にしない。

scriptは実artifactのpath・SHA-256とhost・tool version・対象範囲を人間向けに出力し、専用 report JSON や正式な proof JSON を新設しない。独立 pure controller の既存 JSON summary はその stage の継承 stdout に出るが、Linux gate の署名付き証跡ではない。Cargoのglobal runner等も信頼するhost設定に含む。出力を保存する場合もローカル観測記録であり、既存のWindows PHP／DB証跡を更新したことにはならない。
