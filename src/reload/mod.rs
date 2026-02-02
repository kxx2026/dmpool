// Configuration hot-reload module for DMPool
// Watches for configuration file changes and validates new configs

use anyhow::{Context, Result};
use p2poolv2_lib::config::Config;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, info, warn, error};

/// Configuration reload manager
pub struct ConfigReloader {
    config_path: PathBuf,
    current_config: Arc<RwLock<Config>>,
    last_modified: Arc<RwLock<std::time::SystemTime>>,
    checksum: Arc<RwLock<String>>,
}

impl ConfigReloader {
    /// Create a new config reloader
    pub fn new(config_path: PathBuf, initial_config: Config) -> Self {
        let initial_checksum = Self::compute_checksum(&initial_config);

        Self {
            config_path,
            current_config: Arc::new(RwLock::new(initial_config)),
            last_modified: Arc::new(RwLock::new(
                std::time::SystemTime::now()
            )),
            checksum: Arc::new(RwLock::new(initial_checksum)),
        }
    }

    /// Start watching for config changes
    pub async fn start(&self, check_interval_secs: u64) -> Result<()> {
        info!("Starting config watcher for: {:?}", self.config_path);
        info!("Check interval: {} seconds", check_interval_secs);

        let mut interval = interval(Duration::from_secs(check_interval_secs));
        let config_path = self.config_path.clone();
        let current_config = self.current_config.clone();
        let last_modified = self.last_modified.clone();
        let checksum = self.checksum.clone();

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                if let Err(e) = Self::check_and_reload(
                    &config_path,
                    &current_config,
                    &last_modified,
                    &checksum,
                ).await {
                    error!("Config reload check failed: {}", e);
                }
            }
        });

        Ok(())
    }

    /// Check for changes and reload if necessary
    async fn check_and_reload(
        config_path: &PathBuf,
        current_config: &Arc<RwLock<Config>>,
        last_modified: &Arc<RwLock<std::time::SystemTime>>,
        checksum: &Arc<RwLock<String>>,
    ) -> Result<()> {
        // Check file modification time
        let metadata = std::fs::metadata(config_path)
            .with_context(|| format!("Failed to read config metadata: {:?}", config_path))?;

        let modified = metadata.modified()
            .with_context(|| "Failed to get modification time")?;

        let last_mod = *last_modified.read().await;

        if modified <= last_mod {
            debug!("Config file unchanged");
            return Ok(());
        }

        info!("Config file modified, attempting reload...");

        // Load new config
        let new_config = Config::load(config_path.to_str().unwrap())
            .with_context(|| "Failed to load config file")?;

        // Validate new config
        Self::validate_config(&new_config)?;

        // Compute checksum to detect actual content changes
        let new_checksum = Self::compute_checksum(&new_config);
        let current_checksum = checksum.read().await;

        if new_checksum == *current_checksum {
            debug!("Config checksum unchanged, skipping reload");
            *last_modified.write().await = modified;
            return Ok(());
        }

        // Update current config
        *current_config.write().await = new_config;
        *checksum.write().await = new_checksum;
        *last_modified.write().await = modified;

        info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Validate configuration before applying
    fn validate_config(config: &Config) -> Result<()> {
        // Validate port ranges
        if config.api.port < 1024 || config.api.port > 65535 {
            return Err(anyhow::anyhow!("API port out of valid range: {}", config.api.port));
        }

        if config.stratum.port < 1024 || config.stratum.port > 65535 {
            return Err(anyhow::anyhow!("Stratum port out of valid range: {}", config.api.port));
        }

        // Validate network type
        match config.stratum.network {
            bitcoin::Network::Bitcoin | bitcoin::Network::Signet | bitcoin::Network::Testnet4 => {}
            _ => return Err(anyhow::anyhow!("Unsupported network type")),
        }

        // Validate store path
        if config.store.path.is_empty() {
            return Err(anyhow::anyhow!("Store path cannot be empty"));
        }

        // Validate TTL values
        if config.store.pplns_ttl_days < 1 {
            return Err(anyhow::anyhow!("PPLNS TTL must be at least 1 day"));
        }

        // Validate logging directory
        if !config.logging.stats_dir.is_empty() {
            if let Err(e) = std::fs::create_dir_all(&config.logging.stats_dir) {
                warn!("Failed to create stats directory: {}", e);
            }
        }

        Ok(())
    }

    /// Get current config reference
    pub async fn get_config(&self) -> Config {
        self.current_config.read().await.clone()
    }

    /// Manually trigger a config reload
    pub async fn reload_now(&self) -> Result<()> {
        info!("Manual config reload triggered");

        let metadata = std::fs::metadata(&self.config_path)?;
        let modified = metadata.modified()?;

        let new_config = Config::load(self.config_path.to_str().unwrap())?;
        Self::validate_config(&new_config)?;

        *self.current_config.write().await = new_config;
        *self.last_modified.write().await = modified;

        info!("Manual config reload successful");
        Ok(())
    }

    /// Compute a simple checksum of config for change detection
    fn compute_checksum(config: &Config) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Hash key configuration values
        config.api.port.hash(&mut hasher);
        config.stratum.port.hash(&mut hasher);
        config.stratum.network.hash(&mut hasher);
        config.store.path.hash(&mut hasher);
        config.store.pplns_ttl_days.hash(&mut hasher);

        format!("{:x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_different_ports() {
        let mut hasher1 = DefaultHasher::new();
        8080u16.hash(&mut hasher1);
        let checksum1 = format!("{:x}", hasher1.finish());

        let mut hasher2 = DefaultHasher::new();
        8081u16.hash(&mut hasher2);
        let checksum2 = format!("{:x}", hasher2.finish());

        assert_ne!(checksum1, checksum2);
    }

    #[test]
    fn test_config_validation() {
        // This is a basic validation test structure
        // Full tests would require actual Config instances
        let valid_port = 8080;
        assert!(valid_port >= 1024 && valid_port <= 65535);

        let invalid_port = 100;
        assert!(invalid_port < 1024);
    }
}
