//! `omny tower run` — CLI binary for running a Tower node.
//!
//! Usage:
//!   omny-tower [OPTIONS]
//!
//! Options:
//!   --mode pharos|harbor|intermediary  Operating mode (default: pharos)
//!   --name NAME            Tower name (default: "Omnidea Tower")
//!   --port PORT            Relay port (default: 7777)
//!   --data-dir PATH        Data directory (default: ./tower_data)
//!   --seed SEED_URL        Seed peer URL (can be repeated)
//!   --public-url URL       Public URL for announcements
//!   --community PUBKEY     Community pubkey to serve (Harbor, can be repeated)
//!   --upstream URL         Upstream relay URL (Intermediary mode, required)
//!   --config PATH          Path to config JSON file

use std::path::PathBuf;
use std::time::Duration;

use tower::{Tower, TowerConfig, TowerMode};
use url::Url;

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .init();

    let config = match parse_args() {
        Ok(config) => config,
        Err(msg) => {
            eprintln!("error: {msg}");
            print_usage();
            std::process::exit(1);
        }
    };

    log::info!(
        "starting Tower node: mode={}, name={}, port={}",
        config.mode,
        config.name,
        config.port
    );
    log::info!("data directory: {}", config.data_dir.display());
    if !config.seed_peers.is_empty() {
        log::info!("seed peers: {}", config.seed_peers.len());
    }
    if config.mode == TowerMode::Harbor && !config.communities.is_empty() {
        log::info!("serving {} communities", config.communities.len());
    }
    if config.mode == TowerMode::Intermediary {
        if let Some(ref upstream) = config.upstream_relay {
            log::info!("intermediary forwarding to: {upstream}");
        }
    }

    let tower = match Tower::start(config.clone()) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("fatal: failed to start Tower: {e}");
            std::process::exit(1);
        }
    };

    let status = tower.status();
    log::info!("Tower online:");
    log::info!("  mode:    {}", status.mode);
    log::info!("  relay:   {}", status.relay_url);
    log::info!("  pubkey:  {}", status.pubkey.as_deref().unwrap_or("none"));
    log::info!("  peers:   {}", status.gospel_peers);

    // Initial announcement.
    if let Err(e) = tower.announce() {
        log::warn!("initial announcement failed: {e}");
    }

    // Wrap in Arc for spawn_blocking closures inside the async loop.
    let tower = std::sync::Arc::new(tower);

    // Run the main loop on Omnibus's runtime (don't create a second one).
    tower.omnibus().runtime().block_on(async {
        let announce_interval = Duration::from_secs(config.announce_interval_secs);
        let mut announce_tick = tokio::time::interval(announce_interval);
        announce_tick.tick().await; // skip immediate tick (we already announced)

        let gospel_interval = Duration::from_secs(config.gospel_interval_secs);
        let mut gospel_tick = tokio::time::interval(gospel_interval);
        gospel_tick.tick().await; // skip immediate tick

        // Fast ticker for live gospel sync (default 2s).
        let live_interval = Duration::from_secs(config.gospel_live_interval_secs);
        let mut live_tick = tokio::time::interval(live_interval);
        live_tick.tick().await; // skip immediate tick

        log::info!("Tower running. Press Ctrl+C to stop.");

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    log::info!("shutting down...");
                    break;
                }
                _ = announce_tick.tick() => {
                    // announce() → omnibus.publish() → runtime.block_on()
                    // We're inside block_on(), so offload to avoid nesting.
                    let t = tower.clone();
                    match tokio::task::spawn_blocking(move || t.announce()).await {
                        Ok(Err(e)) => log::warn!("announcement failed: {e}"),
                        Err(e) => log::warn!("announce task panicked: {e}"),
                        _ => {}
                    }
                }
                _ = gospel_tick.tick() => {
                    tower.run_gospel_cycle().await;
                }
                _ = live_tick.tick() => {
                    let t = tower.clone();
                    let _ = tokio::task::spawn_blocking(move || {
                        t.process_live_events();
                    }).await;
                }
            }
        }
    });

    let status = tower.status();
    log::info!("Tower shut down after {}s uptime", status.uptime_secs);
}

fn parse_args() -> Result<TowerConfig, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut config = TowerConfig::default();

    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                i += 1;
                let mode_str = args.get(i).ok_or("--mode requires a value")?;
                config.mode = match mode_str.as_str() {
                    "pharos" => TowerMode::Pharos,
                    "harbor" => TowerMode::Harbor,
                    "intermediary" => TowerMode::Intermediary,
                    other => return Err(format!("unknown mode: {other} (use pharos, harbor, or intermediary)")),
                };
            }
            "--name" => {
                i += 1;
                config.name = args.get(i).ok_or("--name requires a value")?.clone();
            }
            "--port" => {
                i += 1;
                config.port = args
                    .get(i)
                    .ok_or("--port requires a value")?
                    .parse()
                    .map_err(|_| "invalid port number")?;
            }
            "--data-dir" => {
                i += 1;
                config.data_dir = PathBuf::from(
                    args.get(i).ok_or("--data-dir requires a value")?,
                );
            }
            "--seed" => {
                i += 1;
                let url_str = args.get(i).ok_or("--seed requires a URL")?;
                let url: Url = url_str
                    .parse()
                    .map_err(|e| format!("invalid seed URL: {e}"))?;
                config.seed_peers.push(url);
            }
            "--public-url" => {
                i += 1;
                let url_str = args.get(i).ok_or("--public-url requires a URL")?;
                let url: Url = url_str
                    .parse()
                    .map_err(|e| format!("invalid public URL: {e}"))?;
                config.public_url = Some(url);
            }
            "--community" => {
                i += 1;
                let pubkey = args.get(i).ok_or("--community requires a pubkey")?;
                config.communities.push(pubkey.clone());
            }
            "--upstream" => {
                i += 1;
                let url_str = args.get(i).ok_or("--upstream requires a URL")?;
                config.upstream_relay = Some(url_str.clone());
            }
            "--config" => {
                i += 1;
                let path = args.get(i).ok_or("--config requires a path")?;
                let data = std::fs::read_to_string(path)
                    .map_err(|e| format!("read config: {e}"))?;
                config = serde_json::from_str(&data)
                    .map_err(|e| format!("parse config: {e}"))?;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unknown option: {other}")),
        }
        i += 1;
    }

    Ok(config)
}

fn print_usage() {
    eprintln!(
        r#"
omny-tower — Run an Omnidea Tower node

USAGE:
    omny-tower [OPTIONS]

OPTIONS:
    --mode <pharos|harbor|intermediary>  Operating mode (default: pharos)
    --name <NAME>            Tower name (default: "Omnidea Tower")
    --port <PORT>            Relay port (default: 7777)
    --data-dir <PATH>        Data directory (default: ./tower_data)
    --seed <URL>             Seed peer URL (repeatable)
    --public-url <URL>       Public URL for announcements
    --community <PUBKEY>     Community to serve, Harbor only (repeatable)
    --upstream <URL>         Upstream relay URL (Intermediary mode, required)
    --config <PATH>          Load config from JSON file
    --help                   Show this help

EXAMPLES:
    # Run a Pharos node (lightweight directory)
    omny-tower --mode pharos --name "My Pharos" --port 7777

    # Run a Harbor node serving a community
    omny-tower --mode harbor --name "Art Community" --community abc123...

    # Run an Intermediary node (privacy relay)
    omny-tower --mode intermediary --upstream wss://upstream.omnidea.net

    # Connect to seed peers for gospel sync
    omny-tower --seed wss://tower1.omnidea.net --seed wss://tower2.omnidea.net

    # Load from config file
    omny-tower --config tower.json
"#
    );
}
