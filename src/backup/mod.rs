// Backup Module for DMPool
// Handles database backup, compression, validation, and recovery

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::info;

/// Validate a path is safe for use with external commands
fn validate_safe_path(path: &Path) -> Result<()> {
    let path_str = path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8 characters"))?;

    // Must be absolute path
    if !path_str.starts_with('/') {
        return Err(anyhow::anyhow!("Path must be absolute: {}", path_str));
    }

    // Check for dangerous characters or patterns
    let dangerous_patterns = [
        ";",          // Command separator
        "&",          // Background operator
        "|",          // Pipe operator
        "$(",         // Command substitution
        "`",          // Command substitution
        "\n",         // Newline injection
        "\r",         // Carriage return
        "\t",         // Tab
        ">",          // Redirect output
        "<",          // Redirect input
        "*/../",      // Directory traversal
        "..",         // Parent directory (might be okay in some contexts)
        "\\0",        // Null byte
    ];

    for pattern in &dangerous_patterns {
        if path_str.contains(pattern) {
            // ".." might be okay in some contexts, so check more carefully
            if *pattern == ".." {
                // Only allow ".." as a path component (e.g., "/home/../user" is okay)
                // But not at the start or suspicious positions
                if path_str == "/.." || path_str.contains("/../") {
                    // Check if it's trying to escape root
                    continue;
                }
            }
            return Err(anyhow::anyhow!("Path contains dangerous pattern '{}': {}", pattern, path_str));
        }
    }

    // Check if path component starts with "-" (could be interpreted as tar option)
    for component in path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            if name.starts_with('-') && name.len() > 1 {
                return Err(anyhow::anyhow!("Path component starts with dash (could be interpreted as option): {}", name));
            }
        }
    }

    Ok(())
}

/// Safely convert path to string for command arguments
fn safe_path_str(path: &Path) -> Result<String> {
    validate_safe_path(path)?;
    Ok(path.to_str()
        .ok_or_else(|| anyhow::anyhow!("Path contains invalid UTF-8 characters"))?
        .to_string())
}

/// Backup configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupConfig {
    /// Source database path
    pub db_path: PathBuf,
    /// Backup directory
    pub backup_dir: PathBuf,
    /// Number of backups to retain
    pub retention_count: usize,
    /// Enable compression (gzip)
    pub compress: bool,
    /// Backup interval in hours
    pub interval_hours: u64,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            db_path: PathBuf::from("./data"),
            backup_dir: PathBuf::from("./backups"),
            retention_count: 7,
            compress: true,
            interval_hours: 24,
        }
    }
}

/// Backup metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupMetadata {
    /// Unique backup ID
    pub id: String,
    /// Timestamp of backup
    pub timestamp: DateTime<Utc>,
    /// Backup file path
    pub file_path: PathBuf,
    /// Original database size in bytes
    pub original_size: u64,
    /// Backup size in bytes (after compression if enabled)
    pub backup_size: u64,
    /// Compression ratio (if compressed)
    pub compression_ratio: Option<f64>,
    /// Whether backup is validated
    pub validated: bool,
    /// Schema version at time of backup
    pub schema_version: u32,
    /// Checksum for integrity verification
    pub checksum: String,
}

/// Backup statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupStats {
    pub total_backups: usize,
    pub total_size_bytes: u64,
    pub latest_backup: Option<DateTime<Utc>>,
    pub oldest_backup: Option<DateTime<Utc>>,
    pub disk_usage_bytes: u64,
}

/// Backup manager
pub struct BackupManager {
    config: BackupConfig,
}

impl BackupManager {
    /// Create a new backup manager
    pub fn new(config: BackupConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(BackupConfig::default())
    }

    /// Ensure backup directory exists
    fn ensure_backup_dir(&self) -> Result<()> {
        if !self.config.backup_dir.exists() {
            fs::create_dir_all(&self.config.backup_dir)
                .context("Failed to create backup directory")?;
        }
        Ok(())
    }

    /// Generate backup filename
    fn generate_backup_filename(&self) -> String {
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let compression_suffix = if self.config.compress { ".tar.gz" } else { ".tar" };
        format!("dmpool_backup_{}{}", timestamp, compression_suffix)
    }

    /// Get current schema version (simplified - should read from DB)
    fn get_schema_version(&self) -> u32 {
        // TODO: Read actual schema version from database
        1
    }

    /// Calculate file checksum (SHA-256)
    fn calculate_checksum(&self, file_path: &Path) -> Result<String> {
        use sha2::{Digest, Sha256};
        let mut file = fs::File::open(file_path)
            .context("Failed to open file for checksum")?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)
            .context("Failed to read file for checksum")?;
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Get directory size
    fn get_dir_size(&self, path: &Path) -> Result<u64> {
        let mut total = 0u64;
        if path.is_dir() {
            for entry in fs::read_dir(path)
                .context("Failed to read directory")?
            {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    total += self.get_dir_size(&path)?;
                } else {
                    total += entry.metadata()?.len();
                }
            }
        }
        Ok(total)
    }

    /// Create a backup
    pub async fn create_backup(&self) -> Result<BackupMetadata> {
        self.ensure_backup_dir()?;

        if !self.config.db_path.exists() {
            return Err(anyhow::anyhow!("Database path does not exist: {:?}", self.config.db_path));
        }

        let backup_id = uuid::Uuid::new_v4().to_string();
        let filename = self.generate_backup_filename();
        let backup_path = self.config.backup_dir.join(&filename);

        info!("Creating backup: {}", filename);

        // Get original database size
        let original_size = self.get_dir_size(&self.config.db_path)?;

        // Validate all paths before using them
        let backup_path_str = safe_path_str(&backup_path)?;
        let parent_dir = self.config.db_path.parent()
            .unwrap_or(Path::new("."));
        let parent_dir_str = safe_path_str(&parent_dir)?;

        // Use "./" prefix for file argument to prevent it from being interpreted as an option
        let db_file = self.config.db_path.file_name()
            .ok_or_else(|| anyhow::anyhow!("Database path has no file name"))?;

        // Validate the file name doesn't contain dangerous characters
        let db_file_str = db_file.to_str()
            .ok_or_else(|| anyhow::anyhow!("Database file name contains invalid UTF-8"))?;

        // Check if file name starts with dash
        let db_file_safe = if db_file_str.starts_with('-') {
            format!("./{}", db_file_str)
        } else {
            db_file_str.to_string()
        };

        // Validate file name for safety
        if db_file_str.contains(';') || db_file_str.contains('&') || db_file_str.contains('|')
            || db_file_str.contains('$') || db_file_str.contains('`') || db_file_str.contains('\\')
            || db_file_str.contains('\n') || db_file_str.contains('\r') {
            return Err(anyhow::anyhow!("Database file name contains dangerous characters: {}", db_file_str));
        }

        // Create tar archive (optionally compressed)
        let status = if self.config.compress {
            Command::new("tar")
                .args([
                    "-czf",
                    &backup_path_str,
                    "-C",
                    &parent_dir_str,
                    &db_file_safe,
                ])
                .status()
                .context("Failed to execute tar command")?
        } else {
            Command::new("tar")
                .args([
                    "-cf",
                    &backup_path_str,
                    "-C",
                    &parent_dir_str,
                    &db_file_safe,
                ])
                .status()
                .context("Failed to execute tar command")?
        };

        if !status.success() {
            return Err(anyhow::anyhow!("Backup creation failed with exit code: {:?}", status.code()));
        }

        // Get backup size
        let backup_size = fs::metadata(&backup_path)
            .context("Failed to get backup file metadata")?
            .len();

        // Calculate compression ratio
        let compression_ratio = if self.config.compress && original_size > 0 {
            Some((original_size as f64 - backup_size as f64) / original_size as f64 * 100.0)
        } else {
            None
        };

        // Calculate checksum
        let checksum = self.calculate_checksum(&backup_path)?;

        let metadata = BackupMetadata {
            id: backup_id,
            timestamp: Utc::now(),
            file_path: backup_path.clone(),
            original_size,
            backup_size,
            compression_ratio,
            validated: false,
            schema_version: self.get_schema_version(),
            checksum,
        };

        // Save metadata
        self.save_metadata(&metadata)?;

        // Validate the backup
        self.validate_backup(&metadata).await?;

        info!(
            "Backup created successfully: {} (size: {} bytes, compressed: {:.1}%)",
            filename,
            backup_size,
            compression_ratio.unwrap_or(0.0)
        );

        Ok(metadata)
    }

    /// Save backup metadata to JSON file
    fn save_metadata(&self, metadata: &BackupMetadata) -> Result<()> {
        let meta_path = self.get_metadata_path(&metadata.id);
        let json = serde_json::to_string_pretty(metadata)
            .context("Failed to serialize metadata")?;
        fs::write(&meta_path, json)
            .context("Failed to write metadata file")?;
        Ok(())
    }

    /// Get metadata file path for a backup ID
    fn get_metadata_path(&self, backup_id: &str) -> PathBuf {
        self.config.backup_dir.join(format!("{}.meta.json", backup_id))
    }

    /// Load backup metadata
    pub fn load_metadata(&self, backup_id: &str) -> Result<BackupMetadata> {
        let meta_path = self.get_metadata_path(backup_id);
        let json = fs::read_to_string(&meta_path)
            .context("Failed to read metadata file")?;
        let metadata: BackupMetadata = serde_json::from_str(&json)
            .context("Failed to parse metadata")?;
        Ok(metadata)
    }

    /// Validate backup integrity
    pub async fn validate_backup(&self, metadata: &BackupMetadata) -> Result<bool> {
        info!("Validating backup: {}", metadata.id);

        // Check if backup file exists
        if !metadata.file_path.exists() {
            return Err(anyhow::anyhow!("Backup file not found: {:?}", metadata.file_path));
        }

        // Verify checksum
        let current_checksum = self.calculate_checksum(&metadata.file_path)?;
        if current_checksum != metadata.checksum {
            return Err(anyhow::anyhow!(
                "Backup checksum mismatch: expected {}, got {}",
                metadata.checksum,
                current_checksum
            ));
        }

        // Update metadata as validated
        let mut updated = metadata.clone();
        updated.validated = true;
        self.save_metadata(&updated)?;

        info!("Backup validated successfully: {}", metadata.id);
        Ok(true)
    }

    /// List all backups
    pub fn list_backups(&self) -> Result<Vec<BackupMetadata>> {
        let mut backups = Vec::new();

        if !self.config.backup_dir.exists() {
            return Ok(backups);
        }

        for entry in fs::read_dir(&self.config.backup_dir)
            .context("Failed to read backup directory")?
        {
            let entry = entry?;
            let path = entry.path();

            // Load metadata files
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                    if name.ends_with(".meta.json") {
                        let backup_id = name.trim_end_matches(".meta.json");
                        if let Ok(metadata) = self.load_metadata(backup_id) {
                            backups.push(metadata);
                        }
                    }
                }
            }
        }

        // Sort by timestamp (newest first)
        backups.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(backups)
    }

    /// Get backup statistics
    pub fn get_stats(&self) -> Result<BackupStats> {
        let backups = self.list_backups()?;

        let total_size_bytes: u64 = backups.iter().map(|b| b.backup_size).sum();
        let disk_usage_bytes = self.get_dir_size(&self.config.backup_dir).unwrap_or(0);

        Ok(BackupStats {
            total_backups: backups.len(),
            total_size_bytes,
            latest_backup: backups.first().map(|b| b.timestamp),
            oldest_backup: backups.last().map(|b| b.timestamp),
            disk_usage_bytes,
        })
    }

    /// Restore from a backup
    pub async fn restore_backup(&self, backup_id: &str, target_path: Option<&Path>) -> Result<()> {
        let metadata = self.load_metadata(backup_id)?;

        info!("Restoring backup: {} from {:?}", backup_id, metadata.file_path);

        // Validate checksum before restore
        let current_checksum = self.calculate_checksum(&metadata.file_path)?;
        if current_checksum != metadata.checksum {
            return Err(anyhow::anyhow!(
                "Backup checksum mismatch - restore aborted"
            ));
        }

        let restore_path = target_path.unwrap_or(&self.config.db_path);

        // Ensure target directory exists or create it
        if !restore_path.exists() {
            fs::create_dir_all(restore_path)
                .context("Failed to create restore directory")?;
        }

        // Extract backup
        let status = Command::new("tar")
            .args([
                "-xzf",
                metadata.file_path.to_str().unwrap(),
                "-C",
                restore_path.parent().unwrap_or(Path::new(".")).to_str().unwrap(),
            ])
            .status()
            .context("Failed to execute tar extract command")?;

        if !status.success() {
            return Err(anyhow::anyhow!("Backup extraction failed with exit code: {:?}", status.code()));
        }

        info!("Backup restored successfully to: {:?}", restore_path);
        Ok(())
    }

    /// Delete old backups based on retention policy
    pub async fn cleanup_old_backups(&self) -> Result<usize> {
        let mut backups = self.list_backups()?;
        let deleted_count = 0;

        if backups.len() <= self.config.retention_count {
            info!("No old backups to clean up ({} <= {})", backups.len(), self.config.retention_count);
            return Ok(0);
        }

        // Remove oldest backups beyond retention limit
        while backups.len() > self.config.retention_count {
            if let Some(backup) = backups.pop() {
                // Delete backup file
                if backup.file_path.exists() {
                    fs::remove_file(&backup.file_path)
                        .context("Failed to delete backup file")?;
                }

                // Delete metadata file
                let meta_path = self.get_metadata_path(&backup.id);
                if meta_path.exists() {
                    fs::remove_file(&meta_path)
                        .context("Failed to delete metadata file")?;
                }

                info!("Deleted old backup: {}", backup.id);
            }
        }

        Ok(deleted_count)
    }

    /// Delete a specific backup
    pub async fn delete_backup(&self, backup_id: &str) -> Result<bool> {
        let metadata = self.load_metadata(backup_id)?;

        // Delete backup file
        if metadata.file_path.exists() {
            fs::remove_file(&metadata.file_path)
                .context("Failed to delete backup file")?;
        }

        // Delete metadata file
        let meta_path = self.get_metadata_path(backup_id);
        if meta_path.exists() {
            fs::remove_file(&meta_path)
                .context("Failed to delete metadata file")?;
        }

        info!("Deleted backup: {}", backup_id);
        Ok(true)
    }
}
