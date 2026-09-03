use arbitrage_rust::config::load_config;
use arbitrage_rust::notifier::DiscordNotifier;
use arbitrage_rust::rpc::RpcClient;
use arbitrage_rust::runner;
use arbitrage_rust::storage::Storage;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    if let Err(error) = start().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn start() -> Result<(), arbitrage_rust::errors::AppError> {
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
