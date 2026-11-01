# _truncate_post_slugのconditional decoded UTF-8候補

全体checkpoint（2026-10-29）：`target/truncate-slug-and-post-name-pipeline-full-check.log` exit0。Rust536＋独立29（ignore1）、Python254実行（253成功・1skip）、正式72 API482,951（新truncate16,064）、全8 fresh DB61、Clippy・server/WP Linux check・license215観測・実HTTPが成功。旧71 input/result SHAを実prefixで保持、最終11checker全exit0（`target/truncate-slug-current-check-summary.json`）、全fixture回収済。native Linuxは未実施。

固定WP 7.1.3 `wp-includes/post.php:5745–5758` の返値を、明示usize lengthと最終host capで扱う。APIは `truncate_post_slug_conditional_decoded_utf8_bytes(raw, length, max_output)`。HTML除去、sanitize、lowercase、URL validation、slugの一意性、status／DB書込み／公開policyは含めない。

raw byte長がlength以下ならdecodeせず、そのまま最後のASCII hyphenだけを除去する。超えた場合だけPHP urldecode相当の単回処理を行う。元の`+`はSPACE、正しい大小hexの`%HH`は1 byte、それ以外のpercentはliteralのままで、生成されたpercentやplusを再処理しない。decodedがrawと同じなら、rawの先頭length bytesを取る。UTF-8 scalar境界を守るsubstringではない。

decodedが変わった枝だけ、decoded全体がvalid UTF-8であることをprofileとして要求し、既存URI helperをlength・encode_ascii=true・中間cap usize::MAXで呼ぶ。source budgetより後のinvalid tailも拒否する。変更が無い枝とdecode未実行枝にはUTF-8 guardを課さず、任意Bytesやbytecutで不正UTF-8になった出力も保持する。PHP invalid-decoded経路全域の互換は主張しない。

length0は分岐により異なる。decode同一ならraw substringは空だが、decodeが変わった場合のURI length0はunlimitedになる。ASCIIはuppercase percent、非ASCIIはUTF-8 bytesのlowercase percentで再符号化される。最後は全分岐でASCII `-`だけをrtrimし、その完成長へ最終capを適用する。従って `%2d`の長い列をlength0で処理して空にする場合、cap0で成功する。中間へ最終capを流用しない。

借用可能な分岐は完成範囲だけfallible reserveしてcopyし、URI結果はowned Vecをtruncateして再利用する。decoded中間は最大raw長のfallible allocationを行う。固定error UnsupportedValue／OutputLimit／AllocationFailedにinputや任意診断を保持せず、部分結果を返さない。capは中間確保・capacity・RSS・peak・同時実行・共有poolの制限ではない。unsafeや新依存は追加しない。

public6契約は3分岐／length0、invalid raw保持とUTF-8 bytecut、非再帰decodeとhex case、最終rtrimとcap、changed decoded wholeUTF8 admission、入力不変と閉errorを対象とする。private契約はcapacity overflowと固定分類を扱う。対象native／PHP正式比較／DB再測定／全体gateの成功は実施後に区別して記録する。WP source変更はDB dev-only pipelineのprovenanceもstaleにするため、親が必要な実再測定を行い、証跡を手動restampしない。

2026-10-29の対象検証ではprivate1件・public6件が成功し、WP crate全target Clippy（offline/locked、-D warnings）と担当Rust 2fileのrustfmtが通過した。incremental hard-linkのcopy fallback環境警告はあったが終了codeは0。正式PHP比較、Linux実行、DB実測、全体gateの成功はこの限定確認に含めない。
## 限定actual比較とconditional domain

formal harnessは実 `post.php:5745` の `_truncate_post_slug(raw,length)` をselected loaderで呼び、実URI/binary-safe encoding helpersを使う。全post.php bootstrapやDB/HTML処理へ進まない。原file全SHAとformatting/functions/UTF-8 helperの依存hashをregistryへ束縛する。NoMbstringと関連function不存在、nonnegative signed64 length（probeではusizeへ変換）、final host cap65536を実metadataで照合する。Intl/locale/HTML policyをこのAPIへ追加しない。

rawはUTF-8一律guardを設けない。sourceと同じstrlen条件・一度だけのurldecode後のstrict bytes比較で3枝を分類し、変更decoded URI枝だけ、打切り範囲外を含むwhole decoded UTF-8を要求する。raw no-truncate/same-decode substring枝は不正UTF-8/NULを保持し、byte切断で不正列が生じてもbyte codecで返す。`+`はSPACE、`%2B`はliteral PLUS、`%25`の生成percentを再decodeしない。length0はchanged URI枝でunlimited、それ以外の非空raw substring枝では空となり、最後にliteral dashだけをrtrimする。

新16,064件の限定実PHP比較は差分0。source branch counterと入力由来の独立分類は、raw no-truncate13,172 / raw substring1,155 / decoded URI1,737で一致し、実reference callsは16,064回だった。全256byte、valid/invalid percent、plusと生成percent、NUL・invalid raw・UTF-8 scalar切断、mixed hex、0/200/大length・末尾dashを含む。トップレベルの呼出しだけをこのcounterへ数え、内部URI呼出しをAPI69のcounterへ加算しない。

型/domain/profileとfixture出力上限の拒否は両実装合計34 processで確認した。changed decoded invalidはsourceが返すfalseや正常prefixだと主張せず、明示domain外の拒否として分ける。URI budgetの後ろに不正decoded byteがある場合も拒否する。PHP拒否検査は `display_errors=stderr` を指定し、stdoutの成功値と診断のsecret marker漏洩を認めない。native error/callback errorをempty slugへ潰さない。

新Python4契約は条件付きguard・非再帰decode、実named sourceの0長/byte切断/trim、悪型の事前拒否、各MB function存在と内部exceptionの秘匿を検査する。evidence全34件（追加2件とmandatory-field拒否）、既存case generator5件、WP全target Clippy/fmtも通過した。旧71 API全466,887入力は順序付きSHAを固定し、後続正式比較では旧prefixの両結果SHAも実再照合する。

72 APIs / 482,951比較はこの段階では正式全gateの予定値で、正式3proofやcrypto relation/DB fixtureの再認定を先取りしない。rootがsource freeze後に実再生成し、手動hash restampは行わない。これらの有限bytes比較からslug一意性・予約語・post_name保存/公開policyや一般URI/HTML安全性を採用しない。
