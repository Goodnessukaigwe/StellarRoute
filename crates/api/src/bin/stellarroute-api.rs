//! StellarRoute API Server Binary

use sqlx::postgres::PgPoolOptions;
use std::time::Duration;
use stellarroute_api::{telemetry, Server, ServerConfig};
use tracing::{error, info};

#[tokio::main]
async fn main() {
    // Initialize structured logging (reads RUST_LOG and LOG_FORMAT env vars)
    telemetry::init();

    info!("Starting StellarRoute API Server");

    // Get database URL from environment
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/stellarroute".to_string());

    // Read pool configuration from environment variables
    let max_connections: u32 = std::env::var("DB_MAX_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let min_connections: u32 = std::env::var("DB_MIN_CONNECTIONS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2);
    let connection_timeout_secs: u64 = std::env::var("DB_CONNECTION_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);
    let idle_timeout_secs: u64 = std::env::var("DB_IDLE_TIMEOUT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(600);
    let max_lifetime_secs: u64 = std::env::var("DB_MAX_LIFETIME")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(1800);
    let statement_timeout_ms: u64 = std::env::var("DB_STATEMENT_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);
    let lock_timeout_ms: u64 = std::env::var("DB_LOCK_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(2000);
    let idle_in_txn_timeout_ms: u64 = std::env::var("DB_IDLE_IN_TXN_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5000);

    info!(
        "Connecting to database (pool: min={}, max={}, timeout={}s)...",
        min_connections, max_connections, connection_timeout_secs
    );
    info!(
        "DB guardrails: statement_timeout={}ms lock_timeout={}ms idle_in_txn_timeout={}ms",
        statement_timeout_ms, lock_timeout_ms, idle_in_txn_timeout_ms
    );
    let pool = match PgPoolOptions::new()
        .max_connections(max_connections)
        .min_connections(min_connections)
        .acquire_timeout(Duration::from_secs(connection_timeout_secs))
        .idle_timeout(Duration::from_secs(idle_timeout_secs))
        .max_lifetime(Duration::from_secs(max_lifetime_secs))
        .after_connect(move |mut conn, _meta| {
            Box::pin(async move {
                sqlx::query(&format!("SET statement_timeout = '{}ms'", statement_timeout_ms))
                    .execute(&mut conn)
                    .await?;
                sqlx::query(&format!("SET lock_timeout = '{}ms'", lock_timeout_ms))
                    .execute(&mut conn)
                    .await?;
                sqlx::query(&format!(
                    "SET idle_in_transaction_session_timeout = '{}ms'",
                    idle_in_txn_timeout_ms
                ))
                .execute(&mut conn)
                .await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await
    {
        Ok(pool) => {
            info!(
                "✅ Database connection pool established (max_connections={})",
                max_connections
            );
            pool
        }
        Err(e) => {
            error!("❌ Failed to connect to database: {}", e);
            std::process::exit(1);
        }
    };

    // Create server configuration
    let config = ServerConfig {
        host: std::env::var("API_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
        port: std::env::var("API_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000),
        enable_cors: true,
        enable_compression: true,
        redis_url: std::env::var("REDIS_URL").ok(),
        quote_cache_ttl_seconds: std::env::var("QUOTE_CACHE_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2),
    };

    // Create and start server
    let server = Server::new(config, pool).await;

    if let Err(e) = server.start().await {
        error!("Server error: {}", e);
        std::process::exit(1);
    }
}
