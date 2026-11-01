# 固定PHP依存archiveの由来確認

2026-11-02。P0の配布条件確認に向けた、固定リポジトリのメタデータ調査。既存の[リンク観測](README.ja.md#raw-linkに入ったarchiveの観測)とは別に、archiveごとのsource/build/license対応を確認する。

[記録した一覧](evidence/dependency-source-inventory.json)は、`wasix-org/php-wasix-deps`のcommit `b77ecbede97aba2df32c86eaeb54ff9f5e44f543`に対する[再帰tree API](https://api.github.com/repos/wasix-org/php-wasix-deps/git/trees/b77ecbede97aba2df32c86eaeb54ff9f5e44f543?recursive=1)の観測である。APIは6005 entry、`truncated=false`を返した。`lib-eh/*.a`と`pgsql-eh`配下のarchive計28件のpath・サイズは、source照合済みの[Cranelift](evidence/ci-41323679290-cranelift/provenance.json)・[LLVM](evidence/ci-41323679290-llvm/provenance.json)両CIの事前ビルド済みarchive一覧と一致した。SHA-256は両builderが記録した値を参照する。今回archiveを取得・再hash・再ビルドしたという意味ではない。

固定treeのトップレベルfileは`README.md`だけであり、rootのbuild script・Makefile・Cargo.toml・`.github` workflowは見つからなかった。[README](https://github.com/wasix-org/php-wasix-deps/blob/b77ecbede97aba2df32c86eaeb54ff9f5e44f543/README.md)はビルド済みライブラリの集積で、多くは同organizationのリポジトリからビルドしたと説明するが、archiveごとの元commitや再ビルド手順は示していない。参照先もPHPの`8.3.4-wasix` branchで、今回の固定PHP 8.5.12 sourceとの個別対応証拠にはならない。

treeにはcomponent別include領域と、SQLite/zlib等のライセンス名文書が含まれる。ファイル名による抽出を、全ファイル本文のライセンス監査や、archiveの元sourceを特定した証拠にはしない。特に`libicrt.a`・`libwasix_rust_libs.a`・`libwasix_sendmail.a`について、同名で見つかるpathは`lib`/`lib-eh`のarchiveであり、名前から依存crate・version・licenseを推定しない。

次に必要なのは、各archiveのビルド元commit、patch、toolchain・recipeとの対応と、含まれるcomponentのlicense/notice一覧である。取得したsourceとarchiveが対応する証拠も必要になる。28件中6件のlive map観測・17件の抽出履歴・5件の未観測/曖昧という分類は維持し、未観測分を監査対象から外さない。現時点で配布条件確認済み・製品採用済みにはしない。

後続の[内部メタデータ確認](../php-static-archive-probe/README.ja.md)では3archiveを実際に取得・再hashし、このtree観測・既存CIのbyte pinと照合した。Rust member名とregistry内の版文字列候補を記録したが、前段のtree照合とは区別し、完全なsource/build/license対応の達成にはしない。
