use crate::config::PoolConfig;
use crate::dex::{DexKind, DexPrice, read_pubkey, read_u64, require_sol_usdc};
use crate::errors::AppError;
use crate::rpc::AccountData;
use chrono::Utc;
use rust_decimal::Decimal;
use rust_decimal::prelude::FromPrimitive;

const FEE_RATE_OFFSET: usize = 45;
const SQRT_PRICE_OFFSET: usize = 65;
const TOKEN_MINT_A_OFFSET: usize = 101;
const TOKEN_VAULT_A_OFFSET: usize = 133;
const TOKEN_MINT_B_OFFSET: usize = 181;
const TOKEN_VAULT_B_OFFSET: usize = 213;
const MINT_DECIMALS_OFFSET: usize = 44;
const MINT_IS_INITIALIZED_OFFSET: usize = 45;
const MINT_ACCOUNT_MIN_LEN: usize = 82;
const Q64: u128 = 1_u128 << 64;

#[derive(Debug, Clone)]
pub struct OrcaPoolMeta {
    pub token_mint_a: String,
    pub token_vault_a: String,
    pub token_mint_b: String,
    pub token_vault_b: String,
    pub fee_rate: u16,
    pub sqrt_price: u128,
}

/// Whirlpool価格計算に必要なpool本体と依存アカウント。
#[derive(Debug, Clone)]
pub struct OrcaPoolAccounts {
    pub pool: AccountData,
    pub token_mint_a: AccountData,
    pub token_mint_b: AccountData,
    pub base_vault: Option<AccountData>,
    pub quote_vault: Option<AccountData>,
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

pub fn decode_price(
    config: &PoolConfig,
    accounts: &OrcaPoolAccounts,
) -> Result<DexPrice, AppError> {
    require_sol_usdc(&config.pair, &config.base_mint, &config.quote_mint)?;
    let meta = decode_pool_meta(&accounts.pool.data)?;

    ensure_account_address(
        &accounts.token_mint_a,
        &meta.token_mint_a,
        "Whirlpool token mint A",
    )?;
    ensure_account_address(
        &accounts.token_mint_b,
        &meta.token_mint_b,
        "Whirlpool token mint B",
    )?;

    let token_a_decimals = mint_decimals(&accounts.token_mint_a, "Whirlpool token mint A")?;
    let token_b_decimals = mint_decimals(&accounts.token_mint_b, "Whirlpool token mint B")?;
    let raw_price = whirlpool_price(meta.sqrt_price, token_a_decimals, token_b_decimals)?;
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
    if let Some(quote_vault) = &accounts.quote_vault {
        let expected = if meta.token_mint_b == config.quote_mint {
            &meta.token_vault_b
        } else {
            &meta.token_vault_a
        };
        if &quote_vault.address != expected {
            return Err(AppError::Decode(
                "Whirlpool quote vault account does not match pool metadata".to_string(),
            ));
        }
    }

    let fee = Decimal::from(meta.fee_rate) / Decimal::from(1_000_000u64);
    let mut slot = accounts
        .pool
        .slot
        .max(accounts.token_mint_a.slot)
        .max(accounts.token_mint_b.slot);
    if let Some(base_vault) = &accounts.base_vault {
        slot = slot.max(base_vault.slot);
    }
    if let Some(quote_vault) = &accounts.quote_vault {
        slot = slot.max(quote_vault.slot);
    }

    Ok(DexPrice {
        dex: DexKind::Orca,
        pair: config.pair.clone(),
        pool_address: config.pool_address.clone(),
        price,
        fee_adjusted_price: Some(price * (Decimal::ONE + fee)),
        slippage_adjusted_price: None,
        liquidity: None,
        slot: Some(slot),
        observed_at: Utc::now(),
    })
}

fn whirlpool_price(
    sqrt_price: u128,
    token_a_decimals: u8,
    token_b_decimals: u8,
) -> Result<Decimal, AppError> {
    // Q64.64のsqrt_priceをDecimalへ変換してから二乗し、token A/Bのdecimal差を補正する。
    // f64を使わないことで、価格比較に使う値の丸め誤差を小さく保つ。
    let sqrt = decimal_from_u128(sqrt_price, "Whirlpool sqrt_price")?
        / decimal_from_u128(Q64, "Q64 scale")?;
    let raw_price = sqrt * sqrt;
    Ok(raw_price * decimal_adjustment(token_a_decimals, token_b_decimals))
}

fn decimal_adjustment(token_a_decimals: u8, token_b_decimals: u8) -> Decimal {
    let mut adjustment = Decimal::ONE;
    if token_a_decimals >= token_b_decimals {
        for _ in 0..(token_a_decimals - token_b_decimals) {
            adjustment *= Decimal::from(10u64);
        }
    } else {
        for _ in 0..(token_b_decimals - token_a_decimals) {
            adjustment /= Decimal::from(10u64);
        }
    }
    adjustment
}

fn decimal_from_u128(value: u128, field: &str) -> Result<Decimal, AppError> {
    Decimal::from_u128(value).ok_or_else(|| {
        AppError::Decode(format!(
            "failed to convert {field} to Decimal without precision loss"
        ))
    })
}

fn mint_decimals(account: &AccountData, field: &str) -> Result<u8, AppError> {
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
    account
        .data
        .get(MINT_DECIMALS_OFFSET)
        .copied()
        .ok_or_else(|| AppError::Decode(format!("{field} account is missing decimals")))
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

#[cfg(test)]
mod tests {
    use super::*;

    const WSOL: &str = "So11111111111111111111111111111111111111112";
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    fn pool_config(pool_address: String) -> PoolConfig {
        PoolConfig {
            dex: DexKind::Orca,
            pair: "SOL/USDC".to_string(),
            pool_address,
            lb_pair_address: None,
            base_mint: WSOL.to_string(),
            quote_mint: USDC.to_string(),
            price_orientation: None,
            auto_discovery: None,
            enabled: true,
        }
    }

    fn account(address: String, data: Vec<u8>) -> AccountData {
        AccountData {
            address,
            owner: "owner".to_string(),
            lamports: 0,
            data,
            slot: 1,
        }
    }

    fn write_u64(data: &mut [u8], offset: usize, value: u64) {
        data[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn write_u128(data: &mut [u8], offset: usize, value: u128) {
        data[offset..offset + 16].copy_from_slice(&value.to_le_bytes());
    }

    fn write_pubkey(data: &mut [u8], offset: usize, value: &str) {
        let decoded = bs58::decode(value).into_vec().unwrap();
        data[offset..offset + 32].copy_from_slice(&decoded);
    }

    fn mint_data(decimals: u8) -> Vec<u8> {
        let mut data = vec![0; MINT_ACCOUNT_MIN_LEN];
        data[MINT_DECIMALS_OFFSET] = decimals;
        data[MINT_IS_INITIALIZED_OFFSET] = 1;
        data
    }

    fn whirlpool_data(
        token_mint_a: &str,
        token_vault_a: &str,
        token_mint_b: &str,
        token_vault_b: &str,
        sqrt_price: u128,
    ) -> Vec<u8> {
        let mut data = vec![0; TOKEN_VAULT_B_OFFSET + 32];
        write_u64(&mut data, FEE_RATE_OFFSET, 2_000);
        write_u128(&mut data, SQRT_PRICE_OFFSET, sqrt_price);
        write_pubkey(&mut data, TOKEN_MINT_A_OFFSET, token_mint_a);
        write_pubkey(&mut data, TOKEN_VAULT_A_OFFSET, token_vault_a);
        write_pubkey(&mut data, TOKEN_MINT_B_OFFSET, token_mint_b);
        write_pubkey(&mut data, TOKEN_VAULT_B_OFFSET, token_vault_b);
        data
    }

    #[test]
    fn whirlpool_price_applies_token_decimal_adjustment() {
        let price = whirlpool_price(Q64 / 2, 9, 6).unwrap();

        assert_eq!(price, Decimal::from(250u64));
    }

    #[test]
    fn decode_price_inverts_when_whirlpool_mints_are_reversed() {
        let pool_address = bs58::encode([1; 32]).into_string();
        let token_vault_a = bs58::encode([2; 32]).into_string();
        let token_vault_b = bs58::encode([3; 32]).into_string();
        let accounts = OrcaPoolAccounts {
            pool: account(
                pool_address.clone(),
                whirlpool_data(USDC, &token_vault_a, WSOL, &token_vault_b, Q64 * 2),
            ),
            token_mint_a: account(USDC.to_string(), mint_data(6)),
            token_mint_b: account(WSOL.to_string(), mint_data(9)),
            base_vault: Some(account(token_vault_b, vec![0; 72])),
            quote_vault: Some(account(token_vault_a, vec![0; 72])),
        };

        let price = decode_price(&pool_config(pool_address), &accounts).unwrap();

        assert_eq!(price.price, Decimal::from(250u64));
    }

    #[test]
    fn decode_price_rejects_uninitialized_token_mint() {
        let pool_address = bs58::encode([1; 32]).into_string();
        let token_vault_a = bs58::encode([2; 32]).into_string();
        let token_vault_b = bs58::encode([3; 32]).into_string();
        let accounts = OrcaPoolAccounts {
            pool: account(
                pool_address.clone(),
                whirlpool_data(WSOL, &token_vault_a, USDC, &token_vault_b, Q64 / 2),
            ),
            token_mint_a: account(WSOL.to_string(), mint_data(9)),
            token_mint_b: account(USDC.to_string(), vec![0; MINT_ACCOUNT_MIN_LEN]),
            base_vault: Some(account(token_vault_a, vec![0; 72])),
            quote_vault: Some(account(token_vault_b, vec![0; 72])),
        };

        let error = decode_price(&pool_config(pool_address), &accounts).unwrap_err();

        assert!(error.to_string().contains("token mint B account is not initialized"));
    }
}
