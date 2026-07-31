pub mod orca;
pub mod raydium;
pub mod meteora;

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DexKind {
    Raydium,
    Orca,
    MeteoraDlmm,
}

impl DexKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Raydium => "Raydium",
            Self::Orca => "Orca",
            Self::MeteoraDlmm => "Meteora-DLMM",
        }
    }
}

impl fmt::Display for DexKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for DexKind {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.to_ascii_lowercase().as_str() {
            "raydium" => Ok(Self::Raydium),
            "orca" => Ok(Self::Orca),
            "meteora" | "meteora_dlmm" | "meteora-dlmm" => Ok(Self::MeteoraDlmm),
            other => Err(format!("unsupported dex `{other}`")),
        }
    }
}

impl Serialize for DexKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DexKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::from_str(&value).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone)]
pub struct DexPrice {
    pub dex: DexKind,
    pub pair: String,
    pub pool_address: String,
    pub price: Decimal,
    pub fee_adjusted_price: Option<Decimal>,
    pub slippage_adjusted_price: Option<Decimal>,
    pub liquidity: Option<Decimal>,
    pub slot: Option<u64>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct PoolAccounts {
    pub pool: crate::rpc::AccountData,
    pub base_vault: Option<crate::rpc::AccountData>,
    pub quote_vault: Option<crate::rpc::AccountData>,
}

pub fn token_amount(data: &[u8]) -> Result<u64, crate::errors::AppError> {
    let bytes = data
        .get(64..72)
        .ok_or_else(|| crate::errors::AppError::Decode("token account is shorter than SPL Token amount field".to_string()))?;
    Ok(u64::from_le_bytes(bytes.try_into().expect("slice length checked")))
}

pub fn decimal_amount(amount: u64, decimals: u8) -> Decimal {
    Decimal::from_i128_with_scale(amount as i128, decimals as u32)
}

pub fn read_pubkey(data: &[u8], offset: usize, field: &str) -> Result<String, crate::errors::AppError> {
    let bytes = data
        .get(offset..offset + 32)
        .ok_or_else(|| crate::errors::AppError::Decode(format!("pool account is missing {field}")))?;
    Ok(bs58::encode(bytes).into_string())
}

pub fn read_u64(data: &[u8], offset: usize, field: &str) -> Result<u64, crate::errors::AppError> {
    let bytes = data
        .get(offset..offset + 8)
        .ok_or_else(|| crate::errors::AppError::Decode(format!("pool account is missing {field}")))?;
    Ok(u64::from_le_bytes(bytes.try_into().expect("slice length checked")))
}

pub fn require_sol_usdc(pair: &str, base_mint: &str, quote_mint: &str) -> Result<(), crate::errors::AppError> {
    const WSOL: &str = "So11111111111111111111111111111111111111112";
    const USDC: &str = "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v";

    if pair != "SOL/USDC" || base_mint != WSOL || quote_mint != USDC {
        return Err(crate::errors::AppError::Decode(
            "only SOL/USDC with WSOL base mint and USDC quote mint is supported".to_string(),
        ));
    }

    Ok(())
}
