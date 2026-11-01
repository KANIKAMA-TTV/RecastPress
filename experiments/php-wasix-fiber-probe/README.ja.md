# PHP FiberのWASIX backend実験

PHP `f4bfae79d5c19028349082f707930cf0951d23b9`を対象とする独立P0実験。既存の[小C実証](../php-wasix-context-probe/README.ja.md)で専用stackの前提を確認したが、PHP Fiberの移植・製品互換性はまだ受け入れていない。

## 通常PHPの比較基準

```text
python experiments/php-wasix-fiber-probe/native_contract.py --record
```

[固定PHP fixture](fixtures/fiber_contract.php)は、動的4 Fiberの終了順/再帰local、入れ子のstart/resume、値/return、throw/catch/finally、suspend中Fiberのcycle GC/WeakReference、API errorの6ケースをJSONの型・値・traceで比較する。通常PHP8.5.12の実行で[基準記録](evidence/native-fiber-contract.json)を保存した。改変14件と不正JSON2件の拒否は、実PHPケース数へ加算しない。

CLIは`-n -d fiber.stack_size=2097152`を固定する。この2MiBは比較を揃える試験設定であり、製品defaultを決めるものではない。元ZIPと配布78ファイルの完全な集合/全SHA、source不変、10秒期限、所有processの回収を実行前後で確認する。共通verifierのWP3 source副検証はreceiptへ明記するが、この試験でWPは実行しない。公開記録へローカル絶対pathやobject IDを保存しない。

## backendの境界

次の試作はNTS・WASI thread1・fresh WASI instance 1要求に限定する。ZendのVMstate/observer/transfer/Throwable/finally/GCを維持し、u64 context IDはprivate descriptorへ格納する。初回stackless entryとraw-switch直後のSP復元を使い、固定2slotの小C adapterをそのまま汎用Fiberへ持ち込まない。

WASIXの取消しは遅延するため、PHPの論理cleanupと物理Cstackの寿命を分ける。PHP request-endで物理領域まで解放せず、ZendMM request heap外に保持してWASI executor/instance終了へ委ねる案を検証する。guest内free/reuse、stack guard、persistent/ZTS、即時全資源回収、OOMの発生時点は別の確認事項である。native基準の成功をpatched PHPの成功へ付け替えない。
