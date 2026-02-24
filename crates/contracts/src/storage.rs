use crate::types::Asset;
use soroban_sdk::{contracttype, Address, BytesN, Env, Vec};

#[contracttype]
pub enum StorageKey {
    Admin,
    FeeRate,
    FeeTo,
    Paused,
    SupportedPool(Address),
    PoolCount,
    SwapNonce(Address),
    // ── Persistent ─────────────────────────────────────────────────────
    TotalSwapVolume,
    // ── Instance — TTL tracking ────────────────────────────────────────
    PoolList,
    LastTtlExtension,
    // ── Temporary (auto-expiring) ──────────────────────────────────────
    PendingUpgrade,
    Commitment(BytesN<32>),
    RateLimit(Address),
}

// ── TTL Constants (in ledger sequences, ~5s per ledger) ──────────────────

pub const DAY_IN_LEDGERS: u32 = 17_280;

/// Instance storage: extend +30 days, threshold at 25% (~7 days)
pub const INSTANCE_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;
pub const INSTANCE_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;

/// Persistent pool keys: extend +90 days, threshold at 25% (~22 days)
pub const POOL_TTL_EXTEND_TO: u32 = 90 * DAY_IN_LEDGERS;
pub const POOL_TTL_THRESHOLD: u32 = 22 * DAY_IN_LEDGERS;

/// Persistent swap volume: extend +30 days, threshold at 25% (~7 days)
pub const VOLUME_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;
pub const VOLUME_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;

/// Persistent swap nonce: extend +30 days, threshold at 25% (~7 days)
pub const NONCE_TTL_EXTEND_TO: u32 = 30 * DAY_IN_LEDGERS;
pub const NONCE_TTL_THRESHOLD: u32 = 7 * DAY_IN_LEDGERS;

/// Temporary storage TTLs
pub const PENDING_UPGRADE_TTL: u32 = 6 * 720; // ~6 hours
pub const COMMITMENT_TTL: u32 = 720; // ~1 hour
pub const RATE_LIMIT_TTL: u32 = 120; // ~10 minutes

// ── TTL Extension Helpers ────────────────────────────────────────────────

/// Extend instance TTL after any write. Only extends if remaining < threshold.
pub fn extend_instance_ttl(e: &Env) {
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_TTL_THRESHOLD, INSTANCE_TTL_EXTEND_TO);
}

/// Extend a persistent storage key's TTL using the threshold pattern.
pub fn extend_persistent_ttl(e: &Env, key: &StorageKey, threshold: u32, extend_to: u32) {
    if e.storage().persistent().has(key) {
        e.storage()
            .persistent()
            .extend_ttl(key, threshold, extend_to);
    }
}

/// Extend a specific pool's persistent TTL (+90 days).
pub fn extend_pool_ttl(e: &Env, pool: &Address) {
    let key = StorageKey::SupportedPool(pool.clone());
    extend_persistent_ttl(e, &key, POOL_TTL_THRESHOLD, POOL_TTL_EXTEND_TO);
}

/// Extend the swap nonce TTL for a specific user (+30 days).
pub fn extend_nonce_ttl(e: &Env, address: &Address) {
    let key = StorageKey::SwapNonce(address.clone());
    extend_persistent_ttl(e, &key, NONCE_TTL_THRESHOLD, NONCE_TTL_EXTEND_TO);
}

/// Extend the total swap volume TTL (+30 days).
pub fn extend_volume_ttl(e: &Env) {
    extend_persistent_ttl(
        e,
        &StorageKey::TotalSwapVolume,
        VOLUME_TTL_THRESHOLD,
        VOLUME_TTL_EXTEND_TO,
    );
}

pub fn get_admin(e: &Env) -> Address {
    e.storage().instance().get(&StorageKey::Admin).unwrap()
}

pub fn set_admin(e: &Env, admin: &Address) {
    e.storage().instance().set(&StorageKey::Admin, admin);
}

pub fn get_fee_rate(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&StorageKey::FeeRate)
        .unwrap_or(0)
}

pub fn set_fee_rate(e: &Env, rate: u32) {
    e.storage().instance().set(&StorageKey::FeeRate, &rate);
}

pub fn get_fee_to(e: &Env) -> Address {
    e.storage().instance().get(&StorageKey::FeeTo).unwrap()
}

pub fn get_fee_to_optional(e: &Env) -> Option<Address> {
    e.storage().instance().get(&StorageKey::FeeTo)
}

pub fn get_pool_count(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&StorageKey::PoolCount)
        .unwrap_or(0)
}

pub fn set_pool_count(e: &Env, count: u32) {
    e.storage().instance().set(&StorageKey::PoolCount, &count);
}

pub fn get_paused(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&StorageKey::Paused)
        .unwrap_or(false)
}

pub fn is_initialized(e: &Env) -> bool {
    e.storage().instance().has(&StorageKey::Admin)
}

pub fn is_supported_pool(e: &Env, pool: Address) -> bool {
    e.storage()
        .persistent()
        .has(&StorageKey::SupportedPool(pool))
}

/// Get the list of all registered pool addresses (for TTL enumeration).
pub fn get_pool_list(e: &Env) -> Vec<Address> {
    e.storage()
        .instance()
        .get(&StorageKey::PoolList)
        .unwrap_or_else(|| Vec::new(e))
}

/// Add a pool address to the enumerable pool list.
pub fn add_to_pool_list(e: &Env, pool: &Address) {
    let mut list = get_pool_list(e);
    list.push_back(pool.clone());
    e.storage().instance().set(&StorageKey::PoolList, &list);
}

// ── Nonces ───────────────────────────────────────────────────────────────

pub fn get_nonce(e: &Env, address: Address) -> i128 {
    let key = StorageKey::SwapNonce(address);
    e.storage().persistent().get(&key).unwrap_or(0)
}

pub fn increment_nonce(e: &Env, address: Address) {
    let key = StorageKey::SwapNonce(address.clone());
    let current = get_nonce(e, address.clone());
    e.storage().persistent().set(&key, &(current + 1));
    extend_nonce_ttl(e, &address);
}

// ── Swap Volume ──────────────────────────────────────────────────────────

pub fn get_total_swap_volume(e: &Env) -> i128 {
    e.storage()
        .persistent()
        .get(&StorageKey::TotalSwapVolume)
        .unwrap_or(0)
}

pub fn add_swap_volume(e: &Env, amount: i128) {
    let current = get_total_swap_volume(e);
    e.storage()
        .persistent()
        .set(&StorageKey::TotalSwapVolume, &(current + amount));
    extend_volume_ttl(e);
}

// ── TTL Extension Tracking ───────────────────────────────────────────────

pub fn get_last_ttl_extension(e: &Env) -> u32 {
    e.storage()
        .instance()
        .get(&StorageKey::LastTtlExtension)
        .unwrap_or(0)
}

pub fn set_last_ttl_extension(e: &Env, ledger: u32) {
    e.storage()
        .instance()
        .set(&StorageKey::LastTtlExtension, &ledger);
}

// ── Token Transfer ───────────────────────────────────────────────────────

pub fn transfer_asset(e: &Env, asset: &Asset, from: &Address, to: &Address, amount: i128) {
    if let Asset::Soroban(address) = asset {
        let client = soroban_sdk::token::Client::new(e, address);
        client.transfer(from, to, &amount);
    }
}
