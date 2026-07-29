use crate::config::{EmbedColors, NotificationConfig};
use crate::errors::{AppError, ErrorSeverity, MonitorErrorRecord};
use crate::pricing::PriceSpread;
use serde_json::{json, Value};

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
            .map_err(|error| AppError::Notification(format!("failed to send Discord webhook: {error}")))?;
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
    let raydium_price = if spread.dex_a.dex.as_str() == "Raydium" {
        spread.dex_a.price
    } else {
        spread.dex_b.price
    };
    let orca_price = if spread.dex_a.dex.as_str() == "Orca" {
        spread.dex_a.price
    } else {
        spread.dex_b.price
    };
    let slot = [spread.dex_a.slot, spread.dex_b.slot]
        .into_iter()
        .flatten()
        .max()
        .map(|value| value.to_string())
        .unwrap_or_else(|| "n/a".to_string());
    let fee_adjusted_reference_spread = spread
        .fee_adjusted_reference_spread
        .map(|value| value.to_string())
        .unwrap_or_else(|| "disabled".to_string());

    json!({
        "username": bot_name,
        "embeds": [{
            "title": "SOL/USDC Price Spread",
            "description": "Raydium and Orca price spread summary",
            "color": embed_colors.normal,
            "fields": [
                field("Raydium", format!("{raydium_price} USDC"), true),
                field("Orca", format!("{orca_price} USDC"), true),
                field("Spread", format!("{} USDC / {} bps", spread.absolute_spread, spread.spread_bps), false),
                field("Higher", spread.higher_dex.to_string(), true),
                field("Lower", spread.lower_dex.to_string(), true),
                field("Direction", spread.comparison_direction.clone(), false),
                field("Slot", slot, true),
                field("Observed", spread.calculated_at.to_rfc3339(), true),
                field("Fee Adjusted Reference Spread", fee_adjusted_reference_spread, false),
                field("Errors", "none", true),
            ],
            "timestamp": spread.calculated_at.to_rfc3339(),
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
            "description": error.message,
            "color": color,
            "fields": [
                field("Component", error.component.clone(), true),
                field("Severity", error.severity.as_str(), true),
                field("DEX", error.dex.map(|dex| dex.to_string()).unwrap_or_else(|| "n/a".to_string()), true),
                field("Pool", error.pool_address.clone().unwrap_or_else(|| "n/a".to_string()), false),
                field("Retry", if error.retry_planned { "planned" } else { "not planned" }, true),
                field("Consecutive Errors", error.consecutive_count.to_string(), true),
                field("Source", error.source.clone().unwrap_or_else(|| "n/a".to_string()), false),
            ],
            "timestamp": error.occurred_at.to_rfc3339(),
            "footer": {
                "text": format!("{environment} | Helius HTTP RPC")
            }
        }]
    })
}

fn field(name: impl Into<String>, value: impl Into<String>, inline: bool) -> Value {
    json!({
        "name": name.into(),
        "value": value.into(),
        "inline": inline,
    })
}
