# valid UTF-8のutf8_uri_encode候補

全体checkpoint（2026-10-29）：`target/uri-title-and-credential-pipeline-full-check.log` exit0。Rust520＋独立29（ignore1）、Python241実行（240成功・1skip）、正式70 API461,522（新URI5920／title2538）、全8 fresh DB57、Clippy・server/WP Linux check・license215観測・実HTTP成功。旧68のinput/result SHAを実prefix再計算で保持、最終11checker全exit0（`target/uri-title-current-check-summary.json`）、全DB回収済。native Linux実行は未実施。

2026-10-29。固定WP 7.1.3 `wp-includes/formatting.php:1160` の `utf8_uri_encode` を、Bytes・全入力valid UTF-8・非負のtyped length・strict boolへ限定する。APIは `utf8_uri_encode_utf8_bytes(raw, length: usize, encode_ascii: bool, max_output: usize)`。新しいURL parser、slug/principal、HTML escaping、Unicode normalizationやtrimを追加しない。

最初に既存 `wp_is_valid_utf8_bytes` で全rawを検査する。sourceのlengthで出力されない末尾であっても、不正UTF-8ならUnsupportedValue。元PHPのinvalid UTF-8や型coercionの全挙動はこのprofileへ含めない。NUL、controls、BOM、noncharactersはvalid UTF-8として受理し、特殊な削除・変換をしない。

ASCIIはencode_ascii=falseなら全bytesをそのまま残す。NUL、`%`、空白、slash等も同じである。trueならrawurlencodeのASCII unreserved `A-Z a-z 0-9 - . _ ~` だけを残し、それ以外を大文字hex `%XX` にする。非ASCII scalarは元UTF-8の各byteを小文字hex `%xx` にする。従ってASCIIのSPACEは `%20`、éは `%c3%a9`、€は `%e2%82%ac` になる。ASCII modeを変えても非ASCIIのhex caseは変えない。

source length0はunlimited、非zeroは**符号化後bytes**の上限である。ASCII tokenは1または3 bytes、非ASCII scalar tokenは6/9/12 bytesを一組として扱う。最初のtokenが残りに入らなければそこでbreakし、後の短い文字を拾って埋めたりtokenの途中を返したりしない。これはvalid UTF-8において元sourceのoctet収集と同じprefixになる。

`max_output` は別のhost内部の最終output capで、source length0をhost unlimitedへ読み替えない。同じwalkerでcount→checked host cap→exact logical lengthのfallible reserve→emitを行い、部分結果は返さない。source lengthで最初のtokenが入らず空になる場合、rawが非空でもcap0で成功する。入力長を出力capの拒否条件へ転用しない。source budgetのchecked overflowはnonzero上限へ入らないtokenとして停止し、unlimited output/host加算のoverflowはOutputLimitへ閉じる。

`Utf8UriEncodeError::{UnsupportedValue,OutputLimit,AllocationFailed}` はpayloadのない固定分類で、Debug/Display/sourceにinputや任意provider文字列を保持しない。count後の出力確保はfallibleだが、入力の確保、allocator capacity、peak/RSS、shared BudgetPool、concurrencyや全allocation failureの管理をこのcapで保証しない。unsafe・新依存・DB/server接続は追加しない。実PHPにおけるmbstring encoding stackの操作はnativeにはなく、reference wrapperの限定profileで扱う。

public6契約は全128 ASCII bytesとexact unreserved、ASCII/nonASCII hex case、2/3/4-byte scalar境界、whole-token prefix停止、source budgetとhost cap、NUL/empty、invalid trailing UTF-8、large prefix/usize MAXと診断の秘匿を検査する。private2契約はchecked overflowとfallible capacity overflowを、巨大入力を実確保せず検査する。正式reference corpusは別担当が既存68 APIからappend-onlyでAPI69へ追加し、後続gateでactual PHPと比較する。ここではその成功を先取りしない。

対象検証ではpublic6件・private2件が成功し、WP crateのall-target Clippy（offline/locked、`-D warnings`）と担当2 Rust fileのrustfmtも通過した。Windowsのincremental cacheではhard-link失敗後のcopy fallback警告が出たが、テスト・Clippyの終了codeは0である。全体gateや正式PHP比較、Linux実行の成功をこの対象検証から推定しない。

title helperからの呼出しは別のsource-derived契約であり、このAPI自身はtitle加工や公開route選別をしない。WP src/lib変更はDB dev-only pipelineが追跡するため、関連DB/alpha/binlog/bootstrap/rotation/restart証跡もstaleにする。親のsource freeze後に必要な実再測定を行い、手動restampしない。担当はCargo・DB・正式proofを変更せず、DBを起動しない。

## 限定reference比較と正式gateへの引渡し

PHP oracleは固定formatting.phpの実 `utf8_uri_encode` と、functions.phpの実 `mbstring_binary_safe_encoding` / `reset_mbstring_encoding` をselected loaderで読み込む。全WP bootstrapやhelper stubで置換しない。PHP8.5.12、noMbstringに加え `mb_internal_encoding` と `mb_strtolower` の不存在を実測metadataに含め、入力Bytes・whole valid UTF-8・非負signed64 length・exact bool・明示profileを両言語で先に検証する。native probeではlengthのusize変換も検査する。negative length、float、非Bytes、打切りより後の不正UTF-8、別profile等はsubset外であり、PHP falseや部分成功に混ぜない。

新URI 5,920件とtitle 2,538件の限定actual比較は全8,458件で差分0だった。URIは両ASCII mode、0/unlimitedと正のlength、全ASCII・2/3/4byte scalar・i64 MAX length等を含む。profile/counterの型と値を各batchで厳密照合し、reference call数は**トップレベルのURI比較caseだけ**を数える。title helper内部から実URIが呼ばれる分はこのcounterへ加算しない。拒否protocolは両実装合計64 processで確認した。PHP拒否試験は `display_errors=stderr` を指定し、stdoutの成功値・secret markerの診断漏洩がないことを検査した。

旧68 API全453,064件の順序付き入力SHAと旧結果SHAを固定し、corpusは末尾へだけ追加する。正式全比較では旧prefixの両実装結果を再hashして以前の値と一致しなければ失敗する。70 APIs / 461,522件はこの段階では正式全gate予定値で、限定比較を正式3証跡やgeneration/legacy/DB証跡へ手動restampしない。新wrapper Python4件・evidence全30件（追加2件とmandatory-field拒否）・既存case生成5件・WP全target Clippy/fmtも通過した。全体gateはrootがsource freeze後に実測する。
