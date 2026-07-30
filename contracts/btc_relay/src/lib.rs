#![no_std]

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, symbol_short, Address, Bytes, BytesN,
    Env, Vec,
};

const CLAIMED_TX_TTL_LEDGERS: u32 = 6_048_000;
const MAX_MERKLE_PROOF_DEPTH: u32 = 32;

#[contractclient(name = "BtcCryptoClient")]
pub trait BtcCryptoTrait {
    fn double_sha256(env: Env, data: Bytes) -> BytesN<32>;
    fn extract_merkle_root(env: Env, header: Bytes) -> BytesN<32>;
    fn extract_target(env: Env, header: Bytes) -> BytesN<32>;
    fn hash_meets_target(env: Env, hash: BytesN<32>, target: BytesN<32>) -> bool;
    fn compute_merkle_root(
        env: Env,
        tx_id: BytesN<32>,
        proof: Vec<BytesN<32>>,
        tx_index: u32,
    ) -> BytesN<32>;
}

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    Claimed(BytesN<32>),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    pub admin: Address,
    pub wrapped_btc_token: Address,
    pub min_confirmations: u32,
    pub crypto_contract: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtInit {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub wrapped_btc_token: Address,
    pub min_confirmations: u32,
    pub crypto_contract: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtCfgUpd {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub admin: Address,
    pub wrapped_btc_token: Address,
    pub min_confirmations: u32,
    pub crypto_contract: Address,
}

#[contracttype]
#[derive(Clone)]
pub struct EvtRelayOk {
    pub version: u32,
    pub ledger: u32,
    pub actor: Address,
    pub tx_id: BytesN<32>,
    pub recipient: Address,
    pub amount_sat: i128,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpvProof {
    pub block_header: Bytes,
    pub tx_id: BytesN<32>,
    pub merkle_proof: Vec<BytesN<32>>,
    pub tx_index: u32,
    pub amount_sat: i128,
    pub recipient: Address,
}

#[contract]
pub struct BtcRelayContract;

fn validate_config(config: &Config) {
    if config.min_confirmations == 0 || config.min_confirmations > MAX_MERKLE_PROOF_DEPTH {
        panic!("invalid confirmation requirement");
    }
}

#[contractimpl]
impl BtcRelayContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        wrapped_btc_token: Address,
        min_confirmations: u32,
        crypto_contract: Address,
    ) {
        if env.storage().instance().has(&DataKey::Config) {
            panic!("already initialized");
        }

        let config = Config {
            admin: admin.clone(),
            wrapped_btc_token: wrapped_btc_token.clone(),
            min_confirmations,
            crypto_contract: crypto_contract.clone(),
        };
        validate_config(&config);
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("btc"), symbol_short!("init")),
            EvtInit {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: admin.clone(),
                admin,
                wrapped_btc_token,
                min_confirmations,
                crypto_contract,
            },
        );
    }

    pub fn update_config(env: Env, config: Config) {
        let current: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");
        current.admin.require_auth();
        validate_config(&config);

        env.storage().instance().set(&DataKey::Config, &config);
        env.events().publish(
            (symbol_short!("btc"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: current.admin,
                admin: config.admin.clone(),
                wrapped_btc_token: config.wrapped_btc_token.clone(),
                min_confirmations: config.min_confirmations,
                crypto_contract: config.crypto_contract.clone(),
            },
        );
    }

    pub fn verify_and_claim(env: Env, proof: SpvProof) -> (Address, i128) {
        let config: Config = env
            .storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized");

        if proof.amount_sat <= 0 {
            panic!("amount must be positive");
        }
        if proof.block_header.len() != 80 {
            panic!("invalid block header length");
        }
        if proof.merkle_proof.len() < config.min_confirmations {
            panic!("insufficient merkle proof depth");
        }
        if proof.merkle_proof.len() > MAX_MERKLE_PROOF_DEPTH {
            panic!("merkle proof too deep");
        }
        if (proof.tx_index as u64) >= (1u64 << proof.merkle_proof.len()) {
            panic!("transaction index outside merkle proof");
        }

        let claimed_key = DataKey::Claimed(proof.tx_id.clone());
        if env.storage().persistent().has(&claimed_key) {
            panic!("tx already claimed");
        }

        let crypto = BtcCryptoClient::new(&env, &config.crypto_contract);
        let header_hash = crypto.double_sha256(&proof.block_header);
        let target = crypto.extract_target(&proof.block_header);
        if !crypto.hash_meets_target(&header_hash, &target) {
            panic!("block header fails proof-of-work check");
        }

        let merkle_root = crypto.extract_merkle_root(&proof.block_header);
        let computed_root = crypto.compute_merkle_root(
            &proof.tx_id,
            &proof.merkle_proof,
            &proof.tx_index,
        );
        if merkle_root != computed_root {
            panic!("merkle proof does not match block header");
        }

        env.storage().persistent().set(&claimed_key, &true);
        env.storage()
            .persistent()
            .extend_ttl(&claimed_key, CLAIMED_TX_TTL_LEDGERS, CLAIMED_TX_TTL_LEDGERS);

        env.events().publish(
            (symbol_short!("btc"), symbol_short!("relay_ok")),
            EvtRelayOk {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: proof.recipient.clone(),
                tx_id: proof.tx_id,
                recipient: proof.recipient.clone(),
                amount_sat: proof.amount_sat,
            },
        );

        (proof.recipient, proof.amount_sat)
    }

    pub fn get_config(env: Env) -> Config {
        env.storage()
            .instance()
            .get(&DataKey::Config)
            .expect("not initialized")
    }

    pub fn is_claimed(env: Env, tx_id: BytesN<32>) -> bool {
        env.storage().persistent().has(&DataKey::Claimed(tx_id))
    }
}
