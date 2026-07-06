#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contractclient, symbol_short,
    Address, Env, token,
};

const POSITION_TTL_LEDGERS: u32 = 6_048_000;
const MAX_BPS: i128 = 10_000;

#[contractclient(name = "PriceOracleClient")]
pub trait PriceOracleTrait {
    fn get_price(env: Env, asset: Address) -> i128;
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    Position(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub oracle: Address,
    pub collateral_token: Address,
    pub debt_token: Address,
    pub min_health_factor: i128,
    pub liquidation_bonus_bps: i128,
    pub ltv_bps: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Position {
    pub collateral_amount: i128,
    pub debt_amount: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub oracle: Address,
    pub collateral_token: Address,
    pub debt_token: Address,
    pub min_health_factor: i128,
    pub liquidation_bonus_bps: i128,
    pub ltv_bps: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtCfgUpd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub oracle: Address,
    pub collateral_token: Address,
    pub debt_token: Address,
    pub min_health_factor: i128,
    pub liquidation_bonus_bps: i128,
    pub ltv_bps: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtDeposit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub borrower: Address,
    pub collateral_amount: i128,
    pub borrow_amount: i128,
    pub health_factor: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtLiquidate {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub liquidator: Address,
    pub borrower: Address,
    pub repay_amount: i128,
    pub collateral_seized: i128,
    pub health_factor: i128,
}

#[contract]
pub struct LendingLiquidationContract;

#[contractimpl]
impl LendingLiquidationContract {
    pub fn initialize(env: Env, config: Config) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        Self::validate_config(&config);
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("lend"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: config.admin.clone(),
                admin: config.admin.clone(),
                oracle: config.oracle.clone(),
                collateral_token: config.collateral_token.clone(),
                debt_token: config.debt_token.clone(),
                min_health_factor: config.min_health_factor,
                liquidation_bonus_bps: config.liquidation_bonus_bps,
                ltv_bps: config.ltv_bps,
            },
        );
    }

    pub fn update_config(env: Env, config: Config) {
        let current_config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        current_config.admin.require_auth();
        Self::validate_config(&config);
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("lend"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: current_config.admin.clone(),
                admin: config.admin.clone(),
                oracle: config.oracle.clone(),
                collateral_token: config.collateral_token.clone(),
                debt_token: config.debt_token.clone(),
                min_health_factor: config.min_health_factor,
                liquidation_bonus_bps: config.liquidation_bonus_bps,
                ltv_bps: config.ltv_bps,
            },
        );
    }
        Self::validate_config(&config);
        env.storage().instance().set(&DataKey::Config, &config);
    }

    pub fn update_config(env: Env, config: Config) {
        let current: Config = env.storage().instance().get(&DataKey::Config).expect("not initialized");
        current.admin.require_auth();
        Self::validate_config(&config);
        env.storage().instance().set(&DataKey::Config, &config);
    }

    pub fn deposit_and_borrow(
        env: Env,
        borrower: Address,
        collateral_amount: i128,
        borrow_amount: i128,
    ) {
        borrower.require_auth();
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("not initialized");

        if collateral_amount <= 0 || borrow_amount <= 0 {
            panic!("amounts must be positive");
        }

        let mut pos: Position = env
            .storage()
            .persistent()
            .get(&DataKey::Position(borrower.clone()))
            .unwrap_or(Position { collateral_amount: 0, debt_amount: 0 });

        let new_collateral = pos.collateral_amount.checked_add(collateral_amount).expect("overflow");
        let new_debt = pos.debt_amount.checked_add(borrow_amount).expect("overflow");

        let oracle = PriceOracleClient::new(&env, &config.oracle);
        let col_price = oracle.get_price(&config.collateral_token);
        let debt_price = oracle.get_price(&config.debt_token);

        Self::validate_prices(col_price, debt_price);

        let new_pos = Position {
            collateral_amount: new_collateral,
            debt_amount: new_debt,
        };

        let hf = Self::compute_health_factor(&new_pos, col_price, debt_price, config.ltv_bps);
        if hf < config.min_health_factor {
            panic!("borrow exceeds LTV");
        }

        let col_token = token::Client::new(&env, &config.collateral_token);
        col_token.transfer(&borrower, &env.current_contract_address(), &collateral_amount);

        let debt_token = token::Client::new(&env, &config.debt_token);
        debt_token.transfer(&env.current_contract_address(), &borrower, &borrow_amount);

        pos.collateral_amount = new_collateral;
        pos.debt_amount = new_debt;

        env.storage().persistent().set_with_ttl(&DataKey::Position(borrower), &pos, POSITION_TTL_LEDGERS);

        env.events().publish(
            (symbol_short!("lend"), symbol_short!("deposit")),
            EvtDeposit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: borrower.clone(),
                borrower: borrower.clone(),
                collateral_amount: new_collateral,
                borrow_amount: new_debt,
                health_factor: hf,
            },
        );
    }

    pub fn liquidate(env: Env, liquidator: Address, borrower: Address, repay_amount: i128) {
        liquidator.require_auth();
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("not initialized");

        if repay_amount <= 0 {
            panic!("repay amount must be positive");
        }

        let mut pos: Position = env
            .storage()
            .persistent()
            .get(&DataKey::Position(borrower.clone()))
            .expect("position not found");

        if pos.debt_amount == 0 {
            panic!("no debt to liquidate");
        }

        let oracle = PriceOracleClient::new(&env, &config.oracle);
        let col_price = oracle.get_price(&config.collateral_token);
        let debt_price = oracle.get_price(&config.debt_token);

        Self::validate_prices(col_price, debt_price);

        let hf = Self::compute_health_factor(&pos, col_price, debt_price, config.ltv_bps);
        if hf >= config.min_health_factor {
            panic!("position is healthy, cannot liquidate");
        }

        let actual_repay = if repay_amount > pos.debt_amount {
            pos.debt_amount
        } else {
            repay_amount
        };

        let collateral_seized = Self::calculate_collateral_to_seize(
            actual_repay,
            pos.collateral_amount,
            col_price,
            debt_price,
            config.liquidation_bonus_bps,
        );

        let debt_token = token::Client::new(&env, &config.debt_token);
        debt_token.transfer(&liquidator, &env.current_contract_address(), &actual_repay);

        let col_token = token::Client::new(&env, &config.collateral_token);
        col_token.transfer(&env.current_contract_address(), &liquidator, &collateral_seized);

        pos.debt_amount = pos.debt_amount.checked_sub(actual_repay).expect("underflow");
        pos.collateral_amount = pos.collateral_amount.checked_sub(collateral_seized).expect("underflow");
        
        env.storage().persistent().set_with_ttl(&DataKey::Position(borrower.clone()), &pos, POSITION_TTL_LEDGERS);

        env.events().publish(
            (symbol_short!("lend"), symbol_short!("liquidate")),
            EvtLiquidate {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: liquidator.clone(),
                liquidator,
                borrower: borrower.clone(),
                repay_amount: actual_repay,
                collateral_seized,
                health_factor: hf,
            },
        );
    }

    pub fn health_factor(env: Env, borrower: Address) -> i128 {
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("not initialized");
        let pos: Position = env
            .storage()
            .persistent()
            .get(&DataKey::Position(borrower))
            .unwrap_or(Position { collateral_amount: 0, debt_amount: 0 });

        if pos.debt_amount == 0 {
            return i128::MAX;
        }

        let oracle = PriceOracleClient::new(&env, &config.oracle);
        let col_price = oracle.get_price(&config.collateral_token);
        let debt_price = oracle.get_price(&config.debt_token);

        Self::validate_prices(col_price, debt_price);

        Self::compute_health_factor(&pos, col_price, debt_price, config.ltv_bps)
    }

    pub fn get_position(env: Env, borrower: Address) -> Option<Position> {
        env.storage().persistent().get(&DataKey::Position(borrower))
    }

    fn validate_config(config: &Config) {
        assert!(config.min_health_factor > 0, "invalid min health factor");
        assert!(config.liquidation_bonus_bps >= 0 && config.liquidation_bonus_bps <= MAX_BPS, "invalid bonus bps");
        assert!(config.ltv_bps > 0 && config.ltv_bps <= MAX_BPS, "invalid ltv bps");
    }

    fn validate_prices(col_price: i128, debt_price: i128) {
        assert!(col_price > 0, "invalid collateral price");
        assert!(debt_price > 0, "invalid debt price");
    }

    fn calculate_collateral_to_seize(
        actual_repay: i128,
        available_collateral: i128,
        col_price: i128,
        debt_price: i128,
        bonus_bps: i128,
    ) -> i128 {
        let repay_value = actual_repay
            .checked_mul(debt_price).expect("overflow")
            .checked_div(100_000_000).expect("div zero");

        let collateral_seized = repay_value
            .checked_mul(MAX_BPS + bonus_bps).expect("overflow")
            .checked_div(MAX_BPS).expect("div zero")
            .checked_mul(100_000_000).expect("overflow")
            .checked_div(col_price).expect("div zero");

        if collateral_seized > available_collateral {
            available_collateral
        } else {
            collateral_seized
        }
    }

    fn compute_health_factor(
        pos: &Position,
        col_price: i128,
        debt_price: i128,
        ltv_bps: i128,
    ) -> i128 {
        if pos.debt_amount == 0 {
            return i128::MAX;
        }
        pos.collateral_amount
            .checked_mul(col_price).expect("overflow")
            .checked_mul(ltv_bps).expect("overflow")
            .checked_div(
                pos.debt_amount
                    .checked_mul(debt_price).expect("overflow")
            ).expect("div zero")
    }
}

mod test;
