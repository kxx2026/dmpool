// Authentication and Authorization module for DMPool Admin
// JWT-based authentication with bcrypt password hashing

use anyhow::{Context, Result};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Password strength requirements
const MIN_PASSWORD_LENGTH: usize = 12;
const MAX_PASSWORD_LENGTH: usize = 128;

/// Password validation result
#[derive(Debug, Clone)]
pub struct PasswordValidation {
    pub is_valid: bool,
    pub errors: Vec<String>,
}

impl PasswordValidation {
    pub fn valid() -> Self {
        Self {
            is_valid: true,
            errors: Vec::new(),
        }
    }

    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            is_valid: false,
            errors,
        }
    }
}

/// Validate password strength
pub fn validate_password_strength(password: &str) -> PasswordValidation {
    let mut errors = Vec::new();

    // Check length
    if password.len() < MIN_PASSWORD_LENGTH {
        errors.push(format!(
            "Password must be at least {} characters long (got {})",
            MIN_PASSWORD_LENGTH,
            password.len()
        ));
    }

    if password.len() > MAX_PASSWORD_LENGTH {
        errors.push(format!(
            "Password must be at most {} characters long (got {})",
            MAX_PASSWORD_LENGTH,
            password.len()
        ));
    }

    // Check for uppercase letters
    if !password.chars().any(|c| c.is_uppercase()) {
        errors.push("Password must contain at least one uppercase letter".to_string());
    }

    // Check for lowercase letters
    if !password.chars().any(|c| c.is_lowercase()) {
        errors.push("Password must contain at least one lowercase letter".to_string());
    }

    // Check for numbers
    if !password.chars().any(|c| c.is_ascii_digit()) {
        errors.push("Password must contain at least one number".to_string());
    }

    // Check for special characters
    if !password.chars().any(|c| !c.is_alphanumeric()) {
        errors.push("Password must contain at least one special character (!@#$%^&*(),.?\":{}|<>])".to_string());
    }

    // Check for common weak passwords
    let weak_passwords = [
        "password", "Password123!", "Admin123!", "12345678", "qwerty123",
        "letmein123", "welcome123", "monkey123", "dragon123",
    ];
    if weak_passwords.contains(&password) {
        errors.push("Password is too common and weak".to_string());
    }

    if errors.is_empty() {
        PasswordValidation::valid()
    } else {
        PasswordValidation::invalid(errors)
    }
}

/// Claims encoded in JWT token
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// User ID
    pub sub: String,
    /// Username
    pub name: String,
    /// Role (permissions)
    pub role: String,
    /// Issued at
    pub iat: i64,
    /// Expiration time
    pub exp: i64,
}

/// User record stored in database
#[derive(Clone, Serialize, Deserialize)]
pub struct User {
    pub username: String,
    pub password_hash: String,
    pub role: String,
    pub created_at: i64,
    pub last_login: Option<i64>,
}

/// Login request
#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

/// Login response
#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_info: UserInfo,
    pub expires_in: u64, // seconds
}

/// User info returned after login
#[derive(Serialize)]
pub struct UserInfo {
    pub username: String,
    pub role: String,
}

/// Auth state manager
pub struct AuthManager {
    secret: String,
    users: Arc<RwLock<Vec<User>>>,
}

impl AuthManager {
    pub fn new(secret: String) -> Self {
        Self {
            secret,
            users: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Initialize with default admin user
    pub async fn init_default_admin(&self, username: &str, password: &str) -> Result<()> {
        // Validate password strength
        let validation = validate_password_strength(password);
        if !validation.is_valid {
            let error_msg = format!("Password validation failed: {}", validation.errors.join("; "));
            warn!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg)).context("Invalid password");
        }

        let mut users = self.users.write().await;

        // Check if admin already exists
        if users.iter().any(|u| u.username == username) {
            info!("Admin user '{}' already exists, skipping creation", username);
            return Ok(());
        }

        // Hash password
        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        let user = User {
            username: username.to_string(),
            password_hash,
            role: "admin".to_string(),
            created_at: Utc::now().timestamp(),
            last_login: None,
        };

        users.push(user);
        info!("Created default admin user '{}'", username);
        Ok(())
    }

    /// Authenticate user
    pub async fn authenticate(&self, username: &str, password: &str) -> Result<Option<User>> {
        let users = self.users.read().await;

        if let Some(user) = users.iter().find(|u| u.username == username) {
            let is_valid = bcrypt::verify(password, &user.password_hash)
                .unwrap_or(false);

            if is_valid {
                // Update last login
                let mut users = self.users.write().await;
                if let Some(u) = users.iter_mut().find(|u| u.username == username) {
                    u.last_login = Some(Utc::now().timestamp());
                }
                return Ok(Some(user.clone()));
            }
        }

        Ok(None)
    }

    /// Generate JWT token
    pub fn generate_token(&self, user: &User) -> Result<String> {
        let expiration = Utc::now()
            .checked_add_signed(Duration::hours(24))
            .unwrap_or_else(|| Utc::now() + Duration::hours(24))
            .timestamp();

        let claims = Claims {
            sub: user.username.clone(),
            name: user.username.clone(),
            role: user.role.clone(),
            iat: Utc::now().timestamp(),
            exp: expiration,
        };

        let encoding_key = EncodingKey::from_secret(self.secret.as_ref());
        let token = jsonwebtoken::encode(&jsonwebtoken::Header::default(), &claims, &encoding_key)
            .map_err(|e| anyhow::anyhow!("Failed to encode token: {}", e))?;

        Ok(token)
    }

    /// Verify JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims> {
        let decoding_key = DecodingKey::from_secret(self.secret.as_ref());
        let validation = jsonwebtoken::Validation::new(jsonwebtoken::Algorithm::HS256);
        let decoded = jsonwebtoken::decode::<Claims>(token, &decoding_key, &validation)
            .map_err(|e| anyhow::anyhow!("Invalid token: {}", e))?;

        Ok(decoded.claims)
    }

    /// Create user
    pub async fn create_user(&self, username: &str, password: &str, role: &str) -> Result<()> {
        // Validate password strength
        let validation = validate_password_strength(password);
        if !validation.is_valid {
            let error_msg = format!("Password validation failed: {}", validation.errors.join("; "));
            warn!("{}", error_msg);
            return Err(anyhow::anyhow!(error_msg)).context("Invalid password");
        }

        let password_hash = bcrypt::hash(password, bcrypt::DEFAULT_COST)
            .map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

        let user = User {
            username: username.to_string(),
            password_hash,
            role: role.to_string(),
            created_at: Utc::now().timestamp(),
            last_login: None,
        };

        let mut users = self.users.write().await;
        users.push(user);
        info!("Created user '{}' with role '{}'", username, role);
        Ok(())
    }

    /// Get user by username
    pub async fn get_user(&self, username: &str) -> Option<User> {
        let users = self.users.read().await;
        users.iter().find(|u| u.username == username).cloned()
    }
}

/// Authenticated user extractor
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub username: String,
    pub role: String,
}

/// Require authentication middleware
pub async fn require_auth(
    State(auth): State<Arc<AuthManager>>,
    headers: HeaderMap,
) -> Result<AuthenticatedUser, StatusCode> {
    // Get token from Authorization header
    let auth_header = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .ok_or_else(|| {
            warn!("Missing Authorization header");
            StatusCode::UNAUTHORIZED
        })?;

    if !auth_header.starts_with("Bearer ") {
        warn!("Invalid Authorization header format");
        return Err(StatusCode::UNAUTHORIZED);
    }

    let token = &auth_header[7..]; // Remove "Bearer "

    // Verify token
    let claims = auth.verify_token(token)
        .map_err(|e| {
            warn!("Token verification failed: {}", e);
            StatusCode::UNAUTHORIZED
        })?;

    Ok(AuthenticatedUser {
        username: claims.name.clone(),
        role: claims.role,
    })
}

/// Require role middleware
pub fn require_role(required_role: &'static str) -> impl Fn(AuthenticatedUser) -> Result<AuthenticatedUser, StatusCode> {
    move |user: AuthenticatedUser| {
        if user.role == required_role || user.role == "admin" {
            Ok(user)
        } else {
            warn!(
                "User '{}' with role '{}' attempted to access role='{}' resource",
                user.username, user.role, required_role
            );
            Err(StatusCode::FORBIDDEN)
        }
    }
}

/// Login endpoint
pub async fn login(
    State(auth): State<Arc<AuthManager>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, StatusCode> {
    match auth.authenticate(&req.username, &req.password).await {
        Ok(Some(user)) => {
            let token = auth.generate_token(&user)
                .map_err(|e| {
                    error!("Failed to generate token: {}", e);
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

            let expires_in = 24 * 3600; // 24 hours

            info!("User '{}' logged in successfully", req.username);

            Ok(Json(LoginResponse {
                token,
                user_info: UserInfo {
                    username: user.username,
                    role: user.role,
                },
                expires_in,
            }))
        }
        Ok(None) => {
            warn!("Failed login attempt for user '{}'", req.username);
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            error!("Authentication error: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

/// Current user info endpoint
pub async fn me(user: AuthenticatedUser) -> impl IntoResponse {
    Json(UserInfo {
        username: user.username.clone(),
        role: user.role.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let password = "test123";
        let hash = bcrypt::hash(password, bcrypt::DEFAULT_COST).unwrap();
        assert!(bcrypt::verify(password, &hash).unwrap());

        // Wrong password should fail
        assert!(!bcrypt::verify("wrong", &hash).unwrap());
    }

    #[test]
    fn test_jwt_generation() {
        let secret = "test_secret".to_string();
        let auth = AuthManager::new(secret);

        let user = User {
            username: "test".to_string(),
            password_hash: "hash".to_string(),
            role: "user".to_string(),
            created_at: 0,
            last_login: None,
        };

        let token = auth.generate_token(&user).unwrap();
        let claims = auth.verify_token(&token).unwrap();

        assert_eq!(claims.name, "test");
        assert_eq!(claims.role, "user");
    }
}
