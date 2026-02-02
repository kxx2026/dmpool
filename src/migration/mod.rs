//! Database Migration Module
//!
//! Handles schema versioning and migrations for DMPool database.
//!
//! # Features
//! - Version tracking in database
//! - Sequential migration execution
//! - Rollback support
//! - Idempotent operations

pub mod error;
pub mod schema;
pub mod runner;

pub use error::MigrationError;
pub use schema::{Migration, SchemaVersion};
pub use runner::MigrationRunner;

use p2poolv2_lib::store::Store;
use std::sync::Arc;

/// Current database schema version
pub const CURRENT_SCHEMA_VERSION: u32 = 1;

/// Initialize migration system and run pending migrations
///
/// # Arguments
/// * `store` - Arc to the database store
///
/// # Returns
/// * `Ok(u32)` - Current schema version after migrations
/// * `Err(MigrationError)` - Migration error
pub async fn setup_migrations(store: Arc<Store>) -> Result<u32, MigrationError> {
    let runner = MigrationRunner::new(store);
    runner.run_pending().await
}
