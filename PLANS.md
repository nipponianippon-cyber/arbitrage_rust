# Raydium・Orca・Meteora-DLMM対応のSolana DEX価格監視Bot実装計画

このPLANS.mdは`SPEC.md`、リポジトリ内の既存実装、実装中に確認した公式ドキュメント、公式SDKまたは公式実装、保存済みfixture、手動確認結果を根拠に更新する。根拠にした外部仕様や実装上の判断は、必要に応じて「根拠メモ」または「実装メモ」に記録する。

## 目的 / 全体像

ローカル環境で動作するRust製Botを実装する。BotはHeliusのSolana HTTP RPCを使い、設定ファイルで明示されたRaydium、Orca、Meteora-DLMMのSOL/USDCプール状態を30秒ごとに取得する。各DEXのオンチェーンプール状態から価格をBot内部で算出し、全DEXで比較基準を`USDC per SOL`に統一する。

各監視サイクルでは、Raydium、Orca、Meteora-DLMMの全組み合わせについて価格差、価格差率、高いDEX、安いDEX、手数料考慮後の参考差分を計算し、Discord WebhookのEmbedで通知する。同じ監視結果、価格差、Meteora-DLMM固有状態、エラー情報はSQLiteに保存する。

初期実装は価格監視専用とする。自動売買、トランザクション作成、秘密鍵またはウォレット管理、署名、トランザクション送信、Jupiterなどのオフチェーン集約APIによる価格取得、Helius WebSocket、Enhanced API、Webhookは実装しない。

## 進捗

- [x] `SPEC.md`を根拠に、初期実装の対象と非対象範囲を整理した。
- [x] `SPEC.md`を根拠に、PLANS.mdをMeteora-DLMM、全組み合わせ価格差、Discord Embed、SQLiteログ、fixture取得補助を含む計画へ更新した。
- [x] 設定読み込みと設定バリデーションを実装する。
- [x] Helius HTTP RPCクライアントを実装する。
- [x] DEX共通インターフェースとRaydium価格デコードを実装する。
- [x] Orca価格デコードを実装する。
- [x] Orca Whirlpool価格のmint decimal補正を実装する。
- [ ] Meteora-DLMMのLbPair、mint account、必要に応じたBinArray取得と価格デコードを実装する。
- [ ] 手数料、スリッページ、価格差計算を実装する。
- [x] SQLite保存を実装する。
- [x] Discord Embed通知を実装する。
- [x] 30秒周期の監視ループを実装する。
- [x] 開発・テスト用fixture取得補助を実装する。
- [ ] 単体テスト、結合テスト、手動確認を実施する。

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

スリッページ計算を行う場合は、active bin周辺のBinArrayも取得し、公式SDKまたは既存crateのquoteロジックを利用して想定取引サイズに対する価格インパクトを算出する。

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
- Meteora-DLMMではactive bin価格を基準価格とし、手数料とスリッページは公式SDKまたは既存crateのquote結果に基づいて算出する。
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

異常発生時は通常の価格差通知とは別に、エラー通知Embedを送信する。エラー通知Embedには少なくとも次を含める。

- 発生時刻
- コンポーネント名
- 重要度
- エラーメッセージ
- 対象DEX
- 対象プールアドレス
- リトライ予定の有無
- 連続エラー回数

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
- スリッページ検証を行う場合: active bin周辺のBinArray

fixtureファイルはGit管理しない。保存先は`tests/fixtures/local/`などのローカル生成ディレクトリを想定し、`.gitignore`で除外する。fixtureの更新は手動で行う。自動更新、定期更新、CI上でのmainnet RPC取得は初期実装に含めない。

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
- 裁定判定しきい値に依存せず、毎サイクル通知対象を作れる。

### マイルストーン5: SQLite保存

監視結果、価格差、Meteora-DLMM固有状態、エラーをSQLiteへ保存する。

完了条件:

- `price_observations`へDEXごとの価格取得結果を保存できる。
- `price_spreads`へ全組み合わせ価格差を保存できる。
- `meteora_dlmm_states`へ`active_id`、`bin_step`、手数料、status、liquidityを保存できる。
- `monitor_errors`へRPC、デコード、価格計算、DB保存、Discord通知のエラーを保存できる。

### マイルストーン6: Discord Embed通知

通常通知Embedと異常通知Embedを実装する。通常通知には3 DEX価格と全組み合わせ価格差を含め、Meteora-DLMM固有詳細は含めない。

完了条件:

- 各監視サイクルで価格差通知Embedを送信できる。
- 異常発生時にエラー通知Embedを送信できる。
- Embedに監視時刻、対象ペア、DEX別価格、価格差、価格差率、高いDEX、安いDEX、比較方向、slot、手数料考慮後の参考差分、エラー有無を含められる。
- Discord通知失敗時にSQLiteへエラー保存し、連続失敗時は標準出力または標準エラーへ出力できる。

### マイルストーン7: 監視ループ

30秒周期の監視ループを実装し、設定読み込み、RPC取得、DEXデコード、価格差計算、SQLite保存、Discord通知、エラー処理を結合する。

完了条件:

- 30秒ごとにRaydium、Orca、Meteora-DLMMの価格を取得する。
- 3 DEXすべての価格取得に成功した場合だけ価格差を計算する。
- いずれか1 DEXでも価格取得に失敗した場合、そのサイクル全体を失敗扱いにして価格差計算をスキップする。
- エラーをSQLiteへ保存し、必要に応じてDiscordへ異常通知する。

### マイルストーン8: fixture取得とテスト

開発・テスト用fixture取得補助と、fixtureまたはモックRPCレスポンスを使ったテストを実装する。

完了条件:

- Helius HTTP RPCから対象poolと依存アカウントのJSON fixtureを手動生成できる。
- 通常の`cargo test`はネットワークなしで成功する。
- 保存済みfixtureを使い、RPCへ接続せずにDEXデコードを検証できる。
- Helius RPCを実際に叩くテストは通常の`cargo test`から分離し、明示指定時だけ実行される。

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
- Meteora-DLMM quote計算
- 価格計算
- 手数料考慮後価格の計算
- スリッページ計算の有効・無効切り替え
- 価格差率計算
- Discord Embed通知メッセージ生成
- Discord Embedの通常通知・異常通知テンプレート選択
- SQLite保存処理
- fixture JSONからRPCレスポンス相当のaccount dataを復元したDEXデコード
- 異常fixtureでのデコードエラー

結合テストでは次を検証する。

- モックRPCレスポンスを使った監視サイクルの実行
- Raydium、Orca、Meteora-DLMMのいずれか1つが取得失敗した時の挙動
- SQLite保存失敗時の挙動
- Discord通知失敗時の挙動
- Discord Embedペイロード生成とWebhook送信
- 連続エラー時の異常通知
- 通常の結合テストがネットワークを使わず、fixtureまたはモックRPCレスポンスで実行できること
- 公式SDKまたは既存crateとの照合テストを通常のオフラインテストと分離できること

手動確認では次を検証する。

- ローカル環境でBotを起動できること
- 30秒ごとにRaydium、Orca、Meteora-DLMMの価格を取得すること
- 各サイクルでDiscord通知が送信されること
- Discord通知がEmbed形式で表示されること
- SQLiteに価格観測結果と価格差が保存されること
- SQLiteにMeteora-DLMM固有状態が保存されること
- RPC障害や不正設定時に異常通知されること
- 開発用fixture取得スクリプトを手動実行し、Helius RPCから対象poolと依存アカウントのJSON fixtureを生成できること
- 生成済みfixtureを使ったオフラインテストで、Raydium、Orca、Meteora-DLMMのデコード結果が再現できること

fixture検証の受け入れ基準は次の通り。

- 通常の`cargo test`はネットワークなしで成功する。
- 保存済みfixtureを読み込むテストはRPCへ接続しない。
- 出力価格が`USDC per SOL`で正の値である。
- 外部照合値または公式SDK出力との乖離が設定した許容bps以内である。
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
- 通常の`cargo test`がネットワークなしで成功する。
- fixture取得補助を手動実行でき、fixture本体はGit管理されない。

## 実装メモ

- 2026-09-03: `src/lib.rs`を追加し、通常監視Botと手動fixture取得補助が同じ設定、RPC、DEXデコード実装を使えるようにした。
- 2026-09-03: `src/bin/fetch_fixtures.rs`を追加した。`config.toml`の有効poolを読み、pool本体、Raydium/Orcaのvault、Meteora-DLMMのtoken X/Y mint accountをHelius HTTP RPCの`getMultipleAccounts`で取得し、`tests/fixtures/local/`へRPCレスポンスに近いJSONとして保存する。
- 2026-09-03: `price_observations`に`lb_pair_address`と`slippage_adjusted_price`を保存できるようにし、既存DB向けには不足列を追加するだけの移行にした。全組み合わせ価格差は既存互換のため`price_spread_pairs`へ保存し、従来の`price_spreads`はRaydium vs Orca互換用として残す。
- 2026-09-03: スリッページは、`trade_size_usdc`とDEXデコーダが返した`liquidity`がある場合に参考買値として`slippage_adjusted_price`へ保存する。Meteora-DLMMのBinArray取得と公式SDK互換quote照合は未完了のため、関連進捗は未完了のままとする。
- 2026-09-03: `config.example.toml`と`.env.example`を追加した。具体的なmainnet poolアドレスは固定せず`未定`のままにしている。
- 2026-09-03: `do-plan`スキルの制約により、今回の作業ではimport、テスト、ビルド、プログラム実行による検証は行っていない。検証はファイル再読込、検索、差分確認による静的確認に限定した。
- 2026-09-03: OrcaはWhirlpoolのみを対象とし、旧Orca Constant Product AMM/CPMMは初期実装の対象外とする。Whirlpool価格は`sqrt_price`だけでなく、token A/B mint accountから読んだdecimalで補正する計画に更新した。
- 2026-09-03: Orca Whirlpoolの価格デコードを更新し、Whirlpool本体から読んだtoken A/B mintのmint accountを追加取得してdecimal補正するようにした。価格計算は`f64`ではなく`Decimal`で`(sqrt_price / 2^64)^2 * 10^(decimals_a - decimals_b)`を計算し、token A/Bがbase/quoteと逆向きの場合は反転する。`do-plan`スキルの制約により、今回もimport、テスト、ビルド、プログラム実行による検証は行っていない。

## 根拠メモ

- Orca公式ドキュメントでは、現在のOrcaはconcentrated liquidity poolを中心に構成され、Whirlpoolは2022年に導入されたconcentrated liquidity programと説明されている。根拠: https://docs.orca.so/support/about
- Orca公式ドキュメントのPrice & Ticksでは、Whirlpoolがsquare-root priceで価格を追跡し、Whirlpool accountがcurrent sqrt-priceとcurrent tick-indexを保持すると説明されている。根拠: https://docs.orca.so/developers/architecture/price-and-ticks
- Orca公式SDK概要では、Rust向けに`orca_whirlpools`、価格変換やquoteなどの計算用に`orca_whirlpools_core`が提供されている。根拠: https://docs.orca.so/developers/sdks/overview
- Orca公式Whirlpool Parametersでは、Whirlpool program ID、config address、fee tierが公開されている。根拠: https://docs.orca.so/developers/architecture/whirlpool-parameters
- Orca公式実装または公式SDKの価格変換関数に合わせ、Whirlpoolの`sqrt_price`、token A decimal、token B decimalを入力にした価格変換を照合対象とする。根拠: https://github.com/orca-so/whirlpools

## 未決事項

未決事項は、実装中に固定値として決めない。必要な場合は`config.example.toml`の未定値または明示的なTODOとして扱う。

- `config.example.toml`に記載するRaydium SOL/USDCの具体的なプールアドレス
- `config.example.toml`に記載するOrca SOL/USDCの具体的なプールアドレス
- `config.example.toml`に記載するMeteora-DLMM SOL/USDCの具体的なLbPairアドレス
- 裁定判定しきい値
- 想定取引サイズ
- スリッページ計算に使う想定取引サイズ
- Raydiumで対象とするプール種別
- Meteora-DLMMの自動探索方法
- SQLiteスキーマの詳細
- Discord Embed通知の最終デザイン
- ローカル実行時の起動方法

## 冪等性と復旧

SQLiteスキーマ初期化は既存データを消さない方法で行う。fixtureファイルは`tests/fixtures/local/`などのローカル生成ディレクトリに保存し、Git管理しない。

設定不備は起動時に検出してBotを停止する。一時的なRPC失敗はリトライ対象とし、同一コンポーネントで連続エラーが発生した場合はDiscordへ異常通知する。Discord通知失敗時はSQLiteへエラーを記録し、連続失敗時は標準出力または標準エラーにも出力する。

Meteora-DLMMのアカウントレイアウト、PDA導出、quote計算を自前実装する場合は、公式SDKまたは既存crateの挙動と照合できる形でテストする。Meteora-DLMMおよびOrca Whirlpoolの価格式・手数料式は、公式SDKまたは公式実装に準じる既存crateとの照合を初期実装の受け入れ条件とする。
