# 製品用PHP実行基盤の比較材料

調査日：2026-11-01。以下は調査開始時点の一次ソース読取りメモであり、当時は候補をビルド・実行していなかった。同日後続の実測は末尾の追補と個別実験へ分ける。製品採用、配布条件、Windows対応の確定を示さない。Node.jsなしでRustから直接ホストする方針と、比較用PHP 8.5.12・WP 7.1.3を維持する。

## 現在の実証との区別

現在のPHP 8.2.6 WASIp1成果物は、閉じたfixtureでRustホストの能力を確認するために使っている。PHP全体の互換性や再現可能な製品ビルドは未確認。

WLRの[固定レシピ](https://github.com/webassemblylabs/webassembly-language-runtimes/tree/5a40c308763309d227dd1dd85fe3e9397282c6d9/php/v8.2.6/patches)には、ビルド対応以外の挙動変更もある。Zend内部のbailout/setjmp系の変更、WASIでのFiber無効化、stat成功後のread/write/execute権限判定をtrueへ変えるパッチを、8.5へそのまま移す前提にしない。Zend内部の変更を「PHP言語のtry/catch全体が無効」と解釈しない。このレシピと現在の取得バイナリが同一ビルドであることを証明したわけでもない。

## 比較候補

| 候補 | 固定できる材料 | 未実証・必要な作業 |
| --- | --- | --- |
| WASIX PHP 8.5.12 | [PHP commit f4bfae79](https://github.com/wasix-org/php/tree/f4bfae79d5c19028349082f707930cf0951d23b9)、[公式CI](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/.github/workflows/wasix-pr.yaml)、Wasmer 7.2.0 | WASIX拡張ABIのため現Wasmtime/WASIp1ホストへ差し替えられない。RustのWasmer/wasmer-wasixで別実験を作る。現レシピはCLI・CGI無効なので、要求SAPIとCGI、RPC、回収を検証する。 |
| PHP 8.5.12の標準WASIp1移植 | [WASI SDK 34のsetjmp資料](https://github.com/WebAssembly/wasi-sdk/blob/5a0bf653a1a06e1c18867567c5937006d3394a69/SetjmpLongjmp.md) | 標準Wasm EHを用いた移植と、EH対応engineの組合せが必要。現[Wasmtime 36.0.19の変換器](https://github.com/bytecodealliance/wasmtime/blob/v36.0.19/crates/cranelift/src/translate/code_translator.rs)には例外命令の未実装箇所があり、SDKだけの更新では成立しない。 |
| WordPress PlaygroundのPHP 8.5 | [固定ビルド](https://github.com/WordPress/wordpress-playground/blob/a4dbcd0bf65a82e10a5d3821f5023aeee81097e6/packages/php-wasm/compile/php/Dockerfile)、[SAPI](https://github.com/WordPress/wordpress-playground/blob/a4dbcd0bf65a82e10a5d3821f5023aeee81097e6/packages/php-wasm/compile/php/php_wasm.c) | Emscripten固有JS、Asyncify/JSPI、動的リンクへ依存する。現成果物をWASIp1として直接実行する案ではなく、SAPIや個別パッチの比較資料とする。 |

## 再現ビルドとRust版

WASIX PHPの公式CIは、wasixcc v0.4.4、sysroot v2026-07-03.1、LLVM 21.1.206、Binaryen 130を指定する。[構成スクリプト](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/wasix-configure-eh.sh)と、[依存commit b77ecbed](https://github.com/wasix-org/php-wasix-deps/tree/b77ecbede97aba2df32c86eaeb54ff9f5e44f543)の元ソース・ビルド方法も固定対象にする。CIのNode/pnpm使用を製品のNode実行依存と混同しない。

[Wasmer 7.2.0](https://github.com/wasmerio/wasmer/blob/v7.2.0/Cargo.toml)のMSRVは1.93であり、現在のworkspace MSRV 1.94を保つ比較の開始点にできる。Wasmer 7.5.0は1.96、[Wasmtime 50.0.1](https://github.com/bytecodealliance/wasmtime/tree/c307ec81de02852d815e9849f7c89d955c03c30d)は1.97.0を必要とする。後者は正式EHを提供するが、現engineの単純差替えとして扱わない。

次の独立実験は、Wasmer 7.2.0とWASIX PHP 8.5.12の組合せを第一比較候補とし、Windowsでの実行、PHPの異常回収、Fiber、権限照会、要求I/Oと専用RPCを確かめる。WASIX側にも[条件付きFiber失敗](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/Zend/zend_fibers.c)があり、版が新しいことだけで互換性を認定しない。製品採用前には、実際の依存とビルドの監査、通常PHPとの比較を行う。

## 同日追補：成果物とRustホストの実測

[実物検査](../php-runtime-candidate-probe/README.ja.md)で、公式runの`php-wasix-64`をZIP・WasmのSHAで固定した。CIの中間設定はlegacy EHだが、build scriptが`--emit-exnref`を適用し、最終バイナリは標準EHだった。WASIX拡張とshared imported memoryを持ち、現WASIp1ホストへの直接差替えはできない。

[Rust WASIXホスト](../php-wasix-host-probe/README.ja.md)のWindows Craneliftはmodule compileでpanicし、PHPを実行できなかった。Linux LLVMはRust 1.94.0でビルドし、PHP 8.5.12の固定出力と整数幅8Bを観測した。nativeの6ケース比較基準は別に保存したが、このLinux基本実行が例外・Fiber等の一致まで示すわけではない。

初回正式配備先はLinux Docker Compose・Railway・Lightsail Linuxであり、Windows nativeは元の初回対応範囲に含まれない。Windows不具合の診断と、Linux上でのP0成立実証を区別する。CLI版からWPの要求SAPIへつなぐ入口、権限照会、要求I/O・RPC・回収・配布監査を引き続き検証する。

## 同日追補：Fiber差分の原因

[言語契約の実測](../php-wasix-host-probe/evidence/linux-llvm-41295645988/case-comparison.json)では基本値・例外がnativeと一致し、Fiberは失敗した。固定PHPの[configure](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/configure.ac#L1199-L1307)はwasm用ASMを持たず、`__wasi__`ではucontextの検査・defineも省く。SDKも[ucontext headerを除外](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/Makefile#L488-L500)する。`--disable-fiber-asm`やhostのthreading設定だけで直る問題ではない。

[WASIのinit FAILURE分岐](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/Zend/zend_fibers.c#L418-L464)と、startがsuspendへ到達せずresumeで状態エラーになる流れは、今回の`Cannot resume a fiber that is not suspended`と整合する。SDKの[context API](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-bottom-half/headers/public/wasix/context.h)へPHPを接続するには、Zend backendまたはucontext adapterの移植と、[Wasmerのasync/context条件](https://github.com/wasmerio/wasmer/blob/v7.2.0/lib/wasix/src/state/context_switching.rs#L113-L159)を含む検証が必要で、実装規模は未確定である。

候補採用を保留し、通常PHPの期待値を維持する。同じZendを使うCGIビルドだけではこの差分を修正しない。新SAPI・module再利用・資源制御を独立して進め、互換範囲を黙って削減しない。

## 同日追補：guest演算予算の次の実証

以下はWasmer固定commit `93bd021012baafda53df2030db3ce1758dc1e336`のsource調査で、調査時点では依存追加・実行は未実施だった。後続の[小WAT実測](../php-wasix-host-probe/README.ja.md#小watの演算予算と有限のテスト実行)を区別して保存する。[Metering](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/middlewares/src/metering.rs)にはsafeな初期予算設定・残量照会・再設定の公開APIがあり、[LLVM](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/compiler-llvm/src/config.rs)もmiddlewareを通す。予算はinstance globalへ入り、同一moduleの新instanceでは初期値に戻る。一つのMeteringを別moduleのcompileへ流用するとpanicするため、小WATとPHPには別engineを用意する。

[instance setup callback](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/wasix/src/runtime/mod.rs)で開始値を照合し、recycle時は公開instanceとstoreから実`Remaining`／`Exhausted`を読む案とする。setupは`Instance::new`後なので、Wasm start sectionのない固定moduleだけを先行対象にし、compile時の初期予算も省かない。LLVM＋formal EH＋Meteringの組合せは未実証であり、小WATの正常→無限loop→正常とthrow/catch領域を先に確認してから、別engineの固定PHPへ進む。

これはWasm演算の重み付き予算で、時間・native命令・PHP opcodeの上限ではない。hostcall・待機・compileを止める保証もなく、外側の所有process期限とmemory制約を残す。trap文言や非zero終了だけで予算による停止と判定せず、実`Exhausted`、EOF・回収、次要求の初期予算を成功条件にする。PHPのfinallyや通常shutdownの完走も前提にしない。

## 同日追補：compilerのライセンス宣言

`wasmer-compiler-llvm=7.2.0`はCargoの`license`文字列ではなく別LICENSEを持ち、[固定一次ソース](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/compiler-llvm/LICENSE)はBusiness Source License 1.1で、production利用の追加grantにSponsorship条件を記している。変更先はMPL 2.0、Change Dateは公開から4年との記載である。Cargo.lockと一致する配布crateのLICENSEも照合した。LLVM native toolchain本体のライセンスと、このWasmer compiler crateの条件を混同しない。製品採用・配布可否の判定は未了とする。

比較候補の`wasmer-compiler-cranelift=7.2.0`はMIT、`cranelift-codegen=0.133.0`は`Apache-2.0 WITH LLVM-exception`を宣言する。これだけでbundle全体の監査完了とせず、Linux Craneliftを同じPHP artifact・言語契約・資源実験で独立比較する。Windowsのcompile失敗をLinuxの失敗と推定せず、LLVMの成功を他backendへ移さない。

## 同日追補：Fiber移植前のC context実証案

未実装・未実行の次段階である。固定SDKの[公開header](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-bottom-half/headers/public/wasix/context.h)はu64のcontext IDと`void(void)` callbackを用い、userdata/stack引数や公開current-context getterはない。main IDは`wasix_context_main` macroを使い、A/BのIDを返却値のまま保持する。wasm32 pointerへIDを詰めない。Rustは既存Runnerのasynchronous threadingと公開`supports_async()`の確認を使う案で、利用側のunsafeや手動のprivate Store初期化は不要である。

PHP backendを書き換える前に、main/A/Bのvolatile配列と深さ4の再帰frameを同時に生かし、固定8周の切替・canary・順序・生存address範囲の非重複を確認する。callbackはmainへswitchしてsuspendし、main側がdestroyする。[destroy](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/wasix/src/syscalls/wasix/context_destroy.rs)は取消しの予約なので、A破棄後にBを実行してexecutorを進め、即時解放を主張しない。内容が復元されてもaddressが重なる場合は独立したC stackの証明としない。

既存の固定wasixcc/sysroot/Binaryenを使い、legacy EH sysrootでstatic-main Cをcompileして、検証を有効にした`wasm-opt --emit-exnref --asyncify`へ渡す案とする。Node/native PHP/Wasmer CLIは介さない。既存wrapperとRust async実装の読取りでは線形`__stack_pointer`の切替を確認できておらず、実測前にPHP Fiber適合へ進めない。任意位置中断、blocking hostcall、C++例外、全資源即時解放はこの有限C fixtureでも別の確認事項である。
