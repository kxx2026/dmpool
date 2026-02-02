//! Migration error types

use std::fmt;

pub type Result<T> = std::result::Result<T, MigrationError>;

#[derive(Debug)]
pub enum MigrationError {
    Database(String),
    VersionCorrupted(String),
    MigrationFailed { version: u32, message: String },
    RollbackFailed { version: u32, message: String },
    InvalidVersion(u32),
}

impl fmt::Display for MigrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MigrationError::Database(msg) => write!(f, "Database error: {}", msg),
            MigrationError::VersionCorrupted(msg) => write!(f, "Version corrupted: {}", msg),
            MigrationError::MigrationFailed { version, message } => {
                write!(f, "Migration {} failed: {}", version, message)
            }
            MigrationError::RollbackFailed { version, message } => {
                write!(f, "Rollback to {} failed: {}", version, message)
            }
            MigrationError::InvalidVersion(v) => write!(f, "Invalid schema version: {}", v),
        }
    }
}

impl std::error::Error for MigrationError {}
