#![no_std]
use soroban_sdk::{
    contract, contractimpl, contracttype, contractclient, symbol_short,
    Address, Bytes, BytesN, Env, Vec,
};

// TTL for claimed transaction records: ~30 days (6_048_000 ledgers at 5s/ledger)
// Keeps transaction history available for audit while allowing old records to expire
const CLAIMED_TX_TTL_LEDGERS: u32 = 6_048_000;

// ---------------------------------------------------------------------------
// BtcCrypto sub-contract client
// Heavy crypto helpers (double-SHA256, Merkle, PoW) live in btc_relay_crypto
// to keep this contract's Wasm bytecode within Soroban's size limit.
// ---------------------------------------------------------------------------
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

// ---------------------------------------------------------------------------
// Storage keys
// ---------------------------------------------------------------------------
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Config,
    /// Tracks which tx_ids have already been claimed (replay protection)
    Claimed(BytesN<32>),
}

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Config {
    /// Contract admin (can update config)
    pub admin: Address,
    /// Stellar token to mint/release when a valid BTC tx is proven
    pub wrapped_btc_token: Address,
    /// Minimum number of confirmations (merkle depth) required
    pub min_confirmations: u32,
    /// Address of the deployed btc_relay_crypto sub-contract
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

// ---------------------------------------------------------------------------
// SPV proof submitted by the relayer
// ---------------------------------------------------------------------------
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SpvProof {
    /// Raw 80-byte Bitcoin block header
    pub block_header: Bytes,
    /// Bitcoin transaction id (little-endian, 32 bytes)
    pub tx_id: BytesN<32>,
    /// Merkle proof: ordered list of 32-byte sibling hashes
    pub merkle_proof: Vec<BytesN<32>>,
    /// Index of the transaction in the block (used to determine left/right)
    pub tx_index: u32,
    /// Amount of satoshis locked in the BTC transaction
    pub amount_sat: i128,
    /// Stellar recipient address that should receive the wrapped asset
    pub recipient: Address,
}

// ---------------------------------------------------------------------------
// Contract
// ---------------------------------------------------------------------------
#[contract]
pub struct BtcRelayContract;

#[contractimpl]
impl BtcRelayContract {
    /// Initialize the relay.
    /// `crypto_contract` is the address of the deployed `btc_relay_crypto` sub-contract.
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
        env.storage().instance().set(
            &DataKey::Config,
            &Config { admin: admin.clone(), wrapped_btc_token: wrapped_btc_token.clone(), min_confirmations, crypto_contract: crypto_contract.clone() },
        );

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

    /// Update configuration. Admin only.
    pub fn update_config(env: Env, config: Config) {
        let current: Config = env.storage().instance().get(&DataKey::Config).expect("not initialized");
        current.admin.require_auth();
        env.storage().instance().set(&DataKey::Config, &config);

        env.events().publish(
            (symbol_short!("btc"), symbol_short!("cfg_upd")),
            EvtCfgUpd {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: current.admin.clone(),
                admin: config.admin.clone(),
                wrapped_btc_token: config.wrapped_btc_token.clone(),
                min_confirmations: config.min_confirmations,
                crypto_contract: config.crypto_contract.clone(),
            },
        );
    }

    /// Core SPV verification gate.
    ///
    /// Validates:
    ///   1. The block header has valid proof-of-work (hash ≤ target encoded in header).
    ///   2. The tx_id is committed to the block via the Merkle proof.
    ///   3. The proof depth meets `min_confirmations`.
    ///   4. The tx_id has not been claimed before (replay protection).
    ///
    /// On success, emits a `RelayOk` event and marks the tx as claimed.
    pub fn verify_and_claim(env: Env, proof: SpvProof) -> (Address, i128) {
        let config: Config = env.storage().instance().get(&DataKey::Config).expect("not initialized");
        let crypto = BtcCryptoClient::new(&env, &config.crypto_contract);

        // --- 1. Replay protection ---
        let claimed_key = DataKey::Claimed(proof.tx_id.clone());
        if env.storage().persistent().has(&claimed_key) {
            panic!("tx already claimed");
        }

        // --- 2. Validate block header length ---
        if proof.block_header.len() != 80 {
            panic!("invalid block header length");
        }

        // --- 3. Proof-of-Work check (delegated to crypto sub-contract) ---
        let header_hash = crypto.double_sha256(&proof.block_header);
        let target = crypto.extract_target(&proof.block_header);
        if !crypto.hash_meets_target(&header_hash, &target) {
            panic!("block header fails proof-of-work check");
        }

        // --- 4. Merkle proof depth check ---
        if proof.merkle_proof.len() < config.min_confirmations {
            panic!("insufficient merkle proof depth");
        }

        // --- 5. Merkle inclusion proof (delegated to crypto sub-contract) ---
        let merkle_root = crypto.extract_merkle_root(&proof.block_header);
        let computed_root = crypto.compute_merkle_root(
            &proof.tx_id,
            &proof.merkle_proof,
            &proof.tx_index,
        );
        if merkle_root != computed_root {
            panic!("merkle proof invalid: tx not in block");
        }

        // --- 6. Mark as claimed with TTL ---
        // Store claimed record with TTL to maintain audit trail while expiring old records
        env.storage().persistent().set_with_ttl(&claimed_key, &true, CLAIMED_TX_TTL_LEDGERS);

        // --- 7. Emit success event ---
        env.events().publish(
            (symbol_short!("btc"), symbol_short!("relay_ok")),
            EvtRelayOk {
                version: 1,
                ledger: env.ledger().sequence(),
                actor: config.admin.clone(),
                tx_id: proof.tx_id.clone(),
                recipient: proof.recipient.clone(),
                amount_sat: proof.amount_sat,
            },
        );

        (proof.recipient, proof.amount_sat)
    }

    /// Returns whether a given tx_id has already been claimed.
    pub fn is_claimed(env: Env, tx_id: BytesN<32>) -> bool {
        env.storage().persistent().has(&DataKey::Claimed(tx_id))
    }

    /// Returns the current config.
    pub fn get_config(env: Env) -> Config {
        env.storage().instance().get(&DataKey::Config).expect("not initialized")
    }
}

mod test;
