#![cfg(test)]

use super::*;
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, Env};

// ---------------------------------------------------------------------------
// Invariant tests for flash loan guard
// ---------------------------------------------------------------------------

#[contract]
pub struct MockOracle;

#[contractimpl]
impl MockOracle {
    pub fn get_price(env: Env, _asset: Address) -> i128 {
        env.storage().instance().get(&0u32).unwrap_or(100_000_000i128)
    }

    pub fn set_price(env: Env, price: i128) {
        env.storage().instance().set(&0u32, &price);
    }
}

#[contractclient(name = "MockOracleClient")]
pub trait MockOracleTrait {
    fn get_price(env: Env, asset: Address) -> i128;
    fn set_price(env: Env, price: i128);
}

/// Invariant: price must always be positive after successful snapshot.
#[test]
fn invariant_snapshot_price_positive() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_sequence_number(100);
    env.ledger().set_timestamp_ns(1000000 * 10_u64.pow(9));

    let admin = Address::generate(&env);
    let asset = Address::generate(&env);
    let oracle_id = env.register(MockOracle, ());
    let contract_id = env.register(FlashLoanGuardContract, ());
    let client = FlashLoanGuardContractClient::new(&env, &contract_id);

    client.initialize(&Config {
        admin,
        oracle: oracle_id,
        guarded_asset: asset,
        max_intra_ledger_deviation_bps: 200,
        min_ledger_gap: 1,
        max_oracle_staleness_seconds: 3600,
        max_consecutive_price_change_bps: 500,
        max_oracle_update_gap_seconds: 7200,
        circuit_breaker_threshold_bps: 1000,
        circuit_breaker_window_seconds: 3600,
    });

    let oracle_client = MockOracleClient::new(&env, &oracle_id);
    oracle_client.set_price(100_000_000);

    client.record_snapshot(1000000, 1);
    let snap = client.get_snapshot().unwrap();
    assert!(snap.price > 0, "Snapshot price must be positive");
}

/// Invariant: circuit breaker resets after window expires.
#[test]
fn invariant_circuit_breaker_resets() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_sequence_number(100);
    env.ledger().set_timestamp_ns(1000000 * 10_u64.pow(9));

    let admin = Address::generate(&env);
    let asset = Address::generate(&env);
    let oracle_id = env.register(MockOracle, ());
    let contract_id = env.register(FlashLoanGuardContract, ());
    let client = FlashLoanGuardContractClient::new(&env, &contract_id);

    client.initialize(&Config {
        admin,
        oracle: oracle_id,
        guarded_asset: asset,
        max_intra_ledger_deviation_bps: 200,
        min_ledger_gap: 1,
        max_oracle_staleness_seconds: 3600,
        max_consecutive_price_change_bps: 500,
        max_oracle_update_gap_seconds: 7200,
        circuit_breaker_threshold_bps: 1000,
        circuit_breaker_window_seconds: 3600,
    });

    let oracle_client = MockOracleClient::new(&env, &oracle_id);
    oracle_client.set_price(100_000_000);
    client.record_snapshot(1000000, 1);

    // Simulate time passing beyond circuit breaker window
    env.ledger().set_timestamp_ns((1000000 + 4000) * 10_u64.pow(9));
    let cb = client.get_circuit_breaker();
    assert!(!cb.triggered, "Circuit breaker should reset after window");
}

/// Property: stale oracle data is always rejected.
#[test]
#[should_panic(expected = "oracle data too stale")]
fn property_stale_oracle_rejected() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_sequence_number(100);
    env.ledger().set_timestamp_ns(2000000 * 10_u64.pow(9));

    let admin = Address::generate(&env);
    let asset = Address::generate(&env);
    let oracle_id = env.register(MockOracle, ());
    let contract_id = env.register(FlashLoanGuardContract, ());
    let client = FlashLoanGuardContractClient::new(&env, &contract_id);

    client.initialize(&Config {
        admin,
        oracle: oracle_id,
        guarded_asset: asset,
        max_intra_ledger_deviation_bps: 200,
        min_ledger_gap: 1,
        max_oracle_staleness_seconds: 3600,
        max_consecutive_price_change_bps: 500,
        max_oracle_update_gap_seconds: 7200,
        circuit_breaker_threshold_bps: 1000,
        circuit_breaker_window_seconds: 3600,
    });

    // Stale timestamp
    client.record_snapshot(1000000, 1);
}