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
```

LLVMは実行checkerに`--llvm`を付ける。runtime比較の前に既存のWPソース準備が必要。Fiber/runtime比較はcompile済みmoduleから3つのfresh要求を作り、native CGIの既存6ケースと応答全体を照合する。PHP JSON・warning・native期待値を補正して合格にはしない。実権限照会のallow/deny、応答読了、invalid/IOエラーなし、scratch実書込み・readonly不変も検証する。providerのopen/close計数はRust VirtualFile単位であり、guest fd数ではない。

`cgi_check.py`は同じ3patch生成物で、既存のGET/Cookie・POST/raw body・404/2 Cookie/7B binaryの3要求を比較する。通常のCGI検証と同じ実行・検証処理を使い、固定生成物のloaderだけを替える。Fiber/権限照会の実行をこの3要求で証明したとは扱わない。

## 初回の実ビルド・実行結果

source `edc6b645aead72b00eb482994a58e69957879118`の[CI 41337652464](https://github.com/KANIKAMA-Carcinization/RecastPress/actions/runs/41337652464)は両backendで成功した。PHPの3patch build、最終assembly命令検査、Linux37単体テスト、Fiber6ケース×fresh3要求、runtime6ケース×fresh3要求が通過した。[Craneliftの原記録と元Git照合](evidence/ci-41337652464-cranelift/provenance.json)、[LLVMの原記録と元Git照合](evidence/ci-41337652464-llvm/provenance.json)を保存する。Windows36単体テストは別のローカル観測である。

両backendのruntime各要求では、権限照会5回（allow3・deny2）、input140B、応答25Bを全読了し、missing/invalid/IOエラーは0だった。通常12Bの書込・読出し、readonly17Bの不変、`is_writable=false`、実書込拒否、`Permission denied`のwarningが同時にnativeと一致した。権限照会のunderlying VirtualFileはopen/close各5、終了時active0。scratchは別に4handleをcloseしopen0だった。全WASI資源の回収証明ではない。

生成Wasmは両jobとも39,150,496Bだが、hashは別でありbit再現性は未証明。各jobのbuilderと実行checkerは同jobの実byteを照合した。native期待値・warning・PHP JSONは変更せず、元の2patch候補の2差分とfailureも保持する。

GET/POST/Cookie/binaryの追加`cgi_check.py`はこの初回CIには含まれない。既存3要求の共通検証を通す次のCIで確認する。

全WP bootstrap、wpdb・実DB、HTTP listener、全資源回収、stack guard/free/reuse、配布条件、3配備先の受入は未完了。
