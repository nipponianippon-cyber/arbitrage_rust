use crate::config::PoolConfig;
use crate::dex::{read_pubkey, require_sol_usdc, DexKind, DexPrice};
use crate::errors::AppError;
use crate::rpc::AccountData;
use chrono::{DateTime, Utc};
use rust_decimal::prelude::FromPrimitive;
use rust_decimal::Decimal;

/// LbPair上のoffset
const STATUS_OFFSET: usize = 32;
const BASE_FEE_BPS_OFFSET: usize = 48;
const VARIABLE_FEE_BPS_OFFSET: usize = 56;
const ACTIVE_ID_OFFSET: usize = 75;
const BIN_STEP_OFFSET: usize = 79;
const TOKEN_X_MINT_OFFSET: usize = 88;
const TOKEN_Y_MINT_OFFSET: usize = 120;
const RESERVE_X_OFFSET: usize = 152;
const RESERVE_Y_OFFSET: usize = 184;

/// token_x, token_y上のoffset
const TOKEN_X_DECIMAL_OFFSET: usize = 44;
const TOKEN_Y_DECIMAL_OFFSET: usize = 44;

/// Meteora-DLMMのLbPairから読み取った監視用メタ情報。
#[derive(Debug, Clone)]
pub struct MeteoraPoolMeta {
    pub active_id: i32,
    pub bin_step: u16,
    pub token_x_mint: String,
    pub token_y_mint: String,
    pub reserve_x: String,
    pub reserve_y: String,
    pub token_x_decimal: u8,
    pub token_y_decimal: u8,
    pub base_fee_bps: Option<Decimal>,
    pub variable_fee_bps: Option<Decimal>,
    pub status: Option<String>,
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

/// MeteoraはLbPair本体だけでactive bin価格を読めるよう、入力型をDEX専用に分ける。
#[derive(Debug, Clone)]
pub struct MeteoraPoolAccounts {
    pub lb_pair: AccountData,
}

/// LbPairアカウントからactive bin価格計算に必要なフィールドを取り出す。
pub fn decode_pool_meta(data: &[u8]) -> Result<MeteoraPoolMeta, AppError> {
    let base_fee_bps = read_u64_optional(data, BASE_FEE_BPS_OFFSET).map(Decimal::from);
    let variable_fee_bps = read_u64_optional(data, VARIABLE_FEE_BPS_OFFSET).map(Decimal::from);
    Ok(MeteoraPoolMeta {
        active_id: read_i32(data, ACTIVE_ID_OFFSET, "Meteora active id")?,
        bin_step: read_u16(data, BIN_STEP_OFFSET, "Meteora bin step")?,
        token_x_mint: read_pubkey(data, TOKEN_X_MINT_OFFSET, "Meteora token X mint")?,
        token_y_mint: read_pubkey(data, TOKEN_Y_MINT_OFFSET, "Meteora token Y mint")?,
        reserve_x: read_pubkey(data, RESERVE_X_OFFSET, "Meteora reserve X")?,
        reserve_y: read_pubkey(data, RESERVE_Y_OFFSET, "Meteora reserve Y")?,
        token_x_decimal: read_u8(data, TOKEN_X_DECIMAL_OFFSET, "Meteora token X decimals")?,
        token_y_decimal: read_u8(data, TOKEN_Y_DECIMAL_OFFSET, "Meteora token Y decimals")?,
        base_fee_bps,
        variable_fee_bps,
        status: read_u8(data, STATUS_OFFSET, "Meteora status")
            .ok()
            .map(|value| value.to_string()),
    })
}

/// LbPairのactive bin価格をUSDC per SOLへ正規化して返す。
pub fn decode_price(
    pool: &PoolConfig,
    accounts: &MeteoraPoolAccounts,
) -> Result<(DexPrice, MeteoraDlmmState), AppError> {
    require_sol_usdc(&pool.pair, &pool.base_mint, &pool.quote_mint)?;
    let meta = decode_pool_meta(&accounts.lb_pair.data)?;
    let raw_price = active_bin_price(meta.active_id, meta.bin_step, meta.token_x_decimal, meta.token_y_decimal)?;
    let price = if meta.token_x_mint == pool.base_mint && meta.token_y_mint == pool.quote_mint {
        raw_price
    } else if meta.token_x_mint == pool.quote_mint && meta.token_y_mint == pool.base_mint {
        if raw_price <= Decimal::ZERO {
            return Err(AppError::Decode("Meteora-DLMM raw price is zero".to_string()));
        }
        Decimal::ONE / raw_price
    } else {
        return Err(AppError::Decode(
            "Meteora-DLMM token mints do not match configured SOL/USDC mints".to_string(),
        ));
    };

    let total_fee_bps = match (meta.base_fee_bps, meta.variable_fee_bps) {
        (Some(base), Some(variable)) => Some(base + variable),
        (Some(base), None) => Some(base),
        (None, Some(variable)) => Some(variable),
        (None, None) => None,
    };
    let fee_adjusted_price = total_fee_bps.map(|fee_bps| price * (Decimal::ONE + fee_bps / Decimal::from(10_000u64)));
    let observed_at = Utc::now();
    let state = MeteoraDlmmState {
        lb_pair_address: pool.account_address(),
        active_id: meta.active_id,
        bin_step: meta.bin_step,
        token_x_mint: meta.token_x_mint,
        token_y_mint: meta.token_y_mint,
        base_fee_bps: meta.base_fee_bps,
        variable_fee_bps: meta.variable_fee_bps,
        total_fee_bps,
        status: meta.status,
        liquidity: None,
        slot: Some(accounts.lb_pair.slot),
        observed_at,
    };

    Ok((
        DexPrice {
            dex: DexKind::MeteoraDlmm,
            pair: pool.pair.clone(),
            pool_address: pool.account_address(),
            price,
            fee_adjusted_price,
            slippage_adjusted_price: None,
            liquidity: None,
            slot: Some(accounts.lb_pair.slot),
            observed_at,
        },
        state,
    ))
}

fn active_bin_price(
    active_id: i32,
    bin_step: u16,
    token_x_decimals: u8,
    token_y_decimals: u8,
) -> Result<Decimal, AppError> {
    // DLMMのbin価格はSDK検証前の最小実装。fixture検証で必要なら補正する。
    let step = 1.0 + f64::from(bin_step) / 10_000.0;
    let decimal_adjustment = 10_f64.powi(i32::from(token_x_decimals) - i32::from(token_y_decimals));
    Decimal::from_f64(step.powi(active_id) * decimal_adjustment).ok_or_else(|| {
        AppError::Decode("failed to convert Meteora-DLMM active bin price to Decimal".to_string())
    })
}

fn read_u8(data: &[u8], offset: usize, field: &str) -> Result<u8, AppError> {
    data.get(offset)
        .copied()
        .ok_or_else(|| AppError::Decode(format!("LbPair account is missing {field}")))
}

fn read_u16(data: &[u8], offset: usize, field: &str) -> Result<u16, AppError> {
    let bytes = data
        .get(offset..offset + 2)
        .ok_or_else(|| AppError::Decode(format!("LbPair account is missing {field}")))?;
    Ok(u16::from_le_bytes(bytes.try_into().expect("slice length checked")))
}

fn read_i32(data: &[u8], offset: usize, field: &str) -> Result<i32, AppError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::Decode(format!("LbPair account is missing {field}")))?;
    Ok(i32::from_le_bytes(bytes.try_into().expect("slice length checked")))
}

fn read_u64_optional(data: &[u8], offset: usize) -> Option<u64> {
    let bytes = data.get(offset..offset + 8)?;
    Some(u64::from_le_bytes(bytes.try_into().ok()?))
}
