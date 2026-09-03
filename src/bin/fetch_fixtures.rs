use arbitrage_rust::config::{AppConfig, PoolConfig, load_config};
use arbitrage_rust::dex::meteora;
use arbitrage_rust::dex::meteora::MeteoraDlmmQuoteDirection;
use arbitrage_rust::dex::orca;
use arbitrage_rust::dex::raydium;
use arbitrage_rust::dex::DexKind;
use arbitrage_rust::errors::AppError;
use arbitrage_rust::rpc::{AccountData, RpcClient};
use base64::Engine;
use chrono::Utc;
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone)]
struct FixtureTarget {
    address: String,
    dex: DexKind,
    pair: String,
    role: &'static str,
}

#[derive(Debug, Serialize)]
struct AccountFixture<'a> {
    requested_address: &'a str,
    rpc_method: &'static str,
    context_slot: u64,
    owner: &'a str,
    lamports: u64,
    data_base64: String,
    fetched_at: String,
    dex: &'a str,
    pair: &'a str,
    role: &'a str,
}

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), AppError> {
    let mut args = std::env::args().skip(1);
    let config_path = args.next().unwrap_or_else(|| "config.toml".to_string());
    let output_dir = args
        .next()
        .unwrap_or_else(|| "tests/fixtures/local".to_string());

    let config = load_config(config_path)?;
    let enabled: Vec<&PoolConfig> = config.pools.iter().filter(|pool| pool.enabled).collect();
    let rpc = RpcClient::new(config.helius_rpc_url.clone());

    let pool_addresses: Vec<String> = enabled.iter().map(|pool| pool.account_address()).collect();
    let pool_accounts = rpc.get_multiple_accounts(&pool_addresses).await?;
    let mut account_map: HashMap<String, AccountData> = pool_accounts
        .into_iter()
        .map(|account| (account.address.clone(), account))
        .collect();

    // pool本体を先にデコードして、価格計算に必要な依存アカウントを同じ形式で集める。
    let mut targets = Vec::new();
    let mut dependent_addresses = Vec::new();
    for pool in &enabled {
        let pool_address = pool.account_address();
        targets.push(FixtureTarget {
            address: pool_address.clone(),
            dex: pool.dex,
            pair: pool.pair.clone(),
            role: "pool",
        });

        let account = account_map.get(&pool_address).ok_or_else(|| {
            AppError::Rpc(format!("missing fetched pool account {pool_address}"))
        })?;
        collect_dependent_targets(
            &config,
            pool,
            account,
            &mut targets,
            &mut dependent_addresses,
        )?;
    }

    dependent_addresses.sort();
    dependent_addresses.dedup();
    let dependent_accounts = rpc.get_multiple_accounts(&dependent_addresses).await?;
    for account in dependent_accounts {
        account_map.insert(account.address.clone(), account);
    }

    std::fs::create_dir_all(&output_dir)?;
    for target in &targets {
        let account = account_map.get(&target.address).ok_or_else(|| {
            AppError::Rpc(format!(
                "missing fetched fixture account {}",
                target.address
            ))
        })?;
        write_fixture(Path::new(&output_dir), target, account)?;
    }

    Ok(())
}

fn collect_dependent_targets(
    config: &AppConfig,
    pool: &PoolConfig,
    account: &AccountData,
    targets: &mut Vec<FixtureTarget>,
    dependent_addresses: &mut Vec<String>,
) -> Result<(), AppError> {
    match pool.dex {
        DexKind::Raydium => {
            let meta = raydium::decode_pool_meta(&account.data)?;
            push_dependency(pool, targets, dependent_addresses, meta.base_vault, "base_vault");
            push_dependency(pool, targets, dependent_addresses, meta.quote_vault, "quote_vault");
        }
        DexKind::Orca => {
            let meta = orca::decode_pool_meta(&account.data)?;
            let (base_vault, quote_vault) = orca::vault_addresses_for_config(pool, &meta)?;
            push_dependency(
                pool,
                targets,
                dependent_addresses,
                meta.token_mint_a,
                "token_mint_a",
            );
            push_dependency(
                pool,
                targets,
                dependent_addresses,
                meta.token_mint_b,
                "token_mint_b",
            );
            push_dependency(pool, targets, dependent_addresses, base_vault, "base_vault");
            push_dependency(pool, targets, dependent_addresses, quote_vault, "quote_vault");
        }
        DexKind::MeteoraDlmm => {
            let meta = meteora::decode_pool_meta(&account.data)?;
            push_dependency(
                pool,
                targets,
                dependent_addresses,
                meta.token_x_mint,
                "token_x_mint",
            );
            push_dependency(
                pool,
                targets,
                dependent_addresses,
                meta.token_y_mint,
                "token_y_mint",
            );
            if config.pricing.consider_slippage {
                // quote用BinArrayのPDA解決は公式SDKヘルパーに任せ、取得自体は既存RPC経路にそろえる。
                let quotes = meteora::quote_both_directions_with_official_sdk(
                    pool,
                    &config.helius_rpc_url,
                    config.pricing.trade_size_usdc,
                    config.pricing.meteora_dlmm_bin_array_count,
                    config.pricing.meteora_dlmm_slippage_bps,
                );
                for quote in quotes {
                    let role = match quote.direction {
                        MeteoraDlmmQuoteDirection::UsdcToSol => "bin_array_usdc_to_sol",
                        MeteoraDlmmQuoteDirection::SolToUsdc => "bin_array_sol_to_usdc",
                    };
                    for address in quote.bin_array_addresses {
                        push_dependency(pool, targets, dependent_addresses, address, role);
                    }
                }
            }
        }
    }
    Ok(())
}

fn push_dependency(
    pool: &PoolConfig,
    targets: &mut Vec<FixtureTarget>,
    dependent_addresses: &mut Vec<String>,
    address: String,
    role: &'static str,
) {
    dependent_addresses.push(address.clone());
    targets.push(FixtureTarget {
        address,
        dex: pool.dex,
        pair: pool.pair.clone(),
        role,
    });
}

fn write_fixture(
    output_dir: &Path,
    target: &FixtureTarget,
    account: &AccountData,
) -> Result<(), AppError> {
    let fixture = AccountFixture {
        requested_address: &target.address,
        rpc_method: "getMultipleAccounts",
        context_slot: account.slot,
        owner: &account.owner,
        lamports: account.lamports,
        data_base64: base64::engine::general_purpose::STANDARD.encode(&account.data),
        fetched_at: Utc::now().to_rfc3339(),
        dex: target.dex.as_str(),
        pair: &target.pair,
        role: target.role,
    };
    let name = format!(
        "{}_{}_{}.json",
        dex_file_name(target.dex),
        target.role,
        target.address
    );
    let path = output_dir.join(name);
    let body = serde_json::to_vec_pretty(&fixture)?;
    std::fs::write(path, body)?;
    Ok(())
}

fn dex_file_name(dex: DexKind) -> &'static str {
    match dex {
        DexKind::Raydium => "raydium",
        DexKind::Orca => "orca",
        DexKind::MeteoraDlmm => "meteora_dlmm",
    }
}
