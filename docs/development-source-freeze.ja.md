# 開発gate中の選択source固定確認

`scripts/check.py` に、1回の実行開始時に選択sourceのbaselineを取得し、各commandの前後と最後に同じbaselineへ照合する `SourceFreeze` を追加した。baselineを各command後の状態へ更新しない。長い検証中にsourceの追加・削除・長さ／SHA変化を見落として成功扱いすることを減らす開発用guardであり、compiler入力の完全なsnapshotや署名付きattestationではない。

## 選択範囲

必須のroot `Cargo.toml`／`Cargo.lock` と `compatibility/registry.json` を含め、`crates`／`scripts`／`tools`／`experiments`／`examples`／`compatibility` の6treeを走査する。存在するroot `rustfmt.toml`／`.rustfmt.toml`／`rust-toolchain`／`rust-toolchain.toml` と `.cargo/config`／`.cargo/config.toml` も含める。`.cargo` credentialsは読まない。任意configの新規出現や削除も次照合で選択集合の差になる。

generated evidenceの `compatibility/evidence`、artifact directory名 `target`／`node_modules`／`__pycache__`／`.git`／`docs`、`.md`／`.pyc` fileは内容の選択対象外。docsはgate中に更新できるが、source／registry等を変更すればguardが失敗する。選択外のroot file、外部tool、依存cache、環境変数、Cargo wrapper／global configの全てを固定したとは扱わない。

## 比較と失敗

各snapshotは相対pathに対するbyte長とSHA256の集合である。選択fileの読取りではlstat→open／fstat→読取り→fstat／lstatを照合し、device／inode／size／mtimeの可視変化を拒否する。mtimeは読取り前後の整合確認であり、baseline間でbyteが同じfileのtouchを必ず拒否する仕組みではない。Windowsのlstat／fstatでctimeの意味が異なることがあるため、ctimeを比較へ使用しない。

検出したlink／Windows reparse point、special file、読取失敗、範囲超過も失敗にする。固定診断は `Development source freeze check failed`。pathやsource内容を診断へ出さない。command自体が失敗した場合はそのstatusを保持し、後確認でも失敗すれば固定note `Source freeze verification also failed after the command failure` を追加してstderrへ表示する。guard失敗を元commandの成功へ変換しない。

`--help` はbaseline取得より前に処理する。既存の `run`／command起動・待機・回収方針は変更しない。本guardは子processのdeadlineやprocess-tree隔離を追加するものではない。

## 有限上限と限界

選択file最大10,000、走査entry最大20,000、1file16MiB、合計128MiB、tree深さ64、相対path UTF8最大4,096bytes。64KiB chunkでhashする。これは受理する走査範囲を制限する値であり、CPU／RSS quotaや全gateの期限ではない。

解決済みworkspace rootと祖先、環境と実行toolを信頼する。全tree・全fileを同時に読むatomic snapshotではなく、照合間の変更後復元（ABA）や敵対的なraceを排除しない。個別fileの可視stat確認も完全なfilesystem隔離ではない。sourceが一致しても、外部toolや環境が同一だったこと、試験が網羅的だったこと、配布物が安全なことは保証しない。

## 観測と現在の状態

stage／mainの13契約はいずれも12成功・Windows link1 skip（`target/gate-source-freeze-main-contracts.log`）。workspace単発観測は436file／4,362,354bytes、capture約0.896秒／verify約0.272秒（`target/gate-source-freeze-workspace-observation.json`）。これは固定hostの1回の観測で、性能保証や将来の件数ではない。

初stageではWindows ctimeの意味差により失敗した記録を保持し、比較対象をdevice／inode／size／mtimeへ修正した後の結果と分ける。追加・削除・byte変化、拒否と上限、固定診断、失敗commandのstatus／note保持、help先行等を有限契約で確認する。

実guard統合gateはexit0（`target/gate-source-freeze-full-check.log`、controller16498も出力回収後exit0）。Rust610（ignore1）、native pure313（62 artifact／features空／normal WP1）、isolated binlog29、Python415（408成功・skip7）、認証3種のactual比較、影響するfresh DB4 mode各62、Clippy、WP／server Linux cross compile、license file観測218、実HTTPが成功し全fixtureを回収した。最終11checkerも全exit0（`target/gate-source-freeze-current-check-summary.json`、controller28105回収済み）。Linux cross compileはnative Linux実行ではない。

前段sanitize79の完成履歴を保持する。今回fresh再実測は `check.py` のdirect source差を受けたrotation／restart／alpha2 backendの計4 modeで、全8DB再測定ではない。mode内でMySQL通常DB baseline proofもbyteが変わるため、「4 mode＝更新JSON4個」とは扱わない。既binlog2／overlap／Maria snapshotの4proofは旧byteを保持した。通常Maria baselineも同byteだったが、SHA一致からwrite eventが無かったとは推測しない。

WP正式79 API／519,014とhooks572／dispatcher405は今回再比較せず、3proofのbyte同一とcurrent checkerで保持確認した。`target/gate-source-freeze-retained-input-identities.json` は選択した157 source／dependency／保持観測の一致を記録する。新しいWP API、DB契約、認証・UI policyは追加していない。checkpoint生成はdocs freeze後の別操作で、次workbench候補はmain未採用。
