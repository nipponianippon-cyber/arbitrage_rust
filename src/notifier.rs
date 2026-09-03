use crate::config::{EmbedColors, NotificationConfig};
use crate::dex::{DexKind, DexPrice};
use crate::errors::{AppError, ErrorSeverity, MonitorErrorRecord};
use crate::pricing::PriceSpread;
use serde_json::{Value, json};

const PRICE_DECIMAL_PLACES: u32 = 4;
const SPREAD_USDC_DECIMAL_PLACES: u32 = 6;
const BPS_DECIMAL_PLACES: u32 = 2;
const MAX_EMBED_FIELDS: usize = 12;
const ERROR_DESCRIPTION_MAX_CHARS: usize = 500;
const ERROR_SOURCE_MAX_CHARS: usize = 300;
const SHORT_ADDRESS_EDGE_CHARS: usize = 8;
const DISCORD_FIELD_NAME_MAX_CHARS: usize = 256;
const DISCORD_FIELD_VALUE_MAX_CHARS: usize = 1024;

#[derive(Debug, Clone)]
pub struct DiscordNotifier {
    webhook_url: String,
    enabled: bool,
    bot_name: String,
    environment: String,
    embed_colors: EmbedColors,
    http: reqwest::Client,
}

impl DiscordNotifier {
    pub fn new(webhook_url: impl Into<String>, config: &NotificationConfig) -> Self {
        Self {
            webhook_url: webhook_url.into(),
            enabled: config.discord_enabled,
            bot_name: config.bot_name.clone(),
            environment: config.environment.clone(),
            embed_colors: config.embed_colors.clone(),
            http: reqwest::Client::new(),
        }
    }

    pub async fn send_price_spread(&self, spread: &PriceSpread) -> Result<(), AppError> {
        self.send_payload(build_price_spread_embed_payload(
            spread,
            &self.bot_name,
            &self.environment,
            &self.embed_colors,
        ))
        .await
    }

    pub async fn send_price_spreads(
        &self,
        prices: &[DexPrice],
        spreads: &[PriceSpread],
    ) -> Result<(), AppError> {
        self.send_payload(build_price_spreads_embed_payload(
            prices,
            spreads,
            &self.bot_name,
            &self.environment,
            &self.embed_colors,
        ))
        .await
    }

    pub async fn send_error(&self, error: &MonitorErrorRecord) -> Result<(), AppError> {
        self.send_payload(build_error_embed_payload(
            error,
            &self.bot_name,
            &self.environment,
            &self.embed_colors,
        ))
        .await
    }

    async fn send_payload(&self, payload: Value) -> Result<(), AppError> {
        if !self.enabled {
            tracing::info!("{payload}");
            return Ok(());
        }

        let response = self
            .http
            .post(&self.webhook_url)
            .json(&payload)
            .send()
            .await
            .map_err(|error| {
                AppError::Notification(format!("failed to send Discord webhook: {error}"))
            })?;
        let status = response.status();
        if !status.is_success() {
            return Err(AppError::Notification(format!(
                "Discord webhook returned HTTP {status}"
            )));
        }
        Ok(())
    }
}

pub fn build_price_spread_embed_payload(
    spread: &PriceSpread,
    bot_name: &str,
    environment: &str,
    embed_colors: &EmbedColors,
) -> Value {
    let slot = [spread.dex_a.slot, spread.dex_b.slot]
        .into_iter()
        .flatten()
        .max()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let fee_adjusted_reference_spread = spread
        .fee_adjusted_reference_spread
        .map(|value| format!("{} USDC", format_spread_usdc(value)))
        .unwrap_or_else(|| "disabled".to_string());

    let fields = limit_fields(vec![
        field(
            spread.dex_a.dex.to_string(),
            format!("{} USDC", format_price(spread.dex_a.price)),
            true,
        ),
        field(
            spread.dex_b.dex.to_string(),
            format!("{} USDC", format_price(spread.dex_b.price)),
            true,
        ),
        field(
            "Spread",
            format!(
                "{} USDC / {} bps",
                format_spread_usdc(spread.absolute_spread),
                format_bps(spread.spread_bps)
            ),
            false,
        ),
        field("Higher", spread.higher_dex.to_string(), true),
        field("Lower", spread.lower_dex.to_string(), true),
        field("Direction", spread.comparison_direction.clone(), false),
        field("Slot", slot, true),
        field("Observed", spread.calculated_at.to_rfc3339(), true),
        field("Fee Adjusted Reference Spread", fee_adjusted_reference_spread, false),
        field("Errors", "none", true),
    ]);

    json!({
        "username": bot_name,
        "embeds": [{
            "title": "SOL/USDC Price Spread",
            "description": format!("{} and {} price spread summary", spread.dex_a.dex, spread.dex_b.dex),
            "color": embed_colors.normal,
            "fields": fields,
            "timestamp": spread.calculated_at.to_rfc3339(),
            "footer": {
                "text": format!("{environment} | Helius HTTP RPC")
            }
        }]
    })
}

/// 3 DEXの価格と全組み合わせ価格差を1つのDiscord Embedへまとめる。
pub fn build_price_spreads_embed_payload(
    prices: &[DexPrice],
    spreads: &[PriceSpread],
    bot_name: &str,
    environment: &str,
    embed_colors: &EmbedColors,
) -> Value {
    let slot = prices
        .iter()
        .filter_map(|price| price.slot)
        .max()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let observed_at = prices
        .iter()
        .map(|price| price.observed_at)
        .max()
        .unwrap_or_else(chrono::Utc::now);
    let mut fields = vec![
        field("Raydium", price_value(prices, DexKind::Raydium), true),
        field("Orca", price_value(prices, DexKind::Orca), true),
        field(
            "Meteora-DLMM",
            price_value(prices, DexKind::MeteoraDlmm),
            true,
        ),
    ];

    // Discordには詳細なMeteora内部状態を出さず、比較に必要な概要だけを載せる。
    for spread in spreads {
        let fee_adjusted_reference_spread = spread
            .fee_adjusted_reference_spread
            .map(|value| format!("{} USDC", format_spread_usdc(value)))
            .unwrap_or_else(|| "disabled".to_string());
        fields.push(field(
            format!("{} vs {}", spread.dex_a.dex, spread.dex_b.dex),
            format!(
                "{} USDC / {} bps\nHigher: {}\nLower: {}\nFee adjusted: {}\n{}",
                format_spread_usdc(spread.absolute_spread),
                format_bps(spread.spread_bps),
                spread.higher_dex,
                spread.lower_dex,
                fee_adjusted_reference_spread,
                spread.comparison_direction
            ),
            false,
        ));
    }
    fields.push(field("Slot", slot, true));
    fields.push(field("Observed", observed_at.to_rfc3339(), true));
    fields.push(field("Errors", "none", true));
    let fields = limit_fields(fields);

    json!({
        "username": bot_name,
        "embeds": [{
            "title": "SOL/USDC Price Spread",
            "description": "Raydium, Orca, and Meteora-DLMM price spread summary",
            "color": embed_colors.normal,
            "fields": fields,
            "timestamp": observed_at.to_rfc3339(),
            "footer": {
                "text": format!("{environment} | Helius HTTP RPC")
            }
        }]
    })
}

pub fn build_error_embed_payload(
    error: &MonitorErrorRecord,
    bot_name: &str,
    environment: &str,
    embed_colors: &EmbedColors,
) -> Value {
    let color = match error.severity {
        ErrorSeverity::Info => embed_colors.normal,
        ErrorSeverity::Warning => embed_colors.warning,
        ErrorSeverity::Critical => embed_colors.error,
    };
    json!({
        "username": bot_name,
        "embeds": [{
            "title": "SOL/USDC Monitor Error",
            "description": truncate_with_ellipsis(&error.message, ERROR_DESCRIPTION_MAX_CHARS),
            "color": color,
            "fields": limit_fields(vec![
                field("Component", error.component.clone(), true),
                field("Severity", error.severity.as_str(), true),
                field("DEX", error.dex.map(|dex| dex.to_string()).unwrap_or_else(|| "n/a".to_string()), true),
                field(
                    "Pool",
                    error
                        .pool_address
                        .as_deref()
                        .map(shorten_address)
                        .unwrap_or_else(|| "n/a".to_string()),
                    false,
                ),
                field("Retry", if error.retry_planned { "planned" } else { "not planned" }, true),
                field("Consecutive Errors", error.consecutive_count.to_string(), true),
                field(
                    "Source",
                    error
                        .source
                        .as_deref()
                        .map(|source| truncate_with_ellipsis(source, ERROR_SOURCE_MAX_CHARS))
                        .unwrap_or_else(|| "n/a".to_string()),
                    false,
                ),
            ]),
            "timestamp": error.occurred_at.to_rfc3339(),
            "footer": {
                "text": format!("{environment} | Helius HTTP RPC")
            }
        }]
    })
}

fn field(name: impl Into<String>, value: impl Into<String>, inline: bool) -> Value {
    let name = truncate_with_ellipsis(&name.into(), DISCORD_FIELD_NAME_MAX_CHARS);
    let value = normalize_field_value(value.into());
    json!({
        "name": name,
        "value": value,
        "inline": inline,
    })
}

fn price_value(prices: &[DexPrice], dex: DexKind) -> String {
    prices
        .iter()
        .find(|price| price.dex == dex)
        .map(|price| format!("{} USDC", format_price(price.price)))
        .unwrap_or_else(|| "n/a".to_string())
}

fn format_price(value: rust_decimal::Decimal) -> String {
    format_decimal(value, PRICE_DECIMAL_PLACES)
}

fn format_spread_usdc(value: rust_decimal::Decimal) -> String {
    format_decimal(value, SPREAD_USDC_DECIMAL_PLACES)
}

fn format_bps(value: rust_decimal::Decimal) -> String {
    format_decimal(value, BPS_DECIMAL_PLACES)
}

fn format_decimal(value: rust_decimal::Decimal, decimal_places: u32) -> String {
    let mut value = value.round_dp(decimal_places).to_string();
    if decimal_places == 0 {
        return value;
    }

    // rust_decimalの文字列表現が末尾ゼロを省いても、Discord表示では桁を固定する。
    let decimal_places = decimal_places as usize;
    match value.find('.') {
        Some(point_index) => {
            let current_places = value.len() - point_index - 1;
            if current_places < decimal_places {
                value.push_str(&"0".repeat(decimal_places - current_places));
            }
        }
        None => {
            value.push('.');
            value.push_str(&"0".repeat(decimal_places));
        }
    }
    value
}

fn limit_fields(mut fields: Vec<Value>) -> Vec<Value> {
    if fields.len() <= MAX_EMBED_FIELDS {
        return fields;
    }

    // Discordでは一覧性を優先し、上限を超えたfieldは最後のSummaryに圧縮する。
    let overflow = fields.split_off(MAX_EMBED_FIELDS - 1);
    let summary = overflow
        .iter()
        .map(summary_line)
        .collect::<Vec<_>>()
        .join("\n");
    fields.push(field("Summary", summary, false));
    fields
}

fn summary_line(field: &Value) -> String {
    let name = field
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Field");
    let value = field
        .get("value")
        .and_then(Value::as_str)
        .and_then(|value| value.lines().next())
        .unwrap_or("n/a");
    format!("{name}: {value}")
}

fn normalize_field_value(value: String) -> String {
    let value = if value.is_empty() {
        "n/a".to_string()
    } else {
        value
    };
    truncate_with_ellipsis(&value, DISCORD_FIELD_VALUE_MAX_CHARS)
}

fn truncate_with_ellipsis(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }

    let mut truncated = value.chars().take(max_chars - 3).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn shorten_address(value: &str) -> String {
    let chars = value.chars().collect::<Vec<_>>();
    if chars.len() <= SHORT_ADDRESS_EDGE_CHARS * 2 + 3 {
        return value.to_string();
    }

    let prefix = chars
        .iter()
        .take(SHORT_ADDRESS_EDGE_CHARS)
        .collect::<String>();
    let suffix = chars
        .iter()
        .skip(chars.len() - SHORT_ADDRESS_EDGE_CHARS)
        .collect::<String>();
    format!("{prefix}...{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dex::DexPrice;
    use crate::errors::{ErrorSeverity, MonitorErrorRecord};
    use crate::pricing::PriceSpread;
    use chrono::Utc;
    use rust_decimal::Decimal;
    use std::str::FromStr;

    fn decimal(value: &str) -> Decimal {
        Decimal::from_str(value).expect("test decimal must be valid")
    }

    fn dex_price(dex: DexKind, price: &str) -> DexPrice {
        DexPrice {
            dex,
            pair: "SOL/USDC".to_string(),
            pool_address: format!("{dex}-pool"),
            price: decimal(price),
            fee_adjusted_price: None,
            slippage_adjusted_price: None,
            liquidity: None,
            slot: Some(123),
            observed_at: Utc::now(),
        }
    }

    fn price_spread(index: usize) -> PriceSpread {
        let dex_a = dex_price(DexKind::Raydium, "182.123456");
        let dex_b = dex_price(DexKind::Orca, "182.000000");
        PriceSpread {
            pair: "SOL/USDC".to_string(),
            dex_a,
            dex_b,
            absolute_spread: decimal("0.1234567"),
            spread_bps: decimal("12.346"),
            higher_dex: DexKind::Raydium,
            lower_dex: DexKind::Orca,
            comparison_direction: format!("buy on Orca, sell on Raydium #{index}"),
            fee_adjusted_reference_spread: Some(decimal("0.0123456")),
            calculated_at: Utc::now(),
        }
    }

    #[test]
    fn price_spread_embed_uses_display_rounding_only() {
        let spread = price_spread(1);

        let payload =
            build_price_spread_embed_payload(&spread, "bot", "local", &EmbedColors::default());
        let fields = payload["embeds"][0]["fields"]
            .as_array()
            .expect("fields must be an array");

        assert_eq!(fields[0]["value"], "182.1235 USDC");
        assert_eq!(fields[2]["value"], "0.123457 USDC / 12.35 bps");
        assert_eq!(fields[8]["value"], "0.012346 USDC");
    }

    #[test]
    fn price_spreads_embed_limits_fields_and_summarizes_overflow() {
        let prices = vec![
            dex_price(DexKind::Raydium, "182.123456"),
            dex_price(DexKind::Orca, "181.987654"),
            dex_price(DexKind::MeteoraDlmm, "182.000001"),
        ];
        let spreads = (0..14).map(price_spread).collect::<Vec<_>>();

        let payload = build_price_spreads_embed_payload(
            &prices,
            &spreads,
            "bot",
            "local",
            &EmbedColors::default(),
        );
        let fields = payload["embeds"][0]["fields"]
            .as_array()
            .expect("fields must be an array");

        assert_eq!(fields.len(), MAX_EMBED_FIELDS);
        assert_eq!(fields.last().unwrap()["name"], "Summary");
    }

    #[test]
    fn error_embed_shortens_long_display_values() {
        let error = MonitorErrorRecord::new(
            "rpc",
            ErrorSeverity::Warning,
            "x".repeat(600),
            Some("s".repeat(400)),
        )
        .with_pool_context(DexKind::MeteoraDlmm, "12345678ABCDEFGHabcdefgh87654321");

        let payload = build_error_embed_payload(&error, "bot", "local", &EmbedColors::default());
        let embed = &payload["embeds"][0];
        let fields = embed["fields"].as_array().expect("fields must be an array");

        let description = embed["description"].as_str().unwrap();
        let pool = fields[3]["value"].as_str().unwrap();
        let source = fields[6]["value"].as_str().unwrap();

        assert_eq!(description.chars().count(), ERROR_DESCRIPTION_MAX_CHARS);
        assert!(description.ends_with("..."));
        assert_eq!(pool, "12345678...87654321");
        assert_eq!(source.chars().count(), ERROR_SOURCE_MAX_CHARS);
        assert!(source.ends_with("..."));
    }
}
