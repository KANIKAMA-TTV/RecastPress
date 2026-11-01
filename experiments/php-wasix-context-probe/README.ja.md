# WASIX C contextの有限実証

PHP Fiberの移植に入る前に、固定Cプログラムでcontext切替と生存中のC stackを検証するP0実験。PHP・WP・実DBは実行せず、製品runtimeも選定しない。SDK APIだけの実測と、専用stackを管理する次の実装を区別する。

## 確認する動作

main/A/Bの3つの実行状態を作り、mainとA/B各4段の再帰frameにvolatile配列を置く。8周の固定切替でcanary・順序・初期の同時生存9範囲の非重複を確認する。context IDはu64のまま保持し、pointerへ変換しない。

続いてBの再帰frameをsuspendしたままAの4frameを通常unwindし、mainの深いstack書込み後にもBの値が保たれることを要求する。Aの終了済みframeのpointerは参照せず、初期range記録と現在の生存状態を区別する。その後inactive Aのdestroy、Bの追加2周・通常unwind・destroyを確認する。callbackの通常return、ENOTSUP、errnoの差分、stack重複、変数破損は失敗である。

destroyは取消しの予約であり、直後の全資源解放を主張しない。C++例外・任意位置中断・blocking hostcall・PHPのfinally/Fiber適合も別の検証対象である。

## ビルドと実行

```text
python experiments/php-wasix-context-probe/build.py --validate-only
```

Linuxの[専用workflow](../../.github/workflows/php-wasix-context.yml)は、[既存CGIのprepare](../php-wasix-cgi-build/prepare.py)で4資材のサイズ・SHAを照合して準備し、PHP本体やdepsのcheckout/buildをせず、このCだけをcompileする。legacy EH sysrootに合わせたraw出力を、validationを有効にしたBinaryenでformal EH/Asyncifyへ変換する。compile/optは各120秒、raw/final moduleは各64MiB以下で、timeout時は所有process groupを回収する。

```text
python experiments/php-wasix-cgi-build/prepare.py --record
python experiments/php-wasix-context-probe/build.py --record
python experiments/php-wasix-host-probe/context_check.py --record
```

Rustは固定`--context-contract`だけを受け付け、同jobのreceipt・C source・manifest・実Wasmを照合して1回compileする。公開APIによる非同期WasiRunnerを使い、host環境・host directory・networkを渡さず、stdinは閉じ、両出力を各64KiBに制限する。外側checkerは所有hostに60秒の期限を設け、失敗時にも取得できたraw JSON・出力・host errorを保持する。LLVM jobはCビルド後に別系統のhost LLVM22.1を準備し、`--llvm`で同じ条件を実行する。

[manifest](manifest.json)のSDK commitはcontext APIを読んだ参照sourceであり、取得済みsysrootがそのsourceからbuildされたattestationではない。4資材の取得物SHA、参照API source、今回Cのbuild/実行を分けて記録する。バイナリはGitやCI artifactへ再配布しない。

## SDK APIだけの実測

source `a0cbade`の[LLVM](evidence/ci-41310854007-llvm/provenance.json)と[Cranelift](evidence/ci-41310854007-cranelift/provenance.json)は同じ46,414BのWasmを実行し、8周の切替・初期9範囲の非重複・Aの4frame unwindまで成功した。その後のmainによるstack書込みで、suspend中のBのvolatile値が壊れた。両方とも`post_a_unwind_clobber`の`volatile-stack-canary-changed`でexit1となり、EOFと所有hostの回収を確認した。Cのcompile/optやhost非同期設定の失敗と混ぜない。

[importer](import_ci_evidence.py)は元Gitの入力hash・ZIPのbyte/SHA・tool/module・rawJSON・19イベントの途中traceを照合し、原記録の失敗をそのまま保存する。この固定ケースではSDK APIの素結線だけではstackを保てなかった。SDK全般やPHP Fiberの実装可否をこの負例だけで決めない。

## 専用stackを使う次の実装

元のC fixtureは変更せず、fixture objectだけにheaderを強制includeし、create/switch/destroyを小adapterへ渡す。A/Bに各64KiBの16B整列heap stackを用意し、初回entryはassembly thunkがC prologより前にstackを切り替える。switchはWasm localへSPを保存し、raw importから通常復帰した直後に戻す。errnoの処理はその後のCで行う。

最適化後のWasmもRustの公開parserで検査し、export/type/importの同一性、shimの7/9命令の閉じた形、thunkの6命令、復帰前の余分なcallやlinear-memory書込みが無いことを必須にする。errnoをWasm localへ置く9命令形でも、old SPと別のlocalであることを検査する。正常2形と危険な29変形をguest実行なしで確認した。生成物名も`context.managed.wasm`へ分ける。2つのheap stackは実験の終了まで保持し、destroy直後のfree/reuse、trap/cancelからの通常復帰、即時全資源回収は主張しない。

source `1462082`の[Cranelift](evidence/ci-41314440077-cranelift/provenance.json)と[LLVM](evidence/ci-41314440077-llvm/provenance.json)は同じ46,942Bのmanaged Wasmを実行し、19項目・22イベントすべてに成功した。最終Wasmの9命令shim/6命令entryを検査し、元と同じC fixtureでA終了後のB保持、A破棄後のB継続、errno・破棄を確認した。PHP Fiber、stack guard、free/reuse、全資源即時回収の受入へは広げない。
