# RP開発者向け: CGI runtimeのreadonly互換性差分

調査日: 2026-11-02（JST）。対象実装は `2649132e3fd082a01d13ffe4850cc268a3a62889`、CI runは `41328695777`。本書は観測結果と修正候補の報告であり、新ABIの仕様決定や修正実装の受入ではない。

## 結論と原記録

Cranelift・LLVMの両backendで、それぞれfreshな3要求がPHP CGI処理を完了した。しかしreadonlyケースにはnativeとの差分が2件あり、全6ケース比較は失敗のままとする。基本処理、例外、Fiber、WP pure hooks、通常IOの他5ケースは各要求でnativeと一致した。

原記録と、その出力を再解析した派生記録は次のとおり。

| backend | 元のruntime実行記録 | 元のstage記録 | 派生した差分・来歴 |
|---|---|---|---|
| Cranelift | [linux-cranelift-cgi-runtime-contract.json](evidence/ci-41328695777-cranelift/linux-cranelift-cgi-runtime-contract.json) | [ci-stages.json](evidence/ci-41328695777-cranelift/ci-stages.json) | [provenance.json](evidence/ci-41328695777-cranelift/provenance.json) |
| LLVM | [linux-llvm-cgi-runtime-contract.json](evidence/ci-41328695777-llvm/linux-llvm-cgi-runtime-contract.json) | [ci-stages.json](evidence/ci-41328695777-llvm/ci-stages.json) | [provenance.json](evidence/ci-41328695777-llvm/provenance.json) |

比較基準は固定Windows PHP 8.5.12の[native CGIレシート](evidence/native-cgi-runtime-contract.json)。実行fixtureは既存runtime fixtureの入口guardだけをCGI用に変更し、6ケース本文のbyte一致を検証したものである。Windows readonly属性はこの固定操作の比較基準であり、Unix mode bitや製品の権限境界と同一とは扱わない。

## 両backend・全3要求で共通した結果

対象要求は `normal_first`、`normal_repeat`、`normal_final`。readonlyケースの差分は各要求で次の2件だけだった。

| 比較箇所 | native | Wasm |
|---|---|---|
| `/cases/readonly_io/value/writable` | `false` | `true` |
| `/cases/readonly_io/warnings/0/message` のerrno文言 | `Permission denied` | `Operation not permitted` |

警告全体は同じ `file_put_contents(/scratch/readonly.txt): Failed to open stream: ` を先頭に持つ。差分記録は省略・切詰めされていない。readonlyのPHP観測は `readable=true`、`write_result=false`、`unchanged=true` であり、書込拒否自体はnativeと一致する。

Rust側のscratch FSでも各要求について次を確認した。

- 通常ファイルへの書込は12byteで、最終内容は `ordinary\0\xff\r\n` と一致。
- readonlyファイルは `readonly fixture\n` の17byteを維持し、PHPの実読取結果とも一致。
- readonlyへのmutating open拒否を4件記録。これはhost FSのopen拒否回数であり、PHPからの書込関数呼出しが4回あったという意味ではない。
- scratch handleは4件closeされ、要求終了後のopen handleは0件。

このhandle観測はscratch FSの計数範囲であり、全WASIタスク・全プロセス資源の即時回収を証明しない。要求の `ok=true` はWasiRunnerの正常終了を表すだけで、native互換性の成功ではない。原レシートの `outcome=execution_failed`、`runtime_contract_passed=false`、CIの `runtime_contract=failure` を維持する。WP全体のbootstrap、実DB、製品採用も未検証である。

## sourceから説明できる三つの層

### 1. PHPの組込み関数とaccess照会

固定PHP source `f4bfae79d5c19028349082f707930cf0951d23b9` のplain-file処理では、`is_writable()` は `FS_IS_W` から `VCWD_ACCESS(path, W_OK)` を呼ぶ。非Windowsのmacro／virtual CWD処理は最終的にlibcの `access()` へ進む。plain-fileのこの経路では、後段のstat結果だけを修正しても組込み関数の返り値は直らない。[PHP filestat.c](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/ext/standard/filestat.c#L785)、[CWD macro](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/Zend/zend_virtual_cwd.h#L295)、[virtual_access](https://github.com/wasix-org/php/blob/f4bfae79d5c19028349082f707930cf0951d23b9/Zend/zend_virtual_cwd.c#L1345)

後述のrelease tag source `09503b230e8acc721c1e013ca28a41bf149e4ee2` では、EH用Makefileはbottom-halfの `posix.c` とcloudlibc sourceを選択する。`posix.c` の `access()` はpathをpreopen fdと相対pathに解決し、`__wasilibc_nocwd_faccessat()` を呼ぶ。この関数は対象のfilestatで存在・種別を確認した後、`W_OK` を親directory fdの `fs_rights_inheriting` に `FD_WRITE` があるかで判定する。対象ファイル固有のmutating open拒否は照会しない。[Makefile-eh](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/Makefile-eh#L64)、[posix.c](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-bottom-half/sources/posix.c#L67)、[faccessat.c](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-bottom-half/cloudlibc/src/libc/unistd/faccessat.c)

このtag実装に基づけば、同じscratch内の通常ファイルを書ける親capabilityを持つ場合、readonlyファイルの照会も許可され得る。今回の `is_writable=true` と実open拒否の併存を説明できる。ただし、配布asset中の関数命令をこのsourceと照合したわけではないため、実assetへの説明にはその未確認部分が残る。

### 2. statのmode情報不足

固定Wasmer source `93bd021012baafda53df2030db3ce1758dc1e336` のvirtual-fs `Metadata` とWASI `Filestat` は、Unix mode／owner権限のフィールドを持たない。libcの `to_public_stat()` も `st_mode` にfiletypeを合成する処理であり、対象の書込許可を取得する経路ではない。[Metadata](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/virtual-fs/src/lib.rs#L682)、[Filestat](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/wasi-types/src/wasi/bindings.rs#L1169)、[stat_impl.h](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-bottom-half/cloudlibc/src/libc/sys/stat/stat_impl.h)

これは別の互換性上の制約である。今回のboolean差を「mode情報がないから」だけで説明せず、前項のaccessが親fdの継承capabilityを調べる経路と区別する。Metadataに情報を追加するだけでは、既存のguest accessはその情報を利用しない。

### 3. 実open拒否とerrno

Wasmer 7.2系の固定sourceでは `FsError::PermissionDenied` は `Errno::Perm` に変換される。tag側libcのopenは `path_open2` から返ったエラーをerrnoに保持する。strerrorの定義では `EPERM` と `EACCES` に別の文言があり、今回の警告差と整合する。[host変換](https://github.com/wasmerio/wasmer/blob/93bd021012baafda53df2030db3ce1758dc1e336/lib/wasix/src/fs/mod.rs#L2841)、[libc openat.c](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-bottom-half/cloudlibc/src/libc/fcntl/openat.c)、[strerror定義](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/libc-top-half/musl/src/errno/__strerror.h)

したがって、対象別の権限照会が正しくなっても実open拒否のerrno表現は別途検討が必要である。警告文だけを置換しても、この二つの経路の整合性は確保できない。

## 固定releaseとsourceの証拠範囲

調査時のGitHub APIで、使用中sysrootと次の対応を確認した。

| 項目 | 確認した値・一次資料 |
|---|---|
| release | [`v2026-07-03.1`](https://github.com/wasix-org/wasix-libc/releases/tag/v2026-07-03.1)。APIの `target_commitish` は `main` |
| tagの参照先 | [tag ref API](https://api.github.com/repos/wasix-org/wasix-libc/git/ref/tags/v2026-07-03.1) はcommit `09503b230e8acc721c1e013ca28a41bf149e4ee2` を直接参照 |
| 配布asset | [release API](https://api.github.com/repos/wasix-org/wasix-libc/releases/tags/v2026-07-03.1) のasset `465375751`、`sysroot-eh.tar.gz`、6,142,222byte、SHA-256 `8c54240afabda1106c19f284bdfff77f29932caa55f82756f87d01f69ffd50f4`。既存build pinと一致 |
| tagのRelease run | [28659258439](https://github.com/wasix-org/wasix-libc/actions/runs/28659258439)、同commit、tag push、success |
| mainのbuild run | [28658934346](https://github.com/wasix-org/wasix-libc/actions/runs/28658934346)、同commit、success |

固定tagの[release定義](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/.github/workflows/release.yml)はEH artifactをtarにしてreleaseへ公開する構成で、[build定義](https://github.com/wasix-org/wasix-libc/blob/09503b230e8acc721c1e013ca28a41bf149e4ee2/.github/workflows/build_cxx_sysroot.yml)は各variantをbuildする。これによりrelease・tag・runの関係は確認できる。

一方、コンテナ・action・tool取得にはfloatingな要素があり、使用assetの再現buildやobject単位のsource照合は行っていない。tagの参照先と配布物のbyte pinを確認したことを、asset全体のsource/build attestation達成と扱わない。本書のlibc内部経路の説明も、この区別を維持する。

## 第一候補と保留事項

第一候補は、Rustが管理する仮想FSポリシーを副作用なく照会する純粋な権限照会と、それを利用するguest access adaptationの組合せである。任意の実ホストpathやホストOSの権限を開放せず、要求に付与した仮想path・capabilityの範囲で判定する。既存PHPの `is_writable()` が通るaccess経路へ接続すれば、プラグイン側の呼出しを変えずに反映できる可能性がある。

実openも同じRust側ポリシーを適用し、拒否理由とerrnoを対応させる必要がある。権限照会だけを実装してwrite結果を推定しない。この候補は未実装・未受入であり、transport、ABI/frame、errno契約、対象操作、更新時の整合性はまだ固定しない。

次の方法は汎用修正にしない。

- **write-openによる照会:** truncate／createを使わなくても、device・FIFO等のopenには副作用や待機があり得る。access照会とopenの意味も同一ではなく、全pathへ展開する根拠がない。
- **警告文の置換・比較除外:** 実際の拒否理由を修正せず観測差を隠すため、現在の期待値と成功条件を維持する。
- **全EPERMをEACCESへ置換:** open以外も含むerrnoの意味を一律に変える。host APIが拒否理由を区別できるか、どの操作にどのerrnoを返すかを別途設計する。

受入には、通常IOが引き続き成功し、readonlyのPHP照会・実書込拒否・不変な実byte・errnoが同時に整合する証拠が必要である。既存の全体failureを変更するのは、その修正と再検証が完了してからとする。
