use super::math::*;
use crate::{dex::DexKind, errors::AppError};
use num_bigint::BigUint;
use serde::Serialize;

/// 入力をすべて消費できた場合だけ候補判定に使えるlocal quote。
#[derive(Debug, Clone, Serialize)]
pub struct LocalQuote {
    pub input_raw: u64,
    pub consumed_raw: u64,
    pub output_raw: u64,
    pub fee_raw: u64,
    pub complete: bool,
    pub boundary_inputs: Vec<u64>,
}

#[derive(Debug, Clone)]
pub(super) struct Tick {
    pub index: i32,
    pub liquidity_net: i128,
}

#[derive(Debug, Clone)]
pub(super) struct Bin {
    pub id: i32,
    pub x: u64,
    pub y: u64,
    pub price: u128,
}

#[derive(Debug, Clone)]
pub(super) struct VariableFee {
    pub base: u128,
    pub control: u32,
    pub bin_step: u16,
    pub reference: u32,
    pub index_reference: i32,
    pub max_accumulator: u32,
}

#[derive(Debug, Clone)]
pub(super) enum PoolMath {
    Raydium {
        sol: u64,
        usdc: u64,
        fee_n: u64,
        fee_d: u64,
    },
    Orca {
        sol_is_a: bool,
        sqrt: u128,
        liquidity: u128,
        current_tick: i32,
        fee: u16,
        lower_tick: i32,
        upper_tick: i32,
        ticks: Vec<Tick>,
    },
    Meteora {
        sol_is_x: bool,
        active_id: i32,
        bins: Vec<Bin>,
        fee: VariableFee,
    },
}

/// 検証済みsnapshotから構成したpool。quoteのたびに元の状態から計算する。
#[derive(Debug, Clone)]
pub struct LocalPool {
    pub dex: DexKind,
    pub address: String,
    pub(super) math: PoolMath,
}

impl LocalPool {
    /// buy=true: USDC→SOL、false: SOL→USDC UI価格をquoteに流用しない。
    pub fn quote(&self, input: u64, buy: bool) -> Result<LocalQuote, AppError> {
        if input == 0 { return Err(error("zero quote input")); }
        match &self.math {
            PoolMath::Raydium { sol, usdc, fee_n, fee_d } => {
                let (reserve_in, reserve_out) = if buy { (*usdc, *sol) } else { (*sol, *usdc) };
                let fee = to_u64(mul_div(input as u128, *fee_n as u128, *fee_d as u128, true)?)?;
                let net = input.checked_sub(fee).ok_or_else(|| error("fee exceeds input"))?;
                let output = mul_div(net as u128, reserve_out as u128,
                    reserve_in as u128 + net as u128, false)?;
                Ok(LocalQuote { input_raw: input, consumed_raw: input, output_raw: to_u64(output)?,
                    fee_raw: fee, complete: true, boundary_inputs: vec![] })
            }
            PoolMath::Orca { sol_is_a, sqrt, liquidity, current_tick, fee,
                lower_tick, upper_tick, ticks } => {
                let a_to_b = buy != *sol_is_a;
                let mut targets: Vec<_> = ticks.iter().filter(|t| {
                    if a_to_b { t.index <= *current_tick } else { t.index > *current_tick }
                }).map(|t| (t.index, Some(t.liquidity_net))).collect();
                if a_to_b { targets.reverse(); }
                let edge = if a_to_b { *lower_tick } else { *upper_tick };
                // 未取得配列への越境を避ける。配列末端の先の流動性は推測しない。
                targets.push((edge, None));
                let (mut p, mut l, mut remaining, mut output, mut fees) = (*sqrt, *liquidity, input, 0u128, 0u128);
                let mut boundaries = Vec::new();
                for (index, liquidity_net) in targets {
                    if remaining == 0 { break; }
                    let target = tick_sqrt(index)?;
                    if (a_to_b && target > p) || (!a_to_b && target < p) {
                        return Err(error("tick sequence is inconsistent with pool price"));
                    }
                    let (lo, hi) = if a_to_b { (target, p) } else { (p, target) };
                    let needed = if a_to_b { delta_a(l, lo, hi, true)? } else { delta_b(l, lo, hi, true)? };
                    let available = mul_div(remaining as u128, 1_000_000 - *fee as u128, 1_000_000, false)?;
                    let reaches = available >= needed;
                    let next = if reaches { target } else { next_sqrt(l, p, available, a_to_b)? };
                    let (step_lo, step_hi) = if a_to_b { (next, p) } else { (p, next) };
                    let used = if reaches { needed } else if a_to_b {
                        delta_a(l, step_lo, step_hi, true)?
                    } else { delta_b(l, step_lo, step_hi, true)? };
                    let fee_amount = if reaches {
                        mul_div(used, *fee as u128, 1_000_000 - *fee as u128, true)?
                    } else { (remaining as u128).checked_sub(used)
                        .ok_or_else(|| error("Whirlpool input rounding exceeds remaining amount"))? };
                    let produced = if a_to_b { delta_b(l, step_lo, step_hi, false)? }
                        else { delta_a(l, step_lo, step_hi, false)? };
                    remaining = remaining.checked_sub(to_u64(used + fee_amount)?)
                        .ok_or_else(|| error("Whirlpool consumed more than input"))?;
                    output = output.checked_add(produced).ok_or_else(|| error("output overflow"))?;
                    fees += fee_amount;
                    p = next;
                    if reaches {
                        boundaries.push(input - remaining);
                        if let Some(net) = liquidity_net {
                            // 左向きにtickを跨ぐときはliquidity_netを逆符号で適用する。
                            let magnitude = net.unsigned_abs();
                            l = if (net >= 0) != a_to_b { l.checked_add(magnitude) }
                                else { l.checked_sub(magnitude) }
                                .ok_or_else(|| error("invalid liquidity at tick crossing"))?;
                        } else { break; }
                    }
                }
                Ok(LocalQuote { input_raw: input, consumed_raw: input - remaining,
                    output_raw: to_u64(output)?, fee_raw: to_u64(fees)?, complete: remaining == 0,
                    boundary_inputs: boundaries })
            }
            PoolMath::Meteora { sol_is_x, active_id, bins, fee } => {
                let x_to_y = buy != *sol_is_x;
                let mut ordered: Vec<_> = bins.iter().filter(|b| {
                    if x_to_y { b.id <= *active_id } else { b.id >= *active_id }
                }).collect();
                if x_to_y { ordered.reverse(); }
                let (mut remaining, mut output, mut fees) = (input, 0u128, 0u128);
                let mut boundaries = Vec::new();
                for bin in ordered {
                    if remaining == 0 { break; }
                    let capacity = if x_to_y { bin.y } else { bin.x };
                    if capacity == 0 { continue; }
                    let distance = (i64::from(bin.id) - i64::from(fee.index_reference)).unsigned_abs();
                    let accumulator = (u64::from(fee.reference) + distance * 10_000)
                        .min(u64::from(fee.max_accumulator));
                    let v = accumulator as u128 * fee.bin_step as u128;
                    let variable = mul_div(fee.control as u128, v * v, 100_000_000_000, true)?;
                    let rate = (fee.base + variable).min(100_000_000);
                    let needed = if x_to_y { mul_div(capacity as u128, Q64, bin.price, true)? }
                        else { mul_div(capacity as u128, bin.price, Q64, true)? };
                    let full_fee = mul_div(needed, rate, 1_000_000_000 - rate, true)?;
                    let full_input = needed.checked_add(full_fee)
                        .ok_or_else(|| error("DLMM input amount overflow"))?;
                    let (used, charged, produced) = if full_input <= remaining as u128 {
                        (needed, full_fee, capacity as u128)
                    } else {
                        let charged = mul_div(remaining as u128, rate, 1_000_000_000, true)?;
                        let used = remaining as u128 - charged;
                        let produced = if x_to_y { mul_div(used, bin.price, Q64, false)? }
                            else { mul_div(used, Q64, bin.price, false)? };
                        (used, charged, produced.min(capacity as u128))
                    };
                    remaining -= to_u64(used + charged)?;
                    output = output.checked_add(produced).ok_or_else(|| error("output overflow"))?;
                    fees += charged;
                    if produced == capacity as u128 { boundaries.push(input - remaining); }
                }
                Ok(LocalQuote { input_raw: input, consumed_raw: input - remaining,
                    output_raw: to_u64(output)?, fee_raw: to_u64(fees)?, complete: remaining == 0,
                    boundary_inputs: boundaries })
            }
        }
    }

    // fee適用前の限界価格を有理数で返し、f64やDecimalの丸めで候補を選ばない。
    pub(super) fn rate(&self, buy: bool) -> Result<(BigUint, BigUint), AppError> {
        let (n, d) = match &self.math {
            PoolMath::Raydium { sol, usdc, .. } => (BigUint::from(*sol), BigUint::from(*usdc)),
            PoolMath::Orca { sol_is_a, sqrt, .. } => {
                let sq = BigUint::from(*sqrt) * *sqrt;
                let q = BigUint::from(Q64) * Q64;
                if *sol_is_a { (q, sq) } else { (sq, q) }
            }
            PoolMath::Meteora { sol_is_x, active_id, bins, .. } => {
                let p = bins.iter().find(|b| b.id == *active_id)
                    .ok_or_else(|| error("active bin is missing"))?.price;
                if p == 0 { return Err(error("active bin price is zero")); }
                if *sol_is_x { (BigUint::from(Q64), BigUint::from(p)) }
                    else { (BigUint::from(p), BigUint::from(Q64)) }
            }
        };
        Ok(if buy { (n, d) } else { (d, n) })
    }

    pub(super) fn within_impact(&self, q: &LocalQuote, buy: bool, limit: u16) -> Result<bool, AppError> {
        let (n, d) = self.rate(buy)?;
        // DEX feeも含む保守的な実効価格悪化率。取得済み範囲での完全約定だけを許可する。
        Ok(q.complete && q.output_raw > 0 &&
            BigUint::from(q.output_raw) * d * 10_000u32 >=
            BigUint::from(q.input_raw) * n * (10_000u32 - u32::from(limit)))
    }
}
