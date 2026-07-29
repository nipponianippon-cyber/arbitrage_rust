# Solana DEX価格監視Botを実装する

このExecPlanは生きたドキュメントです。「進捗」、「驚きと発見」、「決定ログ」、および「結果と反省」のセクションは、作業が進むにつれて常に最新の状態に保つ必要があります。このファイルだけを読んだ次の実装者が、現在のリポジトリを調査し直しながら、Solana上のRaydiumとOrcaのSOL/USDC価格監視Botを完成させられる状態を保ちます。

## 目的 / 全体像

この変更によって、ローカルで動くRust製BotがHeliusのSolana HTTP RPCからオンチェーンのプール情報を取得し、RaydiumとOrcaのSOL/USDC価格を30秒ごとに比較できるようになります。Botは各監視サイクルで価格、価格差、価格差率、高いDEX、安いDEX、比較方向、RPC取得slot、手数料考慮後の参考差分をDiscord Embedとして通知し、同じ内容とエラー情報をSQLiteへ保存します。

初期実装は監視専用です。秘密鍵、ウォレット、署名、トランザクション作成、注文送信、自動売買は実装しません。将来自動売買へ拡張しやすいよう、設定、RPC、DEX別デコード、価格計算、保存、通知、監視ループを分離します。

## 進捗

- [x] (2026-07-27 17:32 +09:00) `SPEC.md`、`Cargo.toml`、`src/main.rs` を読み、現在のプロジェクトが最小構成のRustバイナリであることを確認した。
- [x] (2026-07-27 17:32 +09:00) `EXECPLAN.md` が存在しないことを確認し、新規計画としてこのファイルを作成した。
- [x] (2026-07-27) 設定、エラー型、ログ基盤、依存crateを追加し、起動時に`.env`と`config.toml`を検証できるようにした。
- [x] (2026-07-27) `src/rpc.rs`、`src/dex/`、`src/pricing.rs`、`src/storage.rs`、`src/notifier.rs`、`src/runner.rs`、`config.example.toml`、`.env.example`を追加し、監視Botの初期骨格を静的に実装した。
- [x] (2026-07-30) `SPEC.md`の更新に合わせ、Discord通知を`content`本文ではなく`embeds`配列を含むJSONペイロードに変更した。通常通知Embedには価格差、DEX別価格、比較方向、RPC取得slot、手数料考慮後の参考差分、エラー有無を含める。
- [x] (2026-07-30) エラー通知Embedに発生時刻、コンポーネント名、重要度、メッセージ、対象DEX、対象プールアドレス、リトライ予定、連続エラー回数を含めるため、`MonitorErrorRecord`を拡張した。
- [ ] `config.example.toml`と`src/config.rs`に`discord_embed_enabled`、`bot_name`、`environment`、`notification.embed_colors.normal`、`warning`、`error`を反映し、既存設定との互換性を決めてテストする。
- [x] (2026-07-30) `config.example.toml`と`src/config.rs`へDiscord Embed設定項目と既定値を静的に追加し、`discord_embed_enabled = false`は設定エラーにする方針を実装した。
- [ ] Helius HTTP RPCクライアント、Raydium/Orcaデコード、価格計算、SQLite保存、runnerの初期実装を`cargo fmt`、`cargo check`、`cargo test`で検証し、型エラーと失敗テストを修正する。
- [ ] Raydium AMM v4とOrca Whirlpoolのデコードオフセットを固定fixtureまたは実プールアカウントで検証し、誤りがあればデコーダと計画の決定ログを更新する。
- [ ] 30秒周期の手動確認手順を更新し、DiscordでEmbed表示になること、SQLiteに価格観測・価格差・異常が保存されることを確認する。
- [x] (2026-07-30 01:36 +09:00) `SPEC.md`を再読し、Discord Embed要件が追加されていること、その時点の`src/notifier.rs`がまだ`{"content": message}`形式であることを確認した。

## 驚きと発見

- 観察: 現在のリポジトリには`SPEC.md`、`Cargo.toml`、`Cargo.lock`、`src/main.rs`だけがあり、`Cargo.toml`の`[dependencies]`は空で、`src/main.rs`は`Hello, world!`のみを出力する。
  証拠: `rg --files` は `Cargo.toml`、`Cargo.lock`、`SPEC.md`、`src\main.rs` を返した。
- 観察: `implement-execplan`スキルの制約により、今回の実装では`cargo fmt`、`cargo test`、`cargo check`、`cargo run`を実行していない。
  証拠: スキルはインポート、プロジェクト実行、テスト、ビルド、実行による検証を禁止しているため、差分とファイル再読込による静的確認だけを行った。
- 観察: Raydium AMM v4とOrca Whirlpoolのデコードオフセットは初期実装としてコード化したが、固定fixtureや実プールアカウントで未検証である。
  証拠: `src/dex/raydium.rs`と`src/dex/orca.rs`にオフセット定数を置いたが、テスト実行は禁止されている。
- 観察: `SPEC.md`はDiscord Webhook通知をEmbed形式に更新しており、`src/notifier.rs`も`{"username": "...", "embeds": [...]}`をPOSTする形へ更新済みである。
  証拠: `src/notifier.rs`に`build_price_spread_embed_payload`と`build_error_embed_payload`を追加し、`send_payload`は生成済みJSONをWebhookへPOSTする。
- 観察: 既存SQLite DBへの互換性を考慮し、追加列は`CREATE TABLE IF NOT EXISTS`に含めたうえで、起動時に不足していれば`ALTER TABLE ... ADD COLUMN`する実装にした。
  証拠: `src/storage.rs`の`init_schema`は`price_spreads`へ`comparison_direction`と`fee_adjusted_reference_spread`、`monitor_errors`へ`dex`、`pool_address`、`retry_planned`、`consecutive_count`を追加する。

## 決定ログ

- 決定: 初期実装はRaydium AMM v4とOrca Whirlpoolを明示的な対応対象にする。
  根拠: `SPEC.md`ではRaydiumとOrcaのSOL/USDCプールが対象だが、具体的なプール種別は未決である。Raydium AMM v4とOrca WhirlpoolはSolana上の代表的なプール形式であり、最初の監視Botとして境界を明確にできる。設定値が別形式を指している場合は起動時またはデコード時に明示エラーにする。
  日付/著者: 2026-07-27 / Codex

- 決定: RPCはSolana SDKの高水準クライアントではなく、`reqwest`でJSON-RPCを直接呼ぶ薄い実装から始める。
  根拠: 初期実装で必要なのは`getMultipleAccounts`によるアカウント取得であり、直接JSON-RPCを使うと依存関係と将来のAPI境界を小さく保てる。署名や送信を行わないため、ウォレット系crateは不要である。
  日付/著者: 2026-07-27 / Codex

- 決定: SQLiteは`rusqlite`の`bundled` featureで実装する。
  根拠: Botはローカル実行で、監視周期は30秒と低頻度である。同期I/Oで十分であり、`bundled`を使うことでローカルのSQLiteライブラリ有無に左右されにくい。
  日付/著者: 2026-07-27 / Codex

- 決定: Discord通知本文の生成はHTTP送信から分離する。
  根拠: 通知フォーマットは単体テストしやすく、Webhook URLなしでも価格差通知Embedと異常通知EmbedのJSON構造を検証できる。
  日付/著者: 2026-07-27 / Codex

- 決定: Discord通知は`content`文字列ではなく、通常通知と異常通知の2種類のEmbedペイロードを生成するAPIへ移行する。
  根拠: 2026-07-30時点の`SPEC.md`はDiscord Embedによるリッチ通知を初期実装のスコープに含め、Embedの`title`、`description`、`color`、`fields`、`timestamp`、`footer`を指定している。現行実装はプレーン本文送信なので、仕様準拠には通知モジュールと設定の更新が必要である。
  日付/著者: 2026-07-30 / Codex

- 決定: 今回の実装では実値を含む`.env`、`config.toml`、実行時DBを置く`data/`を`.gitignore`へ追加する。
  根拠: これらは秘密情報またはローカル実行時成果物であり、テンプレートとして`.env.example`と`config.example.toml`を別に追加したため、実値ファイルを追跡対象にしない方が安全である。
  日付/著者: 2026-07-27 / Codex

## 結果と反省

設定読み込み、エラー型、JSON-RPCクライアント、Raydium/Orcaデコーダ、価格差計算、SQLite保存、Discord Embed通知、30秒周期runner、起動処理、設定例は初期実装済みである。2026-07-30時点の`SPEC.md`が要求するEmbed通知へ静的に移行し、通常通知と異常通知のJSONペイロード生成、通知設定、SQLite追加列、エラー文脈を実装した。依存解決、フォーマット、コンパイル、単体テスト、実RPC、SQLite、Discord送信は未検証である。次の作業では、`cargo fmt`、`cargo check`、`cargo test`、Raydium/Orcaのfixture検証を行う必要がある。

## コンテキストと概要

リポジトリルートは `C:\Users\Owner\Documents\arbitrage_rust` である。現時点のプロジェクトはRust 2024 editionのバイナリcrateで、`Cargo.toml`には`tokio`、`reqwest`、`serde`、`rusqlite`などの依存関係が追加済みである。`src/main.rs`は`config`、`dex`、`errors`、`notifier`、`pricing`、`rpc`、`runner`、`storage`を読み込み、`config.toml`、SQLite、RPC client、Discord notifierを組み立てて`runner::run_forever`を起動する。`src/notifier.rs`は現時点でプレーン本文のDiscord Webhook送信を実装しているが、`SPEC.md`はDiscord Embed通知を要求しているため、次の実装ではここを更新する。

RPCはRemote Procedure Callの略で、この計画ではBotがHeliusのHTTPエンドポイントへJSONを送ってSolanaアカウント情報を取得する通信を指す。DEXは分散型取引所のことで、この計画ではRaydiumとOrcaを指す。プールはDEX上でトークン残高や現在価格を保持するオンチェーンアカウントであり、Botは設定ファイルに書かれたプールアドレスを読み取って監視する。SQLiteはローカルファイルとして保存できる軽量データベースで、このBotでは監視結果とエラー履歴の保存先になる。

`SPEC.md`には未決事項が残っている。RaydiumとOrcaの具体的なプールアドレス、裁定判定しきい値、想定取引サイズ、価格インパクト計算の厳密さ、対象プール種別、SQLiteスキーマ詳細、Discord Embed通知の最終デザイン、起動方法が未確定である。この計画では、初期実装に必要な境界を次のように固定する。プール種別はRaydium AMM v4とOrca Whirlpoolを対象とし、プールアドレスは`config.toml`で指定する。裁定判定は行わず、価格差の大小に関係なく毎サイクル通知する。想定取引サイズが未設定の場合、価格インパクト計算は無効にする。Discord通知は`SPEC.md`に合わせ、通常通知と異常通知をEmbedペイロードとして生成する。

## 作業計画

最初に、`Cargo.toml`へ必要な依存crateを追加する。非同期実行には`tokio`、HTTPには`reqwest`、設定とJSON/TOML処理には`serde`、`serde_json`、`toml`、`.env`読み込みには`dotenvy`、エラー型には`thiserror`、時刻には`chrono`、小数計算には`rust_decimal`、base64デコードには`base64`、Solanaアドレスの基本検証には`bs58`、SQLite保存には`rusqlite`、ログには`tracing`と`tracing-subscriber`を使う。

次に、`src`配下を機能別モジュールへ分ける。`src/config.rs`は`.env`と`config.toml`を読み込み、必須値がない場合は起動を止める。`src/errors.rs`は`AppError`、`ErrorSeverity`、`MonitorErrorRecord`を定義する。`src/rpc.rs`はHelius HTTP RPCの`getMultipleAccounts`を呼び、アカウントのbase64データ、slot、アカウント所有者、lamportsを構造体として返す。`src/dex/mod.rs`はDEX共通の`DexKind`、`DexPrice`、`PoolDecoder`に相当する境界を定義し、`src/dex/raydium.rs`と`src/dex/orca.rs`が各プール形式のデコードを担当する。

Raydium AMM v4では、プールアカウントからbase vault、quote vault、base mint、quote mint、base decimal、quote decimal、swap fee numerator、swap fee denominatorを取り出す。価格はSPL Token vaultアカウントのamountを読み、quote数量をbase数量で割ってSOLあたりのUSDC価格として算出する。SPL Token Accountのamountはアカウントデータ内の64バイト目から始まる8バイトのlittle-endian整数として読み取る。RaydiumのOpenBook未決済残高を初期価格に含めるかは複雑さが上がるため初期実装では含めず、必要になった場合は別マイルストーンで追加する。

Orca Whirlpoolでは、プールアカウントからtoken mint A、token vault A、token mint B、token vault B、fee rate、sqrt priceを取り出す。Whirlpoolの`sqrt_price`は固定小数点の平方根価格で、`(sqrt_price / 2^64)^2`に小数桁補正を加えてtoken B per token Aの価格に変換する。設定の`base_mint`と`quote_mint`を見て、SOL/USDCの向きに合わせて価格を反転するかどうかを決める。価格の向きが不明な場合は、静かに推測せずデコードエラーとして記録する。

`src/pricing.rs`は、`DexPrice`の比較、絶対価格差、価格差率、手数料考慮後価格、任意の価格インパクト計算を扱う。手数料考慮後価格は、比較用の参考価格として`price * (1 + fee_rate)`または`price * (1 - fee_rate)`を機械的に適用するのではなく、買う側と売る側を区別できる関数にする。初期通知では、各DEXでSOLを買う場合の手数料込み参考価格と、SOLを売る場合の手数料控除後参考価格を計算できるようにする。`trade_size_usdc`が`null`の場合、価格インパクトは`None`にして通知にも「disabled」と出せる形にする。

`src/storage.rs`はSQLiteファイルとテーブル作成、価格観測、価格差、エラー記録を担当する。起動時に`CREATE TABLE IF NOT EXISTS`で次の3テーブルを作る。`price_observations`はDEXごとの価格、手数料考慮後価格、slot、RPC成否、エラーを保存する。`price_spreads`はRaydiumとOrcaの比較結果、比較方向、手数料考慮後の参考差分を保存する。`monitor_errors`はRPC、デコード、価格計算、DB保存、Discord通知のエラーを保存し、可能であれば対象DEX、対象プールアドレス、リトライ予定、連続エラー回数も保存する。全ての時刻はRFC3339文字列で保存する。

`src/notifier.rs`はDiscord WebhookへのHTTP POSTとEmbedペイロード生成を担当する。`build_price_spread_embed_payload`と`build_error_embed_payload`をHTTP送信から分け、Webhook URLがなくても単体テストでJSON構造を確認できるようにする。通常通知Embedは`title`、`description`、`color`、`fields`、`timestamp`、`footer`を持ち、fieldsにはDEX別価格、価格差、価格差率、高いDEX、安いDEX、比較方向、slot、取得時刻、手数料考慮後の参考差分、エラー有無を入れる。異常通知Embedは発生時刻、コンポーネント名、重要度、メッセージ、対象DEX、対象プールアドレス、リトライ予定、連続エラー回数を入れる。Discord送信が失敗した場合は`monitor_errors`へ記録し、さらに標準エラーへ短く出力する。

`src/runner.rs`は30秒周期の監視ループを持つ。1回のサイクルでは、有効なプール設定を読み、必要なプールアカウントとvaultアカウントをRPCから取得し、DEX別デコーダで`DexPrice`へ変換し、RaydiumとOrcaの両方が揃った場合だけ`PriceSpread`を計算して保存と通常通知を行う。片方が失敗した場合は価格差計算をスキップし、エラー保存と異常通知を行う。監視ループの内部処理は`run_once`として分離し、統合テストでは30秒待たずに1サイクルだけ検証できるようにする。

最後に、`src/main.rs`は起動処理だけに薄く保つ。`.env`読み込み、設定読み込み、ログ初期化、SQLite初期化、RPCクライアントとDiscord通知クライアントの組み立て、`runner`の起動を行う。`config.example.toml`と`.env.example`を追加し、ユーザーが実値を入れるべき項目を示す。`config.example.toml`には`notification.discord_embed_enabled`、`notification.bot_name`、`notification.environment`、`notification.embed_colors`を含める。実際の`config.toml`と`.env`は秘密情報やローカル値を含むため、必要であれば`.gitignore`へ追加する。

## マイルストーン

### マイルストーン1: 起動設定とプロジェクト骨格

このマイルストーンでは、Botが設定ファイルと環境変数を読み、起動前に不備を検出できる状態にする。`Cargo.toml`に依存crateを追加し、`src/config.rs`、`src/errors.rs`、`src/main.rs`、`config.example.toml`、`.env.example`を整える。完了時には、`cargo test`で設定読み込みとバリデーションの単体テストが通り、`HELIUS_RPC_URL`や`DISCORD_WEBHOOK_URL`がない場合に分かりやすいエラーで終了する。

### マイルストーン2: RPC取得とアカウントデータ解析の土台

このマイルストーンでは、Helius HTTP RPCへ`getMultipleAccounts`を送る薄いクライアントを作る。実ネットワークに依存しない単体テストでは、JSON-RPCレスポンスのサンプル文字列からbase64データ、slot、ownerを取り出せることを確認する。完了時には、実装者が有効な`HELIUS_RPC_URL`を入れたローカル環境で1回だけRPC取得を試せる補助テストまたは手動確認手順が存在する。

### マイルストーン3: DEXデコードと価格算出

このマイルストーンでは、Raydium AMM v4とOrca Whirlpoolのプールデコードを実装する。Raydiumはプールアカウントからvaultと手数料を取り、vaultのSPL Token amountから残高比価格を作る。OrcaはWhirlpoolの`sqrt_price`から価格を作り、token mintの向きを設定と照合する。完了時には、固定バイト列fixtureまたは小さな手作りバッファを使う単体テストで、正しいvault、mint、fee、価格方向が得られることを確認できる。

### マイルストーン4: 価格差、SQLite、Discord Embed通知

このマイルストーンでは、DEXごとの価格を比較し、保存とDiscord Embed通知を行う実用部分を実装する。`src/pricing.rs`は絶対価格差、bps単位の価格差率、高いDEX、安いDEX、比較方向、手数料考慮後の参考差分を返す。`src/storage.rs`は3テーブルを作り、成功サイクルとエラーサイクルを保存する。`src/notifier.rs`は通常通知Embedと異常通知EmbedのJSONペイロードを作り、WebhookへPOSTする。完了時には、Webhook送信をモックしたテストで、価格差通知と異常通知のEmbedが`SPEC.md`の項目を含み、`content`だけの旧形式では送信されないことを確認できる。

### マイルストーン5: 監視ループ結合と手動受け入れ

このマイルストーンでは、`runner`が全モジュールを結合して30秒ごとの監視を行う。`run_once`で1サイクルを検証し、`run_forever`で本番の周期実行を行う。完了時には、ユーザーが`config.toml`と`.env`に実値を入れて`cargo run`を実行すると、SQLiteファイルが作られ、DiscordにSOL/USDCの価格差Embed通知が届く。RPC、デコード、DB、Discordのいずれかで失敗した場合は、Botが停止すべき設定エラーを除き、エラーを記録して次サイクルへ進む。

## 具体的な手順

作業ディレクトリは常に `C:\Users\Owner\Documents\arbitrage_rust` とする。実装者は作業前に `git status --short` を実行し、ユーザー由来の未追跡または変更済みファイルを把握する。現在確認済みの状態では、`.gitignore`、`Cargo.lock`、`Cargo.toml`、`SPEC.md`、`src/`が未追跡である。これは既存状態として扱い、ユーザーの明示指示なしに削除しない。

最初の編集では、`Cargo.toml`の`[dependencies]`へ以下を追加する。バージョンは実装時点でCargoが解決できる安定版を使い、既存の`Cargo.lock`は`cargo check`または`cargo test`で更新する。

    tokio = { version = "1", features = ["macros", "rt-multi-thread", "time"] }
    reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
    serde = { version = "1", features = ["derive"] }
    serde_json = "1"
    toml = "0.8"
    dotenvy = "0.15"
    thiserror = "1"
    chrono = { version = "0.4", features = ["serde", "clock"] }
    rust_decimal = { version = "1", features = ["serde"] }
    base64 = "0.22"
    bs58 = "0.5"
    rusqlite = { version = "0.32", features = ["bundled"] }
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["env-filter"] }

次に、`src/config.rs`を作成または更新する。`AppConfig`は`bot`、`database`、`pricing`、`notification`、`pools`、`helius_rpc_url`、`discord_webhook_url`を持つ。`helius_rpc_url`と`discord_webhook_url`は`.env`またはプロセス環境変数から読み、その他は`config.toml`から読む。`NotificationConfig`には`discord_enabled`、`discord_embed_enabled`、`notify_every_cycle`、`notify_on_error`、`bot_name`、`environment`、`embed_colors`を持たせる。`embed_colors`には`normal`、`warning`、`error`を整数で持たせ、未指定時の既定値を用意する。`PoolConfig`には`dex`、`pair`、`pool_address`、`base_mint`、`quote_mint`、`enabled`を持たせる。`DexKind`は`Raydium`と`Orca`だけを許可し、それ以外の文字列は設定エラーにする。`interval_seconds`が0、`pools`が空、SOL/USDCのRaydiumまたはOrcaが欠ける場合も設定エラーにする。

`src/errors.rs`を作成し、設定、RPC、デコード、価格計算、DB、通知のエラー分類を定義する。外部に表示するエラーは人間が読める短い文を持ち、内部原因がある場合は`source`として保持する。`MonitorErrorRecord`はSQLite保存とDiscord異常通知に再利用する。

`src/rpc.rs`を作成し、`RpcClient::get_multiple_accounts(&self, addresses: &[String]) -> Result<Vec<AccountData>, AppError>`を実装する。リクエストボディはJSON-RPC 2.0の`getMultipleAccounts`で、encodingは`base64`、commitmentは`confirmed`にする。`AccountData`は`address`、`owner`、`lamports`、`data`、`slot`を持つ。HTTP失敗、JSON-RPC error、base64デコード失敗、null accountはそれぞれRPCエラーとして分類する。

`src/dex/mod.rs`、`src/dex/raydium.rs`、`src/dex/orca.rs`を作成する。`DexPrice`は`dex`、`pair`、`pool_address`、`price`、`fee_adjusted_price`、`liquidity`、`slot`、`observed_at`を持つ。RaydiumとOrcaのデコーダは、設定されたpool accountと必要なvault accountを入力として受け、`DexPrice`を返す。デコーダは設定の`base_mint`と`quote_mint`を必ず照合し、SOL/USDC以外または向きが判断できない場合はデコードエラーにする。

`src/pricing.rs`を作成し、`PriceSpread`と価格計算関数を実装する。`calculate_spread(raydium: DexPrice, orca: DexPrice) -> Result<PriceSpread, AppError>`は、価格が0以下またはペア不一致の場合に価格計算エラーを返す。`spread_bps`は安いDEX価格を分母にして、`absolute_spread / lower_price * 10000`で計算する。

`src/storage.rs`を作成または更新し、`Storage::open(path)`、`Storage::init_schema()`、`insert_price_observation`、`insert_price_spread`、`insert_monitor_error`を実装する。テーブルは次の列を最低限含む。既にテーブルが存在する場合は破壊的なDROPを行わず、必要な列追加は`ALTER TABLE ... ADD COLUMN`または互換的な新テーブル作成で扱う。

    price_observations: id, observed_at, dex, pair, pool_address, price, fee_adjusted_price, liquidity, slot, rpc_success, error_kind, error_message
    price_spreads: id, calculated_at, pair, raydium_price, orca_price, absolute_spread, spread_bps, higher_dex, lower_dex, comparison_direction, fee_adjusted_reference_spread
    monitor_errors: id, occurred_at, component, severity, message, source, dex, pool_address, retry_planned, consecutive_count

`src/notifier.rs`を作成または更新し、`DiscordNotifier::send_price_spread`、`DiscordNotifier::send_error`、`build_price_spread_embed_payload`、`build_error_embed_payload`を実装する。Webhook HTTP POSTのJSONは`{"username": "...", "embeds": [...]}`とする。`notification.discord_enabled`がfalseの場合、HTTP送信は行わず、生成したEmbed JSONをログ出力できる形にする。`notification.discord_embed_enabled`がfalseの場合の扱いは、初期実装では互換用にプレーン本文へ戻すのではなく設定エラーにしてよい。理由は`SPEC.md`がEmbedを初期実装スコープとして要求しているためである。

`src/runner.rs`を作成し、`run_once`と`run_forever`を実装する。`run_once`は設定から有効プールを選び、RPCで必要アカウントを取得し、DEX価格を作り、保存、価格差計算、通知、エラー処理を一通り行う。`run_forever`は`tokio::time::interval`で`bot.interval_seconds`ごとに`run_once`を呼ぶ。`Ctrl+C`で終了できるよう、必要なら`tokio::signal::ctrl_c`を使う。

`src/main.rs`を更新し、`tracing_subscriber`初期化、設定読み込み、storage初期化、RPC client、notifier、runner起動を順に行う。起動時設定エラーは標準エラーに出して終了する。監視中の一時エラーはrunner内で記録し、プロセスを落とさない。

最後に、`config.example.toml`と`.env.example`を追加または更新する。`config.example.toml`は`SPEC.md`の推奨構成に合わせ、`discord_embed_enabled = true`、`bot_name = "solana-dex-price-monitor"`、`environment = "local"`、`[notification.embed_colors]`の`normal = 3447003`、`warning = 16776960`、`error = 15158332`を含める。`config.example.toml`のpool addressは、実行しやすさを優先して既知の候補アドレスを置いてもよいが、実プール種別がデコーダ前提と一致することをfixtureまたは手動RPCで確認するまでは「検証未完了」と計画に記録する。`.env.example`には`HELIUS_RPC_URL`と`DISCORD_WEBHOOK_URL`のキーだけを置き、実値は書かない。

各マイルストーンの終わりに次を実行する。

    cargo fmt
    cargo test
    cargo check

依存関係追加直後にネットワーク制限でcrate取得に失敗した場合は、実装者はその出力を「驚きと発見」に記録し、ネットワーク許可を得て同じコマンドを再実行する。テストや実行で実DBファイルが必要な場合は、`data/`配下へ作成し、同じパスを再利用できるようにする。

## 検証と受け入れ

単体テストでは、設定読み込み、設定不備、JSON-RPCレスポンス解析、base64デコード失敗、Raydiumデコード、Orcaデコード、SPL Token amount読取、価格差率計算、手数料考慮後価格、Discord Embedペイロード生成、通常通知Embedと異常通知Embedのテンプレート選択、SQLiteテーブル作成とinsertを検証する。`cargo test`を実行し、全テストが成功することを受け入れ条件にする。

設定検証の受け入れでは、`.env`に`HELIUS_RPC_URL`または`DISCORD_WEBHOOK_URL`がない状態で`cargo run`を実行すると、Botは監視ループに入らず、どの値が不足しているかを含む設定エラーを表示する。`config.toml`のプールアドレスが`未定`または空文字のままでも同じく設定エラーにする。

手動の成功確認では、ユーザーが`.env`と`config.toml`へ実値を入れた後、次を実行する。

    cargo run

期待される観察結果は、起動ログに設定読み込み成功とSQLite初期化成功が表示され、最初の監視サイクルでRaydium価格、Orca価格、価格差、価格差率が計算されることである。Discord Webhookが有効なら、DiscordチャンネルにEmbed通知が届く。Embedには次の情報が分かれたfieldsとして表示される。

    title: SOL/USDC Price Spread
    description: Raydium と Orca の価格差概要
    Raydium: <number> USDC
    Orca: <number> USDC
    Spread: <number> USDC / <number> bps
    Higher: <Raydium or Orca>
    Lower: <Raydium or Orca>
    Direction: buy on <lower DEX>, sell on <higher DEX>
    Slot: <slot number or n/a>
    Fee Adjusted Reference Spread: <number or disabled>
    Errors: none
    timestamp: <RFC3339 timestamp>
    footer: local | Helius HTTP RPC

SQLiteの受け入れでは、`config.toml`の`database.path`に指定したファイルが作成され、`price_observations`にRaydiumとOrcaの観測行、`price_spreads`に比較行が追加されることを確認する。確認にはSQLite CLIがある環境なら次を使う。

    sqlite3 data/arbitrage_monitor.sqlite "select pair, raydium_price, orca_price, spread_bps from price_spreads order by id desc limit 1;"

SQLite CLIがない環境では、`rusqlite`を使うテストでinsert後のselectを検証する。

異常系の受け入れでは、片方のプールアドレスを不正なbase58文字列または存在しないアカウントに変更し、`run_once`相当のテストまたは手動実行で価格差計算がスキップされ、`monitor_errors`へエラーが保存され、Discord異常通知Embedまたは標準エラー出力が発生することを確認する。異常通知Embedには`Component`、`Severity`、`DEX`、`Pool`、`Retry`、`Consecutive Errors`が含まれる。

## 冪等性と復旧

`Storage::init_schema()`は`CREATE TABLE IF NOT EXISTS`だけを使い、複数回実行しても既存データを消さない。`config.example.toml`と`.env.example`はテンプレートであり、実値を含む`config.toml`と`.env`を上書きしない。実装中にテストDBを作る場合は、テスト専用の一時ディレクトリまたは`data/test_*.sqlite`を使い、本番用DBと混ぜない。

RPCやDiscordの一時失敗はBot全体を停止させず、`monitor_errors`に記録して次サイクルで再試行する。設定不備、DBオープン失敗、プール形式不一致のように継続しても成功しない可能性が高い問題は、起動時または該当サイクルで明示的にエラー化する。デコーダのオフセットやプール形式が誤っていた場合は、fixtureテストを追加して失敗を再現してから修正し、「驚きと発見」と「決定ログ」を更新する。

ユーザーの未追跡ファイルや変更済みファイルは削除しない。`Cargo.lock`は依存関係解決で更新されるため、`Cargo.toml`と一緒に扱う。実装者が途中で方針を変える場合は、変更理由をこのExecPlanの「決定ログ」に追記し、「進捗」の該当チェック項目も分割または更新する。

## アーティファクトとメモ

2026-07-30 01:36 +09:00時点で確認済みのプロジェクト構造は次の通りである。

    Cargo.toml
    Cargo.lock
    SPEC.md
    EXECPLAN.md
    config.example.toml
    src\config.rs
    src\dex\mod.rs
    src\dex\orca.rs
    src\dex\raydium.rs
    src\errors.rs
    src\main.rs
    src\notifier.rs
    src\note.ipynb
    src\pricing.rs
    src\rpc.rs
    src\runner.rs
    src\storage.rs

現在の`Cargo.toml`はRust 2024 editionのバイナリcrateで、依存関係は追加済みである。

    [package]
    name = "arbitrage_rust"
    version = "0.1.0"
    edition = "2024"

    [dependencies]
    tokio = { version = "1", features = ["macros", "rt-multi-thread", "time", "signal"] }
    reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
    serde = { version = "1", features = ["derive"] }
    serde_json = "1"
    toml = "0.8"
    dotenvy = "0.15"
    thiserror = "1"
    chrono = { version = "0.4", features = ["serde", "clock"] }
    rust_decimal = { version = "1", features = ["serde"] }
    base64 = "0.22"
    bs58 = "0.5"
    rusqlite = { version = "0.32", features = ["bundled"] }
    tracing = "0.1"
    tracing-subscriber = { version = "0.3", features = ["env-filter"] }

現在の`src/main.rs`は起動処理を持つ。

    mod config;
    mod dex;
    mod errors;
    mod notifier;
    mod pricing;
    mod rpc;
    mod runner;
    mod storage;

`src/notifier.rs`はEmbed形式へ更新済みであり、通常通知と異常通知は次の形のWebhook JSONをPOSTする。

    {"username": "...", "embeds": [...]}

`SPEC.md`で要求されている主要なデータ構造は、実装時にRust型として具体化する。`PoolConfig`は設定用、`DexPrice`はDEX別の価格観測、`PriceSpread`は2つのDEXの比較結果、`MonitorError`は異常記録を表す。

## インターフェースと依存関係

`src/config.rs`には次を定義する。

    pub struct AppConfig {
        pub bot: BotConfig,
        pub database: DatabaseConfig,
        pub pricing: PricingConfig,
        pub notification: NotificationConfig,
        pub pools: Vec<PoolConfig>,
        pub helius_rpc_url: String,
        pub discord_webhook_url: String,
    }

    pub fn load_config(config_path: impl AsRef<Path>) -> Result<AppConfig, AppError>;

    pub struct NotificationConfig {
        pub discord_enabled: bool,
        pub discord_embed_enabled: bool,
        pub notify_every_cycle: bool,
        pub notify_on_error: bool,
        pub bot_name: String,
        pub environment: String,
        pub embed_colors: EmbedColors,
    }

    pub struct EmbedColors {
        pub normal: u32,
        pub warning: u32,
        pub error: u32,
    }

`src/dex/mod.rs`には次を定義する。

    pub enum DexKind {
        Raydium,
        Orca,
    }

    pub struct DexPrice {
        pub dex: DexKind,
        pub pair: String,
        pub pool_address: String,
        pub price: Decimal,
        pub fee_adjusted_price: Option<Decimal>,
        pub liquidity: Option<Decimal>,
        pub slot: Option<u64>,
        pub observed_at: DateTime<Utc>,
    }

`src/rpc.rs`には次を定義する。

    pub struct RpcClient {
        endpoint: String,
        http: reqwest::Client,
    }

    pub async fn get_multiple_accounts(&self, addresses: &[String]) -> Result<Vec<AccountData>, AppError>;

`src/pricing.rs`には次を定義する。

    pub struct PriceSpread {
        pub pair: String,
        pub dex_a: DexPrice,
        pub dex_b: DexPrice,
        pub absolute_spread: Decimal,
        pub spread_bps: Decimal,
        pub higher_dex: DexKind,
        pub lower_dex: DexKind,
        pub comparison_direction: String,
        pub fee_adjusted_reference_spread: Option<Decimal>,
        pub calculated_at: DateTime<Utc>,
    }

    pub fn calculate_spread(dex_a: DexPrice, dex_b: DexPrice) -> Result<PriceSpread, AppError>;

`src/storage.rs`には次を定義する。

    pub struct Storage {
        conn: rusqlite::Connection,
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Storage, AppError>;
    pub fn init_schema(&self) -> Result<(), AppError>;
    pub fn insert_price_observation(&self, price: &DexPrice) -> Result<(), AppError>;
    pub fn insert_price_spread(&self, spread: &PriceSpread) -> Result<(), AppError>;
    pub fn insert_monitor_error(&self, error: &MonitorErrorRecord) -> Result<(), AppError>;

`src/notifier.rs`には次を定義する。

    pub struct DiscordNotifier {
        webhook_url: String,
        enabled: bool,
        bot_name: String,
        environment: String,
        embed_colors: EmbedColors,
        http: reqwest::Client,
    }

    pub fn build_price_spread_embed_payload(
        spread: &PriceSpread,
        bot_name: &str,
        environment: &str,
        embed_colors: &EmbedColors,
    ) -> serde_json::Value;

    pub fn build_error_embed_payload(
        error: &MonitorErrorRecord,
        bot_name: &str,
        environment: &str,
        embed_colors: &EmbedColors,
    ) -> serde_json::Value;

`src/runner.rs`には次を定義する。

    pub async fn run_once(
        config: &AppConfig,
        rpc: &RpcClient,
        storage: &Storage,
        notifier: &DiscordNotifier,
    ) -> Result<(), AppError>;

    pub async fn run_forever(
        config: AppConfig,
        rpc: RpcClient,
        storage: Storage,
        notifier: DiscordNotifier,
    ) -> Result<(), AppError>;

`MonitorErrorRecord`は既存の`occurred_at`、`component`、`severity`、`message`、`source`に加えて、Embed表示とSQLite保存のために`dex: Option<DexKind>`、`pool_address: Option<String>`、`retry_planned: bool`、`consecutive_count: u32`を持てる形へ拡張する。既存コードへの影響が大きい場合は、`MonitorErrorContext`のような通知専用補助型を作り、`MonitorErrorRecord`本体は最小変更に留めてもよい。その場合も、異常通知EmbedとSQLiteには`SPEC.md`が要求する項目を渡せる必要がある。

これらの名前は後続実装の安定した目印である。実装中に所有権や非同期境界の都合で引数型を`Arc<Storage>`などへ変更する場合は、このセクションと「決定ログ」を更新する。
