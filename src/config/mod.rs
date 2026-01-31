// Configuration validation module for DMPool

use p2poolv2_lib::config::Config;
use anyhow::Result;

/// Configuration validation result
#[derive(Debug, Clone)]
pub struct ValidationResult {
    pub is_valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl ValidationResult {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: vec![],
            warnings: vec![],
        }
    }
    
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
            warnings: vec![],
        }
    }
    
    pub fn with_warning(mut self, warning: String) -> Self {
        self.warnings.push(warning);
        self
    }
    
    pub fn extend_errors(&mut self, mut errors: Vec<String>) {
        self.errors.append(&mut errors);
        self.is_valid = false;
    }
}

/// Validate configuration
pub fn validate_config(config: &Config) -> ValidationResult {
    let mut result = ValidationResult::valid();
    
    let stratum_errors = validate_stratum_config(config);
    if !stratum_errors.is_empty() {
        result.extend_errors(stratum_errors);
    }
    
    let api_errors = validate_api_config(config);
    if !api_errors.is_empty() {
        result.extend_errors(api_errors);
    }
    
    let store_warnings = validate_store_config(config);
    for warning in store_warnings {
        result = result.with_warning(warning);
    }
    
    result
}

/// Validate stratum section
fn validate_stratum_config(config: &Config) -> Vec<String> {
    let mut errors = vec![];
    
    if config.stratum.port < 1024 || config.stratum.port > 65535 {
        errors.push(format!(
            "Stratum port {} is out of valid range (1024-65535)",
            config.stratum.port
        ));
    }
    
    if !is_valid_hostname(&config.stratum.hostname) {
        errors.push(format!(
            "Invalid hostname '{}'",
            config.stratum.hostname
        ));
    }
    
    let network_str = config.stratum.network.to_string();
    if !matches!(network_str.as_str(), "main" | "signet" | "testnet4") {
        errors.push(format!(
            "Invalid network '{}'. Must be: main, signet, or testnet4",
            network_str
        ));
    }
    
    if config.stratum.pool_signature.as_ref().map_or(0, |s| s.len()) > 16 {
        errors.push(format!(
            "Pool signature too long: {} bytes (max 16)",
            config.stratum.pool_signature.as_ref().map_or(0, |s| s.len())
        ));
    }
    
    errors
}

/// Validate API section
fn validate_api_config(config: &Config) -> Vec<String> {
    let mut errors = vec![];
    
    if config.api.port < 1024 || config.api.port > 65535 {
        errors.push(format!(
            "API port {} is out of valid range (1024-65535)",
            config.api.port
        ));
    }
    
    if config.api.hostname != "127.0.0.1" && config.api.hostname != "0.0.0.0" {
        errors.push(format!(
            "API hostname '{}' should be 127.0.0.1 or 0.0.0.0 for security",
            config.api.hostname
        ));
    }
    
    let has_user = config.api.auth_user.is_some() || !config.api.auth_user.as_ref().map_or(false, |u| u.is_empty());
    let has_token = config.api.auth_token.is_some() || !config.api.auth_token.as_ref().map_or(false, |t| t.is_empty());
    
    if has_user != has_token {
        errors.push("Both auth_user and auth_token must be set together".to_string());
    }
    
    errors
}

/// Validate store section
fn validate_store_config(config: &Config) -> Vec<String> {
    let mut warnings = vec![];
    
    if !std::path::Path::new(&config.store.path).exists() {
        warnings.push(format!(
            "Database path '{}' does not exist, will be created on startup",
            config.store.path
        ));
    }
    
    if config.store.pplns_ttl_days < 1 {
        warnings.push("PPLNS TTL days is less than 1, shares may expire too quickly".to_string());
    }
    
    warnings
}

fn is_valid_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }
    
    let allowed_chars = hostname
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '.');
    
    if !allowed_chars {
        return false;
    }
    
    !hostname.starts_with('-') 
        && !hostname.starts_with('.')
        && !hostname.ends_with('-')
        && !hostname.ends_with('.')
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_validation_result() {
        let result = ValidationResult::valid();
        assert!(result.is_valid);
    }
    
    #[test]
    fn test_hostname_validation() {
        assert!(is_valid_hostname("127.0.0.1"));
        assert!(is_valid_hostname("mining-pool.example.com"));
        assert!(!is_valid_hostname("-invalid.com"));
    }
}
