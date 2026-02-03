// Alert System for DMPool
// Supports multiple alert channels (Email, Telegram, Webhook)
// with configurable rules and alert aggregation

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Alert severity levels
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AlertLevel {
    /// Informational - no action required
    Info,
    /// Warning - attention needed
    Warning,
    /// Critical - immediate action required
    Critical,
}

impl AlertLevel {
    /// Returns the numeric severity for comparison
    pub fn severity(&self) -> u8 {
        match self {
            Self::Info => 1,
            Self::Warning => 2,
            Self::Critical => 3,
        }
    }
}

impl std::fmt::Display for AlertLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Info => write!(f, "INFO"),
            Self::Warning => write!(f, "WARNING"),
            Self::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Alert channel types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertChannel {
    Email {
        smtp_server: String,
        smtp_port: u16,
        username: String,
        password: String,
        from_address: String,
        to_addresses: Vec<String>,
    },
    Telegram {
        bot_token: String,
        chat_id: String,
    },
    Webhook {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
}

/// Alert condition types
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlertCondition {
    /// Hashrate below threshold (TH/s)
    HashrateBelow { threshold: f64, duration_minutes: u64 },
    /// Hashrate above threshold (TH/s)
    HashrateAbove { threshold: f64, duration_minutes: u64 },
    /// Block not found within duration
    NoBlock { duration_minutes: u64 },
    /// Worker count below threshold
    WorkerCountBelow { threshold: u64 },
    /// Database error
    DatabaseError,
    /// API error
    ApiError,
    /// Custom message
    Custom { message: String },
}

/// Alert rule definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlertRule {
    /// Unique rule identifier
    pub id: String,
    /// Rule name
    pub name: String,
    /// Description
    pub description: String,
    /// Alert condition
    pub condition: AlertCondition,
    /// Alert level
    pub level: AlertLevel,
    /// Whether the rule is enabled
    pub enabled: bool,
    /// Channels to send alerts to
    pub channels: Vec<String>,
    /// Cooldown period between alerts (minutes)
    pub cooldown_minutes: u64,
    /// Last time this rule was triggered
    #[serde(skip)]
    last_triggered: Option<DateTime<Utc>>,
}

/// Alert notification
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alert {
    /// Unique alert ID
    pub id: String,
    /// Rule that triggered this alert
    pub rule_id: String,
    /// Alert level
    pub level: AlertLevel,
    /// Alert title
    pub title: String,
    /// Alert message
    pub message: String,
    /// Additional context (JSON)
    pub context: serde_json::Value,
    /// Timestamp when alert was triggered
    pub triggered_at: DateTime<Utc>,
    /// Whether alert has been acknowledged
    pub acknowledged: bool,
    /// Channel that was used
    pub channel: String,
}

/// Alert statistics
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlertStats {
    pub total_alerts: usize,
    pub active_alerts: usize,
    pub acknowledged_alerts: usize,
    pub alerts_by_level: HashMap<String, usize>,
    pub alerts_by_rule: HashMap<String, usize>,
}

/// Alert manager configuration
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AlertConfig {
    /// Global enable/disable
    pub enabled: bool,
    /// Alert channels (key: channel name)
    pub channels: HashMap<String, AlertChannel>,
    /// Alert rules
    pub rules: Vec<AlertRule>,
    /// Maximum history size
    pub max_history: usize,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            channels: HashMap::new(),
            rules: Vec::new(),
            max_history: 1000,
        }
    }
}

/// Alert manager
pub struct AlertManager {
    config: Arc<RwLock<AlertConfig>>,
    history: Arc<RwLock<Vec<Alert>>>,
}

impl AlertManager {
    /// Create a new alert manager
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config: Arc::new(RwLock::new(config)),
            history: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Create with default configuration
    pub fn default() -> Self {
        Self::new(AlertConfig::default())
    }

    /// Add an alert channel
    pub async fn add_channel(&self, name: String, channel: AlertChannel) {
        let mut config = self.config.write().await;
        config.channels.insert(name.clone(), channel);
        info!("Added alert channel: {}", name);
    }

    /// Remove an alert channel
    pub async fn remove_channel(&self, name: &str) -> bool {
        let mut config = self.config.write().await;
        config.channels.remove(name).is_some()
    }

    /// Add an alert rule
    pub async fn add_rule(&self, rule: AlertRule) {
        let name = rule.name.clone();
        let mut config = self.config.write().await;
        config.rules.push(rule);
        info!("Added alert rule: {}", name);
    }

    /// Remove an alert rule
    pub async fn remove_rule(&self, rule_id: &str) -> bool {
        let mut config = self.config.write().await;
        if let Some(pos) = config.rules.iter().position(|r| r.id == rule_id) {
            config.rules.remove(pos);
            info!("Removed alert rule: {}", rule_id);
            return true;
        }
        false
    }

    /// Trigger an alert by rule ID
    pub async fn trigger_alert(
        &self,
        rule_id: &str,
        context: serde_json::Value,
    ) -> Result<()> {
        let config = self.config.read().await;

        if !config.enabled {
            return Ok(());
        }

        let rule = config.rules.iter()
            .find(|r| r.id == rule_id)
            .ok_or_else(|| anyhow::anyhow!("Rule not found: {}", rule_id))?;

        if !rule.enabled {
            return Ok(());
        }

        // Check cooldown
        if let Some(last_triggered) = rule.last_triggered {
            let elapsed = Utc::now().signed_duration_since(last_triggered).num_minutes();
            if elapsed < rule.cooldown_minutes as i64 {
                return Ok(()); // Still in cooldown
            }
        }

        // Clone values we need after dropping config
        let rule_name = rule.name.clone();
        let rule_level = rule.level;
        let rule_id_clone = rule.id.clone();

        let alert = Alert {
            id: uuid::Uuid::new_v4().to_string(),
            rule_id: rule.id.clone(),
            level: rule.level,
            title: format!("{} Alert: {}", rule.level, rule.name),
            message: self.format_message(&rule.condition, &context)?,
            context,
            triggered_at: Utc::now(),
            acknowledged: false,
            channel: rule.channels.first().cloned().unwrap_or_default(),
        };

        // Send to channels
        for channel_name in &rule.channels {
            if let Some(channel) = config.channels.get(channel_name) {
                if let Err(e) = self.send_alert(channel, &alert).await {
                    error!("Failed to send alert via {}: {}", channel_name, e);
                }
            }
        }

        // Add to history
        let mut history = self.history.write().await;
        history.push(alert.clone());

        // Trim history if needed
        if history.len() > config.max_history {
            let remove_count = history.len() - config.max_history;
            history.drain(0..remove_count);
        }

        // Update last triggered time (requires write access to config)
        drop(config);
        drop(history);
        let mut config = self.config.write().await;
        if let Some(rule) = config.rules.iter_mut().find(|r| r.id == rule_id_clone) {
            rule.last_triggered = Some(Utc::now());
        }

        info!("Alert triggered: {} ({})", rule_name, rule_level);
        Ok(())
    }

    /// Format alert message based on condition
    fn format_message(&self, condition: &AlertCondition, _context: &serde_json::Value) -> Result<String> {
        Ok(match condition {
            AlertCondition::HashrateBelow { threshold, .. } => {
                format!("Pool hashrate has dropped below {} TH/s", threshold)
            }
            AlertCondition::HashrateAbove { threshold, .. } => {
                format!("Pool hashrate has exceeded {} TH/s", threshold)
            }
            AlertCondition::NoBlock { duration_minutes } => {
                format!("No block found in the last {} minutes", duration_minutes)
            }
            AlertCondition::WorkerCountBelow { threshold } => {
                format!("Worker count has dropped below {}", threshold)
            }
            AlertCondition::DatabaseError => {
                "Database error detected".to_string()
            }
            AlertCondition::ApiError => {
                "API error detected".to_string()
            }
            AlertCondition::Custom { message } => {
                message.clone()
            }
        })
    }

    /// Send alert via a specific channel
    async fn send_alert(&self, channel: &AlertChannel, alert: &Alert) -> Result<()> {
        match channel {
            AlertChannel::Email { .. } => {
                // TODO: Implement email sending
                warn!("Email alert not yet implemented: {}", alert.title);
                Ok(())
            }
            AlertChannel::Telegram { bot_token, chat_id } => {
                self.send_telegram_alert(bot_token, chat_id, alert).await
            }
            AlertChannel::Webhook { url, headers } => {
                self.send_webhook_alert(url, headers, alert).await
            }
        }
    }

    /// Send Telegram alert
    async fn send_telegram_alert(&self, bot_token: &str, chat_id: &str, alert: &Alert) -> Result<()> {
        let message = format!(
            "*{}* {}\n\n{}\n\n{}",
            alert.level,
            alert.title,
            alert.message,
            alert.triggered_at.format("%Y-%m-%d %H:%M:%S UTC")
        );

        let url = format!("https://api.telegram.org/bot{}/sendMessage", bot_token);
        let client = reqwest::Client::new();
        
        let response = client
            .post(&url)
            .json(&serde_json::json!({
                "chat_id": chat_id,
                "text": message,
                "parse_mode": "Markdown"
            }))
            .send()
            .await
            .context("Failed to send Telegram alert")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Telegram API error: {}", response.status()));
        }

        Ok(())
    }

    /// Send webhook alert
    async fn send_webhook_alert(
        &self,
        url: &str,
        headers: &Option<HashMap<String, String>>,
        alert: &Alert,
    ) -> Result<()> {
        let client = reqwest::Client::new();
        let mut request = client.post(url).json(alert);

        if let Some(hdrs) = headers {
            for (key, value) in hdrs {
                request = request.header(key, value);
            }
        }

        let response = request
            .send()
            .await
            .context("Failed to send webhook alert")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("Webhook error: {}", response.status()));
        }

        Ok(())
    }

    /// Get alert history
    pub async fn get_history(&self, limit: Option<usize>) -> Vec<Alert> {
        let history = self.history.read().await;
        let mut result = history.clone();
        
        // Reverse to show newest first
        result.reverse();
        
        if let Some(limit) = limit {
            result.truncate(limit);
        }
        
        result
    }

    /// Acknowledge an alert
    pub async fn acknowledge_alert(&self, alert_id: &str) -> Result<bool> {
        let mut history = self.history.write().await;
        if let Some(alert) = history.iter_mut().find(|a| a.id == alert_id) {
            alert.acknowledged = true;
            info!("Alert acknowledged: {}", alert_id);
            return Ok(true);
        }
        Ok(false)
    }

    /// Get alert statistics
    pub async fn get_stats(&self) -> AlertStats {
        let history = self.history.read().await;
        let config = self.config.read().await;

        let mut alerts_by_level = HashMap::new();
        let mut alerts_by_rule = HashMap::new();

        for alert in history.iter() {
            *alerts_by_level.entry(alert.level.to_string()).or_insert(0) += 1;
            *alerts_by_rule.entry(alert.rule_id.clone()).or_insert(0) += 1;
        }

        AlertStats {
            total_alerts: history.len(),
            active_alerts: history.iter().filter(|a| !a.acknowledged).count(),
            acknowledged_alerts: history.iter().filter(|a| a.acknowledged).count(),
            alerts_by_level,
            alerts_by_rule,
        }
    }

    /// Get all rules
    pub async fn get_rules(&self) -> Vec<AlertRule> {
        let config = self.config.read().await;
        config.rules.clone()
    }

    /// Get all channels
    pub async fn get_channels(&self) -> HashMap<String, AlertChannel> {
        let config = self.config.read().await;
        config.channels.clone()
    }

    /// Clear old history
    pub async fn cleanup_old_history(&self, keep_last: usize) -> usize {
        let mut history = self.history.write().await;
        let original_len = history.len();

        if history.len() > keep_last {
            let drain_count = history.len() - keep_last;
            history.drain(0..drain_count);
        }

        original_len - history.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_alert_level_severity() {
        assert_eq!(AlertLevel::Info.severity(), 1);
        assert_eq!(AlertLevel::Warning.severity(), 2);
        assert_eq!(AlertLevel::Critical.severity(), 3);
    }

    #[test]
    fn test_alert_level_display() {
        assert_eq!(AlertLevel::Info.to_string(), "INFO");
        assert_eq!(AlertLevel::Warning.to_string(), "WARNING");
        assert_eq!(AlertLevel::Critical.to_string(), "CRITICAL");
    }
}
