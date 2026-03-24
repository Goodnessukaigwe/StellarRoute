//! StellarRoute Routing Engine
//!
//! Provides pathfinding algorithms for optimal swap routing across SDEX and Soroban AMM pools.

pub mod error;
pub mod normalization;
pub mod pathfinder;

/// Routing engine
pub struct RoutingEngine {
    // TODO: Implement routing engine
}

impl RoutingEngine {
    /// Create a new routing engine instance
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for RoutingEngine {
    fn default() -> Self {
        Self::new()
    }
}
