//! Database layer for the indexer
//!
//! Handles database connections, migrations, and data persistence.

pub mod archival;
pub mod connection;
pub mod health;
pub mod health_scores;
mod migrations;

pub use archival::ArchivalManager;
pub use connection::Database;
pub use health::{HealthMetric, HealthMonitor, PoolStats};
pub use health_scores::{HealthScoreRecord, HealthScoreWriter};
