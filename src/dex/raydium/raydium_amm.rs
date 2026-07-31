use crate::config::PoolConfig;
use crate::dex::{decimal_amount, read_pubkey, read_u64, require_sol_usdc, token_amount, DexKind, DexPrice, PoolAccounts};
use crate::errors::AppError;
use chrono::Utc;
use rust_decimal::Decimal;

const BASE_DECIMAL_OFFSET: usize = 32;
const QUOTE_DECIMAL_OFFSET: usize = 40;
const SWAP_FEE_NUMERATOR_OFFSET: usize = 120;
const SWAP_FEE_DENOMINATOR_OFFSET: usize = 128;
const BASE_VAULT_OFFSET: usize = 336;
const QUOTE_VAULT_OFFSET: usize = 368;
const BASE_MINT_OFFSET: usize = 400;
const QUOTE_MINT_OFFSET: usize = 432;

#[derive(Debug, Clone)]
pub struct RaydiumPoolMeta {
    pub base_vault: String,
    pub quote_vault: String,
    pub base_mint: String,
    pub quote_mint: String,
    pub base_decimals: u8,
    pub quote_decimals: u8,
    pub swap_fee_numerator: u64,
    pub swap_fee_denominator: u64,
}

pub fn decode_pool_meta(data: &[u8]) -> Result<RaydiumPoolMeta, AppError> {
    Ok(RaydiumPoolMeta {
        base_vault: read_pubkey(data, BASE_VAULT_OFFSET, "Raydium base vault")?,
        quote_vault: read_pubkey(data, QUOTE_VAULT_OFFSET, "Raydium quote vault")?,
        base_mint: read_pubkey(data, BASE_MINT_OFFSET, "Raydium base mint")?,
        quote_mint: read_pubkey(data, QUOTE_MINT_OFFSET, "Raydium quote mint")?,
        base_decimals: read_u64(data, BASE_DECIMAL_OFFSET, "Raydium base decimals")? as u8,
        quote_decimals: read_u64(data, QUOTE_DECIMAL_OFFSET, "Raydium quote decimals")? as u8,
        swap_fee_numerator: read_u64(data, SWAP_FEE_NUMERATOR_OFFSET, "Raydium swap fee numerator")?,
        swap_fee_denominator: read_u64(data, SWAP_FEE_DENOMINATOR_OFFSET, "Raydium swap fee denominator")?,
    })
}

pub fn decode_price(config: &PoolConfig, accounts: &PoolAccounts) -> Result<DexPrice, AppError> {
    require_sol_usdc(&config.pair, &config.base_mint, &config.quote_mint)?;
    let meta = decode_pool_meta(&accounts.pool.data)?;
    if meta.base_mint != config.base_mint || meta.quote_mint != config.quote_mint {
        return Err(AppError::Decode(
            "Raydium pool mint order does not match configured SOL/USDC mints".to_string(),
        ));
    }

    let base_vault = accounts
        .base_vault
        .as_ref()
        .ok_or_else(|| AppError::Decode("Raydium base vault account is missing".to_string()))?;
    let quote_vault = accounts
        .quote_vault
        .as_ref()
        .ok_or_else(|| AppError::Decode("Raydium quote vault account is missing".to_string()))?;

    if base_vault.address != meta.base_vault || quote_vault.address != meta.quote_vault {
        return Err(AppError::Decode(
            "Raydium vault accounts do not match pool metadata".to_string(),
        ));
    }

    let base = decimal_amount(token_amount(&base_vault.data)?, meta.base_decimals);
    let quote = decimal_amount(token_amount(&quote_vault.data)?, meta.quote_decimals);
    if base <= Decimal::ZERO {
        return Err(AppError::Decode("Raydium base vault amount is zero".to_string()));
    }

    let price = quote / base;
    let fee_adjusted_price = if meta.swap_fee_denominator > 0 {
        let fee = Decimal::from(meta.swap_fee_numerator) / Decimal::from(meta.swap_fee_denominator);
        Some(price * (Decimal::ONE + fee))
    } else {
        None
    };

    Ok(DexPrice {
        dex: DexKind::Raydium,
        pair: config.pair.clone(),
        pool_address: config.pool_address.clone(),
        price,
        fee_adjusted_price,
        slippage_adjusted_price: None,
        liquidity: Some(quote),
        slot: Some(accounts.pool.slot.max(base_vault.slot).max(quote_vault.slot)),
        observed_at: Utc::now(),
    })
}
