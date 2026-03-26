//! AMM Pool State Aggregator
//!
//! This module provides continuous tracking of AMM pool reserves, fees, and lifecycle events.
//! It polls registered pools from the router contract and updates the database with current state.

use crate::db::Database;
use crate::error::Result;
use crate::models::{PoolReserve, PoolState};
use crate::soroban::{SorobanRpc, SorobanRpcClient};
use chrono::Utc;
use serde_json;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Configuration for AMM pool indexing
#[derive(Clone, Debug)]
pub struct AmmConfig {
    /// Router contract address to query for registered pools
    pub router_contract: String,
    /// Poll interval for pool state updates
    pub poll_interval_secs: u64,
    /// Stale threshold in seconds (pools not updated within this time are considered stale)
    pub stale_threshold_secs: u64,
    /// Maximum number of pools to process per batch
    pub batch_size: usize,
}

impl Default for AmmConfig {
    fn default() -> Self {
        Self {
            router_contract: String::new(),
            poll_interval_secs: 30,
            stale_threshold_secs: 300, // 5 minutes
            batch_size: 50,
        }
    }
}

/// AMM pool aggregator service
pub struct AmmAggregator {
    config: AmmConfig,
    db: Database,
    soroban: SorobanRpcClient,
}

impl AmmAggregator {
    pub fn new(config: AmmConfig, db: Database, soroban: SorobanRpcClient) -> Self {
        Self {
            config,
            db,
            soroban,
        }
    }

    /// Start the continuous aggregation loop
    pub async fn start_aggregation(&self) -> Result<()> {
        info!("Starting AMM pool aggregation loop");

        let mut interval =
            tokio::time::interval(Duration::from_secs(self.config.poll_interval_secs));

        loop {
            interval.tick().await;

            if let Err(e) = self.aggregate_once().await {
                error!("AMM aggregation cycle failed: {}", e);
                // Continue the loop despite errors
            }
        }
    }

    /// Perform a single aggregation cycle
    pub async fn aggregate_once(&self) -> Result<()> {
        debug!("Starting AMM pool aggregation cycle");

        // Get registered pools from router contract
        let pools = self.get_registered_pools().await?;
        info!("Found {} registered pools", pools.len());

        // Process pools in batches
        for batch in pools.chunks(self.config.batch_size) {
            if let Err(e) = self.process_pool_batch(batch).await {
                warn!("Failed to process pool batch: {}", e);
                // Continue with next batch
            }
        }

        // Clean up stale pools
        self.cleanup_stale_pools().await?;

        debug!("Completed AMM pool aggregation cycle");
        Ok(())
    }

    /// Get list of registered pools from router contract
    async fn get_registered_pools(&self) -> Result<Vec<String>> {
        // Call router contract to get pool list
        let request = serde_json::json!({
            "contractId": self.config.router_contract,
            "key": {
                "contract": self.config.router_contract,
                "key": "PoolList",
                "durability": "persistent"
            }
        });

        match self.soroban.request("getContractData", request).await {
            Ok(data) => {
                // Parse the XDR to get pool addresses
                self.parse_pool_list(&data)
            }
            Err(e) => {
                warn!("Failed to query router contract for pools: {}. Using configured pools as fallback.", e);
                self.get_configured_pools().await
            }
        }
    }

    /// Parse pool list from contract data
    fn parse_pool_list(&self, _data: &serde_json::Value) -> Result<Vec<String>> {
        // TODO: Implement proper XDR decoding for Vec<Address>
        // For now, return empty vec
        Ok(vec![])
    }

    /// Get pools from configuration (fallback when router query fails)
    async fn get_configured_pools(&self) -> Result<Vec<String>> {
        // TODO: Load from config or database
        // For now, return empty vec
        Ok(vec![])
    }

    /// Process a batch of pools
    async fn process_pool_batch(&self, pool_addresses: &[String]) -> Result<()> {
        for address in pool_addresses {
            if let Err(e) = self.process_pool(address).await {
                warn!("Failed to process pool {}: {}", address, e);
            }
        }
        Ok(())
    }

    /// Process a single pool
    async fn process_pool(&self, pool_address: &str) -> Result<()> {
        // Get pool state from Soroban RPC
        let state = self.get_pool_state(pool_address).await?;

        // Resolve asset IDs
        let selling_asset_id = self.resolve_asset_id(&state.token_a).await?;
        let buying_asset_id = self.resolve_asset_id(&state.token_b).await?;

        // Update database
        self.update_pool_reserve(&PoolReserve {
            pool_address: pool_address.to_string(),
            selling_asset_id,
            buying_asset_id,
            reserve_selling: rust_decimal::Decimal::from_i128_with_scale(state.reserve_a, 0),
            reserve_buying: rust_decimal::Decimal::from_i128_with_scale(state.reserve_b, 0),
            fee_bps: state.fee_bps,
            last_updated_ledger: state.ledger_sequence,
            updated_at: Utc::now(),
        })
        .await?;

        debug!("Updated pool {} reserves", pool_address);
        Ok(())
    }

    /// Get pool state from Soroban RPC
    async fn get_pool_state(&self, pool_address: &str) -> Result<PoolState> {
        // Get contract data
        let contract_data = self.soroban.get_pool_state(pool_address).await?;

        // Parse the XDR data to extract reserves and fee
        // This is a simplified implementation - real implementation would decode XDR
        self.parse_pool_state(&contract_data, pool_address)
    }

    /// Parse pool state from contract data (simplified)
    fn parse_pool_state(
        &self,
        _contract_data: &serde_json::Value,
        pool_address: &str,
    ) -> Result<PoolState> {
        // TODO: Implement proper XDR decoding
        // For now, return mock data
        Ok(PoolState {
            address: pool_address.to_string(),
            token_a: "CDUMMYTOKENA".to_string(),
            token_b: "CDUMMYTOKENB".to_string(),
            reserve_a: 1000000000, // 1000 units
            reserve_b: 2000000000, // 2000 units
            fee_bps: 30,           // 0.3%
            ledger_sequence: 12345,
        })
    }

    /// Resolve asset ID from contract address
    async fn resolve_asset_id(&self, contract_address: &str) -> Result<uuid::Uuid> {
        use sqlx::Row;

        // Check if asset exists in database
        let pool = self.db.pool();
        let row = sqlx::query("SELECT id FROM assets WHERE asset_type = $1 AND asset_issuer = $2")
            .bind("soroban")
            .bind(contract_address)
            .fetch_optional(pool)
            .await?;

        if let Some(row) = row {
            return Ok(row.get("id"));
        }

        // Insert new asset
        let id = uuid::Uuid::new_v4();
        sqlx::query(
            "INSERT INTO assets (id, asset_type, asset_issuer, created_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind("soroban")
        .bind(contract_address)
        .bind(Utc::now())
        .execute(pool)
        .await?;

        Ok(id)
    }

    /// Update pool reserve in database
    async fn update_pool_reserve(&self, reserve: &PoolReserve) -> Result<()> {
        let pool = self.db.pool();
        sqlx::query("SELECT upsert_amm_pool_reserve($1, $2, $3, $4, $5, $6, $7)")
            .bind(&reserve.pool_address)
            .bind(reserve.selling_asset_id)
            .bind(reserve.buying_asset_id)
            .bind(reserve.reserve_selling.to_string())
            .bind(reserve.reserve_buying.to_string())
            .bind(reserve.fee_bps)
            .bind(reserve.last_updated_ledger)
            .execute(pool)
            .await?;

        Ok(())
    }

    /// Clean up stale pools
    async fn cleanup_stale_pools(&self) -> Result<()> {
        let threshold =
            Utc::now() - chrono::Duration::seconds(self.config.stale_threshold_secs as i64);
        let pool = self.db.pool();

        let result = sqlx::query("DELETE FROM amm_pool_reserves WHERE updated_at < $1")
            .bind(threshold)
            .execute(pool)
            .await?;

        if result.rows_affected() > 0 {
            info!("Cleaned up {} stale pool entries", result.rows_affected());
        }

        Ok(())
    }
}
