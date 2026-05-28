use crate::utils::{config, horizon, notifications, print as p, soroban, stream::SorobanEventStream};
use anyhow::Result;
use clap::Args;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

#[derive(Args)]
pub struct MonitorArgs {
    /// Contract ID (starts with 'C...') to monitor via Soroban RPC getEvents
    #[arg(long)]
    pub contract: Option<String>,

    /// Comma-separated list of event names to filter (best-effort; matches topic strings)
    #[arg(long)]
    pub events: Option<String>,

    /// Stream events continuously until Ctrl+C (contract mode)
    #[arg(long)]
    pub follow: bool,

    /// Wallet name from starforge config to monitor
    #[arg(long)]
    pub wallet: Option<String>,

    /// Threshold amount in XLM to trigger a notification (wallet mode)
    #[arg(long)]
    pub threshold: Option<f64>,

    /// Network to use (overrides config)
    #[arg(long)]
    pub network: Option<String>,

    /// Poll interval in seconds
    #[arg(long, default_value = "2")]
    pub interval: u64,
}

pub fn handle(args: MonitorArgs) -> Result<()> {
    let cfg = config::load()?;
    let network = args.network.as_deref().unwrap_or(&cfg.network);
    config::validate_network(network)?;

    p::header("Real-time Monitoring");
    p::separator();
    p::kv("Network", network);
    p::separator();
    println!();

    match (&args.contract, &args.wallet) {
        (Some(contract_id), None) => {
            monitor_contract(contract_id, args.events.as_deref(), network, args.interval)
        }
        (None, Some(wallet_name)) => {
            monitor_wallet(wallet_name, args.threshold, network, args.interval)
        }
        _ => anyhow::bail!("Specify either --contract or --wallet (but not both)"),
    }
}

fn monitor_contract(
    contract_id: &str,
    events_filter: Option<&str>,
    network: &str,
    interval: u64,
    follow: bool,
) -> Result<()> {
    config::validate_contract_id(contract_id)?;

    let filter_set: Option<Vec<String>> = events_filter.map(|s| {
        s.split(',')
            .map(|x| x.trim().to_lowercase())
            .filter(|x| !x.is_empty())
            .collect()
    });

    let rpc_url = soroban::rpc_url(network);

    notifications::info(&format!(
        "Streaming contract events from {}.",
        rpc_url
    ));

    let mut stream =
        SorobanEventStream::new(rpc_url, contract_id.to_string()).with_poll_interval(interval);
    loop {
        let batch = stream.next_batch()?;
        for event in batch {
            let as_text = event.value.to_string();
            if let Some(ref filters) = filter_set {
                let mut matches = false;
                for f in filters {
                    if as_text.to_lowercase().contains(f) {
                        matches = true;
                        break;
                    }
                    printed_any = true;
                    notifications::success(&format!(
                        "Ledger {} event {}: {}",
                        event.ledger, event.id, as_text
                    ));
                }

                if !follow {
                    break;
                }
                stream.sleep();
            }
            Err(err) => {
                if !follow && !printed_any {
                    return Err(err);
                }
                notifications::warn(&format!(
                    "Event stream error: {}. Reconnecting with backoff…",
                    err
                ));
                stream.sleep_backoff();
            }
        }
    }

    Ok(())
}

fn monitor_wallet(
    wallet_name: &str,
    threshold: Option<f64>,
    network: &str,
    interval: u64,
) -> Result<()> {
    let cfg = config::load()?;
    let wallet = cfg
        .wallets
        .iter()
        .find(|w| w.name == wallet_name)
        .ok_or_else(|| anyhow::anyhow!("Wallet '{}' not found", wallet_name))?;

    let threshold = threshold.unwrap_or(0.0);
    if threshold <= 0.0 {
        notifications::warn("No --threshold provided; will print balance changes only.");
    }

    notifications::info(&format!(
        "Monitoring wallet {} ({}) on {}.",
        wallet.name, wallet.public_key, network
    ));

    let mut last_balance: Option<f64> = None;
    loop {
        let account = horizon::fetch_account(&wallet.public_key, network)?;
        let native = account
            .balances
            .iter()
            .find(|b| b.asset_type == "native")
            .and_then(|b| b.balance.parse::<f64>().ok())
            .unwrap_or(0.0);

        if last_balance
            .map(|b| (b - native).abs() > f64::EPSILON)
            .unwrap_or(true)
        {
            notifications::info(&format!("XLM balance: {:.7}", native));
            last_balance = Some(native);
        }

        if threshold > 0.0 && native >= threshold {
            notifications::success(&format!(
                "Threshold met: {:.7} XLM (>= {:.7})",
                native, threshold
            ));
        }

        std::thread::sleep(std::time::Duration::from_secs(interval.max(1)));
    }
}
