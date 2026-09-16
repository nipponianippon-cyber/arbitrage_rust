# Raydium・Orca・Meteora-DLMM対応のSolana DEX価格監視Bot実装計画

このPLANS.mdは`SPEC.md`、リポジトリ内の既存実装、実装中に確認した公式ドキュメント、公式SDKまたは公式実装、保存済みfixture、手動確認結果を根拠に更新する。根拠にした外部仕様や実装上の判断は、必要に応じて「根拠メモ」または「実装メモ」に記録する。

## 目的 / 全体像

ローカル環境で動作するRust製Botを実装する。BotはHeliusのSolana HTTP RPCを使い、設定ファイルで明示されたRaydium、Orca、Meteora-DLMMのSOL/USDCプール状態を30秒ごとに取得する。各DEXのオンチェーンプール状態から価格をBot内部で算出し、全DEXで比較基準を`USDC per SOL`に統一する。

各監視サイクルでは、Raydium、Orca、Meteora-DLMMの全組み合わせについて価格差、価格差率、高いDEX、安いDEX、手数料考慮後の参考差分を計算し、Discord WebhookのEmbedで通知する。同じ監視結果、価格差、Meteora-DLMM固有状態、エラー情報はSQLiteに保存する。

初期実装は価格監視専用とする。自動売買、トランザクション作成、秘密鍵またはウォレット管理、署名、トランザクション送信、Jupiterなどのオフチェーン集約APIによる価格取得、Helius WebSocket、Enhanced API、Webhookは実装しない。

2026-09-09の追加依頼により、マイルストーン10の裁定候補判定を実装対象へ追加する。マイルストーン11の送信直前再quoteには着手しない。専用小額ウォレット、自分の資金による実行、通常RPCによるsimulation・送信、Flashloan、Jito送信、Jito Bundleも引き続き対象外とする。

## 進捗

- [x] `SPEC.md`を根拠に、初期実装の対象と非対象範囲を整理した。
- [x] `SPEC.md`を根拠に、PLANS.mdをMeteora-DLMM、全組み合わせ価格差、Discord Embed、SQLiteログ、fixture取得補助を含む計画へ更新した。
- [x] 設定読み込みと設定バリデーションを実装する。
- [x] Helius HTTP RPCクライアントを実装する。
- [x] DEX共通インターフェースとRaydium価格デコードを実装する。
- [x] Orca価格デコードを実装する。
- [x] Orca Whirlpool価格のmint decimal補正を実装する。
- [x] Meteora-DLMMのLbPair、mint account、必要に応じたBinArray取得と価格デコードを実装する。
- [x] 手数料、スリッページ、価格差計算を実装する。
- [x] SQLite保存を実装する。
- [x] Discord Embed通知を実装する。
- [x] Discord Embedの短縮・整形強化を実装する。
- [x] 30秒周期の監視ループを実装する。
- [x] 開発・テスト用fixture取得補助を実装する。
- [ ] Meteora-DLMM active bin価格式を公式SDK出力fixtureで照合する。
- [ ] 単体テスト、結合テスト、手動確認を実施する。
- [x] 将来の裁定候補判定・自動売買拡張のマイルストーンを定義する（マイルストーン10着手前の計画整理）。
- [x] マイルストーン10のlocal quote、整数投入量探索、固定費控除、snapshot検査、候補保存・状態変化通知を実装する。
- [ ] マイルストーン10の追加テストを実行し、3 DEXの同一snapshot・同一入力による公式実装とのraw amount照合を行う（静的確認のみ実施）。

## スコープ

初期実装に含める。

- Helius HTTP RPCによるSolanaオンチェーンデータ取得
- RaydiumのSOL/USDCプール価格監視
- Orca WhirlpoolのSOL/USDCプール価格監視
- Meteora-DLMMのSOL/USDC LbPair価格監視
- `.env`と`config.toml`による設定読み込み
- 30秒間隔の定期監視
- Raydium、Orca、Meteora-DLMMの全組み合わせ価格差計算
- Discord Webhook Embedによる通常通知と異常通知
- SQLiteへの監視結果、価格差、Meteora-DLMM固有状態、エラー保存
- RPCエラー、価格取得失敗、Discord通知失敗などの異常検知と通知
- 開発・テスト用の実Solanaアカウントfixture取得補助

初期実装に含めない。

- 自動売買
- トランザクション作成
- 秘密鍵またはウォレット管理
- 署名
- トランザクション送信
- Jupiterなどのオフチェーン集約APIを使った価格取得
- Helius WebSocket、Enhanced API、Webhookの利用
- 複数トークンペアの動的探索
- Orca旧Constant Product AMM/CPMMプール対応
- Meteora-DLMMプールの自動探索
- 裁定利益の確定判定
- 裁定候補判定の実装
- 3 DEXの裁定用local AMM mathによるquote実装
- 投入量の数値最適化
- expectedProfitとminProfitThresholdによる候補選別
- 送信直前の再quoteと採算性確認
- 専用小額ウォレット、署名、トランザクション作成・送信
- 自分の資金による実取引
- Flashloan提供元との統合
- Jito送信およびJito Bundle送信
- fixtureの自動更新
- fixtureファイルのGit管理
- 本番実行時のfixture利用

## 前提条件

実装言語はRustとする。Botはローカル環境で実行する。Solana RPCプロバイダはHeliusとし、初期実装ではHTTP RPCのみを利用する。

監視対象プールはBotが自動探索せず、設定ファイルで明示指定する。初期実装の監視対象ペアはSOL/USDCのみとする。Meteora-DLMMのLbPairアドレスも設定ファイルで手動指定し、自動探索は将来対応とする。

価格はオンチェーンプール状態からBot内部で算出する。価格比較の基準は全DEXで`USDC per SOL`に統一する。DEX手数料とスリッページは考慮するが、初期実装ではSPECの推奨設計に従い、想定取引サイズが必要な処理は設定で明示する。

Meteora-DLMMの実装では公式SDKまたは既存crateの利用を許可する。

## システム構成

SPECで推奨されているモジュール境界に沿って実装する。

- `config`: `.env`と`config.toml`の読み込み、設定値検証
- `rpc`: Helius RPCクライアント
- `dex`: DEX共通インターフェース
- `dex::raydium`: Raydiumプールデコードと価格計算
- `dex::orca`: Orcaプールデコードと価格計算
- `dex::meteora_dlmm`: Meteora-DLMM LbPair/BinArrayデコード、active bin価格、quote計算
- `pricing`: 価格差、手数料、スリッページ計算
- `storage`: SQLite保存
- `notifier`: Discord通知
- `errors`: エラー型と分類
- `runner`: 30秒周期の監視ループ
- `arbitrage`: マイルストーン10の整合snapshot、local quote、投入量探索、候補評価

データフローは次の通り。

1. Botが`.env`と`config.toml`を読み込む。
2. 設定値を検証し、監視対象プール一覧を構築する。
3. 30秒ごとにHelius HTTP RPCからプールアカウントを取得する。
4. DEX別デコーダでプール状態を解釈する。
5. SOL/USDC価格を`USDC per SOL`として算出する。
6. DEX間価格差を全組み合わせで計算する。
7. 結果をSQLiteへ保存する。
8. Discordへ価格差を通知する。
9. エラー発生時はSQLiteへ記録し、必要に応じてDiscordへ異常通知する。

## 裁定候補判定と将来拡張方針

以下のうち候補判定までをマイルストーン10として実装する。送信直前再quote以降は将来マイルストーンとして管理する。上記「初期実装に含めない」はマイルストーン1〜9時点の範囲を表し、今回の追加範囲はマイルストーン10の記載を優先する。

### 裁定候補判定

裁定候補判定と後続の送信直前再quoteでは、次の順序を固定する。1〜6をマイルストーン10、7をマイルストーン11の範囲とする。

1. Raydium、Orca Whirlpool、Meteora-DLMMのpool状態を更新し、RPC context slotを記録する。
2. 同一ペア・同一方向について、安いDEXで`USDC -> SOL`、高いDEXで`SOL -> USDC`を行う粗い価格差を検出する。逆方向も同じ条件で評価する。
3. Raydium、Orca Whirlpool、Meteora-DLMMの3 DEXすべてについて、裁定実行用のlocal AMM mathでquoteを計算する。Jupiter等の集約APIは使わない。公式SDKまたは公式実装は、local mathの照合とテスト用途に限定する。
4. raw amountの整数単位で投入量を探索する。投入量の上下限、流動性、partial fill、slippage上限などの実行可能範囲を先に決め、粗い探索、利益の高い区間の再探索、区間端とtick/bin境界の確認を組み合わせる。単一のBrent法だけに依存しない。
5. DEX fee、Solana base fee、priority fee、Jito tip、Flashloan fee、slippage bufferを差し引く。初期の将来実装ではこれらのコストを固定設定値として扱う。
6. `expectedProfit > minProfitThreshold`を満たす場合だけ候補として保存・通知する。利益のcanonical unitはUSDC raw amountとし、設定値と表示値はUSDCのDecimalへ変換する。
7. 送信直前に全対象状態を再取得し、同じlocal mathで再quoteする。再quote後も採算性が残る場合だけtransaction作成へ進む。

状態取得は、pool本体、vault、tick/bin arrayを可能な限り同じsnapshotとして扱う。RPC呼び出しが複数回に分かれる場合はcontext slotの最大差を初期値2 slotsまで許容し、超過時は全状態を再取得して1回だけ再試行する。それでも整合しない場合は候補を破棄する。

候補判定結果は毎回SQLiteへ保存し、Discord通知は新規候補、利益の意味のある変化、再quote結果、送信結果などの状態変化に限定する。将来の状態名は少なくとも`detected`、`requoted_profitable`、`requoted_unprofitable`、`quote_failed`、`transaction_built`、`submitted`、`confirmed`、`simulation_failed`、`send_failed`、`expired`を想定する。

### 実行拡張の段階

裁定候補判定の後に実取引を追加する場合は、次の順序で分離する。

1. 専用の小額ウォレットを用意し、既存の資産保管ウォレットとは分離する。
2. Flashloanを使わず、専用ウォレットの自分の資金だけでsimulation・実行を検証する。
3. 通常RPCでsimulation、transaction送信、confirmation確認を検証する。
4. 対応mint、流動性、fee、同一transaction内のborrow/repay条件を確認したうえでFlashloan提供元を決定する。
5. 通常RPC送信と分離したJito送信経路を追加し、priority feeとJito tipを別コストとして記録する。
6. 必要性を確認した後、Jito Bundleへ拡張する。

上記6段階は今回の実装対象外とし、今回の追加範囲は裁定候補の計算・保存・通知までとする。

## 設定設計

起動時に`.env`と`config.toml`を読み込む。

`.env`には少なくとも次を定義する。

- `HELIUS_RPC_URL`
- `DISCORD_WEBHOOK_URL`

`config.toml`には少なくとも次を定義する。

- 監視間隔
- 対象DEX
- 対象トークンペア
- Raydiumプールアドレス
- Orcaプールアドレス
- Meteora-DLMM LbPairアドレス
- SQLite DBパス
- 通知設定
- エラー通知設定
- 手数料・スリッページ計算設定

推奨構成は次の通り。

```toml
[bot]
interval_seconds = 30
pair = "SOL/USDC"

[database]
path = "data/arbitrage_monitor.sqlite"

[pricing]
consider_dex_fee = true
consider_slippage = true
trade_size_usdc = 100.0
price_orientation = "usdc_per_sol"
meteora_dlmm_bin_array_count = 4
meteora_dlmm_slippage_bps = 50

[notification]
discord_enabled = true
discord_embed_enabled = true
notify_every_cycle = true
notify_on_error = true
bot_name = "solana-dex-price-monitor"
environment = "local"

[notification.embed_colors]
normal = 3447003
warning = 16776960
error = 15158332

[[pools]]
dex = "raydium"
pair = "SOL/USDC"
pool_address = "未定"
base_mint = "So11111111111111111111111111111111111111112"
quote_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
enabled = true

[[pools]]
dex = "orca"
pair = "SOL/USDC"
pool_address = "未定"
base_mint = "So11111111111111111111111111111111111111112"
quote_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
enabled = true

[[pools]]
dex = "meteora_dlmm"
pair = "SOL/USDC"
lb_pair_address = "未定"
base_mint = "So11111111111111111111111111111111111111112"
quote_mint = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
price_orientation = "usdc_per_sol"
auto_discovery = false
enabled = true
```

SPEC本文には具体的なmainnetプールアドレスを固定しない。開発、テスト、手動確認に使う具体的なSOL/USDCプールアドレスは`config.example.toml`に記載する。

設定バリデーションでは少なくとも次を検証する。

- 監視間隔が設定されていること
- 対象ペアが初期実装の対象である`SOL/USDC`であること
- 価格方向が`usdc_per_sol`であること
- RaydiumとOrcaでは`pool_address`が指定されていること
- Meteora-DLMMでは`lb_pair_address`が指定されていること
- Meteora-DLMMの`auto_discovery`は初期実装ではfalseであること
- `consider_slippage = true`の場合、想定取引サイズが正の値で指定されていること
- `meteora_dlmm_bin_array_count`が設定されている場合、1以上の値であること
- `meteora_dlmm_slippage_bps`が設定されている場合、0以上のbps値であること
- SQLite DBパス、通知設定、エラー通知設定が読み込めること
- `.env`から`HELIUS_RPC_URL`と`DISCORD_WEBHOOK_URL`が読み込めること

## 主要データ構造

SPECに合わせ、少なくとも次の概念を実装する。

```text
PoolConfig
- dex: DexKind
- pair: String
- pool_address: String
- lb_pair_address: Option<String>
- base_mint: String
- quote_mint: String
- price_orientation: String
- enabled: bool
```

```text
DexPrice
- dex: DexKind
- pair: String
- pool_address: String
- price: Decimal
- fee_adjusted_price: Option<Decimal>
- slippage_adjusted_price: Option<Decimal>
- liquidity: Option<Decimal>
- slot: Option<u64>
- observed_at: DateTime
```

```text
PriceSpread
- pair: String
- dex_a: DexPrice
- dex_b: DexPrice
- absolute_spread: Decimal
- spread_bps: Decimal
- higher_dex: DexKind
- lower_dex: DexKind
- calculated_at: DateTime
```

```text
MonitorError
- occurred_at: DateTime
- component: String
- severity: ErrorSeverity
- message: String
- source: Option<String>
```

```text
MeteoraDlmmState
- lb_pair_address: String
- active_id: i32
- bin_step: u16
- token_x_mint: String
- token_y_mint: String
- base_fee_bps: Option<Decimal>
- variable_fee_bps: Option<Decimal>
- total_fee_bps: Option<Decimal>
- status: Option<String>
- liquidity: Option<Decimal>
- slot: Option<u64>
- observed_at: DateTime
```

`base_fee_bps`、`variable_fee_bps`、`total_fee_bps`は、LbPairから直接デコードしたu64値ではなく、Meteora公式SDK互換の手数料式で算出したraw fee rateをbpsへ変換した値として保存する。

```text
MeteoraDlmmQuote
- lb_pair_address: String
- direction: MeteoraDlmmQuoteDirection
- input_mint: String
- output_mint: String
- requested_input_amount: Decimal
- requested_input_amount_raw: u64
- consumed_input_amount: Option<Decimal>
- consumed_input_amount_raw: Option<u64>
- output_amount: Option<Decimal>
- output_amount_raw: Option<u64>
- fee_amount: Option<Decimal>
- fee_amount_raw: Option<u64>
- protocol_fee_amount: Option<Decimal>
- protocol_fee_amount_raw: Option<u64>
- price_impact_bps: Option<Decimal>
- effective_price: Option<Decimal>
- end_price: Option<Decimal>
- bin_array_count: usize
- bin_array_addresses: Vec<String>
- partial_fill: bool
- success: bool
- error_message: Option<String>
- slot: Option<u64>
- observed_at: DateTime
```

`MeteoraDlmmQuoteDirection`は`UsdcToSol`と`SolToUsdc`を持つ。`pricing.trade_size_usdc`は両方向quoteで流用し、`USDC -> SOL`では入力USDC量としてそのまま使う。`SOL -> USDC`では同一観測サイクルのMeteora active bin価格から同等USDC価値のSOL量へ換算した値を入力量とする。

## RPC設計

Helius HTTP RPCを利用する。価格監視では複数プールの状態をまとめて取得できるため、可能であれば`getMultipleAccounts`を優先する。初期実装で利用するRPCメソッド候補は次の通り。

- `getAccountInfo`
- `getMultipleAccounts`
- `getTokenAccountBalance`
- `getLatestBlockhash`

RPCクライアントは次を扱う。

- Helius RPC URLの読み込み
- JSON-RPCリクエスト送信
- RPC context slotの取得
- account owner、lamports、base64 encoded account dataの取得
- RPCレスポンス不正、アカウント未取得、接続失敗のエラー化
- 複数アカウント取得時の成功・失敗の対応付け

Helius WebSocket、Enhanced API、Webhookは初期実装では利用しない。

## DEX別価格取得

Raydium、Orca、Meteora-DLMMの各実装は、DEX固有のプール形式に従ってアカウントデータをデコードし、SOL/USDC価格を算出する。比較用の最終価格は全DEXで`USDC per SOL`へ正規化する。

Raydiumでは、設定されたSOL/USDCプールアドレスからプール状態を取得し、必要な残高または現在価格に基づいて価格を算出する。

Orcaでは、対象プール種別をWhirlpoolに限定する。旧Orca Constant Product AMM/CPMMは初期実装に含めない。

Orca Whirlpoolでは、設定されたSOL/USDC WhirlpoolアドレスからWhirlpool本体を取得し、Whirlpool上の`token_mint_a`、`token_mint_b`、`token_vault_a`、`token_vault_b`、`fee_rate`、`sqrt_price`をデコードする。さらに`token_mint_a`と`token_mint_b`のmint accountを追加取得し、mint account dataからtoken A/B decimalを読み取る。

Orca Whirlpoolの基準価格は、公式SDKまたは公式実装の`sqrt_price_to_price`相当の挙動に合わせる。Whirlpoolの`sqrt_price`はQ64.64形式として扱い、token Aをtoken Bで評価する価格を次の形で算出する。

```text
price_b_per_a = (sqrt_price / 2^64)^2 * 10^(decimals_a - decimals_b)
```

Whirlpoolのtoken A/Bが設定上のbase/quoteと同じ向きなら`price_b_per_a`をそのまま`USDC per SOL`として扱う。token A/Bが設定上のquote/baseと逆向きなら、0除算を避けたうえで`1 / price_b_per_a`に反転して`USDC per SOL`へ正規化する。

丸め誤差を抑えるため、実装では可能な限り整数演算、`Decimal`、または公式Rust core SDKを利用する。`f64`を使う場合は最終表示または比較直前に限定し、通常テストまたは明示実行の照合テストで許容bpsを固定する。

Meteora-DLMMでは、初期実装の監視価格は現在のactive bin価格とする。LbPairアカウントから次を取得する。

- `active_id`
- `bin_step`
- token X/Y mint
- 手数料計算に必要な`parameters`
- 手数料計算に必要な`vParameters`
- プール状態

Meteora-DLMMのtoken X/Y decimalはLbPair上の値として扱わない。LbPairから取得したtoken X/Y mintアドレスのmint accountを追加取得し、mint account dataから読み取る。

Meteora-DLMM active bin価格式は、Meteora公式TypeScript SDKを明示実行して生成した参照fixtureで照合する。照合対象はactive bin価格式のみとし、LbPair decode offset、手数料式、BinArray quote、スリッページ計算はこの照合スコープに含めない。

SDK参照fixtureには、同じSDK実行内で取得または算出した`lb_pair_address`、`active_id`、`bin_step`、token X/Y mint、token X/Y decimals、SDKのprice per lamport、SDKのUI価格、`USDC per SOL`へ正規化した期待価格、SDKパッケージ名、SDKバージョン、生成時刻を保存する。active binは時間で変化するため、Rust側の通常テストではlive RPCを叩かず、fixture内の`active_id`、`bin_step`、decimals、期待価格だけを使って価格式を検証する。

通常の`cargo test`は、保存済みSDK参照fixtureを読むだけで成功する。Meteora公式SDKを実行してfixtureを再生成する処理は、Node/TypeScript依存とネットワーク依存を持つ明示実行コマンドとして通常テストから分離する。

active bin価格式の許容誤差は原則`0.01 bps`以内とする。既存の`f64`ベース実装がこの許容誤差を満たせない場合は、SDK fixtureに合わせて`Decimal`または高精度計算へ置き換える。

スリッページ計算を行う場合は、active bin周辺のBinArrayも取得し、Meteora公式SDKまたは公式Rust integration相当のquoteロジックを利用して想定取引サイズに対する価格インパクトを算出する。quote方向は`USDC -> SOL`と`SOL -> USDC`の両方向とする。

BinArray取得では、Meteora公式SDKの`getBinArrayForSwap(swapForY, count)`または公式Rust integrationの`get_bin_array_pubkeys_for_swap`相当の挙動に合わせる。初期値は両方向それぞれ`count = 4`とし、設定の`pricing.meteora_dlmm_bin_array_count`で変更できるようにする。`swapForY = true`または`swap_for_y = true`はtoken Xからtoken Yへのswap、falseはtoken Yからtoken Xへのswapとして扱う。SOL/USDCのbase/quote方向とLbPairのtoken X/Y方向を照合し、`USDC -> SOL`と`SOL -> USDC`を正しいswap方向へ変換する。

quote計算はactive bin価格取得とは独立した補助結果として扱う。Meteora-DLMMのactive bin価格、mint decimal、手数料計算に成功していれば、両方向quoteまたはいずれか一方向quoteが失敗してもそのサイクルのMeteora価格取得は成功扱いにする。quote失敗時は`slippage_adjusted_price = None`または成功した方向のみの値とし、失敗詳細はSQLiteのquote詳細テーブルと`monitor_errors`へ保存する。Discord通知は既存方針通りMeteora-DLMM固有詳細を表示しない。

## 価格計算

各DEXのプール状態から次を算出する。

- SOL建てまたはUSDC建ての基準価格
- DEX手数料を考慮した価格
- 想定取引サイズが設定されている場合の価格インパクト
- 最終的な比較用価格

初期実装での推奨方針は次の通り。

- 基本価格はプール残高または現在価格から算出する。
- DEX手数料は設定値として持ち、比較用価格に反映する。
- 価格インパクト計算は実装可能な範囲で関数を分離する。
- スリッページ計算を有効にする場合、設定ファイルで想定取引サイズを必須にする。
- Meteora-DLMMではactive bin価格を基準価格とし、手数料とスリッページは公式SDKまたは公式Rust integration相当のquote結果に基づいて算出する。
- Meteora-DLMMのquoteは`USDC -> SOL`と`SOL -> USDC`の両方向で計算する。
- Meteora-DLMMのquote入力サイズは既存の`pricing.trade_size_usdc`を流用する。`SOL -> USDC`では、active bin価格で同等USDC価値になるSOL量を算出して入力量に変換する。
- Meteora-DLMMのquoteでSDKへ渡すallowed slippageは`pricing.meteora_dlmm_slippage_bps`を使い、初期値は50bpsとする。
- Meteora-DLMMのquote失敗はactive bin価格取得失敗とは分離し、価格監視、価格差計算、Discord通知は継続する。失敗理由と取得できた部分結果はSQLiteへ保存する。
- 将来自動売買を行う場合、想定取引サイズごとの見積もり計算を追加する。

Meteora-DLMMの手数料計算は次のSPEC指定に従う。

- base feeとvariable feeはLbPair上の`base_fee_bps`や`variable_fee_bps`という固定フィールドから直接読まない。
- base fee raw rateは`baseFactor * binStep * 10 * 10^baseFeePowerFactor`で算出する。
- variable fee raw rateは、`variableFeeControl > 0`の場合に`ceil(variableFeeControl * (volatilityAccumulator * binStep)^2 / 100_000_000_000)`で算出する。
- `variableFeeControl == 0`の場合、variable fee raw rateは0とする。
- total fee raw rateは`base fee + variable fee`を`MAX_FEE_RATE`で上限クリップする。
- 公式SDKの`FEE_PRECISION`は`1_000_000_000`なので、SQLiteへ保存するbps値は`raw_rate / 100_000`として扱う。

## 価格差計算

Raydium、Orca、Meteora-DLMMのSOL/USDC価格を全組み合わせで比較する。比較対象は次の3組。

- Raydium vs Orca
- Raydium vs Meteora-DLMM
- Orca vs Meteora-DLMM

各比較では少なくとも次を算出する。

- Raydium価格
- Orca価格
- Meteora-DLMM価格
- 絶対価格差
- 価格差率
- 高いDEX
- 安いDEX
- 手数料考慮後の参考差分

裁定判定しきい値は未定とする。初期実装では、価格差の大小にかかわらず、監視サイクルごとに全組み合わせの価格差をDiscordへ通知する。

## SQLite保存

SQLiteには少なくとも次のテーブルを用意する。

- `price_observations`
- `price_spreads`
- `meteora_dlmm_states`
- `meteora_dlmm_quotes`
- `monitor_errors`

`price_observations`にはDEXごとの価格取得結果を保存する。保存対象は少なくとも次を含む。

- 監視時刻
- 対象ペア
- DEX名
- プールアドレス
- Meteora-DLMM LbPairアドレス
- 算出価格
- 手数料考慮後価格
- RPCレスポンスの成否
- エラー種別
- エラーメッセージ

`price_spreads`にはDEX間の比較結果を保存する。保存対象は少なくとも次を含む。

- 監視時刻
- 対象ペア
- 比較対象DEX
- 価格差
- 価格差率
- 高いDEX
- 安いDEX
- 手数料考慮後の参考差分

`meteora_dlmm_states`にはMeteora-DLMM固有状態を保存する。保存対象は少なくとも次を含む。

- Meteora-DLMMの`active_id`
- Meteora-DLMMの`bin_step`
- Meteora-DLMMのbase fee bps
- Meteora-DLMMのvariable fee bps
- Meteora-DLMMのtotal fee bps
- Meteora-DLMMのstatus
- Meteora-DLMMのliquidity
- RPC slot
- 観測時刻

`meteora_dlmm_quotes`にはMeteora-DLMMの両方向quote詳細を保存する。保存対象は少なくとも次を含む。

- 観測時刻
- LbPairアドレス
- quote方向
- input mint
- output mint
- requested input amount
- requested input amount raw
- consumed input amount
- consumed input amount raw
- output amount
- output amount raw
- fee amount
- fee amount raw
- protocol fee amount
- protocol fee amount raw
- price impact bps
- effective price
- end price
- 取得したBinArray数
- 使用したBinArrayアドレス
- partial fill有無
- quote成功有無
- quote失敗理由
- RPC slot

raw amountはSPL token amountの`u64`値なので、SQLite上では符号付きINTEGERの上限に依存しないようTEXTとして保存する。

`monitor_errors`にはRPC、デコード、価格計算、DB保存、Discord通知のエラーを保存する。

## Discord通知

Discord通知はWebhookのEmbed機能を使う。通常の価格差通知と異常通知は別々のEmbedテンプレートを使う。

価格差通知Embedには少なくとも次を含める。

- 監視時刻
- 対象ペア
- Raydium価格
- Orca価格
- Meteora-DLMM価格
- 価格差
- 価格差率
- 高いDEX
- 安いDEX
- 比較方向
- RPC取得slot
- 手数料考慮後の参考差分
- エラー有無

Meteora-DLMM固有の`active_id`、`bin_step`、fee、statusなどの詳細情報はDiscordには表示しない。これらはSQLiteへ保存する。

Embedの表示仕様は次の通り。

- `title`には対象ペアと通知種別を表示する。
- `description`には価格差の概要を短く表示する。
- `color`は通知種別に応じて変更する。
- `fields`にはDEX別価格、価格差、価格差率、高いDEX、安いDEX、slot、取得時刻を分けて表示する。
- `timestamp`には監視時刻を設定する。
- `footer`にはBot名、実行環境、RPC種別を表示する。

Discord Embedの短縮・整形ルールは次の通り。

- SQLiteへ保存する値は丸めず、Discord表示専用のformatterで丸める。
- DEX価格は小数4桁固定で表示する。
- 価格差USDCは小数6桁固定で表示する。
- bpsは小数2桁固定で表示し、`0.01 bps`単位へ丸める。
- 末尾ゼロは残し、Discord上で数値の桁が揃う表示にする。
- 通常通知Embedの`fields`は最大12個を初期上限とする。Discord仕様上の上限25個に依存せず、読みやすさを優先する。
- 将来DEXや比較対象が増えて12 fieldsを超える場合は、優先度の低い詳細を`Summary` fieldへまとめる。完全な内容はSQLiteログを正とする。
- 初期実装では短縮・整形用の桁数とfield上限はコード上の定数にし、`config.toml`の設定項目は増やさない。

異常発生時は通常の価格差通知とは別に、エラー通知Embedを送信する。エラー通知Embedには少なくとも次を含める。

- 発生時刻
- コンポーネント名
- 重要度
- エラーメッセージ
- 対象DEX
- 対象プールアドレス
- リトライ予定の有無
- 連続エラー回数

エラー通知Embedの短縮・整形ルールは次の通り。

- `description`のエラーメッセージは最大500文字に短縮し、超過時は末尾に`...`を付ける。
- `Source` fieldは最大300文字に短縮し、超過時は末尾に`...`を付ける。
- PoolアドレスはDiscord表示上は`先頭8文字...末尾8文字`へ短縮する。完全なアドレスはSQLiteへ保存する。
- エラー通知Embedも最大12 fieldsを初期上限とし、超過する詳細は`Summary` fieldへまとめる。

Discordのフィールド数、文字数、ペイロードサイズの制限を超えないようにする。制限超過が見込まれる場合は詳細情報を短縮し、完全な内容はSQLiteログへ保存する。

## エラー処理

次のエラーを分類して扱う。

- 設定ファイル不備
- Helius RPC接続失敗
- RPCレスポンス不正
- プールアカウント未取得
- DEXプールデコード失敗
- Meteora-DLMM LbPairデコード失敗
- Meteora-DLMM BinArray取得失敗
- Meteora-DLMM quote計算失敗
- 価格計算失敗
- SQLite保存失敗
- Discord通知失敗

推奨挙動は次の通り。

- 設定ファイル不備は起動時に検出し、Botを停止する。
- 一時的なRPC失敗はリトライ対象とする。
- 同一コンポーネントで連続エラーが発生した場合はDiscordへ異常通知する。
- Raydium、Orca、Meteora-DLMMのいずれか1つでも価格取得に失敗した場合、そのサイクル全体を失敗扱いとし、価格差計算をスキップする。
- Meteora-DLMMのactive bin価格取得に成功し、quoteだけが失敗した場合は、そのサイクルの価格取得は成功扱いにする。quote失敗は`meteora_dlmm_quotes`と`monitor_errors`へ保存し、価格差計算と通常Discord通知は継続する。
- エラーは可能な限りSQLiteへ保存する。
- Discord通知失敗時は標準エラーへ出力する。

## 開発・テスト用fixture取得

DEXデコード検証のため、開発・テスト専用に実Solanaアカウントfixtureを取得できる補助機能を用意する。fixture取得元は初期実装ではHelius HTTP RPCに限定する。

fixture取得機能はBot本体の通常監視機能には含めず、開発用スクリプトまたはテスト補助として扱う。本番実行時にfixtureを読み込んで価格監視する機能は初期実装に含めない。

fixtureにはRPCレスポンスに近いJSON形式で次を保存する。

- リクエストしたアカウントアドレス
- RPCメソッド名
- RPC context slot
- account owner
- lamports
- base64 encoded account data
- 取得時刻
- fixture作成時に使った対象DEXと対象ペア

fixture対象には、pool本体だけでなく、価格デコードに必要な依存アカウントも含める。

- Raydium: base/quote vault
- Orca: Whirlpool本体、token A/B mint account、必要なvault確認用アカウント
- Meteora-DLMM: LbPairとtoken X/Y mint account
- スリッページ検証を行う場合: 両方向quoteに必要なactive bin周辺のBinArray。初期値は各方向`count = 4`で、`pricing.meteora_dlmm_bin_array_count`に従う。

fixtureファイルはGit管理しない。保存先は`tests/fixtures/local/`などのローカル生成ディレクトリを想定し、`.gitignore`で除外する。fixtureの更新は手動で行う。自動更新、定期更新、CI上でのmainnet RPC取得は初期実装に含めない。

Meteora-DLMM公式SDK照合用には、RPCレスポンスfixtureとは別にSDK参照fixtureを用意する。SDK参照fixtureは複数のMeteora-DLMM poolを対象にし、`bin_step`、`active_id`、token X/Y方向、decimal差、両方向quoteが異なるケースを含める。fixture生成は明示実行だけで行い、通常の`cargo test`やCIではSDK、Node、ネットワークを要求しない。

## マイルストーン

### マイルストーン1: 設定読み込み

`.env`と`config.toml`を読み込み、必要な設定値を検証する。Raydium、Orca、Meteora-DLMMの監視対象を明示設定できるようにする。Meteora-DLMMは`lb_pair_address`を使い、自動探索は行わない。

完了条件:

- `HELIUS_RPC_URL`と`DISCORD_WEBHOOK_URL`を読み込める。
- `config.toml`から監視間隔、DBパス、通知設定、価格計算設定、pool設定を読み込める。
- SOL/USDC以外、未指定アドレス、不正な価格方向、スリッページ有効時の想定取引サイズ不足を設定エラーにできる。

### マイルストーン2: Helius RPC取得

Helius HTTP RPCクライアントを実装し、`getMultipleAccounts`を優先して必要なアカウントを取得する。必要に応じて`getAccountInfo`、`getTokenAccountBalance`、`getLatestBlockhash`も使える設計にする。

完了条件:

- 複数アカウントを取得し、account data、owner、lamports、slotを扱える。
- RPC接続失敗、レスポンス不正、アカウント未取得を分類できる。
- Helius HTTP RPC以外のHelius機能を使っていない。

### マイルストーン3: DEX価格デコード

Raydium、Orca Whirlpool、Meteora-DLMMのDEX別デコーダを実装する。Orca WhirlpoolではWhirlpool本体からtoken A/B mintを読み、mint accountからdecimalを読む。Meteora-DLMMではLbPairからactive bin価格に必要な状態を読み取り、token X/Y decimalはmint accountから読む。

完了条件:

- RaydiumのSOL/USDC価格を算出できる。
- Orca WhirlpoolのSOL/USDC価格を、token A/B decimal補正込みで算出できる。
- Meteora-DLMMのactive bin価格を算出できる。
- 各DEXの比較用価格が`USDC per SOL`に統一される。
- Orca Whirlpoolのtoken A/Bがbase/quoteと逆向きの場合でも価格を正しく反転できる。
- Meteora-DLMMのbase fee、variable fee、total feeをSPEC指定式で算出できる。

### マイルストーン4: 価格差と手数料・スリッページ

3 DEXの価格から全組み合わせの価格差を計算する。DEX手数料を比較用価格に反映し、スリッページ計算は想定取引サイズが設定されている場合に扱う。

完了条件:

- Raydium vs Orca、Raydium vs Meteora-DLMM、Orca vs Meteora-DLMMの3比較を算出できる。
- 絶対価格差、価格差率、高いDEX、安いDEX、手数料考慮後の参考差分を得られる。
- Meteora-DLMMでは`USDC -> SOL`と`SOL -> USDC`の両方向quoteを計算し、想定取引サイズに対する実効価格と価格インパクトを保存用データとして得られる。
- Meteora-DLMMのquoteだけが失敗しても、active bin価格が取得できていれば価格差計算を継続できる。
- 裁定判定しきい値に依存せず、毎サイクル通知対象を作れる。

### マイルストーン5: SQLite保存

監視結果、価格差、Meteora-DLMM固有状態、エラーをSQLiteへ保存する。

完了条件:

- `price_observations`へDEXごとの価格取得結果を保存できる。
- `price_spreads`へ全組み合わせ価格差を保存できる。
- `meteora_dlmm_states`へ`active_id`、`bin_step`、手数料、status、liquidityを保存できる。
- `meteora_dlmm_quotes`へ両方向quoteの入力額、出力量、手数料、価格インパクト、使用BinArray、partial fill、成功可否、失敗理由を保存できる。
- `monitor_errors`へRPC、デコード、価格計算、DB保存、Discord通知のエラーを保存できる。

### マイルストーン6: Discord Embed通知

通常通知Embedと異常通知Embedを実装する。通常通知には3 DEX価格と全組み合わせ価格差を含め、Meteora-DLMM固有詳細は含めない。表示専用の丸め、短縮、field上限を適用し、Discord上で読みやすいEmbedに整える。

完了条件:

- 各監視サイクルで価格差通知Embedを送信できる。
- 異常発生時にエラー通知Embedを送信できる。
- Embedに監視時刻、対象ペア、DEX別価格、価格差、価格差率、高いDEX、安いDEX、比較方向、slot、手数料考慮後の参考差分、エラー有無を含められる。
- Discord表示ではDEX価格を小数4桁、価格差USDCを小数6桁、bpsを小数2桁固定で表示できる。
- 通常通知Embedと異常通知Embedの`fields`を最大12個に収め、超過分を`Summary` fieldへまとめられる。
- エラー通知Embedでは長いエラーメッセージ、source、poolアドレスを短縮し、完全な情報はSQLiteログへ保存できる。
- 短縮・整形はDiscord表示にだけ適用し、SQLiteへ保存する数値やエラー本文は丸めたり短縮したりしない。
- Discord通知失敗時にSQLiteへエラー保存し、連続失敗時は標準出力または標準エラーへ出力できる。

### マイルストーン7: 監視ループ

30秒周期の監視ループを実装し、設定読み込み、RPC取得、DEXデコード、価格差計算、SQLite保存、Discord通知、エラー処理を結合する。

完了条件:

- 30秒ごとにRaydium、Orca、Meteora-DLMMの価格を取得する。
- 3 DEXすべての価格取得に成功した場合だけ価格差を計算する。
- いずれか1 DEXでも価格取得に失敗した場合、そのサイクル全体を失敗扱いにして価格差計算をスキップする。
- Meteora-DLMMのquoteのみが失敗した場合は、価格取得失敗として扱わず、quote失敗詳細を保存して価格差計算を継続する。
- エラーをSQLiteへ保存し、必要に応じてDiscordへ異常通知する。

### マイルストーン8: fixture取得とテスト

開発・テスト用fixture取得補助と、fixtureまたはモックRPCレスポンスを使ったテストを実装する。

完了条件:

- Helius HTTP RPCから対象poolと依存アカウントのJSON fixtureを手動生成できる。
- Meteora-DLMMのfixture取得では、LbPair、token X/Y mint account、両方向quoteに必要なBinArrayを保存できる。
- 通常の`cargo test`はネットワークなしで成功する。
- 保存済みfixtureを使い、RPCへ接続せずにDEXデコードを検証できる。
- Helius RPCを実際に叩くテストは通常の`cargo test`から分離し、明示指定時だけ実行される。

### マイルストーン9: Meteora-DLMM公式SDK照合

Meteora公式TypeScript SDKを明示実行して、Meteora-DLMM active bin価格式と両方向quoteの参照fixtureを生成する。Rust実装は保存済みSDK参照fixtureを読み込み、`active_id`、`bin_step`、token X/Y decimalsから算出した価格がSDKの正規化済み期待価格と一致すること、さらに同一入力サイズ、同一方向、同一BinArray条件のquote結果がSDK出力と一致することを検証する。

完了条件:

- SDK fixture生成スクリプトを明示実行し、複数のMeteora-DLMM poolについてactive bin参照値と両方向quote参照値をJSON保存できる。
- SDK参照fixtureには、`lb_pair_address`、`active_id`、`bin_step`、token X/Y mint、token X/Y decimals、SDK price per lamport、SDK UI価格、`USDC per SOL`へ正規化した期待価格、両方向quoteの入力額、出力量、消費入力額、fee、protocol fee、price impact、end price、使用BinArray、partial fill有無、SDKパッケージ名、SDKバージョン、生成時刻を含める。
- 通常の`cargo test`ではSDK参照fixtureを読み込むだけで、Node/TypeScript、Meteora公式SDK、ネットワークを要求しない。
- Rustのactive bin価格式とSDK期待価格の乖離が原則`0.01 bps`以内である。
- RustのMeteora-DLMM quote結果はSDK参照fixtureのquote結果と、raw amount単位または明示した許容誤差以内で一致する。
- 既存の`f64`ベース実装が許容誤差を満たせない場合、`Decimal`または高精度計算へ置き換える作業を実施対象にする。
- quote照合は、Meteora公式SDKまたは公式Rust integration相当のBinArray取得順、swap方向、partial fill設定、slippage bps設定を固定して行う。

### マイルストーン10: 裁定候補判定（実装済み・実行検証待ち）

プール状態を更新し、同一ペア・同一方向の粗い価格差を検出した後、3 DEXすべてのlocal AMM mathで実行可能なquoteを計算する。公式SDKは照合・テスト用途だけに使う。投入量はraw amountの整数単位で、粗い探索と局所的な再探索を組み合わせてnet profitが最大となる値を求める。

完了条件:

- Raydium、Orca Whirlpool、Meteora-DLMMの各方向についてlocal quoteを計算できる。
- `USDC -> SOL -> USDC`の2レッグ経路を評価し、逆方向も評価できる。
- DEX fee、Solana base fee、priority fee、Jito tip、Flashloan fee、slippage bufferを固定設定値として差し引ける。
- expected profitをUSDC raw amountで比較し、`expectedProfit > minProfitThreshold`だけを候補にできる。
- snapshot slotの整合性を検査し、不整合な状態から候補を作らない。
- 候補評価、quote失敗、利益、投入量、差し引いたコストをSQLiteへ保存できる。送信直前再quote結果の保存はマイルストーン11に分離する。
- 候補通知は状態変化時に限定し、同一候補を毎サイクル重複通知しない。

実装内容と対応範囲:

- `src/arbitrage/`に設定、raw amount整数math、DEX別local quote、snapshot取得、経路探索を実装した。中間積は`num-bigint`を使い、出力量は切り捨て、入力側feeと必要入力量は切り上げる。
- Raydiumは既存デコーダに合わせてAMM v4（752 bytes）の稼働済みpoolを対象とし、現行公式`swap_base_in_v2`のvault準備金から`need_take_pnl`を控除する方式で両方向quoteを計算する。Raydium CLMM/CPMMは対象外。
- Orcaは通常feeのWhirlpoolと連続したfixed TickArrayを対象に、Q64.64整数math、tick越境、`liquidity_net`の増減を計算する。adaptive fee、dynamic TickArray、Token-2022拡張は未対応としてquote失敗にする。
- Meteora-DLMMは稼働済みpermissionless pool、標準input fee、連続BinArrayを対象に、bin価格・在庫による両方向quote、Clock timestampによるvolatility reference減衰、bin越境によるvariable fee更新を計算する。permission/activation制御、limit-order liquidity、別fee modeはquote失敗として除外する。
- `[arbitrage]`は省略可能で、既存設定は従来の価格監視を継続する。有効化には`config.arbitrage.example.toml`のセクションを既存設定へ追加し、3 poolのprogram ownerとtick/bin配列を明示する。配列は各pool最大16個で、現在tick/binを含む連続範囲が必要。範囲外へ価格が動いた場合は設定更新が必要。
- pool本体の初回取得は依存アカウント発見専用。quoteにはpool本体、mint、vault、配列、Clockを同じ`getMultipleAccounts`で取り直したsnapshotだけを使う。全アカウントの最小・最大slotを検査し、許容差の初期値は2。取得失敗・slot不整合では全取得を1回だけ再試行し、失敗結果も保存する。
- 粗い限界価格比較は各DEXペアの逆方向を含む全6経路で行う。入力上下限、step、coarse点数、最大2レッグquote評価回数、レッグごとの実効価格悪化上限を設定する。coarse探索、tick/bin境界と前後1 rawの確認、第2レッグ境界のUSDC入力への二分逆引き、良い区間の再探索と広い区間の探索を組み合わせる。部分約定・ゼロ出力・impact上限超過は採用しない。有限の探索予算内の最良値であり、大域最適性は保証しない。
- `base_fee_usdc`、`priority_fee_usdc`、`jito_tip_usdc`、`flashloan_fee_usdc`、`slippage_buffer_usdc`は明示した固定USDC額を差し引く。DEX feeはsnapshot内のオンチェーン率を各local quoteへ反映し、レッグ別入力tokenのraw feeとして保存する。DEX feeを固定費として再度差し引く二重控除は行わない。
- USDC設定は6桁を超える小数を拒否し、利益は符号付き整数`i128`のraw amountで比較する。`expected_profit_raw > min_profit_raw`の厳密比較だけで`detected`とする。
- `arbitrage_evaluations`に全6経路の`detected`、`unprofitable`、`no_spread`、`quote_failed`を毎回保存する。全探索点の両レッグquote、partial fill、失敗理由、固定費内訳は`details_json`へ保存する。raw値の主要列はTEXTとし、既存DBのデータは削除しない。
- `arbitrage_notification_state`に最後に送信成功した状態と利益を保存する。新規候補、最後の通知値から設定額以上の利益変化、候補消失・復活だけを通知し、再起動後も状態を引き継ぐ。Webhook失敗時は通知状態を更新せず、次サイクルで再試行する。
- 裁定モード有効時は通常監視のMeteora SDK補助quoteを呼ばず、裁定判定をlocal mathだけで行う。裁定モード無効時の既存価格監視とSDK補助quoteは変更しない。
- マイルストーン11の再quote、transaction作成、simulation、署名、送信は実装していない。ここでのsnapshot再取得は整合性回復の再試行に限る。

検証状況:

- raw丸め、Q64.64 tick値、両方向quote、mint方向反転、tick/bin越境、partial fill、固定費、利益閾値との等号、探索予算、slot差、状態通知、SQLite保存のオフラインテストを追加した。
- 3 DEXのバイナリsnapshotからのデコード接続、Raydium PnL控除、owner不一致、設定不備、slot不整合時の全取得再試行を検証するテストも追加した。候補専用RPCは1リクエスト10秒でtimeoutにする。
- 保存済み`tests/fixtures/local/arbitrage_quotes.json`があれば、3 DEX・両方向の公式参照quoteと出力、fee、消費入力、完全約定有無を整数単位で比較する。fixtureには`src/arbitrage/tests.rs`の`Reference`形式（SDK名・version、timestamp、pool設定、quoteアカウント設定、owner/slot/base64付き全アカウント、期待quote）を使用する。fixture未生成時は明示的なSKIPPEDメッセージを出し、公式照合済みとは扱わない。
- `do-plan`スキルの制約によりテスト・ビルド・Bot起動・SDK実行は行っていない。追加テストと実fixture照合は実行検証待ちであり、運用前に対応pool形式とraw quoteの照合が必要。

### 将来マイルストーン11: 送信直前再quote（今回対象外）

transaction作成前に最新のpool状態を再取得し、同一のlocal quote mathで再quoteする。再quote後もminProfitThresholdを超える場合だけ、後続のtransaction作成へ進む。

完了条件:

- 再quote専用の最新snapshotを取得できる。
- `requoted_profitable`、`requoted_unprofitable`、`quote_failed`、`expired`を区別して保存できる。
- 再quote不採算またはquote失敗時にtransactionを作成・送信しない。

### 将来マイルストーン12: 専用小額ウォレットと自分の資金による実行（今回対象外）

既存資産と分離した専用小額ウォレットを用意し、Flashloanを使わずに自分のUSDC/SOLでtransactionをsimulation、送信、confirmation確認する。秘密鍵を`config.toml`やSQLiteへ保存しない。

### 将来マイルストーン13: Flashloan統合（今回対象外）

USDC対応、流動性、flashloan fee、borrow/repay instruction、同一transaction内での原子性、mainnet稼働状況を確認したうえで提供元を選定し、`borrow -> 2 legs -> repay`を実装する。

### 将来マイルストーン14: Jito送信（今回対象外）

通常RPC送信と分離したJito送信経路を追加する。priority feeとJito tipを別々に見積もり、送信結果と実際のlanding状況を保存する。transaction本体とtipの扱いも、失敗時に不要なtipを支払わない構成を検証する。

### 将来マイルストーン15: Jito Bundle拡張（今回対象外）

Jito Bundleが必要なケースを確認した後、Bundle送信、Bundle status取得、失敗・期限切れ・landing確認を追加する。単一transactionで裁定処理全体を原子的に実行できる場合は、Bundle導入を必須としない。

## テスト計画

単体テストでは次を検証する。

- 設定ファイルの読み込みとバリデーション
- Raydiumプールデコード
- Orcaプールデコード
- Orca Whirlpool token A/B mint accountからのdecimal取得
- Orca Whirlpool sqrt_priceのdecimal補正込み価格計算
- Orca Whirlpool token A/Bがbase/quoteと逆向きの場合の価格反転
- Meteora-DLMM LbPairデコード
- Meteora-DLMM token X/Y mint accountからのdecimal取得
- Meteora-DLMM BinArray取得結果の解釈
- Meteora-DLMM active bin価格計算
- Meteora-DLMM active bin価格計算とSDK参照fixtureの照合
- Meteora-DLMM quote計算
- Meteora-DLMM quote計算とSDK参照fixtureの照合
- Meteora-DLMMの`USDC -> SOL` quote
- Meteora-DLMMの`SOL -> USDC` quote
- Meteora-DLMMのquote失敗時にactive bin価格監視を継続する分岐
- 価格計算
- 手数料考慮後価格の計算
- スリッページ計算の有効・無効切り替え
- 価格差率計算
- Discord Embed通知メッセージ生成
- Discord Embedの通常通知・異常通知テンプレート選択
- Discord Embed表示専用formatterで、DEX価格、価格差USDC、bpsを指定桁数に丸めて末尾ゼロ付きで出力できること
- Discord Embedのfield数が最大12個に収まり、超過分が`Summary` fieldへまとめられること
- エラー通知Embedで長いエラーメッセージ、source、poolアドレスが表示用に短縮されること
- Discord Embedの短縮・丸めがSQLite保存値へ影響しないこと
- SQLite保存処理
- fixture JSONからRPCレスポンス相当のaccount dataを復元したDEXデコード
- 異常fixtureでのデコードエラー

結合テストでは次を検証する。

- モックRPCレスポンスを使った監視サイクルの実行
- Raydium、Orca、Meteora-DLMMのいずれか1つが取得失敗した時の挙動
- SQLite保存失敗時の挙動
- Discord通知失敗時の挙動
- Discord Embedペイロード生成とWebhook送信
- Discord Embedペイロードが通常通知・異常通知ともにfield数上限と文字数短縮ルールを満たすこと
- 連続エラー時の異常通知
- 通常の結合テストがネットワークを使わず、fixtureまたはモックRPCレスポンスで実行できること
- 公式SDKまたは既存crateとの照合テストを通常のオフラインテストと分離できること
- Meteora-DLMM active binおよびquote SDK参照fixtureの再生成は明示実行に分離されていること

手動確認では次を検証する。

- ローカル環境でBotを起動できること
- 30秒ごとにRaydium、Orca、Meteora-DLMMの価格を取得すること
- 各サイクルでDiscord通知が送信されること
- Discord通知がEmbed形式で表示されること
- SQLiteに価格観測結果と価格差が保存されること
- SQLiteにMeteora-DLMM固有状態が保存されること
- SQLiteにMeteora-DLMMの両方向quote詳細が保存されること
- RPC障害や不正設定時に異常通知されること
- 開発用fixture取得スクリプトを手動実行し、Helius RPCから対象poolと依存アカウントのJSON fixtureを生成できること
- Meteora-DLMM active binおよびquote SDK参照fixture生成スクリプトを手動実行し、複数poolの期待価格とquote JSONを生成できること
- 生成済みfixtureを使ったオフラインテストで、Raydium、Orca、Meteora-DLMMのデコード結果が再現できること

fixture検証の受け入れ基準は次の通り。

- 通常の`cargo test`はネットワークなしで成功する。
- 保存済みfixtureを読み込むテストはRPCへ接続しない。
- 出力価格が`USDC per SOL`で正の値である。
- 外部照合値または公式SDK出力との乖離が設定した許容bps以内である。
- Meteora-DLMM active bin価格式は、SDK参照fixtureの正規化済み期待価格と`0.01 bps`以内で一致する。
- Meteora-DLMM quoteは、SDK参照fixtureの両方向quote結果とraw amount単位または明示した許容誤差以内で一致する。
- Orca Whirlpool、Meteora-DLMMのdecimal補正と価格方向の反転が期待通りである。
- Raydium、Orca Whirlpool、Meteora-DLMMそれぞれのmint、vault、LbPair、token X/Y mintがfixture期待値と一致する。
- Orca Whirlpoolの価格式は公式SDKまたは公式実装の`sqrt_price_to_price`相当の結果と、設定した許容bps以内で一致する。
- Meteora-DLMMのbase fee、variable fee、total feeが公式SDK互換式の結果と一致する。
- 公式SDKまたは既存crateとの照合を実行できない場合は、通常テストの失敗ではなく警告として扱い、照合未実行であることをテスト出力または手動確認手順に明記する。

## 受け入れ条件

初期実装は次を満たした時点で完了とする。

- Botがローカル環境で起動できる。
- `.env`と`config.toml`から必要設定を読み込める。
- Helius HTTP RPCからRaydium、Orca、Meteora-DLMMのSOL/USDC監視に必要なアカウント情報を取得できる。
- 各DEXの価格をオンチェーンプール状態から算出できる。
- 価格比較基準が全DEXで`USDC per SOL`に統一されている。
- Orca Whirlpool価格はmint accountから読んだtoken A/B decimalで補正されている。
- Raydium、Orca、Meteora-DLMMの全組み合わせ価格差を計算できる。
- 各監視サイクルでDiscord Embedの価格差通知を送信できる。
- 異常発生時にエラー通知Embedを送信できる。
- SQLiteへ価格観測結果、価格差、Meteora-DLMM固有状態、エラーを保存できる。
- SQLiteへMeteora-DLMMの両方向quote詳細、使用BinArray、partial fill、quote失敗理由を保存できる。
- 通常の`cargo test`がネットワークなしで成功する。
- fixture取得補助を手動実行でき、fixture本体はGit管理されない。
- Meteora-DLMM active bin価格式が、複数poolのSDK参照fixtureに対して原則`0.01 bps`以内で一致する。
- Meteora-DLMM quote計算が、複数poolのSDK参照fixtureに対してraw amount単位または明示した許容誤差以内で一致する。

## 実装メモ

- 2026-09-09: 追加依頼に従いマイルストーン10の実装を追加した。詳細は同マイルストーンの対応範囲・検証状況を参照。マイルストーン11以降は未着手。既存のSPEC.md編集は変更していない。

- 2026-09-09: 裁定候補判定と自動売買拡張の方針を追加した。今回の実装では、専用小額ウォレット、自分の資金による実行、通常RPCでのsimulation・送信、Flashloan、Jito送信、Jito Bundleを実装せず、将来マイルストーンとして記録する。将来の裁定quoteは3 DEXのlocal AMM mathを正とし、公式SDKは照合・テスト用途に限定する。利益単位はUSDC raw amount、投入量探索は粗い探索と局所再探索の組み合わせ、再quote・slot整合性・状態変化通知を採用する方針とした。
- 2026-09-03: `src/lib.rs`を追加し、通常監視Botと手動fixture取得補助が同じ設定、RPC、DEXデコード実装を使えるようにした。
- 2026-09-03: `src/bin/fetch_fixtures.rs`を追加した。`config.toml`の有効poolを読み、pool本体、Raydium/Orcaのvault、Meteora-DLMMのtoken X/Y mint accountをHelius HTTP RPCの`getMultipleAccounts`で取得し、`tests/fixtures/local/`へRPCレスポンスに近いJSONとして保存する。
- 2026-09-03: `price_observations`に`lb_pair_address`と`slippage_adjusted_price`を保存できるようにし、既存DB向けには不足列を追加するだけの移行にした。全組み合わせ価格差は既存互換のため`price_spread_pairs`へ保存し、従来の`price_spreads`はRaydium vs Orca互換用として残す。
- 2026-09-03: スリッページは、`trade_size_usdc`とDEXデコーダが返した`liquidity`がある場合に参考買値として`slippage_adjusted_price`へ保存する。Meteora-DLMMのBinArray取得と公式SDK互換quote照合は未完了のため、関連進捗は未完了のままとする。
- 2026-09-03: `config.example.toml`と`.env.example`を追加した。具体的なmainnet poolアドレスは固定せず`未定`のままにしている。
- 2026-09-03: `do-plan`スキルの制約により、今回の作業ではimport、テスト、ビルド、プログラム実行による検証は行っていない。検証はファイル再読込、検索、差分確認による静的確認に限定した。
- 2026-09-03: OrcaはWhirlpoolのみを対象とし、旧Orca Constant Product AMM/CPMMは初期実装の対象外とする。Whirlpool価格は`sqrt_price`だけでなく、token A/B mint accountから読んだdecimalで補正する計画に更新した。
- 2026-09-03: Orca Whirlpoolの価格デコードを更新し、Whirlpool本体から読んだtoken A/B mintのmint accountを追加取得してdecimal補正するようにした。価格計算は`f64`ではなく`Decimal`で`(sqrt_price / 2^64)^2 * 10^(decimals_a - decimals_b)`を計算し、token A/Bがbase/quoteと逆向きの場合は反転する。`do-plan`スキルの制約により、今回もimport、テスト、ビルド、プログラム実行による検証は行っていない。
- 2026-09-03: Meteora-DLMM active bin価格式は、Meteora公式TypeScript SDKを明示実行して生成した複数poolのSDK参照fixtureで照合する方針にした。通常の`cargo test`は保存済みfixtureのみを読み、SDK、Node、ネットワークを要求しない。許容誤差は原則`0.01 bps`以内とし、既存の`f64`ベース実装で満たせない場合は`Decimal`または高精度計算へ置き換える。
- 2026-09-03: `tools/meteora-dlmm-sdk-fixture/`にMeteora公式TypeScript SDKのactive bin参照fixture生成ヘルパーを追加し、`src/dex/meteora/meteora_amm.rs`に生成済みfixtureが存在する場合だけ`0.01 bps`以内で照合するオフラインテストを追加した。実fixture生成、SDK install、`cargo test`は`do-plan`スキルの制約により実行していないため、進捗チェックは未完了のままとする。
- 2026-09-03: 次のMeteora-DLMM実装方針を確定した。quote方向は`USDC -> SOL`と`SOL -> USDC`の両方向、想定取引サイズは既存の`pricing.trade_size_usdc`を流用、quoteロジックはMeteora公式SDKまたは公式Rust integration相当に合わせる。BinArrayは両方向それぞれ公式helper相当で最大`count = 4`を初期値として取得し、設定で変更可能にする。quoteのみ失敗した場合はactive bin価格による監視と価格差計算を継続し、詳細はSQLiteへ保存する。SDK参照fixtureはactive bin価格だけでなく両方向quote照合にも広げる。
- 2026-09-03: `pricing.meteora_dlmm_bin_array_count`と`pricing.meteora_dlmm_slippage_bps`を追加し、Meteora-DLMMの両方向quoteを公式TypeScript SDKヘルパーで取得する実装を追加した。Rust側はSDK helperのJSONを`MeteoraDlmmQuote`へ変換し、quote成功時は`USDC -> SOL`の実効価格を`slippage_adjusted_price`へ反映する。quote失敗時はMeteora価格取得を失敗扱いにせず、`meteora_dlmm_quotes`と`monitor_errors`へ記録する。`fetch_fixtures`は公式SDK helperが返したBinArrayアドレスをHelius fixture取得対象へ追加する。do-planスキルの制約により、import、テスト、ビルド、プログラム実行による検証は行っていない。
- 2026-09-03: Discord Embedの短縮・整形方針を確定した。Discord表示専用formatterでDEX価格は小数4桁、価格差USDCは小数6桁、bpsは小数2桁固定に丸め、末尾ゼロを残す。通常通知と異常通知の`fields`は最大12個を初期上限とし、超過分は`Summary` fieldへまとめる。エラー通知では`description`を最大500文字、`Source` fieldを最大300文字、Poolアドレスを`先頭8文字...末尾8文字`へ短縮する。これらはコード上の定数とし、SQLite保存値には適用しない。
- 2026-09-03: `src/notifier.rs`にDiscord表示専用formatter、field数上限処理、長文短縮、Poolアドレス短縮を追加した。通常通知ではDEX価格、価格差USDC、bps、手数料考慮後参考差分を表示専用に丸める。異常通知ではエラーメッセージ、source、Poolアドレスを表示用に短縮する。payload生成の単体テストを追加したが、do-planスキルの制約によりimport、テスト、ビルド、プログラム実行による検証は行っていない。

## 根拠メモ

- マイルストーン10: Raydium公式AMMの`process_swap_base_in_v2`の準備金補正・fee丸めを参照した。https://github.com/raydium-io/raydium-amm/blob/master/program/src/processor.rs
- マイルストーン10: Orca公式coreのtick整数定数とWhirlpool/fixed TickArrayのレイアウトを参照した。https://github.com/orca-so/whirlpools/blob/main/rust-sdk/core/src/math/tick.rs / https://github.com/orca-so/whirlpools/blob/main/programs/whirlpool/src/state/fixed_tick_array.rs
- マイルストーン10: Meteora公式IDLのLbPair/BinArray/Bin/StaticParameters/VariableParametersと、fee・bin quote・volatility更新を参照した。https://github.com/MeteoraAg/dlmm-sdk/blob/main/ts-client/src/dlmm/idl/idl.json / https://github.com/MeteoraAg/dlmm-sdk/blob/main/ts-client/src/dlmm/helpers/fee.ts / https://github.com/MeteoraAg/dlmm-sdk/blob/main/ts-client/src/dlmm/helpers/bin.ts / https://github.com/MeteoraAg/dlmm-sdk/blob/main/ts-client/src/dlmm/index.ts

- Orca公式ドキュメントでは、現在のOrcaはconcentrated liquidity poolを中心に構成され、Whirlpoolは2022年に導入されたconcentrated liquidity programと説明されている。根拠: https://docs.orca.so/support/about
- Orca公式ドキュメントのPrice & Ticksでは、Whirlpoolがsquare-root priceで価格を追跡し、Whirlpool accountがcurrent sqrt-priceとcurrent tick-indexを保持すると説明されている。根拠: https://docs.orca.so/developers/architecture/price-and-ticks
- Orca公式SDK概要では、Rust向けに`orca_whirlpools`、価格変換やquoteなどの計算用に`orca_whirlpools_core`が提供されている。根拠: https://docs.orca.so/developers/sdks/overview
- Orca公式Whirlpool Parametersでは、Whirlpool program ID、config address、fee tierが公開されている。根拠: https://docs.orca.so/developers/architecture/whirlpool-parameters
- Orca公式実装または公式SDKの価格変換関数に合わせ、Whirlpoolの`sqrt_price`、token A decimal、token B decimalを入力にした価格変換を照合対象とする。根拠: https://github.com/orca-so/whirlpools
- Meteora公式TypeScript SDKには、active bin取得の`getActiveBin()`、price per lamportからUI価格へ変換する`fromPricePerLamport()`、bin IDから価格を得る`getPriceOfBinByBinId`が用意されている。Meteora-DLMM active bin価格式の参照値は、これらのSDK出力をfixture化して照合する。根拠: https://github.com/MeteoraAg/docs/blob/main/developer-guides/dlmm/typescript-sdk/reference.mdx
- Meteora公式TypeScript SDKでは、swap前に`getBinArrayForSwap(swapForY, count)`でswap方向のBinArrayを取得し、`swapQuote(inAmount, swapForY, allowedSlippage, binArrays, ...)`でquoteを計算する。`swapForY = true`はtoken Xからtoken Yへのswapを表す。根拠: https://github.com/MeteoraAg/docs/blob/main/developer-guides/dlmm/typescript-sdk/reference.mdx
- Meteora公式Rust integrationでは、`get_bin_array_pubkeys_for_swap`がLbPair、必要に応じたbitmap extension、swap方向、countからswap用BinArray pubkeyを解決する。`swap_for_y = true`はtoken Xからtoken Y、falseはtoken Yからtoken Xを表す。根拠: https://github.com/MeteoraAg/docs/blob/main/developer-guides/dlmm/rust-integration/library.mdx

## 未決事項

未決事項は、実装中に固定値として決めない。必要な場合は`config.example.toml`の未定値または明示的なTODOとして扱う。

- `config.example.toml`に記載するRaydium SOL/USDCの具体的なプールアドレス
- `config.example.toml`に記載するOrca SOL/USDCの具体的なプールアドレス
- `config.example.toml`に記載するMeteora-DLMM SOL/USDCの具体的なLbPairアドレス
- 裁定判定しきい値
- 想定取引サイズ
- 裁定候補判定で使う最小利益しきい値の具体値
- 投入量探索の上下限、探索精度、最大quote回数
- 裁定local quoteの未対応形式（Whirlpool adaptive fee/dynamic TickArray、Meteoraの別fee mode/limit order/activation制御）を拡張する時期
- 固定コストとして使うbase fee、priority fee、Jito tip、Flashloan fee、slippage bufferの具体値
- 専用小額ウォレットの署名方式
- Flashloan提供元と対応mint、fee、流動性、borrow/repay仕様
- 通常RPCとJitoのどちらを標準送信経路にするか
- Jito Bundleを導入する条件
- Raydium AMM v4以外のプール種別を追加する時期
- Meteora-DLMM active bin SDK参照fixtureに含める具体的な複数pool
- Meteora-DLMMの自動探索方法
- ローカル実行時の起動方法

## 冪等性と復旧

SQLiteスキーマ初期化は既存データを消さない方法で行う。fixtureファイルは`tests/fixtures/local/`などのローカル生成ディレクトリに保存し、Git管理しない。

設定不備は起動時に検出してBotを停止する。一時的なRPC失敗はリトライ対象とし、同一コンポーネントで連続エラーが発生した場合はDiscordへ異常通知する。Discord通知失敗時はSQLiteへエラーを記録し、連続失敗時は標準出力または標準エラーにも出力する。

Meteora-DLMMのactive bin価格式とquote計算は、Meteora公式TypeScript SDKを明示実行して生成したSDK参照fixtureと通常のオフラインテストで照合する。Meteora-DLMMのアカウントレイアウト、PDA導出、quote計算を自前実装する場合も、公式SDKまたは公式Rust integration相当の挙動と照合できる形でテストする。Meteora-DLMM active bin価格式、quote計算、Orca Whirlpoolの価格式・手数料式は、公式SDKまたは公式実装に準じる既存crateとの照合を初期実装の受け入れ条件とする。
