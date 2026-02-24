use crate::types::Route;
use soroban_sdk::{symbol_short, Address, Env, Symbol};

pub fn initialized(e: &Env, admin: Address, fee_rate: u32) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("init"));
    e.events().publish(topics, (admin, fee_rate));
}

pub fn admin_changed(e: &Env, old_admin: Address, new_admin: Address) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("adm_chg"));
    e.events().publish(topics, (old_admin, new_admin));
}

pub fn pool_registered(e: &Env, pool_address: Address) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("reg_pol"));
    e.events().publish(topics, pool_address);
}

pub fn paused(e: &Env) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("paused"));
    e.events().publish(topics, ());
}

pub fn unpaused(e: &Env) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("unpaused"));
    e.events().publish(topics, ());
}

pub fn swap_executed(
    e: &Env,
    sender: Address,
    amount_in: i128,
    amount_out: i128,
    fee: i128,
    route: Route,
) {
    let topics = (
        Symbol::new(e, "StellarRoute"),
        symbol_short!("swap"),
        sender,
    );
    e.events().publish(
        topics,
        (amount_in, amount_out, fee, route, e.ledger().sequence()),
    );
}

pub fn ttl_extended(e: &Env, pools_extended: u32, ledger: u32) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("ttl_ext"));
    e.events().publish(topics, (pools_extended, ledger));
}

pub fn ttl_warning(e: &Env, estimated_remaining: u64, threshold: u32) {
    let topics = (Symbol::new(e, "StellarRoute"), symbol_short!("ttl_wrn"));
    e.events().publish(topics, (estimated_remaining, threshold));
}
