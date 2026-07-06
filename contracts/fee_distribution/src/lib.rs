#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, token, symbol_short,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
    DistributionNonce,
    LastDistribution,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub treasury: Address,
    pub ai_agent_pool: Address,
    pub lp_pool: Address,
    pub treasury_bps: u32,
    pub ai_agent_bps: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DistributionRecord {
    pub nonce: u32,
    pub token: Address,
    pub from: Address,
    pub amount: i128,
    pub treasury_share: i128,
    pub ai_agent_share: i128,
    pub lp_share: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub treasury: Address,
    pub ai_agent_pool: Address,
    pub lp_pool: Address,
    pub treasury_bps: u32,
    pub ai_agent_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtCfgUpd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub treasury: Address,
    pub ai_agent_pool: Address,
    pub lp_pool: Address,
    pub treasury_bps: u32,
    pub ai_agent_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtSplit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub nonce: u32,
    pub token: Address,
    pub from: Address,
    pub amount: i128,
    pub treasury_share: i128,
    pub ai_agent_share: i128,
    pub lp_share: i128,
}

#[contract]
pub struct FeeDistributionContract;

#[contractimpl]
impl FeeDistributionContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        treasury: Address,
        ai_agent_pool: Address,
        lp_pool: Address,
        treasury_bps: u32,
        ai_agent_bps: u32,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        if treasury_bps + ai_agent_bps > 10_000 {
            panic!("Invalid basis points: sum exceeds 10000");
        }
        env.storage().instance().set(&DataKey::Config, &Config { admin: admin.clone(), treasury: treasury.clone(), ai_agent_pool: ai_agent_pool.clone(), lp_pool: lp_pool.clone(), treasury_bps, ai_agent_bps });
        env.storage().instance().set(&DataKey::DistributionNonce, &0u32);

        env.events().publish(
            (symbol_short!("fees"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                admin,
                treasury,
                ai_agent_pool,
                lp_pool,
                treasury_bps,
                ai_agent_bps,
            },
        );
    }

    pub fn update_config(env: Env, config: Config) {
        let current_config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        current_config.admin.require_auth();
        if config.treasury_bps + config.ai_agent_bps > 10_000 {
            panic!("Invalid basis points: sum exceeds 10000");
        }
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("fees"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: current_config.admin.clone(),
                admin: config.admin.clone(),
                treasury: config.treasury.clone(),
                ai_agent_pool: config.ai_agent_pool.clone(),
                lp_pool: config.lp_pool.clone(),
                treasury_bps: config.treasury_bps,
                ai_agent_bps: config.ai_agent_bps,
            },
        );
    }

    pub fn distribute(env: Env, token_addr: Address, from: Address, amount: i128) -> DistributionRecord {
        if amount <= 0 {
            panic!("amount must be positive");
        }
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        let treasury_share = amount.checked_mul(config.treasury_bps as i128).expect("Multiplication overflow").checked_div(10_000).expect("Division by zero");
        let ai_agent_share = amount.checked_mul(config.ai_agent_bps as i128).expect("Multiplication overflow").checked_div(10_000).expect("Division by zero");
        let lp_share = amount.checked_sub(treasury_share).expect("Subtraction underflow").checked_sub(ai_agent_share).expect("Subtraction underflow");
        let client = token::Client::new(&env, &token_addr);

        if treasury_share > 0 {
            client.transfer_from(&env.current_contract_address(), &from, &config.treasury, &treasury_share);
        }
        if ai_agent_share > 0 {
            client.transfer_from(&env.current_contract_address(), &from, &config.ai_agent_pool, &ai_agent_share);
        }
        if lp_share > 0 {
            client.transfer_from(&env.current_contract_address(), &from, &config.lp_pool, &lp_share);
        }

        let nonce = env.storage().instance().get::<DataKey, u32>(&DataKey::DistributionNonce).unwrap_or(0);
        let record = DistributionRecord { nonce: nonce + 1, token: token_addr.clone(), from: from.clone(), amount, treasury_share, ai_agent_share, lp_share };
        env.storage().instance().set(&DataKey::DistributionNonce, &(nonce + 1));
        env.storage().instance().set(&DataKey::LastDistribution, &record);
        env.events().publish(
            (symbol_short!("fees"), symbol_short!("split")),
            EvtSplit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: from.clone(),
                nonce: record.nonce,
                token: token_addr.clone(),
                from: from.clone(),
                amount,
                treasury_share,
                ai_agent_share,
                lp_share,
            },
        );
        record
    }

    pub fn last_distribution(env: Env) -> Option<DistributionRecord> {
        env.storage().instance().get(&DataKey::LastDistribution)
    }

    pub fn get_config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).expect("Not initialized")
    }
}

mod test;
