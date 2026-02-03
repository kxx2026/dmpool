// Health check module for DMPool
// Enhanced health monitoring with database/RPC/ZMQ/Bitcoin node integration

use anyhow::Result;
use p2poolv2_lib::store::Store;
use p2poolv2_lib::config::Config;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// Comprehensive health check response structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthStatus {
    pub status: String,
    pub database: ComponentStatus,
    pub bitcoin_node: BitcoinNodeStatus,
    pub stratum: StratumStatus,
    pub zmq: ComponentStatus,
    pub uptime_seconds: u64,
    pub memory_mb: Option<u64>,
}

/// Bitcoin node detailed status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitcoinNodeStatus {
    pub status: String,
    pub rpc_latency_ms: Option<u64>,
    pub blockchain: BlockchainInfo,
    pub network: NetworkInfo,
    pub sync_progress: f64,  // 0.0 to 1.0
    pub message: String,
}

/// Blockchain information from Bitcoin node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockchainInfo {
    pub blocks: u64,
    pub headers: u64,
    pub initial_block_download: bool,
    pub verification_progress: f64,
    pub block_time_seconds: Option<u64>,
    pub best_block_hash: String,
}

/// Network information from Bitcoin node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub connections: u32,
    pub network_active: bool,
    pub peer_count: u32,
}

/// Stratum service status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StratumStatus {
    pub status: String,
    pub listening: bool,
    pub active_connections: u32,
    pub shares_per_second: f64,
    pub current_difficulty: f64,
    pub message: String,
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
    active_connections: std::sync::Arc<std::sync::atomic::AtomicU32>,
    shares_per_second: std::sync::Arc<std::sync::atomic::AtomicU64>,  // Store as fixed-point (3 decimal places)
    current_difficulty: std::sync::Arc<std::sync::atomic::AtomicU64>,  // Store as fixed-point (2 decimal places)
}

impl HealthChecker {
    pub fn new(config: Config) -> Self {
        Self {
            start_time: Instant::now(),
            config,
            store: None,
            last_block_height: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            active_connections: std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0)),
            shares_per_second: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            current_difficulty: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
        }
    }

    pub fn with_store(mut self, store: Arc<Store>) -> Self {
        self.store = Some(store);
        self
    }

    pub fn update_block_height(&self, height: u64) {
        self.last_block_height.store(height, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn update_connections(&self, count: u32) {
        self.active_connections.store(count, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn update_shares_per_second(&self, shares: f64) {
        // Store as fixed-point with 3 decimal places
        self.shares_per_second.store((shares * 1000.0) as u64, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn update_difficulty(&self, difficulty: f64) {
        // Store as fixed-point with 2 decimal places
        self.current_difficulty.store((difficulty * 100.0) as u64, std::sync::atomic::Ordering::Relaxed);
    }

    fn get_shares_per_second(&self) -> f64 {
        self.shares_per_second.load(std::sync::atomic::Ordering::Relaxed) as f64 / 1000.0
    }

    fn get_difficulty(&self) -> f64 {
        self.current_difficulty.load(std::sync::atomic::Ordering::Relaxed) as f64 / 100.0
    }

    /// Perform comprehensive health check
    pub async fn check(&self) -> HealthStatus {
        let db_status = self.check_database().await;
        let bitcoin_status = self.check_bitcoin_node().await;
        let stratum_status = self.check_stratum().await;
        let zmq_status = self.check_zmq().await;

        let overall_status = match (
            db_status.status.as_str(),
            bitcoin_status.status.as_str(),
            stratum_status.status.as_str(),
            zmq_status.status.as_str(),
        ) {
            ("healthy", "healthy", "healthy", "healthy") => "healthy",
            ("unhealthy", _, _, _) | (_, "unhealthy", _, _) | (_, _, "unhealthy", _) | (_, _, _, "unhealthy") => "unhealthy",
            _ => "degraded",
        };

        let memory_mb = self.get_memory_usage();

        HealthStatus {
            status: overall_status.to_string(),
            database: db_status,
            bitcoin_node: bitcoin_status,
            stratum: stratum_status,
            zmq: zmq_status,
            uptime_seconds: self.start_time.elapsed().as_secs(),
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

    /// Check Bitcoin RPC connectivity and get blockchain info
    async fn check_bitcoin_node(&self) -> BitcoinNodeStatus {
        let start = Instant::now();
        let latency = start.elapsed().as_millis() as u64;

        // Try to get blockchain info from Bitcoin RPC
        match self.get_blockchain_info().await {
            Ok(blockchain) => {
                let network = match self.get_network_info().await {
                    Ok(n) => n,
                    Err(_e) => NetworkInfo {
                        connections: 0,
                        network_active: false,
                        peer_count: 0,
                    },
                };

                // Calculate sync progress
                let sync_progress = if blockchain.headers > 0 {
                    blockchain.blocks as f64 / blockchain.headers as f64
                } else {
                    1.0
                };

                let status = if blockchain.initial_block_download || sync_progress < 0.999 {
                    "syncing"
                } else if network.connections == 0 {
                    "degraded"
                } else {
                    "healthy"
                };

                let message = if blockchain.initial_block_download {
                    format!("同步中... {}/{} ({:.1}%)",
                        blockchain.blocks,
                        blockchain.headers,
                        sync_progress * 100.0
                    )
                } else if sync_progress >= 0.999 {
                    format!("已同步，高度: {}，连接: {} 个节点",
                        blockchain.blocks,
                        network.connections
                    )
                } else {
                    format!("节点运行中，高度: {}", blockchain.blocks)
                };

                BitcoinNodeStatus {
                    status: status.to_string(),
                    rpc_latency_ms: Some(latency),
                    blockchain,
                    network,
                    sync_progress,
                    message,
                }
            }
            Err(e) => {
                BitcoinNodeStatus {
                    status: "unhealthy".to_string(),
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
                    message: format!("无法连接 Bitcoin RPC: {}", e),
                }
            }
        }
    }

    /// Query Bitcoin RPC for blockchain info
    async fn get_blockchain_info(&self) -> Result<BlockchainInfo> {
        use bitcoincore_rpc::RpcApi;

        let rpc_url = &self.config.bitcoinrpc.url;
        let rpc_user = &self.config.bitcoinrpc.username;
        let rpc_pass = &self.config.bitcoinrpc.password;

        let rpc = bitcoincore_rpc::Client::new(
            rpc_url,
            bitcoincore_rpc::Auth::UserPass(rpc_user.clone(), rpc_pass.clone()),
        ).map_err(|e| anyhow::anyhow!("Failed to create RPC client: {}", e))?;

        // Get blockchain info
        let info: Value = rpc.call("getblockchaininfo", &[])
            .map_err(|e| anyhow::anyhow!("RPC call failed: {}", e))?;

        Ok(BlockchainInfo {
            blocks: info["blocks"].as_u64().unwrap_or(0),
            headers: info["headers"].as_u64().unwrap_or(0),
            initial_block_download: info["initialblockdownload"].as_bool().unwrap_or(false),
            verification_progress: info["verificationprogress"].as_f64().unwrap_or(0.0),
            block_time_seconds: info.get("mediantime").and_then(|t| t.as_u64()).map(|_| 600),
            best_block_hash: info["bestblockhash"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Query Bitcoin RPC for network info
    async fn get_network_info(&self) -> Result<NetworkInfo> {
        use bitcoincore_rpc::RpcApi;

        let rpc_url = &self.config.bitcoinrpc.url;
        let rpc_user = &self.config.bitcoinrpc.username;
        let rpc_pass = &self.config.bitcoinrpc.password;

        let rpc = bitcoincore_rpc::Client::new(
            rpc_url,
            bitcoincore_rpc::Auth::UserPass(rpc_user.clone(), rpc_pass.clone()),
        ).map_err(|e| anyhow::anyhow!("Failed to create RPC client: {}", e))?;

        // Get network info
        let info: Value = rpc.call("getnetworkinfo", &[])
            .map_err(|e| anyhow::anyhow!("RPC call failed: {}", e))?;

        Ok(NetworkInfo {
            connections: info["connections"].as_u64().unwrap_or(0) as u32,
            network_active: info["networkactive"].as_bool().unwrap_or(false),
            peer_count: info["connections"].as_u64().unwrap_or(0) as u32,
        })
    }

    /// Check Stratum service status
    async fn check_stratum(&self) -> StratumStatus {
        let active_connections = self.active_connections.load(std::sync::atomic::Ordering::Relaxed);
        let shares_per_second = self.get_shares_per_second();
        let current_difficulty = self.get_difficulty();

        // Check if stratum port is listening
        let is_listening = match timeout(
            Duration::from_secs(1),
            TcpStream::connect(format!("{}:{}", self.config.stratum.hostname, self.config.stratum.port))
        ).await {
            Ok(Ok(_)) => true,
            _ => false,
        };

        let status = if is_listening {
            "healthy"
        } else {
            "unhealthy"
        };

        let message = if is_listening {
            format!("端口 {} 监听中，{} 个活跃连接",
                self.config.stratum.port,
                active_connections
            )
        } else {
            format!("端口 {} 未监听", self.config.stratum.port)
        };

        StratumStatus {
            status: status.to_string(),
            listening: is_listening,
            active_connections,
            shares_per_second,
            current_difficulty,
            message,
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
            bitcoin_node: BitcoinNodeStatus {
                status: "healthy".to_string(),
                rpc_latency_ms: Some(100),
                blockchain: BlockchainInfo {
                    blocks: 800000,
                    headers: 800000,
                    initial_block_download: false,
                    verification_progress: 1.0,
                    block_time_seconds: Some(600),
                    best_block_hash: "00000000000000000000".to_string(),
                },
                network: NetworkInfo {
                    connections: 10,
                    network_active: true,
                    peer_count: 8,
                },
                sync_progress: 1.0,
                message: "OK".to_string(),
            },
            stratum: StratumStatus {
                status: "healthy".to_string(),
                listening: true,
                active_connections: 5,
                shares_per_second: 0.0,
                current_difficulty: 32.0,
                message: "OK".to_string(),
            },
            zmq: ComponentStatus::healthy(),
            uptime_seconds: 3600,
            memory_mb: Some(512),
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("healthy"));
        assert!(json.contains("800000"));
    }
}
