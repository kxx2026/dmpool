// Health check module for DMPool
// Provides enhanced health monitoring beyond the basic OK response

use anyhow::Result;
use p2poolv2_lib::store::Store;
use p2poolv2_lib::config::Config;
use serde::{Deserialize, Serialize};
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
}

/// Health checker instance
pub struct HealthChecker {
    start_time: Instant,
    config: Config,
}

impl HealthChecker {
    pub fn new(config: Config) -> Self {
        Self {
            start_time: Instant::now(),
            config,
        }
    }
    
    /// Perform comprehensive health check
    pub async fn check(&self) -> HealthStatus {
        let db_status = self.check_database().await;
        let rpc_status = self.check_bitcoin_rpc().await;
        let zmq_status = self.check_zmq().await;
        
        let overall_status = if db_status.status == "healthy" 
            && rpc_status.status == "healthy" 
            && zmq_status.status == "healthy" {
            "healthy"
        } else {
            "degraded"
        };
        
        HealthStatus {
            status: overall_status.to_string(),
            database: db_status,
            bitcoin_rpc: rpc_status,
            zmq: zmq_status,
            uptime_seconds: self.start_time.elapsed().as_secs(),
            active_connections: 0,
            last_block_height: None,
        }
    }
    
    /// Check database connectivity
    async fn check_database(&self) -> ComponentStatus {
        let start = Instant::now();
        
        let db_path = format!("{}_health_check", self.config.store.path);
        match Store::new(db_path.clone(), true) {
            Ok(_store) => {
                let _ = std::fs::remove_file(&db_path);
                ComponentStatus::healthy()
                    .with_latency(start.elapsed().as_millis() as u64)
            }
            Err(e) => {
                ComponentStatus::unhealthy(format!("Database error: {}", e))
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
                .with_latency(start.elapsed().as_millis() as u64),
            Ok(Err(e)) => ComponentStatus::unhealthy(format!("Connection failed: {}", e)),
            Err(_) => ComponentStatus::unhealthy("Connection timeout"),
        }
    }
    
    /// Check ZMQ endpoint connectivity
    async fn check_zmq(&self) -> ComponentStatus {
        let zmq_url = &self.config.stratum.zmqpubhashblock;
        let parts: Vec<&str> = zmq_url.split("://").collect();
        
        if parts.len() != 2 {
            return ComponentStatus::unhealthy("Invalid ZMQ URL format");
        }
        
        let host_port = parts[1];
        
        match timeout(Duration::from_secs(2), TcpStream::connect(host_port)).await {
            Ok(Ok(_)) => ComponentStatus::healthy(),
            Ok(Err(e)) => ComponentStatus::unhealthy(format!("ZMQ connection failed: {}", e)),
            Err(_) => ComponentStatus::unhealthy("ZMQ connection timeout"),
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
    }
    
    #[test]
    fn test_component_status_unhealthy() {
        let status = ComponentStatus::unhealthy("Test error");
        assert_eq!(status.status, "unhealthy");
        assert_eq!(status.message, "Test error");
    }
}
