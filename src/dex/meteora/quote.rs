use rust_decimal::Decimal;
use serde::Deserialize;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;
use chrono::Utc;

use crate::config::PoolConfig;

use super::types::{MeteoraDlmmQuote, MeteoraDlmmQuoteDirection};

/// Meteora公式TypeScript SDKを使って、監視用の両方向quoteを取得する。
///
/// quoteは補助データなので、この関数はSDK実行失敗を`Err`にせず、失敗レコードとして返す。
pub fn quote_both_directions_with_official_sdk(
    pool: &PoolConfig,
    rpc_url: &str,
    trade_size_usdc: Option<Decimal>,
    bin_array_count: usize,
    slippage_bps: u16,
) -> Vec<MeteoraDlmmQuote> {
    let Some(trade_size_usdc) = trade_size_usdc else {
        return failed_quote_records(
            pool,
            Decimal::ZERO,
            bin_array_count,
            "pricing.trade_size_usdc is not configured",
        );
    };
    if trade_size_usdc <= Decimal::ZERO {
        return failed_quote_records(
            pool,
            trade_size_usdc,
            bin_array_count,
            "pricing.trade_size_usdc must be positive",
        );
    }

    // 公式SDKのquoteロジックを使うため、Rust側では結果JSONを保存用構造へ変換するだけにする。
    let script = Path::new("tools")
        .join("meteora-dlmm-sdk-fixture")
        .join("quote-both-directions.mjs");
    let output = Command::new("node")
        .arg(script)
        .arg("--pool")
        .arg(pool.account_address())
        .arg("--rpc-url")
        .arg(rpc_url)
        .arg("--base-mint")
        .arg(&pool.base_mint)
        .arg("--quote-mint")
        .arg(&pool.quote_mint)
        .arg("--trade-size-usdc")
        .arg(trade_size_usdc.to_string())
        .arg("--bin-array-count")
        .arg(bin_array_count.to_string())
        .arg("--slippage-bps")
        .arg(slippage_bps.to_string())
        .output();

    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return failed_quote_records(
                pool,
                trade_size_usdc,
                bin_array_count,
                format!("failed to execute Meteora-DLMM SDK quote helper: {error}"),
            );
        }
    };

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if stderr.is_empty() { stdout } else { stderr };
        return failed_quote_records(
            pool,
            trade_size_usdc,
            bin_array_count,
            format!("Meteora-DLMM SDK quote helper failed: {detail}"),
        );
    }

    let parsed: Result<SdkQuoteOutput, serde_json::Error> = serde_json::from_slice(&output.stdout);
    let parsed = match parsed {
        Ok(parsed) => parsed,
        Err(error) => {
            return failed_quote_records(
                pool,
                trade_size_usdc,
                bin_array_count,
                format!("failed to parse Meteora-DLMM SDK quote JSON: {error}"),
            );
        }
    };

    parsed
        .quotes
        .into_iter()
        .map(|quote| quote.into_record(pool, bin_array_count))
        .collect()
}

#[derive(Debug, Deserialize)]
struct SdkQuoteOutput {
    quotes: Vec<SdkQuoteRecord>,
}

#[derive(Debug, Deserialize)]
struct SdkQuoteRecord {
    direction: String,
    input_mint: String,
    output_mint: String,
    requested_input_amount: String,
    requested_input_amount_raw: String,
    consumed_input_amount: Option<String>,
    consumed_input_amount_raw: Option<String>,
    output_amount: Option<String>,
    output_amount_raw: Option<String>,
    fee_amount: Option<String>,
    fee_amount_raw: Option<String>,
    protocol_fee_amount: Option<String>,
    protocol_fee_amount_raw: Option<String>,
    price_impact_bps: Option<String>,
    effective_price: Option<String>,
    end_price: Option<String>,
    bin_array_addresses: Vec<String>,
    partial_fill: bool,
    success: bool,
    error_message: Option<String>,
}

impl SdkQuoteRecord {
    fn into_record(
        self,
        pool: &PoolConfig,
        _configured_bin_array_count: usize,
    ) -> MeteoraDlmmQuote {
        let direction = match self.direction.as_str() {
            "USDC -> SOL" => MeteoraDlmmQuoteDirection::UsdcToSol,
            "SOL -> USDC" => MeteoraDlmmQuoteDirection::SolToUsdc,
            _ => MeteoraDlmmQuoteDirection::UsdcToSol,
        };
        MeteoraDlmmQuote {
            lb_pair_address: pool.account_address(),
            direction,
            input_mint: self.input_mint,
            output_mint: self.output_mint,
            requested_input_amount: decimal_from_str_or_zero(&self.requested_input_amount),
            requested_input_amount_raw: u64_from_str_or_zero(&self.requested_input_amount_raw),
            consumed_input_amount: decimal_from_optional_str(self.consumed_input_amount.as_deref()),
            consumed_input_amount_raw: u64_from_optional_str(
                self.consumed_input_amount_raw.as_deref(),
            ),
            output_amount: decimal_from_optional_str(self.output_amount.as_deref()),
            output_amount_raw: u64_from_optional_str(self.output_amount_raw.as_deref()),
            fee_amount: decimal_from_optional_str(self.fee_amount.as_deref()),
            fee_amount_raw: u64_from_optional_str(self.fee_amount_raw.as_deref()),
            protocol_fee_amount: decimal_from_optional_str(self.protocol_fee_amount.as_deref()),
            protocol_fee_amount_raw: u64_from_optional_str(
                self.protocol_fee_amount_raw.as_deref(),
            ),
            price_impact_bps: decimal_from_optional_str(self.price_impact_bps.as_deref()),
            effective_price: decimal_from_optional_str(self.effective_price.as_deref()),
            end_price: decimal_from_optional_str(self.end_price.as_deref()),
            bin_array_count: self.bin_array_addresses.len(),
            bin_array_addresses: self.bin_array_addresses,
            partial_fill: self.partial_fill,
            success: self.success,
            error_message: self.error_message,
            slot: None,
            observed_at: Utc::now(),
        }
    }
}

fn failed_quote_records(
    pool: &PoolConfig,
    trade_size_usdc: Decimal,
    bin_array_count: usize,
    message: impl Into<String>,
) -> Vec<MeteoraDlmmQuote> {
    let message = message.into();
    vec![
        failed_quote_record(
            pool,
            MeteoraDlmmQuoteDirection::UsdcToSol,
            trade_size_usdc,
            bin_array_count,
            &message,
        ),
        failed_quote_record(
            pool,
            MeteoraDlmmQuoteDirection::SolToUsdc,
            trade_size_usdc,
            bin_array_count,
            &message,
        ),
    ]
}

fn failed_quote_record(
    pool: &PoolConfig,
    direction: MeteoraDlmmQuoteDirection,
    requested_input_amount: Decimal,
    bin_array_count: usize,
    message: &str,
) -> MeteoraDlmmQuote {
    let (input_mint, output_mint) = match direction {
        MeteoraDlmmQuoteDirection::UsdcToSol => (&pool.quote_mint, &pool.base_mint),
        MeteoraDlmmQuoteDirection::SolToUsdc => (&pool.base_mint, &pool.quote_mint),
    };
    MeteoraDlmmQuote {
        lb_pair_address: pool.account_address(),
        direction,
        input_mint: input_mint.to_string(),
        output_mint: output_mint.to_string(),
        requested_input_amount,
        requested_input_amount_raw: 0,
        consumed_input_amount: None,
        consumed_input_amount_raw: None,
        output_amount: None,
        output_amount_raw: None,
        fee_amount: None,
        fee_amount_raw: None,
        protocol_fee_amount: None,
        protocol_fee_amount_raw: None,
        price_impact_bps: None,
        effective_price: None,
        end_price: None,
        bin_array_count,
        bin_array_addresses: Vec::new(),
        partial_fill: false,
        success: false,
        error_message: Some(message.to_string()),
        slot: None,
        observed_at: Utc::now(),
    }
}

fn decimal_from_str_or_zero(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap_or(Decimal::ZERO)
}

fn decimal_from_optional_str(value: Option<&str>) -> Option<Decimal> {
    value.and_then(|value| Decimal::from_str(value).ok())
}

fn u64_from_str_or_zero(value: &str) -> u64 {
    value.parse().unwrap_or(0)
}

fn u64_from_optional_str(value: Option<&str>) -> Option<u64> {
    value.and_then(|value| value.parse().ok())
}
