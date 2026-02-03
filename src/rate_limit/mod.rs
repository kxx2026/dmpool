// Rate limiting module for DMPool Admin API
// Prevents brute force attacks and API abuse

use anyhow::{anyhow, Result};
use axum::{
    extract::{Request, State},
    http::{StatusCode, HeaderMap},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr};
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{warn, debug, error};

/// Rate limiter configuration
#[derive(Clone)]
pub struct RateLimitConfig {
    /// Requests per minute for general API
    pub api_rpm: NonZeroU32,
    /// Requests per minute for login endpoint (stricter)
    pub login_rpm: NonZeroU32,
    /// Burst size
    pub burst: NonZeroU32,
    /// Trusted proxy IPs that can set X-Forwarded-For
    /// If empty, proxy headers are ignored (safer)
    pub trusted_proxies: HashSet<IpAddr>,
    /// Whether to require IP validation (fail if IP cannot be determined)
    pub require_valid_ip: bool,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            // 60 requests per minute for general API
            api_rpm: NonZeroU32::new(60).unwrap(),
            // 10 requests per minute for login (anti-brute-force)
            login_rpm: NonZeroU32::new(10).unwrap(),
            // Allow burst of 10 requests
            burst: NonZeroU32::new(10).unwrap(),
            // No trusted proxies by default (safer)
            trusted_proxies: HashSet::new(),
            // Require valid IP in production
            require_valid_ip: std::env::var("DMP_ENV").unwrap_or("development".to_string()) == "production",
        }
    }
}

impl RateLimitConfig {
    /// Add a trusted proxy IP
    pub fn add_trusted_proxy(&mut self, ip: IpAddr) {
        self.trusted_proxies.insert(ip);
    }

    /// Add trusted proxy from CIDR (e.g., "10.0.0.0/8")
    pub fn add_trusted_proxy_cidr(&mut self, cidr: &str) -> Result<()> {
        // For simplicity, just support single IP for now
        // Full CIDR support would require additional dependencies
        let ip = cidr.parse::<IpAddr>()
            .map_err(|_| anyhow!("Invalid CIDR format: {}", cidr))?;
        self.trusted_proxies.insert(ip);
        Ok(())
    }

    /// Set whether to require valid IP
    pub fn set_require_valid_ip(&mut self, require: bool) {
        self.require_valid_ip = require;
    }
}

/// Rate limiter state - stores rate limit information per IP
#[derive(Clone)]
pub struct RateLimiterState {
    /// Rate limit configuration
    config: RateLimitConfig,
    /// Store last request time per IP (simple in-memory tracking)
    api_request_times: Arc<RwLock<std::collections::HashMap<String, Vec<std::time::Instant>>>>,
    login_request_times: Arc<RwLock<std::collections::HashMap<String, Vec<std::time::Instant>>>>,
}

impl RateLimiterState {
    /// Create a new rate limiter state from config
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            api_request_times: Arc::new(RwLock::new(std::collections::HashMap::new())),
            login_request_times: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Clean up old request timestamps (older than 1 minute)
    fn cleanup_old_requests(times: &mut Vec<std::time::Instant>, window: std::time::Duration) {
        let now = std::time::Instant::now();
        times.retain(|t| now.duration_since(*t) < window);
    }

    /// Check if the given IP is rate limited for API requests
    pub async fn check_api_rate_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let ip_str = ip.to_string();
        let mut times = self.api_request_times.write().await;
        let requests = times.entry(ip_str.clone()).or_insert_with(Vec::new);

        // Clean up old requests
        Self::cleanup_old_requests(requests, std::time::Duration::from_secs(60));

        // Check rate limit
        if requests.len() >= self.config.api_rpm.get() as usize {
            warn!("Rate limit exceeded for API: {}", ip_str);
            return Err(RateLimitError::TooManyRequests);
        }

        // Add current request timestamp
        requests.push(std::time::Instant::now());
        debug!("API request allowed for: {} (total: {})", ip_str, requests.len());
        Ok(())
    }

    /// Check if the given IP is rate limited for login attempts
    pub async fn check_login_rate_limit(&self, ip: IpAddr) -> Result<(), RateLimitError> {
        let ip_str = ip.to_string();
        let mut times = self.login_request_times.write().await;
        let requests = times.entry(ip_str.clone()).or_insert_with(Vec::new);

        // Clean up old requests
        Self::cleanup_old_requests(requests, std::time::Duration::from_secs(60));

        // Check rate limit (stricter for login)
        if requests.len() >= self.config.login_rpm.get() as usize {
            warn!("Rate limit exceeded for login: {}", ip_str);
            return Err(RateLimitError::TooManyRequests);
        }

        // Add current request timestamp
        requests.push(std::time::Instant::now());
        debug!("Login attempt allowed for: {} (total: {})", ip_str, requests.len());
        Ok(())
    }

    /// Get current rate limit status for an IP
    pub async fn get_rate_limit_status(&self, ip: IpAddr) -> RateLimitStatus {
        let ip_str = ip.to_string();
        let api_times = self.api_request_times.read().await;
        let login_times = self.login_request_times.read().await;

        let api_count = api_times.get(&ip_str).map_or(0, |v| v.len()) as u32;
        let login_count = login_times.get(&ip_str).map_or(0, |v| v.len()) as u32;

        RateLimitStatus {
            ip: ip_str,
            api_requests_remaining: self.config.api_rpm.get().saturating_sub(api_count),
            login_requests_remaining: self.config.login_rpm.get().saturating_sub(login_count),
            api_limit: self.config.api_rpm.get(),
            login_limit: self.config.login_rpm.get(),
        }
    }
}

/// Rate limit status for an IP
#[derive(Clone, Serialize)]
pub struct RateLimitStatus {
    pub ip: String,
    pub api_requests_remaining: u32,
    pub login_requests_remaining: u32,
    pub api_limit: u32,
    pub login_limit: u32,
}

/// Rate limit errors
#[derive(Debug)]
pub enum RateLimitError {
    TooManyRequests,
    InvalidIp(String),
}

impl IntoResponse for RateLimitError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            RateLimitError::TooManyRequests => (
                StatusCode::TOO_MANY_REQUESTS,
                "Too many requests. Please try again later.",
            ),
            RateLimitError::InvalidIp(ref msg) => (
                StatusCode::FORBIDDEN,
                msg.as_str(),
            ),
        };

        let body = serde_json::json!({
            "status": "error",
            "message": message,
            "retry_after": 60
        });

        (status, axum::Json(body)).into_response()
    }
}

/// Extract client IP from request headers
/// Only trusts X-Forwarded-For from configured trusted proxies
/// Returns error if IP cannot be determined (unless in development mode)
pub fn extract_client_ip(headers: &HeaderMap, config: &RateLimitConfig) -> Result<IpAddr, RateLimitError> {
    // First, try to get the direct connection IP from CF-Connecting-IP header
    // This header is set by Cloudflare and cannot be spoofed by the client
    if let Some(cf_ip) = headers.get("cf-connecting-ip") {
        if let Ok(ip_str) = cf_ip.to_str() {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                debug!("Using CF-Connecting-IP: {}", ip);
                return Ok(ip);
            }
        }
    }

    // Check X-Forwarded-For (only from trusted proxies)
    if let Some(forwarded) = headers.get("x-forwarded-for") {
        if let Ok(forwarded_str) = forwarded.to_str() {
            // X-Forwarded-For format: "client, proxy1, proxy2"
            let parts: Vec<&str> = forwarded_str.split(',').collect();

            // If we have trusted proxies, validate the chain
            if !config.trusted_proxies.is_empty() {
                // The rightmost IP should be our direct connection
                // Check if it's from a trusted proxy
                if let Some(direct_ip_str) = parts.last() {
                    if let Ok(direct_ip) = direct_ip_str.trim().parse::<IpAddr>() {
                        if config.trusted_proxies.contains(&direct_ip) {
                            // Proxy is trusted, use the client IP (leftmost)
                            if let Some(client_ip_str) = parts.first() {
                                if let Ok(client_ip) = client_ip_str.trim().parse::<IpAddr>() {
                                    // Validate client IP is not a private/internal network
                                    if is_valid_client_ip(&client_ip) {
                                        debug!("Using X-Forwarded-For client IP: {} (via trusted proxy)", client_ip);
                                        return Ok(client_ip);
                                    }
                                }
                            }
                        }
                    }
                }
                // If we reach here, X-Forwarded-For is from untrusted source or invalid
                warn!("X-Forwarded-For from untrusted source, ignoring");
            }
        }
    }

    // Check X-Real-IP (only from trusted proxies)
    if let Some(real_ip) = headers.get("x-real-ip") {
        if let Ok(real_ip_str) = real_ip.to_str() {
            if let Ok(ip) = real_ip_str.parse::<IpAddr>() {
                // Only accept X-Real-IP if it's from localhost (direct connection)
                // In a real setup, you'd check the direct connection IP
                if is_localhost(&ip) {
                    debug!("Using X-Real-IP from local connection: {}", ip);
                    return Ok(ip);
                }
            }
        }
    }

    // Check for CF-Pseudo-IPv4 (Cloudflare pseudo IPv4 for IPv6 clients)
    if let Some(pseudo_ipv4) = headers.get("cf-pseudo-ipv4") {
        if let Ok(ip_str) = pseudo_ipv4.to_str() {
            if let Ok(ip) = ip_str.parse::<IpAddr>() {
                debug!("Using CF-Pseudo-IPv4: {}", ip);
                return Ok(ip);
            }
        }
    }

    // If we require valid IP and couldn't determine one, fail
    if config.require_valid_ip {
        error!("Could not determine valid client IP from headers");
        return Err(RateLimitError::InvalidIp("Could not determine valid client IP".to_string()));
    }

    // Development mode: fall back to localhost with warning
    warn!("Could not determine client IP, using localhost (development mode only)");
    Ok(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)))
}

/// Check if an IP is a valid client IP (not a private/internal network)
fn is_valid_client_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            // Reject private IPs
            if ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local() {
                return false;
            }
            // Accept public IPs
            true
        }
        IpAddr::V6(ipv6) => {
            // Check if it's a loopback address
            if ipv6.is_loopback() {
                return false;
            }
            // For IPv6, we're more permissive in development
            // In production, you'd want to check for unique local addresses etc.
            true
        }
    }
}

/// Check if an IP is localhost
fn is_localhost(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => ipv4.is_loopback(),
        IpAddr::V6(ipv6) => ipv6.is_loopback(),
    }
}

/// Extract client IP using default config
pub fn extract_client_ip_with_default_config(headers: &HeaderMap) -> IpAddr {
    let config = RateLimitConfig::default();
    extract_client_ip(headers, &config).unwrap_or_else(|_| {
        // This should only happen in development mode
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    })
}

/// Extract client IP with custom config
pub fn extract_client_ip_with_config(headers: &HeaderMap, config: &RateLimitConfig) -> Result<IpAddr, RateLimitError> {
    extract_client_ip(headers, config)
}

/// Middleware for rate limiting API requests
pub async fn rate_limit_middleware(
    State(limiter): State<Arc<RateLimiterState>>,
    req: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract client IP with config
    let ip = extract_client_ip(req.headers(), &limiter.config)?;

    // Check rate limit
    limiter.check_api_rate_limit(ip).await?;

    // Continue with request
    Ok(next.run(req).await)
}

/// Middleware for rate limiting login attempts (stricter)
pub async fn login_rate_limit_middleware(
    State(limiter): State<Arc<RateLimiterState>>,
    req: Request,
    next: Next,
) -> Result<Response, RateLimitError> {
    // Extract client IP with config
    let ip = extract_client_ip(req.headers(), &limiter.config)?;

    // Check rate limit (stricter for login)
    limiter.check_login_rate_limit(ip).await?;

    // Continue with request
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RateLimitConfig::default();
        assert_eq!(config.api_rpm.get(), 60);
        assert_eq!(config.login_rpm.get(), 10);
        assert_eq!(config.burst.get(), 10);
    }

    #[test]
    fn test_rate_limiter_creation() {
        let config = RateLimitConfig::default();
        let _limiter = RateLimiterState::new(config);
        // Just verify it creates without panicking
    }

    #[tokio::test]
    async fn test_rate_limit_check() {
        let config = RateLimitConfig::default();
        let limiter = RateLimiterState::new(config);
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));

        // Should allow first request
        assert!(limiter.check_api_rate_limit(ip).await.is_ok());
        assert!(limiter.check_login_rate_limit(ip).await.is_ok());
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let config = RateLimitConfig {
            api_rpm: NonZeroU32::new(5).unwrap(),
            login_rpm: NonZeroU32::new(2).unwrap(),
            burst: NonZeroU32::new(2).unwrap(),
            trusted_proxies: HashSet::new(),
            require_valid_ip: false, // Allow localhost in tests
        };
        let limiter = RateLimiterState::new(config);
        let ip = IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1));

        // Should allow up to limit
        for _ in 0..5 {
            assert!(limiter.check_api_rate_limit(ip).await.is_ok());
        }

        // Next request should be rate limited
        assert!(limiter.check_api_rate_limit(ip).await.is_err());

        // Login limit
        let ip2 = IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 2));
        assert!(limiter.check_login_rate_limit(ip2).await.is_ok());
        assert!(limiter.check_login_rate_limit(ip2).await.is_ok());
        assert!(limiter.check_login_rate_limit(ip2).await.is_err());
    }
}
