#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, Address, Env, token, symbol_short,
};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RelayerStatus {
    Active,
    UnstakeRequested,
    InDispute,
    Slashed,
    Withdrawn,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RelayerInfo {
    pub stake_amount: i128,
    pub status: RelayerStatus,
    pub unstake_requested_at: u64,
    pub dispute_count: u32,
    pub last_transition_at: u64,
}

#[contracttype]
pub enum DataKey {
    Config,
    Relayer(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub staking_token: Address,
    pub treasury: Address,
    pub slashing_bps: u32,
    pub unbonding_period: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub staking_token: Address,
    pub treasury: Address,
    pub slashing_bps: u32,
    pub unbonding_period: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtStake {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub relayer: Address,
    pub stake_amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtUnstake {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub relayer: Address,
    pub requested_at: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtDispute {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub relayer: Address,
    pub dispute_count: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtSlashed {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub relayer: Address,
    pub slash_amount: i128,
    pub new_stake: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtWithdraw {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub relayer: Address,
    pub withdrawn_amount: i128,
}

#[contract]
pub struct RelayerSlashingContract;

#[contractimpl]
impl RelayerSlashingContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        staking_token: Address,
        treasury: Address,
        slashing_bps: u32,
        unbonding_period: u64,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&DataKey::Config, &Config {
            admin: admin.clone(),
            staking_token: staking_token.clone(),
            treasury: treasury.clone(),
            slashing_bps,
            unbonding_period,
        });

        env.events().publish(
            (symbol_short!("relayer"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin,
                admin: admin.clone(),
                staking_token,
                treasury,
                slashing_bps,
                unbonding_period,
            },
        );
    }

    pub fn register_relayer(env: Env, relayer: Address, amount: i128) {
        relayer.require_auth();
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        let token_client = token::Client::new(&env, &config.staking_token);
        token_client.transfer(&relayer, &env.current_contract_address(), &amount);

        let now = env.ledger().timestamp();
        let mut info = env.storage().persistent().get(&DataKey::Relayer(relayer.clone())).unwrap_or(RelayerInfo {
            stake_amount: 0,
            status: RelayerStatus::Active,
            unstake_requested_at: 0,
            dispute_count: 0,
            last_transition_at: now,
        });

        info.status = RelayerStatus::Active;
        info.stake_amount += amount;
        info.last_transition_at = now;
        env.storage().persistent().set(&DataKey::Relayer(relayer.clone()), &info);
        env.events().publish(
            (symbol_short!("relayer"), symbol_short!("stake")),
            EvtStake {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: relayer.clone(),
                relayer,
                stake_amount: info.stake_amount,
            },
        );
    }

    pub fn request_unstake(env: Env, relayer: Address) {
        relayer.require_auth();
        let mut info: RelayerInfo = env.storage().persistent().get(&DataKey::Relayer(relayer.clone())).expect("Relayer not found");
        info.status = RelayerStatus::UnstakeRequested;
        info.unstake_requested_at = env.ledger().timestamp();
        info.last_transition_at = info.unstake_requested_at;
        env.storage().persistent().set(&DataKey::Relayer(relayer.clone()), &info);
        env.events().publish(
            (symbol_short!("relayer"), symbol_short!("unstake")),
            EvtUnstake {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: relayer.clone(),
                relayer,
                requested_at: info.unstake_requested_at,
            },
        );
    }

    pub fn dispute_relayer(env: Env, relayer: Address) {
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        config.admin.require_auth();
        let mut info: RelayerInfo = env.storage().persistent().get(&DataKey::Relayer(relayer.clone())).expect("Relayer not found");
        info.status = RelayerStatus::InDispute;
        info.dispute_count = info.dispute_count.saturating_add(1);
        info.last_transition_at = env.ledger().timestamp();
        env.storage().persistent().set(&DataKey::Relayer(relayer.clone()), &info);
        env.events().publish(
            (symbol_short!("relayer"), symbol_short!("dispute")),
            EvtDispute {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: config.admin.clone(),
                relayer,
                dispute_count: info.dispute_count,
            },
        );
    }

    pub fn slash_relayer(env: Env, relayer: Address) {
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        config.admin.require_auth();
        let mut info: RelayerInfo = env.storage().persistent().get(&DataKey::Relayer(relayer.clone())).expect("Relayer not found");
        if info.status == RelayerStatus::Slashed {
            return;
        }

        let slash_amount = (info.stake_amount * config.slashing_bps as i128) / 10_000;
        info.stake_amount = info.stake_amount.checked_sub(slash_amount).expect("Underflow");
        info.status = RelayerStatus::Slashed;
        info.last_transition_at = env.ledger().timestamp();
        let token_client = token::Client::new(&env, &config.staking_token);
        token_client.transfer(&env.current_contract_address(), &config.treasury, &slash_amount);
        env.storage().persistent().set(&DataKey::Relayer(relayer.clone()), &info);
        env.events().publish(
            (symbol_short!("relayer"), symbol_short!("slashed")),
            EvtSlashed {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: config.admin.clone(),
                relayer,
                slash_amount,
                new_stake: info.stake_amount,
            },
        );
    }

    pub fn withdraw_stake(env: Env, relayer: Address) {
        relayer.require_auth();
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        let info: RelayerInfo = env.storage().persistent().get(&DataKey::Relayer(relayer.clone())).expect("Relayer not found");

        if info.status == RelayerStatus::Slashed {
            panic!("slashed relayers cannot withdraw");
        }
        if info.status != RelayerStatus::UnstakeRequested {
            panic!("Unstake not requested");
        }
        if env.ledger().timestamp() < info.unstake_requested_at + config.unbonding_period {
            panic!("Unbonding period not met");
        }

        let token_client = token::Client::new(&env, &config.staking_token);
        token_client.transfer(&env.current_contract_address(), &relayer, &info.stake_amount);
        let withdrawn = RelayerInfo { status: RelayerStatus::Withdrawn, ..info };
        env.storage().persistent().set(&DataKey::Relayer(relayer.clone()), &withdrawn);
        env.events().publish(
            (symbol_short!("relayer"), symbol_short!("withdraw")),
            EvtWithdraw {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: relayer.clone(),
                relayer,
                withdrawn_amount: withdrawn.stake_amount,
            },
        );
    }

    pub fn get_relayer_info(env: Env, relayer: Address) -> Option<RelayerInfo> {
        env.storage().persistent().get(&DataKey::Relayer(relayer))
    }
}

mod test;
