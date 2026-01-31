// Common test utilities for DMPool integration tests

use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

/// Get the project root directory
pub fn project_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Get the test data directory
pub fn test_data_dir() -> PathBuf {
    project_root().join("tests").join("data")
}

/// Get a temporary test directory
pub fn temp_dir(name: &str) -> PathBuf {
    std::env::temp_dir().join("dmpool_test").join(name)
}

/// Test configuration builder
pub struct TestConfig {
    pub db_path: PathBuf,
    pub stratum_port: u16,
    pub api_port: u16,
    pub network: p2poolv2_lib::network::Network,
}

impl Default for TestConfig {
    fn default() -> Self {
        Self {
            db_path: temp_dir("test_db").join("store.db"),
            stratum_port: 13333,
            api_port: 14684,
            network: p2poolv2_lib::network::Network::Signet,
        }
    }
}

impl TestConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_stratum_port(mut self, port: u16) -> Self {
        self.stratum_port = port;
        self
    }

    pub fn with_api_port(mut self, port: u16) -> Self {
        self.api_port = port;
        self
    }

    pub fn build(&self) -> String {
        format!(
            r#"
[store]
path = "{}"
background_task_frequency_hours = 24
pplns_ttl_days = 1

[stratum]
hostname = "127.0.0.1"
port = {}
start_difficulty = 1
minimum_difficulty = 1
bootstrap_address = "bcrt1qce93hy5rhg02s6aeu7mfdvxg76x66pqqtrvzs3"
zmqpubhashblock = "tcp://127.0.0.1:28334"
network = "{}"
pool_signature = "test_pool"

[bitcoinrpc]
url = "http://127.0.0.1:18443"
username = "bitcoin"
password = "bitcoin"

[logging]
level = "info"

[api]
hostname = "127.0.0.1"
port = {}
"#,
            self.db_path.display(),
            self.stratum_port,
            self.network.as_str(),
            self.api_port
        )
    }
}

/// Setup a test database
pub async fn setup_test_db() -> anyhow::Result<PathBuf> {
    let db_path = temp_dir("setup_test_db").join("store.db");
    
    // Create parent directories
    if let Some(parent) = db_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    
    // Remove existing database if present
    if db_path.exists() {
        tokio::fs::remove_file(&db_path).await?;
    }
    
    Ok(db_path)
}

/// Cleanup test database
pub async fn cleanup_test_db(db_path: PathBuf) -> anyhow::Result<()> {
    if db_path.exists() {
        tokio::fs::remove_file(&db_path).await?;
    }
    Ok(())
}

/// Wait for a service to be ready (with retry)
pub async fn wait_for_service(addr: &str, max_retries: u32) -> anyhow::Result<()> {
    for i in 0..max_retries {
        match tokio::net::TcpStream::connect(addr).await {
            Ok(_) => return Ok(()),
            Err(e) if i < max_retries - 1 => {
                sleep(Duration::from_millis(200)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
    Err(anyhow::anyhow!("Service not ready after {} retries", max_retries))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_generation() {
        let config = TestConfig::new()
            .with_stratum_port(3333)
            .with_api_port(46884);
        
        let config_str = config.build();
        assert!(config_str.contains("port = 3333"));
        assert!(config_str.contains("port = 46884"));
        assert!(config_str.contains("network = \"signet\""));
    }
}
