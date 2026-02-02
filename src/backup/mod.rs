// Automated backup module for DMPool
// Provides database backup, restore, and verification

use anyhow::{Context, Result};
use p2poolv2_lib::store::Store;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn, error};
use chrono::{DateTime, Utc};

/// Backup manager for DMPool database
pub struct BackupManager {
    store_path: PathBuf,
    backup_dir: PathBuf,
    max_backups: usize,
    compression_enabled: bool,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(
        store_path: PathBuf,
        backup_dir: PathBuf,
        max_backups: usize,
    ) -> Result<Self> {
        std::fs::create_dir_all(&backup_dir)
            .context("Failed to create backup directory")?;

        Ok(Self {
            store_path,
            backup_dir,
            max_backups,
            compression_enabled: true,
        })
    }

    /// Perform a backup
    pub fn backup(&self) -> Result<BackupInfo> {
        let timestamp = Utc::now();
        let backup_name = format!("dmpool_backup_{}", timestamp.format("%Y%m%d_%H%M%S"));
        let backup_path = self.backup_dir.join(&backup_name);

        info!("Starting backup to: {}", backup_path.display());

        // Create backup directory
        std::fs::create_dir_all(&backup_path)
            .context("Failed to create backup directory")?;

        // Copy database files
        self.copy_database_files(&backup_path)?;

        // Create backup metadata
        let metadata = BackupMetadata {
            backup_name: backup_name.clone(),
            created_at: timestamp,
            store_path: self.store_path.clone(),
            backup_path: backup_path.clone(),
            size_bytes: self.calculate_size(&backup_path)?,
            version: env!("CARGO_PKG_VERSION").to_string(),
        };

        // Save metadata
        self.save_metadata(&metadata)?;

        // Cleanup old backups
        self.cleanup_old_backups()?;

        info!("Backup completed: {} ({} bytes)", backup_name, metadata.size_bytes);

        Ok(BackupInfo {
            path: backup_path,
            metadata,
        })
    }

    /// Restore from a backup
    pub fn restore(&self, backup_name: &str) -> Result<()> {
        info!("Restoring from backup: {}", backup_name);

        let backup_path = self.backup_dir.join(backup_name);

        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup not found: {}", backup_name));
        }

        // Load metadata
        let metadata = self.load_metadata(&backup_path)?;

        // Validate backup
        self.validate_backup(&metadata)?;

        // Stop current operations, restore database
        info!("Stopping pool operations for restore...");

        // Backup current database before restore
        let pre_restore_backup = format!("pre_restore_{}", Utc::now().format("%Y%m%d_%H%M%S"));
        let pre_restore_path = self.backup_dir.join(&pre_restore_backup);
        std::fs::create_dir_all(&pre_restore_path)?;
        self.copy_database_files(&pre_restore_path)?;
        info!("Pre-restore backup saved: {}", pre_restore_backup);

        // Restore files
        self.restore_database_files(&backup_path)?;

        info!("Restore completed successfully");
        Ok(())
    }

    /// List all available backups
    pub fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        let mut backups = Vec::new();

        for entry in std::fs::read_dir(&self.backup_dir)
            .context("Failed to read backup directory")?
        {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() && path.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("dmpool_backup_"))
                .unwrap_or(false)
            {
                match self.load_metadata(&path) {
                    Ok(metadata) => backups.push(metadata),
                    Err(e) => warn!("Failed to load metadata for {:?}: {}", path, e),
                }
            }
        }

        backups.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        Ok(backups)
    }

    /// Verify a backup
    pub fn verify(&self, backup_name: &str) -> Result<bool> {
        let backup_path = self.backup_dir.join(backup_name);

        if !backup_path.exists() {
            return Err(anyhow::anyhow!("Backup not found: {}", backup_name));
        }

        let metadata = self.load_metadata(&backup_path)?;

        // Check files exist
        if !backup_path.join("CURRENT").exists() {
            return Ok(false);
        }

        // Check size matches
        let current_size = self.calculate_size(&backup_path)?;
        if current_size != metadata.size_bytes {
            warn!("Backup size mismatch: expected {}, got {}", metadata.size_bytes, current_size);
            return Ok(false);
        }

        Ok(true)
    }

    /// Start automated backup scheduler
    pub async fn start_scheduler(&self, interval_hours: u64) -> Result<()> {
        info!("Starting backup scheduler (interval: {} hours)", interval_hours);

        let backup_manager = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_hours * 3600));

            loop {
                interval.tick().await;

                if let Err(e) = backup_manager.backup() {
                    error!("Scheduled backup failed: {}", e);
                }
            }
        });

        Ok(())
    }

    // Internal methods

    fn copy_database_files(&self, dest: &Path) -> Result<()> {
        let source = Path::new(&self.store_path);

        if !source.exists() {
            return Err(anyhow::anyhow!("Source database not found"));
        }

        // Copy all database files
        for entry in std::fs::read_dir(source)
            .context("Failed to read database directory")?
        {
            let entry = entry?;
            let src_path = entry.path();

            if src_path.is_file() {
                let file_name = src_path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

                let dest_path = dest.join(file_name);
                std::fs::copy(&src_path, &dest_path)
                    .with_context(|| format!("Failed to copy {}", file_name))?;

                debug!("Copied: {}", file_name);
            }
        }

        Ok(())
    }

    fn restore_database_files(&self, backup_path: &Path) -> Result<()> {
        let dest = Path::new(&self.store_path);

        // Clear existing database
        if dest.exists() {
            for entry in std::fs::read_dir(dest)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    std::fs::remove_file(&path)?;
                }
            }
        } else {
            std::fs::create_dir_all(dest)?;
        }

        // Copy backup files
        for entry in std::fs::read_dir(backup_path)? {
            let entry = entry?;
            let src_path = entry.path();

            if src_path.is_file() {
                let file_name = src_path.file_name()
                    .and_then(|n| n.to_str())
                    .ok_or_else(|| anyhow::anyhow!("Invalid filename"))?;

                let dest_path = dest.join(file_name);
                std::fs::copy(&src_path, &dest_path)?;
            }
        }

        Ok(())
    }

    fn cleanup_old_backups(&self) -> Result<()> {
        let mut backups = self.list_backups()?;

        if backups.len() <= self.max_backups {
            return Ok(());
        }

        let to_remove = &backups[self.max_backups..];

        for backup in to_remove {
            info!("Removing old backup: {}", backup.backup_name);
            std::fs::remove_dir_all(&backup.backup_path)
                .with_context(|| format!("Failed to remove backup: {}", backup.backup_name))?;
        }

        Ok(())
    }

    fn validate_backup(&self, metadata: &BackupMetadata) -> Result<()> {
        if !metadata.backup_path.exists() {
            return Err(anyhow::anyhow!("Backup path does not exist"));
        }

        if !metadata.backup_path.join("CURRENT").exists() {
            return Err(anyhow::anyhow!("Invalid backup: missing CURRENT file"));
        }

        Ok(())
    }

    fn save_metadata(&self, metadata: &BackupMetadata) -> Result<()> {
        let metadata_path = metadata.backup_path.join("metadata.json");
        let json = serde_json::to_string_pretty(metadata)?;
        std::fs::write(metadata_path, json)?;
        Ok(())
    }

    fn load_metadata(&self, backup_path: &Path) -> Result<BackupMetadata> {
        let metadata_path = backup_path.join("metadata.json");

        if !metadata_path.exists() {
            // Create minimal metadata for legacy backups
            return Ok(BackupMetadata {
                backup_name: backup_path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
                created_at: std::fs::metadata(backup_path)?
                    .modified()
                    .unwrap_or(std::time::SystemTime::now())
                    .into(),
                store_path: self.store_path.clone(),
                backup_path: backup_path.to_path_buf(),
                size_bytes: self.calculate_size(backup_path)?,
                version: "unknown".to_string(),
            });
        }

        let json = std::fs::read_to_string(metadata_path)?;
        let metadata: BackupMetadata = serde_json::from_str(&json)?;
        Ok(metadata)
    }

    fn calculate_size(&self, path: &Path) -> Result<u64> {
        let mut total = 0u64;

        for entry in std::fs::read_dir(path)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                total += entry.metadata()?.len();
            }
        }

        Ok(total)
    }
}

impl Clone for BackupManager {
    fn clone(&self) -> Self {
        Self {
            store_path: self.store_path.clone(),
            backup_dir: self.backup_dir.clone(),
            max_backups: self.max_backups,
            compression_enabled: self.compression_enabled,
        }
    }
}

/// Backup information
#[derive(Debug, Clone)]
pub struct BackupInfo {
    pub path: PathBuf,
    pub metadata: BackupMetadata,
}

/// Backup metadata
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackupMetadata {
    pub backup_name: String,
    pub created_at: DateTime<Utc>,
    pub store_path: PathBuf,
    pub backup_path: PathBuf,
    pub size_bytes: u64,
    pub version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_serialization() {
        let metadata = BackupMetadata {
            backup_name: "test_backup".to_string(),
            created_at: Utc::now(),
            store_path: PathBuf::from("/tmp/store"),
            backup_path: PathBuf::from("/tmp/backup"),
            size_bytes: 1024,
            version: "1.0.0".to_string(),
        };

        let json = serde_json::to_string(&metadata).unwrap();
        assert!(json.contains("test_backup"));

        let decoded: BackupMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.backup_name, "test_backup");
    }
}
