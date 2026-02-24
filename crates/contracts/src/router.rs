use crate::errors::ContractError;
use crate::events;
use crate::storage::{
    self, extend_instance_ttl, get_fee_rate, get_fee_to, increment_nonce, is_supported_pool,
    transfer_asset, StorageKey, INSTANCE_TTL_EXTEND_TO, INSTANCE_TTL_THRESHOLD, POOL_TTL_EXTEND_TO,
    POOL_TTL_THRESHOLD,
};
use crate::types::{QuoteResult, Route, SwapParams, SwapResult, TTLStatus};
use soroban_sdk::{contract, contractimpl, symbol_short, vec, Address, Env, IntoVal, Symbol};

const CONTRACT_VERSION: u32 = 1;

#[contract]
pub struct StellarRoute;

#[contractimpl]
impl StellarRoute {
    pub fn initialize(
        e: Env,
        admin: Address,
        fee_rate: u32,
        fee_to: Address,
    ) -> Result<(), ContractError> {
        if e.storage().instance().has(&StorageKey::Admin) {
            return Err(ContractError::AlreadyInitialized);
        }
        if fee_rate > 1000 {
            return Err(ContractError::InvalidAmount);
        }

        e.storage().instance().set(&StorageKey::Admin, &admin);
        e.storage().instance().set(&StorageKey::FeeRate, &fee_rate);
        e.storage().instance().set(&StorageKey::FeeTo, &fee_to);
        e.storage().instance().set(&StorageKey::Paused, &false);

        storage::set_last_ttl_extension(&e, e.ledger().sequence());

        events::initialized(&e, admin, fee_rate);
        extend_instance_ttl(&e);
        Ok(())
    }

    pub fn set_admin(e: Env, new_admin: Address) -> Result<(), ContractError> {
        let admin = storage::get_admin(&e);
        admin.require_auth();

        e.storage().instance().set(&StorageKey::Admin, &new_admin);
        events::admin_changed(&e, admin, new_admin);
        extend_instance_ttl(&e);
        Ok(())
    }

    pub fn register_pool(e: Env, pool: Address) -> Result<(), ContractError> {
        storage::get_admin(&e).require_auth();

        let key = StorageKey::SupportedPool(pool.clone());
        if e.storage().persistent().has(&key) {
            return Err(ContractError::PoolNotSupported);
        }

        e.storage().persistent().set(&key, &true);
        storage::extend_pool_ttl(&e, &pool);

        let new_count = storage::get_pool_count(&e) + 1;
        storage::set_pool_count(&e, new_count);
        storage::add_to_pool_list(&e, &pool);

        events::pool_registered(&e, pool);
        extend_instance_ttl(&e);
        Ok(())
    }

    pub fn pause(e: Env) -> Result<(), ContractError> {
        storage::get_admin(&e).require_auth();
        e.storage().instance().set(&StorageKey::Paused, &true);
        events::paused(&e);
        extend_instance_ttl(&e);
        Ok(())
    }

    pub fn unpause(e: Env) -> Result<(), ContractError> {
        storage::get_admin(&e).require_auth();
        e.storage().instance().set(&StorageKey::Paused, &false);
        events::unpaused(&e);
        extend_instance_ttl(&e);
        Ok(())
    }

    // --- Read-only getters for deployment verification and monitoring ---

    pub fn version(_e: Env) -> u32 {
        CONTRACT_VERSION
    }

    pub fn get_admin(e: Env) -> Result<Address, ContractError> {
        if !storage::is_initialized(&e) {
            return Err(ContractError::NotInitialized);
        }
        Ok(storage::get_admin(&e))
    }

    pub fn get_fee_rate_value(e: Env) -> u32 {
        storage::get_fee_rate(&e)
    }

    pub fn get_fee_to_address(e: Env) -> Result<Address, ContractError> {
        storage::get_fee_to_optional(&e).ok_or(ContractError::NotInitialized)
    }

    pub fn is_paused(e: Env) -> bool {
        storage::get_paused(&e)
    }

    pub fn get_pool_count(e: Env) -> u32 {
        storage::get_pool_count(&e)
    }

    pub fn is_pool_registered(e: Env, pool: Address) -> bool {
        storage::is_supported_pool(&e, pool)
    }

    // --- Core operations ---

    pub fn require_not_paused(e: &Env) -> Result<(), ContractError> {
        let paused: bool = e
            .storage()
            .instance()
            .get(&StorageKey::Paused)
            .unwrap_or(false);
        if paused {
            return Err(ContractError::Paused);
        }
        Ok(())
    }

    /// Public entry point for users to get quotes
    pub fn get_quote(e: Env, amount_in: i128, route: Route) -> Result<QuoteResult, ContractError> {
        if amount_in <= 0 || route.hops.is_empty() || route.hops.len() > 4 {
            return Err(ContractError::InvalidRoute);
        }

        let mut current_amount = amount_in;
        let mut total_impact_bps: u32 = 0;

        for i in 0..route.hops.len() {
            let hop = route.hops.get(i).unwrap();
            if !is_supported_pool(&e, hop.pool.clone()) {
                return Err(ContractError::PoolNotSupported);
            }

            let call_result = e.try_invoke_contract::<i128, soroban_sdk::Error>(
                &hop.pool,
                &Symbol::new(&e, "adapter_quote"),
                vec![
                    &e,
                    hop.source.into_val(&e),
                    hop.destination.into_val(&e),
                    current_amount.into_val(&e),
                ],
            );

            current_amount = match call_result {
                Ok(Ok(val)) => val,
                _ => return Err(ContractError::PoolCallFailed),
            };
            total_impact_bps += 5;
        }

        let fee_rate = get_fee_rate(&e);
        let fee_amount = (current_amount * fee_rate as i128) / 10000;
        let final_output = current_amount - fee_amount;

        Ok(QuoteResult {
            expected_output: final_output,
            price_impact_bps: total_impact_bps,
            fee_amount,
            route: route.clone(),
            valid_until: (e.ledger().sequence() + 120) as u64,
        })
    }

    pub fn execute_swap(
        e: Env,
        sender: Address,
        params: SwapParams,
    ) -> Result<SwapResult, ContractError> {
        sender.require_auth();
        StellarRoute::require_not_paused(&e)?;

        if e.ledger().sequence() as u64 > params.deadline {
            return Err(ContractError::DeadlineExceeded);
        }

        if params.route.hops.is_empty() || params.route.hops.len() > 4 {
            return Err(ContractError::InvalidRoute);
        }

        let mut current_input_amount = params.amount_in;

        let first_hop = params.route.hops.get(0).unwrap();
        transfer_asset(
            &e,
            &first_hop.source,
            &sender,
            &first_hop.pool,
            params.amount_in,
        );

        for i in 0..params.route.hops.len() {
            let hop = params.route.hops.get(i).unwrap();

            if !is_supported_pool(&e, hop.pool.clone()) {
                return Err(ContractError::PoolNotSupported);
            }

            let call_result = e.try_invoke_contract::<i128, soroban_sdk::Error>(
                &hop.pool,
                &symbol_short!("swap"),
                vec![
                    &e,
                    hop.source.into_val(&e),
                    hop.destination.into_val(&e),
                    current_input_amount.into_val(&e),
                    0_i128.into_val(&e),
                ],
            );

            current_input_amount = match call_result {
                Ok(Ok(val)) => val,
                _ => return Err(ContractError::PoolCallFailed),
            };
        }

        let fee_rate = get_fee_rate(&e);
        let fee_amount = (current_input_amount * fee_rate as i128) / 10000;
        let final_output = current_input_amount - fee_amount;

        if final_output < params.min_amount_out {
            return Err(ContractError::SlippageExceeded);
        }

        let last_hop = params.route.hops.get(params.route.hops.len() - 1).unwrap();

        transfer_asset(
            &e,
            &last_hop.destination,
            &e.current_contract_address(),
            &params.recipient,
            final_output,
        );

        transfer_asset(
            &e,
            &last_hop.destination,
            &e.current_contract_address(),
            &get_fee_to(&e),
            fee_amount,
        );

        increment_nonce(&e, sender.clone());
        storage::add_swap_volume(&e, params.amount_in);

        // Extend TTLs for pools used in this route
        for i in 0..params.route.hops.len() {
            let hop = params.route.hops.get(i).unwrap();
            storage::extend_pool_ttl(&e, &hop.pool);
        }
        extend_instance_ttl(&e);

        // Check TTL health and emit warning if needed
        Self::check_ttl_health(&e);

        events::swap_executed(
            &e,
            sender,
            params.amount_in,
            final_output,
            fee_amount,
            params.route.clone(),
        );

        Ok(SwapResult {
            amount_in: params.amount_in,
            amount_out: final_output,
            route: params.route,
            executed_at: e.ledger().sequence() as u64,
        })
    }

    // --- TTL Management ---

    /// Public function anyone can call to extend all storage TTLs.
    /// No authorization required — keeping the contract alive is a public good.
    pub fn extend_storage_ttl(e: Env) {
        // Extend instance TTL (Admin, FeeRate, FeeTo, Paused, PoolCount, PoolList)
        extend_instance_ttl(&e);

        // Extend all registered pool TTLs
        let pool_list = storage::get_pool_list(&e);
        let pools_extended = pool_list.len();
        for i in 0..pool_list.len() {
            let pool = pool_list.get(i).unwrap();
            storage::extend_pool_ttl(&e, &pool);
        }

        // Extend TotalSwapVolume TTL
        storage::extend_volume_ttl(&e);

        // Record when this extension was performed
        storage::set_last_ttl_extension(&e, e.ledger().sequence());

        events::ttl_extended(&e, pools_extended, e.ledger().sequence());
    }

    /// Returns estimated TTL status for monitoring. Values are estimates
    /// based on when extend_storage_ttl was last called.
    pub fn get_ttl_status(e: Env) -> TTLStatus {
        let current_ledger = e.ledger().sequence();
        let last_extended = storage::get_last_ttl_extension(&e);

        let elapsed = current_ledger.saturating_sub(last_extended);

        let instance_remaining = INSTANCE_TTL_EXTEND_TO.saturating_sub(elapsed) as u64;
        let pools_remaining = POOL_TTL_EXTEND_TO.saturating_sub(elapsed) as u64;

        let needs_extension = instance_remaining < INSTANCE_TTL_THRESHOLD as u64
            || pools_remaining < POOL_TTL_THRESHOLD as u64;

        TTLStatus {
            instance_ttl_remaining: instance_remaining,
            pools_min_ttl: pools_remaining,
            needs_extension,
            last_extended_ledger: last_extended,
        }
    }

    /// Returns the total swap volume tracked by the contract.
    pub fn get_total_swap_volume(e: Env) -> i128 {
        storage::get_total_swap_volume(&e)
    }

    /// Internal: check TTL health and emit warning if below threshold.
    fn check_ttl_health(e: &Env) {
        let last_extended = storage::get_last_ttl_extension(e);
        if last_extended == 0 {
            return;
        }

        let elapsed = e.ledger().sequence().saturating_sub(last_extended);
        let pools_remaining = POOL_TTL_EXTEND_TO.saturating_sub(elapsed);

        if pools_remaining < POOL_TTL_THRESHOLD {
            events::ttl_warning(e, pools_remaining as u64, POOL_TTL_THRESHOLD);
        }
    }
}
