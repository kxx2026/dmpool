use anyhow::Result;
use dmpool::health::{HealthChecker, HealthStatus, ComponentStatus, BitcoinNodeStatus, StratumStatus, BlockchainInfo, NetworkInfo};
use p2poolv2_lib::config::Config;
use std::env;
use axum::{Json, Router, routing::get};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    println!("DMPool Health Check Service starting...");

    let config_path = env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let config = Config::load(&config_path)
        .map_err(|e| anyhow::anyhow!("Failed to load config: {}", e))?;

    let health_checker = HealthChecker::new(config.clone());

    let port = env::var("HEALTH_PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("0.0.0.0:{}", port);

    let app = Router::new()
        .route("/health", get(health_handler))
        .route("/ready", get(ready_handler));

    let listener = TcpListener::bind(&addr).await?;
    println!("Health check service listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_handler() -> Json<HealthStatus> {
    Json(HealthStatus {
        status: "healthy".to_string(),
        database: ComponentStatus::healthy(),
        bitcoin_node: BitcoinNodeStatus {
            status: "unknown".to_string(),
            rpc_latency_ms: None,
            blockchain: BlockchainInfo {
                blocks: 0,
                headers: 0,
                initial_block_download: false,
                verification_progress: 0.0,
                block_time_seconds: None,
                best_block_hash: "".to_string(),
            },
            network: NetworkInfo {
                connections: 0,
                network_active: false,
                peer_count: 0,
            },
            sync_progress: 0.0,
            message: "Not initialized".to_string(),
        },
        stratum: StratumStatus {
            status: "unknown".to_string(),
            listening: false,
            active_connections: 0,
            shares_per_second: 0.0,
            current_difficulty: 0.0,
            message: "Not initialized".to_string(),
        },
        zmq: ComponentStatus {
            status: "unknown".to_string(),
            message: "Not initialized".to_string(),
            latency_ms: None,
        },
        uptime_seconds: 0,
        memory_mb: None,
    })
}

async fn ready_handler() -> &'static str {
    "OK"
}
