#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidationConfig {
    pub threshold_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
}

#[contractclient(name = "IntentMarketValidatorClient")]
pub trait IntentMarketValidatorTrait {
    fn initialize(env: Env, threshold_bps: u32);
    fn validate(env: Env, intent_value: i128, market_value: i128) -> bool;
    fn update_config(env: Env, config: ValidationConfig);
    fn get_config(env: Env) -> ValidationConfig;
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub threshold_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtCfgUpd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub threshold_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtDevAlert {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub market_value: i128,
    pub intent_value: i128,
    pub deviation_bps: i128,
}

#[contract]
pub struct IntentMarketValidatorContract;

#[contractimpl]
impl IntentMarketValidatorContract {
    pub fn initialize(env: Env, threshold_bps: u32) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        let config = ValidationConfig { threshold_bps };
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("intent"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: env.current_contract_address(),
                threshold_bps,
            },
        );
    }

    pub fn validate(env: Env, intent_value: i128, market_value: i128) -> bool {
        if intent_value <= 0 || market_value <= 0 {
            panic!("Invalid values");
        }

        let config: ValidationConfig = env.storage().instance().get(&DataKey::Config).expect("Not initialized");

        let diff = if market_value > intent_value {
            market_value - intent_value
        } else {
            intent_value - market_value
        };

        let deviation_bps = diff
            .checked_mul(10000)
            .expect("Deviation math overflow")
            .checked_div(intent_value)
            .expect("Deviation math division error");

        if deviation_bps > config.threshold_bps as i128 {
            env.events().publish(
                (symbol_short!("intent"), symbol_short!("dev_alert")),
                EvtDevAlert {
                    version: 1,
                    ledger: env.ledger().sequence(),
                    actor: env.current_contract_address(),
                    market_value,
                    intent_value,
                    deviation_bps,
                },
            );
            panic!("Intent vs Market: deviation exceeds threshold");
        }

        true
    }

    pub fn update_config(env: Env, config: ValidationConfig) {
        let _current: ValidationConfig = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("intent"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: env.current_contract_address(),
                threshold_bps: config.threshold_bps,
            },
        );
    }

    pub fn get_config(env: Env) -> ValidationConfig {
        env.storage().instance().get(&DataKey::Config).expect("Not initialized")
    }
}

mod test;
