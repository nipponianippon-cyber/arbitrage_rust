use crate::dex::{DexKind, DexPrice};
use crate::errors::AppError;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;

#[derive(Debug, Clone)]
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

pub fn calculate_spread(dex_a: DexPrice, dex_b: DexPrice) -> Result<PriceSpread, AppError> {
    if dex_a.pair != dex_b.pair {
        return Err(AppError::Pricing(format!(
            "pair mismatch: {} vs {}",
            dex_a.pair, dex_b.pair
        )));
    }
    if dex_a.price <= Decimal::ZERO || dex_b.price <= Decimal::ZERO {
        return Err(AppError::Pricing(
            "prices must be greater than zero".to_string(),
        ));
    }

    let (higher_dex, lower_dex, higher_price, lower_price) = if dex_a.price >= dex_b.price {
        (dex_a.dex, dex_b.dex, dex_a.price, dex_b.price)
    } else {
        (dex_b.dex, dex_a.dex, dex_b.price, dex_a.price)
    };
    let absolute_spread = higher_price - lower_price;
    let spread_bps = absolute_spread / lower_price * Decimal::from(10_000u64);
    let comparison_direction = format!("buy on {lower_dex}, sell on {higher_dex}");
    let fee_adjusted_reference_spread =
        match (dex_a.fee_adjusted_price, dex_b.fee_adjusted_price) {
            (Some(a), Some(b)) if a >= b => Some(a - b),
            (Some(a), Some(b)) => Some(b - a),
            _ => None,
        };

    Ok(PriceSpread {
        pair: dex_a.pair.clone(),
        dex_a,
        dex_b,
        absolute_spread,
        spread_bps,
        higher_dex,
        lower_dex,
        comparison_direction,
        fee_adjusted_reference_spread,
        calculated_at: Utc::now(),
    })
}

pub fn fee_adjusted_buy_price(price: Decimal, fee_rate: Decimal) -> Result<Decimal, AppError> {
    if price <= Decimal::ZERO || fee_rate < Decimal::ZERO {
        return Err(AppError::Pricing(
            "price must be positive and fee_rate must be non-negative".to_string(),
        ));
    }
    Ok(price * (Decimal::ONE + fee_rate))
}

pub fn fee_adjusted_sell_price(price: Decimal, fee_rate: Decimal) -> Result<Decimal, AppError> {
    if price <= Decimal::ZERO || fee_rate < Decimal::ZERO || fee_rate >= Decimal::ONE {
        return Err(AppError::Pricing(
            "price must be positive and fee_rate must be in [0, 1)".to_string(),
        ));
    }
    Ok(price * (Decimal::ONE - fee_rate))
}

pub fn price_impact_bps(trade_size_usdc: Option<Decimal>, liquidity_usdc: Option<Decimal>) -> Option<Decimal> {
    let trade_size = trade_size_usdc?;
    let liquidity = liquidity_usdc?;
    if trade_size <= Decimal::ZERO || liquidity <= Decimal::ZERO {
        return None;
    }
    Some(trade_size / liquidity * Decimal::from(10_000u64))
}
