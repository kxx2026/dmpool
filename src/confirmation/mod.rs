// Configuration Confirmation Module for DMPool Admin
// Ensures dangerous config changes require explicit confirmation

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

/// Configuration change that requires confirmation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConfigChangeRequest {
    /// Unique ID for this change request
    pub id: String,
    /// Parameter being changed
    pub parameter: String,
    /// Old value
    pub old_value: serde_json::Value,
    /// New value
    pub new_value: serde_json::Value,
    /// User requesting the change
    pub username: String,
    /// IP address of the user
    pub ip_address: String,
    /// Timestamp when the request was created
    pub created_at: DateTime<Utc>,
    /// Expiration time (10 minutes)
    pub expires_at: DateTime<Utc>,
    /// Whether this change has been confirmed
    pub confirmed: bool,
    /// Whether this change has been applied
    pub applied: bool,
}

/// Risk level for configuration changes
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    /// Safe - no confirmation needed
    Safe,
    /// Low - minimal risk
    Low,
    /// Medium - single confirmation required
    Medium,
    /// High - double confirmation required
    High,
    /// Critical - explicit approval required
    Critical,
}

/// Configuration change metadata
#[derive(Clone, Serialize)]
pub struct ConfigMeta {
    /// Risk level
    pub risk_level: RiskLevel,
    /// Description of the risk
    pub risk_description: String,
    /// Recommended value (if applicable)
    pub recommended_value: Option<String>,
}

/// Configuration confirmation manager
pub struct ConfigConfirmation {
    /// Pending change requests
    pending: Arc<RwLock<HashMap<String, ConfigChangeRequest>>>,
    /// Configuration metadata for each parameter
    config_meta: HashMap<String, ConfigMeta>,
    /// Confirmation timeout in seconds
    confirmation_timeout: i64,
}

impl ConfigConfirmation {
    /// Create a new confirmation manager
    pub fn new() -> Self {
        let mut config_meta = HashMap::new();

        // Define risk levels for each configuration parameter
        config_meta.insert("pplns_ttl_days".to_string(), ConfigMeta {
            risk_level: RiskLevel::Critical,
            risk_description: "TTL < 7天会导致矿工损失收益，TTL = 0会导致矿池无法支付".to_string(),
            recommended_value: Some("7".to_string()),
        });

        config_meta.insert("donation".to_string(), ConfigMeta {
            risk_level: RiskLevel::Critical,
            risk_description: "donation = 10000 会导致矿工收益为0（100%捐赠）".to_string(),
            recommended_value: Some("0".to_string()),
        });

        config_meta.insert("ignore_difficulty".to_string(), ConfigMeta {
            risk_level: RiskLevel::Critical,
            risk_description: "禁用难度验证会导致不公平的PPLNS分配，可能被攻击".to_string(),
            recommended_value: Some("false".to_string()),
        });

        config_meta.insert("start_difficulty".to_string(), ConfigMeta {
            risk_level: RiskLevel::Medium,
            risk_description: "过高会导致矿工连接困难，过低会增加服务器负载".to_string(),
            recommended_value: Some("32".to_string()),
        });

        config_meta.insert("minimum_difficulty".to_string(), ConfigMeta {
            risk_level: RiskLevel::Medium,
            risk_description: "过低会导致低算力矿工占便宜，过高会排除小矿工".to_string(),
            recommended_value: Some("16".to_string()),
        });

        config_meta.insert("pool_signature".to_string(), ConfigMeta {
            risk_level: RiskLevel::Low,
            risk_description: "更改pool签名会影响支付识别".to_string(),
            recommended_value: None,
        });

        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            config_meta,
            confirmation_timeout: 600, // 10 minutes
        }
    }

    /// Check if a config change requires confirmation
    pub fn requires_confirmation(&self, parameter: &str) -> bool {
        match self.config_meta.get(parameter) {
            Some(meta) => meta.risk_level != RiskLevel::Safe && meta.risk_level != RiskLevel::Low,
            None => true, // Unknown parameters require confirmation
        }
    }

    /// Get the risk level for a parameter
    pub fn get_risk_level(&self, parameter: &str) -> RiskLevel {
        self.config_meta
            .get(parameter)
            .map(|m| m.risk_level)
            .unwrap_or(RiskLevel::Medium)
    }

    /// Create a change request for a configuration parameter
    pub async fn create_change_request(
        &self,
        parameter: String,
        old_value: serde_json::Value,
        new_value: serde_json::Value,
        username: String,
        ip_address: String,
    ) -> Result<ConfigChangeRequest> {
        let id = uuid::Uuid::new_v4().to_string();
        let created_at = Utc::now();
        let expires_at = created_at + chrono::Duration::seconds(self.confirmation_timeout);

        let log_value = new_value.clone();
        let request = ConfigChangeRequest {
            id: id.clone(),
            parameter: parameter.clone(),
            old_value,
            new_value,
            username,
            ip_address,
            created_at,
            expires_at,
            confirmed: false,
            applied: false,
        };

        // Store the pending request
        let mut pending = self.pending.write().await;
        pending.insert(id.clone(), request.clone());

        info!(
            "Created config change request: {} = {:?} (waiting confirmation)",
            parameter, log_value
        );

        Ok(request)
    }

    /// Confirm a pending change request
    pub async fn confirm_change(&self, id: &str) -> Result<bool> {
        let mut pending = self.pending.write().await;

        match pending.get_mut(id) {
            Some(request) => {
                // Check if expired
                if Utc::now() > request.expires_at {
                    pending.remove(id);
                    return Ok(false);
                }

                request.confirmed = true;
                info!(
                    "Config change confirmed: {} = {:?}",
                    request.parameter, request.new_value
                );
                Ok(true)
            }
            None => Err(anyhow::anyhow!("Change request not found or expired")),
        }
    }

    /// Apply a confirmed change request
    pub async fn apply_change(&self, id: &str) -> Result<ConfigChangeRequest> {
        let mut pending = self.pending.write().await;

        match pending.get(id) {
            Some(request) => {
                // Check if confirmed
                if !request.confirmed {
                    return Err(anyhow::anyhow!("Change not confirmed"));
                }

                // Check if expired
                if Utc::now() > request.expires_at {
                    pending.remove(id);
                    return Err(anyhow::anyhow!("Change request expired"));
                }

                // Mark as applied
                let mut request = request.clone();
                request.applied = true;
                pending.insert(id.to_string(), request.clone());

                // Remove from pending after applying
                pending.remove(id);

                info!(
                    "Config change applied: {} = {:?}",
                    request.parameter, request.new_value
                );

                Ok(request)
            }
            None => Err(anyhow::anyhow!("Change request not found or expired")),
        }
    }

    /// Cancel a pending change request
    pub async fn cancel_change(&self, id: &str) -> Result<bool> {
        let mut pending = self.pending.write().await;
        Ok(pending.remove(id).is_some())
    }

    /// Get all pending change requests
    pub async fn get_pending(&self) -> Vec<ConfigChangeRequest> {
        let pending = self.pending.read().await;
        let mut result: Vec<ConfigChangeRequest> = pending.values().cloned().collect();

        // Filter out expired requests
        let now = Utc::now();
        result.retain(|r| r.expires_at > now);

        result
    }

    /// Get a specific change request
    pub async fn get_request(&self, id: &str) -> Option<ConfigChangeRequest> {
        let pending = self.pending.read().await;
        pending.get(id).cloned()
    }

    /// Clean up expired change requests
    pub async fn cleanup_expired(&self) -> usize {
        let mut pending = self.pending.write().await;
        let now = Utc::now();
        let original_len = pending.len();
        pending.retain(|_, r| r.expires_at > now);
        original_len - pending.len()
    }

    /// Get configuration metadata for a parameter
    pub fn get_config_meta(&self, parameter: &str) -> Option<&ConfigMeta> {
        self.config_meta.get(parameter)
    }

    /// Validate a new configuration value
    pub fn validate_value(&self, parameter: &str, value: &serde_json::Value) -> Result<(), String> {
        match parameter {
            "pplns_ttl_days" => {
                if let Some(days) = value.as_i64() {
                    if days < 1 {
                        return Err("TTL不能小于1天".to_string());
                    }
                    if days < 7 {
                        warn!("TTL={}天低于标准的7天，矿工可能损失收益", days);
                    }
                } else {
                    return Err("TTL必须是整数".to_string());
                }
            }
            "donation" => {
                if let Some(donation) = value.as_i64() {
                    if donation < 0 || donation > 10000 {
                        return Err("donation必须在0-10000之间".to_string());
                    }
                    if donation == 10000 {
                        return Err("donation=10000意味着100%捐赠，矿工收益为0！".to_string());
                    }
                    if donation > 500 {
                        warn!("donation={}较高，相当于{}%捐赠", donation, donation / 100);
                    }
                }
            }
            "ignore_difficulty" => {
                if let Some(ignore) = value.as_bool() {
                    if ignore {
                        return Err("禁用难度验证非常危险！可能导致不公平的PPLNS分配".to_string());
                    }
                }
            }
            "start_difficulty" | "minimum_difficulty" => {
                if let Some(diff) = value.as_i64() {
                    if diff < 8 || diff > 512 {
                        return Err("难度必须在8-512之间".to_string());
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Default for ConfigConfirmation {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_risk_levels() {
        let conf = ConfigConfirmation::new();

        assert_eq!(
            conf.get_risk_level("pplns_ttl_days"),
            RiskLevel::Critical
        );
        assert_eq!(conf.get_risk_level("donation"), RiskLevel::Critical);
        assert_eq!(
            conf.get_risk_level("ignore_difficulty"),
            RiskLevel::Critical
        );
    }

    #[test]
    fn test_validation() {
        let conf = ConfigConfirmation::new();

        // Test pplns_ttl_days validation
        assert!(conf.validate_value("pplns_ttl_days", &json!(7)).is_ok());
        assert!(conf
            .validate_value("pplns_ttl_days", &json!(0))
            .is_err());

        // Test donation validation
        assert!(conf.validate_value("donation", &json!(0)).is_ok());
        assert!(conf.validate_value("donation", &json!(10000)).is_err());

        // Test ignore_difficulty validation
        assert!(conf
            .validate_value("ignore_difficulty", &json!(true))
            .is_err());
        assert!(conf
            .validate_value("ignore_difficulty", &json!(false))
            .is_ok());
    }

    #[tokio::test]
    async fn test_change_request_flow() {
        let conf = ConfigConfirmation::new();

        // Create a change request
        let request = conf
            .create_change_request(
                "pplns_ttl_days".to_string(),
                json!(7),
                json!(14),
                "admin".to_string(),
                "127.0.0.1".to_string(),
            )
            .await
            .unwrap();

        assert!(!request.confirmed);
        assert!(!request.applied);

        // Confirm the change
        assert!(conf.confirm_change(&request.id).await.unwrap());

        // Get the request
        let confirmed = conf.get_request(&request.id).await.unwrap();
        assert!(confirmed.confirmed);

        // Apply the change
        let applied = conf.apply_change(&request.id).await.unwrap();
        assert!(applied.applied);

        // Request should be removed after application
        assert!(conf.get_request(&request.id).await.is_none());
    }
}
