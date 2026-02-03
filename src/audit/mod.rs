// Audit Logging module for DMPool Admin
// Records all admin operations for security and compliance
// Supports file-based persistence for long-term storage

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::{File, OpenOptions};
use tokio::io::{AsyncWriteExt, AsyncReadExt};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Audit log entry
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditLog {
    /// Unique ID for this log entry
    pub id: String,
    /// Timestamp of the operation
    pub timestamp: DateTime<Utc>,
    /// User who performed the action
    pub username: String,
    /// Action performed (e.g., "login", "config_update", "ban_worker")
    pub action: String,
    /// Resource affected (e.g., "/api/config", "worker:address")
    pub resource: String,
    /// IP address of the user
    pub ip_address: String,
    /// Additional details (JSON)
    pub details: serde_json::Value,
    /// Success or failure
    pub success: bool,
    /// Error message if failed
    pub error: Option<String>,
}

/// Audit log filter options
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditFilter {
    /// Filter by username
    pub username: Option<String>,
    /// Filter by action
    pub action: Option<String>,
    /// Filter by resource
    pub resource: Option<String>,
    /// Start time (Unix timestamp)
    pub start_time: Option<i64>,
    /// End time (Unix timestamp)
    pub end_time: Option<i64>,
    /// Maximum results to return
    pub limit: Option<usize>,
}

impl Default for AuditFilter {
    fn default() -> Self {
        Self {
            username: None,
            action: None,
            resource: None,
            start_time: None,
            end_time: None,
            limit: Some(100),
        }
    }
}

/// Audit log manager with file persistence
pub struct AuditLogger {
    /// In-memory cache for recent logs
    logs: Arc<RwLock<Vec<AuditLog>>>,
    /// Maximum number of logs to keep in memory
    max_logs: usize,
    /// Path to the audit log file (JSONL format)
    log_file: Option<PathBuf>,
    /// Whether to enable file persistence
    persistence_enabled: bool,
}

impl AuditLogger {
    /// Create a new audit logger with file persistence
    pub fn new(max_logs: usize, log_file: Option<PathBuf>) -> Self {
        let persistence_enabled = log_file.is_some();
        Self {
            logs: Arc::new(RwLock::new(Vec::new())),
            max_logs,
            log_file,
            persistence_enabled,
        }
    }

    /// Create with default settings and no file persistence
    pub fn default() -> Self {
        Self::new(10000, None)
    }

    /// Create with file persistence (async version)
    pub async fn with_persistence_async(max_logs: usize, log_dir: PathBuf) -> Result<Self> {
        // Ensure log directory exists
        tokio::fs::create_dir_all(&log_dir).await
            .context("Failed to create audit log directory")?;

        let log_file = log_dir.join("audit.jsonl");
        Ok(Self::new(max_logs, Some(log_file)))
    }

    /// Create with file persistence (blocking version using std::fs)
    pub fn with_persistence_blocking(max_logs: usize, log_dir: PathBuf) -> Result<Self> {
        // Ensure log directory exists
        std::fs::create_dir_all(&log_dir)
            .context("Failed to create audit log directory")?;

        let log_file = log_dir.join("audit.jsonl");
        Ok(Self::new(max_logs, Some(log_file)))
    }

    /// Log an action
    pub async fn log(&self, entry: AuditLog) {
        // Write to file if persistence is enabled
        if self.persistence_enabled {
            if let Some(ref log_file) = self.log_file {
                if let Err(e) = Self::append_to_file(log_file, &entry).await {
                    error!("Failed to write audit log to file: {}", e);
                }
            }
        }

        let mut logs = self.logs.write().await;

        // Add log
        logs.push(entry.clone());

        // Trim if exceeded max
        if logs.len() > self.max_logs {
            let remove_count = logs.len() - self.max_logs;
            logs.drain(0..remove_count);
            warn!("Removed {} old audit logs to stay under limit", remove_count);
        }

        // Log to tracing
        if entry.success {
            info!(
                "AUDIT: {} {} {} from {}",
                entry.username,
                entry.action,
                entry.resource,
                entry.ip_address
            );
        } else {
            warn!(
                "AUDIT: FAILED {} {} {} from {}: {}",
                entry.username,
                entry.action,
                entry.resource,
                entry.ip_address,
                entry.error.as_deref().unwrap_or(&"unknown".to_string())
            );
        }
    }

    /// Append a log entry to the file (JSONL format - one JSON per line)
    async fn append_to_file(log_file: &PathBuf, entry: &AuditLog) -> Result<()> {
        let json_str = serde_json::to_string(entry)
            .context("Failed to serialize audit log")?;

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .append(true)
            .open(log_file)
            .await
            .context("Failed to open audit log file")?;

        file.write_all(json_str.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(())
    }

    /// Load audit logs from file on startup
    pub async fn load_from_file(&self) -> Result<usize> {
        if !self.persistence_enabled {
            return Ok(0);
        }

        let log_file = self.log_file.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No log file configured"))?;

        if !log_file.exists() {
            return Ok(0);
        }

        let mut file = File::open(log_file).await
            .context("Failed to open audit log file")?;

        let mut contents = Vec::new();
        file.read_to_end(&mut contents).await
            .context("Failed to read audit log file")?;

        let mut logs = self.logs.write().await;
        let initial_count = logs.len();

        for line in contents.split(|&b| b == b'\n') {
            if line.is_empty() {
                continue;
            }

            let json_str = std::str::from_utf8(line)
                .context("Invalid UTF-8 in audit log")?;

            if let Ok(entry) = serde_json::from_str::<AuditLog>(json_str) {
                logs.push(entry);
            }
        }

        let loaded_count = logs.len() - initial_count;
        info!("Loaded {} audit logs from file", loaded_count);

        Ok(loaded_count)
    }

    /// Create a new audit log entry builder
    pub fn entry(&self, username: String, action: String, resource: String, ip_address: String) -> AuditLogBuilder {
        AuditLogBuilder {
            username,
            action,
            resource,
            ip_address,
            details: serde_json::json!({}),
            success: true,
            error: None,
            logger: self.logs.clone(),
        }
    }

    /// Query audit logs with optional filter
    pub async fn query(&self, filter: AuditFilter) -> Vec<AuditLog> {
        let logs = self.logs.read().await;
        let mut results = logs.clone();

        // Apply filters
        if let Some(username) = &filter.username {
            results.retain(|log| log.username == *username);
        }
        if let Some(action) = &filter.action {
            results.retain(|log| log.action == *action);
        }
        if let Some(resource) = &filter.resource {
            results.retain(|log| log.resource.contains(resource));
        }
        if let Some(start) = filter.start_time {
            let start_dt = DateTime::from_timestamp(start, 0).unwrap_or_default();
            results.retain(|log| log.timestamp >= start_dt);
        }
        if let Some(end) = filter.end_time {
            let end_dt = DateTime::from_timestamp(end, 0).unwrap_or_else(|| Utc::now());
            results.retain(|log| log.timestamp <= end_dt);
        }

        // Reverse to show newest first
        results.reverse();

        // Apply limit
        if let Some(limit) = filter.limit {
            results.truncate(limit);
        }

        results
    }

    /// Get recent audit logs
    pub async fn recent(&self, count: usize) -> Vec<AuditLog> {
        let logs = self.logs.read().await;
        let len = logs.len();
        let start = if len > count { len - count } else { 0 };
        logs[start..].to_vec()
    }

    /// Get all audit logs (use with caution)
    pub async fn all(&self) -> Vec<AuditLog> {
        let logs = self.logs.read().await;
        logs.clone()
    }

    /// Clear old audit logs (older than specified days)
    pub async fn cleanup_old(&self, days: i64) -> Result<usize> {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        let mut logs = self.logs.write().await;
        let original_len = logs.len();
        logs.retain(|log| log.timestamp > cutoff);
        Ok(original_len - logs.len())
    }

    /// Get statistics about audit logs
    pub async fn stats(&self) -> AuditStats {
        let logs = self.logs.read().await;
        let mut action_counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        let mut success_count = 0;
        let mut failure_count = 0;

        for log in logs.iter() {
            *action_counts.entry(log.action.clone()).or_insert(0) += 1;
            if log.success {
                success_count += 1;
            } else {
                failure_count += 1;
            }
        }

        let mut top_actions: Vec<(String, usize)> = action_counts.into_iter().collect();
        top_actions.sort_by(|a, b| b.1.cmp(&a.1));
        top_actions.truncate(10);

        AuditStats {
            total_logs: logs.len(),
            success_count,
            failure_count,
            top_actions,
            oldest_log: logs.first().map(|l| l.timestamp),
            newest_log: logs.last().map(|l| l.timestamp),
        }
    }

    /// Rotate audit log file (move current to archive and start fresh)
    pub async fn rotate_logs(&self) -> Result<PathBuf> {
        if !self.persistence_enabled {
            return Err(anyhow::anyhow!("File persistence not enabled"));
        }

        let log_file = self.log_file.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No log file configured"))?;

        if !log_file.exists() {
            return Err(anyhow::anyhow!("Log file does not exist"));
        }

        // Create archive filename with timestamp
        let timestamp = Utc::now().format("%Y%m%d_%H%M%S");
        let archive_path = log_file.with_file_name(format!("audit_{}.jsonl", timestamp));

        // Move current log to archive
        tokio::fs::rename(log_file, &archive_path).await
            .context("Failed to rotate audit log file")?;

        info!("Rotated audit log: {:?} -> {:?}", log_file, archive_path);

        Ok(archive_path)
    }

    /// Export audit logs to JSON file
    pub async fn export(&self, output_path: PathBuf) -> Result<usize> {
        let logs = self.logs.read().await;

        let mut file = File::create(&output_path).await
            .context("Failed to create export file")?;

        for log in logs.iter() {
            let json_str = serde_json::to_string(log)
                .context("Failed to serialize audit log")?;
            file.write_all(json_str.as_bytes()).await?;
            file.write_all(b"\n").await?;
        }

        file.flush().await?;

        info!("Exported {} audit logs to {:?}", logs.len(), output_path);
        Ok(logs.len())
    }

    /// Get log file path if persistence is enabled
    pub fn log_file_path(&self) -> Option<&PathBuf> {
        self.log_file.as_ref()
    }
}

/// Audit statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuditStats {
    pub total_logs: usize,
    pub success_count: usize,
    pub failure_count: usize,
    pub top_actions: Vec<(String, usize)>,
    pub oldest_log: Option<DateTime<Utc>>,
    pub newest_log: Option<DateTime<Utc>>,
}

/// Builder for creating audit log entries
pub struct AuditLogBuilder {
    username: String,
    action: String,
    resource: String,
    ip_address: String,
    details: serde_json::Value,
    success: bool,
    error: Option<String>,
    logger: Arc<RwLock<Vec<AuditLog>>>,
}

impl AuditLogBuilder {
    /// Add details to the log entry
    pub fn details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }

    /// Set success status
    pub fn success(mut self, success: bool) -> Self {
        self.success = success;
        self
    }

    /// Set error message
    pub fn error(mut self, error: String) -> Self {
        self.error = Some(error);
        self.success = false;
        self
    }

    /// Build and log the entry
    pub async fn log(self) {
        let error_msg = self.error.clone();
        let entry = AuditLog {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp: Utc::now(),
            username: self.username,
            action: self.action,
            resource: self.resource,
            ip_address: self.ip_address,
            details: self.details,
            success: self.success,
            error: error_msg.clone(),
        };

        let mut logs = self.logger.write().await;
        logs.push(entry.clone());

        // Log to tracing
        if self.success {
            info!(
                "AUDIT: {} {} {} from {}",
                entry.username,
                entry.action,
                entry.resource,
                entry.ip_address
            );
        } else {
            warn!(
                "AUDIT: FAILED {} {} {} from {}: {}",
                entry.username,
                entry.action,
                entry.resource,
                entry.ip_address,
                error_msg.as_deref().unwrap_or(&"unknown".to_string())
            );
        }
    }
}

/// Helper macro for creating audit log entries
#[macro_export]
macro_rules! audit_log {
    ($logger:expr, $username:expr, $action:expr, $resource:expr, $ip:expr) => {
        $logger.entry(
            $username.to_string(),
            $action.to_string(),
            $resource.to_string(),
            $ip.to_string(),
        )
    };
    ($logger:expr, $username:expr, $action:expr, $resource:expr, $ip:expr, details=$details:expr) => {
        $logger.entry(
            $username.to_string(),
            $action.to_string(),
            $resource.to_string(),
            $ip.to_string(),
        )
        .details($details)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_audit_log_creation() {
        let logger = AuditLogger::new(100, None);
        let entry = AuditLog {
            id: "test-1".to_string(),
            timestamp: Utc::now(),
            username: "admin".to_string(),
            action: "login".to_string(),
            resource: "/api/auth/login".to_string(),
            ip_address: "127.0.0.1".to_string(),
            details: json!({}),
            success: true,
            error: None,
        };

        logger.log(entry).await;
        assert_eq!(logger.all().await.len(), 1);
    }

    #[tokio::test]
    async fn test_audit_log_query() {
        let logger = AuditLogger::new(100, None);

        // Add some test logs
        logger.log(AuditLog {
            id: "1".to_string(),
            timestamp: Utc::now(),
            username: "admin".to_string(),
            action: "login".to_string(),
            resource: "/api/auth/login".to_string(),
            ip_address: "127.0.0.1".to_string(),
            details: json!({}),
            success: true,
            error: None,
        }).await;

        logger.log(AuditLog {
            id: "2".to_string(),
            timestamp: Utc::now(),
            username: "user".to_string(),
            action: "config_update".to_string(),
            resource: "/api/config".to_string(),
            ip_address: "127.0.0.2".to_string(),
            details: json!({}),
            success: true,
            error: None,
        }).await;

        // Query for admin logs
        let filter = AuditFilter {
            username: Some("admin".to_string()),
            ..Default::default()
        };
        let results = logger.query(filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].username, "admin");
    }

    #[tokio::test]
    async fn test_audit_log_max_limit() {
        let logger = AuditLogger::new(5, None);

        // Add 10 logs
        for i in 0..10 {
            logger.log(AuditLog {
                id: format!("test-{}", i),
                timestamp: Utc::now(),
                username: "admin".to_string(),
                action: "test".to_string(),
                resource: "/test".to_string(),
                ip_address: "127.0.0.1".to_string(),
                details: json!({}),
                success: true,
                error: None,
            }).await;
        }

        // Should only keep last 5
        let all = logger.all().await;
        assert_eq!(all.len(), 5);
    }
}
