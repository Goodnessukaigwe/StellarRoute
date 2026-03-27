//! Background graph manager for routing caching
use std::sync::Arc;
use tokio::sync::RwLock;
use sqlx::{PgPool, Row};
use tracing::{info, error};

use stellarroute_routing::pathfinder::LiquidityEdge;

/// Daemon that maintains an active in-memory cache of the routing graph
pub struct GraphManager {
    db: PgPool,
    edges: Arc<RwLock<Vec<LiquidityEdge>>>,
}

impl GraphManager {
    /// Create uninitialized graph manager
    pub fn new(db: PgPool) -> Self {
        Self {
            db,
            edges: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Retrieve the current live copy of the routing graph
    pub async fn get_edges(&self) -> Vec<LiquidityEdge> {
        self.edges.read().await.clone()
    }

    /// Spawn a background task to keep the graph updated
    pub fn start_sync(self: Arc<Self>) {
        info!("Starting background routing graph sync task");
        let manager = self.clone();
        tokio::spawn(async move {
            // Initial sync immediately
            if let Err(e) = manager.sync_graph().await {
                error!("Failed initial sync for routing graph: {}", e);
            }
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
            loop {
                interval.tick().await;
                if let Err(e) = manager.sync_graph().await {
                    error!("Failed to sync routing graph: {}", e);
                }
            }
        });
    }

    async fn sync_graph(&self) -> Result<(), sqlx::Error> {
        let assets = sqlx::query("SELECT id, asset_type, asset_code, asset_issuer FROM assets")
            .fetch_all(&self.db).await?;
            
        let mut hash_map = std::collections::HashMap::with_capacity(assets.len());
        for row in assets {
            let id: uuid::Uuid = row.get("id");
            let a_type: String = row.get("asset_type");
            let a_code: Option<String> = row.get("asset_code");
            let a_iss: Option<String> = row.get("asset_issuer");
            
            let canon = if a_type != "native" {
                if let Some(iss) = a_iss { 
                    format!("{}:{}", a_code.unwrap_or_default(), iss)
                } else { 
                    a_code.unwrap_or_default() 
                }
            } else {
                "native".to_string()
            };
            hash_map.insert(id, canon);
        }

        let rows = sqlx::query(
            r#"
            SELECT selling_asset_id, buying_asset_id, venue_type, venue_ref, price, available_amount
            FROM normalized_liquidity
            WHERE available_amount > 0
            "#
        ).fetch_all(&self.db).await?;

        let mut next_edges: Vec<LiquidityEdge> = Vec::with_capacity(rows.len());

        for r in rows {
            let s_id: uuid::Uuid = r.get("selling_asset_id");
            let b_id: uuid::Uuid = r.get("buying_asset_id");
            
            if let (Some(e_from), Some(e_to)) = (hash_map.get(&s_id), hash_map.get(&b_id)) {
                let price_str: String = r.get("price");
                let avail_str: String = r.get("available_amount");
                let venue_type: String = r.get("venue_type");
                
                let price = price_str.parse::<f64>().ok();
                let avail = avail_str.parse::<f64>().ok();
                
                if let (Some(p), Some(a)) = (price, avail) {
                    if p > 0.0 && a > 0.0 {
                        let is_amm = venue_type == "amm";
                        next_edges.push(LiquidityEdge {
                            from: e_from.clone(),
                            to: e_to.clone(),
                            venue_type,
                            venue_ref: r.get("venue_ref"),
                            liquidity: (a * 1e7) as i128,
                            price: p,
                            fee_bps: if is_amm { 30 } else { 20 },
                        });
                    }
                }
            }
        }
        
        *self.edges.write().await = next_edges;
        Ok(())
    }
}
