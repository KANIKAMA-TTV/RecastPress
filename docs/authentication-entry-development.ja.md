# 標準login入口のsource順序と未接続部分

2026-10-29。固定WP 7.1.3の読取りと、公開dummyだけのPHP 8.5.12 scratch観測を記録する。DBログイン・session発行の成功証跡ではない。現在のraw user lookup／password helpers／credential CASを、そのまま認証済みprincipalの発行として扱わない。

## S1・S2・passwordの区別

`pluggable.php:684 wp_authenticate`は、usernameを`sanitize_user`したS1、passwordをPHP trim6したP1としてauthenticate callbacksへ渡す。低level`wp_check_password`自体はtrimしない。生成時trim、入口trim、低level検証非trimの三つを混同しない。

`user.php:153`のusername callbackはprevious `WP_User`なら即返す。次にPHP emptyを判定するので、空文字だけでなく文字列`"0"`も対象。emptyの場合のprevious `WP_Error`は保持するが、nonemptyなら常にprevious errorを優先するわけではない。

`class-wp-user.php:202 get_data_by('login', S1)`はtrimとempty判定を行い、その後にもう一度`sanitize_user`したS2をcache／SQLへ使う。S2の後にはempty再判定がない。二回のsanitizeを一回へ短縮せず、SQL部分だけを提供する`user_by_login_text_query`へ前処理完了の意味を持たせない。

`sanitize_user`は冪等ではない。公開scratchでは`%&x;61`→S1 `%61`→S2 空文字となり、record-only wpdbによって空SQL keyまで進むことを確認した。`&x;0`→S1 `"0"`はcallbackのemptyで止まる。FF+A+FFは`" A "`→`"A"`、FFだけはSPACE→空文字。trimの後にwhitespace圧縮が走る順序による。

NoLiteral `<` の文字列でも`strip_tags`はNULを除去する。`a\0b`は`ab`、`25 00 36 31`（hex）はNUL除去によってpercent patternが成立し空になる。元bytesがinvalid UTF-8でも、NUL除去後にvalidとなる場合があるため、将来の明示UTF-8 subsetはこの段階の後で判定する。

## usernameとemailのcallback順

`user.php:242`のemail callbackが受け取るのはS2ではなくS1。usernameが見つかっただけではemail fallbackを止めない。username認証が`WP_User`成功なら終了するが、password不一致などのerrorの後でもS1が有効emailなら、別email rowの認証が成功し得る。lookup優先と認証成功優先を同一視しない。

lookup後の`wp_authenticate_user`、password検証、rehash判定・更新、その後のcache／action／cookie処理は別の段階である。現在のSQL lookupに順序・認可・cacheを入れず、raw credential CASにも自動generationやhookを入れない。

単一サイトのこのchainに`user_status != 0`一律banはない。spam判定はmultisite条件付きであり、raw schema fieldを独自のban条件へ変えない。

## IDと残件

`get_data_by('id', ...)`はPHP int化と`<1`拒否を行う。固定64-bit PHPでu64 MAX文字列はPHP_INT_MAXへ飽和し、`WP_User::init`もIDをint化する。Rust raw readerのID0／u64 MAX保持はstorage契約であり、WP認証主体IDの同等性を証明しない。principal ID domainは後続で明示する。

NoLiteral < のbyte kernel、no-Intl／明示localeによるusername処理、S1／P1／S2を区別した前処理は68 API checkpointで検証済み。initial-null／empty relevant filters／host-supplied lookupの限定credential stageも全体gateで検証済みで、username一致時は停止、確定不一致時だけemailへ進む。同じraw row内のhash検証・rehash hintまでを扱い、principal発行や自動書込みは行わない。未対応hashや不明なlookup結果を不一致へ読み替えない。

未決・未接続はprincipal domain、filter/cache統合、CAS conflict／COMMIT unknown時のlogin処理、session保存・鍵・期限・失効・回復、OIDC連携、HTTP／HTTPS境界。これらを推測して採用せず管理ready503を維持する。既決のSSO三段階・初期off／OIDCのみ／MFA非実装は[認証設計](authentication.ja.md)に従う。

scratchは`target/reference-auth-entry-observation.json`、`target/reference-auth-entry-whitespace-observation.jsonl`。lookupはrecord-only stubであり、実DBのcollation／credential照合・認証成功のproofではない。
