use crate::config::{AppConfig, PoolConfig};
use crate::dex::meteora;
use crate::dex::meteora::MeteoraDlmmState;
use crate::dex::orca;
use crate::dex::raydium;
use crate::dex::{DexKind, DexPrice, PoolAccounts};
use crate::errors::{AppError, ErrorSeverity, MonitorErrorRecord};
use crate::notifier::DiscordNotifier;
use crate::pricing::calculate_all_spreads;
use crate::rpc::{AccountData, RpcClient};
use crate::storage::Storage;
use std::collections::HashMap;
use tokio::time::{self, Duration};

pub async fn run_once(
    config: &AppConfig,
    rpc: &RpcClient,
    storage: &Storage,
    notifier: &DiscordNotifier,
) -> Result<(), AppError> {
    let enabled: Vec<&PoolConfig> = config.pools.iter().filter(|pool| pool.enabled).collect();
    let pool_addresses: Vec<String> = enabled.iter().map(|pool| pool.account_address()).collect();
    let pool_accounts = rpc.get_multiple_accounts(&pool_addresses).await?;
    let account_map: HashMap<String, AccountData> = pool_accounts
        .into_iter()
        .map(|account| (account.address.clone(), account))
        .collect();

    let mut vault_addresses = Vec::new();
    for pool in &enabled {
        let account = account_map
            .get(&pool.account_address())
            .ok_or_else(|| AppError::Rpc(format!("missing fetched pool account {}", pool.account_address())))?;
        match pool.dex {
            DexKind::Raydium => {
                let meta = raydium::decode_pool_meta(&account.data)?;
                vault_addresses.push(meta.base_vault);
                vault_addresses.push(meta.quote_vault);
            }
            DexKind::Orca => {
                let meta = orca::decode_pool_meta(&account.data)?;
                let (base_vault, quote_vault) = orca::vault_addresses_for_config(pool, &meta)?;
                vault_addresses.push(base_vault);
                vault_addresses.push(quote_vault);
            }
            DexKind::MeteoraDlmm => {
                let _meta = meteora::decode_pool_meta(&account.data)?;
            }
        }
    }
    vault_addresses.sort();
    vault_addresses.dedup();

    let vault_accounts = rpc.get_multiple_accounts(&vault_addresses).await?;
    let mut all_accounts = account_map;
    for account in vault_accounts {
        all_accounts.insert(account.address.clone(), account);
    }

    let mut prices = Vec::new();
    for pool in enabled {
        match decode_pool_price(pool, &all_accounts) {
            Ok((price, meteora_state)) => {
                storage.insert_price_observation(&price)?;
                if let Some(state) = meteora_state {
                    storage.insert_meteora_dlmm_state(&state)?;
                }
                prices.push(price);
            }
            Err(error) => {
                let record = error
                    .to_monitor_record()
                    .with_pool_context(pool.dex, pool.account_address());
                storage.insert_failed_price_observation(
                    pool.dex.as_str(),
                    &pool.pair,
                    &pool.account_address(),
                    &error,
                )?;
                storage.insert_monitor_error(&record)?;
                if config.notification.notify_on_error {
                    if let Err(notification_error) = notifier.send_error(&record).await {
                        eprintln!("{notification_error}");
                    }
                }
            }
        }
    }

    if prices.len() == 3 {
        let spreads = calculate_all_spreads(&prices)?;
        for spread in &spreads {
            storage.insert_price_spread(spread)?;
        }
        if config.notification.notify_every_cycle {
            if let Err(error) = notifier.send_price_spreads(&prices, &spreads).await {
                let record = error.to_monitor_record();
                eprintln!("{error}");
                storage.insert_monitor_error(&record)?;
            }
        }
    } else {
        let record = MonitorErrorRecord::new(
            "runner",
            ErrorSeverity::Warning,
            "skipped spread calculation because one or more of the three DEX prices were unavailable",
            None,
        )
        .with_consecutive_count(1);
        storage.insert_monitor_error(&record)?;
        if config.notification.notify_on_error {
            if let Err(error) = notifier.send_error(&record).await {
                eprintln!("{error}");
            }
        }
    }

    Ok(())
}

pub async fn run_forever(
    config: AppConfig,
    rpc: RpcClient,
    storage: Storage,
    notifier: DiscordNotifier,
) -> Result<(), AppError> {
    let mut interval = time::interval(Duration::from_secs(config.bot.interval_seconds));
    loop {
        tokio::select! {
            _ = interval.tick() => {
                if let Err(error) = run_once(&config, &rpc, &storage, &notifier).await {
                    let record = error.to_monitor_record();
                    eprintln!("{error}");
                    storage.insert_monitor_error(&record)?;
                    if config.notification.notify_on_error {
                        if let Err(notification_error) = notifier.send_error(&record).await {
                            eprintln!("{notification_error}");
                        }
                    }
                }
            }
            result = tokio::signal::ctrl_c() => {
                result.map_err(|error| AppError::Io(error))?;
                tracing::info!("received Ctrl+C, shutting down");
                return Ok(());
            }
        }
    }
}

fn decode_pool_price(
    pool: &PoolConfig,
    accounts: &HashMap<String, AccountData>,
) -> Result<(DexPrice, Option<MeteoraDlmmState>), AppError> {
    let pool_account = accounts
        .get(&pool.account_address())
        .ok_or_else(|| AppError::Rpc(format!("missing fetched pool account {}", pool.account_address())))?
        .clone();

    match pool.dex {
        DexKind::Raydium => {
            let meta = raydium::decode_pool_meta(&pool_account.data)?;
            let pool_accounts = PoolAccounts {
                pool: pool_account,
                base_vault: accounts.get(&meta.base_vault).cloned(),
                quote_vault: accounts.get(&meta.quote_vault).cloned(),
            };
            raydium::decode_price(pool, &pool_accounts).map(|price| (price, None))
        }
        DexKind::Orca => {
            let meta = orca::decode_pool_meta(&pool_account.data)?;
            let (base_vault, quote_vault) = orca::vault_addresses_for_config(pool, &meta)?;
            let pool_accounts = PoolAccounts {
                pool: pool_account,
                base_vault: accounts.get(&base_vault).cloned(),
                quote_vault: accounts.get(&quote_vault).cloned(),
            };
            orca::decode_price(pool, &pool_accounts).map(|price| (price, None))
        }
        DexKind::MeteoraDlmm => {
            let pool_accounts = meteora::MeteoraPoolAccounts {
                lb_pair: pool_account,
            };
            meteora::decode_price(pool, &pool_accounts).map(|(price, state)| (price, Some(state)))
        }
    }
}
