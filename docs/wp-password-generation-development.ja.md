# 既定 WP bcrypt 生成の限定候補

後続全体実測：`target/wp-password-generation-and-alpha-full-check.log`でRust449＋独立29、Python188、正式64 API・437,198比較とhooks572／dispatcher405、別枠の生成10ダミー相互検証各24回、8 DB fixture各45項目、Clippy・server/WP Linux all-target check・license214・実HTTPが成功し全fixture回収。専用artifactは `compatibility/evidence/wp-password-generation-crosscheck.json`。実行は `python scripts/compare_password_generation.py` と `python scripts/check_password_generation_evidence.py`、全体入口は `scripts/check.py --with-password-generation`。ランダム出力のbyte一致／salt一意性／実OS RNG故障や認証成功を実証したという意味ではない。

2026-10-29。固定 WP 7.1.3 `wp-includes/pluggable.php:2753` の `wp_hash_password` から、Bytes、empty custom hasher、empty `wp_hash_password_algorithm` / `wp_hash_password_options` filters、固定 PHP 8.5.12 の既定 bcrypt cost12 に限定した候補。新 API は `wp_hash_password_default_bcrypt_bytes(password, HostMaxBcryptCost)`。公開server、認証、session、credential保存や更新policyへ接続しない。既存verify-only APIと公開serverの依存境界は維持する。

元の順序に従い、trim前の入力が4096 bytesを超える場合は `WpPasswordHashOutcome::PasswordTooLong` を返す。明示的な `as_wp_storage_bytes()` はこの場合 `b"*"` を返すが、usable hashではない。長さが適合する場合だけ、必須の `HostMaxBcryptCost` が12以上かを検査する。defaultを持たないhost内部allowanceで、製品の推奨cost・manual設定・wall timeやconcurrency制限ではない。host maximumが12未満ならHMAC・provider・RNGの前に `CostLimit` となる。

次に端のspace/TAB/LF/CR/NUL/VTだけを借用sliceで除去する。FF、Unicode空白、内部NUL、非UTF-8 bytesは除去しない。既存verify-only APIと同じprivate HMAC-SHA384 helperを使い、key `wp-sha384` の48-byte digestをstandard Base64の64 bytesへ変換する。verify側は入力をtrimしないため、前後trim bytesを含む元入力で生成hashをそのままverifyするとfalseになり得る。生成のtrimと検証のraw bytesを無断で統一しない。

暗号処理は固定providerへ委ねる。`bcrypt::hash_with_result(prehash, 12)` のOS RNGでsaltを生成し、`format_for_version(TwoY)` を明示して `$wp` を付ける。provider既定のTwoB表記を採用しない。供給salt、固定seed、entropy fallback、独自bcrypt/HMAC、別algorithmは追加しない。OS RNG失敗は `EntropyUnavailable`、その他のprovider異常は `ProviderFailure` となる。error Debug/Displayは有限の固定診断で、入力・hash・provider文字列を保持しない。

正常返値 `WpPasswordHashOutcome::Hash(WpPasswordHash)` は63 ASCII bytesのopaque move-only owner。Clone/Displayや可変・owned内部escapeを持たず、Debugはredactedである。保存値を利用する場合だけ借用 `as_wp_storage_bytes()` を明示する。呼出側がその借用bytesをcopy/logすることまでは防止しない。

使用するproviderは既存のbcrypt 0.19.3（alloc+zeroize）、hmac 0.12.1、sha2 0.10.9、base64 0.22.1、zeroize 1.9.1で、依存の追加・変更はない。派生48/64-byte owners、返値ownerと生成直後のformat Stringはdrop時にzeroizeする。bcryptは自身のpass bufferを消去する。呼出元入力、HMAC内部、HashParts/providerの内部・一時allocation、compiler copies、allocatorや全process memoryの完全消去は保証しない。provider/format allocationはshared poolへ未接続で、allocation abortを全てtyped failureへ変換する契約でもない。依存の法的承認や暗号audit完了を意味しない。

private provider seamの4契約は、raw length→costの順序と呼出0、正確なtrim6、固定prehash/cost12/TwoYとredacted owner、entropy/その他失敗および不正provider返値の閉じた分類を検査する。public2契約は、実OS-random cost12生成を既存verifyでtrim済みbinary/NUL/非UTF-8入力へ照合し、元入力の非trim検証との差、元入力不変、overlength markerとhost拒否の区別を確認する。実RNG故障を発生させたという主張ではなく、そのfailure経路はprivate seamで検査する。

出力はランダムであるため、既存の全出力byte一致registryへ新APIとして加算しない。現在の正式64 API・437,198件と別の独立相互検証harnessで、Rust生成→固定actual PHP検証、actual PHP生成→Rust検証、TwoY/cost/envelope、trim境界とcallback profileを検査する方針である。この文書時点では新harnessや全体gateの成功を先取りしない。実ユーザーsecretを使わずpublic dummyだけを扱い、観測hashは通常の診断へ含めない。

generation以外のlegacy形式、Argon、custom hasher/filter、PHP型coercion、rehash、password更新、unknown-user timing、HTTP auth、session、認可、rate limit、bounded blocking実行は未対応。生成APIの存在をlogin可能やpassword保存policyの採用として扱わない。
