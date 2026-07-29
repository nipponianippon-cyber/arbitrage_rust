mod config;
mod dex;
mod errors;
mod notifier;
mod pricing;
mod rpc;
mod runner;
mod storage;

use crate::config::load_config;
use crate::notifier::DiscordNotifier;
use crate::rpc::RpcClient;
use crate::storage::Storage;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    if let Err(error) = start().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn start() -> Result<(), errors::AppError> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = load_config("config.toml")?;
    let storage = Storage::open(&config.database.path)?;
    storage.init_schema()?;

    let rpc = RpcClient::new(config.helius_rpc_url.clone());
    let notifier = DiscordNotifier::new(config.discord_webhook_url.clone(), &config.notification);

    runner::run_forever(config, rpc, storage, notifier).await
}
