# Prepared bundle のbyte identity receipt候補

listener を起動せず receipt identity と bundle admission の集計を確認する one-shot CLI は [`check-package <bundle> <receipt>`](package-check-development.ja.md) を参照。

後続実測（2026-10-29）：receipt発行とpublic `--receipt` CLI接続、両DB由来の固定A/B snapshot、mismatch候補のlisten前拒否と既存sealed世代保持を、`target/wp-bcrypt-and-alpha-publication-full-check.log`で確認した。全体gateと8 fixture各45契約が成功し全子を回収。詳細は[α経路](alpha-publication-development.ja.md)、固定例は[起動手順](running.ja.md)を参照。以下のlibrary単体対象検証に続く結果であり、署名／認可／live公開を追加するものではない。

2026-10-29。既存v1／v2 bundleのbytesを識別する実験用sidecarを `recastpress_server::bundle_receipt` へ追加した。ファイル／IPCの製品方式、新しい公開承認・signature方式・世代切替protocolは採用しない。既存bundleのformat／admission・PreparedSite・Revision・sealed routerは維持する。

## Receipt schemaとAPI

schema1は `{format_version: 1, payload_bytes: u64, sha256: lower64hex}` の3fieldだけである。発行結果は決定的なcompact JSON＋LF。受理時はJSON structの厳密deserializationによりduplicate／unknown／欠落field・型違いを拒否し、versionは整数1、digestは正確な64 byteの小文字ASCII hexだけを受ける。receipt入力は1KiB、payload入力は既存8MiBに制限し、それぞれ1 byte sentinelで超過を検出する。

- `receipt_from_payload(&[u8]) -> Result<Vec<u8>, BundleReceiptError>`: payload boundと既存strict bundle admissionを通過した場合だけreceiptを発行する。
- `receipt_from_file(&Path)`: 共有するbounded single readの後、同bufferへ上記を適用する薄いwrapper。
- `verify_payload(payload_bytes, receipt_bytes) -> Result<PreparedSite, BundleReceiptError>`: bound／schema／長さ／SHA256を確認後、同一のborrowed payload bufferへ既存admissionを適用する。
- `load_verified(payload_path, receipt_path)`: explicitな2pathを各1回読む。receiptを先にbounded読取り・構文確認し、payloadは既存shared readerで1回読んだowned bufferをhash確認とadmissionの両方に使う。

receipt入力はfield順序や通常のJSON空白を許すが、payload identityは元bytesの長さとSHA256である。payloadのJSON空白・key順序変更もidentityを変える。同じpage内容であることやHTTP ETagと同一の意味ではない。receipt自体へbundle formatの別欄は追加せず、payloadのv1／v2は既存loaderが判定する。

## 同一bufferと既存admission

`bundle.rs`は既存single-open・8MiB＋sentinel読取り部分だけをprivate shared helperへ抽出した。既存 `load`／`parse`／`check` の署名と診断は維持する。receipt側は原payloadの再open／再readや、hash後のpath再解決を行わない。identity不一致ではadmissionを実行せず、identity一致でも不正JSON・target／MIME／status／redirect／duplicate等は既存admissionが拒否する。

両input内のURL・filename・referencesを解決する機能はない。ファイルを読むのは明示されたpayloadPathとreceiptPathだけである。読取り後のファイル変更・削除は、既に所有したPreparedSiteのbytesを変えない。一方、読取り中の変更、payload／sidecarのatomic pair更新、永続writeの耐障害性、symlink／filesystem identity、mount／TOCTOU全般、file watcherは新たに保証しない。

SHA256と長さは一致したbytesの識別だけである。両ファイルを書き換えられる主体はmatching pairを発行できる。signature、producer認証、公開許可、PII除去、anti-rollback、時系列／新旧判断、ソースDB identity、cross-process CAS、IPC security、容量式、Wasm packageの保証ではない。JSON入力・decoded bodies・hash作業等は既存どおり未課金で、input byte boundはRSS／peak／時間上限ではない。

## 型付き拒否とprivacy

`BundleReceiptError`は固定enumであり、本文・path・入力digest・任意のJSON／IOエラー文字列を保持しない。Debug／Displayとも固定診断だけで、source errorの全文を連結しない。receipt syntax／version／digest encoding、payload length／digest mismatch、両inputの上限、既存admission、payload／receipt IO、receipt encodingの拒否を区別する。部分的なreceiptやPreparedSiteは返さない。

この文書はlibraryの契約を記録する。root担当のone-shot receipt発行／public CLI接続・prepared posts保存demoとは分離し、未検証のCLI導線や全体gateの成功を先取りしない。[既存bundle検査](bundle-check-development.ja.md)

## 対象検証

private2契約は、実ファイルをhash後のadmission直前に書き換えても、admissionが読取り済み原bufferを受け取ることと、identity不一致時にadmission callbackが呼ばれないことを確認する。callback注入はprivate試験専用であり、製品APIへ公開しない。receipt readerの1KiB＋sentinelとtyped IO errorも検査する。

新integration5契約は、独立Python hashlibで求めた固定122 bytesのdigest、決定的3fields／非漏洩、v1・v2の両payload、duplicate／unknown／型／version／digest encoding、tamper／長さ不一致、identity一致でもadmission拒否、exact1KiB／8MiBと1byte超過、実ファイル変更／欠落と既存PreparedSiteの不変を検査する。既存bundle契約も共有read refactor後に維持する。

対象Rust試験、server ClippyとLinux crosscheckを検証範囲とする。native Linux、実DB、公開HTTPのlive切替・withdrawal保証はこのreceipt単体の試験に含めない。公開routerは起動時Publisherをdropし、ETag／cacheを同じsealed世代で構築する既存契約を維持する。receiptを保存しただけで稼働中の旧世代が切り替わることはなく、既に送信・保持されたBodyの撤回も保証しない。
