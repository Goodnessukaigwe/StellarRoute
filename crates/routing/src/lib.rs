//! StellarRoute Routing Engine
//!
//! Provides pathfinding algorithms for optimal swap routing across SDEX and Soroban AMM pools.
//! Supports N-hop paths with safety bounds, cycle prevention, and price impact calculation.

pub mod error;
pub mod health;
pub mod impact;
pub mod normalization;
pub mod pathfinder;

pub use impact::{AmmQuoteCalculator, OrderbookImpactCalculator};
pub use pathfinder::{LiquidityEdge, Pathfinder, PathfinderConfig, SwapPath};

/// Routing engine with integrated pathfinding and impact calculations
pub struct RoutingEngine {
    pathfinder: Pathfinder,
    amm_calculator: AmmQuoteCalculator,
    orderbook_calculator: OrderbookImpactCalculator,
}

impl RoutingEngine {
    /// Create a new routing engine instance with default config
    pub fn new() -> Self {
        Self::with_config(PathfinderConfig::default())
    }

    /// Create a new routing engine with custom config
    pub fn with_config(config: PathfinderConfig) -> Self {
        Self {
            pathfinder: Pathfinder::new(config),
            amm_calculator: AmmQuoteCalculator,
            orderbook_calculator: OrderbookImpactCalculator,
        }
    }

    /// Get reference to pathfinder
    pub fn pathfinder(&self) -> &Pathfinder {
        &self.pathfinder
    }

    /// Get reference to AMM calculator
    pub fn amm_calculator(&self) -> &AmmQuoteCalculator {
        &self.amm_calculator
    }

    /// Get reference to orderbook calculator
    pub fn orderbook_calculator(&self) -> &OrderbookImpactCalculator {
        &self.orderbook_calculator
    }
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}
