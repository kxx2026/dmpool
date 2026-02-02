// Admin API module for DMPool
// Provides web-based management interface

use anyhow::Result;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use p2poolv2_lib::config::Config;
use p2poolv2_lib::store::Store;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Admin state
#[derive(Clone)]
pub struct AdminState {
    pub config: Arc<RwLock<Config>>,
    pub store: Arc<Store>,
}

/// Dashboard metrics
#[derive(Debug, Serialize, Deserialize)]
pub struct DashboardMetrics {
    pub pool_hashrate_ths: f64,
    pub active_workers: u64,
    pub total_shares: u64,
    pub blocks_found: u64,
    pub uptime_seconds: u64,
    pub pplns_window_shares: u64,
    pub current_difficulty: f64,
}

/// Configuration view (safe for display)
#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigView {
    pub stratum_port: u16,
    pub stratum_hostname: String,
    pub start_difficulty: u32,
    pub minimum_difficulty: u32,
    pub pplns_ttl_days: u32,
    pub difficulty_multiplier: f64,
    pub pool_signature: Option<String>,
    pub network: String,
    pub donation: Option<u16>,
    pub fee: Option<u16>,
    pub ignore_difficulty: bool,
}

/// Worker info
#[derive(Debug, Serialize, Deserialize)]
pub struct WorkerInfo {
    pub address: String,
    pub worker_name: String,
    pub hashrate_ths: f64,
    pub shares_count: u64,
    pub last_seen: String,
    pub is_banned: bool,
}

/// Create admin API router
pub fn create_admin_router() -> Router<AdminState> {
    Router::new()
        .route("/api/admin/dashboard", get(dashboard_handler))
        .route("/api/admin/config", get(config_handler).post(update_config_handler))
        .route("/api/admin/workers", get(workers_handler))
        .route("/api/admin/health", get(admin_health_handler))
        .route("/api/admin/reload", get(reload_config_handler))
}

/// Dashboard metrics handler
async fn dashboard_handler(State(state): State<AdminState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let store = &state.store;

    // Get basic metrics
    let tip = store.get_chain_tip();
    let height = store.get_tip_height();

    // Get shares count (this is a simplified version)
    let total_shares = match store.get_n_shares(100) {
        Ok(shares) => shares.len() as u64,
        Err(_) => 0,
    };

    let metrics = DashboardMetrics {
        pool_hashrate_ths: 0.0, // TODO: Calculate from shares
        active_workers: 0,       // TODO: Get from tracker
        total_shares,
        blocks_found: height,
        uptime_seconds: 0,       // TODO: Track uptime
        pplns_window_shares: 0,  // TODO: Get from PPLNS state
        current_difficulty: 1.0, // TODO: Get current network difficulty
    };

    Json(metrics)
}

/// Get current configuration (safe view)
async fn config_handler(State(state): State<AdminState>) -> impl IntoResponse {
    let config = state.config.read().await;

    let view = ConfigView {
        stratum_port: config.stratum.port,
        stratum_hostname: config.stratum.hostname.clone(),
        start_difficulty: config.stratum.start_difficulty,
        minimum_difficulty: config.stratum.minimum_difficulty,
        pplns_ttl_days: config.store.pplns_ttl_days,
        difficulty_multiplier: 1.0, // TODO: Get from config
        pool_signature: config.stratum.pool_signature.clone(),
        network: config.stratum.network.to_string(),
        donation: config.stratum.donation,
        fee: None, // TODO: Get from config
        ignore_difficulty: config.stratum.ignore_difficulty.unwrap_or(false),
    };

    Json(view)
}

/// Update configuration (selected parameters only)
#[allow(unused_variables)]
async fn update_config_handler(
    State(state): State<AdminState>,
    Json(update): Json<ConfigUpdate>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;

    // Only allow safe updates at runtime
    if let Some(difficulty) = update.start_difficulty {
        if difficulty >= 8 && difficulty <= 512 {
            config.stratum.start_difficulty = difficulty;
            info!("Updated start_difficulty to {}", difficulty);
        }
    }

    if let Some(difficulty) = update.minimum_difficulty {
        if difficulty >= 8 && difficulty <= 256 {
            config.stratum.minimum_difficulty = difficulty;
            info!("Updated minimum_difficulty to {}", difficulty);
        }
    }

    if let Some(signature) = update.pool_signature {
        if signature.len() <= 16 {
            config.stratum.pool_signature = Some(signature);
            info!("Updated pool_signature");
        }
    }

    // Note: This doesn't persist to file, just runtime update
    // For persistence, write to config file and trigger reload

    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "message": "Configuration updated (runtime only, restart required for persistence)"
    })))
}

/// List workers
async fn workers_handler(State(state): State<AdminState>) -> impl IntoResponse {
    let store = &state.store;

    // Get recent shares to identify workers
    let shares = match store.get_n_shares(100) {
        Ok(s) => s,
        Err(_) => Vec::new(),
    };

    // Group by worker
    let mut workers: std::collections::HashMap<String, WorkerInfo> = std::collections::HashMap::new();

    for share in shares {
        let key = format!("{}{}", share.miner_txid, share.miner_msg);
        workers.entry(key).or_insert_with(|| WorkerInfo {
            address: share.miner_txid,
            worker_name: share.miner_msg,
            hashrate_ths: 0.0,
            shares_count: 0,
            last_seen: String::new(),
            is_banned: false,
        }).shares_count += 1;
    }

    Json(workers.into_values().collect::<Vec<_>>())
}

/// Admin health check
async fn admin_health_handler() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "healthy",
        "message": "Admin API is operational"
    }))
}

/// Reload configuration from file
async fn reload_config_handler(State(state): State<AdminState>) -> impl IntoResponse {
    // TODO: Implement config reload from file
    info!("Config reload requested");

    (StatusCode::OK, Json(serde_json::json!({
        "status": "ok",
        "message": "Config reload triggered"
    })))
}

/// Configuration update request
#[derive(Debug, Deserialize)]
pub struct ConfigUpdate {
    pub start_difficulty: Option<u32>,
    pub minimum_difficulty: Option<u32>,
    pub pool_signature: Option<String>,
    // Note: pplns_ttl_days and ignore_difficulty require restart
}

/// Serve admin panel
pub async fn serve_admin_panel(port: u16, state: AdminState) -> Result<()> {
    let app = Router::new()
        .nest_service("/admin", create_admin_router().with_state(state.clone()))
        .route("/", get(admin_index_handler))
        .fallback(admin_static_handler);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("Admin panel listening on port {}", port);

    axum::serve(listener, app).await?;

    Ok(())
}

/// Admin index page handler
async fn admin_index_handler() -> impl IntoResponse {
    let html = include_str!("../../static/admin/index.html");
    axum::response::Html(html)
}

/// Serve static files
async fn admin_static_handler() -> impl IntoResponse {
    axum::response::Html("<h1>File not found</h1>")
}
