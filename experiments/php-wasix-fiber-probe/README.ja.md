# PHP FiberのWASIX backend実験

PHP `f4bfae79d5c19028349082f707930cf0951d23b9`を対象とする独立P0実験。既存の[小C実証](../php-wasix-context-probe/README.ja.md)で専用stackの前提を確認したが、PHP Fiberの移植・製品互換性はまだ受け入れていない。

## 通常PHPの比較基準

```text
python experiments/php-wasix-fiber-probe/native_contract.py --record
```

[固定PHP fixture](fixtures/fiber_contract.php)は、動的4 Fiberの終了順/再帰local、入れ子のstart/resume、値/return、throw/catch/finally、suspend中Fiberのcycle GC/WeakReference、API errorの6ケースをJSONの型・値・traceで比較する。通常PHP8.5.12の実行で[基準記録](evidence/native-fiber-contract.json)を保存した。改変14件と不正JSON2件の拒否は、実PHPケース数へ加算しない。

CLIは`-n -d fiber.stack_size=2097152`を固定する。この2MiBは比較を揃える試験設定であり、製品defaultを決めるものではない。元ZIPと配布78ファイルの完全な集合/全SHA、source不変、10秒期限、所有processの回収を実行前後で確認する。共通verifierのWP3 source副検証はreceiptへ明記するが、この試験でWPは実行しない。公開記録へローカル絶対pathやobject IDを保存しない。

## backendの境界

試作用patchはNTS・WASI thread1・fresh WASI instance 1要求に限定する。ZendのVMstate/observer/transfer/Throwable/finally/GCを維持し、u64 context IDはprivate descriptorへ格納する。初回stackless entryとraw-switch直後のSP復元を使い、固定2slotの小C adapterをそのまま汎用Fiberへ持ち込まない。

WASIXの取消しは遅延するため、PHPの論理cleanupと物理Cstackの寿命を分ける。PHP request-endで物理領域まで解放せず、ZendMM request heap外に保持してWASI executor/instance終了へ委ねる案を検証する。guest内free/reuse、stack guard、persistent/ZTS、即時全資源回収、OOMの発生時点は別の確認事項である。native基準の成功をpatched PHPの成功へ付け替えない。

## 独立ビルドと実行経路

```text
python experiments/php-wasix-fiber-probe/build.py --validate-only
# Linuxの固定source/tool準備後
python experiments/php-wasix-fiber-probe/build.py --record
python experiments/php-wasix-fiber-probe/fiber_check.py --record
```

[patch](patches/0001-nts-fresh-wasi-zend-fibers.patch)はconfigure.ac、Zend/zend_fibers.c、新規assemblyの3ファイルに限定し、全対象の元blob/SHAと適用後の全SHAを照合する。既存CGIのPHP checkoutは流用せず、`--enable-rp-wasix-fibers --disable-zts`でCLIだけを作る。native PHPはstub生成用build toolであり、実行fallbackではない。

64bit-long用scriptの`-flto`を消したとは扱わず、verified wasixccのcompiler post flagsで`-fno-lto`を後置する。検証を有効にしたBinaryenで、最初の試作では`-O3`を使わない。raw/final各128MiB、configure600秒/make1200秒/optimizer300秒、各commandの合計出力16MiBで制限し、失敗時は所有groupを停止してleaderを回収する。

Rustの`--fiber-contract`は専用build receipt/生成物を照合し、元の公式CLI artifactを読む前に分岐する。compile前に最終Wasmの3定義関数・ABI・非alias、mutable i32 SP、bootstrap address、7/9命令switch、6命令entryを検査する。parser専用の正常2形と危険な79変形の拒否はWindowsで確認したが、これはPHP実行ではない。

Linuxの[独立CI](../../.github/workflows/php-wasix-fiber.yml)は両compilerで生成物を作り、compile1回・fresh環境3回で通常PHPの同じ6ケースを厳密比較する。所有Rust processはcompile込み600秒で回収する。失敗時も生出力と失敗phaseを保持し、JSON記録だけをアップロードする。[manifest](manifest.json)のstatic verificationはpatch起案時点の観測であり、実ビルド・実行は下記の別receiptで判定する。

source `a05a2ca`の初回CI `41320223325`は[Cranelift](evidence/ci-41320223325-cranelift/provenance.json)・[LLVM](evidence/ci-41320223325-llvm/provenance.json)ともtool/source/patch照合を通過したが、configureのC compiler確認で停止した。PHP make・module生成・guest実行へは進んでいない。adapter専用exportはconfigureの小さいconftestには存在しないため、指定をmake_cli時だけへ限定して再診断する。最初の記録にはconfig.log内の実link診断が無く、初回の具体的なlinker error自体を確認したとは主張しない。次のfailure時にはその限定excerptも記録する。

## 固定6ケースの実行結果

source `24c0b57`の[Cranelift記録](evidence/ci-41320732582-cranelift/provenance.json)と[LLVM記録](evidence/ci-41320732582-llvm/provenance.json)は、PHPの実ビルド、最終Wasm検査、Linux27 unit、6ケースをfresh3環境で比較する診断がすべて通った。最終Wasmは各39,082,257Bで、9命令switch/6命令entryとbootstrap addressの一致をcompile前に検査した。複数Fiberの終了順、値、例外/finally、suspend中cycle GC/WeakReference、API errorのnative値/型/traceを変更していない。

両jobの最終SHAは異なるため、bit再現buildとはしない。1つのcompile済みmoduleをcloneして3つの要求状態を作った成立実証であり、製品性能・全PHP/WP互換性の値でもない。通常PHP実行やコンパイル済みWasmバイナリをimporterが取得・再実行した記録へ読み替えず、固定ZIP・元Git入力・同jobの生成物/生出力を照合した履歴として保持する。

この試作の取消し完了、要求中に論理破棄されたPHP状態の寿命、stack guard、guest内free/reuse、OOM/INIの追加条件、全thread/FD/RSS回収、配布権利、正式3環境は未受入である。通常PHPの6ケース成功でそのgateを置き換えない。
