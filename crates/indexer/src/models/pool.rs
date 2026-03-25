use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct PoolReserve {
    pub pool_address: String,
    pub selling_asset_id: uuid::Uuid,
    pub buying_asset_id: uuid::Uuid,
    pub reserve_selling: rust_decimal::Decimal,
    pub reserve_buying: rust_decimal::Decimal,
    pub fee_bps: i32,
    pub last_updated_ledger: i64,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PoolState {
    pub address: String,
    pub token_a: String,
    pub token_b: String,
    pub reserve_a: i128,
    pub reserve_b: i128,
    pub fee_bps: i32,
    pub ledger_sequence: i64,
}
