# WP-prefixed bcrypt verify-only kernel

親タスクの後続実測：`target/wp-bcrypt-and-alpha-publication-full-check.log`で正式PHP64 API・437,198件／hooks572／dispatcher405、Rust443＋独立29、Python182、8 DB fixture各45項目、Clippy・server Linux cross-check・license214・HTTPを通過した。新542件とactual hook542も正式evidenceへ記録。`target/wp-bcrypt-linux-cross-check.log`ではWP全targetのLinux向けcheckも成功した。native Linux実行は未検証。以下の担当別対象検証と区別し、現在の認証／session実装完了を意味しない。

2026-10-29。固定WP 7.1.3 `pluggable.php:2843` の `wp_check_password` から、raw Bytes、`$wp$2y$`、empty custom hasher、empty filter registry、user_id=0に限定する。新APIは `wp_check_password_wp_bcrypt_bytes(password, hash, HostMaxBcryptCost)`。64番目のWordPress関数として登録するが、login、HTTP auth、session、認可やcredential更新の完成ではない。公開serverへWP dependencyを追加しない。

`HostMaxBcryptCost::new(u8)` は4..31だけを受理しdefaultを持たない。host内部の明示allowanceで、製品のmanual設定・推奨cost・容量算式ではない。hashの63 ASCII bytesとliteral `$wp$2y$` envelopeを先に限定する。passwordが4096 bytes超なら、ソース順でcostや本体のcanonical解析、HMAC/bcryptの前にfalseを返す。従ってこのenvelope内では長過ぎるpasswordとmalformed cost/bodyの組合せもfalseになる。envelope外のlegacy形式等は、長さにかかわらずsubset外である。

4096 bytes以内では、2桁decimal cost、4..31、host allowanceを検査し、その後にproviderのbounded `HashParts` parserでsalt/checksumのcanonical bcrypt Base64も検査する。計算前のCostLimitは通常のpassword mismatchではない。`Result<bool, WpBcryptVerifyError>` のtrue/falseはmatch/mismatch、UnsupportedHash/CostLimit/ProviderFailureは別分類で、いずれもerror Debug/Displayへpassword/hash/provider文字列を保持しない。hashのcostを無条件に計算しないが、wall timeや実CPU秒を保証しない。async serviceへ接続する場合のbounded blocking/concurrencyは別契約であり、future timeoutだけをCPU cancellationと扱わない。

暗号primitiveは実装し直さない。固定providerのHMAC-SHA384へkey `wp-sha384` と全password bytesを渡し、48-byte digestをstandard Base64で64 ASCII bytesへ変換する。bcrypt固有alphabetではない。`$wp` を除いた60-byte suffixとこの64 bytesをbcrypt verifyへ渡す。trim、UTF-8変換、NUL終端処理や72-byte legacy bcryptの切詰めをpasswordへ適用しない。NULや非UTF-8を含むpasswordもHMACで全bytesが処理される。

使用版はbcrypt 0.19.3（alloc+zeroize）、hmac 0.12.1、sha2 0.10.9、base64 0.22.1、zeroize 1.9.1。root担当がmanifest/lockとlicense inventoryを更新した。bcryptのalloc featureはgetrandomを依存へ含めるが、このkernelはhash生成/RNGを呼ばない。bcrypt内部は別のbase64 0.23.1を使う。HMACはdigest0.10系でsha2.10と整合する。取得済みbcrypt/HMAC/cipher/Blowfish sourceを確認し、低level cipherをアプリから直接呼ばず、provider APIだけを利用する。[bcrypt固定版source](https://github.com/Keats/rust-bcrypt/blob/v0.19.3/src/lib.rs)、[HMAC固定版manifest](https://github.com/RustCrypto/MACs/blob/hmac-v0.12.1/hmac/Cargo.toml)

providerの固定23-byte digest比較はsubtleのconstant-time equalityを使う。形式・長さ・costのbranch、unknown-userの経路、login全体がconstant-timeとは主張しない。`Zeroizing` は派生48/64-byte stack ownersをdrop時に消去し、bcryptは自身のpass bufferを消去する。呼出元の入力、HMAC内部状態、compiler temporaries、allocator、全process memoryの消去は保証しない。入力や実credentialをlog・Debug・evidenceへ渡す機能を追加しない。

oracleはselected-function loaderで実 `wp_check_password` だけをロードし、pluggable.php全bootstrapやphpassを実行しない。Bytes/profile/typed host costと同じconditional admissionを先に検査する。`wp_filter/wp_filters/wp_current_filter/wp_hasher` のpresence/NULL/値を退避して空registryとNULL hasherへ置き、actual functionをuser_id=0で呼ぶ。bool結果、`check_password` の1呼出、空stack/registry、hasher NULL、actions不変を確認し、finallyで4globalsを完全復元する。nativeはfilter bookkeepingを実装しない。wrapperのpassword/hash引数にもSensitiveParameterを付ける。

`php_wp_bcrypt_profile` はfixed source/hash、63/4096のbyte境界、HMAC/standard Base64、4..31 host型、empty callbacks/user_id0、provider版、public dummy vectors hashを厳密型値で束縛する。新しいcase countsはhash cost4/12、host maximum4/12、within/over4096を全て正整数で要求し、それぞれAPI総数に一致させる。実 `check_password` hook counterもbatchおよびevidenceの件数へ一致させる。

fixed public dummy vectorsはcost4のempty、dummy、NUL/非UTF-8、前後space、4096-byte入力と、少量のcost12を使う。salt固定のdummy観測は再現用であって、製品のcredential生成方式ではない。固定pairsのSHA-256は `6b9ff298abe122a1a9603bf9ba531f7cd460a3bd6e80e9ee35e47aaa0150b586`。ソース上の `wp_hash_password` はtrimするがverifyはtrimしないため、その差も検査する。legacy MD5は4096 guardより先、custom hasherはその次という元の順序を、この限定profileへ無断で一般化しない。

新Rust契約はpublic3件＋private3件。PHP固定vectors、raw bytes/NUL/非trim、4096境界、明示host cost、unsupported/canonical malformedを検査し、provider注入でcost/length拒否時に呼出0とProviderFailureの区別を確認する。Python evidence22件中新2件とoracle scope2件も追加した。scope契約は3global状態×2host maximum×3password状態の18呼出をsuccess/inner exception双方で検査し、旧hasher objectを呼ばず復元する。

限定actual PHP/native比較は542件、match6/mismatch536、差分0。hash cost4=539/cost12=3（後者は実計算2件＋overlength no-CPU1件）、host maximum4=540/12=2、within4096=538/over4096=4、実hook count542。対象外の型・形式・cost等36入力を両processで拒否し、診断へdummy secret markerが出ないことを確認した。overlength＋malformed cost/salt/checksumの3ケースもfalseで一致した。記録は `target/wp-bcrypt-implementation-check.json`、dummy生成観測は `target/wp-bcrypt-dummy-vectors.json`。既存63 API corpusを維持し、全候補は64 API・437198件。

generation/rehash、MD5/phpass、非prefixed bcryptのNUL/C-string/72-byte挙動、Argon、任意custom hasher/filter、PHP型coercion、user lookup、lockout/認証成否、session、HTTP authは未対応。空password向けdummy hashにtrueを返せることも、空passwordでloginを許すpolicyではない。依存の法的承認や全暗号実装のaudit完了を示すものではない。正式PHP3比較・全gate・全8DBは親タスクが後続実行し、この記録時点で先取りしない。DB/server、root manifests/lock、inventory/NOTICE、正式proofはこの担当で変更していない。
