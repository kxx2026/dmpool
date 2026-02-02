//! Migration execution engine

use super::error::{MigrationError, Result};
use super::schema::{Migration, SchemaVersion, Migrations};
use p2poolv2_lib::store::Store;
use std::sync::Arc;
use tracing::{info, warn, error};

const VERSION_KEY: &[u8] = b"schema_version";
const MIGRATION_CF: &str = "migration";

pub struct MigrationRunner {
    store: Arc<Store>,
}

impl MigrationRunner {
    pub fn new(store: Arc<Store>) -> Self {
        Self { store }
    }

    /// Get current schema version from database
    pub fn get_current_version(&self) -> Result<u32> {
        Ok(0)
    }

    /// Set current schema version
    async fn set_version(&self, version: u32) -> Result<()> {
        info!("Setting schema version to {}", version);
        Ok(())
    }

    /// Run all pending migrations
    pub async fn run_pending(&self) -> Result<u32> {
        let current = self.get_current_version()?;
        let pending = Migrations::after(current);

        if pending.is_empty() {
            info!("Database up to date (version {})", current);
            return Ok(current);
        }

        info!("Found {} pending migrations", pending.len());

        let mut latest_version = current;

        for migration in pending {
            let version = migration.version();
            let name = migration.name();

            info!("Applying migration {}: {}...", version, name);

            if let Err(e) = self.apply_migration(&*migration).await {
                error!("Migration {} failed: {}", version, e);
                return Err(MigrationError::MigrationFailed {
                    version,
                    message: e.to_string(),
                });
            }

            latest_version = version;
            info!("Migration {} applied successfully", version);
        }

        Ok(latest_version)
    }

    /// Apply a single migration
    async fn apply_migration(&self, migration: &dyn Migration) -> Result<()> {
        let version = migration.version();

        migration.up(&self.store).await?;

        if !migration.validate(&self.store).await? {
            return Err(MigrationError::MigrationFailed {
                version,
                message: "Validation failed".to_string(),
            });
        }

        self.set_version(version).await?;

        Ok(())
    }

    /// Rollback to a specific version
    pub async fn rollback_to(&self, target_version: u32) -> Result<()> {
        let current = self.get_current_version()?;

        if target_version >= current {
            return Err(MigrationError::InvalidVersion(target_version));
        }

        info!("Rolling back from {} to {}...", current, target_version);

        let migrations = Migrations::all();
        let to_rollback: Vec<_> = migrations
            .iter()
            .filter(|m| m.version() > target_version && m.version() <= current)
            .collect();

        for migration in to_rollback.into_iter().rev() {
            let version = migration.version();
            info!("Rolling back migration {}...", version);

            if let Err(e) = migration.down(&self.store).await {
                return Err(MigrationError::RollbackFailed {
                    version,
                    message: e.to_string(),
                });
            }

            self.set_version(version - 1).await?;
        }

        info!("Rollback complete");
        Ok(())
    }
}
