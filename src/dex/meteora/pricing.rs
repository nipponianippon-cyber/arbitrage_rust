use crate::config::PoolConfig;
use crate::dex::{DexKind, DexPrice, require_sol_usdc};
use crate::errors::AppError;

use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;
use chrono::Utc;

use super::types::{MeteoraDlmmState, MeteoraPoolAccounts};
use super::decoder::{decode_pool_meta, ensure_account_address, mint_decimals, FEE_RATE_TO_BPS_DIVISOR, MAX_FEE_RATE};

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
