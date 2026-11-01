# Rust管理権限とPHP組込みの接続実験

v0.5 P0の独立候補。既存のCGI/Fiber候補で観測した[readonlyの2差分](../php-wasix-cgi-runtime-probe/findings.ja.md)を、nativeの期待値を変えずに再比較する。製品runtime・公開ABI・全WordPress互換性の採用決定ではない。

## 変更範囲

固定PHPソースへ既存のFiber patch、CGI prefork拒否patch、今回のmanaged-access patchを順に適用する。ビルド手順と固定ツールは[結合候補](../php-wasix-fiber-cgi-probe/README.ja.md)から継承し、専用のcheckoutとWasmを生成する。

WASI限定のPHP組込み照会から、privateな`/rp-p0-fs/access`を通じてRustの実権限を問い合わせる。対象は固定fixture、3つのWP pure hookソース、scratchで、照会はファイルを変更しない。通常の書込み可否は引き続きRust VFSが決める。元のopenがEPERMで失敗し、同じpathとmodeの照会が明示的な拒否を返した場合だけEACCESへ補正する。

要求は`RPA1`・mode 1B・UTF-8 path長2B little endian・絶対path。応答は`RPR1`・status 1B。path最大4096B、要求ごと最大64照会。明示的なunmanaged応答だけが従来のaccess処理へ戻る。通信失敗や不完全応答をunmanagedとして扱わない。この形式は実験内部用で、製品ABIではない。

## 検証

```text
python experiments/php-wasix-managed-access-probe/build.py --validate-only
python experiments/php-wasix-host-probe/unit_check.py --record
```

Linuxの専用CIはソース・ツール・3patch・最終Wasmを照合してから、Cranelift/LLVMそれぞれで次を実行する。

```text
python experiments/php-wasix-managed-access-probe/build.py --record
python experiments/php-wasix-managed-access-probe/managed_check.py --fiber --record
python experiments/php-wasix-managed-access-probe/managed_check.py --record
python experiments/php-wasix-managed-access-probe/cgi_check.py --record
python experiments/php-wasix-managed-access-probe/rpc_check.py --record
```

LLVMは実行checkerに`--llvm`を付ける。runtime比較の前に既存のWPソース準備が必要。Fiber/runtime比較はcompile済みmoduleから3つのfresh要求を作り、native CGIの既存6ケースと応答全体を照合する。PHP JSON・warning・native期待値を補正して合格にはしない。実権限照会のallow/deny、応答読了、invalid/IOエラーなし、scratch実書込み・readonly不変も検証する。providerのopen/close計数はRust VirtualFile単位であり、guest fd数ではない。

`cgi_check.py`は同じ3patch生成物で、既存のGET/Cookie・POST/raw body・404/2 Cookie/7B binaryの3要求を比較する。通常のCGI検証と同じ実行・検証処理を使い、固定生成物のloaderだけを替える。Fiber/権限照会の実行をこの3要求で証明したとは扱わない。

## 初回の実ビルド・実行結果

source `edc6b645aead72b00eb482994a58e69957879118`の[CI 41337652464](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41337652464)は両backendで成功した。PHPの3patch build、最終assembly命令検査、Linux37単体テスト、Fiber6ケース×fresh3要求、runtime6ケース×fresh3要求が通過した。[Craneliftの原記録と元Git照合](evidence/ci-41337652464-cranelift/provenance.json)、[LLVMの原記録と元Git照合](evidence/ci-41337652464-llvm/provenance.json)を保存する。Windows36単体テストは別のローカル観測である。

両backendのruntime各要求では、権限照会5回（allow3・deny2）、input140B、応答25Bを全読了し、missing/invalid/IOエラーは0だった。通常12Bの書込・読出し、readonly17Bの不変、`is_writable=false`、実書込拒否、`Permission denied`のwarningが同時にnativeと一致した。権限照会のunderlying VirtualFileはopen/close各5、終了時active0。scratchは別に4handleをcloseしopen0だった。全WASI資源の回収証明ではない。

生成Wasmは両jobとも39,150,496Bだが、hashは別でありbit再現性は未証明。各jobのbuilderと実行checkerは同jobの実byteを照合した。native期待値・warning・PHP JSONは変更せず、元の2patch候補の2差分とfailureも保持する。

## CGI 3要求の回帰確認

追加の`cgi_check.py`はsource `0f994c31ccf542468460b48d17326a8e212dffcf`の[CI 41340215839](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41340215839)で両backendとも通過した。[Cranelift](evidence/ci-41340215839-cranelift/provenance.json)・[LLVM](evidence/ci-41340215839-llvm/provenance.json)の元Git照合と生JSONを保存する。GET/Cookie、POST/raw body、404/2 Cookie/7B binaryの3要求すべてが、元のnative CGI基準と一致した。既存のSCRIPT_FILENAMEの宣言済み正規化だけを維持し、JSONの型・値、status、Cookie順序、binary本文の期待値は変えない。stdoutの実byteからCGIを再解析し、保存したresponseと一致することも検証した。

このrunではLinux37単体テスト、Fiber/runtime各6ケース×fresh3要求も再度通過。3patch profileの各job生成物を実行前後に照合しており、job間のbit再現性を主張しない。

## 専用RPCのCGI統合

既存の[private RPC診断](../php-wasix-host-probe/rpc_check.py)の4ケースを、同じ3patch候補のCGI入口へ接続する。元のNORMAL/INCOMPLETEのPHP本文はbyte完全一致で維持し、固定CGI guardとContent-Type、閉じたcase選択だけを加える。1compileから4つのModule.cloneを渡し、要求ごとに新しいWASIとRpcFsを作る。

受入条件はnormal_first、normal_second、incomplete_ignored、normal_recoveryの順に維持する。不完全frameはPHPが正常終了しても`ok=false`・`rpc_valid=false`であり、close時の拒否理由を保持する。正常3要求ではcache miss→set→hit・固定DB行・reopenを確認し、要求local cacheとnonceを分離する。CGI本文内のRPC風binary literalはstdoutのまま保持し、callbackとしてdispatchしない。

共通CGI処理はexit・shutdown・EOF・quotaを確認して完全な応答を解析する。RPC側はその実本文とRpcFs.finishの両方を検証し、取得失敗時も必ずfinishする。新checkerはraw stdout・response・PHP JSON・callback snapshotを直接照合し、元CLIのreportへ変換しない。外側のbuild1200秒・実行600秒、出力上限・cleanup失敗時の成功取消も維持する。

実装とWindows36単体テスト、リリースビルド、固定本文の構文確認・84入力の純粋照合が通過。実CGIでの4要求は次のLinux CIで確認する。実DB・標準wpdb・永続cache・製品ABIは実装していない。

全WP bootstrap、wpdb・実DB、HTTP listener、全資源回収、stack guard/free/reuse、配布条件、3配備先の受入は未完了。
