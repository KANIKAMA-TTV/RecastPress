# HTTPキャッシュの開発メモ

更新日：2026-10-26（日本時間）。DB領域以外の継続開発として、公開HTTPと既存のPageCacheを接続した。

最新検証（記録更新2026-10-27、日本時間）：target/filtered-postmeta-and-cgroup-full-check.logでworkspace Rust190件＋独立experiment13件・Python61件、HTTP・Clippy・Linux向けcross-checkが成功。WP39 API・266,493ケースは以前のform gateで比較済みで、後続WP無変更のためPHP再実行を省略・hash照合。CLI cacheは実監視・容量未確認の停止を維持する。

## 今回の範囲

起動時に固定した公開世代のGET本文をキャッシュする。ステータスとContent-Typeは毎回その公開世代から読む。Host・targetの検査に成功した要求だけがキャッシュ経路へ入り、キーはルーターごとに分離したサイト、検証済みHost、クエリー全体、Content-Typeで構成する。クエリーの順序・重複・符号化を統合しない。TTLは追加しない。

HTTPのCache-Controlは引き続きno-store。これはブラウザーやCDNへの指示であり、この開発用の不変な公開本文を内部メモリへ保持する契約とは区別する。任意の上流HTTP応答の保存条件を解釈する実装ではない。将来Cookie・Vary・Set-Cookie・外部依存を持つ応答へ、この条件をそのまま流用しない。

キャッシュ本文はArcで共有し、ヒット時の本文コピーを避ける。キー・本文と管理情報の論理予算内で退避する。配信中のBodyが保持するバイト列や構築時の一時割当は、エントリーの削除だけでは解放されない。プロセス全体の実メモリ上限・同時要求の予算を保証したものではなく、性能効果も未測定。

## 共有論理poolとの接続範囲

最新phase3のtarget/shared-buffer-and-pretty-json-full-check.logはworkspace251件＋独立experiment19件・Python86件、PHP45 API・304,071ケース、3 DB各39項目、HTTP・Clippy・Linux向けcross-check・license208観測を含め成功し全fixtureを回収。postmetaだけ同じpoolのInternalRows／Index／ChangeBufferがcallback中同時に正で、pool0拒否と最終drop後claimed0を確認した。他ProbeBufferは専用poolで、decoder／input／frame比較コピー／position／outer Vec等は課金外。process全体のRSSやcross-process・自動容量・CLI cacheの接続ではない。[共有buffer候補](budgeted-change-buffer-development.ja.md)。

target/budgeted-meta-view-and-loader-cache-final-check.logでDB owner候補を含む全gateが成功。workspace238件＋独立experiment19件・Python86件、PHP再比較45 API・299,957ケース、通常MariaDB／MySQLと専用GTID MySQL各39項目、HTTP・Clippy・Linux向けcross-check・license208観測を通過し、全3 fixtureを回収した。初回gate終盤のschema helper記録の古さはDDL不変の再生成と起動前guardで修正した。これは下記のDB logical ownerの実測で、CLI cacheや全RSS監視への接続ではない。

共有BudgetPoolにはDB専用BudgetedPostMetaViewの行／索引と、postmeta有限windowのraw保持を接続する候補を追加した。行予約をpayload取得・cloneより前に行い、old＋newを同時にclaimedへ残し、最終Arcまでleaseを保持する。既存unbudgeted APIの割当やHTTP cacheはこのownerの対象外である。論理row／index chargeだけで、node／capacity／driver／ref map／peak／RSS・cross-process・自動配分算式を保証しない。内部hostのallowanceであり製品の手動容量設定を追加せず、CLI cacheの保存・ヒット停止も維持する。[予算付きDB viewの範囲と統合gateの経緯](budgeted-meta-view-development.ja.md)。

## 共有対象からの除外

- GET以外、200以外、本文付き要求、HEAD、生成したエラーとヘルスチェックは保存しない。
- Cookie・Authorization・Proxy-Authorization・X-WP-Nonce・未知のヘッダーは保存にもヒットにも使わず通常配信する。
- Cache-Control・Pragma・Range・条件付き要求ヘッダー・本文関連ヘッダーも通常配信する。条件付き／部分応答そのものの実装を追加したものではない。
- クエリー名は開発用にs・page・pagedだけを許可する。未知・符号化された名前・空要素・セミコロンを含むクエリーは通常配信する。要求の拒否やルーティングの正規化には使わない。

クエリー値に含まれる任意の秘密を自動判定する機能はない。ここで扱うのは作成者が公開専用と確認した固定ページ束であり、ログイン・プレビュー・期限付きURL・動的なユーザー別描画の対応を示さない。未知のヘッダーを除外するため、独自ヘッダーを付けるプロキシでは通常配信に戻る場合がある。

## 停止と制御

public_router_with_cacheはルーターと内部制御用のPublicCacheControlを分けて返す。制御は認証なしのHTTP経路へ公開しない。容量は将来の内部アロケーターから渡し、利用者向け容量設定は追加しない。監視・公開世代の照合が終わり、有限の予算も得られるまで停止する。

通常CLIにはアロケーターも監視もないため、初期設定onのまま保存・ヒットとも停止する。監視断、再照合開始、off、容量縮小は全パージする。監視回復を自己申告する起動フラグは設けない。ロック破損時はキャッシュを再利用せず公開世代から通常配信する。

公開世代を実行中に置換する権限は引き続き起動前に破棄する。この制約で、配信中のBodyも同じ不変世代を参照する。パージは送信済み／配信中のBodyを撤回しない。ライブ切替の失効・送信境界が実装できるまで、このアダプターを動的なPublisherへ接続しない。

## 検証と残件

HTTP経由で監視／予算の前提、クエリー分離と全パージ、認証等の除外、HEAD／エラー／Host境界、停止・復帰・予算縮小、別ルーター間の分離を試験する。内部試験では、ウォーム済み本文を認証要求が再利用しないことと、ロック破損時の通常配信を確認する。

実アロケーター、ファイル監視、複数プロセスの予算・失効、ライブ公開、同時生成の制限、負荷測定、管理画面は未実装。このHTTPアダプターはDB監視へ接続していない。

## 明示共有poolを持つcore cache候補（2026-10-27、限定実装・検証済み）

BudgetedPageCacheはprivateなPageCacheと明示BudgetPoolを保持する新しいcore API候補。既存HTTP constructorとCLIは従来PageCacheを継続し、監視・自動容量未確認時の保存／ヒット停止も維持する。poolの容量を利用者向け設定へ追加するものではない。

localの停止・ticket・容量を検査し、既存と共通のentry chargeを計算してからResourceKind::ResponseCacheを予約する。成功後にだけ置換・local LRU退避・clock更新へ進む。insertはlocal拒否をOk(false)、global予約拒否をErr(BudgetError)で区別する。共有poolはold＋newを同時に課金し、global不足時には既存の値とLRUを保つ。global不足は退避前に拒否するため、local LRUが動く場合も先にglobal予約できる余裕が必要となる。

新しいget_sharedはBudgetedCachedBodyを返す。本文Arcと同じentryのreservation Arcを両方保持し、borrow-onlyのas_bytes、charged_bytesと、両者を保持するCloneを公開する。purge／evict／cache破棄後も外部handleがあれば全entry chargeを残し、最後のhandle破棄後に返す。keyが先に破棄されてもkey分を含む全chargeを保持する保守的なモデルである。保持した本文は配信の認可やfreshness tokenではなく、ticket／公開世代の確認は別に必要。

CacheStatusのcharged_bytesとentriesはresidentだけを表す。purge後のstatus=0はglobal poolへの返却を意味せず、外部handleが残ればpool.statsに課金が残る。local apply_budget縮小は旧ticketを失効して全パージし、global pool.set_limit縮小は保持ownerを消さずdebtとして扱う。既存bytesの借用は残るが、新しい予約は余裕ができるまで拒否する。監視断は従来どおりヒット停止・パージする。

Entryへlease slotを追加するため、既存PageCacheもsize_of::<Entry>()の増分をlogical chargeへ反映する。停止やLRUの規則を変える変更ではないが、同じlocal予算に入るentry数は変わりうる。入力のkey／Vecは予約前にcallerが確保済みであり、reservationのArc node、handle metadata、HashMap bucket／退避用一時領域、allocatorの丸め、コピー／HTTP Body、実RSS／peakやcross-processをこのモデルで制限しない。自動容量・本番の監視／失効との接続は未完了。

新integration9件とprivate Unavailable admission注入1件で、local拒否とglobal拒否の優先、入力capacityのcharge、旧値／LRU／clock保持、置換old＋new、purge／evict／cache破棄後のhandle／Clone寿命、monitor／epoch、local縮小とglobal debt、他ResourceKindとの競合を確認した。Unavailableは共通admissionの失敗注入で、実ledger poisonは既存resource_budget unitで確認しており、両者を同じ試験とは扱わない。独立レビューでも新10件と既存cache／publication15件が成功した。

target/budgeted-cache-shared-pool-full-check.logでworkspace Rust276件＋独立experiment19件・Python89件、3所有DB fixture各39項目、cold restore／pre-COMMIT kill／filter／共有pool／overlap、HTTP・Clippy・Linux cross-check・license208観測と証跡照合が成功し、全3 fixtureを回収した。両nativeのpostmeta_shared_pool_commits=3／refusal_preserved=true／released=true、overlap prefix134も維持。今回PHP再実行なしでWP45 API・310,363ケース／hooks572／dispatcher405を保持し、別途WP current hashを照合した。製品cacheアロケーター／監視／配信全体の予算接続を完成したものではない。

## 明示共有poolを受け取るHTTP経路（2026-10-27、限定実装・検証済み）

public_router_with_budgeted_cache(site, host, pool)だけがBudgetedPageCacheを使うopt-in API。従来public_router／public_router_with_cacheとCLIはlegacy backendを維持する。private factoryでHost・reserved health routeの検証、準備済みsiteの公開、唯一のPublisherをlisten前に破棄する処理とrouter構造を共用する。公開GETは準備済み本文を取り出すだけで、PHP／DB／feed生成やjob投入を加えない。

初期状態は従来と同じ設定on・Reconciling・local予算0。PublicCacheControlは内部capabilityで、HTTP操作routeへ公開しない。apply_allocationはlocalだけを変え、渡されたglobal poolのlimitを変更しない。poolの容量とsource監視はcallerが別途満たす前提で、自動容量式や実監視の完成とは扱わない。

cacheのprivate backend enumがlegacy／budgetedを共通操作し、budgetedでは本文handle全体をprivate CachedOwnerへ入れてBytes::from_ownerへmoveする。cache lock内でticket・lookup／insert・Body構築までを行う。nonempty Body未poll、収集後のBytes、clone／nonempty sliceに実ownerが残る間は同entry予約を保持する。cache residentがpurge後0でもBody側のownerが残ればpool課金は残る。

locked bytes1.12.1ではfrom_ownerのownerは最終owner参照まで保持されるが、empty sliceはowner無しBytesを返す。http-body-util0.1.5 Full::newはempty Bytesを即破棄し、空Bodyには保持ownerが残らない場合がある。課金は実owner参照の寿命であり、空Bodyの最終dropまで予約が残る保証はしない。Bytes ownerのBox、明示copy、multipart集約、transport buffer、準備済みPublicPage本体は課金外で、配信全体のRSS上限ではない。

初版のlocal拒否／stale ticket／pool拒否は準備済みpageのcopy fallbackで応答した。後続の[共有本文owner候補](shared-public-body-development.ja.md)では未cache応答をpageのArc ownerへ置き換え、この応答用payload copyを削減する。cold missのcache insert用Vec copyと既存CachedOwner／entry leaseは維持し、共有page ownerはcache pool課金外である。Host／method／認証・未知header／query／body／statusの除外規則とno-store等を維持する。purge・monitor lossは新lookupを止めるが、取得済みBodyを撤回しない。返却後のpoll／送信時再照合やlive publicationの失効は未実装で、sealed startupという制約を維持する。更新後の検証結果は専用記録へ分ける。

共有化の対象試験と `target/shared-body-full-check.log` のretry全gateは成功した。cache hitのowner lease、cold insert copy、eligibility／monitor方針を保ち、全8 fresh DB各62・HTTP・Linux cross-check等を確認した。最終初回は3cryptoにbinary置換のstaleを検出し、現binaryで3種を実再観測した後、最終11も全exit0（`target/shared-body-current-check-summary.json`）。WP正式75／508,297はregistry current、pure300はsource不変の直前観測を維持し、今回再実測した結果へ混ぜない。[対象数・失敗履歴と依存Wasm再確認](shared-public-body-development.ja.md)を参照。

新HTTP integration10件とprivate stale-admission unit1件で、初期停止／local制御、未poll Responseのpurge後保持、複数hitの同entry charge、Bytes clone／nonempty slice、empty Full／sliceの早期解放、to_bytes上限失敗時の返却、global拒否fallbackと旧cache保持、監視loss後の取得済みBody、Host／HEAD／認証等の除外、legacy／reserved routeを確認した。追加の実API間試験は、事前render_budgetedのRendering owner保持中にResponseCacheを同poolで拒否・許可し、router／control破棄後も独立した両owner寿命で各chargeを返すことを確認する。公開pageへ渡した本文copyは未課金であり、GET内でfeedを生成した試験ではない。

独立レビューで新integration9件・legacy cache7件・HTTP8件・private unit3件の計27件と、追加cross-owner1件が成功した。target/budgeted-http-cache-owner-check.logでworkspace Rust287件・Python89件、HTTP・Clippy・Linux cross-check・license208観測・DB current hashを確認。WP／binlog／overlapも別途照合し、PHP／DB live／独立experimentを再実行せず、WP45 API310,363ケース／hooks572／dispatcher405と前回の独立19件・3 fixture各39項目を維持する。

## Linux host物理RAMの観測候補（2026-10-26）

serverのmemory_observation moduleに、stdだけで/proc/meminfoを読む独立候補を追加した。MemTotalとMemAvailableを必須とし、MemFreeを代替にしない。kBは1024bytesとしてchecked計算し、64KiBまでの入力（超過検出に1byteだけ追加）、重複・欠落・不正単位／符号／数値・overflow・total0・available>totalを検査する。読取りで値を観測するだけで、予算の予約や容量選定をしない。

Windowsを含む非LinuxはUnsupportedを返す。workspaceのunsafe_code=forbidを維持し、手動FFI・allow例外・新依存を追加していない。将来のsafe Windows wrapper採用はDEV-03の未決事項。この候補はhostのusable RAMとavailable推定値で、cgroup／job object／process RSS、共有poolの余裕、製品配分や自動算式を提供しない。CLI／cacheへ未接続で、CLIの保存・ヒット停止は継続する。実監視が完成したとは扱わない。

Windows上のparser等4件とUnsupportedを含め、target/memory-observation-and-html-class-check.logでworkspace Rust173件・Python44件、HTTP・Clippy・Linux向けcross-checkが成功。Linux native実読は未実行。WP35 API・249,241ケースは直前のtarget/html-class-filter-full-check.logで比較済みで、memory追加後はWPソース無変更のためPHP再実行を省略しhash照合した。DB実測は直前のbyte-preflight checkpoint（Rust152件＋独立experiment5件、両DB各36項目）を保持する。

根拠は[Linux公式proc仕様](https://docs.kernel.org/filesystems/proc.html)と[kB出力の公式実装](https://github.com/torvalds/linux/blob/master/fs/proc/meminfo.c)。MemAvailableはswapなしに新applicationへ利用可能な量の推定であり、cacheへ配ってよい容量とは異なる。Windowsは[MEMORYSTATUSEX公式仕様](https://learn.microsoft.com/windows/win32/api/sysinfoapi/ns-sysinfoapi-memorystatusex)と[GlobalMemoryStatusEx公式仕様](https://learn.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-globalmemorystatusex)を調査したが、現在の実装対象に含めていない。

## caller指定cgroup v2の1 level観測（2026-10-26）

cgroup_memory_observation::observe_cgroup_v2_levelは、callerが指定したdirectoryのmemory.current／memory.max／memory.highをstdだけで読み、current_bytesとmax_bytes／high_bytesを返す。limitのNoneは`max` tokenであり、read失敗・祖先制約なしの意味ではない。各fileは128bytesまで（超過検出に1byte追加）、ASCII unsigned decimalとoptional final LFだけを受け、missing／不正値／overflowを拒否する。単一levelのraw観測候補で、membership・mount・namespace・祖先を探索せず、指定pathがcgroupfsに属することも証明しない。

3回のreadは非atomicで、currentは子孫の使用量を含む。memory.maxは一時的な超過を許す場合があり、memory.highはthrottle／reclaimの境界なのでcurrent>max／highやhigh>maxを不正値として捨てない。観測値の順序を補正して配分量を作らない。[公式cgroup v2 memory interface](https://docs.kernel.org/admin-guide/cgroup-v2.html#memory)

非LinuxはUnsupported。新依存・unsafe例外・新権限を追加せず、実効headroom・cgroup v1・祖先制約・process RSS・Windows job・共有pool・自動容量式・CLI cacheへ接続しない。5件のparser／bounded file／Unsupported等をWindowsで検証しLinux向けcross-checkが通過したが、native Linuxのcgroup実読は未実行。実監視・製品の自動配分が完成したものではない。

## self membershipと可視祖先の限定観測（2026-10-27）

別module cgroup_membership_observationは、boundedな/proc/self/cgroupとmountinfoを照合し、unique cgroup2 mount root `/`の場合だけleaf→可視境界のmemory.current／max／highを読む。各levelのdomain、submount／検出symlink拒否、前後のproc内容とnamespace target device／inodeの不変を検査する。外祖先は常にUnknown、全読取りはNonAtomicで、全可視maxを無制限容量へ変換しない。namespaceは表示を仮想化するため、root `/`もホスト全階層の根の証明ではない。[公式namespace仕様](https://docs.kernel.org/admin-guide/cgroup-v2.html#namespace)

境界の3file全部NotFoundだけBoundaryUnobservedとする。途中の全欠落、境界の一部欠落、PermissionDenied、不正値はエラーであり、max tokenのNoneと混同しない。限定byte path・escape・mount root条件、TOCTOU／ABA／真正性の未保証、安全なfd resolverの残件と単体検証範囲は[資源観測開発記録](cgroup-memory-development.ja.md)を参照。新unit5件はWindowsで成功しLinux向けcompileも通過したが、native Linux観測は未実行。unsafe／依存を追加せず、自動算式・共有pool・CLI cacheへ未接続のまま保存・ヒット停止を維持する。
