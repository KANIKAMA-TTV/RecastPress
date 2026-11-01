# 固定PHP参照関数loaderの開発記録

2026-10-27時点。scripts/reference_functions.phpは開発用oracleが固定WordPressソースから要求した関数だけをevalするためのhelperで、製品のPHP実行経路ではない。外側でidentityを検証した参照fileが、そのPHP process中に不変であることを前提とする。任意ソースの安全な実行、一般PHP parser、file変更検知を提供しない。

## Process内snippet cache

同じfile文字列keyについて、初回だけtoken_get_all(..., TOKEN_PARSE)とindex作成を行う。static cacheに保持するのは抽出済みcode stringsだけで、token配列は保持しない。導入時のoracleでは4fileの48loadを48回token化していたものが、4回になる。cacheした全codeを実行することはなく、要求されたsnippetだけを従来どおり毎回evalする。function_existsによるskipは入れず、同じ関数を再loadした時のPHP redeclaration失敗も維持する。

抽出規則は以前のhelperと同じ。T_FUNCTIONの後はWHITESPACEだけをskipし、次tokenの文字列を厳密に照合する。大文字小文字を正規化せず、先頭の重複snippetを採用する。bodyを読み終えても外側のscan位置を飛ばさず、nested named functionを独立に見つける。通常braceとT_CURLY_OPEN／T_DOLLAR_OPEN_CURLY_BRACESによるinterpolationの深さを同じ方法で数える。

実際のname指定に対するby-reference markerやfunction直後commentの未対応、missing／非string nameの失敗も変えない。method／abstract declaration等の構文を一般化して修正する変更ではなく、固定ソースで選択している関数の抽出規則を維持する。ソースを途中で変更した場合のcache更新は行わない。

## Bytes照合と有限benchmark

変更前helperをtarget/reference-loader-benchmark/reference_functions.before.phpへ保存し、eval箇所だけをreturnへ置換した観測用baselineと、導入時の新helperの4file・48snippetを照合した。全件でraw bytes・長さ・SHA-256が一致した。観測結果はtarget/reference-loader-benchmark/implemented-results.json、初期prototypeの観測は同directoryのresults.jsonに残す。いずれも正式なAPI等価性corpus件数へ加算しない。

Windowsの固定PHP 8.5.12を-nで実行し、既定memory_limit=128Mを変更せず各方式5回測定した。warmなローカルfile環境のmedianは次のとおり。

| 観測 | 変更前 | snippet cache実装 |
| --- | --- | --- |
| bootstrap wall time | 約0.474秒 | 約0.053秒 |
| bootstrap CPU time | 約0.469秒 | 約0.063秒 |
| process全体wall time | 約0.567秒 | 約0.141秒 |
| memory_get_peak_usage(true) | 14MiB | 14MiB |
| token化回数 | 48 | 4 |

抽出したcode文字列の合計は626,996bytesで、index等を含むprocess終了時のlive memoryは増える。これは有限な起動部分の観測であり、すべてのsource／runtimeやfullgate全体の速度・実RSS改善を保証しない。token配列の常駐や128MiB上限の引上げで得た結果ではない。

## 契約試験とgate履歴

scripts/test_reference_functions.pyのsynthetic6試験は、first duplicate payload、strict name／missing／非string、byref／comment未対応、nested、2種のcurly interpolation、cacheのcode-string構造、要求外のtop-level／関数が実行されないこと、異なるfile key、再evalのredeclaration失敗を確認する。固定WordPressのdownloadは不要で、開発用PHP CLIがあれば合成fileだけで実行できる。PHP配置は既存test_inventoryと同じRECASTPRESS_TEST_PHPまたはpinned toolを使い、未配置時はskipする。現pinned PHPでは6試験と構文検査が成功した。

target/budgeted-meta-view-and-loader-cache-full-check.logではWPのprimitive／hook／dispatcher比較と、通常MariaDB／MySQL・専用GTID MySQLの各39項目が通過した後、最後のschema generator証跡が旧reference_functions.php hashのままだったため全体gateは失敗した。この実行全体を成功扱いしない。DDLを固定ソースから実際に再生成し、本文bytesとSHAが不変であることを確認してtool provenanceを更新した。DB起動前にもschema generator tool hashを検査するguardを追加した。

補修後のtarget/budgeted-meta-view-and-loader-cache-final-check.logは成功。workspace Rust238件（通常ignore1）＋独立experiment19件・Python86件、WP45 API・299,957ケース／hooks572／dispatcher405、通常MariaDB／MySQL・専用GTID MySQLの各39項目、HTTP・Clippy・Linux向けcross-check・license観測208件・current provenance照合を通過した。全3所有fixtureは停止・回収済み。DDL本文のSHA不変と起動前generator guardの補修後に全体完了を確認した記録で、最初の失敗を取り消して成功扱いしたものではない。native Linux実行・製品CDC・R61全体の予算や自動容量の完成は主張しない。

その後JSON fallback用helpersをoracleへ追加し、参照fileとload対象は増えている。上記48load／4fileと速度・memory表は導入時の有限benchmarkであり、現在の全helper集合を再測定した値ではない。loader本体と不変sourceのcache規則は維持し、追加APIの比較は現在のsource集合で実行する。
