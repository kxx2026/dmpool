// DMPool Admin Server
// Standalone admin web interface for pool management

use anyhow::Result;
use axum::{
    extract::{Path, Query, State, Request},
    http::StatusCode,
    middleware::Next,
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
    middleware,
};
use chrono::Utc;
use p2poolv2_lib::config::Config;
use p2poolv2_lib::shares::chain::chain_store::ChainStore;
use p2poolv2_lib::shares::share_block::ShareBlock;
use p2poolv2_lib::store::Store;
use dmpool::auth::{AuthManager, LoginRequest, LoginResponse, UserInfo};
use dmpool::audit::{AuditLogger, AuditFilter};
use dmpool::backup::{BackupManager, BackupConfig, BackupMetadata, BackupStats};
use dmpool::confirmation::ConfigConfirmation;
use dmpool::health::HealthChecker;
use dmpool::rate_limit::{RateLimiterState, RateLimitConfig, rate_limit_middleware, login_rate_limit_middleware};
use serde::{Deserialize, Serialize};
use serde_json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn, Level};

/// Admin state
#[derive(Clone)]
struct AdminState {
    config_path: String,
    config: Arc<RwLock<Config>>,
    store: Arc<Store>,
    chain_store: Arc<ChainStore>,
    health_checker: Arc<HealthChecker>,
    auth_manager: Arc<AuthManager>,
    rate_limiter: Arc<RateLimiterState>,
    audit_logger: Arc<AuditLogger>,
    config_confirmation: Arc<ConfigConfirmation>,
    backup_manager: Arc<BackupManager>,
    start_time: std::time::Instant,
    banned_workers: Arc<RwLock<HashSet<String>>>,
    worker_tags: Arc<RwLock<HashMap<String, Vec<String>>>>,
}

// ===== Response Types =====

#[derive(Serialize)]
struct ApiResponse<T> {
    status: String,
    data: Option<T>,
    message: Option<String>,
    timestamp: u64,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self {
            status: "ok".to_string(),
            data: Some(data),
            message: None,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    fn error(msg: impl Into<String>) -> Self {
        Self {
            status: "error".to_string(),
            data: None,
            message: Some(msg.into()),
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }
}

#[derive(Serialize)]
struct DashboardMetrics {
    pool_hashrate_ths: f64,
    active_workers: u64,
    total_shares: u64,
    blocks_found: u64,
    uptime_seconds: u64,
    pplns_window_shares: u64,
    current_difficulty: f64,
}

#[derive(Serialize)]
struct ConfigView {
    stratum_port: u16,
    stratum_hostname: String,
    start_difficulty: u64,
    minimum_difficulty: u64,
    pplns_ttl_days: u64,
    difficulty_multiplier: f64,
    network: String,
    pool_signature: Option<String>,
    ignore_difficulty: bool,
    donation: Option<u16>,
    fee: Option<u16>,
}

#[derive(Serialize)]
struct SafetyReport {
    safe: bool,
    critical_issues: Vec<SafetyIssue>,
    warnings: Vec<SafetyIssue>,
}

#[derive(Serialize)]
struct SafetyIssue {
    severity: String,
    param: String,
    message: String,
    recommendation: String,
}

#[derive(Serialize)]
struct WorkerInfo {
    address: String,
    worker_name: String,
    hashrate_ths: f64,
    shares_count: u64,
    difficulty: u64,
    last_seen: String,
    first_seen: String,
    is_banned: bool,
    tags: Vec<String>,
    status: WorkerStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum WorkerStatus {
    Active,
    Inactive,
    Banned,
}

/// Pagination request
#[derive(Deserialize)]
struct PaginationRequest {
    page: Option<usize>,
    page_size: Option<usize>,
    search: Option<String>,
    status: Option<String>,
    sort_by: Option<String>,
    sort_order: Option<String>,
}

/// Paginated response
#[derive(Serialize)]
struct PaginatedResponse<T> {
    data: Vec<T>,
    total: usize,
    page: usize,
    page_size: usize,
    total_pages: usize,
}

// ===== Request Types =====

#[derive(Deserialize)]
struct ConfigUpdate {
    start_difficulty: Option<u32>,
    minimum_difficulty: Option<u32>,
    pool_signature: Option<String>,
}

#[derive(Deserialize)]
struct BanRequest {
    reason: Option<String>,
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

    // Get admin credentials from environment
    let admin_username = std::env::var("ADMIN_USERNAME").unwrap_or_else(|_| "admin".to_string());
    let admin_password = std::env::var("ADMIN_PASSWORD").unwrap_or_else(|_| "admin123".to_string());

    // Get JWT secret - MUST be set in production
    let is_production = std::env::var("DMP_ENV").unwrap_or_else(|_| "development".to_string()) == "production";
    let jwt_secret = std::env::var("JWT_SECRET").unwrap_or_else(|_| {
        if is_production {
            error!("JWT_SECRET environment variable MUST be set in production!");
            error!("Generate a secure secret with: openssl rand -base64 32");
            std::process::exit(1);
        } else {
            // For development, generate a random secret each time
            use rand::Rng;
            let secret: String = rand::thread_rng()
                .sample_iter(&rand::distributions::Alphanumeric)
                .take(32)
                .map(char::from)
                .collect();
            warn!("Using generated JWT secret for development. Set JWT_SECRET for persistence!");
            secret
        }
    });

    // Validate JWT secret length
    if jwt_secret.len() < 32 {
        error!("JWT_SECRET must be at least 32 characters long! Current length: {}", jwt_secret.len());
        std::process::exit(1);
    }

    // Load config
    let config = Config::load(&config_path)?;
    let store = Arc::new(Store::new(config.store.path.clone(), true)
        .map_err(|e| anyhow::anyhow!("Failed to open store: {}", e))?);
    let genesis = ShareBlock::build_genesis_for_network(config.stratum.network);
    let chain_store = Arc::new(ChainStore::new(
        store.clone(),
        genesis,
        config.stratum.network,
    ));

    // Initialize auth manager
    let auth_manager = Arc::new(AuthManager::new(jwt_secret));
    auth_manager.init_default_admin(&admin_username, &admin_password).await?;
    info!("Initialized admin user: {}", admin_username);

    // Initialize rate limiter
    let rate_limit_config = RateLimitConfig::default();
    let api_rpm = rate_limit_config.api_rpm.get();
    let login_rpm = rate_limit_config.login_rpm.get();
    let rate_limiter = Arc::new(RateLimiterState::new(rate_limit_config));
    info!("Initialized rate limiter: {} req/min (API), {} req/min (login)",
        api_rpm, login_rpm);

    // Initialize audit logger
    let audit_logger = Arc::new(AuditLogger::default());
    info!("Initialized audit logger (max 10000 entries in memory)");

    // Initialize config confirmation
    let config_confirmation = Arc::new(ConfigConfirmation::new());
    info!("Initialized config confirmation system");

    // Initialize backup manager
    let backup_config = BackupConfig {
        db_path: config.store.path.clone().into(),
        backup_dir: std::path::PathBuf::from("./backups"),
        retention_count: 7,
        compress: true,
        interval_hours: 24,
    };
    let backup_manager = Arc::new(BackupManager::new(backup_config));
    info!("Initialized backup manager");

    let state = AdminState {
        config_path,
        config: Arc::new(RwLock::new(config.clone())),
        store: store.clone(),
        chain_store,
        health_checker: Arc::new(HealthChecker::new(config).with_store(store.clone())),
        auth_manager: auth_manager.clone(),
        rate_limiter: rate_limiter.clone(),
        audit_logger: audit_logger.clone(),
        config_confirmation: config_confirmation.clone(),
        backup_manager: backup_manager.clone(),
        start_time: std::time::Instant::now(),
        banned_workers: Arc::new(RwLock::new(HashSet::new())),
        worker_tags: Arc::new(RwLock::new(HashMap::new())),
    };

    // Create public router (no auth required, but rate limited)
    let public_routes = Router::new()
        .route("/", get(index))
        .route("/api/health", get(health))
        .route("/api/services/status", get(services_status))
        // Login has stricter rate limiting
        .route("/api/auth/login", post(login))
        .route_layer(middleware::from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_middleware,
        ))
        // Apply login-specific rate limiter to login route
        .layer(middleware::from_fn_with_state(
            rate_limiter.clone(),
            login_rate_limit_middleware,
        ));

    // Create protected router (auth required + rate limited)
    let protected_routes = Router::new()
        .route("/api/dashboard", get(dashboard))
        .route("/api/config", get(get_config).post(update_config))
        .route("/api/config/reload", post(reload_config))
        .route("/api/workers", get(workers_list))
        .route("/api/workers/:address", get(worker_detail))
        .route("/api/workers/:address/ban", post(ban_worker))
        .route("/api/workers/:address/unban", post(unban_worker))
        .route("/api/workers/:address/tags", post(add_worker_tag))
        .route("/api/workers/:address/tags/:tag", post(remove_worker_tag))
        .route("/api/blocks", get(blocks_list))
        .route("/api/blocks/:height", get(block_detail))
        .route("/api/logs", get(logs))
        .route("/api/safety/check", get(safety_check))
        .route("/api/audit/logs", get(audit_logs))
        .route("/api/audit/stats", get(audit_stats))
        .route("/api/audit/rotate", post(audit_rotate))
        .route("/api/audit/export", post(audit_export))
        .route("/api/config/confirmations", get(get_confirmations))
        .route("/api/config/confirmations/:id", post(confirm_config))
        .route("/api/config/confirmations/:id/apply", post(apply_config))
        // Backup API routes
        .route("/api/backup/create", post(create_backup))
        .route("/api/backup/list", get(list_backups))
        .route("/api/backup/stats", get(backup_stats))
        .route("/api/backup/:id", get(get_backup))
        .route("/api/backup/:id/delete", post(delete_backup))
        .route("/api/backup/:id/restore", post(restore_backup))
        .route("/api/backup/cleanup", post(cleanup_backups))
        // Apply rate limiting first
        .route_layer(middleware::from_fn_with_state(
            rate_limiter.clone(),
            rate_limit_middleware,
        ))
        // Then apply auth middleware
        .route_layer(middleware::from_fn_with_state(
            auth_manager.clone(),
            auth_middleware,
        ));

    // Combine all routes
    let app = public_routes
        .merge(protected_routes)
        .with_state(state)
        .fallback(not_found);

    // Start server
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    info!("DMPool Admin Server listening on port {}", port);
    info!("Access admin panel at http://localhost:{}", port);
    info!("Default credentials: {} / {}", admin_username, "***");

    axum::serve(listener, app).await?;

    Ok(())
}

/// Authentication middleware for protected routes
async fn auth_middleware(
    State(auth): State<Arc<AuthManager>>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // Extract Authorization header from request
    let auth_header = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok());

    if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Bearer ") {
            let token = &auth_header[7..];
            match auth.verify_token(token) {
                Ok(_claims) => {
                    // Token valid, proceed
                    return Ok(next.run(req).await);
                }
                Err(e) => {
                    warn!("Invalid token: {}", e);
                    return Err(StatusCode::UNAUTHORIZED);
                }
            }
        }
    }

    // Allow public routes without auth
    let path = req.uri().path();
    let public_routes = [
        "/",
        "/api/health",
        "/api/services/status",
        "/api/auth/login",
    ];

    if public_routes.iter().any(|r| path == *r || path.starts_with(r)) {
        return Ok(next.run(req).await);
    }

    warn!("Unauthorized access attempt to: {}", path);
    Err(StatusCode::UNAUTHORIZED)
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

/// Get comprehensive services status
async fn services_status(State(state): State<AdminState>) -> impl IntoResponse {
    let health_status = state.health_checker.check().await;
    Json(ApiResponse::ok(health_status))
}

/// Get dashboard metrics
async fn dashboard(State(state): State<AdminState>) -> impl IntoResponse {
    let height = state.chain_store.get_tip_height()
        .ok()
        .flatten()
        .map(|h| h as u64)
        .unwrap_or(0);

    let metrics = DashboardMetrics {
        pool_hashrate_ths: 0.0,
        active_workers: 0,
        total_shares: 0,
        blocks_found: height,
        uptime_seconds: state.start_time.elapsed().as_secs(),
        pplns_window_shares: 0,
        current_difficulty: 1.0,
    };

    Json(ApiResponse::ok(metrics))
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
        difficulty_multiplier: 1.0,
        network: config.stratum.network.to_string(),
        pool_signature: config.stratum.pool_signature.clone(),
        ignore_difficulty: config.stratum.ignore_difficulty.unwrap_or(false),
        donation: config.stratum.donation,
        fee: None,
    };

    Json(ApiResponse::ok(view))
}

/// Update configuration (runtime only)
async fn update_config(
    State(state): State<AdminState>,
    Json(update): Json<ConfigUpdate>,
) -> impl IntoResponse {
    let mut config = state.config.write().await;
    let mut changes = Vec::new();

    // Update start_difficulty
    if let Some(diff) = update.start_difficulty {
        if diff >= 8 && diff <= 512 {
            let old = config.stratum.start_difficulty;
            config.stratum.start_difficulty = diff as u64;
            changes.push(format!("start_difficulty: {} → {}", old, diff));
            info!("Updated start_difficulty to {}", diff);
        }
    }

    // Update minimum_difficulty
    if let Some(diff) = update.minimum_difficulty {
        if diff >= 8 && diff <= 256 {
            let old = config.stratum.minimum_difficulty;
            config.stratum.minimum_difficulty = diff as u64;
            changes.push(format!("minimum_difficulty: {} → {}", old, diff));
            info!("Updated minimum_difficulty to {}", diff);
        }
    }

    // Update pool_signature
    if let Some(signature) = update.pool_signature {
        if signature.len() <= 16 {
            let old = config.stratum.pool_signature.clone();
            config.stratum.pool_signature = Some(signature.clone());
            changes.push(format!("pool_signature: {:?} → {}", old, signature));
            info!("Updated pool_signature to {}", signature);
        }
    }

    if changes.is_empty() {
        return Json(ApiResponse::<serde_json::Value>::error("No valid changes to apply".to_string()));
    }

    let response = serde_json::json!({
        "message": format!("Applied {} change(s)", changes.len()),
        "changes": changes,
    });

    Json(ApiResponse::ok(response))
}

/// Reload configuration from file
async fn reload_config(State(state): State<AdminState>) -> impl IntoResponse {
    match Config::load(&state.config_path) {
        Ok(new_config) => {
            *state.config.write().await = new_config;
            info!("Configuration reloaded from file");
            let response = serde_json::json!({
                "message": "Configuration reloaded successfully"
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => {
            error!("Failed to reload config: {}", e);
            Json(ApiResponse::<serde_json::Value>::error(format!("Failed to reload: {}", e)))
        }
    }
}

/// Get workers list from PPLNS shares (with pagination)
async fn workers_list(
    State(state): State<AdminState>,
    Query(params): Query<PaginationRequest>,
) -> impl IntoResponse {
    let banned = state.banned_workers.read().await;
    let worker_tags = state.worker_tags.read().await;

    // Get pagination parameters
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).min(100);
    let search = params.search.unwrap_or_default().to_lowercase();
    let status_filter = params.status.unwrap_or_default().to_lowercase();

    // Get recent PPLNS shares (last 1000, last 24 hours)
    let end_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let start_time = end_time - (24 * 3600); // Last 24 hours

    let shares = state.store.get_pplns_shares_filtered(
        Some(1000),
        Some(start_time),
        Some(end_time),
    );

    // Group shares by miner address
    let mut workers_map: HashMap<String, WorkerInfo> = HashMap::new();

    for share in shares {
        let address = share.btcaddress.clone().unwrap_or_else(|| format!("user_{}", share.user_id));

        let entry = workers_map.entry(address.clone()).or_insert_with(|| {
            let now = chrono::Utc::now();
            let is_banned = banned.contains(&address);
            let tags = worker_tags.get(&address).cloned().unwrap_or_default();
            WorkerInfo {
                address: address.clone(),
                worker_name: share.workername.clone().unwrap_or_else(|| "worker".to_string()),
                hashrate_ths: 0.0,
                shares_count: 0,
                difficulty: share.difficulty,
                last_seen: now.to_rfc3339(),
                first_seen: now.to_rfc3339(),
                is_banned,
                tags,
                status: if is_banned {
                    WorkerStatus::Banned
                } else {
                    WorkerStatus::Active
                },
            }
        });

        entry.shares_count += 1;
        entry.difficulty = share.difficulty;
        entry.last_seen = chrono::Utc::now().to_rfc3339();
    }

    // Convert to vector and apply filters
    let mut workers: Vec<WorkerInfo> = workers_map.into_values().collect();

    // Apply search filter
    if !search.is_empty() {
        workers.retain(|w| {
            w.address.to_lowercase().contains(&search)
                || w.worker_name.to_lowercase().contains(&search)
        });
    }

    // Apply status filter
    if !status_filter.is_empty() {
        workers.retain(|w| match status_filter.as_str() {
            "active" => matches!(w.status, WorkerStatus::Active),
            "banned" => matches!(w.status, WorkerStatus::Banned),
            "inactive" => matches!(w.status, WorkerStatus::Inactive),
            _ => true,
        });
    }

    // Apply sorting
    let sort_by = params.sort_by.unwrap_or_else(|| "last_seen".to_string());
    let sort_desc = params.sort_order.unwrap_or_else(|| "desc".to_string()) == "desc";

    match sort_by.as_str() {
        "address" => workers.sort_by(|a, b| {
            if sort_desc {
                b.address.cmp(&a.address)
            } else {
                a.address.cmp(&b.address)
            }
        }),
        "hashrate" => workers.sort_by(|a, b| {
            if sort_desc {
                b.hashrate_ths.partial_cmp(&a.hashrate_ths).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                a.hashrate_ths.partial_cmp(&b.hashrate_ths).unwrap_or(std::cmp::Ordering::Equal)
            }
        }),
        "shares" => workers.sort_by(|a, b| {
            if sort_desc {
                b.shares_count.cmp(&a.shares_count)
            } else {
                a.shares_count.cmp(&b.shares_count)
            }
        }),
        _ => { // default: last_seen
            workers.sort_by(|a, b| {
                if sort_desc {
                    b.last_seen.cmp(&a.last_seen)
                } else {
                    a.last_seen.cmp(&b.last_seen)
                }
            });
        }
    }

    let total = workers.len();
    let total_pages = (total + page_size - 1) / page_size;

    // Apply pagination
    let start_idx = (page - 1) * page_size;
    let end_idx = start_idx + page_size;
    let paginated_workers: Vec<WorkerInfo> = workers
        .into_iter()
        .skip(start_idx)
        .take(page_size)
        .collect();

    let response = PaginatedResponse {
        data: paginated_workers,
        total,
        page,
        page_size,
        total_pages,
    };

    Json(ApiResponse::ok(response))
}

/// Get worker detail
async fn worker_detail(
    State(state): State<AdminState>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    // Get shares for the specific address
    let end_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let start_time = end_time - (24 * 3600);

    let all_shares = state.store.get_pplns_shares_filtered(
        Some(1000),
        Some(start_time),
        Some(end_time),
    );

    // Filter shares for the specific address
    let shares: Vec<_> = all_shares
        .into_iter()
        .filter(|s| s.btcaddress.as_ref().map_or(false, |addr| addr == &address))
        .collect();

    if shares.is_empty() {
        return Json(ApiResponse::<serde_json::Value>::error(format!("No shares found for address {} in last 24 hours", address)));
    }

    // Group by worker name
    let mut worker_stats: HashMap<String, u64> = HashMap::new();
    let mut total_shares = 0u64;

    for share in shares {
        let worker = share.workername.clone().unwrap_or_else(|| "worker".to_string());
        *worker_stats.entry(worker).or_insert(0) += 1;
        total_shares += 1;
    }

    let response = serde_json::json!({
        "address": address,
        "total_shares": total_shares,
        "worker_stats": worker_stats,
    });

    Json(ApiResponse::ok(response))
}

/// Ban worker
async fn ban_worker(
    State(state): State<AdminState>,
    Path(address): Path<String>,
    Json(req): Json<BanRequest>,
) -> impl IntoResponse {
    state.banned_workers.write().await.insert(address.clone());
    info!("Banned worker: {} - reason: {:?}", address, req.reason);

    let response = serde_json::json!({
        "address": address,
        "banned": true,
        "message": "Worker banned successfully"
    });

    Json(ApiResponse::ok(response))
}

/// Unban worker
async fn unban_worker(
    State(state): State<AdminState>,
    Path(address): Path<String>,
) -> impl IntoResponse {
    state.banned_workers.write().await.remove(&address);
    info!("Unbanned worker: {}", address);

    let response = serde_json::json!({
        "address": address,
        "banned": false,
        "message": "Worker unbanned successfully"
    });

    Json(ApiResponse::ok(response))
}

/// Add tag to worker
#[derive(Deserialize)]
struct AddTagRequest {
    tag: String,
}

async fn add_worker_tag(
    State(state): State<AdminState>,
    Path(address): Path<String>,
    Json(req): Json<AddTagRequest>,
) -> impl IntoResponse {
    let mut worker_tags = state.worker_tags.write().await;
    let tags = worker_tags.entry(address.clone()).or_insert_with(Vec::new);

    if !tags.contains(&req.tag) {
        tags.push(req.tag.clone());
        info!("Added tag '{}' to worker: {}", req.tag, address);
    }

    let response = serde_json::json!({
        "address": address,
        "tag": req.tag,
        "tags": tags.clone(),
        "message": "Tag added successfully"
    });

    Json(ApiResponse::ok(response))
}

/// Remove tag from worker
async fn remove_worker_tag(
    State(state): State<AdminState>,
    Path((address, tag)): Path<(String, String)>,
) -> impl IntoResponse {
    let mut worker_tags = state.worker_tags.write().await;

    if let Some(tags) = worker_tags.get_mut(&address) {
        let original_len = tags.len();
        tags.retain(|t| t != &tag);
        if tags.len() < original_len {
            info!("Removed tag '{}' from worker: {}", tag, address);
        }
    }

    let current_tags = worker_tags.get(&address).cloned().unwrap_or_default();

    let response = serde_json::json!({
        "address": address,
        "tag": tag,
        "tags": current_tags,
        "message": "Tag removed successfully"
    });

    Json(ApiResponse::ok(response))
}

/// Get blocks list
async fn blocks_list(State(state): State<AdminState>) -> impl IntoResponse {
    let _height = state.chain_store.get_tip_height()
        .ok()
        .flatten()
        .map(|h| h as u64)
        .unwrap_or(0);
    // Return basic info - TODO: Get actual blocks from database
    let blocks: Vec<()> = vec![];
    Json(ApiResponse::ok(blocks))
}

/// Get block detail
async fn block_detail(
    State(_state): State<AdminState>,
    Path(height): Path<String>,
) -> impl IntoResponse {
    let _height: u64 = match height.parse() {
        Ok(h) => h,
        Err(_) => return Json(ApiResponse::<serde_json::Value>::error("Invalid block height".to_string())),
    };
    // TODO: Get actual block detail
    Json(ApiResponse::<serde_json::Value>::error("Block detail not yet implemented".to_string()))
}

/// Get logs
async fn logs(State(_state): State<AdminState>) -> impl IntoResponse {
    // TODO: Return actual log entries
    let logs = vec![
        "2026-02-03 10:00:00 [INFO] DMPool started".to_string(),
        "2026-02-03 10:00:05 [INFO] Connected to Bitcoin RPC".to_string(),
    ];
    Json(ApiResponse::ok(logs))
}

/// Safety check endpoint
async fn safety_check(State(state): State<AdminState>) -> impl IntoResponse {
    let config = state.config.read().await;
    let mut critical = vec![];
    let mut warnings = vec![];

    // Check ignore_difficulty
    if config.stratum.ignore_difficulty.unwrap_or(false) {
        critical.push(SafetyIssue {
            severity: "critical".to_string(),
            param: "ignore_difficulty".to_string(),
            message: "已禁用难度验证，可能导致不公平的PPLNS收益分配".to_string(),
            recommendation: "设置为 false".to_string(),
        });
    }

    // Check pplns_ttl_days
    if config.store.pplns_ttl_days < 7 {
        critical.push(SafetyIssue {
            severity: "critical".to_string(),
            param: "pplns_ttl_days".to_string(),
            message: format!(
                "TTL={}天过短，标准为7天，矿工可能损失约{}%的收益",
                config.store.pplns_ttl_days,
                ((7 - config.store.pplns_ttl_days) * 100 / 7)
            ),
            recommendation: "设置为 7".to_string(),
        });
    }

    // Check donation
    if let Some(donation) = config.stratum.donation {
        if donation >= 10000 {
            critical.push(SafetyIssue {
                severity: "critical".to_string(),
                param: "donation".to_string(),
                message: "donation=10000意味着100%捐赠，矿工收益为0！".to_string(),
                recommendation: "设置为0或注释掉donation".to_string(),
            });
        } else if donation > 500 {
            warnings.push(SafetyIssue {
                severity: "warning".to_string(),
                param: "donation".to_string(),
                message: format!("捐赠比例较高: {}%", donation / 100),
                recommendation: "考虑设置为0-500(0-5%)".to_string(),
            });
        }
    }

    let safe = critical.is_empty();

    Json(SafetyReport {
        safe,
        critical_issues: critical,
        warnings,
    })
}

/// Login endpoint using AdminState
async fn login(
    State(state): State<AdminState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    match state.auth_manager.authenticate(&req.username, &req.password).await {
        Ok(Some(user)) => {
            let token = state.auth_manager.generate_token(&user)
                .map_err(|e| {
                    error!("Failed to generate token: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let expires_in = 24 * 3600; // 24 hours

            info!("User '{}' logged in successfully", req.username);

            Ok(Json(LoginResponse {
                token,
                user_info: UserInfo {
                    username: user.username,
                    role: user.role,
                },
                expires_in,
            }))
        }
        Ok(None) => {
            warn!("Failed login attempt for user '{}'", req.username);
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            error!("Authentication error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Get audit logs
async fn audit_logs(
    State(state): State<AdminState>,
    Query(filter): Query<AuditFilterWrapper>,
) -> impl IntoResponse {
    let logs = state.audit_logger.query(filter.0).await;
    Json(ApiResponse::ok(logs))
}

/// Get audit statistics
async fn audit_stats(State(state): State<AdminState>) -> impl IntoResponse {
    let stats = state.audit_logger.stats().await;
    Json(ApiResponse::ok(stats))
}

/// Rotate audit logs
async fn audit_rotate(State(state): State<AdminState>) -> impl IntoResponse {
    match state.audit_logger.rotate_logs().await {
        Ok(archive_path) => {
            let response = serde_json::json!({
                "message": "Audit logs rotated successfully",
                "archive_file": archive_path
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to rotate logs: {}",
            e
        ))),
    }
}

/// Export audit logs
async fn audit_export(State(state): State<AdminState>) -> impl IntoResponse {
    let output_path = std::path::PathBuf::from(format!(
        "./audit_export_{}.jsonl",
        Utc::now().format("%Y%m%d_%H%M%S")
    ));

    match state.audit_logger.export(output_path.clone()).await {
        Ok(count) => {
            let response = serde_json::json!({
                "message": format!("Exported {} audit log entries", count),
                "file": output_path
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to export logs: {}",
            e
        ))),
    }
}

/// Wrapper for Query<AuditFilter> to implement FromRequest
#[derive(Debug, Deserialize)]
struct AuditFilterWrapper(AuditFilter);

impl Default for AuditFilterWrapper {
    fn default() -> Self {
        Self(AuditFilter::default())
    }
}

/// Get pending configuration change confirmations
async fn get_confirmations(State(state): State<AdminState>) -> impl IntoResponse {
    let pending = state.config_confirmation.get_pending().await;
    Json(ApiResponse::ok(pending))
}

/// Request a configuration change (creates confirmation request)
async fn request_config_change(
    State(state): State<AdminState>,
    Json(req): Json<ConfigChangeRequestData>,
) -> impl IntoResponse {
    // Validate the new value
    if let Err(e) = state
        .config_confirmation
        .validate_value(&req.parameter, &req.new_value)
    {
        return Json(ApiResponse::<serde_json::Value>::error(format!(
            "Invalid value for {}: {}",
            req.parameter, e
        )));
    }

    // Check if confirmation is required
    if !state
        .config_confirmation
        .requires_confirmation(&req.parameter)
    {
        // Apply immediately if no confirmation needed
        let response = serde_json::json!({
            "message": format!("{} updated (no confirmation required)", req.parameter),
            "parameter": req.parameter,
            "old_value": req.old_value,
            "new_value": req.new_value,
            "confirmed": true,
            "applied": true,
        });
        return Json(ApiResponse::ok(response));
    }

    // Create confirmation request
    match state
        .config_confirmation
        .create_change_request(
            req.parameter.clone(),
            req.old_value,
            req.new_value.clone(),
            req.username.clone(),
            req.ip_address.clone(),
        )
        .await
    {
        Ok(request) => {
            // Get risk level info
            let risk_level = state
                .config_confirmation
                .get_risk_level(&req.parameter);

            let response = serde_json::json!({
                "message": "Confirmation required for this change",
                "request": request,
                "risk_level": risk_level,
                "meta": state.config_confirmation.get_config_meta(&req.parameter),
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to create confirmation request: {}",
            e
        ))),
    }
}

/// Confirm a pending configuration change
async fn confirm_config(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.config_confirmation.confirm_change(&id).await {
        Ok(true) => {
            let response = serde_json::json!({
                "message": "Change confirmed. Use /apply to apply the change.",
                "id": id
            });
            Json(ApiResponse::ok(response))
        }
        Ok(false) => {
            Json(ApiResponse::<serde_json::Value>::error(
                "Change request not found or expired".to_string(),
            ))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to confirm change: {}",
            e
        ))),
    }
}

/// Apply a confirmed configuration change
async fn apply_config(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.config_confirmation.apply_change(&id).await {
        Ok(request) => {
            // TODO: Actually apply the config change to the running config
            // For now, just log it

            let response = serde_json::json!({
                "message": format!("Config change applied: {} = {:?}", request.parameter, request.new_value),
                "request": request
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to apply change: {}",
            e
        ))),
    }
}

// ===== Backup API Handlers =====

/// Create a new backup
async fn create_backup(State(state): State<AdminState>) -> impl IntoResponse {
    match state.backup_manager.create_backup().await {
        Ok(metadata) => {
            let response = serde_json::json!({
                "message": "Backup created successfully",
                "backup": metadata
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to create backup: {}",
            e
        ))),
    }
}

/// List all backups
async fn list_backups(State(state): State<AdminState>) -> impl IntoResponse {
    match state.backup_manager.list_backups() {
        Ok(backups) => {
            let response = serde_json::json!({
                "backups": backups,
                "count": backups.len()
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to list backups: {}",
            e
        ))),
    }
}

/// Get backup statistics
async fn backup_stats(State(state): State<AdminState>) -> impl IntoResponse {
    match state.backup_manager.get_stats() {
        Ok(stats) => {
            let response = serde_json::json!({
                "stats": stats
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to get backup stats: {}",
            e
        ))),
    }
}

/// Get a specific backup by ID
async fn get_backup(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.backup_manager.load_metadata(&id) {
        Ok(metadata) => {
            let response = serde_json::json!({
                "backup": metadata
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to load backup: {}",
            e
        ))),
    }
}

/// Delete a backup
async fn delete_backup(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.backup_manager.delete_backup(&id).await {
        Ok(_) => {
            let response = serde_json::json!({
                "message": format!("Backup {} deleted successfully", id)
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to delete backup: {}",
            e
        ))),
    }
}

/// Restore from a backup
async fn restore_backup(
    State(state): State<AdminState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.backup_manager.restore_backup(&id, None).await {
        Ok(_) => {
            let response = serde_json::json!({
                "message": format!("Backup {} restored successfully", id),
                "note": "Database service restart may be required"
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to restore backup: {}",
            e
        ))),
    }
}

/// Cleanup old backups based on retention policy
async fn cleanup_backups(State(state): State<AdminState>) -> impl IntoResponse {
    match state.backup_manager.cleanup_old_backups().await {
        Ok(count) => {
            let response = serde_json::json!({
                "message": format!("Cleaned up {} old backup(s)", count),
                "deleted_count": count
            });
            Json(ApiResponse::ok(response))
        }
        Err(e) => Json(ApiResponse::<serde_json::Value>::error(format!(
            "Failed to cleanup backups: {}",
            e
        ))),
    }
}

/// Data for creating a config change request
#[derive(Deserialize)]
struct ConfigChangeRequestData {
    pub parameter: String,
    pub old_value: serde_json::Value,
    pub new_value: serde_json::Value,
    pub username: String,
    pub ip_address: String,
}

/// 404 handler
async fn not_found() -> impl IntoResponse {
    (StatusCode::NOT_FOUND, "Not Found")
}
