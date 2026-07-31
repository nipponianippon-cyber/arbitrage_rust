# Raydium・Orca・Meteora-DLMM対応のSolana DEX価格監視Botを完成させる

このExecPlanは生きたドキュメントです。「進捗」、「驚きと発見」、「決定ログ」、および「結果と反省」のセクションは、作業が進むにつれて常に最新の状態に保つ必要があります。このファイルだけを読んだ次の実装者が、現在のリポジトリを調査し直しながら、Solana上のRaydium、Orca、Meteora-DLMMのSOL/USDC価格監視Botを完成させられる状態を保ちます。

## 目的 / 全体像

この変更によって、ローカルで動くRust製BotがHeliusのSolana HTTP RPCからオンチェーンのプール情報を取得し、Raydium、Orca、Meteora-DLMMのSOL/USDC価格を30秒ごとに比較できるようになります。Botは価格比較の基準を全DEXで`USDC per SOL`へ統一し、Raydium vs Orca、Raydium vs Meteora-DLMM、Orca vs Meteora-DLMMの全組み合わせについて、価格差、価格差率、高いDEX、安いDEX、比較方向、RPC取得slot、手数料考慮後の参考差分をDiscord Embedとして通知します。同じ監視結果とエラー情報はSQLiteへ保存します。

初期実装は監視専用です。秘密鍵、ウォレット、署名、トランザクション作成、注文送信、自動売買は実装しません。将来自動売買へ拡張しやすいよう、設定、RPC、DEX別デコード、価格計算、保存、通知、監視ループを分離します。

## 進捗

- [x] (2026-07-31 00:00 +09:00) `SPEC.md`を再読し、対象DEXがRaydium、Orca、Meteora-DLMMへ拡張され、価格差計算が全組み合わせになったことを確認した。
- [x] (2026-07-31 00:00 +09:00) 現在のファイル構成を確認し、`src/dex/raydium/`、`src/dex/orca/`、`src/dex/meteora/`へDEX実装が分割されていること、`src/dex/meteora/meteora_amm.rs`が途中実装であることを確認した。
- [x] (2026-07-31 00:00 +09:00) `PLANS.md`を最新SPECに合わせて、Meteora-DLMM、全組み合わせ価格差、Discord Embed、SQLiteのMeteora固有状態保存を含む計画へ更新した。
- [ ] `DexKind`、`PoolConfig`、設定バリデーションをMeteora-DLMMに対応させる。`dex = "meteora_dlmm"`、`lb_pair_address`、`price_orientation = "usdc_per_sol"`、`auto_discovery = false`を扱えるようにする。
- [ ] `src/dex/meteora/meteora_amm.rs`の途中実装を完成させ、LbPairから`active_id`、`bin_step`、token mint、reserve、手数料、statusをデコードできるようにする。
- [ ] Meteora-DLMMの基準価格をactive bin価格として算出し、全DEXの`DexPrice.price`を`USDC per SOL`へ正規化する。
- [ ] `pricing`を2 DEX専用から任意のDEX価格リストの全組み合わせ比較へ拡張する。
- [ ] SQLiteへ`meteora_dlmm_states`を追加し、`price_spreads`をDEXペア汎用のスキーマへ移行する。
- [ ] Discord Embed通知を3 DEX価格と全組み合わせ価格差の表示へ更新し、Meteora-DLMM固有詳細はDiscordへ出さずSQLiteへ保存する。
- [ ] Raydium、Orca、Meteora-DLMMのいずれか1つでも価格取得に失敗した場合、そのサイクル全体を失敗扱いにして価格差計算をスキップする。
- [ ] `cargo fmt`、`cargo check`、`cargo test`を実行し、コンパイル、単体テスト、静的な仕様整合を確認する。

## 驚きと発見

- 観察: 最新の`SPEC.md`では、初期実装対象にMeteora-DLMMのSOL/USDC LbPair監視が追加されている。
  証拠: `SPEC.md`の「1. 目的」、「2. スコープ」、「3. 前提条件」、「5.2 プール状態取得」にMeteora-DLMM、LbPair、active bin、BinArray、公式SDKまたは既存crateの利用許可が記載されている。
- 観察: 現行コードの`DexKind`には`Meteora` variantがあるが、`FromStr`は`raydium`と`orca`しか受け付けない。
  証拠: `src/dex/mod.rs`の`FromStr for DexKind`は`"meteora"`または`"meteora_dlmm"`をまだ処理していない。
- 観察: `src/dex/meteora/meteora_amm.rs`は途中で終わっており、現時点ではコンパイル不能の可能性が高い。
  証拠: `decode_pool_meta`内で`token_x_decimal:`の後に値がなく、構造体生成が完結していない。
- 観察: 既存の`pricing`と`storage`はRaydium/Orcaの2 DEX比較を前提としている。
  証拠: `src/pricing.rs`の`calculate_spread`は2つの`DexPrice`だけを受け取り、`src/storage.rs`の`price_spreads`は`raydium_price`と`orca_price`列を持つ。

## 決定ログ

- 決定: 価格比較の基準は全DEXで`USDC per SOL`に統一する。
  根拠: `SPEC.md`がこの基準を明記している。DEXごとにtoken orderや価格式が異なるため、デコーダ境界で正規化し、`pricing`は統一済み価格だけを比較する方が安全である。
  日付/著者: 2026-07-31 / Codex

- 決定: Meteora-DLMMの初期監視価格はLbPairのactive bin価格を基準価格にする。
  根拠: `SPEC.md`が初期実装の監視価格を現在のactive bin価格と指定している。想定取引サイズが設定されたスリッページ計算は、BinArrayとquoteロジックを使う追加段階として扱う。
  日付/著者: 2026-07-31 / Codex

- 決定: Meteora-DLMMのプール自動探索は実装しない。
  根拠: `SPEC.md`が自動探索を初期実装に含めない範囲として明記しており、LbPairアドレスは設定ファイルで手動指定する前提である。
  日付/著者: 2026-07-31 / Codex

- 決定: Raydium、Orca、Meteora-DLMMのどれか1つでも価格取得に失敗したサイクルでは、全組み合わせ価格差を計算しない。
  根拠: `SPEC.md`のエラー処理は、3 DEXのいずれか1つでも価格取得に失敗した場合、そのサイクル全体を失敗扱いにして価格差計算をスキップするとしている。欠けたDEXを除いた部分比較は初期実装では行わない。
  日付/著者: 2026-07-31 / Codex

- 決定: DiscordにはMeteora-DLMM固有詳細を表示せず、SQLiteに保存する。
  根拠: `SPEC.md`は`active_id`、`bin_step`、fee、statusなどのMeteora-DLMM詳細をDiscordへ表示しないと指定し、詳細情報はSQLiteへ保存するとしている。
  日付/著者: 2026-07-31 / Codex

## 結果と反省

現時点では計画更新のみを行った。コード実装、フォーマット、コンパイル、テスト、実RPC、SQLite、Discord送信はこの更新では実施していない。次の実装作業では、まずコンパイル不能箇所になり得る`src/dex/meteora/meteora_amm.rs`を完成させ、続いて設定、価格比較、保存、通知を3 DEX前提へ拡張する必要がある。

## コンテキストと概要

リポジトリルートは `C:\Users\Owner\Documents\arbitrage_rust` である。現在のプロジェクトはRust 2024 editionのバイナリcrateで、`Cargo.toml`には`tokio`、`reqwest`、`serde`、`serde_json`、`toml`、`dotenvy`、`thiserror`、`chrono`、`rust_decimal`、`base64`、`bs58`、`rusqlite`、`tracing`、`tracing-subscriber`が追加済みである。

RPCはRemote Procedure Callの略で、この計画ではBotがHeliusのHTTPエンドポイントへJSONを送ってSolanaアカウント情報を取得する通信を指す。DEXは分散型取引所のことで、この計画ではRaydium、Orca、Meteora-DLMMを指す。Meteora-DLMMはMeteoraのDynamic Liquidity Market Makerで、LbPairというプールアカウントと、active binという現在価格帯を使う。SQLiteはローカルファイルとして保存できる軽量データベースで、このBotでは監視結果、Meteora固有状態、エラー履歴の保存先になる。

2026-07-31時点で確認済みの主要ファイルは次の通りである。

    Cargo.toml
    Cargo.lock
    SPEC.md
    PLANS.md
    config.example.toml
    src\config.rs
    src\dex\mod.rs
    src\dex\raydium\mod.rs
    src\dex\raydium\raydium_amm.rs
    src\dex\orca\mod.rs
    src\dex\orca\orca_amm.rs
    src\dex\meteora\mod.rs
    src\dex\meteora\meteora_amm.rs
    src\errors.rs
    src\main.rs
    src\notifier.rs
    src\pricing.rs
    src\rpc.rs
    src\runner.rs
    src\storage.rs

作業ツリーには削除済みの`EXECPLAN.md`、変更済みの`SPEC.md`、削除済みの旧`src/dex/raydium.rs`、`src/dex/orca.rs`、`src/dex/meteora.rs`、新しいDEX別ディレクトリがある。これらはユーザーまたは別作業由来の変更として扱い、明示指示なしに巻き戻さない。

## 作業計画

最初に設定境界を最新SPECへ合わせる。`src/dex/mod.rs`の`DexKind`は`Raydium`、`Orca`、`MeteoraDlmm`を表現できるようにし、設定文字列として`raydium`、`orca`、`meteora_dlmm`を受け付ける。`as_str()`はDiscordとSQLiteで表示するため、`Meteora-DLMM`のように人間が読める名前を返す。`src/config.rs`の`PoolConfig`は、RaydiumとOrcaでは`pool_address`、Meteora-DLMMでは`lb_pair_address`を使えるようにする。単純化のため、内部では監視対象アカウントを返すメソッドを作り、Meteora-DLMMの`lb_pair_address`を`pool_address`相当として扱ってもよいが、設定ファイル上は`SPEC.md`に合わせて`lb_pair_address`を明示できる必要がある。

次にMeteora-DLMMデコーダを完成させる。`src/dex/meteora/meteora_amm.rs`は、LbPairアカウントから`active_id`、`bin_step`、token X/Y mint、reserve X/Y、token decimals、base fee、variable fee、statusを読み取る。現在の途中実装はコンパイル不能の可能性があるため、まず構造体生成を完結させる。正確なアカウントレイアウトが不明な場合は、外部ブログへ依存せず、公式SDKまたは既存crateの型定義やテストデータを確認して、参照した事実を「驚きと発見」に記録する。ネットワークアクセスや依存追加が必要で失敗した場合は、ユーザー承認を得て再実行する。

Meteora-DLMMの価格算出は、初期実装ではactive bin価格を使う。価格式はMeteora-DLMMのbin stepとactive idから得られるbin価格を、token X/Yのmintとdecimalsに基づいて`USDC per SOL`へ正規化する。SOL/USDCの向きがtoken X/Yのどちらかで変わるため、`base_mint`をWSOL、`quote_mint`をUSDCとして照合し、向きが不明ならデコードエラーにする。スリッページ計算を有効にする場合は、`pricing.consider_slippage = true`かつ`pricing.trade_size_usdc`が正の値であることを設定バリデーションで必須にし、BinArray取得とquoteロジックを別関数へ分離する。初期のactive bin価格だけでも監視は成立するが、スリッページ有効時にquoteが未実装なら設定エラーにして誤解を避ける。

`src/pricing.rs`は2 DEX専用の`calculate_spread(dex_a, dex_b)`を残してもよいが、runnerでは`calculate_all_spreads(prices: &[DexPrice]) -> Result<Vec<PriceSpread>, AppError>`のような全組み合わせ関数を使う。Raydium、Orca、Meteora-DLMMの3件が揃った場合、結果は3件になる。`PriceSpread`には`dex_a`、`dex_b`、`absolute_spread`、`spread_bps`、`higher_dex`、`lower_dex`、`comparison_direction`、`fee_adjusted_reference_spread`、`calculated_at`を保持する。`spread_bps`は安いDEX価格を分母にして、`absolute_spread / lower_price * 10000`で計算する。

`src/storage.rs`はRaydium/Orca専用の列からDEXペア汎用へ移行する。既存データを壊さないため、破壊的な`DROP TABLE`は行わない。新しい`price_spreads`には、`dex_a`、`dex_b`、`dex_a_price`、`dex_b_price`、`absolute_spread`、`spread_bps`、`higher_dex`、`lower_dex`、`comparison_direction`、`fee_adjusted_reference_spread`を保存できるようにする。互換性のため既存列を維持する場合でも、新しい全組み合わせ比較を保存できる列または新テーブルを追加する。Meteora-DLMM固有状態は`meteora_dlmm_states`へ保存し、`lb_pair_address`、`active_id`、`bin_step`、`token_x_mint`、`token_y_mint`、`base_fee_bps`、`variable_fee_bps`、`total_fee_bps`、`status`、`liquidity`、`slot`、`observed_at`を含める。

`src/notifier.rs`は3 DEXの価格と3つの価格差を1つのEmbedにまとめる。Discordのフィールド数と文字数制限を超えないよう、Meteora-DLMMの`active_id`、`bin_step`、fee、statusなどの詳細はEmbedに含めない。通常通知Embedには、Raydium価格、Orca価格、Meteora-DLMM価格、Raydium vs Orca、Raydium vs Meteora-DLMM、Orca vs Meteora-DLMM、Higher、Lower、Slot、Observed、Errorsを含める。異常通知Embedには、Component、Severity、DEX、Pool、Retry、Consecutive Errors、Sourceを含める。

`src/runner.rs`は有効な3 DEX設定を読み、必要なプールアカウント、vault、Meteora-DLMMのLbPair、必要ならBinArrayを取得する。1つでも価格取得に失敗した場合は、そのサイクルでは全組み合わせ価格差を計算せず、失敗したDEXの観測失敗、`monitor_errors`、異常通知Embedを記録する。3件すべての`DexPrice`が揃った場合だけ、全組み合わせの`PriceSpread`を保存し、通常通知Embedを送信する。

最後に、`config.example.toml`と`.env.example`を最新SPECへ合わせる。`config.example.toml`にはRaydium、Orca、Meteora-DLMMの3つのpool設定、`discord_embed_enabled = true`、`bot_name`、`environment`、`notification.embed_colors`、`pricing.price_orientation = "usdc_per_sol"`、`trade_size_usdc`の扱いを含める。実値の`.env`と`config.toml`は追跡対象にしない。

## マイルストーン

### マイルストーン1: 設定とDEX種別を3 DEX対応にする

このマイルストーンでは、BotがRaydium、Orca、Meteora-DLMMの3つの有効プール設定を読み、SOL/USDC以外、未設定アドレス、`discord_embed_enabled = false`、スリッページ有効かつ`trade_size_usdc`未設定を起動時に拒否できる状態にする。完了時には、Meteora-DLMMを含む`config.example.toml`を使った設定読み込みテストが成功し、Meteora-DLMMなしの設定は分かりやすい設定エラーになる。

### マイルストーン2: Meteora-DLMMのLbPairデコードとactive bin価格

このマイルストーンでは、`src/dex/meteora/meteora_amm.rs`を完成させ、LbPairから監視に必要な状態を取り出す。完了時には、固定fixtureまたは手作りバッファで`active_id`、`bin_step`、token mint、reserve、手数料、statusを読み取る単体テストが通り、Meteora-DLMMの価格が`USDC per SOL`として`DexPrice`に入る。

### マイルストーン3: 全組み合わせ価格差と保存

このマイルストーンでは、`pricing`と`storage`を3 DEXの全組み合わせに対応させる。完了時には、3つの`DexPrice`から3つの`PriceSpread`が作られ、SQLiteに3行の比較結果と1行のMeteora固有状態が保存される。既存DBに対しても初期化を複数回実行でき、データを消さない。

### マイルストーン4: Discord Embed通知

このマイルストーンでは、通常通知Embedを3 DEX価格と全組み合わせ価格差の表示へ更新し、異常通知EmbedをMeteora-DLMMにも対応させる。完了時には、Webhook送信をモックしたテストで、通常通知のJSONに`Raydium`、`Orca`、`Meteora-DLMM`、3つの比較fieldが含まれ、Meteora固有の`active_id`や`bin_step`がDiscord fieldへ出ないことを確認できる。

### マイルストーン5: runner結合と検証

このマイルストーンでは、`runner`が3 DEXの価格取得、全件成功時の比較、1件失敗時のサイクル失敗扱い、SQLite保存、Discord通知を結合する。完了時には、`cargo fmt`、`cargo check`、`cargo test`が成功し、有効な`.env`と`config.toml`を使った`cargo run`で30秒ごとに3 DEX価格監視が動く。手動確認ではDiscordにEmbed通知が表示され、SQLiteに価格観測、価格差、Meteora固有状態、エラーが保存される。

## 具体的な手順

作業ディレクトリは常に `C:\Users\Owner\Documents\arbitrage_rust` とする。作業前に次を実行し、ユーザー由来の変更を把握する。

    git status --short

現在確認済みの状態では、`SPEC.md`が変更済み、旧DEX単一ファイルが削除済み、新しいDEX別ディレクトリと`PLANS.md`が未追跡である。これらは既存状態として扱い、ユーザーの明示指示なしに削除または復元しない。

まず`src/dex/mod.rs`を更新する。`DexKind::MeteoraDlmm`または既存の`DexKind::Meteora`を`SPEC.md`の表示名に合わせ、`FromStr`で`meteora_dlmm`、必要なら`meteora`も受け付ける。`as_str()`は`Meteora-DLMM`を返す。`PoolAccounts`はRaydium/Orcaのvaultだけでなく、Meteora-DLMMで必要なreserveやBinArrayを渡せる構造に拡張するか、DEX別入力型を分ける。

次に`src/config.rs`を更新する。`PoolConfig`に`lb_pair_address: Option<String>`、`price_orientation: Option<String>`、`auto_discovery: Option<bool>`を追加する。RaydiumとOrcaは`pool_address`を必須にし、Meteora-DLMMは`lb_pair_address`を必須にする。`validate_config`は有効プールにRaydium、Orca、Meteora-DLMMがすべて含まれることを確認する。`pricing.consider_slippage = true`なら`trade_size_usdc`が正の値であることを確認する。

次に`src/dex/meteora/meteora_amm.rs`を完成させる。最低限、`MeteoraDlmmState`、`MeteoraPoolMeta`、`decode_pool_meta`、`decode_price`を定義する。`decode_price`は`PoolConfig`、LbPair account、必要なreserve accountまたはBinArray accountを受け、`DexPrice`と`MeteoraDlmmState`を返す。active bin価格の式、decimals補正、価格の反転条件はテストで固定する。

次に`src/pricing.rs`を更新する。既存の`calculate_spread`を維持しつつ、`calculate_all_spreads(prices: &[DexPrice]) -> Result<Vec<PriceSpread>, AppError>`を追加する。入力が3件未満、ペア不一致、価格0以下の場合は`AppError::Pricing`にする。順序は安定させ、Raydium vs Orca、Raydium vs Meteora-DLMM、Orca vs Meteora-DLMMの順に返す。

次に`src/storage.rs`を更新する。`meteora_dlmm_states`テーブルを追加し、`insert_meteora_dlmm_state`を作る。`price_spreads`はDEXペア汎用にする。既存の`raydium_price`、`orca_price`列があるDBとの互換性が必要な場合は、新しい`price_spread_pairs`テーブルを追加し、runnerは新テーブルへ書く。どちらを選んでも、決定ログへ理由を追記する。

次に`src/notifier.rs`を更新する。単一の`PriceSpread`ではなく、3件の`DexPrice`と複数の`PriceSpread`から通常通知Embedを作る関数を追加する。関数名の例は`build_price_spreads_embed_payload(prices: &[DexPrice], spreads: &[PriceSpread], bot_name: &str, environment: &str, embed_colors: &EmbedColors) -> serde_json::Value`である。異常通知は既存の`build_error_embed_payload`をMeteora-DLMMのDEX名とLbPairアドレスに対応させる。

次に`src/runner.rs`を更新する。Meteora-DLMMのデコードに必要なアカウントを取得し、全DEX価格が揃った場合だけ`calculate_all_spreads`を呼ぶ。全spreadsをSQLiteへ保存し、通常通知Embedは監視サイクルごとに1件送る。価格取得失敗時は失敗したDEXの観測失敗と`monitor_errors`を保存し、`notify_on_error`がtrueなら異常通知Embedを送る。

最後に`config.example.toml`を更新する。Meteora-DLMMの設定例を含め、`lb_pair_address = "未定"`、`price_orientation = "usdc_per_sol"`、`auto_discovery = false`を明記する。`.env.example`がない場合は作成し、`HELIUS_RPC_URL`と`DISCORD_WEBHOOK_URL`だけを書く。

各マイルストーンの終わりに次を実行する。

    cargo fmt
    cargo check
    cargo test

依存関係追加や公式SDK確認でネットワーク制限に当たった場合は、失敗出力を「驚きと発見」に記録し、必要なコマンドをユーザー承認付きで再実行する。

## 検証と受け入れ

単体テストでは、設定読み込み、設定不備、`DexKind`の`meteora_dlmm` parse、Raydiumデコード、Orcaデコード、Meteora-DLMM LbPairデコード、active bin価格計算、スリッページ設定バリデーション、全組み合わせ価格差計算、Discord Embedペイロード生成、SQLite保存処理を検証する。

`cargo test`を実行し、全テストが成功することを受け入れ条件にする。`cargo check`では未使用警告は許容してもよいが、型エラー、未完了式、未解決モジュール、未解決importは残さない。

設定検証の受け入れでは、Meteora-DLMM設定がない`config.toml`で起動すると、Botは監視ループに入らず「enabled SOL/USDC pools must include Raydium, Orca, and Meteora-DLMM」のような設定エラーを表示する。`pricing.consider_slippage = true`かつ`trade_size_usdc`未設定の場合も設定エラーにする。

手動の成功確認では、ユーザーが`.env`と`config.toml`へ実値を入れた後、次を実行する。

    cargo run

期待される観察結果は、起動ログに設定読み込み成功とSQLite初期化成功が表示され、最初の監視サイクルでRaydium価格、Orca価格、Meteora-DLMM価格、3つの価格差、価格差率が計算されることである。Discord Webhookが有効なら、DiscordチャンネルにEmbed通知が届く。Embedには次の情報が表示される。

    title: SOL/USDC Price Spread
    Raydium: <number> USDC
    Orca: <number> USDC
    Meteora-DLMM: <number> USDC
    Raydium vs Orca: <number> USDC / <number> bps
    Raydium vs Meteora-DLMM: <number> USDC / <number> bps
    Orca vs Meteora-DLMM: <number> USDC / <number> bps
    Higher: <DEX name>
    Lower: <DEX name>
    Slot: <slot number or n/a>
    timestamp: <RFC3339 timestamp>
    footer: local | Helius HTTP RPC

SQLiteの受け入れでは、`price_observations`に3 DEXの観測行、DEXペア汎用の価格差テーブルに3行、`meteora_dlmm_states`にMeteora-DLMM固有状態が保存されることを確認する。SQLite CLIがある環境では次のように確認する。

    sqlite3 data/arbitrage_monitor.sqlite "select dex, pair, price from price_observations order by id desc limit 3;"
    sqlite3 data/arbitrage_monitor.sqlite "select dex_a, dex_b, spread_bps from price_spread_pairs order by id desc limit 3;"
    sqlite3 data/arbitrage_monitor.sqlite "select lb_pair_address, active_id, bin_step from meteora_dlmm_states order by id desc limit 1;"

異常系の受け入れでは、Meteora-DLMMのLbPairアドレスを不正なbase58文字列または存在しないアカウントに変更し、`run_once`相当のテストまたは手動実行で価格差計算がスキップされ、`monitor_errors`へエラーが保存され、Discord異常通知Embedまたは標準エラー出力が発生することを確認する。

## 冪等性と復旧

SQLiteスキーマ初期化は`CREATE TABLE IF NOT EXISTS`と不足列追加だけで行い、既存データを消さない。`config.example.toml`と`.env.example`はテンプレートであり、実値を含む`config.toml`と`.env`を上書きしない。テストDBを作る場合は、一時ディレクトリまたは`data/test_*.sqlite`を使い、本番用DBと混ぜない。

RPCやDiscordの一時失敗はBot全体を停止させず、`monitor_errors`に記録して次サイクルで再試行する。設定不備、DBオープン失敗、プール形式不一致、Meteora-DLMMの価格方向不明のように継続しても成功しない可能性が高い問題は、起動時または該当サイクルで明示的にエラー化する。

実装中にMeteora-DLMMのアカウントレイアウトや価格式が現在の仮定と違うことが分かった場合は、先に小さなfixtureテストで失敗を再現し、その後に修正する。方針変更は「決定ログ」に追記し、「進捗」の未完了項目も更新する。

## アーティファクトとメモ

最新`SPEC.md`が求める`config.toml`の概形は次の通りである。

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

Meteora-DLMM固有情報はDiscordに出さず、SQLiteへ保存する。Discordは監視者がすぐ判断するための概要、SQLiteは後から調査するための詳細ログという役割分担にする。

## インターフェースと依存関係

`src/config.rs`には次を定義または更新する。

    pub struct AppConfig {
        pub bot: BotConfig,
        pub database: DatabaseConfig,
        pub pricing: PricingConfig,
        pub notification: NotificationConfig,
        pub pools: Vec<PoolConfig>,
        pub helius_rpc_url: String,
        pub discord_webhook_url: String,
    }

    pub struct PricingConfig {
        pub consider_dex_fee: bool,
        pub consider_slippage: bool,
        pub trade_size_usdc: Option<Decimal>,
        pub price_orientation: String,
    }

    pub struct PoolConfig {
        pub dex: DexKind,
        pub pair: String,
        pub pool_address: Option<String>,
        pub lb_pair_address: Option<String>,
        pub base_mint: String,
        pub quote_mint: String,
        pub price_orientation: Option<String>,
        pub auto_discovery: Option<bool>,
        pub enabled: bool,
    }

`src/dex/mod.rs`には次を定義または更新する。

    pub enum DexKind {
        Raydium,
        Orca,
        MeteoraDlmm,
    }

    pub struct DexPrice {
        pub dex: DexKind,
        pub pair: String,
        pub pool_address: String,
        pub price: Decimal,
        pub fee_adjusted_price: Option<Decimal>,
        pub slippage_adjusted_price: Option<Decimal>,
        pub liquidity: Option<Decimal>,
        pub slot: Option<u64>,
        pub observed_at: DateTime<Utc>,
    }

`src/dex/meteora/meteora_amm.rs`には次を定義する。

    pub struct MeteoraDlmmState {
        pub lb_pair_address: String,
        pub active_id: i32,
        pub bin_step: u16,
        pub token_x_mint: String,
        pub token_y_mint: String,
        pub base_fee_bps: Option<Decimal>,
        pub variable_fee_bps: Option<Decimal>,
        pub total_fee_bps: Option<Decimal>,
        pub status: Option<String>,
        pub liquidity: Option<Decimal>,
        pub slot: Option<u64>,
        pub observed_at: DateTime<Utc>,
    }

    pub fn decode_pool_meta(data: &[u8]) -> Result<MeteoraPoolMeta, AppError>;

    pub fn decode_price(
        pool: &PoolConfig,
        accounts: &MeteoraPoolAccounts,
    ) -> Result<(DexPrice, MeteoraDlmmState), AppError>;

`src/pricing.rs`には次を追加する。

    pub fn calculate_all_spreads(prices: &[DexPrice]) -> Result<Vec<PriceSpread>, AppError>;

`src/storage.rs`には次を追加する。

    pub fn insert_price_spread(&self, spread: &PriceSpread) -> Result<(), AppError>;
    pub fn insert_meteora_dlmm_state(&self, state: &MeteoraDlmmState) -> Result<(), AppError>;

`src/notifier.rs`には次を追加または更新する。

    pub fn build_price_spreads_embed_payload(
        prices: &[DexPrice],
        spreads: &[PriceSpread],
        bot_name: &str,
        environment: &str,
        embed_colors: &EmbedColors,
    ) -> serde_json::Value;

これらの名前は後続実装の安定した目印である。実装中に所有権や非同期境界の都合で引数型を`Arc<Storage>`などへ変更する場合は、このセクションと「決定ログ」を更新する。
