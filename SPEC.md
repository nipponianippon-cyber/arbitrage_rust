# SPEC

## 1. 目的

BotはHeliusのSolana RPCノードを利用し、Solana上の複数DEX間におけるトークン価格を定期監視する。

初期実装ではRaydium、Orca、Meteora-DLMMのSOL/USDCプールを対象に、オンチェーンのプール状態を直接取得して価格を算出し、DEX間の価格差をDiscordへ通知する。

将来的には、この価格監視基盤を自動売買Botへ拡張できる設計とする。ただし初期実装では発注、署名、ウォレット操作、トランザクション送信は行わない。

## 2. スコープ

### 初期実装に含める範囲

- Helius RPCを使ったSolanaオンチェーンデータ取得
- RaydiumのSOL/USDCプール価格監視
- OrcaのSOL/USDCプール価格監視
- Meteora-DLMMのSOL/USDC LbPair価格監視
- 設定ファイルによる監視対象プール指定
- 30秒間隔の定期監視
- Raydium、Orca、Meteora-DLMMの全組み合わせ価格差計算
- Discord Embedによるリッチ通知
- SQLiteへのログ保存
- RPCエラー、価格取得失敗、Discord通知失敗などの異常検知と通知

### 初期実装に含めない範囲

- 自動売買
- トランザクション作成
- 秘密鍵またはウォレット管理
- Jupiterなどのオフチェーン集約APIを使った価格取得
- Helius WebSocket、Enhanced API、Webhookの利用
- 複数トークンペアの動的探索
- Meteora-DLMMプールの自動探索
- 裁定利益の確定判定

## 3. 前提条件

- 実装言語はRustとする。
- Botはローカル環境で実行する。
- Solana RPCプロバイダはHeliusを利用する。
- 初期実装ではHeliusのHTTP RPCのみを利用する。
- 監視対象プールはBotが自動探索せず、設定ファイルで明示指定する。
- Meteora-DLMMのLbPairアドレスは初期実装では設定ファイルで手動指定する。
- Meteora-DLMMの自動探索は将来対応とする。
- 監視対象ペアは初期実装ではSOL/USDCのみとする。
- 価格はオンチェーンプール状態からBot内部で算出する。
- DEX手数料とスリッページは考慮する。ただし初期実装での詳細な扱いは「推奨設計」に従う。
- 価格比較の基準は全DEXで`USDC per SOL`に統一する。
- Meteora-DLMMの実装では公式SDKまたは既存crateの利用を許可する。

## 4. 対象取引

- チェーン: Solana mainnet
- 対象DEX: Raydium, Orca, Meteora-DLMM
- 対象ペア: SOL/USDC
- 取引種別: 初期実装では取引なし。価格監視のみ。
- 将来想定: DEX間の価格差を利用した自動裁定取引

## 5. 主要機能

### 5.1 設定読み込み

Botは起動時に`.env`と`config.toml`を読み込む。

推奨構成は以下とする。

- `.env`: 秘密情報、環境依存値
- `config.toml`: Botの挙動、監視対象、通知条件、DB設定

`.env`には少なくとも以下を定義する。

- `HELIUS_RPC_URL`
- `DISCORD_WEBHOOK_URL`

`config.toml`には少なくとも以下を定義する。

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

### 5.2 プール状態取得

Botは30秒ごとにHelius HTTP RPCへリクエストし、設定ファイルで指定されたRaydium、Orca、Meteora-DLMMのプールアカウント情報を取得する。

Botは取得したアカウントデータをDEXごとのプール形式に従ってデコードし、SOL/USDC価格を算出する。

Meteora-DLMMでは、初期実装の監視価格は現在のactive bin価格とする。BotはLbPairアカウントから`active_id`、`bin_step`、トークンmint、手数料計算に必要な`parameters`と`vParameters`、プール状態を取得する。token X/YのdecimalはLbPair上の値として扱わず、LbPairから取得したtoken X/Y mintアドレスのmint accountを追加取得し、mint account dataから読み取る。

スリッページ計算を行う場合、Botはactive bin周辺のBinArrayも取得し、公式SDKまたは既存crateのquoteロジックを利用して想定取引サイズに対する価格インパクトを算出する。

### 5.3 価格計算

Botは各DEXのプール状態から以下を算出する。

- SOL建てまたはUSDC建ての基準価格
- DEX手数料を考慮した価格
- 想定取引サイズが設定されている場合の価格インパクト
- 最終的な比較用価格

初期実装では取引サイズが未定のため、以下の推奨方針とする。

- 基本価格はプール残高または現在価格から算出する。
- DEX手数料は設定値として持ち、比較用価格に反映する。
- 価格インパクト計算は実装可能な範囲で関数を分離する。
- スリッページ計算を有効にする場合、設定ファイルで想定取引サイズを必須にする。
- Meteora-DLMMではactive bin価格を基準価格とし、手数料とスリッページは公式SDKまたは既存crateのquote結果に基づいて算出する。
- Meteora-DLMMのbase feeとvariable feeはLbPair上の`base_fee_bps`や`variable_fee_bps`という固定フィールドから直接読むのではなく、公式SDKのfee helperと同じ式で算出する。
- Meteora-DLMMのbase fee raw rateは`baseFactor * binStep * 10 * 10^baseFeePowerFactor`で算出する。
- Meteora-DLMMのvariable fee raw rateは、`variableFeeControl > 0`の場合に`ceil(variableFeeControl * (volatilityAccumulator * binStep)^2 / 100_000_000_000)`で算出し、`variableFeeControl == 0`の場合は0とする。
- Meteora-DLMMのtotal fee raw rateは`base fee + variable fee`を`MAX_FEE_RATE`で上限クリップする。公式SDKの`FEE_PRECISION`は`1_000_000_000`なので、SQLiteへ保存するbps値は`raw_rate / 100_000`として扱う。
- 将来自動売買を行う場合、想定取引サイズごとの見積もり計算を追加する。

### 5.4 価格差計算

BotはRaydium、Orca、Meteora-DLMMのSOL/USDC価格を全組み合わせで比較し、以下を算出する。

- Raydium価格
- Orca価格
- Meteora-DLMM価格
- 絶対価格差
- 価格差率
- 高いDEX
- 安いDEX
- 手数料考慮後の参考差分

裁定判定しきい値は未定とする。

初期実装では、価格差の大小にかかわらず、監視サイクルごとに全組み合わせの価格差をDiscordへ通知する。

### 5.5 Discord通知

Botは各監視サイクルで価格差情報をDiscordへ通知する。

Discord通知はWebhookのEmbed機能を使い、価格差、DEX別価格、異常状態を読み取りやすいリッチメッセージとして送信する。

価格差通知のEmbedには少なくとも以下を含める。

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

Discord通知にはMeteora-DLMM固有の`active_id`、`bin_step`、fee、statusなどの詳細情報は表示しない。これらの詳細情報はSQLiteへ保存する。

Embedの表示仕様は以下とする。

- `title`には対象ペアと通知種別を表示する。
- `description`には価格差の概要を短く表示する。
- `color`は通知種別に応じて変更する。
- `fields`にはDEX別価格、価格差、価格差率、高いDEX、安いDEX、slot、取得時刻を分けて表示する。
- `timestamp`には監視時刻を設定する。
- `footer`にはBot名、実行環境、RPC種別を表示する。

異常発生時は通常の価格差通知とは別に、エラー通知Embedを送信する。

エラー通知Embedには少なくとも以下を含める。

- 発生時刻
- コンポーネント名
- 重要度
- エラーメッセージ
- 対象DEX
- 対象プールアドレス
- リトライ予定の有無
- 連続エラー回数

### 5.6 ログ保存

Botは監視結果をSQLiteへ保存する。

保存対象は少なくとも以下とする。

- 監視時刻
- 対象ペア
- DEX名
- プールアドレス
- Meteora-DLMM LbPairアドレス
- 算出価格
- 手数料考慮後価格
- 価格差
- 価格差率
- Meteora-DLMMの`active_id`
- Meteora-DLMMの`bin_step`
- Meteora-DLMMのbase fee bps
- Meteora-DLMMのvariable fee bps
- Meteora-DLMMのstatus
- Meteora-DLMMのliquidity
- RPCレスポンスの成否
- エラー種別
- エラーメッセージ

## 6. トレードルール

初期実装では自動売買を行わないため、エントリー条件、利確条件、損切り条件は実装しない。

将来自動売買を追加する場合の候補条件は以下とする。

- DEX間価格差が設定しきい値を超えること
- DEX手数料、Solana手数料、想定スリッページを差し引いて期待利益が残ること
- 対象プールの流動性が設定値以上であること
- RPC取得結果が十分に新しいこと
- 同一方向の取引が短時間に連続しすぎないこと

## 7. リスク管理

初期実装では実取引を行わないため、資金リスクは発生しない。

ただし将来自動売買へ拡張するため、設計上は以下のリスク管理項目を分離可能にする。

- 最大取引サイズ
- 1日あたり最大損失
- 連続エラー時の取引停止
- 最大許容スリッページ
- 最小期待利益
- RPC遅延またはデータ欠損時の停止
- Discord通知失敗時の扱い

## 8. システム構成

推奨モジュール構成は以下とする。

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

データフローは以下とする。

1. Botが設定ファイルを読み込む。
2. 監視対象プール一覧を構築する。
3. 30秒ごとにHelius RPCからプールアカウントを取得する。
4. DEX別デコーダでプール状態を解釈する。
5. SOL/USDC価格を算出する。
6. DEX間価格差を全組み合わせで計算する。
7. 結果をSQLiteへ保存する。
8. Discordへ価格差を通知する。
9. エラー発生時はSQLiteへ記録し、必要に応じてDiscordへ異常通知する。

## 9. データ構造

### 9.1 PoolConfig

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

### 9.2 DexPrice

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

### 9.3 PriceSpread

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

### 9.4 MonitorError

```text
MonitorError
- occurred_at: DateTime
- component: String
- severity: ErrorSeverity
- message: String
- source: Option<String>
```

### 9.5 MeteoraDlmmState

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

`base_fee_bps`、`variable_fee_bps`、`total_fee_bps`は、LbPairから直接デコードしたu64値ではなく、Meteora公式SDK互換の手数料式で算出したraw fee rateをbpsへ変換した値を保存する。

## 10. 外部API

### 10.1 Helius RPC

BotはHelius HTTP RPCを利用する。

初期実装で利用するRPCメソッド候補は以下とする。

- `getAccountInfo`
- `getMultipleAccounts`
- `getTokenAccountBalance`
- `getLatestBlockhash`

価格監視では複数プールの状態をまとめて取得できるため、可能であれば`getMultipleAccounts`を優先する。

### 10.2 Discord Webhook

BotはDiscord Webhook URLへHTTP POSTし、価格差通知と異常通知を送信する。

Discordへの通知ペイロードは`embeds`を含むJSONとする。通常通知と異常通知は別々のEmbedテンプレートを使う。

通知送信失敗時はSQLiteへエラーを記録する。連続失敗時は標準出力または標準エラーにも出力する。

### 10.3 Meteora-DLMM SDKまたは既存crate

Meteora-DLMMのLbPair、BinArray、手数料、quote計算は公式SDKまたは既存crateの利用を許可する。

BotはMeteora-DLMMのアカウントレイアウト、PDA導出、quote計算を自前実装する場合でも、公式SDKまたは既存crateの挙動と照合できる形でテストする。

Meteora-DLMMのtoken decimalを自前実装で取得する場合は、LbPairからtoken X/Y mintアドレスを読み取り、`getMultipleAccounts`でmint accountを取得して、SPL TokenまたはToken-2022のmint account layoutに基づく`mint_decimals`相当の関数でdecimalを読む。

Meteora-DLMMの手数料を自前実装で算出する場合は、Meteora公式SDKの`getBaseFee`、`getVariableFee`、`getTotalFee`、`FEE_PRECISION`、`MAX_FEE_RATE`と同じ挙動をテストで照合する。特にvariable feeは`volatilityAccumulator * binStep`を二乗し、除算は切り上げで行う。

## 11. 設定項目

推奨する`config.toml`構成は以下とする。

```toml
[bot]
interval_seconds = 30
pair = "SOL/USDC"

[database]
path = "data/arbitrage_monitor.sqlite"

[pricing]
consider_dex_fee = true
consider_slippage = true
# 例。最終値は未定。consider_slippage = true の場合は正の値を必須とする。
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

`.env`構成は以下とする。

```text
HELIUS_RPC_URL=https://mainnet.helius-rpc.com/?api-key=...
DISCORD_WEBHOOK_URL=https://discord.com/api/webhooks/...
```

## 12. ログ・通知

### 12.1 SQLite

SQLiteには以下のテーブルを用意する。

- `price_observations`
- `price_spreads`
- `meteora_dlmm_states`
- `monitor_errors`

`price_observations`にはDEXごとの価格取得結果を保存する。

`price_spreads`にはDEX間の比較結果を保存する。

`meteora_dlmm_states`にはMeteora-DLMMの`active_id`、`bin_step`、手数料、status、liquidityなどの固有状態を保存する。

`monitor_errors`にはRPC、デコード、価格計算、DB保存、Discord通知のエラーを保存する。

### 12.2 Discord

BotはDiscord WebhookのEmbed機能を使って通知する。

価格差通知Embedの例:

```json
{
  "username": "solana-dex-price-monitor",
  "embeds": [
    {
      "title": "SOL/USDC Price Spread",
      "description": "Raydium、Orca、Meteora-DLMM の価格差を検出しました。",
      "color": 3447003,
      "fields": [
        { "name": "Raydium", "value": "000.0000 USDC", "inline": true },
        { "name": "Orca", "value": "000.0000 USDC", "inline": true },
        { "name": "Meteora-DLMM", "value": "000.0000 USDC", "inline": true },
        { "name": "Raydium vs Orca", "value": "0.0000 USDC / 0.00 bps", "inline": false },
        { "name": "Raydium vs Meteora-DLMM", "value": "0.0000 USDC / 0.00 bps", "inline": false },
        { "name": "Orca vs Meteora-DLMM", "value": "0.0000 USDC / 0.00 bps", "inline": false },
        { "name": "Higher", "value": "Raydium", "inline": true },
        { "name": "Lower", "value": "Meteora-DLMM", "inline": true },
        { "name": "Slot", "value": "000000000", "inline": true }
      ],
      "footer": {
        "text": "local | Helius HTTP RPC"
      },
      "timestamp": "2026-07-27T00:00:00Z"
    }
  ]
}
```

異常通知Embedの例:

```json
{
  "username": "solana-dex-price-monitor",
  "embeds": [
    {
      "title": "SOL/USDC Monitor Error",
      "description": "Raydiumプールの取得に失敗しました。",
      "color": 16776960,
      "fields": [
        { "name": "Component", "value": "rpc", "inline": true },
        { "name": "Severity", "value": "warning", "inline": true },
        { "name": "DEX", "value": "Raydium", "inline": true },
        { "name": "Pool", "value": "未定", "inline": false },
        { "name": "Retry", "value": "true", "inline": true },
        { "name": "Consecutive Errors", "value": "1", "inline": true }
      ],
      "footer": {
        "text": "local | Helius HTTP RPC"
      },
      "timestamp": "2026-07-27T00:00:00Z"
    }
  ]
}
```

Embedメッセージ生成では、Discordのフィールド数、文字数、ペイロードサイズの制限を超えないようにする。制限超過が見込まれる場合、Botは詳細情報を短縮し、完全な内容はSQLiteログへ保存する。

## 13. エラー処理

Botは以下のエラーを分類して扱う。

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

推奨する挙動は以下とする。

- 設定ファイル不備は起動時に検出し、Botを停止する。
- 一時的なRPC失敗はリトライ対象とする。
- 同一コンポーネントで連続エラーが発生した場合はDiscordへ異常通知する。
- Raydium、Orca、Meteora-DLMMのいずれか1つでも価格取得に失敗した場合、そのサイクル全体を失敗扱いとし、価格差計算をスキップする。
- エラーは可能な限りSQLiteへ保存する。
- Discord通知失敗時は標準エラーへ出力する。

## 14. テスト観点

### 14.1 単体テスト

- 設定ファイルの読み込みとバリデーション
- Raydiumプールデコード
- Orcaプールデコード
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

### 14.2 結合テスト

- モックRPCレスポンスを使った監視サイクルの実行
- Raydium、Orca、Meteora-DLMMのいずれか1つが取得失敗した時の挙動
- SQLite保存失敗時の挙動
- Discord通知失敗時の挙動
- Discord Embedペイロード生成とWebhook送信
- 連続エラー時の異常通知

### 14.3 手動確認

- ローカル環境でBotを起動できること
- 30秒ごとにRaydium、Orca、Meteora-DLMMの価格を取得すること
- 各サイクルでDiscord通知が送信されること
- Discord通知がEmbed形式で表示されること
- SQLiteに価格観測結果と価格差が保存されること
- SQLiteにMeteora-DLMM固有状態が保存されること
- RPC障害や不正設定時に異常通知されること

## 15. 未決事項

- Raydium SOL/USDCの具体的なプールアドレス
- Orca SOL/USDCの具体的なプールアドレス
- Meteora-DLMM SOL/USDCの具体的なLbPairアドレス
- 裁定判定しきい値
- 想定取引サイズ
- スリッページ計算に使う想定取引サイズ
- Raydiumで対象とするプール種別
- Orcaで対象とするプール種別
- Meteora-DLMMの自動探索方法
- SQLiteスキーマの詳細
- Discord Embed通知の最終デザイン
- ローカル実行時の起動方法
