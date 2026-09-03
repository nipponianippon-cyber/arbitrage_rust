use crate::dex::DexKind;
use crate::errors::AppError;
use rust_decimal::Decimal;
use serde::Deserialize;
use std::collections::HashSet;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub bot: BotConfig,
    pub database: DatabaseConfig,
    pub pricing: PricingConfig,
    pub notification: NotificationConfig,
    pub pools: Vec<PoolConfig>,
    #[serde(skip)]
    pub helius_rpc_url: String,
    #[serde(skip)]
    pub discord_webhook_url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    pub interval_seconds: u64,
    pub pair: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    pub path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PricingConfig {
    pub consider_dex_fee: bool,
    pub consider_slippage: bool,
    pub trade_size_usdc: Option<Decimal>,
    #[serde(default = "default_price_orientation")]
    pub price_orientation: String,
    #[serde(default = "default_meteora_dlmm_bin_array_count")]
    pub meteora_dlmm_bin_array_count: usize,
    #[serde(default = "default_meteora_dlmm_slippage_bps")]
    pub meteora_dlmm_slippage_bps: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NotificationConfig {
    pub discord_enabled: bool,
    #[serde(default = "default_true")]
    pub discord_embed_enabled: bool,
    pub notify_every_cycle: bool,
    pub notify_on_error: bool,
    #[serde(default = "default_bot_name")]
    pub bot_name: String,
    #[serde(default = "default_environment")]
    pub environment: String,
    #[serde(default)]
    pub embed_colors: EmbedColors,
}

#[derive(Debug, Clone, Deserialize)]
pub struct EmbedColors {
    #[serde(default = "default_normal_color")]
    pub normal: u32,
    #[serde(default = "default_warning_color")]
    pub warning: u32,
    #[serde(default = "default_error_color")]
    pub error: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PoolConfig {
    pub dex: DexKind,
    pub pair: String,
    #[serde(default)]
    pub pool_address: String,
    pub lb_pair_address: Option<String>,
    pub base_mint: String,
    pub quote_mint: String,
    pub price_orientation: Option<String>,
    pub auto_discovery: Option<bool>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

fn default_true() -> bool {
    true
}

fn default_bot_name() -> String {
    "solana-dex-price-monitor".to_string()
}

fn default_environment() -> String {
    "local".to_string()
}

fn default_price_orientation() -> String {
    "usdc_per_sol".to_string()
}

fn default_meteora_dlmm_bin_array_count() -> usize {
    4
}

fn default_meteora_dlmm_slippage_bps() -> u16 {
    50
}

fn default_normal_color() -> u32 {
    3_447_003
}

fn default_warning_color() -> u32 {
    16_776_960
}

fn default_error_color() -> u32 {
    15_158_332
}

impl Default for EmbedColors {
    fn default() -> Self {
        Self {
            normal: default_normal_color(),
            warning: default_warning_color(),
            error: default_error_color(),
        }
    }
}

pub fn load_config(config_path: impl AsRef<Path>) -> Result<AppConfig, AppError> {
    dotenvy::dotenv().ok();

    let body = std::fs::read_to_string(config_path.as_ref()).map_err(|error| {
        AppError::Config(format!(
            "failed to read {}: {error}",
            config_path.as_ref().display()
        ))
    })?;
    let mut config: AppConfig = toml::from_str(&body)?;
    config.helius_rpc_url = required_env("HELIUS_RPC_URL")?;
    config.discord_webhook_url = if config.notification.discord_enabled {
        required_env("DISCORD_WEBHOOK_URL")?
    } else {
        std::env::var("DISCORD_WEBHOOK_URL").unwrap_or_default()
    };
    validate_config(&config)?;
    Ok(config)
}

fn required_env(name: &str) -> Result<String, AppError> {
    let value = std::env::var(name)
        .map_err(|_| AppError::Config(format!("missing required environment variable {name}")))?;
    if value.trim().is_empty() {
        return Err(AppError::Config(format!(
            "environment variable {name} must not be empty"
        )));
    }
    Ok(value)
}

pub fn validate_config(config: &AppConfig) -> Result<(), AppError> {
    if config.bot.interval_seconds == 0 {
        return Err(AppError::Config(
            "bot.interval_seconds must be greater than zero".to_string(),
        ));
    }
    if config.bot.pair != "SOL/USDC" {
        return Err(AppError::Config(
            "only bot.pair = \"SOL/USDC\" is supported".to_string(),
        ));
    }
    if config.database.path.trim().is_empty() {
        return Err(AppError::Config(
            "database.path must not be empty".to_string(),
        ));
    }
    if config.pricing.price_orientation != "usdc_per_sol" {
        return Err(AppError::Config(
            "pricing.price_orientation must be \"usdc_per_sol\"".to_string(),
        ));
    }
    if config.pricing.consider_slippage {
        match config.pricing.trade_size_usdc {
            Some(size) if size > Decimal::ZERO => {}
            _ => return Err(AppError::Config(
                "pricing.trade_size_usdc must be set to a positive value when slippage is enabled"
                    .to_string(),
            )),
        }
    }
    if config.pricing.meteora_dlmm_bin_array_count == 0 {
        return Err(AppError::Config(
            "pricing.meteora_dlmm_bin_array_count must be greater than zero".to_string(),
        ));
    }
    if !config.notification.discord_embed_enabled {
        return Err(AppError::Config(
            "notification.discord_embed_enabled must be true because Discord Embed notifications are required".to_string(),
        ));
    }
    if config.notification.bot_name.trim().is_empty() {
        return Err(AppError::Config(
            "notification.bot_name must not be empty".to_string(),
        ));
    }
    if config.notification.environment.trim().is_empty() {
        return Err(AppError::Config(
            "notification.environment must not be empty".to_string(),
        ));
    }

    let enabled: Vec<&PoolConfig> = config.pools.iter().filter(|pool| pool.enabled).collect();
    if enabled.is_empty() {
        return Err(AppError::Config(
            "at least one enabled pool is required".to_string(),
        ));
    }

    let mut seen_dexes = HashSet::new();
    for pool in enabled {
        validate_pool(pool)?;
        seen_dexes.insert(pool.dex.as_str());
    }

    if !seen_dexes.contains("Raydium")
        || !seen_dexes.contains("Orca")
        || !seen_dexes.contains("Meteora-DLMM")
    {
        return Err(AppError::Config(
            "enabled SOL/USDC pools must include Raydium, Orca, and Meteora-DLMM".to_string(),
        ));
    }

    Ok(())
}

fn validate_pool(pool: &PoolConfig) -> Result<(), AppError> {
    if pool.pair != "SOL/USDC" {
        return Err(AppError::Config(format!(
            "{} pool has unsupported pair {}",
            pool.dex, pool.pair
        )));
    }
    match pool.dex {
        DexKind::Raydium | DexKind::Orca => validate_address("pool_address", &pool.pool_address)?,
        DexKind::MeteoraDlmm => {
            validate_address("lb_pair_address", pool.account_address().as_str())?;
            if pool.price_orientation.as_deref() != Some("usdc_per_sol") {
                return Err(AppError::Config(
                    "Meteora-DLMM pool price_orientation must be \"usdc_per_sol\"".to_string(),
                ));
            }
            if pool.auto_discovery.unwrap_or(false) {
                return Err(AppError::Config(
                    "Meteora-DLMM auto_discovery must be false in the initial implementation"
                        .to_string(),
                ));
            }
        }
    }
    validate_address("base_mint", &pool.base_mint)?;
    validate_address("quote_mint", &pool.quote_mint)?;
    crate::dex::require_sol_usdc(&pool.pair, &pool.base_mint, &pool.quote_mint)?;
    Ok(())
}

impl PoolConfig {
    /// DEXごとに設定キーが違うため、監視対象アカウントを1つの入口で取り出す。
    pub fn account_address(&self) -> String {
        match self.dex {
            DexKind::MeteoraDlmm => self.lb_pair_address.clone().unwrap_or_default(),
            DexKind::Raydium | DexKind::Orca => self.pool_address.clone(),
        }
    }
}

fn validate_address(field: &str, value: &str) -> Result<(), AppError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "未定" || trimmed.eq_ignore_ascii_case("placeholder") {
        return Err(AppError::Config(format!(
            "{field} must be set to a real Solana address"
        )));
    }
    bs58::decode(trimmed)
        .into_vec()
        .map_err(|_| AppError::Config(format!("{field} is not valid base58")))?;
    Ok(())
}
