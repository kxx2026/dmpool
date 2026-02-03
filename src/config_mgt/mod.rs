// Smart Configuration Management for DMPool
// Provides versioning, rollback, validation, and diff capabilities

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Configuration version with metadata
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigVersion {
    /// Version ID (e.g., "v20250102120000")
    pub id: String,
    /// Timestamp when this version was created
    pub created_at: DateTime<Utc>,
    /// User who created this version
    pub created_by: String,
    /// Description of changes
    pub description: String,
    /// Parent version ID (for rollback chain)
    pub parent_id: Option<String>,
    /// Configuration data (serialized)
    pub config_data: serde_json::Value,
    /// Validation status
    pub validation_status: ValidationStatus,
}

/// Validation status for configuration
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ValidationStatus {
    /// Not yet validated
    Pending,
    /// Validation passed
    Valid,
    /// Validation failed with errors
    Invalid { errors: Vec<String> },
}

/// Configuration diff between two versions
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigDiff {
    pub version_a: String,
    pub version_b: String,
    pub changes: Vec<ConfigChange>,
    pub summary: ConfigDiffSummary,
}

/// Individual configuration change
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigChange {
    /// Parameter path (e.g., "stratum.port")
    pub path: String,
    /// Old value
    pub old_value: serde_json::Value,
    /// New value
    pub new_value: serde_json::Value,
    /// Type of change
    pub change_type: ChangeType,
}

/// Type of configuration change
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ChangeType {
    /// Value added
    Added,
    /// Value removed
    Removed,
    /// Value modified
    Modified,
    /// No change
    Unchanged,
}

/// Summary of configuration diff
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigDiffSummary {
    pub total_changes: usize,
    pub added: usize,
    pub removed: usize,
    pub modified: usize,
    pub critical_changes: Vec<String>,
}

/// Scheduled configuration change
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScheduledChange {
    /// Unique ID
    pub id: String,
    /// Target version to apply
    pub target_version_id: String,
    /// Scheduled time
    pub scheduled_at: DateTime<Utc>,
    /// Status
    pub status: ScheduleStatus,
    /// Created by
    pub created_by: String,
}

/// Status of scheduled change
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ScheduleStatus {
    Pending,
    Applied,
    Failed { error: String },
    Cancelled,
}

/// Configuration schema for validation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigSchema {
    pub parameter_name: String,
    pub parameter_type: ConfigType,
    pub required: bool,
    pub default_value: Option<serde_json::Value>,
    pub validation_rules: Vec<ValidationRule>,
    pub description: String,
}

/// Configuration parameter types
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ConfigType {
    String,
    Integer { min: i64, max: i64 },
    Float { min: f64, max: f64 },
    Boolean,
    Enum { options: Vec<String> },
}

/// Validation rule
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationRule {
    pub rule_type: String,
    pub params: serde_json::Value,
    pub error_message: String,
}

/// Smart configuration manager
pub struct ConfigManager {
    /// Current active version
    current_version: Arc<RwLock<Option<String>>>,
    /// All configuration versions
    versions: Arc<RwLock<HashMap<String, ConfigVersion>>>,
    /// Storage directory for versions
    storage_dir: PathBuf,
    /// Configuration schema
    schema: Arc<RwLock<HashMap<String, ConfigSchema>>>,
    /// Scheduled changes
    scheduled_changes: Arc<RwLock<Vec<ScheduledChange>>>,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new(storage_dir: PathBuf) -> Self {
        Self {
            current_version: Arc::new(RwLock::new(None)),
            versions: Arc::new(RwLock::new(HashMap::new())),
            storage_dir,
            schema: Arc::new(RwLock::new(Self::build_default_schema())),
            scheduled_changes: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize with default schema
    fn build_default_schema() -> HashMap<String, ConfigSchema> {
        let mut schema = HashMap::new();

        // Stratum settings
        schema.insert("stratum.port".to_string(), ConfigSchema {
            parameter_name: "stratum.port".to_string(),
            parameter_type: ConfigType::Integer { min: 1, max: 65535 },
            required: true,
            default_value: Some(serde_json::json!(3333)),
            validation_rules: vec![],
            description: "Stratum server port".to_string(),
        });

        schema.insert("stratum.start_difficulty".to_string(), ConfigSchema {
            parameter_name: "stratum.start_difficulty".to_string(),
            parameter_type: ConfigType::Integer { min: 8, max: 512 },
            required: true,
            default_value: Some(serde_json::json!(32)),
            validation_rules: vec![],
            description: "Initial difficulty for new connections".to_string(),
        });

        // PPLNS settings
        schema.insert("pplns_ttl_days".to_string(), ConfigSchema {
            parameter_name: "pplns.ttl_days".to_string(),
            parameter_type: ConfigType::Integer { min: 1, max: 30 },
            required: true,
            default_value: Some(serde_json::json!(7)),
            validation_rules: vec![
                ValidationRule {
                    rule_type: "range_warning".to_string(),
                    params: serde_json::json!({"min": 7}),
                    error_message: "TTL below 7 days may cause miner loss".to_string(),
                }
            ],
            description: "PPLNS time-to-live in days".to_string(),
        });

        schema.insert("donation".to_string(), ConfigSchema {
            parameter_name: "donation".to_string(),
            parameter_type: ConfigType::Integer { min: 0, max: 10000 },
            required: true,
            default_value: Some(serde_json::json!(0)),
            validation_rules: vec![
                ValidationRule {
                    rule_type: "critical".to_string(),
                    params: serde_json::json!({"forbidden": 10000}),
                    error_message: "Donation of 100% (10000 basis points) prevents payouts!".to_string(),
                }
            ],
            description: "Pool donation in basis points (0-10000)".to_string(),
        });

        schema
    }

    /// Initialize the configuration manager
    pub async fn initialize(&self) -> Result<()> {
        // Create storage directory
        fs::create_dir_all(&self.storage_dir).await
            .context("Failed to create config storage directory")?;

        // Load existing versions
        self.load_versions().await?;

        info!("Configuration manager initialized with {} versions", 
            self.versions.read().await.len());

        Ok(())
    }

    /// Load existing configuration versions from disk
    async fn load_versions(&self) -> Result<()> {
        let mut versions = self.versions.write().await;
        
        let mut entries = fs::read_dir(&self.storage_dir).await
            .context("Failed to read config storage directory")?;

        while let Some(entry) = entries.next_entry().await
            .context("Failed to read directory entry")? {
                let path = entry.path();
                
                // Only load .json files
                if path.extension().and_then(|s| s.to_str()) != Some("json") {
                    continue;
                }

                let json = fs::read_to_string(&path).await
                    .context("Failed to read version file")?;
                
                let version: ConfigVersion = serde_json::from_str(&json)
                    .context("Failed to parse version file")?;
                
                versions.insert(version.id.clone(), version);
            }

        // Load current version pointer
        let current_file = self.storage_dir.join("current.txt");
        if current_file.exists() {
            let current_id = fs::read_to_string(&current_file).await
                .context("Failed to read current version pointer")?;
            *self.current_version.write().await = Some(current_id);
        }

        Ok(())
    }

    /// Create a new configuration version
    pub async fn create_version(
        &self,
        config_data: serde_json::Value,
        description: String,
        created_by: String,
    ) -> Result<ConfigVersion> {
        // Validate the configuration
        let validation_status = self.validate_config(&config_data).await;

        if !matches!(validation_status, ValidationStatus::Valid) {
            return Err(anyhow::anyhow!(
                "Configuration validation failed: {:?}",
                validation_status
            ));
        }

        // Generate version ID
        let version_id = format!("v{}", Utc::now().format("%Y%m%d%H%M%S"));

        // Get parent version
        let parent_id = self.current_version.read().await.clone();

        let version = ConfigVersion {
            id: version_id.clone(),
            created_at: Utc::now(),
            created_by,
            description: description.clone(),
            parent_id,
            config_data,
            validation_status,
        };

        // Save to disk
        self.save_version(&version).await?;

        // Update current version
        *self.current_version.write().await = Some(version_id.clone());
        self.update_current_pointer(&version_id).await?;

        // Store in memory
        let mut versions = self.versions.write().await;
        versions.insert(version_id.clone(), version.clone());

        info!("Created configuration version {}: {}", version_id, description);

        Ok(version)
    }

    /// Save configuration version to disk
    async fn save_version(&self, version: &ConfigVersion) -> Result<()> {
        let version_file = self.storage_dir.join(format!("{}.json", version.id));
        
        let json = serde_json::to_string_pretty(version)
            .context("Failed to serialize version")?;
        
        fs::write(&version_file, json).await
            .context("Failed to write version file")?;

        Ok(())
    }

    /// Update the current version pointer
    async fn update_current_pointer(&self, version_id: &str) -> Result<()> {
        let current_file = self.storage_dir.join("current.txt");
        fs::write(&current_file, version_id).await
            .context("Failed to write current version pointer")?;
        Ok(())
    }

    /// Get the current configuration version
    pub async fn current_version(&self) -> Option<ConfigVersion> {
        let current_id = self.current_version.read().await.clone()?;
        let versions = self.versions.read().await;
        versions.get(&current_id).cloned()
    }

    /// Get a specific version by ID
    pub async fn get_version(&self, version_id: &str) -> Option<ConfigVersion> {
        let versions = self.versions.read().await;
        versions.get(version_id).cloned()
    }

    /// List all versions
    pub async fn list_versions(&self) -> Vec<ConfigVersion> {
        let versions = self.versions.read().await;
        let mut list: Vec<_> = versions.values().cloned().collect();
        list.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        list
    }

    /// Validate configuration against schema
    pub async fn validate_config(&self, config: &serde_json::Value) -> ValidationStatus {
        let schema = self.schema.read().await;
        let mut errors = Vec::new();

        // Check each parameter against schema
        for (path, param_schema) in schema.iter() {
            let value = config.get(path);

            // Check required fields
            if param_schema.required && value.is_none() {
                errors.push(format!("{} is required", path));
                continue;
            }

            if let Some(val) = value {
                // Type validation
                match &param_schema.parameter_type {
                    ConfigType::String => {
                        if !val.is_string() {
                            errors.push(format!("{} must be a string", path));
                        }
                    }
                    ConfigType::Integer { min, max } => {
                        if let Some(n) = val.as_i64() {
                            if *min > 0 && n < *min {
                                errors.push(format!("{} must be >= {}", path, min));
                            }
                            if *max > 0 && n > *max {
                                errors.push(format!("{} must be <= {}", path, max));
                            }
                        } else {
                            errors.push(format!("{} must be an integer", path));
                        }
                    }
                    ConfigType::Float { min, max } => {
                        if let Some(f) = val.as_f64() {
                            if *min > 0.0 && f < *min {
                                errors.push(format!("{} must be >= {}", path, min));
                            }
                            if *max > 0.0 && f > *max {
                                errors.push(format!("{} must be <= {}", path, max));
                            }
                        } else {
                            errors.push(format!("{} must be a number", path));
                        }
                    }
                    ConfigType::Boolean => {
                        if !val.is_boolean() {
                            errors.push(format!("{} must be a boolean", path));
                        }
                    }
                    ConfigType::Enum { options } => {
                        if let Some(s) = val.as_str() {
                            if !options.contains(&s.to_string()) {
                                errors.push(format!("{} must be one of: {:?}", path, options));
                            }
                        } else {
                            errors.push(format!("{} must be a string", path));
                        }
                    }
                }

                // Run custom validation rules
                for rule in &param_schema.validation_rules {
                    if !self.run_validation_rule(val, rule) {
                        errors.push(rule.error_message.clone());
                    }
                }
            }
        }

        if errors.is_empty() {
            ValidationStatus::Valid
        } else {
            ValidationStatus::Invalid { errors }
        }
    }

    /// Run a validation rule on a value
    fn run_validation_rule(&self, value: &serde_json::Value, rule: &ValidationRule) -> bool {
        match rule.rule_type.as_str() {
            "range_warning" => {
                // This is a warning, not a hard failure
                return true;
            }
            "critical" => {
                if let Some(params) = rule.params.as_object() {
                    if let Some(forbidden) = params.get("forbidden") {
                        if let Some(n) = value.as_i64() {
                            return n != *forbidden;
                        }
                    }
                }
                true
            }
            _ => true
        }
    }

    /// Compare two configuration versions
    pub async fn diff_versions(&self, version_a_id: &str, version_b_id: &str) -> Result<ConfigDiff> {
        let versions = self.versions.read().await;
        
        let version_a = versions.get(version_a_id)
            .ok_or_else(|| anyhow::anyhow!("Version A not found: {}", version_a_id))?;
        let version_b = versions.get(version_b_id)
            .ok_or_else(|| anyhow::anyhow!("Version B not found: {}", version_b_id))?;

        let mut changes = Vec::new();
        let mut added = 0;
        let mut removed = 0;
        let mut modified = 0;
        let mut critical_changes = Vec::new();

        // Collect all keys from both configs
        let mut all_keys: std::collections::HashSet<String> = std::collections::HashSet::new();
        if let Some(obj) = version_a.config_data.as_object() {
            for key in obj.keys() {
                all_keys.insert(key.clone());
            }
        }
        if let Some(obj) = version_b.config_data.as_object() {
            for key in obj.keys() {
                all_keys.insert(key.clone());
            }
        }

        // Compare each key
        for key in all_keys {
            let old_val = version_a.config_data.get(&key);
            let new_val = version_b.config_data.get(&key);

            match (old_val, new_val) {
                (None, Some(n)) => {
                    changes.push(ConfigChange {
                        path: key.clone(),
                        old_value: serde_json::Value::Null,
                        new_value: n.clone(),
                        change_type: ChangeType::Added,
                    });
                    added += 1;
                }
                (Some(o), None) => {
                    changes.push(ConfigChange {
                        path: key.clone(),
                        old_value: o.clone(),
                        new_value: serde_json::Value::Null,
                        change_type: ChangeType::Removed,
                    });
                    removed += 1;
                }
                (Some(o), Some(n)) => {
                    if o != n {
                        changes.push(ConfigChange {
                            path: key.clone(),
                            old_value: o.clone(),
                            new_value: n.clone(),
                            change_type: ChangeType::Modified,
                        });
                        modified += 1;
                    }
                }
                (None, None) => {
                    // Shouldn't happen
                }
            }
        }

        // Identify critical changes
        let critical_params = ["pplns_ttl_days", "donation", "ignore_difficulty"]
            .map(|s| s.to_string());
        for change in &changes {
            if critical_params.contains(&change.path) {
                critical_changes.push(change.path.clone());
            }
        }

        let summary = ConfigDiffSummary {
            total_changes: changes.len(),
            added,
            removed,
            modified,
            critical_changes,
        };

        Ok(ConfigDiff {
            version_a: version_a_id.to_string(),
            version_b: version_b_id.to_string(),
            changes,
            summary,
        })
    }

    /// Rollback to a previous version
    pub async fn rollback(&self, version_id: &str, reason: String, performed_by: String) -> Result<()> {
        let version = self.get_version(version_id).await
            .ok_or_else(|| anyhow::anyhow!("Version not found: {}", version_id))?;

        info!("Rolling back to version {} (reason: {})", version_id, reason);

        // Create a new version for the rollback
        let new_version = self.create_version(
            version.config_data.clone(),
            format!("Rollback to {}", version_id),
            performed_by,
        ).await?;

        info!("Rollback completed as version {}", new_version.id);

        Ok(())
    }

    /// Schedule a configuration change
    pub async fn schedule_change(
        &self,
        config_data: serde_json::Value,
        description: String,
        scheduled_at: DateTime<Utc>,
        created_by: String,
    ) -> Result<String> {
        // Create the target version now
        let target_version = self.create_version(config_data, description.clone(), created_by.clone()).await?;
        let target_version_id = target_version.id.clone();

        let scheduled_change = ScheduledChange {
            id: uuid::Uuid::new_v4().to_string(),
            target_version_id,
            scheduled_at,
            status: ScheduleStatus::Pending,
            created_by,
        };

        let mut changes = self.scheduled_changes.write().await;
        changes.push(scheduled_change.clone());

        info!("Scheduled configuration change {} for application at {}",
            scheduled_change.id, scheduled_at);

        Ok(scheduled_change.id)
    }

    /// Process scheduled changes
    pub async fn process_scheduled_changes(&self) -> Result<usize> {
        let now = Utc::now();
        let mut applied = 0;

        // First, collect the IDs of changes that need to be applied
        let changes_to_apply: Vec<String> = {
            let changes = self.scheduled_changes.read().await;
            changes.iter()
                .filter(|change| change.scheduled_at <= now && change.status == ScheduleStatus::Pending)
                .map(|change| change.id.clone())
                .collect()
        };

        // Process each scheduled change
        for change_id in changes_to_apply {
            // Get the change details
            let (target_version_id, change_id_str) = {
                let changes = self.scheduled_changes.read().await;
                if let Some(change) = changes.iter().find(|c| c.id == change_id) {
                    (change.target_version_id.clone(), change.id.clone())
                } else {
                    continue;
                }
            };

            // Check if target version exists
            if self.get_version(&target_version_id).await.is_none() {
                warn!("Target version {} not found for scheduled change {}", target_version_id, change_id);
                continue;
            }

            // Apply the scheduled change
            match self.rollback(&target_version_id,
                format!("Scheduled change {}", change_id_str),
                "system".to_string()
            ).await {
                Ok(_) => {
                    info!("Applied scheduled change {}", change_id_str);
                    applied += 1;
                    // Mark as applied
                    let mut changes = self.scheduled_changes.write().await;
                    if let Some(change) = changes.iter_mut().find(|c| c.id == change_id) {
                        change.status = ScheduleStatus::Applied;
                    }
                }
                Err(e) => {
                    warn!("Failed to apply scheduled change {}: {}", change_id_str, e);
                    // Mark as failed
                    let mut changes = self.scheduled_changes.write().await;
                    if let Some(change) = changes.iter_mut().find(|c| c.id == change_id) {
                        change.status = ScheduleStatus::Failed { error: e.to_string() };
                    }
                }
            }
        }

        Ok(applied)
    }

    /// Export all versions as JSON
    pub async fn export_versions(&self, output_path: PathBuf) -> Result<()> {
        let versions = self.list_versions().await;

        let json = serde_json::to_string_pretty(&versions)
            .context("Failed to serialize versions")?;

        fs::write(&output_path, json).await
            .context("Failed to write export file")?;

        info!("Exported {} versions to {:?}", versions.len(), output_path);
        Ok(())
    }

    /// Get configuration schema
    pub async fn get_schema(&self) -> HashMap<String, ConfigSchema> {
        self.schema.read().await.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_config_creation() {
        let temp_dir = std::env::temp_dir();
        let storage_dir = temp_dir.join("dmpool_config_test");
        
        let manager = ConfigManager::new(storage_dir);
        manager.initialize().await.unwrap();

        let config = json!({
            "stratum.port": 3333,
            "stratum.start_difficulty": 32,
            "donation": 0,
            "pplns_ttl_days": 7
        });

        let version = manager.create_version(
            config,
            "Test configuration".to_string(),
            "test_user".to_string()
        ).await.unwrap();

        assert_eq!(version.validation_status, ValidationStatus::Valid);
        assert!(manager.list_versions().await.len() > 0);
    }

    #[tokio::test]
    async fn test_config_validation() {
        let temp_dir = std::env::temp_dir();
        let storage_dir = temp_dir.join("dmpool_config_test");
        
        let manager = ConfigManager::new(storage_dir);
        manager.initialize().await.unwrap();

        // Invalid config
        let invalid_config = json!({
            "pplns_ttl_days": 0
        });

        let status = manager.validate_config(&invalid_config).await;
        assert!(matches!(status, ValidationStatus::Invalid { .. }));
    }
}
