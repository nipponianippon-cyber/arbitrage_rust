use crate::{config::PoolConfig, errors::AppError};
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 経済条件は運用者が明示する。セクション省略時は候補判定を無効にする。
#[derive(Debug, Clone, Deserialize)]
pub struct ArbitrageConfig {
    pub enabled: bool,
    pub min_input_usdc: Decimal,
    pub max_input_usdc: Decimal,
    pub min_profit_usdc: Decimal,
    pub notification_profit_change_usdc: Decimal,
    pub input_step_raw: u64,
    pub coarse_points: usize,
    pub max_route_quotes: usize,
    pub max_price_impact_bps: u16,
    #[serde(default = "default_slot_difference")]
    pub max_slot_difference: u64,
    pub costs: FixedCosts,
    pub accounts: Vec<QuoteAccountsConfig>,
}

/// 固定費はすべてUSDC建て。SOL建てネットワーク費用も設定時点で換算する。
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FixedCosts {
    pub base_fee_usdc: Decimal,
    pub priority_fee_usdc: Decimal,
    pub jito_tip_usdc: Decimal,
    pub flashloan_fee_usdc: Decimal,
    pub slippage_buffer_usdc: Decimal,
}

/// 対象poolと、そのquoteに使用してよいtick/bin配列を明示する。
#[derive(Debug, Clone, Deserialize)]
pub struct QuoteAccountsConfig {
    pub pool_address: String,
    pub program_id: String,
    #[serde(default)]
    pub arrays: Vec<String>,
}

fn default_slot_difference() -> u64 { 2 }

/// 小数6桁より細かい設定を丸めず拒否し、利益比較の単位を統一する。
pub fn usdc_raw(value: Decimal) -> Result<u64, AppError> {
    let raw = value.checked_mul(Decimal::from(1_000_000u64))
        .ok_or_else(|| AppError::Config("USDC amount overflow".into()))?;
    if value.is_sign_negative() || !raw.fract().is_zero() {
        return Err(AppError::Config("USDC amounts must be nonnegative with at most 6 decimals".into()));
    }
    raw.to_u64().ok_or_else(|| AppError::Config("USDC raw amount exceeds u64".into()))
}

impl FixedCosts {
    /// quote出力にはDEX手数料を反映済みなので、ここでは二重に引かない。
    pub fn total_raw(&self) -> Result<u64, AppError> {
        [self.base_fee_usdc, self.priority_fee_usdc, self.jito_tip_usdc,
            self.flashloan_fee_usdc, self.slippage_buffer_usdc]
            .into_iter().try_fold(0u64, |sum, value| {
                sum.checked_add(usdc_raw(value)?)
                    .ok_or_else(|| AppError::Config("fixed cost overflow".into()))
            })
    }
}

impl ArbitrageConfig {
    /// 探索量と依存アカウント数にも上限を設け、1サイクルの処理量を制限する。
    pub fn validate(&self, pools: &[PoolConfig]) -> Result<(), AppError> {
        if !self.enabled { return Ok(()); }
        let min = usdc_raw(self.min_input_usdc)?;
        let max = usdc_raw(self.max_input_usdc)?;
        usdc_raw(self.min_profit_usdc)?;
        if min == 0 || max < min || self.input_step_raw == 0
            || !(3..=128).contains(&self.coarse_points)
            || self.max_route_quotes < self.coarse_points + 4
            || self.max_route_quotes > 4096 || self.max_price_impact_bps >= 10_000
            || usdc_raw(self.notification_profit_change_usdc)? == 0 {
            return Err(AppError::Config("invalid arbitrage input range, search budget, impact limit or notification threshold".into()));
        }
        self.costs.total_raw()?;
        let enabled: Vec<_> = pools.iter().filter(|p| p.enabled).collect();
        if enabled.len() != 3 || self.accounts.len() != 3 {
            return Err(AppError::Config("arbitrage requires exactly one pool per DEX and three account configurations".into()));
        }
        let mut seen = HashSet::new();
        for entry in &self.accounts {
            if !seen.insert(entry.pool_address.clone()) {
                return Err(AppError::Config("duplicate arbitrage pool".into()));
            }
            let pool = enabled.iter().find(|p| p.account_address() == entry.pool_address)
                .ok_or_else(|| AppError::Config("arbitrage account configuration does not match an enabled pool".into()))?;
            for address in std::iter::once(&entry.pool_address)
                .chain(std::iter::once(&entry.program_id)).chain(entry.arrays.iter()) {
                if bs58::decode(address).into_vec().map(|v| v.len()).unwrap_or(0) != 32 {
                    return Err(AppError::Config("arbitrage accounts must be 32-byte Solana addresses".into()));
                }
            }
            if pool.dex != crate::dex::DexKind::Raydium && entry.arrays.is_empty() {
                return Err(AppError::Config("Orca and Meteora require explicit tick/bin array addresses".into()));
            }
            if entry.arrays.len() > 16 || entry.arrays.iter().collect::<HashSet<_>>().len() != entry.arrays.len() {
                return Err(AppError::Config("at most 16 distinct arrays per pool are supported".into()));
            }
        }
        Ok(())
    }
}
