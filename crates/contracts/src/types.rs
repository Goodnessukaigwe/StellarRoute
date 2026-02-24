use soroban_sdk::{contracttype, Address, Env, Symbol, Vec};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Asset {
    Native,                  // XLM
    Issued(Address, Symbol), // (issuer, code)
    Soroban(Address),        // Soroban token contract address
}

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PoolType {
    Sdex,
    AmmConstProd,
    AmmStable,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct RouteHop {
    pub source: Asset,
    pub destination: Asset,
    pub pool: Address,
    pub pool_type: PoolType,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub hops: Vec<RouteHop>,
    pub estimated_output: i128,
    pub min_output: i128,
    pub expires_at: u64,
}

#[contracttype]
pub struct SwapParams {
    pub route: Route,
    pub amount_in: i128,
    pub min_amount_out: i128,
    pub recipient: Address,
    pub deadline: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct QuoteResult {
    pub expected_output: i128,
    pub price_impact_bps: u32, // 100 = 1%
    pub fee_amount: i128,
    pub route: Route,
    pub valid_until: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct SwapResult {
    pub amount_in: i128,
    pub amount_out: i128,
    pub route: Route,
    pub executed_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct TTLStatus {
    pub instance_ttl_remaining: u64,
    pub pools_min_ttl: u64,
    pub needs_extension: bool,
    pub last_extended_ledger: u32,
}

// Interface for AMM pools (SEP-like standard)
pub trait LiquidityPoolInterface {
    fn get_rsrvs(e: Env) -> (i128, i128);
    fn swap_out(e: Env, in_asset: Asset, out_asset: Asset, amount_in: i128) -> i128;
}
