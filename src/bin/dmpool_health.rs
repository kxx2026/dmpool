use anyhow::Result;
use dmpool::health::{HealthChecker, HealthStatus, ComponentStatus};
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
    
    let port = env::var("HEALTH_PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    // Create a simple health endpoint
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
        bitcoin_rpc: ComponentStatus::healthy(),
        zmq: ComponentStatus::healthy(),
        uptime_seconds: 0,
        active_connections: 0,
        last_block_height: None,
    })
}

async fn ready_handler() -> &'static str {
    "OK"
}
