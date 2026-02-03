// Two-Factor Authentication (2FA) module for DMPool Admin
// Implements TOTP-based 2FA with QR code setup and backup codes
// TOTP secrets are encrypted at rest using AES-256-GCM

use anyhow::{Context, Result};
use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose};
use chrono::{DateTime, Utc};
use qrcode::QrCode;
use rand::distributions::Distribution;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::sync::RwLock;
use totp_rs::{Algorithm, TOTP};
use tracing::{error, info, warn};

/// Encrypted TOTP secret storage
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EncryptedSecret {
    /// Encrypted secret bytes (base64 encoded)
    pub ciphertext: String,
    /// Nonce used for encryption (base64 encoded)
    pub nonce: String,
}

/// Encryption key manager
struct EncryptionKey {
    key: [u8; 32], // AES-256 key
}

impl EncryptionKey {
    /// Create a new encryption key from environment or generate one
    fn from_env_or_generate() -> Self {
        if let Ok(key_str) = std::env::var("TWO_FACTOR_ENCRYPTION_KEY") {
            // Decode base64 key
            let key_bytes = general_purpose::STANDARD
                .decode(key_str)
                .expect("Invalid TWO_FACTOR_ENCRYPTION_KEY: must be valid base64");

            if key_bytes.len() != 32 {
                panic!("TWO_FACTOR_ENCRYPTION_KEY must be 32 bytes (256 bits) after base64 decoding");
            }

            let mut key = [0u8; 32];
            key.copy_from_slice(&key_bytes);
            Self { key }
        } else {
            // Generate a new key
            let key = Aes256Gcm::generate_key(&mut OsRng);
            let key_array: [u8; 32] = key.into();
            warn!("Generated new TOTP encryption key. Set TWO_FACTOR_ENCRYPTION_KEY environment variable to persist.");
            warn!("Export this key: {}", general_purpose::STANDARD.encode(&key_array));
            Self { key: key_array }
        }
    }

    /// Get the key bytes
    fn as_bytes(&self) -> &[u8; 32] {
        &self.key
    }
}

/// Encrypt data using AES-256-GCM
fn encrypt_data(plaintext: &[u8], key: &EncryptionKey) -> Result<EncryptedSecret> {
    let cipher = Aes256Gcm::new(key.as_bytes().into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher.encrypt(&nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;

    Ok(EncryptedSecret {
        ciphertext: general_purpose::STANDARD.encode(&ciphertext),
        nonce: general_purpose::STANDARD.encode(&nonce),
    })
}

/// Decrypt data using AES-256-GCM
fn decrypt_data(encrypted: &EncryptedSecret, key: &EncryptionKey) -> Result<Vec<u8>> {
    let cipher = Aes256Gcm::new(key.as_bytes().into());

    let nonce = general_purpose::STANDARD
        .decode(&encrypted.nonce)
        .map_err(|e| anyhow::anyhow!("Failed to decode nonce: {}", e))?;

    let ciphertext = general_purpose::STANDARD
        .decode(&encrypted.ciphertext)
        .map_err(|e| anyhow::anyhow!("Failed to decode ciphertext: {}", e))?;

    let nonce = Nonce::from_slice(&nonce);
    let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
        .map_err(|e| anyhow::anyhow!("Decryption failed: {}", e))?;

    Ok(plaintext)
}

/// TOTP secret for a user (stored encrypted at rest)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TotpSecret {
    /// Username associated with this secret
    pub username: String,
    /// Encrypted secret (for storage)
    pub encrypted_secret: Option<EncryptedSecret>,
    /// Decrypted secret (for runtime use only, never serialized)
    #[serde(skip)]
    pub secret: Option<String>,
    /// When this secret was created
    pub created_at: DateTime<Utc>,
    /// Whether 2FA is enabled for this user
    pub enabled: bool,
}

/// Backup codes for account recovery
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BackupCodes {
    /// Username associated with these codes
    pub username: String,
    /// List of backup codes (hashed)
    pub codes: Vec<String>,
    /// When these codes were generated
    pub created_at: DateTime<Utc>,
}

/// 2FA setup response with QR code
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TwoFactorSetup {
    /// Base32 encoded secret (for manual entry)
    pub secret: String,
    /// QR code as base64 PNG image
    pub qr_code: String,
    /// Backup codes for recovery
    pub backup_codes: Vec<String>,
}

/// 2FA verification request
#[derive(Clone, Debug, Deserialize)]
pub struct TwoFactorVerify {
    pub username: String,
    pub code: String,
    pub backup_code: Option<String>,
}

/// 2FA enable request
#[derive(Clone, Debug, Deserialize)]
pub struct TwoFactorEnable {
    pub username: String,
    pub code: String,
}

/// 2FA login request
#[derive(Clone, Debug, Deserialize)]
pub struct TwoFactorLogin {
    pub username: String,
    pub totp_code: Option<String>,
    pub backup_code: Option<String>,
}

/// 2FA status response
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TwoFactorStatus {
    pub enabled: bool,
    pub has_backup_codes: bool,
}

/// Rate limit tracker for 2FA attempts
#[derive(Clone, Debug)]
pub struct TwoFactorRateLimit {
    pub attempts: u32,
    pub locked_until: Option<DateTime<Utc>>,
}

/// Two-Factor Authentication manager
pub struct TwoFactorManager {
    /// TOTP secrets storage
    secrets: Arc<RwLock<HashMap<String, TotpSecret>>>,
    /// Backup codes storage
    backup_codes: Arc<RwLock<HashMap<String, BackupCodes>>>,
    /// Rate limiting for failed TOTP attempts
    rate_limits: Arc<RwLock<HashMap<String, TwoFactorRateLimit>>>,
    /// Rate limiting for backup code attempts (separate from TOTP)
    backup_code_rate_limits: Arc<RwLock<HashMap<String, TwoFactorRateLimit>>>,
    /// Storage directory for persistence
    storage_dir: PathBuf,
    /// Maximum failed attempts before lockout
    max_attempts: u32,
    /// Maximum backup code attempts before lockout (lower than TOTP)
    max_backup_attempts: u32,
    /// Lockout duration in seconds
    lockout_duration: i64,
    /// Issuer name for TOTP (e.g., "DMPool Admin")
    issuer: String,
    /// Encryption key for TOTP secrets
    encryption_key: Arc<EncryptionKey>,
}

impl TwoFactorManager {
    /// Create a new 2FA manager
    pub fn new(storage_dir: PathBuf, issuer: String) -> Self {
        let encryption_key = Arc::new(EncryptionKey::from_env_or_generate());

        Self {
            secrets: Arc::new(RwLock::new(HashMap::new())),
            backup_codes: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
            backup_code_rate_limits: Arc::new(RwLock::new(HashMap::new())),
            storage_dir,
            max_attempts: 5,
            max_backup_attempts: 3, // Fewer attempts for backup codes
            lockout_duration: 300, // 5 minutes
            issuer,
            encryption_key,
        }
    }

    /// Initialize the 2FA manager
    pub async fn initialize(&self) -> Result<()> {
        // Create storage directory
        fs::create_dir_all(&self.storage_dir).await
            .context("Failed to create 2FA storage directory")?;

        // Load existing secrets
        self.load_secrets().await?;

        info!("2FA manager initialized");

        Ok(())
    }

    /// Load TOTP secrets from disk
    async fn load_secrets(&self) -> Result<()> {
        let secrets_file = self.storage_dir.join("totp_secrets.json");
        let backup_file = self.storage_dir.join("backup_codes.json");

        // Load TOTP secrets
        if secrets_file.exists() {
            let json = fs::read_to_string(&secrets_file).await
                .context("Failed to read TOTP secrets file")?;
            let loaded_secrets: HashMap<String, TotpSecret> = serde_json::from_str(&json)
                .context("Failed to parse TOTP secrets")?;

            // Decrypt secrets
            let mut secrets = HashMap::new();
            for (username, mut secret) in loaded_secrets {
                if let Some(encrypted) = secret.encrypted_secret.take() {
                    match decrypt_data(&encrypted, &self.encryption_key) {
                        Ok(decrypted_bytes) => {
                            let secret_string = base32::encode(base32::Alphabet::Rfc4648 { padding: true }, &decrypted_bytes);
                            secret.secret = Some(secret_string);
                        }
                        Err(e) => {
                            error!("Failed to decrypt TOTP secret for user '{}': {}", username, e);
                            continue;
                        }
                    }
                }
                secrets.insert(username, secret);
            }

            let count = secrets.len();
            *self.secrets.write().await = secrets;
            info!("Loaded {} TOTP secrets", count);
        }

        // Load backup codes
        if backup_file.exists() {
            let json = fs::read_to_string(&backup_file).await
                .context("Failed to read backup codes file")?;
            let codes: HashMap<String, BackupCodes> = serde_json::from_str(&json)
                .context("Failed to parse backup codes")?;
            let count = codes.len();
            *self.backup_codes.write().await = codes;
            info!("Loaded backup codes for {} users", count);
        }

        Ok(())
    }

    /// Save TOTP secrets to disk (encrypting before save)
    async fn save_secrets(&self) -> Result<()> {
        let secrets_file = self.storage_dir.join("totp_secrets.json");

        // Encrypt secrets before saving
        let secrets = self.secrets.read().await;
        let mut secrets_to_save = HashMap::new();

        for (username, secret) in secrets.iter() {
            let mut secret_to_save = secret.clone();

            // Encrypt the secret if we have the plaintext
            if let Some(plaintext) = &secret.secret {
                let secret_bytes = base32::decode(base32::Alphabet::Rfc4648 { padding: true }, plaintext)
                    .context("Failed to decode secret for encryption")?;

                let encrypted = encrypt_data(&secret_bytes, &self.encryption_key)
                    .context("Failed to encrypt TOTP secret")?;

                secret_to_save.encrypted_secret = Some(encrypted);
                // Don't save the plaintext
                secret_to_save.secret = None;
            }

            secrets_to_save.insert(username.clone(), secret_to_save);
        }

        drop(secrets);

        let json = serde_json::to_string_pretty(&secrets_to_save)
            .context("Failed to serialize TOTP secrets")?;
        fs::write(&secrets_file, json).await
            .context("Failed to write TOTP secrets file")?;
        Ok(())
    }

    /// Save backup codes to disk
    async fn save_backup_codes(&self) -> Result<()> {
        let backup_file = self.storage_dir.join("backup_codes.json");
        let codes = self.backup_codes.read().await;
        let json = serde_json::to_string_pretty(&*codes)
            .context("Failed to serialize backup codes")?;
        fs::write(&backup_file, json).await
            .context("Failed to write backup codes file")?;
        Ok(())
    }

    /// Generate a new TOTP secret for a user
    pub async fn generate_secret(&self, username: &str) -> Result<TwoFactorSetup> {
        // Generate a random secret (20 bytes = 160 bits)
        let secret_bytes = Self::generate_random_secret();

        // Convert to base32 for display
        let secret_string = base32::encode(base32::Alphabet::Rfc4648 { padding: true }, &secret_bytes);

        // Create TOTP object
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes.clone(),
            Some(self.issuer.clone()),
            username.to_string(),
        ).context("Failed to create TOTP")?;

        // Generate QR code
        let qr_uri = totp.get_url();
        let qr_code = Self::generate_qr_code(&qr_uri)
            .context("Failed to generate QR code")?;

        // Generate backup codes
        let backup_codes = Self::generate_backup_codes();

        // Store the secret (not enabled yet)
        let totp_secret = TotpSecret {
            username: username.to_string(),
            secret: Some(secret_string.clone()),
            encrypted_secret: None, // Will be encrypted when saved
            created_at: Utc::now(),
            enabled: false,
        };

        let mut secrets = self.secrets.write().await;
        secrets.insert(username.to_string(), totp_secret);
        drop(secrets);

        self.save_secrets().await?;

        // Store hashed backup codes
        let hashed_codes: Vec<String> = backup_codes.iter()
            .map(|code| Self::hash_backup_code(code))
            .collect();

        let backup_data = BackupCodes {
            username: username.to_string(),
            codes: hashed_codes,
            created_at: Utc::now(),
        };

        let mut codes = self.backup_codes.write().await;
        codes.insert(username.to_string(), backup_data);
        drop(codes);

        self.save_backup_codes().await?;

        info!("Generated TOTP secret for user '{}'", username);

        Ok(TwoFactorSetup {
            secret: secret_string,
            qr_code,
            backup_codes,
        })
    }

    /// Enable 2FA for a user after verification
    pub async fn enable_2fa(&self, username: &str, code: &str) -> Result<bool> {
        // Check rate limit
        if self.is_rate_limited(username).await {
            return Ok(false);
        }

        // Get the secret
        let secret = {
            let secrets = self.secrets.read().await;
            secrets.get(username).cloned()
        };

        let secret = secret.ok_or_else(|| anyhow::anyhow!("No TOTP secret found for user"))?;

        let secret_value = secret.secret.as_ref()
            .ok_or_else(|| anyhow::anyhow!("TOTP secret not available for user '{}'", username))?;

        // Verify the code
        if self.verify_totp_code(secret_value, code)? {
            // Mark as enabled
            let mut secrets = self.secrets.write().await;
            if let Some(s) = secrets.get_mut(username) {
                s.enabled = true;
            }
            drop(secrets);

            self.save_secrets().await?;
            self.clear_rate_limit(username).await;

            info!("Enabled 2FA for user '{}'", username);
            Ok(true)
        } else {
            self.record_failed_attempt(username).await;
            warn!("Failed 2FA enable attempt for user '{}'", username);
            Ok(false)
        }
    }

    /// Disable 2FA for a user
    pub async fn disable_2fa(&self, username: &str) -> Result<()> {
        let mut secrets = self.secrets.write().await;
        if let Some(secret) = secrets.get_mut(username) {
            secret.enabled = false;
        }
        drop(secrets);

        self.save_secrets().await?;

        info!("Disabled 2FA for user '{}'", username);
        Ok(())
    }

    /// Verify a 2FA code during login
    pub async fn verify_login(&self, username: &str, totp_code: Option<&str>, backup_code: Option<&str>) -> Result<bool> {
        // Get the secret
        let secret = {
            let secrets = self.secrets.read().await;
            secrets.get(username).cloned()
        };

        let secret = match secret {
            Some(s) if s.enabled => s,
            Some(_) => {
                // 2FA not enabled, skip verification
                return Ok(true);
            }
            None => {
                // No 2FA configured, skip verification
                return Ok(true);
            }
        };

        // Try TOTP code first
        if let Some(code) = totp_code {
            // Check rate limit
            if self.is_rate_limited(username).await {
                warn!("User '{}' is rate limited for TOTP", username);
                return Ok(false);
            }

            let secret_value = secret.secret.as_ref()
                .ok_or_else(|| anyhow::anyhow!("TOTP secret not available for user '{}'", username))?;

            if self.verify_totp_code(secret_value, code)? {
                self.clear_rate_limit(username).await;
                info!("User '{}' authenticated via TOTP", username);
                return Ok(true);
            } else {
                self.record_failed_attempt(username).await;
            }
        }

        // Try backup code (with separate rate limiting)
        if let Some(code) = backup_code {
            if self.verify_backup_code_with_rate_limit(username, code).await? {
                // Remove the used backup code
                self.consume_backup_code(username, code).await?;
                self.clear_rate_limit(username).await;
                info!("User '{}' authenticated via backup code", username);
                return Ok(true);
            }
        }

        warn!("Failed 2FA verification for user '{}'", username);
        Ok(false)
    }

    /// Get 2FA status for a user
    pub async fn get_status(&self, username: &str) -> TwoFactorStatus {
        let secrets = self.secrets.read().await;
        let codes = self.backup_codes.read().await;

        let enabled = secrets.get(username)
            .map(|s| s.enabled)
            .unwrap_or(false);

        let has_backup_codes = codes.get(username)
            .map(|c| !c.codes.is_empty())
            .unwrap_or(false);

        TwoFactorStatus {
            enabled,
            has_backup_codes,
        }
    }

    /// Regenerate backup codes for a user
    pub async fn regenerate_backup_codes(&self, username: &str) -> Result<Vec<String>> {
        let backup_codes = Self::generate_backup_codes();

        // Store hashed backup codes
        let hashed_codes: Vec<String> = backup_codes.iter()
            .map(|code| Self::hash_backup_code(code))
            .collect();

        let backup_data = BackupCodes {
            username: username.to_string(),
            codes: hashed_codes,
            created_at: Utc::now(),
        };

        let mut codes = self.backup_codes.write().await;
        codes.insert(username.to_string(), backup_data);
        drop(codes);

        self.save_backup_codes().await?;

        info!("Regenerated backup codes for user '{}'", username);

        Ok(backup_codes)
    }

    /// Check if a user is rate limited
    async fn is_rate_limited(&self, username: &str) -> bool {
        let limits = self.rate_limits.read().await;
        if let Some(limit) = limits.get(username) {
            if let Some(locked_until) = limit.locked_until {
                if Utc::now() < locked_until {
                    return true;
                }
            }
        }
        false
    }

    /// Check if a user is rate limited for backup codes
    async fn is_backup_code_rate_limited(&self, username: &str) -> bool {
        let limits = self.backup_code_rate_limits.read().await;
        if let Some(limit) = limits.get(username) {
            if let Some(locked_until) = limit.locked_until {
                if Utc::now() < locked_until {
                    return true;
                }
            }
        }
        false
    }

    /// Record a failed 2FA attempt
    async fn record_failed_attempt(&self, username: &str) {
        let mut limits = self.rate_limits.write().await;
        let limit = limits.entry(username.to_string()).or_insert_with(|| TwoFactorRateLimit {
            attempts: 0,
            locked_until: None,
        });

        limit.attempts += 1;

        if limit.attempts >= self.max_attempts {
            limit.locked_until = Some(Utc::now() + chrono::Duration::seconds(self.lockout_duration));
            warn!("User '{}' locked out due to too many failed 2FA attempts", username);
        }
    }

    /// Record a failed backup code attempt
    async fn record_failed_backup_attempt(&self, username: &str) {
        let mut limits = self.backup_code_rate_limits.write().await;
        let limit = limits.entry(username.to_string()).or_insert_with(|| TwoFactorRateLimit {
            attempts: 0,
            locked_until: None,
        });

        limit.attempts += 1;

        if limit.attempts >= self.max_backup_attempts {
            limit.locked_until = Some(Utc::now() + chrono::Duration::seconds(self.lockout_duration));
            warn!("User '{}' locked out due to too many failed backup code attempts", username);
        }
    }

    /// Clear rate limit after successful attempt
    async fn clear_rate_limit(&self, username: &str) {
        let mut limits = self.rate_limits.write().await;
        if let Some(limit) = limits.get_mut(username) {
            limit.attempts = 0;
            limit.locked_until = None;
        }
    }

    /// Clear backup code rate limit after successful attempt
    async fn clear_backup_code_rate_limit(&self, username: &str) {
        let mut limits = self.backup_code_rate_limits.write().await;
        if let Some(limit) = limits.get_mut(username) {
            limit.attempts = 0;
            limit.locked_until = None;
        }
    }

    /// Verify a TOTP code
    fn verify_totp_code(&self, secret: &str, code: &str) -> Result<bool> {
        // Convert base32 secret to bytes
        let secret_bytes = base32::decode(base32::Alphabet::Rfc4648 { padding: true }, secret)
            .context("Failed to decode base32 secret")?;

        // Create TOTP object
        let totp = TOTP::new(
            Algorithm::SHA1,
            6,
            1,
            30,
            secret_bytes,
            None,
            String::new(),
        ).context("Failed to create TOTP")?;

        // Check code (allows for 1 step drift = 30 seconds)
        let is_valid = totp.check_current(code)?;

        Ok(is_valid)
    }

    /// Verify a backup code (with rate limiting check must be done before calling)
    async fn verify_backup_code(&self, username: &str, code: &str) -> Result<bool> {
        let hashed = Self::hash_backup_code(code);

        let codes = self.backup_codes.read().await;
        if let Some(backup) = codes.get(username) {
            Ok(backup.codes.contains(&hashed))
        } else {
            Ok(false)
        }
    }

    /// Verify a backup code with rate limiting
    async fn verify_backup_code_with_rate_limit(&self, username: &str, code: &str) -> Result<bool> {
        // Check rate limit first
        if self.is_backup_code_rate_limited(username).await {
            warn!("User '{}' is rate limited for backup codes", username);
            return Ok(false);
        }

        let is_valid = self.verify_backup_code(username, code).await?;

        if is_valid {
            self.clear_backup_code_rate_limit(username).await;
        } else {
            self.record_failed_backup_attempt(username).await;
        }

        Ok(is_valid)
    }

    /// Consume a used backup code
    async fn consume_backup_code(&self, username: &str, code: &str) -> Result<()> {
        let hashed = Self::hash_backup_code(code);

        let mut codes = self.backup_codes.write().await;
        if let Some(backup) = codes.get_mut(username) {
            backup.codes.retain(|c| c != &hashed);
        }

        self.save_backup_codes().await?;
        Ok(())
    }

    /// Generate random secret bytes
    fn generate_random_secret() -> Vec<u8> {
        use rand::Rng;
        let mut secret = [0u8; 20];
        rand::thread_rng().fill(&mut secret);
        secret.to_vec()
    }

    /// Generate random backup codes
    fn generate_backup_codes() -> Vec<String> {
        use rand::distributions::Uniform;
        use std::fmt::Write;

        let dist = Uniform::new_inclusive(0u16, 9999);

        (0..10).map(|_| {
            let mut code = String::new();
            let mut rng = rand::thread_rng();
            for _ in 0..4 {
                let part = dist.sample(&mut rng);
                write!(&mut code, "{:04}", part).unwrap();
            }
            code
        }).collect()
    }

    /// Hash a backup code
    fn hash_backup_code(code: &str) -> String {
        use sha2::{Sha256, Digest};
        let mut hasher = Sha256::new();
        hasher.update(code.as_bytes());
        format!("{:x}", hasher.finalize())
    }

    /// Generate QR code as base64 PNG
    fn generate_qr_code(uri: &str) -> Result<String> {
        let qr_code = QrCode::new(uri.as_bytes())
            .context("Failed to create QR code")?;

        let image = qr_code.render::<image::Luma<u8>>().build();

        let mut buffer = Vec::new();
        let mut cursor = std::io::Cursor::new(&mut buffer);

        image.write_to(&mut cursor, image::ImageFormat::Png)
            .context("Failed to write PNG")?;

        let base64_string = general_purpose::STANDARD.encode(&buffer);
        Ok(format!("data:image/png;base64,{}", base64_string))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_generate_secret() {
        let temp_dir = std::env::temp_dir();
        let manager = TwoFactorManager::new(
            temp_dir.join("2fa_test"),
            "TestApp".to_string()
        );

        manager.initialize().await.unwrap();

        let setup = manager.generate_secret("testuser").await.unwrap();

        assert!(!setup.secret.is_empty());
        assert!(setup.qr_code.starts_with("data:image/png;base64,"));
        assert_eq!(setup.backup_codes.len(), 10);
    }

    #[tokio::test]
    async fn test_2fa_enable_disable() {
        let temp_dir = std::env::temp_dir();
        let manager = TwoFactorManager::new(
            temp_dir.join("2fa_test2"),
            "TestApp".to_string()
        );

        manager.initialize().await.unwrap();

        // Generate secret
        let _setup = manager.generate_secret("testuser").await.unwrap();

        // Check status
        let status = manager.get_status("testuser").await;
        assert!(!status.enabled); // Not enabled yet
    }

    #[test]
    fn test_generate_backup_codes() {
        let codes = TwoFactorManager::generate_backup_codes();
        assert_eq!(codes.len(), 10);
        for code in &codes {
            assert_eq!(code.len(), 16); // 4 groups of 4 digits
        }
    }
}
