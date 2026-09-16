# マイルストーン10の検証メモ

実装範囲・実装結果の正本は`PLANS.md`のマイルストーン10とする。
マイルストーン11以降は今回の対象外。

- `do-plan`スキルの実行禁止規則に従い、import、テスト、ビルド、Bot起動、SDK実行による検証は行っていない。
- 追加した`src/arbitrage/tests.rs`のテストと、3 DEXの実snapshotに対する公式実装とのraw amount照合は未実行。
- 実行検証を行う際は、まずオフライン単体テストを実施し、次に同一snapshot・同一入力・同一方向で公式参照値と比較する。ネットワーク／SDKを使うfixture生成は別途明示実行する。
- 実fixture照合では特にRaydium未回収PnL、Whirlpool tick越境時の1 raw丸め、Meteora variable feeの時間減衰とbin越境を確認する。
