// DMPool Admin Server
// Standalone admin web interface for pool management

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json},
    routing::get,
    Router,
};
use p2poolv2_lib::config::Config;
use p2poolv2_lib::store::Store;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, Level};

/// Admin state
#[derive(Clone)]
struct AdminState {
    config_path: String,
    config: Arc<RwLock<Config>>,
    store: Arc<Store>,
}

/// Dashboard metrics
#[derive(Serialize)]
struct DashboardMetrics {
    pool_hashrate_ths: f64,
    active_workers: u64,
    total_shares: u64,
    blocks_found: u64,
    uptime_seconds: u64,
}

/// Configuration view
#[derive(Serialize)]
struct ConfigView {
    stratum_port: u16,
    stratum_hostname: String,
    start_difficulty: u32,
    minimum_difficulty: u32,
    pplns_ttl_days: u32,
    network: String,
    pool_signature: Option<String>,
    ignore_difficulty: bool,
    donation: Option<u16>,
}

/// Config update request
#[derive(Deserialize)]
struct ConfigUpdate {
    start_difficulty: Option<u32>,
    minimum_difficulty: Option<u32>,
    pool_signature: Option<String>,
}

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let config_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let port: u16 = std::env::var("ADMIN_PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse()
        .unwrap_or(8080);

    // Load config
    let config = Config::load(&config_path)?;
    let store = Arc::new(Store::new(config.store.path.clone(), true)?);

    let state = AdminState {
        config_path,
        config: Arc::new(RwLock::new(config)),
        store,
    };

    // Create router
    let app = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/dashboard", get(dashboard))
        .route("/api/config", get(get_config).post(update_config))
        .route("/api/workers", get(workers))
        .route("/api/config/reload", get(reload_config))
        .fallback(not_found);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("DMPool Admin Server listening on port {}", port);
    info!("Access admin panel at http://localhost:{}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Serve admin panel index
async fn index() -> impl IntoResponse {
    let html = include_str!("../../static/admin/index.html");
    Html(html)
}

/// Health check
async fn health() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "dmpool-admin"
    }))
}

/// Get dashboard metrics
async fn dashboard(State(state): State<AdminState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let tip = state.store.get_chain_tip();
    let height = state.store.get_tip_height();

    // Get recent shares count
    let total_shares = match state.store.get_n_shares(100) {
        Ok(shares) => shares.len() as u64,
        Err(_) => 0,
    };

    let metrics = DashboardMetrics {
        pool_hashrate_ths: 0.0,  // TODO: Calculate from metrics
        active_workers: 0,        // TODO: Get from tracker
        total_shares,
        blocks_found: height,
        uptime_seconds: 0,        // TODO: Track uptime
    };

    Json(metrics)
}

/// Get current configuration
async fn get_config(State(state): State<AdminState>) -> impl IntoResponse {
    let config = state.config.read().await;

    let view = ConfigView {
        stratum_port: config.stratum.port,
        stratum_hostname: config.stratum.hostname.clone(),
        start_difficulty: config.stratum.start_difficulty,
        minimum_difficulty: config.stratum.minimum_difficulty,
        pplns_ttl_days: config.store.pplns_ttl_days,
        network: config.stratum.network.to_string(),
        pool_signature: config.stratum.pool_signature.clone(),
        ignore_difficulty: config.stratum.ignore_difficulty.unwrap_or(false),
        donation: config.stratum.donation,
    };

    Json(view)
}

/// Update configuration (runtime only)
async fn update_config(
    State(state): State<AdminState>,
    Json(update): Json<ConfigUpdate>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;

    // Update allowed fields
    if let Some(diff) = update.start_difficulty {
        if diff >= 8 && diff <= 512 {
            config.stratum.start_difficulty = diff;
            info!("Updated start_difficulty to {}", diff);
        }
    }

    if let Some(diff) = update.minimum_difficulty {
        if diff >= 8 && diff <= 256 {
            config.stratum.minimum_difficulty = diff;
            info!("Updated minimum_difficulty to {}", diff);
        }
    }

    if let Some(signature) = update.pool_signature {
        if signature.len() <= 16 {
            config.stratum.pool_signature = Some(signature);
            info!("Updated pool_signature");
        }
    }

    Json(serde_json::json!({
        "status": "ok",
        "message": "Configuration updated (runtime only)"
    }))
}

/// Get workers list
async fn workers(State(state): State<AdminState>) -> impl IntoResponse {
    let shares = match state.store.get_n_shares(100) {
        Ok(s) => s,
        Err(_) => Vec::new(),
    };

    // Simple worker aggregation
    let workers: Vec<serde_json::Value> = shares
        .into_iter()
        .map(|s| {
            serde_json::json!({
                "address": s.miner_txid,
                "worker": s.miner_msg,
                "hashrate_ths": 0.0,
                "shares_count": 1,
                "is_banned": false
            })
        })
        .collect();

    Json(workers)
}

/// Reload configuration from file
async fn reload_config(State(state): State<AdminState>) -> impl IntoResponse {
    // Reload config from file
    match Config::load(&state.config_path) {
        Ok(new_config) => {
            *state.config.write().await = new_config;
            info!("Configuration reloaded from file");
            Json(serde_json::json!({
                "status": "ok",
                "message": "Configuration reloaded"
            }))
        }
        Err(e) => {
            tracing::error!("Failed to reload config: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                "status": "error",
                "message": format!("Failed to reload: {}", e)
            })))
        }
    }
}

/// 404 handler
async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}
