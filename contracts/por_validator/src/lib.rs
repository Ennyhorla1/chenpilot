#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contractclient, Address, Env, symbol_short};

const DEFAULT_MAX_STALE_LEDERS: u32 = 10_000;
const DEFAULT_PROOF_CADENCE_LEDGERS: u32 = 1_000;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReserveData {
    pub balance: i128,
    pub circulating_supply: i128,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProofRecord {
    pub reserve_data: ReserveData,
    pub is_valid: bool,
    pub verified_ledger: u32,
    pub valid_until_ledger: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VaultSafetyStatus {
    pub is_safe: bool,
    pub proof_is_fresh: bool,
    pub proof_is_valid: bool,
    pub verified_ledger: u32,
    pub valid_until_ledger: u32,
}

#[contractclient(name = "OracleClient")]
pub trait OracleTrait {
    fn get_reserve_data(env: Env) -> ReserveData;
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
    CurrentProof,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub wbtc_token: Address,
    pub oracle: Address,
    pub tolerance_bps: u32,
    pub proof_cadence_ledgers: u32,
    pub max_stale_ledgers: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub wbtc_token: Address,
    pub oracle: Address,
    pub tolerance_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtCfgUpd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub oracle: Address,
    pub tolerance_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtSafetyCfg {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub proof_cadence_ledgers: u32,
    pub max_stale_ledgers: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtProof {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub is_valid: bool,
    pub balance: i128,
    pub circulating_supply: i128,
    pub verified_ledger: u32,
    pub valid_until_ledger: u32,
}

#[contract]
pub struct PoRValidatorContract;

#[contractimpl]
impl PoRValidatorContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        wbtc_token: Address,
        oracle: Address,
        tolerance_bps: u32,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        let config = Config {
            admin: admin.clone(),
            wbtc_token: wbtc_token.clone(),
            oracle: oracle.clone(),
            tolerance_bps,
            proof_cadence_ledgers: DEFAULT_PROOF_CADENCE_LEDGERS,
            max_stale_ledgers: DEFAULT_MAX_STALE_LEDERS,
        };
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("por"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                admin,
                wbtc_token,
                oracle,
                tolerance_bps,
            },
        );
    }

    pub fn update_config(env: Env, config: Config) {
        let current_config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        current_config.admin.require_auth();
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("por"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: current_config.admin.clone(),
                admin: config.admin.clone(),
                oracle: config.oracle.clone(),
                tolerance_bps: config.tolerance_bps,
            },
        );
    }

    pub fn set_safety_policy(env: Env, proof_cadence_ledgers: u32, max_stale_ledgers: u32) {
        let mut config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        config.admin.require_auth();
        config.proof_cadence_ledgers = proof_cadence_ledgers;
        config.max_stale_ledgers = max_stale_ledgers;
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("por"), symbol_short!("safety_cfg")),
            EvtSafetyCfg {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: config.admin.clone(),
                proof_cadence_ledgers,
                max_stale_ledgers,
            },
        );
    }

    pub fn verify_reserves(env: Env) -> ProofRecord {
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        let current_ledger = env.ledger().sequence();
        let oracle_client = OracleClient::new(&env, &config.oracle);
        let reserve_data = oracle_client.get_reserve_data();

        let allowed_supply = reserve_data
            .balance
            .checked_mul((10_000 + config.tolerance_bps) as i128)
            .expect("Multiplication overflow")
            .checked_div(10_000)
            .expect("Division error");
        let is_valid = reserve_data.circulating_supply <= allowed_supply;
        let valid_until_ledger = current_ledger.saturating_add(config.max_stale_ledgers);

        let proof = ProofRecord {
            reserve_data: reserve_data.clone(),
            is_valid,
            verified_ledger: current_ledger,
            valid_until_ledger,
        };
        env.storage().instance().set(&DataKey::CurrentProof, &proof);

        env.events().publish(
            (symbol_short!("por"), symbol_short!("proof")),
            EvtProof {
                version: 1,
                ledger: current_ledger,
                actor: config.admin.clone(),
                is_valid,
                balance: reserve_data.balance,
                circulating_supply: reserve_data.circulating_supply,
                verified_ledger: current_ledger,
                valid_until_ledger,
            },
        );

        proof
    }

    pub fn get_current_proof(env: Env) -> Option<ProofRecord> {
        env.storage().instance().get(&DataKey::CurrentProof)
    }

    pub fn is_valid(env: Env) -> bool {
        Self::vault_safety_status(env).is_safe
    }

    pub fn vault_safety_status(env: Env) -> VaultSafetyStatus {
        let current_ledger = env.ledger().sequence();
        let proof: Option<ProofRecord> = env.storage().instance().get(&DataKey::CurrentProof);

        if let Some(proof) = proof {
            let proof_is_fresh = current_ledger <= proof.valid_until_ledger;
            let is_safe = proof_is_fresh && proof.is_valid;
            VaultSafetyStatus {
                is_safe,
                proof_is_fresh,
                proof_is_valid: proof.is_valid,
                verified_ledger: proof.verified_ledger,
                valid_until_ledger: proof.valid_until_ledger,
            }
        } else {
            VaultSafetyStatus {
                is_safe: false,
                proof_is_fresh: false,
                proof_is_valid: false,
                verified_ledger: 0,
                valid_until_ledger: 0,
            }
        }
    }

    pub fn get_config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).expect("Not initialized")
    }
}

mod test;
