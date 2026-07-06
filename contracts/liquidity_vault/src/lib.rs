#![no_std]
use soroban_sdk::{contract, contractimpl, contracttype, contractclient, Address, Env, symbol_short};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PriceData {
    pub price: i128,
    pub decimals: u32,
    pub timestamp: u64,
}

#[contractclient(name = "PriceOracleClient")]
pub trait PriceOracleTrait {
    fn get_price(env: Env, asset: Address) -> Option<PriceData>;
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DataKey {
    Config,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub oracle: Address,
    pub threshold_bps: u32, // Deviation threshold in Basis Points (100 bps = 1%)
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub oracle: Address,
    pub threshold_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtCfgUpd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub oracle: Address,
    pub threshold_bps: u32,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtSwapOk {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub token_in: Address,
    pub token_out: Address,
    pub amount_in: i128,
    pub market_price: i128,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtDevAlert {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub market_price: i128,
    pub intent_price: i128,
    pub deviation_bps: i128,
}

#[contract]
pub struct LiquidityVaultContract;

#[contractimpl]
impl LiquidityVaultContract {
    /// Initializes the liquidity-protected vault.
    pub fn initialize(env: Env, admin: Address, oracle: Address, threshold_bps: u32) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("Already initialized");
        }
        let config = Config { admin: admin.clone(), oracle: oracle.clone(), threshold_bps };
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("liqv"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin,
                admin: admin.clone(),
                oracle,
                threshold_bps,
            },
        );
    }

    /// Updates the configuration parameters. Only the admin can call this.
    pub fn update_config(env: Env, config: Config) {
        let current: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        current.admin.require_auth();
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("liqv"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: current.admin.clone(),
                admin: config.admin.clone(),
                oracle: config.oracle.clone(),
                threshold_bps: config.threshold_bps,
            },
        );
    }

    /// Executes a swap with on-chain liquidity protection.
    /// Rejects the transaction if the oracle price deviates from the intent price by more than X%.
    ///
    /// `intent_price`: The expected price of 1 unit of `token_in` in terms of `token_out`, 
    /// scaled to 8 decimal places (1e8).
    pub fn execute_protected_swap(
        env: Env,
        token_in: Address,
        token_out: Address,
        amount_in: i128,
        _min_amount_out: i128,
        intent_price: i128,
    ) {
        if amount_in <= 0 || intent_price <= 0 {
            panic!("Invalid parameters");
        }

        let config: Config = env.storage().instance().get(&DataKey::Config).expect("Not initialized");
        let oracle = PriceOracleClient::new(&env, &config.oracle);
        
        // 1. Fetch real-time market prices from the oracle
        let p_in = oracle.get_price(&token_in).expect("Oracle price missing for token_in");
        let p_out = oracle.get_price(&token_out).expect("Oracle price missing for token_out");
        
        // 2. Normalize both prices to 1e8 precision for comparison
        let p_in_norm = normalize_price(p_in.price, p_in.decimals, 8);
        let p_out_norm = normalize_price(p_out.price, p_out.decimals, 8);
        
        // 3. Calculate market exchange rate (Price of In relative to Out)
        // Rate = (PriceUSD_In / PriceUSD_Out) * 1e8
        let market_price = p_in_norm
            .checked_mul(100_000_000)
            .expect("Price math overflow")
            .checked_div(p_out_norm)
            .expect("Price math division error");
        
        // 4. Calculate deviation percentage in Basis Points (bps)
        let diff = if market_price > intent_price {
            market_price - intent_price
        } else {
            intent_price - market_price
        };
        
        let deviation_bps = diff
            .checked_mul(10000)
            .expect("Deviation math overflow")
            .checked_div(intent_price)
            .expect("Deviation math division error");
        
        // 5. Enforce protection threshold
        if deviation_bps > config.threshold_bps as i128 {
            env.events().publish(
                (symbol_short!("liqv"), symbol_short!("dev_alert")),
                EvtDevAlert {
                    version: 1,
                    ledger: env.ledger().sequence(),
                    actor: config.admin.clone(),
                    market_price,
                    intent_price,
                    deviation_bps,
                },
            );
            panic!("Liquidity Protection: Price deviation exceeds allowed threshold");
        }

        // Logic for actual swap execution would be called here (e.g. DEX Router)
        // For this task, we emit the verified swap event.
        env.events().publish(
            (symbol_short!("liqv"), symbol_short!("swap_ok")),
            EvtSwapOk {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: config.admin.clone(),
                token_in,
                token_out,
                amount_in,
                market_price,
            },
        );
    }
    
    /// Returns the current configuration.
    pub fn get_config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).expect("Not initialized")
    }
}

/// Normalizes a value from current_decimals to target_decimals using 10^diff factor.
fn normalize_price(price: i128, current_decimals: u32, target_decimals: u32) -> i128 {
    if current_decimals == target_decimals {
        return price;
    }
    if current_decimals < target_decimals {
        let diff = target_decimals - current_decimals;
        let factor = 10i128.pow(diff);
        price * factor
    } else {
        let diff = current_decimals - target_decimals;
        let factor = 10i128.pow(diff);
        price / factor
    }
}

mod test;
