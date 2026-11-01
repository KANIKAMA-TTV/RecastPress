# v0.5：Windows EH ABIの最小compile診断

PHPもWASIXも使わず、Wasmer 7.2.0／Cranelift 0.133.0で2つの固定WATをコンパイルする。インスタンス化・guest実行はしない。engine不具合を小さく再現する診断であり、PHP互換性の合格試験ではない。

```text
cargo run --offline --locked --manifest-path experiments/php-wasix-eh-compile-probe/Cargo.toml --target-dir target/php-wasix-eh-compile-probe
```

`non-eh.wat`は正常compileした。`try-table-call.wat`は、`try_table / catch_all_ref`内でimport関数を呼ぶだけでWindowsFastcall例外ABIのclobbers未実装panicになった。[Windows実測](evidence/windows-x86_64-msvc.json)へ、別々の所有PID・実行結果・ソースと実行バイナリのhashを保存する。

PHP成果物で起きた`machinst/lower.rs:1107`のindex panicと、最小例の`isa/x64/abi.rs:903`のpanicは別位置である。同一panicを完全に縮小できたとは言わず、Windows例外ABIの問題を示す別の小さな再現として扱う。

標準エラーの絶対user/cacheパスは公開証跡から除き、生ログとビルド成果物はGit対象外へ置く。通常のCompileError・panic・timeout・回収未確認を区別する。`diagnostic_complete`と終了コード0は診断の完了であり、EH compile成功や製品対応を意味しない。
