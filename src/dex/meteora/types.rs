use rust_decimal::Decimal;
use chrono::{DateTime, Utc};

use crate::rpc::AccountData;

pub struct MeteoraPoolMeta {
    pub active_id: i32,
    pub bin_step: u16,
    pub token_x_mint: String,
    pub token_y_mint: String,
    pub reserve_x: String,
    pub reserve_y: String,
    pub status: Option<String>,
    pub base_fee_factor: u16,
    pub base_fee_power_factor: u8,
    pub variable_fee_control: u32,
    pub volatility_accumulator: u32,
}

/// SQLiteへ保存するMeteora-DLMM固有状態。
#[derive(Debug, Clone)]
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

/// Meteora-DLMMの両方向quoteをSQLiteへ保存するための方向表現。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeteoraDlmmQuoteDirection {
    UsdcToSol,
    SolToUsdc,
}

impl MeteoraDlmmQuoteDirection {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::UsdcToSol => "USDC -> SOL",
            Self::SolToUsdc => "SOL -> USDC",
        }
    }
}

/// 公式SDK quoteの結果または失敗理由を、監視サイクルと同じDBへ保存するための値。
#[derive(Debug, Clone)]
pub struct MeteoraDlmmQuote {
    pub lb_pair_address: String,
    pub direction: MeteoraDlmmQuoteDirection,
    pub input_mint: String,
    pub output_mint: String,
    pub requested_input_amount: Decimal,
    pub requested_input_amount_raw: u64,
    pub consumed_input_amount: Option<Decimal>,
    pub consumed_input_amount_raw: Option<u64>,
    pub output_amount: Option<Decimal>,
    pub output_amount_raw: Option<u64>,
    pub fee_amount: Option<Decimal>,
    pub fee_amount_raw: Option<u64>,
    pub protocol_fee_amount: Option<Decimal>,
    pub protocol_fee_amount_raw: Option<u64>,
    pub price_impact_bps: Option<Decimal>,
    pub effective_price: Option<Decimal>,
    pub end_price: Option<Decimal>,
    pub bin_array_count: usize,
    pub bin_array_addresses: Vec<String>,
    pub partial_fill: bool,
    pub success: bool,
    pub error_message: Option<String>,
    pub slot: Option<u64>,
    pub observed_at: DateTime<Utc>,
}

/// MeteoraはLbPairに加えてtoken X/Y mint accountも必要なため、入力型をDEX専用に分ける。
#[derive(Debug, Clone)]
pub struct MeteoraPoolAccounts {
    pub lb_pair: AccountData,
    pub token_x_mint: AccountData,
    pub token_y_mint: AccountData,
}

