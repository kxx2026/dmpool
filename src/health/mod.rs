// Health check module for DMPool
// Enhanced health monitoring with database/RPC/ZMQ integration

use anyhow::Result;
use p2poolv2_lib::store::Store;
use p2poolv2_lib::config::Config;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Health check response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub database: ComponentStatus,
    pub bitcoin_rpc: ComponentStatus,
    pub zmq: ComponentStatus,
    pub uptime_seconds: u64,
    pub active_connections: u64,
    pub last_block_height: Option<u64>,
    pub memory_mb: Option<u64>,
}

/// Individual component status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentStatus {
    pub status: String,
    pub message: String,
    pub latency_ms: Option<u64>,
}

impl ComponentStatus {
    pub fn healthy() -> Self {
        Self {
            status: "healthy".to_string(),
            message: "OK".to_string(),
            latency_ms: None,
        }
    }

    fn unhealthy(message: impl Into<String>) -> Self {
        Self {
            status: "unhealthy".to_string(),
            message: message.into(),
            latency_ms: None,
        }
    }

    fn with_latency(mut self, latency_ms: u64) -> Self {
        self.latency_ms = Some(latency_ms);
        self
    }

    fn with_message(mut self, msg: impl Into<String>) -> Self {
        self.message = msg.into();
        self
    }
}

/// Health checker with Store integration
pub struct HealthChecker {
    start_time: Instant,
    config: Config,
    store: Option<Arc<Store>>,
    last_block_height: std::sync::Arc<std::sync::atomic::AtomicU64>,
}

impl HealthChecker {
    pub fn new(config: Config) -> Self {
        Self {
            start_time: Instant::now(),
            config,
            store: None,
            last_block_height: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn with_store(mut self, store: Arc<Store>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn update_block_height(&self, height: u64) {
        self.last_block_height.store(height, std::sync::atomic::Ordering::Relaxed);
    }

    /// Perform comprehensive health check
    pub async fn check(&self) -> HealthStatus {
        let db_status = self.check_database().await;
        let rpc_status = self.check_bitcoin_rpc().await;
        let zmq_status = self.check_zmq().await;

        let overall_status = match (db_status.status.as_str(), rpc_status.status.as_str(), zmq_status.status.as_str()) {
            ("healthy", "healthy", "healthy") => "healthy",
            ("unhealthy", _, _) | (_, "unhealthy", _) | (_, _, "unhealthy") => "unhealthy",
            _ => "degraded",
        };

        let memory_mb = self.get_memory_usage();

        HealthStatus {
            status: overall_status.to_string(),
            database: db_status,
            bitcoin_rpc: rpc_status,
            zmq: zmq_status,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_connections: 0,
            last_block_height: {
                let h = self.last_block_height.load(std::sync::atomic::Ordering::Relaxed);
                if h > 0 { Some(h) } else { None }
            },
            memory_mb,
        }
    }

    /// Check database connectivity and status
    async fn check_database(&self) -> ComponentStatus {
        let start = Instant::now();

        if let Some(store) = &self.store {
            // get_chain_tip returns BlockHash directly
            let _tip = store.get_chain_tip();
            ComponentStatus::healthy()
                .with_latency(start.elapsed().as_millis() as u64)
                .with_message("Database operational")
        } else {
            // Fallback: try creating a temporary store
            let db_path = format!("{}_health_check", self.config.store.path);
            match Store::new(db_path.clone(), true) {
                Ok(_) => {
                    let _ = std::fs::remove_dir_all(&db_path);
                    ComponentStatus::healthy()
                        .with_latency(start.elapsed().as_millis() as u64)
                        .with_message("Database operational (temporary check)")
                }
                Err(e) => ComponentStatus::unhealthy(format!("Database error: {}", e))
                    .with_latency(start.elapsed().as_millis() as u64),
            }
        }
    }

    /// Check Bitcoin RPC connectivity
    async fn check_bitcoin_rpc(&self) -> ComponentStatus {
        let start = Instant::now();

        let url = &self.config.bitcoinrpc.url;
        let parts: Vec<&str> = url.split("://").collect();
        if parts.len() != 2 {
            return ComponentStatus::unhealthy("Invalid RPC URL format");
        }

        let host_port = parts[1].split('/').next().unwrap_or("127.0.0.1:8332");

        match timeout(Duration::from_secs(5), TcpStream::connect(host_port)).await {
            Ok(Ok(_)) => ComponentStatus::healthy()
                .with_latency(start.elapsed().as_millis() as u64)
                .with_message(format!("Connected to {}", host_port)),
            Ok(Err(e)) => ComponentStatus::unhealthy(format!("Connection failed: {}", e))
                .with_latency(start.elapsed().as_millis() as u64),
            Err(_) => ComponentStatus::unhealthy("Connection timeout (5s)")
                .with_latency(5000),
        }
    }

    /// Check ZMQ endpoint connectivity
    async fn check_zmq(&self) -> ComponentStatus {
        let zmq_url = &self.config.stratum.zmqpubhashblock;
        let parts: Vec<&str> = zmq_url.split("://").collect();

        if parts.len() != 2 || parts[0] != "tcp" {
            return ComponentStatus::unhealthy("Invalid ZMQ URL format (expected tcp://host:port)");
        }

        let host_port = parts[1];

        match timeout(Duration::from_secs(2), TcpStream::connect(host_port)).await {
            Ok(Ok(_)) => ComponentStatus::healthy()
                .with_message(format!("ZMQ listening on {}", host_port)),
            Ok(Err(e)) => ComponentStatus::unhealthy(format!("ZMQ connection failed: {}", e)),
            Err(_) => ComponentStatus::unhealthy("ZMQ connection timeout (2s)"),
        }
    }

    /// Get current process memory usage in MB
    fn get_memory_usage(&self) -> Option<u64> {
        #[cfg(unix)]
        {
            use std::fs;
            match fs::read_to_string("/proc/self/status") {
                Ok(content) => {
                    for line in content.lines() {
                        if line.starts_with("VmRSS:") {
                            let parts: Vec<&str> = line.split_whitespace().collect();
                            if parts.len() >= 2 {
                                let kb: u64 = parts[1].parse().ok()?;
                                return Some(kb / 1024);
                            }
                        }
                    }
                    None
                }
                Err(_) => None,
            }
        }
        #[cfg(not(unix))]
        {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_component_status_creation() {
        let status = ComponentStatus::healthy();
        assert_eq!(status.status, "healthy");
        assert_eq!(status.message, "OK");

        let with_latency = status.with_latency(42);
        assert_eq!(with_latency.latency_ms, Some(42));

        let with_msg = with_latency.with_message("Test");
        assert_eq!(with_msg.message, "Test");
    }

    #[test]
    fn test_component_status_unhealthy() {
        let status = ComponentStatus::unhealthy("Test error");
        assert_eq!(status.status, "unhealthy");
        assert_eq!(status.message, "Test error");
    }

    #[test]
    fn test_health_status_serialization() {
        let status = HealthStatus {
            status: "healthy".to_string(),
            database: ComponentStatus::healthy(),
            bitcoin_rpc: ComponentStatus::unhealthy("RPC down"),
            zmq: ComponentStatus::healthy(),
            uptime_seconds: 3600,
            active_connections: 5,
            last_block_height: Some(800000),
            memory_mb: Some(512),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("800000"));
    }
}
