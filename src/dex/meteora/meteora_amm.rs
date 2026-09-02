use crate::config::PoolConfig;
use crate::dex::{DexKind, DexPrice, read_pubkey, require_sol_usdc};
use crate::errors::AppError;
use crate::rpc::AccountData;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

/// LbPair上のoffset
const BASE_FACTOR: usize = 8;
const VARIABLE_FEE_CONTROL_OFFSET: usize = 16;
const STATUS_OFFSET: usize = 32;
const BASE_FEE_POWER_FACTOR: usize = 34;
const VOLATILITY_ACCUMULATOR_OFFSET: usize = 40;
const ACTIVE_ID_OFFSET: usize = 75;
const BIN_STEP_OFFSET: usize = 79;
const TOKEN_X_MINT_OFFSET: usize = 88;
const TOKEN_Y_MINT_OFFSET: usize = 120;
const RESERVE_X_OFFSET: usize = 152;
const RESERVE_Y_OFFSET: usize = 184;

const MINT_DECIMALS_OFFSET: usize = 44;
const MINT_IS_INITIALIZED_OFFSET: usize = 45;
const MINT_ACCOUNT_MIN_LEN: usize = 82;
const FEE_RATE_TO_BPS_DIVISOR: u64 = 100_000;
const MAX_FEE_RATE: u64 = 100_000_000;

/// Meteora-DLMMのLbPairから読み取った監視用メタ情報。
#[derive(Debug, Clone)]
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

/// MeteoraはLbPairに加えてtoken X/Y mint accountも必要なため、入力型をDEX専用に分ける。
#[derive(Debug, Clone)]
pub struct MeteoraPoolAccounts {
    pub lb_pair: AccountData,
    pub token_x_mint: AccountData,
    pub token_y_mint: AccountData,
}

/// LbPairアカウントからactive bin価格計算に必要なフィールドを取り出す。
pub fn decode_pool_meta(data: &[u8]) -> Result<MeteoraPoolMeta, AppError> {
    Ok(MeteoraPoolMeta {
        active_id: read_i32(data, ACTIVE_ID_OFFSET, "Meteora active id")?,
        bin_step: read_u16(data, BIN_STEP_OFFSET, "Meteora bin step")?,
        token_x_mint: read_pubkey(data, TOKEN_X_MINT_OFFSET, "Meteora token X mint")?,
        token_y_mint: read_pubkey(data, TOKEN_Y_MINT_OFFSET, "Meteora token Y mint")?,
        reserve_x: read_pubkey(data, RESERVE_X_OFFSET, "Meteora reserve X")?,
        reserve_y: read_pubkey(data, RESERVE_Y_OFFSET, "Meteora reserve Y")?,
        status: read_u8(data, STATUS_OFFSET, "Meteora status")
            .ok()
            .map(|value| value.to_string()),
        base_fee_factor: read_u16(data, BASE_FACTOR, "Meteora base factor")?,
        base_fee_power_factor: read_u8(
            data,
            BASE_FEE_POWER_FACTOR,
            "Meteora base fee power factor",
        )?,
        variable_fee_control: read_u32(
            data,
            VARIABLE_FEE_CONTROL_OFFSET,
            "Meteora variable fee control",
        )?,
        volatility_accumulator: read_u32(
            data,
            VOLATILITY_ACCUMULATOR_OFFSET,
            "Meteora volatiliry accumulator",
        )?,
    })
}

/// LbPairのactive bin価格をUSDC per SOLへ正規化して返す。
pub fn decode_price(
    pool: &PoolConfig,
    accounts: &MeteoraPoolAccounts,
) -> Result<(DexPrice, MeteoraDlmmState), AppError> {
    require_sol_usdc(&pool.pair, &pool.base_mint, &pool.quote_mint)?;
    let meta = decode_pool_meta(&accounts.lb_pair.data)?;
    ensure_account_address(
        &accounts.token_x_mint,
        &meta.token_x_mint,
        "Meteora token X mint",
    )?;
    ensure_account_address(
        &accounts.token_y_mint,
        &meta.token_y_mint,
        "Meteora token Y mint",
    )?;

    let token_x_decimals = mint_decimals(&accounts.token_x_mint, "Meteora token X mint")?;
    let token_y_decimals = mint_decimals(&accounts.token_y_mint, "Meteora token Y mint")?;
    let raw_price = active_bin_price(
        meta.active_id,
        meta.bin_step,
        token_x_decimals,
        token_y_decimals,
    )?;
    let price = if meta.token_x_mint == pool.base_mint && meta.token_y_mint == pool.quote_mint {
        raw_price
    } else if meta.token_x_mint == pool.quote_mint && meta.token_y_mint == pool.base_mint {
        if raw_price <= Decimal::ZERO {
            return Err(AppError::Decode(
                "Meteora-DLMM raw price is zero".to_string(),
            ));
        }
        Decimal::ONE / raw_price
    } else {
        return Err(AppError::Decode(
            "Meteora-DLMM token mints do not match configured SOL/USDC mints".to_string(),
        ));
    };

    let base_fee = base_fee(
        meta.base_fee_factor,
        meta.bin_step,
        meta.base_fee_power_factor,
    )?;
    let variable_fee = variable_fee(
        meta.variable_fee_control,
        meta.volatility_accumulator,
        meta.bin_step,
    )?;
    let total_fee = (base_fee + variable_fee).min(Decimal::from(MAX_FEE_RATE));
    let base_fee_bps = Some(raw_fee_rate_to_bps(base_fee));
    let variable_fee_bps = Some(raw_fee_rate_to_bps(variable_fee));
    let total_fee_bps = Some(raw_fee_rate_to_bps(total_fee));
    let fee_adjusted_price =
        total_fee_bps.map(|fee_bps| price * (Decimal::ONE + fee_bps / Decimal::from(10_000u64)));
    let observed_at = Utc::now();
    let state = MeteoraDlmmState {
        lb_pair_address: pool.account_address(),
        active_id: meta.active_id,
        bin_step: meta.bin_step,
        token_x_mint: meta.token_x_mint,
        token_y_mint: meta.token_y_mint,
        base_fee_bps,
        variable_fee_bps,
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

pub fn mint_decimals(account: &AccountData, field: &str) -> Result<u8, AppError> {
    if account.data.len() < MINT_ACCOUNT_MIN_LEN {
        return Err(AppError::Decode(format!(
            "{field} account is shorter than SPL Token mint layout"
        )));
    }
    if account.data[MINT_IS_INITIALIZED_OFFSET] == 0 {
        return Err(AppError::Decode(format!(
            "{field} account is not initialized"
        )));
    }
    read_u8(&account.data, MINT_DECIMALS_OFFSET, field)
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

fn base_fee(
    base_factor: u16,
    bin_step: u16,
    base_fee_power_factor: u8,
) -> Result<Decimal, AppError> {
    let base: f64 = f64::from(base_factor) * f64::from(bin_step);
    let powf: f64 = 10_f64 * 10_f64.powf(f64::from(base_fee_power_factor));
    Decimal::from_f64(base * powf).ok_or_else(|| {
        AppError::Decode("failed to convert Meteora-DLMM base fee to Decimal".to_string())
    })
}

fn variable_fee(
    variable_fee_control: u32,
    volatility_accumulator: u32,
    bin_step: u16,
) -> Result<Decimal, AppError> {
    if variable_fee_control == 0 {
        return Ok(Decimal::ZERO);
    }
    let volatility_step: f64 = f64::from(volatility_accumulator) * f64::from(bin_step);
    let variable_fee: f64 =
        (f64::from(variable_fee_control) * volatility_step.powi(2) / 100_000_000_000.0).ceil();
    Decimal::from_f64(variable_fee).ok_or_else(|| {
        AppError::Decode("failed to convert Meteora-DLMM variable fee to Decimal".to_string())
    })
}

fn raw_fee_rate_to_bps(raw_fee_rate: Decimal) -> Decimal {
    raw_fee_rate / Decimal::from(FEE_RATE_TO_BPS_DIVISOR)
}

fn ensure_account_address(
    account: &AccountData,
    expected: &str,
    field: &str,
) -> Result<(), AppError> {
    if account.address != expected {
        return Err(AppError::Decode(format!(
            "{field} account mismatch: expected {expected}, got {}",
            account.address
        )));
    }
    Ok(())
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
    Ok(u16::from_le_bytes(
        bytes.try_into().expect("slice length checked"),
    ))
}

fn read_u32(data: &[u8], offset: usize, field: &str) -> Result<u32, AppError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::Decode(format!("LbPair account is missing {field}")))?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("slice length checked"),
    ))
}

fn read_i32(data: &[u8], offset: usize, field: &str) -> Result<i32, AppError> {
    let bytes = data
        .get(offset..offset + 4)
        .ok_or_else(|| AppError::Decode(format!("LbPair account is missing {field}")))?;
    Ok(i32::from_le_bytes(
        bytes.try_into().expect("slice length checked"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account(data: Vec<u8>) -> AccountData {
        AccountData {
            address: "mint-address".to_string(),
            owner: "owner".to_string(),
            lamports: 0,
            data,
            slot: 1,
        }
    }

    #[test]
    fn mint_decimals_reads_initialized_mint_account() {
        let mut data = vec![0; MINT_ACCOUNT_MIN_LEN];
        data[MINT_DECIMALS_OFFSET] = 6;
        data[MINT_IS_INITIALIZED_OFFSET] = 1;

        assert_eq!(mint_decimals(&account(data), "mint").unwrap(), 6);
    }

    #[test]
    fn mint_decimals_rejects_uninitialized_mint_account() {
        let data = vec![0; MINT_ACCOUNT_MIN_LEN];

        assert!(mint_decimals(&account(data), "mint").is_err());
    }

    #[test]
    fn variable_fee_returns_zero_when_control_is_zero() {
        assert_eq!(variable_fee(0, 10, 25).unwrap(), Decimal::ZERO);
    }

    #[test]
    fn variable_fee_uses_squared_volatility_step_with_ceil() {
        assert_eq!(variable_fee(1, 10, 25).unwrap(), Decimal::ONE);
    }
}
