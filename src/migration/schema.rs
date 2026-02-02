//! Migration schema definitions

use crate::migration::error::Result;
use p2poolv2_lib::store::Store;
use async_trait::async_trait;

/// Schema version information
#[derive(Debug, Clone)]
pub struct SchemaVersion {
    pub version: u32,
    pub name: String,
    pub applied_at: chrono::DateTime<chrono::Utc>,
}

/// Migration trait for database migrations
#[async_trait]
pub trait Migration: Send + Sync {
    /// Return the version number of this migration
    fn version(&self) -> u32;

    /// Return a descriptive name for this migration
    fn name(&self) -> &str;

    /// Apply the migration
    async fn up(&self, store: &Store) -> Result<()>;

    /// Rollback the migration
    async fn down(&self, store: &Store) -> Result<()>;

    /// Validate the migration was applied successfully
    async fn validate(&self, store: &Store) -> Result<bool> {
        Ok(true)
    }
}

/// Migration registry
pub struct Migrations;

impl Migrations {
    /// Get all migrations in version order
    pub fn all() -> Vec<Box<dyn Migration>> {
        vec![
            // Add migrations here
            // Box::new(migrations::m001_initial::InitialMigration),
        ]
    }

    /// Get migrations after a specific version
    pub fn after(version: u32) -> Vec<Box<dyn Migration>> {
        Self::all()
            .into_iter()
            .filter(|m| m.version() > version)
            .collect()
    }
}
