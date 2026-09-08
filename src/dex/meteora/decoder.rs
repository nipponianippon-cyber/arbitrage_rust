use super::types::{
    MeteoraPoolMeta,
};
use crate::errors::AppError;
use crate::rpc::AccountData;
use crate::dex::read_pubkey;

const BASE_FACTOR: usize = 8;
const VARIABLE_FEE_CONTROL_OFFSET: usize = 16;
const BASE_FEE_POWER_FACTOR: usize = 34;
const VOLATILITY_ACCUMULATOR_OFFSET: usize = 40;
const ACTIVE_ID_OFFSET: usize = 76;
const BIN_STEP_OFFSET: usize = 80;
const STATUS_OFFSET: usize = 82;
const TOKEN_X_MINT_OFFSET: usize = 88;
const TOKEN_Y_MINT_OFFSET: usize = 120;
const RESERVE_X_OFFSET: usize = 152;
const RESERVE_Y_OFFSET: usize = 184;

const MINT_DECIMALS_OFFSET: usize = 44;
const MINT_IS_INITIALIZED_OFFSET: usize = 45;
const MINT_ACCOUNT_MIN_LEN: usize = 82;
pub const MAX_FEE_RATE: u64 = 100_000_000;
pub const FEE_RATE_TO_BPS_DIVISOR: u64 = 100_000;


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

pub fn ensure_account_address(
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
