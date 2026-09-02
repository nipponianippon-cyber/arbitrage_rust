use crate::config::PoolConfig;
use crate::dex::{DexKind, DexPrice, PoolAccounts, read_pubkey, read_u64, require_sol_usdc};
use crate::errors::AppError;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

const FEE_RATE_OFFSET: usize = 45;
const SQRT_PRICE_OFFSET: usize = 65;
const TOKEN_MINT_A_OFFSET: usize = 101;
const TOKEN_VAULT_A_OFFSET: usize = 133;
const TOKEN_MINT_B_OFFSET: usize = 181;
const TOKEN_VAULT_B_OFFSET: usize = 213;
const Q64: f64 = 18_446_744_073_709_551_616.0;

#[derive(Debug, Clone)]
pub struct OrcaPoolMeta {
    pub token_mint_a: String,
    pub token_vault_a: String,
    pub token_mint_b: String,
    pub token_vault_b: String,
    pub fee_rate: u16,
    pub sqrt_price: u128,
}

pub fn decode_pool_meta(data: &[u8]) -> Result<OrcaPoolMeta, AppError> {
    let sqrt_bytes = data
        .get(SQRT_PRICE_OFFSET..SQRT_PRICE_OFFSET + 16)
        .ok_or_else(|| AppError::Decode("Whirlpool account is missing sqrt_price".to_string()))?;
    Ok(OrcaPoolMeta {
        token_mint_a: read_pubkey(data, TOKEN_MINT_A_OFFSET, "Whirlpool token mint A")?,
        token_vault_a: read_pubkey(data, TOKEN_VAULT_A_OFFSET, "Whirlpool token vault A")?,
        token_mint_b: read_pubkey(data, TOKEN_MINT_B_OFFSET, "Whirlpool token mint B")?,
        token_vault_b: read_pubkey(data, TOKEN_VAULT_B_OFFSET, "Whirlpool token vault B")?,
        fee_rate: read_u64(data, FEE_RATE_OFFSET, "Whirlpool fee rate")? as u16,
        sqrt_price: u128::from_le_bytes(sqrt_bytes.try_into().expect("slice length checked")),
    })
}

pub fn decode_price(config: &PoolConfig, accounts: &PoolAccounts) -> Result<DexPrice, AppError> {
    require_sol_usdc(&config.pair, &config.base_mint, &config.quote_mint)?;
    let meta = decode_pool_meta(&accounts.pool.data)?;

    let raw_price = whirlpool_price(meta.sqrt_price)?;
    let price = if meta.token_mint_a == config.base_mint && meta.token_mint_b == config.quote_mint {
        raw_price
    } else if meta.token_mint_a == config.quote_mint && meta.token_mint_b == config.base_mint {
        if raw_price <= Decimal::ZERO {
            return Err(AppError::Decode("Whirlpool raw price is zero".to_string()));
        }
        Decimal::ONE / raw_price
    } else {
        return Err(AppError::Decode(
            "Whirlpool token mints do not match configured SOL/USDC mints".to_string(),
        ));
    };

    if let Some(base_vault) = &accounts.base_vault {
        let expected = if meta.token_mint_a == config.base_mint {
            &meta.token_vault_a
        } else {
            &meta.token_vault_b
        };
        if &base_vault.address != expected {
            return Err(AppError::Decode(
                "Whirlpool base vault account does not match pool metadata".to_string(),
            ));
        }
    }

    let fee = Decimal::from(meta.fee_rate) / Decimal::from(1_000_000u64);
    Ok(DexPrice {
        dex: DexKind::Orca,
        pair: config.pair.clone(),
        pool_address: config.pool_address.clone(),
        price,
        fee_adjusted_price: Some(price * (Decimal::ONE + fee)),
        slippage_adjusted_price: None,
        liquidity: None,
        slot: Some(accounts.pool.slot),
        observed_at: Utc::now(),
    })
}

fn whirlpool_price(sqrt_price: u128) -> Result<Decimal, AppError> {
    let sqrt = sqrt_price as f64 / Q64;
    let price = sqrt * sqrt;
    Decimal::from_f64(price).ok_or_else(|| {
        AppError::Decode("failed to convert Whirlpool sqrt_price to decimal".to_string())
    })
}

pub fn vault_addresses_for_config(
    config: &PoolConfig,
    meta: &OrcaPoolMeta,
) -> Result<(String, String), AppError> {
    if meta.token_mint_a == config.base_mint && meta.token_mint_b == config.quote_mint {
        Ok((meta.token_vault_a.clone(), meta.token_vault_b.clone()))
    } else if meta.token_mint_a == config.quote_mint && meta.token_mint_b == config.base_mint {
        Ok((meta.token_vault_b.clone(), meta.token_vault_a.clone()))
    } else {
        Err(AppError::Decode(
            "Whirlpool token mints do not match configured SOL/USDC mints".to_string(),
        ))
    }
}
